//! The Java-style Resource Packs screen, laid out as Java Edition lays its pack screen out: the
//! packs not in use on the left and the selected packs on the right, each a list of 36-pixel
//! entries (icon, name, description). Under the title a search field filters both lists and a
//! button steps the order of the packs not in use. Pressing an entry selects it; the footer then
//! offers what can be done with it: activate, deactivate, delete (after a question), and move.
//! Packs the screen cannot describe, such as variant choices, stay on the Bedrock-style page.

use launcher::{
    global_resources::{Action, Snapshot},
    menu::{
        JavaSettingsAction, MenuAction, MenuField, MenuView, settings_sections::SettingSection,
    },
};
use resource_pack::InstalledPack;
use ui_engine::engine::screen::PaintError;

use super::frame::{self, Btn, Frame, Spec, put};
use crate::{
    paint::{Bounds, Canvas},
    settings::resources::matches_query,
    theme::TEXT,
};

/// Sizes in Java pixels.
const COLUMN_MAX: f32 = 200.0;
const COLUMN_GAP: f32 = 8.0;
const ENTRY_STEP: f32 = 36.0;
const ICON: f32 = 32.0;
const HEADING: f32 = 16.0;
const PADDING: f32 = 4.0;
const HEADER: f32 = 50.0;
const SEARCH_TOP: f32 = 26.0;
const SEARCH_HEIGHT: f32 = 20.0;
const SEARCH_WIDTH: f32 = 200.0;
const SORT_WIDTH: f32 = 105.0;
const BAR_GAP: f32 = 2.0;
const GREY: [u8; 4] = [128, 128, 128, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const ICON_FILL: [u8; 4] = [38, 38, 38, 255];

fn command(action: Action) -> MenuAction {
    MenuAction::GlobalResources(action)
}

/// One pack of a list.
struct Entry<'a> {
    pack: &'a InstalledPack,
    index: usize,
    selected: bool,
    select: MenuAction,
}

/// The packs of a list that match the search, with their place in the unfiltered list.
fn entries<'a>(snapshot: &'a Snapshot, query: &str, active: bool) -> Vec<Entry<'a>> {
    let packs = if active {
        &snapshot.active
    } else {
        &snapshot.available
    };
    packs
        .iter()
        .enumerate()
        .filter(|(_, pack)| matches_query(pack, query))
        .map(|(index, pack)| Entry {
            pack,
            index,
            selected: snapshot.selected == Some((active, index)),
            select: command(if active {
                Action::SelectActive(index)
            } else {
                Action::SelectAvailable(index)
            }),
        })
        .collect()
}

/// The footer's buttons for what is selected.
fn footer(snapshot: &Snapshot) -> (Vec<Btn<'static>>, Vec<Btn<'static>>) {
    let idle = !snapshot.busy;
    let add = Btn::new("Add Pack...", idle.then_some(command(Action::Import)));
    let done = Btn::new("Done", Some(MenuAction::AddBack));
    let selected = snapshot.selected;
    let available = selected.filter(|(active, index)| !active && *index < snapshot.available.len());
    let active = selected.filter(|(active, index)| *active && *index < snapshot.active.len());
    let (second, third) = match (available, active) {
        (Some((_, index)), _) if snapshot.delete_pending == Some(index) => (
            Btn::new("Keep", idle.then_some(command(Action::CancelDelete))),
            Btn::new(
                "Delete Pack",
                idle.then_some(command(Action::Delete(index))),
            ),
        ),
        (Some((_, index)), _) => (
            Btn::new("Activate", idle.then_some(command(Action::Activate(index)))),
            Btn::new("Delete", idle.then_some(command(Action::AskDelete(index)))),
        ),
        (_, Some((_, index))) => (
            Btn::new(
                "Deactivate",
                idle.then_some(command(Action::Deactivate(index))),
            ),
            Btn::new("Delete", None),
        ),
        _ => (Btn::new("Activate", None), Btn::new("Delete", None)),
    };
    let (up, down) = match active {
        Some((_, index)) => (
            Btn::new(
                "Move Up",
                (idle && index > 0).then_some(command(Action::MoveUp(index))),
            ),
            Btn::new(
                "Move Down",
                (idle && index + 1 < snapshot.active.len())
                    .then_some(command(Action::MoveDown(index))),
            ),
        ),
        None => (Btn::new("Move Up", None), Btn::new("Move Down", None)),
    };
    (vec![add, second, third], vec![up, down, done])
}

/// Draws the Java Resource Packs screen.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<(), PaintError> {
    let snapshot = &*view.global_resources;
    let (first, second) = footer(snapshot);
    let status = if snapshot.message.is_empty() {
        None
    } else {
        Some(snapshot.message.as_str())
    };
    let more = MenuAction::JavaSettings(JavaSettingsAction::Bedrock(
        SettingSection::GlobalTexturePack.definition().index,
    ));
    let spec = Spec {
        title: "Resource Packs",
        subtitle: status,
        header: HEADER,
        rows: &[&first, &second],
        more: Some(more),
    };
    frame::draw(canvas, view, size, &spec, |canvas, frame| {
        search_row(canvas, view, snapshot, frame)?;
        let width = COLUMN_MAX.min(frame.centre - COLUMN_GAP / 2.0 - 12.0);
        let columns = [
            (false, "Available Packs", -width - COLUMN_GAP / 2.0),
            (true, "Selected Packs", COLUMN_GAP / 2.0),
        ];
        for (active, heading, left) in columns {
            let list = entries(snapshot, &view.pack_query, active);
            column(
                canvas, view, snapshot, frame, active, heading, left, width, &list,
            )?;
        }
        Ok(())
    })
}

