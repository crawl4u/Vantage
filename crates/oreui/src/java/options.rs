//! The Java-style Settings screens: a list of option pages, and the page of each settings section,
//! laid out as Java Edition lays its options out. A title 15 Java pixels down, a scrolling list
//! of 150-pixel widgets in two columns between a 33-pixel header and footer, and Done at the
//! bottom. A page lists the options the shared settings registry assigns to its section, drawn as
//! sliders, on and off buttons and buttons that step through their choices. What a page cannot
//! show, such as key bindings, language and resource packs, stays in the Bedrock-style Settings,
//! which the list and every page reach.

use assets::audio_category::AudioCategory;
use launcher::menu::{
    JavaSettingsAction, JavaSettingsView, MenuAction, MenuField, MenuView,
    auth::AuthState,
    settings_options::{
        BindingId, EXTRA_GAMEPAD, EXTRA_KEYS, GAMEPAD_BINDINGS, GAMEPAD_OFFSET, KEY_BINDINGS,
        SETTINGS_OPTIONS, SettingId, SettingKind, SettingsOptions, layout::SettingPanel,
    },
    settings_sections::{SECTION_DEFINITIONS, SettingSection},
    settings_support::{SupportAction, SupportLink},
};
use ui_engine::engine::screen::{PaintError, Translate};

use super::frame::put;

use super::{button, unit};
use crate::{
    focus,
    paint::{Bounds, Canvas, SpriteScrollbar},
    settings,
    theme::TEXT,
};

/// Sizes in Java pixels.
const WIDGET_WIDTH: f32 = 150.0;
const WIDE_WIDTH: f32 = 310.0;
const WIDGET_HEIGHT: f32 = 20.0;
const ROW: f32 = 25.0;
const COLUMN_GAP: f32 = 10.0;
const HEADER: f32 = 33.0;
const FOOTER: f32 = 33.0;
const TITLE_TOP: f32 = 15.0;
const TEXT_HEIGHT: f32 = 8.0;
const LIST_PADDING: f32 = 4.0;
const DONE_WIDTH: f32 = 200.0;
const SEPARATOR: f32 = 2.0;
/// The shade over the world behind the screen, and over the list on it.
const DIM: [u8; 4] = [16, 16, 16, 170];
const LIST_SHADE: [u8; 4] = [0, 0, 0, 90];

/// The list's scroll bar: its sprites, its width, how far right of the centre it sits (the list's
/// row edge plus 6) and its thumb's least height, in Java pixels.
const SCROLLER_TRACK: &str = "sprites/widget/scroller_background.png";
const SCROLLER: &str = "sprites/widget/scroller.png";
const SCROLLBAR_WIDTH: f32 = 6.0;
const SCROLLBAR_OFFSET: f32 = 161.0;
const SCROLLER_MIN: f32 = 32.0;

/// Sprites of a slider and its handle, at rest and highlighted.
const SLIDER: [&str; 2] = [
    "sprites/widget/slider.png",
    "sprites/widget/slider_highlighted.png",
];
const HANDLE: [&str; 2] = [
    "sprites/widget/slider_handle.png",
    "sprites/widget/slider_handle_highlighted.png",
];
/// The handle's size in Java pixels when its sprite cannot say.
const HANDLE_SIZE: [f32; 2] = [8.0, 20.0];

/// One widget of the list.
pub(super) enum Entry {
    Option(SettingId),
    /// A line of text in the warning colour, across the whole list.
    Notice(String),
    /// A label with a text field beside it, across the whole list: what the field edits, its
    /// text, what it shows while empty, the field it focuses and the press that focuses it.
    Field {
        label: String,
        value: String,
        placeholder: String,
        field: MenuField,
        press: MenuAction,
    },
    Link(String, MenuAction),
    /// A button that does nothing, which says something: who is signed in.
    Label(String),
    /// A line of text over what follows, across the whole list.
    Heading(String),
    /// A key binding: the action's name, the button that captures a key for it and, when the key
    /// is not the default, a button that restores it. Its place among the bindings is the index.
    Binding(usize),
}

