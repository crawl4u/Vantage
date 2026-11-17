//! Vantage running natively in the client, outside the WebAssembly sandbox and the mod API.
//!
//! The `vantage` crate holds the mod's own code. Each frame this module lends it the client's
//! state through `host::Frame`, runs its frame, and publishes what it set: its HUD surface and its
//! menu. Input and the menu's pointer, scroll and text handling are the client's own
//! (`input::native`), so nothing here needs a loaded mod.

mod draw;
pub(crate) mod handlers;
mod host;
mod observe;
mod stats;

use super::{ModRuntime, input::NativePanel};
use bevy::{prelude::*, window::{CursorOptions, PrimaryWindow}};
use client_shell::ui_resources::{UiPresentationResource, UiRuntimeResource};
use mod_api::handled_player_mod::Guest;
pub(crate) use draw::NativeRenderOutput;
use std::{path::PathBuf, sync::Arc};

/// The layer and menu owner for native Vantage.
pub(super) const OWNER: mod_ui::PanelUiOwner = mod_ui::PanelUiOwner::Environment(10_000);

/// The newest native HUD surface and its revision.
#[derive(Clone)]
pub(super) struct Published {
    pub(super) surface: Arc<ui::mod_panel::Surface>,
    pub(super) revision: u64,
}

/// Native Vantage's HUD layer, published whether or not any mod runtime exists.
#[derive(Clone, Default, Resource)]
pub(crate) struct NativeHud(Option<Published>);

/// The layer as the presenter takes it, to chain after any loaded component's layers.
pub(super) fn published(hud: &NativeHud) -> impl Iterator<Item = mod_ui::PublishedHudSurface<'_>> {
    hud.0.iter().map(|native| mod_ui::PublishedHudSurface {
        owner: OWNER,
        revision: native.revision,
        surface: &native.surface,
        authority: mod_ui::ActorAuthority { players: true, entities: true },
    })
}

/// Per-frame state of the native runtime.
#[derive(Default)]
pub(super) struct State {
    fps: stats::Fps,
    surface: host::SurfaceSlot,
    panel: host::PanelSlot,
    said: std::collections::HashSet<String>,
    hidden: (ui::mod_hud::HiddenElements, bool),
    out: host::Outputs,
    applied: host::Outputs,
    gp: host::GameplayOut,
    draw: draw::NativeDrawing,
    terrain: super::terrain::NativeTerrain,
    sampling: super::terrain::TerrainSampling,
    bindings: super::bindings::Bindings,
    writes: Vec<mod_host::InputBindingWrite>,
    settings_file: Option<PathBuf>,
    started: bool,
    font_set: bool,
    revision: u64,
}

/// Whether native Vantage wants full light everywhere; the client's fullbright publisher
/// combines it with the loaded mods' own requests.
#[derive(Default, Resource)]
pub(crate) struct NativeFullbright(pub(crate) bool);

/// Item delay fix: the use pulse Vantage asked for, offered to the action filter on the next frame.
#[derive(Default, Resource)]
pub(crate) struct NativeUsePulse(pub(crate) bool);

/// Owner token of Vantage's own touch buttons; mod instances count up from 1.
pub(crate) const NATIVE_TOUCH_OWNER: u64 = 1 << 62;

/// Vantage's touch buttons and the taps on them, passed between its frame and the client's
/// presentation of the buttons.
#[derive(Default, Resource)]
pub(crate) struct NativeTouch {
    pub(crate) buttons: Vec<ui::mod_touch::TouchButton>,
    pub(crate) taps: Vec<String>,
}

/// Presents Vantage's buttons beside any mod's and hands Vantage the taps on its own.
pub(super) fn present_touch<'a>(
    presentation: &mut UiPresentationResource,
    native: Option<&'a mut NativeTouch>,
    others: impl Iterator<Item = (u64, &'a ui::mod_touch::TouchButton)> + Clone,
) {
    let mut native = native;
    let mut taps = Vec::new();
    for (owner, id) in presentation.take_mod_touch_taps() {
        if owner == NATIVE_TOUCH_OWNER {
            taps.push(id);
        }
    }
    if let Some(native) = native.as_deref_mut() {
        native.taps.extend(taps);
        native.taps.truncate(ui::mod_touch::MAX_QUEUED_TOUCH_PRESSES);
    }
    let own: &[ui::mod_touch::TouchButton] = native.map_or(&[], |native| &native.buttons);
    presentation.set_mod_touch_buttons(
        others.chain(own.iter().map(|button| (NATIVE_TOUCH_OWNER, button))),
    );
}