/// The search field and the sort button under the title.
fn search_row(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    snapshot: &Snapshot,
    frame: &Frame,
) -> Result<(), PaintError> {
    let unit = frame.unit;
    let width = SEARCH_WIDTH.min(frame.centre * 2.0 - SORT_WIDTH - 24.0);
    let left = frame.centre - (width + 5.0 + SORT_WIDTH) / 2.0;
    let field = [left, SEARCH_TOP, left + width, SEARCH_TOP + SEARCH_HEIGHT].map(|v| v * unit);
    crate::widgets::java_field(
        canvas,
        view,
        field,
        &view.pack_query,
        "Search packs",
        view.field == Some(MenuField::PackQuery),
        Some(command(Action::Search)),
        unit,
    )?;
    let sort = [
        left + width + 5.0,
        SEARCH_TOP,
        left + width + 5.0 + SORT_WIDTH,
        SEARCH_TOP + SEARCH_HEIGHT,
    ]
    .map(|v| v * unit);
    super::button::draw(
        canvas,
        view,
        sort,
        &format!("Sort: {}", snapshot.sort.label()),
        (!snapshot.busy).then_some(command(Action::CycleSort)),
        unit,
    )
}

/// One scrolling list of packs under its heading.
#[allow(clippy::too_many_arguments)]
fn column(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    snapshot: &Snapshot,
    frame: &Frame,
    active: bool,
    heading: &str,
    left: f32,
    width: f32,
    list: &[Entry<'_>],
) -> Result<(), PaintError> {
    let unit = frame.unit;
    let [x0, x1] = frame.column(left, width);
    let viewport: Bounds = [
        x0,
        frame.viewport[1],
        x1 + (BAR_GAP + 6.0) * unit,
        frame.viewport[3],
    ];
    // The selected list ends in the base pack, which stays under every other.
    let rows = list.len() + usize::from(active);
    let content = (PADDING + HEADING + rows as f32 * ENTRY_STEP + PADDING) * unit;
    let key = if active {
        "java_packs/active"
    } else {
        "java_packs/available"
    };
    frame::scrolled(
        canvas,
        key,
        viewport,
        content,
        x1 + BAR_GAP * unit,
        unit,
        |canvas, offset| {
            let top = viewport[1] - offset + PADDING * unit;
            put(
                canvas,
                heading,
                [x0 + unit, top + 4.0 * unit],
                unit,
                TEXT,
                width * unit,
            )?;
            if list.is_empty() && !active {
                let text = if view.pack_query.is_empty() {
                    "No packs imported"
                } else {
                    "No packs match"
                };
                put(
                    canvas,
                    text,
                    [x0 + unit, top + (HEADING + 4.0) * unit],
                    unit,
                    GREY,
                    width * unit,
                )?;
            }
            for (row, entry) in list.iter().enumerate() {
                let y = top + (HEADING + row as f32 * ENTRY_STEP) * unit;
                let bounds = [x0, y, x1, y + (ENTRY_STEP - 4.0) * unit];
                let subtitle = if active {
                    format!("Priority {}", entry.index + 1)
                } else {
                    String::new()
                };
                pack_entry(canvas, view, snapshot, entry, &subtitle, bounds, unit)?;
            }
            if active {
                let y = top + (HEADING + list.len() as f32 * ENTRY_STEP) * unit;
                base_entry(canvas, [x0, y, x1, y + (ENTRY_STEP - 4.0) * unit], unit)?;
            }
            Ok(())
        },
    )
}

/// The box that marks a selected entry, or a faint one under the pointer.
fn mark(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    bounds: Bounds,
    selected: bool,
    action: Option<MenuAction>,
    unit: f32,
) -> Result<(), PaintError> {
    let state = canvas.interaction(view, action);
    if selected {
        let outer = [
            bounds[0] - 2.0 * unit,
            bounds[1] - 2.0 * unit,
            bounds[2] + 2.0 * unit,
            bounds[3] + 2.0 * unit,
        ];
        canvas.fill(outer, GREY)?;
        canvas.fill(
            [
                outer[0] + unit,
                outer[1] + unit,
                outer[2] - unit,
                outer[3] - unit,
            ],
            BLACK,
        )?;
    } else if state.hovered || state.focused {
        canvas.fill(bounds, [255, 255, 255, 24])?;
    }
    Ok(())
}

/// The entry of one pack: its icon, name, and the first lines of its description.
fn pack_entry(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    snapshot: &Snapshot,
    entry: &Entry<'_>,
    subtitle: &str,
    bounds: Bounds,
    unit: f32,
) -> Result<(), PaintError> {
    mark(
        canvas,
        view,
        bounds,
        entry.selected,
        Some(entry.select),
        unit,
    )?;
    let icon = [
        bounds[0],
        bounds[1],
        bounds[0] + ICON * unit,
        bounds[1] + ICON * unit,
    ];
    canvas.fill(icon, ICON_FILL)?;
    let art = snapshot
        .icons
        .get(&(entry.pack.id.to_string(), entry.pack.revision))
        .and_then(|path| canvas.artwork?.get(path.as_str()).copied());
    if let Some(art) = art {
        canvas.icon_ref(art, icon)?;
    }
    let text_left = bounds[0] + (ICON + 3.0) * unit;
    let room = bounds[2] - text_left;
    put(
        canvas,
        &entry.pack.name,
        [text_left, bounds[1] + unit],
        unit,
        TEXT,
        room,
    )?;
    let version = entry.pack.version.map(|part| part.to_string()).join(".");
    let detail = if subtitle.is_empty() {
        format!("v{version}")
    } else {
        format!("{subtitle} · v{version}")
    };
    put(
        canvas,
        &detail,
        [text_left, bounds[1] + 12.0 * unit],
        unit,
        GREY,
        room,
    )?;
    let description: String = entry
        .pack
        .description
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    put(
        canvas,
        &description,
        [text_left, bounds[1] + 21.0 * unit],
        unit,
        GREY,
        room,
    )?;
    canvas.hit(entry.select, bounds)
}

/// The base game's textures, always at the bottom of the selected list.
fn base_entry(canvas: &mut Canvas<'_>, bounds: Bounds, unit: f32) -> Result<(), PaintError> {
    let icon = [
        bounds[0],
        bounds[1],
        bounds[0] + ICON * unit,
        bounds[1] + ICON * unit,
    ];
    canvas.fill(icon, ICON_FILL)?;
    let text_left = bounds[0] + (ICON + 3.0) * unit;
    let room = bounds[2] - text_left;
    put(
        canvas,
        "Vanilla Textures",
        [text_left, bounds[1] + unit],
        unit,
        TEXT,
        room,
    )?;
    put(
        canvas,
        "Base pack · Always active",
        [text_left, bounds[1] + 12.0 * unit],
        unit,
        GREY,
        room,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(name: &str, description: &str) -> InstalledPack {
        InstalledPack {
            id: Default::default(),
            version: [1, 0, 0],
            name: name.to_owned(),
            description: description.to_owned(),
            min_engine_version: None,
            subpacks: Vec::new(),
            revision: 0,
        }
    }

    #[test]
    fn the_search_keeps_each_packs_place_in_its_list() {
        let snapshot = Snapshot {
            available: vec![
                pack("Alpha", "stone"),
                pack("Beta", "wood"),
                pack("Gamma", "stone and wood"),
            ],
            selected: Some((false, 2)),
            ..Default::default()
        };
        let all = entries(&snapshot, "", false);
        assert_eq!(all.len(), 3);
        let found = entries(&snapshot, "stone", false);
        assert_eq!(
            found.iter().map(|entry| entry.index).collect::<Vec<_>>(),
            [0, 2]
        );
        // The chosen pack stays chosen, and pressing an entry names its real index.
        assert!(found[1].selected && !found[0].selected);
        assert_eq!(found[1].select, command(Action::SelectAvailable(2)));
        assert!(entries(&snapshot, "missing", false).is_empty());
        assert!(entries(&snapshot, "stone", true).is_empty());
    }

    #[test]
    fn the_footer_offers_what_the_selection_allows() {
        let mut snapshot = Snapshot {
            available: vec![pack("A", ""), pack("B", "")],
            active: vec![pack("C", ""), pack("D", "")],
            ..Default::default()
        };
        let labels = |snapshot: &Snapshot| {
            let (first, second) = footer(snapshot);
            first
                .iter()
                .chain(&second)
                .map(|button| (button.label, button.action.is_some()))
                .collect::<Vec<_>>()
        };
        // Nothing chosen: only adding and leaving work.
        let none = labels(&snapshot);
        assert!(none.contains(&("Add Pack...", true)) && none.contains(&("Activate", false)));
        snapshot.selected = Some((false, 1));
        assert!(labels(&snapshot).contains(&("Activate", true)));
        assert!(labels(&snapshot).contains(&("Delete", true)));
        // Asking to delete swaps in the confirmation.
        snapshot.delete_pending = Some(1);
        let asking = labels(&snapshot);
        assert!(asking.contains(&("Delete Pack", true)) && asking.contains(&("Keep", true)));
        snapshot.delete_pending = None;
        // A selected pack at the top of the priority list cannot move up.
        snapshot.selected = Some((true, 0));
        let top = labels(&snapshot);
        assert!(top.contains(&("Move Up", false)) && top.contains(&("Move Down", true)));
        // A busy library accepts nothing.
        snapshot.busy = true;
        assert!(labels(&snapshot).contains(&("Add Pack...", false)));
    }
}