impl Entry {
    /// Whether the widget spans the list's whole width: the master volume, which Java sets above
    /// the other volumes, headings and key bindings.
    fn wide(&self) -> bool {
        match self {
            Self::Option(id) => SETTINGS_OPTIONS[id.index()].volume == Some(AudioCategory::Master),
            Self::Heading(_) | Self::Binding(_) | Self::Notice(_) | Self::Field { .. } => true,
            Self::Link(..) | Self::Label(_) => false,
        }
    }
}

/// What a page holds beyond the registry's options for its section: buttons for an account, for
/// the touch layout, and the key bindings.
fn extras(view: &MenuView, section: SettingSection) -> Vec<Entry> {
    let link = |label: &str, action: MenuAction| Entry::Link(label.to_owned(), action);
    match section {
        SettingSection::Account => {
            if matches!(view.auth_state, AuthState::Authenticated) {
                vec![
                    Entry::Label(format!(
                        "Signed in: {}",
                        launcher::menu::profile::current_name(view)
                    )),
                    link(
                        "Change Gamertag",
                        MenuAction::SettingsSupport(SupportAction::Open(SupportLink::Gamertag)),
                    ),
                    link(
                        "Manage Account",
                        MenuAction::SettingsSupport(SupportAction::Open(SupportLink::Account)),
                    ),
                    link("Sign Out", MenuAction::SignOut),
                ]
            } else {
                vec![link("Sign In", MenuAction::StartSignIn)]
            }
        }
        SettingSection::Updates => vec![link(
            "Check for Updates",
            MenuAction::Update(launcher::menu::updates::Action::CheckNow),
        )],
        SettingSection::Touch => {
            if view.touch_available {
                vec![link("Edit Layout", MenuAction::EditTouchControls)]
            } else {
                vec![Entry::Label("No touch screen found".to_owned())]
            }
        }
        SettingSection::KeyboardAndMouse => {
            let count = KEY_BINDINGS.len() + EXTRA_KEYS.len();
            let mut entries = vec![Entry::Heading("Key Binds".to_owned())];
            entries.extend((0..count).map(Entry::Binding));
            entries.push(link(
                "Reset Key Binds",
                MenuAction::SettingsResetBindings(false),
            ));
            entries
        }
        SettingSection::ControllerAndSwitch => {
            let count = GAMEPAD_BINDINGS.len() + EXTRA_GAMEPAD.len();
            let mut entries = vec![Entry::Heading("Controller Binds".to_owned())];
            entries.extend((0..count).map(|index| Entry::Binding(GAMEPAD_OFFSET + index)));
            entries.push(link(
                "Reset Controller Binds",
                MenuAction::SettingsResetBindings(true),
            ));
            entries
        }
        _ => Vec::new(),
    }
}

/// Whether the Java page of `section` has anything to list.
fn page_has_content(view: &MenuView, section: SettingSection) -> bool {
    !page_options(view, section).is_empty() || !extras(view, section).is_empty()
}

/// The text of the key binding at `index` in the bindings' order, a localization key.
fn binding_label(index: usize) -> &'static str {
    if index >= GAMEPAD_OFFSET {
        let place = index - GAMEPAD_OFFSET;
        GAMEPAD_BINDINGS
            .get(place)
            .map(|(_, label)| *label)
            .or_else(|| {
                EXTRA_GAMEPAD
                    .get(place - GAMEPAD_BINDINGS.len())
                    .map(|(label, _)| *label)
            })
            .unwrap_or("")
    } else {
        KEY_BINDINGS
            .get(index)
            .map(|(_, label)| *label)
            .or_else(|| {
                EXTRA_KEYS
                    .get(index - KEY_BINDINGS.len())
                    .map(|(label, _)| *label)
            })
            .unwrap_or("")
    }
}

/// The options a page of `section` lists, in the registry's order.
fn page_options(view: &MenuView, section: SettingSection) -> Vec<SettingId> {
    let mut found = Vec::new();
    for panel in SettingPanel::ALL
        .iter()
        .filter(|panel| panel.section() == section)
    {
        for option in panel.options() {
            if matches!(option.kind, SettingKind::Host)
                || option.id == SettingId::GuiScale
                || option.id == SettingId::FieldOfView
                || found.contains(&option.id)
                || !settings::available(view, option.id)
            {
                continue;
            }
            found.push(option.id);
        }
    }
    found
}