/// What a Vantage frame changes in the client, beyond the HUD and the menu.
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct Appliers<'w, 's> {
    cameras: Query<'w, 's, &'static GlobalTransform, With<client_presentation::camera::FlyCamera>>,
    camera: Option<ResMut<'w, client_presentation::camera::CameraSettingsAuthority>>,
    time_override: Option<ResMut<'w, crate::environment::VisualTimeOverride>>,
    weather: Option<ResMut<'w, crate::environment::VisualWeatherOverride>>,
    damage_shake: Option<Res<'w, client_presentation::camera::actor_effects::PresentedDamageShake>>,
    input_settings: Option<Res<'w, client_state::RuntimeSettings>>,
    menu: Option<ResMut<'w, client_shell::menu::MenuRuntime>>,
    fullbright: Option<ResMut<'w, NativeFullbright>>,
    use_pulse: Option<ResMut<'w, NativeUsePulse>>,
    pointer: Option<ResMut<'w, super::pointer::Pending>>,
    touch: Option<ResMut<'w, NativeTouch>>,
    windows: Query<'w, 's, (&'static Window, Option<&'static CursorOptions>), With<PrimaryWindow>>,
    focus: Option<Res<'w, client_state::CursorFocus>>,
    driven: Option<Res<'w, client_state::DrivenInput>>,
    suppress_outline: Option<ResMut<'w, crate::block_selection::SuppressBlockOutline>>,
    actor_style: Option<ResMut<'w, client_presentation::actor_publication::ActorPresentationStyle>>,
    network: Option<Res<'w, client_runtime::network::NetworkHandle<super::PackApplication>>>,
    actors: Option<Res<'w, render_scene::ActorRenderFrame>>,
    render_output: Option<ResMut<'w, NativeRenderOutput>>,
    surface_rules: Option<ResMut<'w, super::terrain_surfaces::NativeSurfaceRules>>,
}

