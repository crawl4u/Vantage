//! The client's side of Vantage's host calls. Each method answers from the client's own state;
//! anything not implemented here keeps the refusing default, which the Vantage code already
//! handles as a missing permission.

use std::{path::PathBuf, sync::Arc};
use ui::mod_panel::{HudIndex, Panel, PanelPatch, Surface, SurfacePatch};
use ui::mod_presentation as client_presentation;
use vantage::host::{
    budget, camera, clock, drawing, environment, facts, gameplay, hud, input, item_use, masks,
    panel, player_state, presentation, settings, terrain, touch,
};

/// Gameplay and presentation requests that stay in force until the guest changes them.
#[derive(Default)]
pub(super) struct GameplayOut {
    /// Held sprint and sneak.
    pub intents: Option<(bool, bool)>,
    /// Toggle sprint, toggle sneak and always sprint.
    pub modes: Option<(bool, bool, bool)>,
    pub fullbright: bool,
    /// Item delay fix: a use pulse asked this frame, until the client has offered it.
    pub use_pulse: bool,
    /// Center cursor: where the guest asked to put the pointer (0..1 of the window), until applied.
    pub pointer: Option<[f32; 2]>,
    /// The touch buttons the guest set this frame, until the client has published them.
    pub touch_buttons: Option<Vec<ui::mod_touch::TouchButton>>,
    /// Commands to send as the player, drained each frame.
    pub commands: Vec<String>,
    pub nametag_style: Option<client_presentation::NametagStyle>,
    pub nametag_rules: Vec<client_presentation::NametagRule>,
    pub tints: Vec<client_presentation::TintRule>,
    pub presentation_changed: bool,
    /// Visual rules for block faces, kept until the guest changes them.
    pub surface_rules: Vec<mod_api::terrain_surface::Rule>,
}

fn color(color: presentation::Rgba) -> [f32; 4] {
    [color.r, color.g, color.b, color.a]
}

fn selector(selector: presentation::ActorSelector) -> client_presentation::ActorSelector {
    use client_presentation::ActorSelector as S;
    match selector {
        presentation::ActorSelector::All => S::All,
        presentation::ActorSelector::Players => S::Players,
        presentation::ActorSelector::NonPlayers => S::NonPlayers,
        presentation::ActorSelector::LocalPlayer => S::LocalPlayer,
        presentation::ActorSelector::Riding => S::Riding,
        presentation::ActorSelector::Actor(id) => S::Actor(id),
    }
}

/// What the guest asked the presenter to do this frame. Each call replaces the last, and a frame
/// that makes none leaves the matching setting at its default.
#[derive(Clone, Copy, Default, PartialEq)]
pub(super) struct Outputs {
    pub yaw_offset: Option<f32>,
    /// 0 first person, 1 third person back, 2 third person front.
    pub perspective: Option<u8>,
    /// Field-of-view and look-speed scale.
    pub view_scale: Option<[f32; 2]>,
    /// Roll, pitch and whether the hurt shake is suppressed.
    pub tilt: Option<(f32, f32, bool)>,
    pub time: Option<u32>,
    /// Rain and thunder intensity.
    pub weather: Option<[f32; 2]>,
}

impl Outputs {
    /// Whether any request is in force.
    pub fn any(&self) -> bool {
        *self != Self::default()
    }
}

/// The surface the guest last set, and whether it changed this frame.
#[derive(Default)]
pub(super) struct SurfaceSlot {
    pub current: Option<(Arc<Surface>, HudIndex)>,
    pub changed: bool,
}

/// The menu panel the guest set, the catalog its pages are written against, and its open state.
#[derive(Default)]
pub(super) struct PanelSlot {
    pub panel: Option<Panel>,
    pub changed: bool,
    /// An open or close the guest asked for, until the client has applied it.
    pub open_request: Option<bool>,
    pub open: bool,
}

thread_local! {
    static METRICS: std::cell::RefCell<Option<Arc<mod_ui::LabelMetrics>>> = const { std::cell::RefCell::new(None) };
}

/// Installs the client's label metrics for the guest's text measurements.
pub(super) fn set_metrics(metrics: Option<Arc<mod_ui::LabelMetrics>>) {
    METRICS.with_borrow_mut(|slot| *slot = metrics);
}

/// One frame's view of the client, lent to the Vantage code while it runs.
pub(super) struct Frame<'a> {
    pub native: &'a mut super::NativePanel,
    pub observed: &'a super::observe::Observed,
    pub out: &'a mut Outputs,
    pub gp: &'a mut GameplayOut,
    /// The presented camera, the block under the crosshair and the held controls.
    pub facts: &'a mod_host::GameplayFacts,
    pub draw: &'a mut super::draw::NativeDrawing,
    pub terrain: &'a mut super::super::terrain::NativeTerrain,
    /// The player's own bindings as last sampled, and the changes asked of them this frame.
    pub bindings: Option<Arc<mod_host::InputBindings>>,
    pub writes: &'a mut Vec<mod_host::InputBindingWrite>,
    pub damage_shake: Option<(f32, f32)>,
    /// The installation's food table, for item facts.
    pub food: Option<Arc<assets::RuntimeEquipmentCatalog>>,
    /// The game's own HUD elements as last laid out.
    pub elements: ui::mod_hud::ElementStates,
    /// The game HUD elements Vantage asked to hide, and whether that changed this frame.
    pub hidden: &'a mut (ui::mod_hud::HiddenElements, bool),
    pub fps: f32,
    /// GUI width, height and scale of the laid-out HUD root.
    pub viewport: Option<[f32; 3]>,
    pub settings_file: &'a PathBuf,
    pub surface: &'a mut SurfaceSlot,
    pub panel: &'a mut PanelSlot,
    /// Where the open menu was last laid out, and its last pointer press.
    pub layout: Option<ui::mod_hud::PanelLayout>,
    pub press: Option<ui::mod_hud::PanelPress>,
    /// Chat lines already said, so a line the guest repeats every frame is said once.
    pub said: &'a mut std::collections::HashSet<String>,
    /// The open container screen, if any.
    pub container: Option<&'static str>,
    /// The taps on Vantage's touch buttons since its last frame.
    pub touch_taps: Vec<String>,
}