/// Whether `section` has a Java page of its own, laid out by hand rather than from the settings
/// registry's options.
fn has_page(section: SettingSection) -> bool {
    matches!(
        section,
        SettingSection::GlobalTexturePack
            | SettingSection::Shaders
            | SettingSection::Language
            | SettingSection::StorageManagement
            | SettingSection::Mods
    )
}

/// The section whose page is shown.
fn section(view: &MenuView) -> SettingSection {
    SettingSection::from_index(view.settings.section)
        .filter(|section| view.settings_section_shown(*section))
        .unwrap_or(SettingSection::Accessibility)
}

/// Whether the Java style shows the open Settings screen itself rather than the Bedrock one.
pub(crate) fn shows_java(view: &MenuView) -> bool {
    match view.settings.java_view {
        JavaSettingsView::Hub => true,
        JavaSettingsView::Page | JavaSettingsView::Direct => {
            has_page(section(view)) || page_has_content(view, section(view))
        }
        JavaSettingsView::Bedrock => false,
    }
}

/// The pages Java has, in the order its Options screen lists them. Pages this client adds follow
/// in the settings registry's order.
const JAVA_PAGE_ORDER: &[SettingSection] = &[
    SettingSection::Sound,
    SettingSection::Video,
    SettingSection::KeyboardAndMouse,
    SettingSection::ControllerAndSwitch,
    SettingSection::Touch,
    SettingSection::Language,
    SettingSection::Chat,
    SettingSection::GlobalTexturePack,
    SettingSection::Accessibility,
];

/// What Java calls the page of `section`, where it has a page by that name.
fn java_name(section: SettingSection) -> Option<&'static str> {
    Some(match section {
        SettingSection::Sound => "Music & Sounds",
        SettingSection::Video => "Video Settings",
        SettingSection::KeyboardAndMouse => "Controls",
        SettingSection::Chat => "Chat Settings",
        SettingSection::GlobalTexturePack => "Resource Packs",
        SettingSection::Accessibility => "Accessibility Settings",
        _ => return None,
    })
}

/// The name of the page of `section`: Java's, else the section's own tab title.
fn page_name(view: &MenuView, translate: Translate<'_>, section: SettingSection) -> String {
    java_name(section).map_or_else(
        || {
            let title = section
                .definition()
                .tab_title
                .unwrap_or("hbui.Settings.title");
            settings::text(view, translate, title)
        },
        str::to_owned,
    )
}

/// The widgets of the list of pages: the field of view, then a button for every section.
fn hub_entries(view: &MenuView, translate: Translate<'_>) -> Vec<Entry> {
    let mut entries = vec![Entry::Option(SettingId::FieldOfView)];
    let mut sections: Vec<_> = SECTION_DEFINITIONS
        .iter()
        .filter(|definition| {
            definition.tab_title.is_some() && view.settings_section_shown(definition.id)
        })
        .collect();
    sections.sort_by_key(|definition| {
        JAVA_PAGE_ORDER
            .iter()
            .position(|section| *section == definition.id)
            .unwrap_or(JAVA_PAGE_ORDER.len())
    });
    for definition in sections {
        let action = if !has_page(definition.id) && !page_has_content(view, definition.id) {
            JavaSettingsAction::Bedrock(definition.index)
        } else {
            JavaSettingsAction::Page(definition.index)
        };
        let name = page_name(view, translate, definition.id);
        entries.push(Entry::Link(name, MenuAction::JavaSettings(action)));
    }
    entries
}

/// The widgets of the page of `section`, ending with the way to its full Bedrock-style page.
fn page_entries(view: &MenuView, section: SettingSection) -> Vec<Entry> {
    let mut entries: Vec<Entry> = page_options(view, section)
        .into_iter()
        .map(Entry::Option)
        .collect();
    entries.extend(extras(view, section));
    entries.push(Entry::Link(
        "More settings".to_owned(),
        MenuAction::JavaSettings(JavaSettingsAction::Bedrock(section.definition().index)),
    ));
    entries
}

