//! The Java-style Create New World, Edit World and Create From Template screens, laid out as Java
//! Edition lays its world creation out: a title, tabs under it, the tab's settings as a list of
//! 150-pixel buttons that step through their choices and text fields, and Create (or Play) and
//! Cancel under it. The settings are the ones the existing screens offer, from the same model.

use bridge::GameMode;
use launcher::local_worlds::{
    Screen, Tab, WorldsView, difficulty_label, game_mode_label, options::Control,
    options::WorldOption, world::Difficulty, world::Generator, world_type_label,
};
use launcher::menu::{LocalWorldAction as A, MenuAction, MenuField, MenuView};
use ui_engine::engine::screen::PaintError;

use super::{
    button,
    frame::{self, Btn, Spec},
    options::{Entry, list},
};
use crate::paint::Canvas;

/// Sizes in Java pixels.
const TABS_TOP: f32 = 6.0;
const TAB_WIDTH: f32 = 100.0;
const TAB_GAP: f32 = 4.0;
const BUTTON_HEIGHT: f32 = 20.0;

fn local(action: A) -> MenuAction {
    MenuAction::LocalWorld(action)
}

/// The value after `current` in `values`, wrapping round to the first.
fn next<T: Copy + PartialEq>(values: &[T], current: T) -> T {
    let place = values
        .iter()
        .position(|value| *value == current)
        .unwrap_or(0);
    values[(place + 1) % values.len()]
}

const DIFFICULTIES: [Difficulty; 4] = [
    Difficulty::Peaceful,
    Difficulty::Easy,
    Difficulty::Normal,
    Difficulty::Hard,
];

/// A button that names a setting and its value, and steps it on when pressed.
fn cycle(label: &str, value: &str, action: MenuAction) -> Entry {
    Entry::Link(format!("{label}: {value}"), action)
}

/// The name field every form starts with.
fn name_field(name: &str) -> Entry {
    Entry::Field {
        label: "World Name".to_owned(),
        value: name.to_owned(),
        placeholder: "My World".to_owned(),
        field: MenuField::WorldName,
        press: local(A::NameField),
    }
}

/// The tabs Java shows, with the model tab each opens: Game, World and More. Cheats and
/// multiplayer settings share More.
const JAVA_TABS: [(&str, Tab); 3] = [
    ("Game", Tab::General),
    ("World", Tab::Advanced),
    ("More", Tab::Multiplayer),
];

/// Whether the Java tab that opens `tab` is the one showing, given the model tab that is open.
fn tab_is_open(tab: Tab, open: Tab) -> bool {
    tab == open || (tab == Tab::Multiplayer && open == Tab::Cheats)
}

/// Java's Allow Commands: the cheats switch, which lives in the Game tab.
fn allow_commands(local_view: &WorldsView, editing: bool) -> Option<Entry> {
    let options = local_view
        .edit
        .as_ref()
        .filter(|_| editing)
        .map_or(&local_view.create.options, |edit| &edit.options);
    let id = WorldOption::ActivateCheats;
    let value = options.get(id);
    let text = format!("Allow Commands: {}", if value == 0 { "OFF" } else { "ON" });
    let enabled = !local_view.busy && options.editable(id, editing);
    Some(if enabled {
        Entry::Link(text, local(A::Option(id, 1 - value)))
    } else {
        Entry::Label(text)
    })
}

/// The settings the catalog puts in `tab`, as widgets: a toggle or choice is a button that steps
/// it on, a number is a field, and one that cannot change now is a button that does nothing.
fn catalog_entries(local_view: &WorldsView, tab: Tab, editing: bool) -> Vec<Entry> {
    let options = local_view
        .edit
        .as_ref()
        .filter(|_| editing)
        .map_or(&local_view.create.options, |edit| &edit.options);
    let mut entries = Vec::new();
    for &id in WorldOption::ALL {
        let definition = id.definition();
        // Allow Commands has its own button in the Game tab.
        if definition.tab != tab
            || (editing && definition.creation_only)
            || id == WorldOption::ActivateCheats
        {
            continue;
        }
        let enabled = !local_view.busy && options.editable(id, editing);
        let value = options.get(id);
        match definition.control {
            Control::Toggle => {
                let text = format!(
                    "{}: {}",
                    definition.label,
                    if value == 0 { "OFF" } else { "ON" }
                );
                entries.push(if enabled {
                    Entry::Link(text, local(A::Option(id, 1 - value)))
                } else {
                    Entry::Label(text)
                });
            }
            Control::Choice(choices) => {
                let shown = choices
                    .iter()
                    .find(|choice| choice.value == value)
                    .map_or("", |choice| choice.label);
                let values: Vec<i32> = choices.iter().map(|choice| choice.value).collect();
                let text = format!("{}: {shown}", definition.label);
                entries.push(if enabled && !values.is_empty() {
                    Entry::Link(text, local(A::Option(id, next(&values, value))))
                } else {
                    Entry::Label(text)
                });
            }
            Control::Number { .. } => {
                let text = local_view
                    .option_text
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| value.to_string());
                if enabled {
                    entries.push(Entry::Field {
                        label: definition.label.to_owned(),
                        value: text,
                        placeholder: String::new(),
                        field: MenuField::WorldOption(id),
                        press: local(A::OptionField(id)),
                    });
                } else {
                    entries.push(Entry::Label(format!("{}: {text}", definition.label)));
                }
            }
        }
    }
    entries
}