impl hud::Api for Frame<'_> {
    fn set_surface(&mut self, json: &str) -> Result<(), String> {
        if json.is_empty() {
            self.surface.current = None;
        } else {
            let surface: Surface =
                serde_json::from_str(json).map_err(|e| format!("invalid HUD surface: {e}"))?;
            let index = HudIndex::new(&surface)?;
            self.surface.current = Some((Arc::new(surface), index));
        }
        self.surface.changed = true;
        Ok(())
    }

    fn update_surface(&mut self, json: &str) -> Result<(), String> {
        let patch = SurfacePatch::parse(json)?;
        let (surface, index) = self
            .surface
            .current
            .as_ref()
            .ok_or("no HUD surface to update")?;
        let (next, index) = patch.apply_hud(surface, index)?;
        self.surface.current = Some((Arc::new(next), index));
        self.surface.changed = true;
        Ok(())
    }

    fn read_viewport(&mut self) -> Option<hud::Viewport> {
        self.viewport.map(|[width, height, scale]| hud::Viewport {
            width,
            height,
            scale,
        })
    }

    fn measure_text(&mut self, text: &str, _font: hud::Font) -> Result<f32, String> {
        METRICS
            .with_borrow(|metrics| {
                metrics
                    .as_ref()?
                    .width(text, ui::mod_panel::LabelFont::Native)
            })
            .ok_or_else(|| "text metrics are not ready".to_owned())
    }

    fn read_elements(&mut self) -> Result<Vec<hud::ElementState>, String> {
        Ok(ui::mod_hud::VanillaElement::ALL
            .into_iter()
            .map(|kind| {
                let state = self.elements[kind.index()];
                hud::ElementState {
                    element: wire_element(kind),
                    visible: state.visible,
                    bounds: state.bounds.map(|[x, y, width, height]| hud::Rect {
                        x,
                        y,
                        width,
                        height,
                    }),
                }
            })
            .collect())
    }

    fn set_hidden_elements(&mut self, elements: &[hud::VanillaElement]) -> Result<(), String> {
        let next = ui::mod_hud::HiddenElements::from_elements(
            elements.iter().copied().map(unwire_element),
        );
        if next != self.hidden.0 {
            *self.hidden = (next, true);
        }
        Ok(())
    }

    fn local_message(&mut self, text: &str) -> Result<(), String> {
        if self.said.len() < 512 && self.said.insert(text.to_owned()) {
            bevy::log::info!("VANTAGE {text}");
        }
        Ok(())
    }
}

impl player_state::Api for Frame<'_> {
    fn read_fps(&mut self) -> Result<Option<f32>, String> {
        Ok((self.fps > 0.0).then_some(self.fps))
    }

    fn read_motion(&mut self) -> Result<Option<player_state::Motion>, String> {
        Ok(self.observed.motion.clone())
    }

    fn read_vitals(&mut self) -> Result<Option<player_state::Vitals>, String> {
        Ok(self.observed.vitals.clone())
    }

    fn read_world_time(&mut self) -> Result<Option<player_state::WorldTime>, String> {
        Ok(self.observed.world_time.clone())
    }

    fn read_ping(&mut self) -> Result<Option<u32>, String> {
        Ok(self.observed.ping_ms)
    }

    fn read_world(&mut self) -> Result<Option<player_state::WorldObservation>, String> {
        Ok(self.observed.world.clone())
    }

    fn read_effects(&mut self) -> Result<Option<player_state::Effects>, String> {
        Ok(self.observed.effects.clone())
    }

    fn read_equipment(&mut self) -> Result<Option<player_state::Equipment>, String> {
        Ok(self.observed.equipment.clone())
    }

    fn read_actors_limited(
        &mut self,
        limit: u32,
    ) -> Result<Vec<player_state::ObservedActor>, String> {
        if limit == 0 {
            return Err("actor observation limit exceeds bounds".into());
        }
        Ok(self
            .observed
            .actors
            .iter()
            .take(limit as usize)
            .cloned()
            .collect())
    }

    fn read_combat(
        &mut self,
        after: u64,
        limit: u32,
    ) -> Result<Vec<player_state::CombatEvent>, String> {
        if limit == 0 {
            return Err("combat import limit exceeds bounds".into());
        }
        Ok(self
            .observed
            .combat
            .iter()
            .filter(|event| event.sequence > after)
            .take(limit as usize)
            .cloned()
            .collect())
    }

    fn read_chat(
        &mut self,
        after: u64,
        limit: u32,
    ) -> Result<Vec<player_state::ChatMessage>, String> {
        if limit == 0 {
            return Err("chat import limit exceeds bounds".into());
        }
        Ok(self
            .observed
            .chat
            .iter()
            .filter(|message| message.sequence > after)
            .take(limit as usize)
            .cloned()
            .collect())
    }

    fn read_sidebar(&mut self) -> Result<Option<player_state::Sidebar>, String> {
        Ok(self.observed.sidebar.clone())
    }
}