/// Draws the list of pages or the open page.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
    translate: Translate<'_>,
) -> Result<(), PaintError> {
    let hub = view.settings.java_view == JavaSettingsView::Hub;
    let open = section(view);
    if !hub && has_page(open) {
        return match open {
            SettingSection::GlobalTexturePack => super::packs::draw(canvas, view, size),
            SettingSection::Shaders => super::shaders::draw(canvas, view, size),
            SettingSection::Language => super::language::draw(canvas, view, size),
            SettingSection::StorageManagement => super::storage::draw(canvas, view, size),
            SettingSection::Mods => super::mods::draw(canvas, view, size),
            _ => Ok(()),
        };
    }
    let unit = unit(view, canvas, size);
    let java_width = (size[0] / unit).ceil();
    let (title, entries) = if hub {
        ("Options".to_owned(), hub_entries(view, translate))
    } else {
        (page_name(view, translate, open), page_entries(view, open))
    };
    canvas.fill([0.0, 0.0, size[0], size[1]], DIM)?;
    let screen =
        canvas.begin_focus_region(focus::SCREEN, [0.0, 0.0, size[0], size[1]], None, true)?;
    canvas.focus_delegate(Some(MenuAction::AddBack), None);
    let viewport = [0.0, HEADER * unit, size[0], size[1] - FOOTER * unit];
    canvas.fill(viewport, LIST_SHADE)?;
    let key = if hub {
        "java_options/hub".to_owned()
    } else {
        format!("java_options/{}", open.definition().name)
    };
    list(
        canvas, view, translate, viewport, java_width, &entries, &key, unit,
    )?;
    separators(canvas, size, unit)?;
    button::label_centred(
        canvas,
        &title,
        [
            0.0,
            TITLE_TOP * unit,
            size[0],
            (TITLE_TOP + TEXT_HEIGHT) * unit,
        ],
        unit,
        TEXT,
    )?;
    let centre = (java_width / 2.0).floor();
    let done = [
        centre - DONE_WIDTH / 2.0,
        size[1] / unit - FOOTER + 7.0,
        centre + DONE_WIDTH / 2.0,
        size[1] / unit - FOOTER + 7.0 + WIDGET_HEIGHT,
    ];
    button::draw(
        canvas,
        view,
        done.map(|value| value * unit),
        "Done",
        Some(MenuAction::AddBack),
        unit,
    )?;
    canvas.end_focus_region(screen);
    Ok(())
}

/// The header and footer's lower and upper edges, drawn from Java's separator sprites.
fn separators(canvas: &mut Canvas<'_>, size: [f32; 2], unit: f32) -> Result<(), PaintError> {
    for (name, top) in [
        ("header_separator.png", HEADER * unit - SEPARATOR * unit),
        ("footer_separator.png", size[1] - FOOTER * unit),
    ] {
        let key = crate::images::java::key(name);
        let Some([width, _]) = canvas.sprite_size(&key) else {
            continue;
        };
        let tile = (width * unit).max(1.0);
        let mut x = 0.0;
        while x < size[0] {
            canvas.sprite(
                &key,
                [x, top, (x + tile).min(size[0]), top + SEPARATOR * unit],
                [255; 4],
            )?;
            x += tile;
        }
    }
    Ok(())
}

/// Where one widget sits in the list.
#[derive(Debug, PartialEq)]
struct Place {
    row: usize,
    column: usize,
    /// It spans both columns, as Java's master volume does.
    wide: bool,
}

/// Places `entries` two to a row, a wide one alone on a row of its own.
fn places(entries: &[Entry]) -> Vec<Place> {
    let (mut row, mut column) = (0, 0);
    entries
        .iter()
        .map(|entry| {
            if entry.wide() {
                if column != 0 {
                    row += 1;
                }
                let place = Place {
                    row,
                    column: 0,
                    wide: true,
                };
                row += 1;
                column = 0;
                return place;
            }
            let place = Place {
                row,
                column,
                wide: false,
            };
            column += 1;
            if column == 2 {
                column = 0;
                row += 1;
            }
            place
        })
        .collect()
}