/// The widgets of the open tab of the create or edit form.
fn entries(local_view: &WorldsView, route: Screen) -> Vec<Entry> {
    let editing = route == Screen::Edit;
    let mut found = Vec::new();
    if let Some(error) = local_view.form_error {
        found.push(Entry::Notice(error.to_owned()));
    }
    match (local_view.tab, editing) {
        (Tab::General, false) => {
            let form = &local_view.create;
            found.push(name_field(&form.name));
            found.push(cycle(
                "Game Mode",
                game_mode_label(form.game_mode),
                local(A::GameMode(next(
                    &[GameMode::Survival, GameMode::Creative],
                    form.game_mode,
                ))),
            ));
            found.push(cycle(
                "Difficulty",
                difficulty_label(form.difficulty),
                local(A::Difficulty(next(&DIFFICULTIES, form.difficulty))),
            ));
            found.extend(allow_commands(local_view, false));
            found.push(Entry::Label("Hardcore: OFF".to_owned()));
        }
        (Tab::General, true) => {
            let Some(edit) = &local_view.edit else {
                return found;
            };
            found.push(name_field(&edit.name));
            found.push(cycle(
                "Game Mode",
                game_mode_label(edit.game_mode),
                local(A::GameMode(next(
                    &[GameMode::Survival, GameMode::Creative, GameMode::Adventure],
                    edit.game_mode,
                ))),
            ));
            found.push(cycle(
                "Difficulty",
                difficulty_label(edit.difficulty),
                local(A::Difficulty(next(&DIFFICULTIES, edit.difficulty))),
            ));
            if let Some(world) = &local_view.edited {
                found.push(Entry::Label(format!(
                    "World Type: {}",
                    world_type_label(world.generator)
                )));
                found.push(Entry::Label(format!(
                    "{} · {}",
                    launcher::menu::file_size(world.size_bytes),
                    launcher::menu::civil_date(world.last_played_unix.max(world.created_unix)),
                )));
            }
            found.extend(allow_commands(local_view, true));
            found.push(Entry::Link("Delete World".to_owned(), local(A::Delete)));
        }
        (Tab::Advanced, false) => {
            let form = &local_view.create;
            found.push(Entry::Field {
                label: "World Seed".to_owned(),
                value: form.seed_text.clone(),
                placeholder: "Random".to_owned(),
                field: MenuField::WorldSeed,
                press: local(A::SeedField),
            });
            let flat = form.generator == Generator::Flat;
            found.push(cycle(
                "World Type",
                world_type_label(form.generator),
                local(A::Flat(!flat)),
            ));
            found.extend(catalog_entries(local_view, Tab::Advanced, false));
        }
        (Tab::Advanced, true) if local_view.options_available() => {
            if let Some(world) = &local_view.edited {
                found.push(Entry::Label(format!("World Seed: {}", world.seed)));
                found.push(Entry::Label(format!(
                    "World Type: {}",
                    world_type_label(world.generator)
                )));
            }
            found.extend(catalog_entries(local_view, Tab::Advanced, true));
        }
        (Tab::Multiplayer | Tab::Cheats, _) if local_view.options_available() => {
            found.extend(catalog_entries(local_view, Tab::Multiplayer, editing));
            found.extend(catalog_entries(local_view, Tab::Cheats, editing));
            if !editing {
                found.push(Entry::Link(
                    "Use a Template".to_owned(),
                    local(A::OpenTemplates),
                ));
            }
        }
        (tab, _) if tab.supported() && local_view.options_available() => {
            found.extend(catalog_entries(local_view, tab, editing));
        }
        _ => found.push(Entry::Label("Not available yet".to_owned())),
    }
    found
}

/// The footer's buttons: Create or Play, and the way out.
fn footer(local_view: &WorldsView, route: Screen) -> Vec<Btn<'static>> {
    match route {
        Screen::Create => vec![
            Btn::new(
                "Create New World",
                (!local_view.busy).then_some(local(A::Create)),
            ),
            Btn::new("Cancel", Some(MenuAction::AddBack)),
        ],
        Screen::Edit => vec![
            Btn::new(
                "Play World",
                (!local_view.busy).then_some(local(A::PlayFromEdit)),
            ),
            Btn::new("Done", Some(MenuAction::AddBack)),
        ],
        _ => vec![
            Btn::new("Create New World", Some(local(A::BeginCreate))),
            Btn::new("Back", Some(MenuAction::AddBack)),
        ],
    }
}