impl settings::Api for Frame<'_> {
    fn load(&mut self) -> Result<String, String> {
        std::fs::read_to_string(self.settings_file).map_err(|e| e.to_string())
    }

    fn save(&mut self, json: &str) -> Result<(), String> {
        std::fs::write(self.settings_file, json).map_err(|e| e.to_string())
    }
}

impl input::Api for Frame<'_> {
    fn read_container_screen(&mut self) -> Result<Option<String>, String> {
        Ok(self.container.map(str::to_owned))
    }

    fn position_pointer(&mut self, x: f32, y: f32) -> Result<(), String> {
        if self.container.is_none() {
            return Err("pointer moves need an open container screen".to_owned());
        }
        if !x.is_finite() || !y.is_finite() {
            return Err("pointer position must be finite".to_owned());
        }
        self.gp.pointer = Some([x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)]);
        Ok(())
    }

    fn read_selected_controls(
        &mut self,
        selection: &input::Selection,
    ) -> Result<input::Controls, String> {
        let pick = |keys: &[String], names: &Option<Vec<String>>| -> Vec<String> {
            match names {
                None => keys.to_vec(),
                Some(names) => keys
                    .iter()
                    .filter(|key| names.contains(key))
                    .cloned()
                    .collect(),
            }
        };
        let frame = self.native.controls.clone();
        if !frame.keys_held.is_empty() || !frame.keys_pressed.is_empty() {
            let line = format!(
                "controls held={:?} pressed={:?} focused={} panel_open={} asked_held={:?}",
                frame.keys_held,
                frame.keys_pressed,
                frame.focused,
                self.panel.open,
                selection.keys_held
            );
            if self.said.len() < 512 && self.said.insert(line.clone()) {
                bevy::log::info!("VANTAGE_INPUT {line}");
            }
        }
        Ok(input::Controls {
            seconds: frame.seconds,
            focused: frame.focused,
            gameplay: frame.gameplay,
            panel_open: self.panel.open,
            keys_pressed: pick(&frame.keys_pressed, &selection.keys_pressed),
            keys_held: pick(&frame.keys_held, &selection.keys_held),
            keys_admitted: pick(&frame.keys_admitted, &selection.keys_admitted),
            events: if selection.events {
                frame
                    .events
                    .iter()
                    .map(|event| input::Event {
                        id: event.id.clone(),
                        value: event.value,
                    })
                    .collect()
            } else {
                Vec::new()
            },
            pointer: frame.pointer,
            wheel: frame.wheel,
        })
    }

    fn reserve_keys(&mut self, keys: &[String]) -> Result<(), String> {
        self.native.reserved = keys.to_vec();
        Ok(())
    }

    fn reserve_scroll(&mut self, keys: &[String]) -> Result<(), String> {
        self.native.scroll_keys = keys.to_vec();
        Ok(())
    }

    fn read_scroll(&mut self) -> Result<input::Scroll, String> {
        let [line_x, line_y, pixel_x, pixel_y] = std::mem::take(&mut self.native.scroll);
        Ok(input::Scroll {
            line_x,
            line_y,
            pixel_x,
            pixel_y,
        })
    }

    fn read_native_editing(&mut self) -> Result<bool, String> {
        Ok(false)
    }

    fn read_field_events(&mut self) -> Result<Vec<input::FieldEvent>, String> {
        use ui::mod_panel::FieldEvent;
        Ok(std::mem::take(&mut self.native.field_events)
            .into_iter()
            .map(|event| match event {
                FieldEvent::Change(e) => input::FieldEvent::Change(input::TextEvent {
                    id: e.id,
                    value: e.value,
                }),
                FieldEvent::Submit(e) => input::FieldEvent::Submit(input::TextEvent {
                    id: e.id,
                    value: e.value,
                }),
                FieldEvent::Key(e) => input::FieldEvent::Key(input::FieldKey {
                    id: e.id,
                    key: e.key,
                    shift: e.modifiers.shift,
                    control: e.modifiers.control,
                    alt: e.modifiers.alt,
                    super_key: e.modifiers.super_key,
                }),
            })
            .collect())
    }

    fn read_bindings(
        &mut self,
        after: Option<u64>,
        offset: u32,
        limit: u32,
    ) -> Result<Option<input::Bindings>, String> {
        let bindings = self.bindings.as_ref().ok_or("input bindings unavailable")?;
        let start = offset as usize;
        if start > bindings.entries.len() || limit == 0 {
            return Err("invalid input bindings page".into());
        }
        if after == Some(bindings.revision) {
            return Ok(None);
        }
        let end = (start + limit as usize).min(bindings.entries.len());
        Ok(Some(input::Bindings {
            revision: bindings.revision,
            total: bindings.total,
            entries: bindings.entries[start..end]
                .iter()
                .map(|entry| input::ActionBinding {
                    action: entry.action.clone(),
                    context: match entry.context {
                        mod_host::BindingContext::Gameplay => input::BindingContext::Gameplay,
                        mod_host::BindingContext::Ui => input::BindingContext::Ui,
                    },
                    key: entry.key.clone(),
                    modifiers: input::BindingModifiers {
                        shift: entry.modifiers.shift,
                        control: entry.modifiers.control,
                        alt: entry.modifiers.alt,
                        super_key: entry.modifiers.super_key,
                    },
                })
                .collect(),
        }))
    }

    fn write_binding(&mut self, action: &str, change: &input::BindingChange) -> Result<(), String> {
        let change = match change {
            input::BindingChange::Assign(key) => mod_host::InputBindingChange::Assign(key.clone()),
            input::BindingChange::Unbind => mod_host::InputBindingChange::Unbind,
            input::BindingChange::Reset => mod_host::InputBindingChange::Reset,
        };
        // One change per action, the latest, as the guest intends.
        self.writes.retain(|write| write.action != action);
        self.writes.push(mod_host::InputBindingWrite {
            action: action.to_owned(),
            change,
        });
        Ok(())
    }
}