/// The scrolling list of widgets in two columns.
#[allow(clippy::too_many_arguments, reason = "one screen's drawing state")]
pub(super) fn list(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    translate: Translate<'_>,
    viewport: Bounds,
    java_width: f32,
    entries: &[Entry],
    key: &str,
    unit: f32,
) -> Result<(), PaintError> {
    let places = places(entries);
    let defaults = SettingsOptions::default();
    let rows = places.last().map_or(0, |place| place.row + 1) as f32;
    let content = (LIST_PADDING * 2.0 + rows * ROW) * unit;
    let max = (content - (viewport[3] - viewport[1])).max(0.0);
    if let Some(offset) = canvas.offsets.get_mut(key) {
        *offset = offset.min(max);
    }
    let centre = (java_width / 2.0).floor();
    canvas.sprite_scrollbar = Some(SpriteScrollbar {
        track: crate::images::java::key(SCROLLER_TRACK),
        thumb: crate::images::java::key(SCROLLER),
        x: (centre + SCROLLBAR_OFFSET) * unit,
        width: SCROLLBAR_WIDTH * unit,
        min_thumb: SCROLLER_MIN * unit,
    });
    let scroll = canvas.begin_scroll(key, viewport)?;
    let columns = [
        centre - COLUMN_GAP / 2.0 - WIDGET_WIDTH,
        centre + COLUMN_GAP / 2.0,
    ];
    for (entry, place) in entries.iter().zip(&places) {
        let x = columns[place.column];
        let width = if place.wide { WIDE_WIDTH } else { WIDGET_WIDTH };
        let y = viewport[1] / unit + LIST_PADDING + place.row as f32 * ROW;
        let bounds = [x, y, x + width, y + WIDGET_HEIGHT].map(|value| value * unit);
        let bounds = [
            bounds[0],
            bounds[1] - scroll.offset,
            bounds[2],
            bounds[3] - scroll.offset,
        ];
        match entry {
            Entry::Link(label, action) => {
                button::draw(canvas, view, bounds, label, Some(*action), unit)?;
            }
            Entry::Option(id) => widget(canvas, view, translate, bounds, *id, unit)?,
            Entry::Label(label) => button::draw(canvas, view, bounds, label, None, unit)?,
            Entry::Notice(text) => put(
                canvas,
                text,
                [bounds[0], bounds[1] + 6.0 * unit],
                unit,
                [255, 85, 85, 255],
                bounds[2] - bounds[0],
            )?,
            Entry::Field {
                label,
                value,
                placeholder,
                field,
                press,
            } => {
                let split = 100.0 * unit;
                put(
                    canvas,
                    label,
                    [bounds[0], bounds[1] + 6.0 * unit],
                    unit,
                    TEXT,
                    split - 4.0 * unit,
                )?;
                crate::widgets::java_field(
                    canvas,
                    view,
                    [bounds[0] + split, bounds[1], bounds[2], bounds[3]],
                    value,
                    placeholder,
                    view.field == Some(*field),
                    Some(*press),
                    unit,
                )?;
            }
            Entry::Heading(text) => put(
                canvas,
                text,
                [bounds[0], bounds[1] + 6.0 * unit],
                unit,
                TEXT,
                bounds[2] - bounds[0],
            )?,
            Entry::Binding(index) => {
                binding(canvas, view, translate, bounds, *index, &defaults, unit)?;
            }
        }
    }
    canvas.end_scroll(scroll, content)
}

/// Draws one key binding across `bounds`: its name, the button that captures a key for it, which
/// reads "> ... <" while it waits, and a Reset button once the key is not the default.
fn binding(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    translate: Translate<'_>,
    bounds: Bounds,
    index: usize,
    defaults: &SettingsOptions,
    unit: f32,
) -> Result<(), PaintError> {
    let label = binding_label(index);
    let options = &view.settings_options;
    let capturing = view.settings.key_remap == Some(index as u16);
    let control = options.key_control(index);
    let changed = control != defaults.key_control(index);
    let mut value = control.map_or_else(
        || settings::text(view, translate, "controllerLayoutScreen.unassigned"),
        |control| options.key_name(control),
    );
    if index < GAMEPAD_OFFSET
        && let Some(binding) = BindingId::from_name(label)
        && let Some(second) = options.secondary_key_control(binding)
    {
        value.push_str(" / ");
        value.push_str(&options.key_name(second));
    }
    let shown = if capturing {
        "> ... <".to_owned()
    } else {
        value
    };
    let reset_width = 50.0 * unit;
    let gap = 4.0 * unit;
    let key = [
        bounds[2] - reset_width - gap - 100.0 * unit,
        bounds[1],
        bounds[2] - reset_width - gap,
        bounds[3],
    ];
    let name = settings::text(view, translate, label);
    put(
        canvas,
        &name,
        [bounds[0], bounds[1] + 6.0 * unit],
        unit,
        TEXT,
        key[0] - bounds[0] - gap,
    )?;
    button::draw_lit(
        canvas,
        view,
        key,
        &shown,
        Some(MenuAction::SettingsKey(index as u16)),
        capturing,
        unit,
    )?;
    let reset = [bounds[2] - reset_width, bounds[1], bounds[2], bounds[3]];
    button::draw(
        canvas,
        view,
        reset,
        "Reset",
        (changed && !capturing).then_some(MenuAction::SettingsResetKey(index as u16)),
        unit,
    )
}