/// Runs one Vantage frame against the client's state and publishes what it set.
pub(super) fn update(
    mut state: Local<State>,
    time: Res<Time<Real>>,
    mut presentation: ResMut<UiPresentationResource>,
    mut hud: ResMut<NativeHud>,
    mut native: ResMut<NativePanel>,
    runtime: Option<Res<ModRuntime>>,
    mut gameplay: super::gameplay::GameplayContext,
    ui: Res<UiRuntimeResource>,
    player: Res<client_state::PlayerRuntime>,
    mut observer: Local<super::module_observations::ModuleObserver>,
    appliers: Appliers,
) {
    let Appliers {
        mut camera,
        mut time_override,
        mut weather,
        damage_shake,
        input_settings,
        mut menu,
        mut fullbright,
        mut use_pulse,
        mut pointer,
        mut touch,
        windows,
        focus,
        driven,
        mut suppress_outline,
        mut actor_style,
        network,
        actors,
        mut render_output,
        surface_rules,
        cameras,
    } = appliers;
    let state = &mut *state;
    if state.settings_file.is_none() {
        state.settings_file = launcher::install_layout::InstallLayout::discover()
            .ok()
            .map(|layout| layout.user_config_root.join("vantage.native.settings.json"));
    }
    let Some(settings_file) = state.settings_file.clone() else {
        return;
    };
    if !state.font_set {
        state.font_set = true;
        match client_extensions::modding::font::rasterize(vantage::UI_FONT) {
            Ok(font) => {
                if let Err(error) = presentation.set_mod_panel_font(Some(Arc::new(font))) {
                    warn!("Native Vantage menu font rejected: {error}");
                }
            }
            Err(error) => warn!("Native Vantage menu font invalid: {error}"),
        }
    }
    host::set_metrics(presentation.label_metrics());
    let layout = presentation
        .mod_ui()
        .mod_panel_layout()
        .filter(|(who, _)| *who == OWNER)
        .map(|(_, layout)| layout);
    let press = presentation
        .mod_ui()
        .mod_panel_press()
        .filter(|(who, _)| *who == OWNER)
        .map(|(_, press)| press);
    let (viewport, elements) = presentation.hud_elements();
    let food = ui.food_catalog().cloned();
    state.panel.open = native.open;
    // The camera calls Vantage renews every frame lapse when it stops; the rest stay until changed.
    state.out.yaw_offset = None;
    state.out.perspective = None;
    state.out.view_scale = None;
    let bindings = input_settings
        .as_ref()
        .map(|settings| state.bindings.sample(settings.controls_update().1));
    let snapshot = gameplay.player_state(true, &player, &ui);
    let world = gameplay.world_observation(&ui);
    let modules = observer.capture(&gameplay, &ui, world.as_ref(), &player, None, true);
    let sidebar = world
        .as_ref()
        .and_then(|_| presentation.sidebar_snapshot(&ui));
    let chat = if world.is_some() {
        super::presentation_facts::chat_snapshot(&ui)
    } else {
        Vec::new()
    };
    let observed = observe::convert(snapshot.as_ref(), world.as_ref(), &modules, sidebar, &chat);
    // World drawing follows the connected world, its nearby actors and the actors masks may pick.
    let session = super::packet_render::session(&gameplay);
    state.draw.bind(session);
    if session.permissions.entity_observe
        && let Some(authority) = gameplay.world.as_ref().and_then(|world| world.stream.as_ref()).map(|stream| stream.authority())
        && let Some(view) = gameplay.view.as_ref()
    {
        let dimension = authority.current_dimension();
        let connection = authority.actor_session_id();
        state.draw.reconcile_entities(
            super::anchors::candidates(authority, view.feet_translation().to_array()),
            |key| {
                key.session != connection
                    || key.dimension != dimension
                    || authority
                        .actor(key.runtime_id)
                        .is_none_or(|actor| actor.render_spawn_revision.get() != key.spawn_revision)
            },
        );
    }
    if session.permissions.entity_highlight
        && let Some(actors) = actors.as_deref()
    {
        state.draw.publish_mask_targets(&render_scene::mod_scene::mask_targets(actors));
    }
    state.sampling.refresh_native(&mut state.terrain, &gameplay, world.as_ref());
    state.terrain.begin();
    state.draw.begin();
    let container = super::drive::observed_container_screen(&player, &ui);
    let facts = gameplay.facts(native.controls.gameplay, cameras.single().ok(), None, None);
    let mut frame = host::Frame {
        facts: &facts,
        native: &mut native,
        draw: &mut state.draw,
        terrain: &mut state.terrain,
        observed: &observed,
        out: &mut state.out,
        gp: &mut state.gp,
        bindings,
        writes: &mut state.writes,
        damage_shake: damage_shake.as_deref().and_then(|shake| shake.0).map(|shake| (shake.roll_degrees, shake.pitch_degrees)),
        food,
        elements,
        hidden: &mut state.hidden,
        fps: state.fps.frame(time.elapsed_secs_f64()) as f32,
        viewport: viewport.map(|view| [view.width, view.height, view.scale]),
        settings_file: &settings_file,
        surface: &mut state.surface,
        panel: &mut state.panel,
        layout,
        press,
        said: &mut state.said,
        container,
        touch_taps: touch.as_deref_mut().map(|touch| std::mem::take(&mut touch.taps)).unwrap_or_default(),
    };
    vantage::host::scope(&mut frame, || {
        if !state.started {
            state.started = true;
            vantage::Mod::init();
            info!("VANTAGE_NATIVE started");
        }
        vantage::Mod::frame();
    });
    state.terrain.commit();
    if let Some(mut rules) = surface_rules
        && rules.0 != state.gp.surface_rules
    {
        rules.0.clone_from(&state.gp.surface_rules);
    }
    if let Some(output) = state.draw.finish()
        && let Some(render_output) = render_output.as_deref_mut()
    {
        *render_output = output;
    }
    // The presenter keeps these only for the frame they are set, so they are set every frame.
    let out = state.out;
    {
        if let Some(camera) = camera.as_deref_mut() {
            use semantic_input::PerspectiveMode as Mode;
            camera.set_view_yaw_offset(out.yaw_offset);
            camera.set_mod_perspective(out.perspective.map(|view| match view {
                0 => Mode::FirstPerson,
                1 => Mode::ThirdPersonBack,
                _ => Mode::ThirdPersonFront,
            }));
            let [fov_scale, look_scale] = out.view_scale.unwrap_or([1.0, 1.0]);
            camera.set_view_scale(fov_scale, look_scale);
            camera.set_view_tilt(out.tilt.map(|(roll, pitch, suppress)| client_presentation::camera::ViewTilt {
                roll_degrees: roll,
                pitch_degrees: pitch,
                suppress_hurt_shake: suppress,
            }));
        }
        if let Some(time_override) = time_override.as_deref_mut() {
            time_override.0 = out.time;
        }
        if let Some(weather) = weather.as_deref_mut() {
            weather.0 = out.weather;
        }
        state.applied = out;
    }
    // Held sprint and sneak, the control modes and fullbright stay as the guest last set them.
    let input_owned = native.controls.gameplay;
    gameplay.apply_movement_intents(
        state
            .gp
            .intents
            .map(|(sprint, sneak)| mod_host::GameplayMovementIntents { sprint, sneak }),
        input_owned,
    );
    gameplay.apply_control_modes(state.gp.modes.map(|(toggle_sprint, toggle_sneak, always_sprint)| {
        mod_host::GameplayControlModes {
            toggle_sprint,
            toggle_sneak,
            always_sprint,
        }
    }));
    if let Some(suppress) = suppress_outline.as_deref_mut() {
        suppress.0 = vantage::block_outline_on();
    }
    if let Some(buttons) = state.gp.touch_buttons.take()
        && let Some(touch) = touch.as_deref_mut()
    {
        touch.buttons = buttons;
    }
    if std::mem::take(&mut state.gp.use_pulse)
        && let Some(use_pulse) = use_pulse.as_deref_mut()
    {
        use_pulse.0 = true;
    }
    // Center cursor: the pointer move waits for the container's pointer authority, as a mod's does.
    if let Some(point) = state.gp.pointer.take()
        && let Some(screen) = container
        && let Some(pending) = pointer.as_deref_mut()
        && let Ok((window, _)) = windows.single()
        && !native.open
        && (driven.is_some() || window.focused)
        && super::drive::container_pointer_available(
            &player,
            &ui,
            &presentation,
            menu.as_deref().is_some_and(client_shell::menu::MenuRuntime::is_visible),
            super::drive::pointer_focus_available(window.focused, focus.as_deref(), driven.is_some()),
        )
        && driven.is_none()
    {
        let point = Vec2::new(
            (point[0] * window.width()).min((window.width() - 1.0).max(0.0)),
            (point[1] * window.height()).min((window.height() - 1.0).max(0.0)),
        );
        pending.stage(super::pointer::NATIVE_OWNER, 0, screen, point);
    }
    if let Some(fullbright) = fullbright.as_deref_mut() {
        fullbright.0 = state.gp.fullbright;
    }
    let commands = std::mem::take(&mut state.gp.commands);
    if !commands.is_empty() {
        super::send_commands(network.as_deref(), ui.session_id(), commands);
    }
    if state.gp.presentation_changed {
        state.gp.presentation_changed = false;
        use ui::mod_presentation::{ActorGrants, Presentation, TintState};
        let grants = ActorGrants { players: true, entities: true };
        let hurt = |wanted: TintState| {
            state
                .gp
                .tints
                .iter()
                .filter(|rule| rule.state == wanted)
                .cloned()
                .collect::<Vec<_>>()
        };
        let merged = Arc::new(Presentation {
            nametag_style: state.gp.nametag_style.clone(),
            nametag_rules: (grants, state.gp.nametag_rules.clone()),
            tint_rules: [(grants, hurt(TintState::Hurt)), (grants, hurt(TintState::NotHurt))],
            ..Default::default()
        });
        presentation.set_mod_presentation(Arc::clone(&merged));
        if let Some(style) = actor_style.as_deref_mut() {
            style.presentation = merged;
        }
    }
    for write in std::mem::take(&mut state.writes) {
        let Some(menu) = menu.as_deref_mut() else {
            break;
        };
        let result = super::bindings::key_change(&write).and_then(|(action, change)| {
            menu.change_action_key(action, change).map_err(|_| "refused by the Controls settings")
        });
        if let Err(error) = result {
            debug!(action = %write.action, error, "Vantage binding change refused");
        }
    }
    if state.hidden.1 {
        state.hidden.1 = false;
        presentation.mod_ui_mut().set_hidden_hud_elements(state.hidden.0);
    }
    if state.surface.changed {
        state.surface.changed = false;
        state.revision += 1;
        hud.0 = state.surface.current.as_ref().map(|(surface, _)| Published {
            surface: Arc::clone(surface),
            revision: state.revision,
        });
        // Without a mod runtime nothing else publishes layers, so this is the whole set.
        if runtime.is_none() {
            presentation.mod_ui_mut().set_hud_surfaces(published(&hud));
        }
    }
    if state.panel.changed {
        state.panel.changed = false;
        if let Err(error) = presentation
            .mod_ui_mut()
            .set_mod_panel(OWNER, state.panel.panel.as_ref())
        {
            warn!("Native Vantage menu rejected: {error}");
            state.panel.panel = None;
        }
    }
    if let Some(open) = state.panel.open_request.take() {
        let open = open && state.panel.panel.is_some();
        presentation.mod_ui_mut().set_mod_panel_open(open);
        native.open = open;
    }
}