impl panel::Api for Frame<'_> {
    fn open(&mut self) -> Result<(), String> {
        self.panel.open_request = Some(true);
        Ok(())
    }

    fn close(&mut self) -> Result<(), String> {
        self.panel.open_request = Some(false);
        Ok(())
    }

    fn set_content(&mut self, json: &str) -> Result<(), String> {
        if json.is_empty() {
            self.panel.panel = None;
        } else {
            let panel: Panel =
                serde_json::from_str(json).map_err(|e| format!("invalid mod panel: {e}"))?;
            panel
                .validate()
                .map_err(|e| format!("invalid mod panel: {e}"))?;
            self.panel.panel = Some(panel);
        }
        self.panel.changed = true;
        Ok(())
    }

    fn update_content(&mut self, json: &str) -> Result<(), String> {
        let current = self
            .panel
            .panel
            .as_ref()
            .ok_or("panel update requires retained content")?;
        let patch = PanelPatch::parse(json)?;
        let next = patch
            .apply(current, &std::collections::BTreeMap::new())
            .map_err(|e| format!("invalid panel update: {e}"))?;
        self.panel.panel = Some(next);
        self.panel.changed = true;
        Ok(())
    }

    fn read_layout(&mut self) -> Option<panel::PanelLayout> {
        self.layout
            .filter(|_| self.panel.open)
            .map(|layout| panel::PanelLayout {
                width: layout.size[0],
                height: layout.size[1],
                scale: layout.scale,
                offset: (layout.offset[0], layout.offset[1]),
            })
    }

    fn read_press(&mut self) -> Option<panel::PanelPress> {
        let press = self.press.as_ref().filter(|_| self.panel.open)?;
        let [a, b, c, d, e, f] = press.to_local;
        Some(panel::PanelPress {
            id: press.id.clone(),
            x: press.rect[0],
            y: press.rect[1],
            width: press.rect[2],
            height: press.rect[3],
            held: press.held,
            to_local: panel::LocalMap { a, b, c, d, e, f },
        })
    }
}

/// The guest's name for a game HUD element.
fn wire_element(element: ui::mod_hud::VanillaElement) -> hud::VanillaElement {
    use ui::mod_hud::VanillaElement as E;
    match element {
        E::Crosshair => hud::VanillaElement::Crosshair,
        E::StatusEffects => hud::VanillaElement::StatusEffects,
        E::Sidebar => hud::VanillaElement::Sidebar,
        E::Hunger => hud::VanillaElement::Hunger,
        E::Position => hud::VanillaElement::Position,
    }
}

/// The client's name for a game HUD element the guest names.
fn unwire_element(element: hud::VanillaElement) -> ui::mod_hud::VanillaElement {
    use ui::mod_hud::VanillaElement as E;
    match element {
        hud::VanillaElement::Crosshair => E::Crosshair,
        hud::VanillaElement::StatusEffects => E::StatusEffects,
        hud::VanillaElement::Sidebar => E::Sidebar,
        hud::VanillaElement::Hunger => E::Hunger,
        hud::VanillaElement::Position => E::Position,
    }
}

impl facts::Api for Frame<'_> {
    fn read_item(
        &mut self,
        identifier: &str,
        metadata: u32,
    ) -> Result<Option<facts::ItemFacts>, String> {
        Ok(
            mod_host::item_facts(identifier, metadata, self.food.as_deref()).map(|item| {
                facts::ItemFacts {
                    max_durability: item.max_durability,
                    food: item.food.map(|food| facts::Food {
                        nutrition: food.nutrition,
                        saturation_modifier: food.saturation_modifier,
                    }),
                    potion_effects: item
                        .potion_effects
                        .into_iter()
                        .map(|effect| facts::PotionEffect {
                            effect_id: effect.effect_id,
                            amplifier: effect.amplifier,
                            duration_ticks: effect.duration_ticks,
                        })
                        .collect(),
                }
            }),
        )
    }

    fn read_effect(&mut self, effect_id: i32) -> Result<Option<facts::EffectFacts>, String> {
        Ok(
            mod_host::effect_facts(effect_id).map(|effect| facts::EffectFacts {
                name_key: effect.name_key,
                texture: effect.texture,
            }),
        )
    }
}