/// Draws the widget of the option `id`: a slider, or a button naming its value.
fn widget(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    translate: Translate<'_>,
    bounds: Bounds,
    id: SettingId,
    unit: f32,
) -> Result<(), PaintError> {
    let option = id.definition();
    let index = id.index() as u16;
    let value = view.settings_options.get(index as usize);
    let title = settings::option_title(view, translate, id);
    let word = |key: &str| settings::text(view, translate, key);
    if id == SettingId::FullScreen {
        let label = format!("{title}: {}", on_off(view.fullscreen));
        let action = MenuAction::SettingsFullscreen(!view.fullscreen);
        return button::draw(canvas, view, bounds, &label, Some(action), unit);
    }
    match option.kind {
        SettingKind::Toggle => {
            let label = format!("{title}: {}", on_off(value != 0));
            let action = MenuAction::SettingsOption(index, 1 - value);
            button::draw(canvas, view, bounds, &label, Some(action), unit)
        }
        SettingKind::Slider => {
            let shown = settings::slider_text(id, value, &word);
            let label = if title.contains("%s") {
                title.replace("%s", &shown)
            } else {
                format!("{title}: {shown}")
            };
            let actions: Vec<MenuAction> = if id == SettingId::Msaa {
                view.settings_options
                    .anti_aliasing_support()
                    .counts()
                    .map(|samples| MenuAction::SettingsOption(index, samples as i32))
                    .collect()
            } else {
                (option.min..=option.max)
                    .step_by(option.step as usize)
                    .map(|step| MenuAction::SettingsOption(index, step))
                    .collect()
            };
            let selected = actions
                .iter()
                .position(|action| *action == MenuAction::SettingsOption(index, value))
                .unwrap_or(0);
            slider(canvas, view, bounds, &label, &actions, selected, unit)
        }
        SettingKind::Dropdown(choices) => {
            let visible: Vec<usize> = (0..choices.len())
                .filter(|choice| view.setting_choice_visible(&option, *choice))
                .collect();
            let current = usize::try_from(value).unwrap_or(0);
            let name = choices
                .get(current)
                .map_or_else(String::new, |choice| word(choice.label));
            let next = visible
                .iter()
                .position(|choice| *choice == current)
                .and_then(|at| visible.get((at + 1) % visible.len().max(1)))
                .or_else(|| visible.first())
                .copied();
            let label = format!("{title}: {name}");
            let action = next.map(|choice| MenuAction::SettingsOption(index, choice as i32));
            button::draw(canvas, view, bounds, &label, action, unit)
        }
        SettingKind::Host => Ok(()),
    }
}

fn on_off(on: bool) -> &'static str {
    if on { "ON" } else { "OFF" }
}

