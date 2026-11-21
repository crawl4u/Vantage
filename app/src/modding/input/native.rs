//! Native Vantage's menu input: the same routing a mod's panel gets, with no mod runtime behind it.
//! It samples the physical frame once, drives the presenter's panel (pointer, scroll, text fields,
//! Escape), captures or returns the cursor, and leaves the frame for Vantage to read.

use super::*;

/// What native Vantage's menu and input have in common with the client: its open state and the
/// last physical frame, which Vantage reads when it runs.
#[derive(Resource)]
pub(in crate::modding) struct NativePanel {
    /// Whether the menu is open and owns input.
    pub open: bool,
    /// The last physical frame, in the form a mod's controls are read.
    pub controls: mod_host::ControlFrame,
    /// Field edits the presenter made since Vantage last looked.
    pub field_events: Vec<ui::mod_panel::FieldEvent>,
    /// Keys Vantage owns: removed from gameplay while the menu is closed.
    pub reserved: Vec<String>,
    /// The chord that claims the mouse wheel, and the turn it claimed this frame:
    /// line x, line y, pixel x, pixel y.
    pub scroll_keys: Vec<String>,
    pub scroll: [f32; 4],
}

impl Default for NativePanel {
    fn default() -> Self {
        Self {
            open: false,
            controls: mod_host::empty_controls(),
            field_events: Vec::new(),
            reserved: Vec::new(),
            scroll_keys: Vec::new(),
            scroll: [0.0; 4],
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Routes one physical frame before gameplay authority."
)]
pub(in crate::modding) fn prepare_native_input(
    native: Option<ResMut<NativePanel>>,
    mut physical: Local<PhysicalControls>,
    input: PanelInput,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: Option<ResMut<ButtonInput<MouseButton>>>,
    mut motion: Option<ResMut<AccumulatedMouseMotion>>,
    mut windows: Query<(Entity, &mut Window, &mut CursorOptions), With<PrimaryWindow>>,
    ui: Res<UiRuntimeResource>,
    player: Res<client_state::PlayerRuntime>,
    menu: Option<Res<MenuRuntime>>,
    mut presentation: ResMut<UiPresentationResource>,
    time: Option<Res<Time>>,
    mut focus: Option<ResMut<client_state::CursorFocus>>,
    driven: Option<Res<client_state::DrivenInput>>,
) {
    let Some(mut native) = native else { return };
    let PanelInput {
        keyboard_events,
        ime_events,
        ordered_events,
        mut clipboard,
        mouse_events,
        mut scroll,
        wheel_events,
        reserved,
    } = input;
    let mut reserved = reserved;
    let Ok((entity, window, mut cursor)) = windows.single_mut() else {
        return;
    };
    let (_, wheel_received) = scroll::sample(&mut physical.wheel, wheel_events.as_deref(), entity);
    let focused = driven.is_some()
        || (window.focused && focus.as_ref().is_none_or(|focus| focus.available()));
    if focused {
        let seed_pointer = !physical.held_seeded;
        physical.seed_held(keys.get_pressed());
        if seed_pointer && let Some(buttons) = mouse.as_ref() {
            for (button, name) in POINTER_BUTTONS {
                physical.track_held(
                    name.into(),
                    buttons.pressed(button) && !buttons.just_pressed(button),
                );
            }
        }
    }
    let mut panel_keys = Vec::new();
    for sample in field::samples(
        &mut physical,
        ordered_events.as_deref(),
        keyboard_events.as_deref(),
        ime_events.as_deref(),
        entity,
    ) {
        let field::RawSample::Key(event) = sample else {
            if let field::RawSample::Ime(event) = sample {
                panel_keys.push(PanelSample::Ime(event));
            }
            continue;
        };
        if matches!(event.key_code, KeyCode::Unidentified(_)) {
            continue;
        }
        let name = format!("{:?}", event.key_code);
        let down = event.state == ButtonState::Pressed;
        physical.track_held(name.clone(), down);
        if down && focused {
            panel_keys.push(PanelSample::Key((
                name,
                event
                    .text
                    .as_deref()
                    .or_else(|| match &event.logical_key {
                        bevy::input::keyboard::Key::Character(text) => Some(text.as_str()),
                        _ => None,
                    })
                    .map(str::to_owned),
                event.repeat,
                field::modifiers(&physical.held_keys),
            )));
        }
    }
    let pointer_pressed =
        physical.pointer_edges(mouse_events.as_deref(), mouse.as_deref(), entity, focused);
    if !focused {
        physical.left_held = false;
        physical.held_keys.clear();
        physical.held_seeded = false;
    }
    panel_keys.extend(
        pointer_pressed
            .iter()
            .cloned()
            .map(|key| PanelSample::Key((key, None, false, Default::default()))),
    );
    let raw_key_presses: Vec<_> = panel_keys
        .iter()
        .filter_map(PanelSample::key)
        .filter(|(key, _, repeat, _)| !repeat && key != TEXT_INPUT)
        .take(mod_api::MAX_CONTROL_KEYS)
        .map(|(key, _, _, _)| key.clone())
        .collect();
    let absorbed = menu.as_deref().map_or_else(
        || ui.ui_focused(&player),
        |menu| presentation.base_absorbs_gameplay_input(&player, &ui, menu),
    );
    let was_open = physical.panel_owned;
    let mut open = native.open && presentation.mod_ui().mod_panel_open();
    let editing = open && presentation.mod_ui().mod_panel_editing();
    if !focused {
        presentation.mod_ui_mut().cancel_mod_panel_pointer_input();
    }
    if absorbed {
        presentation.mod_ui_mut().cancel_mod_panel_edit();
    }
    let mut pressed = pointer_pressed;
    let mut events = Vec::new();
    for sample in panel_keys {
        let Some((key, text, repeat, modifiers)) =
            field::route_sample(sample, presentation.mod_ui_mut(), focused && !absorbed)
        else {
            continue;
        };
        if POINTER_BUTTONS.iter().any(|(_, name)| *name == key) {
            continue;
        }
        if open && focused && !absorbed && editing {
            if (modifiers.control || modifiers.super_key)
                && native_editor_shortcut(presentation.mod_ui_mut(), clipboard.as_deref_mut(), &key)
            {
                continue;
            }
            let text = if (modifiers.control || modifiers.super_key) && key != TEXT_INPUT {
                Some("")
            } else {
                text.as_deref()
            };
            events.extend(
                presentation
                    .mod_ui_mut()
                    .mod_panel_field_key(&key, text, modifiers),
            );
        } else if key != TEXT_INPUT && !repeat {
            pressed.push(key);
        }
    }
    let editor_closed_panel = open && !presentation.mod_ui().mod_panel_open();
    open &= presentation.mod_ui().mod_panel_open();
    let close_requested = pressed.iter().any(|key| key == "Escape")
        && open
        && !presentation.mod_ui().mod_panel_capture_key();
    if focused && (absorbed || close_requested) {
        open = false;
    }
    presentation.mod_ui_mut().set_mod_panel_open(open);
    let before_pointer_open = open;
    let primary_pressed = mouse
        .as_ref()
        .is_some_and(|buttons| buttons.just_pressed(MouseButton::Left));
    if open && focused {
        if let Some(position) = window.cursor_position() {
            events.extend(presentation.mod_ui_mut().mod_panel_events(
                position.to_array(),
                primary_pressed,
                physical.left_held,
            ));
            if let Some(scroll) = scroll.as_ref() {
                presentation.mod_ui_mut().scroll_mod_panel(
                    position.to_array(),
                    f64::from(scroll.delta.y),
                    scroll.unit == MouseScrollUnit::Pixel,
                );
            }
        } else if !physical.left_held {
            presentation.mod_ui_mut().cancel_mod_panel_pointer_input();
        }
    }
    let events = events
        .into_iter()
        .take(ui::mod_panel::MAX_PANEL_EVENTS)
        .map(|event| mod_host::ControlEvent {
            id: event.id,
            value: event.value,
        })
        .collect();
    open = presentation.mod_ui().mod_panel_open();
    let pointer_closed = before_pointer_open && !open;
    if !open
        && focused
        && !absorbed
        && (close_requested || pointer_closed || editor_closed_panel)
        && let Some(focus) = focus.as_deref_mut()
    {
        focus.authorize_screen_return();
    }
    native.open = open;
    let restore = physical.finish_panel(
        open,
        focused,
        absorbed,
        crate::camera::mouse_input_active(&window, &cursor, focus.as_deref(), driven.is_some()),
    );
    native.controls = mod_host::ControlFrame {
        seconds: time
            .as_ref()
            .map_or(0.0, |time| time.delta_secs().clamp(0.0, 1.0)),
        focused,
        gameplay: focused && !absorbed && !open,
        panel_open: open,
        keys_pressed: if focused && !absorbed {
            raw_key_presses
        } else {
            Vec::new()
        },
        keys_held: if focused && !absorbed {
            physical.held_sample()
        } else {
            Vec::new()
        },
        keys_admitted: Vec::new(),
        events,
        pointer: window
            .cursor_position()
            .filter(|_| open && focused && !absorbed)
            .and_then(|position| {
                presentation.hud_gui_point(
                    position.to_array(),
                    [window.physical_width(), window.physical_height()],
                    window.scale_factor(),
                )
            })
            .map(|[x, y]| (x, y)),
        wheel: scroll
            .as_deref()
            .filter(|_| open && focused && !absorbed)
            .map_or(0.0, wheel_notches),
    };
    native.scroll = [0.0; 4];
    let chord_held = !native.scroll_keys.is_empty()
        && native
            .scroll_keys
            .iter()
            .all(|key| physical.held_keys.contains(key));
    if !open
        && focused
        && !absorbed
        && wheel_received
        && chord_held
        && let Some(scroll) = scroll.as_deref_mut()
    {
        native.scroll = match scroll.unit {
            MouseScrollUnit::Line => [scroll.delta.x, scroll.delta.y, 0.0, 0.0],
            MouseScrollUnit::Pixel => [0.0, 0.0, scroll.delta.x, scroll.delta.y],
        };
        scroll.delta = Vec2::ZERO;
        if let Some(reserved) = reserved.as_deref_mut() {
            reserved.scroll = true;
        }
    }
    let field_events = presentation
        .mod_ui_mut()
        .take_mod_panel_field_events(super::super::vantage::OWNER);
    if focused && !absorbed && !field_events.is_empty() {
        native.field_events.extend(field_events);
    }
    finish_capture(
        open,
        was_open,
        restore,
        &mut keys,
        mouse.as_deref_mut(),
        motion.as_deref_mut(),
        &mut cursor,
    );
    if !open {
        let owned = native.reserved.clone();
        consume_reserved(
            &mut keys,
            mouse.as_deref_mut(),
            &owned,
            reserved.as_deref_mut(),
        );
    }
}