impl camera::Api for Frame<'_> {
    fn set_view_tilt(&mut self, tilt: Option<camera::ViewTilt>) -> Result<(), String> {
        self.out.tilt = tilt.map(|tilt| (tilt.roll, tilt.pitch, tilt.suppress_hurt_shake));
        Ok(())
    }

    fn set_view_scale(&mut self, fov_scale: f32, look_scale: f32) -> Result<(), String> {
        self.out.view_scale = ((fov_scale - 1.0).abs() > f32::EPSILON
            || (look_scale - 1.0).abs() > f32::EPSILON)
            .then_some([fov_scale, look_scale]);
        Ok(())
    }

    fn set_perspective(&mut self, view: Option<camera::Perspective>) -> Result<(), String> {
        self.out.perspective = view.map(|view| match view {
            camera::Perspective::FirstPerson => 0,
            camera::Perspective::ThirdPersonBack => 1,
            camera::Perspective::ThirdPersonFront => 2,
        });
        Ok(())
    }

    fn set_view_yaw_offset(&mut self, degrees: f32) -> Result<(), String> {
        self.out.yaw_offset = (degrees != 0.0).then_some(degrees);
        Ok(())
    }

    fn read_damage_shake(&mut self) -> Result<Option<camera::DamageShake>, String> {
        Ok(self
            .damage_shake
            .map(|(roll, pitch)| camera::DamageShake { roll, pitch }))
    }
}

impl environment::Api for Frame<'_> {
    fn set_time_override(&mut self, ticks: Option<u32>) -> Result<(), String> {
        self.out.time = ticks;
        Ok(())
    }

    fn set_fullbright(&mut self, enabled: bool) -> Result<(), String> {
        self.gp.fullbright = enabled;
        Ok(())
    }

    fn set_weather_override(&mut self, value: Option<environment::Weather>) -> Result<(), String> {
        self.out.weather = value.map(|weather| match weather {
            environment::Weather::Clear => [0.0, 0.0],
            environment::Weather::Rain => [1.0, 0.0],
            environment::Weather::Thunder => [1.0, 1.0],
        });
        Ok(())
    }
}

impl gameplay::Api for Frame<'_> {
    fn read_camera(&mut self) -> Result<Option<gameplay::CameraPose>, String> {
        Ok(self.facts.camera.map(|pose| gameplay::CameraPose {
            position: gameplay::Vector3 {
                x: pose.position.x,
                y: pose.position.y,
                z: pose.position.z,
            },
            yaw: pose.yaw,
            pitch: pose.pitch,
        }))
    }

    fn read_block_target(&mut self) -> Result<Option<gameplay::Vector3>, String> {
        Ok(self.facts.block_target.map(|target| gameplay::Vector3 {
            x: target.x,
            y: target.y,
            z: target.z,
        }))
    }

    fn read_movement_input(&mut self) -> Result<Option<gameplay::MovementInput>, String> {
        Ok(self
            .facts
            .movement_input
            .map(|input| gameplay::MovementInput {
                right: input.right,
                forward: input.forward,
                jump: input.jump,
                sneak: input.sneak,
                sprint: input.sprint,
            }))
    }

    fn set_movement_intents(
        &mut self,
        intents: Option<gameplay::MovementIntents>,
    ) -> Result<(), String> {
        self.gp.intents = intents.map(|intents| (intents.sprint, intents.sneak));
        Ok(())
    }

    fn set_control_modes(&mut self, modes: Option<gameplay::ControlModes>) -> Result<(), String> {
        self.gp.modes =
            modes.map(|modes| (modes.toggle_sprint, modes.toggle_sneak, modes.always_sprint));
        Ok(())
    }

    fn request_command(&mut self, command: &str) -> Result<(), String> {
        if command.is_empty() || command.len() > 256 || command.chars().any(char::is_control) {
            return Err("a command is one short line".into());
        }
        if self.gp.commands.len() >= 8 {
            return Err("too many commands in one frame".into());
        }
        self.gp.commands.push(command.to_owned());
        Ok(())
    }
}