/// Draws a slider over `bounds` with `label` centred on it: Java's track and handle, one step
/// per action, the handle following the pointer while it drags.
fn slider(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    bounds: Bounds,
    label: &str,
    actions: &[MenuAction],
    selected: usize,
    unit: f32,
) -> Result<(), PaintError> {
    let row = actions
        .first()
        .and_then(|action| action.settings_slider_row());
    let pointer = view
        .settings
        .slider_pointer
        .filter(|pointer| Some(pointer.option) == row && pointer.fraction.is_finite());
    let steps = actions.len().saturating_sub(1).max(1) as f32;
    let fraction = pointer.map_or(selected as f32 / steps, |pointer| {
        pointer.fraction.clamp(0.0, 1.0)
    });
    let focused = view.navigation_focus_visible
        && actions
            .iter()
            .any(|action| Some(*action) == view.focused_action);
    let lit = usize::from(
        focused
            || pointer.is_some()
            || row.is_some_and(|row| {
                view.settings.slider_hovered == Some(row)
                    || view.settings.slider_selected == Some(row)
            }),
    );
    button::nine_slice(canvas, SLIDER[lit], bounds, unit)?;
    let handle_key = crate::images::java::key(HANDLE[lit]);
    let [handle_width, handle_height] = canvas
        .sprite_size(&handle_key)
        .map_or(HANDLE_SIZE, |[width, height]| [width, height]);
    let (handle_width, handle_height) = (handle_width * unit, handle_height * unit);
    let travel = (bounds[2] - bounds[0] - handle_width).max(0.0);
    let left = (bounds[0] + fraction * travel).round();
    let top = bounds[1] + ((bounds[3] - bounds[1]) - handle_height) * 0.5;
    let thumb = [left, top, left + handle_width, top + handle_height];
    canvas.sprite(&handle_key, thumb, [255; 4])?;
    button::label_centred(canvas, label, bounds, unit, TEXT)?;
    if actions.len() < 2 {
        return Ok(());
    }
    let half = handle_width * 0.5;
    let track = [bounds[0] + half, bounds[1], bounds[2] - half, bounds[3]];
    canvas.slider_track(actions[0], track, thumb)?;
    canvas.focus_target(actions[selected.min(actions.len() - 1)], track)?;
    let capture_focus = canvas.capture_focus;
    canvas.capture_focus = false;
    let result = actions.iter().enumerate().try_for_each(|(index, action)| {
        let last = actions.len() - 1;
        let step_left = if index == 0 {
            bounds[0]
        } else {
            bounds[0] + half + travel * (index as f32 - 0.5) / last as f32
        };
        let step_right = if index == last {
            bounds[2]
        } else {
            bounds[0] + half + travel * (index as f32 + 0.5) / last as f32
        };
        canvas.hit(*action, [step_left, bounds[1], step_right, bounds[3]])
    });
    canvas.capture_focus = capture_focus;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> MenuView {
        MenuView::new(true, "Player".into())
    }

    #[test]
    fn the_list_of_pages_follows_javas_order_and_names_then_this_clients_pages() {
        let view = view();
        let labels: Vec<String> = hub_entries(&view, &|_| None)
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Link(label, _) => Some(label),
                _ => None,
            })
            .collect();
        let at = |name: &str| labels.iter().position(|label| label == name);
        let (sounds, video, controls) =
            (at("Music & Sounds"), at("Video Settings"), at("Controls"));
        assert!(sounds < video && video < controls, "{labels:?}");
        assert!(controls.is_some(), "{labels:?}");
        // Java's pages come before the pages this client adds.
        let accessibility = at("Accessibility Settings").unwrap();
        assert!(labels[accessibility + 1..].iter().all(|label| {
            !["Music & Sounds", "Video Settings", "Controls"].contains(&label.as_str())
        }));
    }

    #[test]
    fn a_wide_entry_sits_alone_on_its_row_and_the_rest_pair_up() {
        let mut view = view();
        view.settings.java_view = JavaSettingsView::Page;
        view.settings.section = SettingSection::Sound.definition().index;
        let entries = page_entries(&view, SettingSection::Sound);
        let places = places(&entries);
        assert!(places[0].wide && places[0].row == 0);
        assert_eq!((places[1].row, places[1].column), (1, 0));
        assert_eq!((places[2].row, places[2].column), (1, 1));
        assert!(places.iter().skip(1).all(|place| !place.wide));
    }

    #[test]
    fn account_touch_and_the_bindings_have_java_pages_of_their_own() {
        let mut view = view();
        let opens_page = |view: &MenuView, section: SettingSection| {
            hub_entries(view, &|_| None)
                .into_iter()
                .any(|entry| match entry {
                    Entry::Link(_, MenuAction::JavaSettings(JavaSettingsAction::Page(index))) => {
                        index == section.definition().index
                    }
                    _ => false,
                })
        };
        for section in [
            SettingSection::Account,
            SettingSection::Touch,
            SettingSection::KeyboardAndMouse,
            SettingSection::ControllerAndSwitch,
        ] {
            assert!(opens_page(&view, section), "{section:?}");
        }
        // Signed out, the account page offers signing in; signed in, it offers the rest.
        let labels = |view: &MenuView, section| -> Vec<String> {
            extras(view, section)
                .into_iter()
                .filter_map(|entry| match entry {
                    Entry::Link(label, _) | Entry::Label(label) => Some(label),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(labels(&view, SettingSection::Account), ["Sign In"]);
        view.auth_state = AuthState::Authenticated;
        let signed_in = labels(&view, SettingSection::Account);
        assert!(signed_in.iter().any(|label| label == "Sign Out"));
        assert!(signed_in[0].starts_with("Signed in: "));
        // Without a touch screen the touch page says so and offers no layout editor.
        view.touch_available = false;
        assert_eq!(
            labels(&view, SettingSection::Touch),
            ["No touch screen found"]
        );
        view.touch_available = true;
        assert_eq!(labels(&view, SettingSection::Touch), ["Edit Layout"]);
    }

    #[test]
    fn every_key_binding_has_a_name_and_a_row_of_its_own() {
        let view = view();
        let keys = extras(&view, SettingSection::KeyboardAndMouse);
        let bindings: Vec<usize> = keys
            .iter()
            .filter_map(|entry| match entry {
                Entry::Binding(index) => Some(*index),
                _ => None,
            })
            .collect();
        assert_eq!(bindings.len(), KEY_BINDINGS.len() + EXTRA_KEYS.len());
        let pad = extras(&view, SettingSection::ControllerAndSwitch);
        let pad_bindings = pad
            .iter()
            .filter(|entry| matches!(entry, Entry::Binding(_)))
            .count();
        assert_eq!(pad_bindings, GAMEPAD_BINDINGS.len() + EXTRA_GAMEPAD.len());
        for entry in keys.iter().chain(&pad) {
            if let Entry::Binding(index) = entry {
                assert!(!binding_label(*index).is_empty(), "binding {index}");
                // A binding and a heading span the list, so each sits on a row of its own.
                assert!(entry.wide());
            }
        }
        let placed = places(&keys);
        assert!(placed.iter().filter(|place| place.wide).count() >= bindings.len());
    }

    #[test]
    fn the_field_of_view_is_on_the_list_of_pages_only() {
        let view = view();
        let on_a_page = SECTION_DEFINITIONS
            .iter()
            .any(|definition| page_options(&view, definition.id).contains(&SettingId::FieldOfView));
        assert!(!on_a_page);
        assert!(matches!(
            hub_entries(&view, &|_| None).first(),
            Some(Entry::Option(SettingId::FieldOfView))
        ));
    }

    #[test]
    fn the_list_of_pages_has_a_button_for_every_shown_section_with_a_title() {
        let view = view();
        let entries = hub_entries(&view, &|_| None);
        let links = entries
            .iter()
            .filter(|entry| matches!(entry, Entry::Link(..)))
            .count();
        let sections = SECTION_DEFINITIONS
            .iter()
            .filter(|definition| definition.tab_title.is_some())
            .filter(|definition| view.settings_section_shown(definition.id))
            .count();
        assert_eq!(links, sections);
    }

    #[test]
    fn a_section_with_registry_options_opens_a_java_page_and_one_without_the_bedrock_page() {
        let view = view();
        let target = |section: SettingSection| {
            hub_entries(&view, &|_| None)
                .into_iter()
                .find_map(|entry| match entry {
                    Entry::Link(_, MenuAction::JavaSettings(action)) => match action {
                        JavaSettingsAction::Page(index) | JavaSettingsAction::Bedrock(index)
                            if index == section.definition().index =>
                        {
                            Some(action)
                        }
                        _ => None,
                    },
                    _ => None,
                })
        };
        assert_eq!(
            target(SettingSection::Video),
            Some(JavaSettingsAction::Page(
                SettingSection::Video.definition().index
            ))
        );
        // A section with a page laid out by hand opens it too.
        assert_eq!(
            target(SettingSection::Language),
            Some(JavaSettingsAction::Page(
                SettingSection::Language.definition().index
            ))
        );
    }

    #[test]
    fn no_page_lists_an_option_twice_or_one_that_is_not_a_plain_setting() {
        let view = view();
        for definition in SECTION_DEFINITIONS {
            let options = page_options(&view, definition.id);
            let mut seen = Vec::new();
            for id in options {
                assert!(!seen.contains(&id), "{id:?} is listed twice");
                assert!(!matches!(id.definition().kind, SettingKind::Host));
                seen.push(id);
            }
        }
    }
}