/// Draws the Java create, edit or template screen for `route`.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
    route: Screen,
) -> Result<(), PaintError> {
    let local_view = &view.local;
    let title = match route {
        Screen::Edit => "Edit World",
        Screen::Templates => "Create From Template",
        _ => "Create New World",
    };
    let row = footer(local_view, route);
    // The tabs take the title's place at the top of the create and edit screens, as in Java.
    let spec = Spec {
        title: if route == Screen::Templates {
            title
        } else {
            ""
        },
        subtitle: None,
        header: frame::HEADER,
        rows: &[&row],
        more: None,
        title_top: None,
        footer_gap: None,
    };
    frame::draw(canvas, view, size, &spec, |canvas, frame| {
        let unit = frame.unit;
        let java_width = (size[0] / unit).ceil();
        if route == Screen::Templates {
            let notes = [
                Entry::Heading("You don't own any templates yet.".to_owned()),
                Entry::Heading(
                    "Create a world from scratch or import your own template.".to_owned(),
                ),
            ];
            return list(
                canvas,
                view,
                &|_| None,
                frame.viewport,
                java_width,
                &notes,
                "java_world_templates",
                unit,
            );
        }
        tabs(canvas, view, frame.centre, unit)?;
        let key = format!(
            "java_world_{}_{:?}",
            if route == Screen::Edit {
                "edit"
            } else {
                "create"
            },
            local_view.tab
        );
        list(
            canvas,
            view,
            &|_| None,
            frame.viewport,
            java_width,
            &entries(local_view, route),
            &key,
            unit,
        )
    })
}

/// The tabs under the title, the open one lit.
fn tabs(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    centre: f32,
    unit: f32,
) -> Result<(), PaintError> {
    let total = JAVA_TABS.len() as f32 * TAB_WIDTH + (JAVA_TABS.len() as f32 - 1.0) * TAB_GAP;
    for (place, (label, tab)) in JAVA_TABS.into_iter().enumerate() {
        let x = centre - total / 2.0 + place as f32 * (TAB_WIDTH + TAB_GAP);
        let bounds = [x, TABS_TOP, x + TAB_WIDTH, TABS_TOP + BUTTON_HEIGHT].map(|v| v * unit);
        button::draw_lit(
            canvas,
            view,
            bounds,
            label,
            Some(local(A::Tab(tab))),
            tab_is_open(tab, view.local.tab),
            unit,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cycle_wraps_round_to_the_first_value() {
        assert_eq!(next(&DIFFICULTIES, Difficulty::Peaceful), Difficulty::Easy);
        assert_eq!(next(&DIFFICULTIES, Difficulty::Hard), Difficulty::Peaceful);
        assert_eq!(
            next(
                &[GameMode::Survival, GameMode::Creative],
                GameMode::Creative
            ),
            GameMode::Survival
        );
    }

    #[test]
    fn a_new_world_form_offers_its_name_mode_and_difficulty_and_the_game_mode_steps_on() {
        let local_view = WorldsView::default();
        let listed = entries(&local_view, Screen::Create);
        assert!(matches!(
            listed[0],
            Entry::Field {
                field: MenuField::WorldName,
                ..
            }
        ));
        let steps: Vec<MenuAction> = listed
            .iter()
            .filter_map(|entry| match entry {
                Entry::Link(_, action) => Some(*action),
                _ => None,
            })
            .collect();
        assert_eq!(steps.len(), 3);
        assert_eq!(
            steps[0],
            local(A::GameMode(next(
                &[GameMode::Survival, GameMode::Creative],
                local_view.create.game_mode
            )))
        );
    }

    #[test]
    fn the_footer_creates_or_plays_and_cancelling_is_always_there() {
        let mut local_view = WorldsView::default();
        let create = footer(&local_view, Screen::Create);
        assert_eq!(create[0].action, Some(local(A::Create)));
        assert_eq!(create[1].action, Some(MenuAction::AddBack));
        // While the world is being made nothing creates it twice.
        local_view.busy = true;
        assert!(footer(&local_view, Screen::Create)[0].action.is_none());
        let edit = footer(&WorldsView::default(), Screen::Edit);
        assert_eq!(edit[0].action, Some(local(A::PlayFromEdit)));
    }

    #[test]
    fn a_tab_with_nothing_to_set_says_so() {
        let local_view = WorldsView {
            tab: Tab::ResourcePacks,
            ..Default::default()
        };
        let listed = entries(&local_view, Screen::Create);
        assert!(matches!(&listed[0], Entry::Label(text) if text == "Not available yet"));
    }
}