impl presentation::Api for Frame<'_> {
    fn set_nametag_style(
        &mut self,
        style: Option<presentation::NametagStyle>,
    ) -> Result<(), String> {
        self.gp.nametag_style = style.map(|style| client_presentation::NametagStyle {
            opacity: style.opacity,
            scale: style.scale,
            shadow: style.shadow,
            border_width: style.border_width,
            text_color: style.text_color.map(color),
            own_text_color: style.own_text_color.map(color),
            border_color: style.border_color.map(color),
            background_color: style.background_color.map(color),
        });
        self.gp.presentation_changed = true;
        Ok(())
    }

    fn set_nametag_rules(&mut self, rules: &[presentation::NametagRule]) -> Result<(), String> {
        self.gp.nametag_rules = rules
            .iter()
            .map(|rule| client_presentation::NametagRule {
                actors: selector(rule.actors),
                hud_hidden: rule.hud_hidden,
                visibility: match rule.visibility {
                    presentation::NametagVisibility::Native => {
                        client_presentation::NametagVisibility::Native
                    }
                    presentation::NametagVisibility::Shown => {
                        client_presentation::NametagVisibility::Shown
                    }
                    presentation::NametagVisibility::Hidden => {
                        client_presentation::NametagVisibility::Hidden
                    }
                },
            })
            .collect();
        self.gp.presentation_changed = true;
        Ok(())
    }

    fn set_actor_tints(&mut self, tints: &[presentation::ActorTint]) -> Result<(), String> {
        self.gp.tints = tints
            .iter()
            .map(|rule| client_presentation::TintRule {
                actors: selector(rule.actors),
                state: match rule.state {
                    presentation::TintState::Hurt => client_presentation::TintState::Hurt,
                    presentation::TintState::NotHurt => client_presentation::TintState::NotHurt,
                },
                tint: client_presentation::Tint {
                    body: rule.body.map(color),
                    armor: rule.armor.map(color),
                },
            })
            .collect();
        self.gp.presentation_changed = true;
        Ok(())
    }
}

/// Native code runs without a fuel allowance, so the guest's fuel guard always has room.
impl budget::Api for Frame<'_> {
    fn remaining_fuel(&mut self) -> u64 {
        1 << 40
    }
}

impl clock::Api for Frame<'_> {
    fn read_local_time(&mut self) -> Result<clock::LocalTime, String> {
        let now = chrono::Local::now();
        Ok(clock::LocalTime {
            unix_ms: u64::try_from(now.timestamp_millis()).unwrap_or(0),
            utc_offset_seconds: chrono::Offset::fix(now.offset()).local_minus_utc(),
        })
    }
}

impl item_use::Api for Frame<'_> {
    /// Asks for one use press edge, only while the physical use control is held.
    fn pulse_use(&mut self) -> Result<(), String> {
        if !self.facts.use_held {
            return Err("use pulse requires a held current gameplay use".to_owned());
        }
        self.gp.use_pulse = true;
        Ok(())
    }
}

impl touch::Api for Frame<'_> {
    fn read_presses(&mut self) -> Result<Vec<String>, String> {
        Ok(self.touch_taps.clone())
    }

    fn set_buttons(&mut self, buttons: &[touch::Button]) -> Result<(), String> {
        use ui::mod_hud::Anchor;
        let buttons: Vec<_> = buttons
            .iter()
            .map(|button| ui::mod_touch::TouchButton {
                id: button.id.clone(),
                label: button.label.clone(),
                anchor: match button.anchor {
                    touch::Anchor::TopLeft => Anchor::TopLeft,
                    touch::Anchor::TopCenter => Anchor::TopCenter,
                    touch::Anchor::TopRight => Anchor::TopRight,
                    touch::Anchor::Left => Anchor::Left,
                    touch::Anchor::Center => Anchor::Center,
                    touch::Anchor::Right => Anchor::Right,
                    touch::Anchor::BottomLeft => Anchor::BottomLeft,
                    touch::Anchor::BottomCenter => Anchor::BottomCenter,
                    touch::Anchor::BottomRight => Anchor::BottomRight,
                },
                offset: [button.offset.x, button.offset.y],
                size: [button.size.x, button.size.y],
            })
            .collect();
        ui::mod_touch::validate(&buttons)?;
        self.gp.touch_buttons = Some(buttons);
        Ok(())
    }
}

/// The Vantage host modules this client leaves at their refusing defaults.
macro_rules! refuse {
    ($($module:ident),*) => {$(impl vantage::host::$module::Api for Frame<'_> {})*};
}
refuse!(
    action_filter,
    events,
    render,
    screen,
    sound_handling,
    teleport_handling
);

impl drawing::Api for Frame<'_> {
    fn capabilities(&mut self) -> drawing::Permissions {
        drawing::Api::capabilities(&mut *self.draw)
    }

    fn create_geometry(
        &mut self,
        spec: &drawing::GeometrySpec,
    ) -> Result<drawing::Handle, drawing::DrawError> {
        drawing::Api::create_geometry(&mut *self.draw, spec)
    }

    fn create(&mut self, spec: &drawing::Instance) -> Result<drawing::Handle, drawing::DrawError> {
        drawing::Api::create(&mut *self.draw, spec)
    }

    fn update(
        &mut self,
        id: drawing::Handle,
        spec: &drawing::Instance,
    ) -> Result<(), drawing::DrawError> {
        drawing::Api::update(&mut *self.draw, id, spec)
    }

    fn replace_frame_group(
        &mut self,
        name: &str,
        draws: &[drawing::Instance],
    ) -> Result<(), drawing::DrawError> {
        drawing::Api::replace_frame_group(&mut *self.draw, name, draws)
    }

    fn remove(&mut self, id: drawing::Handle) -> Result<(), drawing::DrawError> {
        drawing::Api::remove(&mut *self.draw, id)
    }

    fn observe_entities(&mut self) -> Result<Vec<drawing::ObservedEntity>, drawing::DrawError> {
        drawing::Api::observe_entities(&mut *self.draw)
    }
}

impl masks::Api for Frame<'_> {
    fn create(&mut self, depth: masks::DepthPolicy) -> Result<drawing::Handle, drawing::DrawError> {
        masks::Api::create(&mut *self.draw, depth)
    }

    fn entity(&mut self, runtime_id: u64) -> Result<drawing::Handle, drawing::DrawError> {
        masks::Api::entity(&mut *self.draw, runtime_id)
    }

    fn select(
        &mut self,
        mask: drawing::Handle,
        entities: &[drawing::Handle],
    ) -> Result<(), drawing::DrawError> {
        masks::Api::select(&mut *self.draw, mask, entities)
    }

    fn remove(&mut self, mask: drawing::Handle) -> Result<(), drawing::DrawError> {
        masks::Api::remove(&mut *self.draw, mask)
    }

    fn set_style(
        &mut self,
        mask: drawing::Handle,
        appearance: Option<masks::Style>,
    ) -> Result<(), drawing::DrawError> {
        masks::Api::set_style(&mut *self.draw, mask, appearance)
    }
}

fn host_position(position: terrain::Position) -> mod_host::TerrainPosition {
    mod_host::TerrainPosition {
        x: position.x,
        y: position.y,
        z: position.z,
    }
}

fn guest_position(position: mod_host::TerrainPosition) -> terrain::Position {
    terrain::Position {
        x: position.x,
        y: position.y,
        z: position.z,
    }
}

fn guest_error(error: mod_host::TerrainReadError) -> terrain::ReadError {
    use mod_host::TerrainReadError as E;
    match error {
        E::Denied => terrain::ReadError::Denied,
        E::Unavailable => terrain::ReadError::Unavailable,
        E::InvalidQuery => terrain::ReadError::InvalidQuery,
        E::OutOfRange => terrain::ReadError::OutOfRange,
        E::BudgetExhausted => terrain::ReadError::BudgetExhausted,
        E::LimitExceeded => terrain::ReadError::LimitExceeded,
    }
}

fn guest_shape(shape: mod_host::TerrainShape) -> terrain::ResolvedShape {
    match shape {
        mod_host::TerrainShape::Unavailable => terrain::ResolvedShape::Unavailable,
        mod_host::TerrainShape::Known(index) => terrain::ResolvedShape::Known(index),
    }
}

fn guest_binding(binding: &mod_host::TerrainShapeBinding) -> terrain::ShapeBinding {
    match binding {
        mod_host::TerrainShapeBinding::NotRequested => terrain::ShapeBinding::NotRequested,
        mod_host::TerrainShapeBinding::Fixed(shape) => {
            terrain::ShapeBinding::Fixed(guest_shape(*shape))
        }
        mod_host::TerrainShapeBinding::PerCell => terrain::ShapeBinding::PerCell,
    }
}

fn guest_flags(flags: mod_host::TerrainPhysicsFlags) -> terrain::PhysicsFlags {
    use mod_host::TerrainPhysicsFlags as H;
    let mut out = terrain::PhysicsFlags::empty();
    for (host, guest) in [
        (H::CLIMBABLE, terrain::PhysicsFlags::CLIMBABLE),
        (H::WATER, terrain::PhysicsFlags::WATER),
        (H::LAVA, terrain::PhysicsFlags::LAVA),
        (H::COBWEB, terrain::PhysicsFlags::COBWEB),
        (H::POWDER_SNOW, terrain::PhysicsFlags::POWDER_SNOW),
        (H::SCAFFOLDING, terrain::PhysicsFlags::SCAFFOLDING),
        (H::PASSABLE, terrain::PhysicsFlags::PASSABLE),
    ] {
        if flags.contains(host) {
            out |= guest;
        }
    }
    out
}

fn host_geometry(geometry: terrain::Geometry) -> mod_host::TerrainGeometry {
    let mut out = mod_host::TerrainGeometry::empty();
    if geometry.contains(terrain::Geometry::COLLISION) {
        out |= mod_host::TerrainGeometry::COLLISION;
    }
    if geometry.contains(terrain::Geometry::SELECTION) {
        out |= mod_host::TerrainGeometry::SELECTION;
    }
    out
}

fn guest_stamp(stamp: mod_host::TerrainStamp) -> terrain::Stamp {
    terrain::Stamp {
        session: stamp.session,
        dimension: stamp.dimension,
        registry: stamp.registry,
        revision: stamp.revision,
    }
}

fn guest_region(region: &mod_host::TerrainRegion) -> terrain::Region {
    terrain::Region {
        stamp: guest_stamp(region.stamp),
        chunks: region
            .chunks
            .iter()
            .map(|chunk| terrain::ChunkRevision {
                x: chunk.x,
                z: chunk.z,
                revision: chunk.revision,
            })
            .collect(),
        layer_count: region.layer_count,
        loaded: region.loaded.clone(),
        states: region.states.clone(),
        palette: region
            .palette
            .iter()
            .map(|state| terrain::BlockState {
                runtime_id: state.runtime_id,
                identifier: state.identifier.clone(),
                canonical_state: state.canonical_state.clone(),
                tags: state.tags.clone(),
                physics: state.physics.as_ref().map(|physics| terrain::Physics {
                    friction: physics.friction,
                    horizontal_speed_factor: physics.horizontal_speed_factor,
                    vertical_speed_factor: physics.vertical_speed_factor,
                    fluid_height: physics.fluid_height,
                    flags: guest_flags(physics.flags),
                    surface: match physics.surface {
                        mod_host::TerrainSurfaceResponse::None => terrain::SurfaceResponse::None,
                        mod_host::TerrainSurfaceResponse::Slime => terrain::SurfaceResponse::Slime,
                        mod_host::TerrainSurfaceResponse::Bed => terrain::SurfaceResponse::Bed,
                        mod_host::TerrainSurfaceResponse::Honey => terrain::SurfaceResponse::Honey,
                        mod_host::TerrainSurfaceResponse::SoulSand => {
                            terrain::SurfaceResponse::SoulSand
                        }
                        mod_host::TerrainSurfaceResponse::BubbleUp => {
                            terrain::SurfaceResponse::BubbleUp
                        }
                        mod_host::TerrainSurfaceResponse::BubbleDown => {
                            terrain::SurfaceResponse::BubbleDown
                        }
                    },
                }),
                collision: guest_binding(&state.collision),
                selection: guest_binding(&state.selection),
            })
            .collect(),
        shape_offsets: region.shape_offsets.clone(),
        shape_boxes: region.shape_boxes.clone(),
        overrides: region
            .overrides
            .iter()
            .map(|entry| terrain::ShapeOverride {
                slot: entry.slot,
                collision: entry.collision.map(guest_shape),
                selection: entry.selection.map(guest_shape),
            })
            .collect(),
    }
}

impl terrain::Api for Frame<'_> {
    fn read_region(
        &mut self,
        query: terrain::RegionQuery,
        after: Option<terrain::Stamp>,
    ) -> Result<terrain::ReadResult, terrain::ReadError> {
        use super::super::terrain::Sample;
        let Some(world) = self.observed.world.as_ref() else {
            return Err(terrain::ReadError::Unavailable);
        };
        let scope = (world.session, world.dimension);
        let (x, y, z) = world.position;
        let query = mod_host::TerrainQuery {
            min: host_position(query.min),
            max: host_position(query.max),
            geometry: host_geometry(query.geometry),
        };
        if let Err(error) = mod_host::terrain_query_size(&query, [x, y, z]) {
            return Err(guest_error(error));
        }
        if !self.terrain.pending.contains(&query) {
            self.terrain.pending.push(query);
        }
        let sample = self
            .terrain
            .subscriptions
            .iter()
            .find(|(candidate, _)| *candidate == query)
            .map(|(_, sample)| sample);
        match sample {
            Some(Sample::Failed(error)) => Err(guest_error(*error)),
            Some(Sample::Region(region))
                if (region.stamp.session, region.stamp.dimension) == scope =>
            {
                let stamp = guest_stamp(region.stamp);
                let same = after.is_some_and(|after| {
                    (
                        after.session,
                        after.dimension,
                        after.registry,
                        after.revision,
                    ) == (
                        stamp.session,
                        stamp.dimension,
                        stamp.registry,
                        stamp.revision,
                    )
                });
                Ok(if same {
                    terrain::ReadResult::Unchanged(stamp)
                } else {
                    terrain::ReadResult::Changed(guest_region(region))
                })
            }
            _ => Ok(terrain::ReadResult::Pending),
        }
    }

    fn find_blocks(
        &mut self,
        identifiers: &[String],
        range: f32,
        limit: u32,
    ) -> Result<Vec<terrain::Position>, terrain::ReadError> {
        let Some(world) = self.observed.world.as_ref() else {
            return Err(terrain::ReadError::Unavailable);
        };
        let scope = (world.session, world.dimension);
        let search = mod_host::BlockSearch::admit(identifiers.to_vec(), range, limit)
            .map_err(guest_error)?;
        if !self.terrain.pending_searches.contains(&search) {
            self.terrain.pending_searches.push(search.clone());
        }
        let sample = self
            .terrain
            .searches
            .iter()
            .find(|(candidate, _)| *candidate == search)
            .and_then(|(_, sample)| sample.as_ref());
        Ok(match sample {
            Some(Err(error)) => return Err(guest_error(*error)),
            Some(Ok(found)) if (found.session, found.dimension) == scope => found
                .positions
                .iter()
                .take(search.limit as usize)
                .copied()
                .map(guest_position)
                .collect(),
            _ => Vec::new(),
        })
    }

    fn set_surface_rules(&mut self, rules: &[terrain::SurfaceRule]) -> Result<(), String> {
        use mod_api::terrain_surface::{Clause, Condition, Rule};
        let rules: Vec<Rule> = rules
            .iter()
            .map(|rule| Rule {
                groups: rule.groups.clone(),
                border: rule.border,
                clauses: rule
                    .clauses
                    .iter()
                    .map(|clause| Clause {
                        tiles: clause.tiles,
                        conditions: clause
                            .conditions
                            .iter()
                            .map(|condition| Condition {
                                offset: [
                                    condition.offset.0,
                                    condition.offset.1,
                                    condition.offset.2,
                                ],
                                present: condition.present,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect();
        if !mod_api::terrain_surface::validate(&rules) {
            return Err("invalid terrain surface rules".into());
        }
        self.gp.surface_rules = rules;
        Ok(())
    }
}
