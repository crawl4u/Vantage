//! The Java-style Resource Packs screen, laid out as Java Edition lays its pack screen out: the
//! packs not in use on the left and the selected packs on the right, each a list of 36-pixel
//! entries (icon, name, description). Under the title a search field filters both lists and a
//! button steps the order of the packs not in use. Pressing an entry selects it; the footer then
//! offers what can be done with it: activate, deactivate, delete (after a question), and move.
//! Packs the screen cannot describe, such as variant choices, stay on the Bedrock-style page.

use java_resource_pack::menu_files::VANILLA_PACK_ICON;
use launcher::{
    global_resources::{Action, Snapshot},
    menu::{
        JavaSettingsAction, MenuAction, MenuField, MenuView,
        settings_sections::SettingSection,
        settings_storage::{StorageAction, StorageFolder},
    },
};
use resource_pack::InstalledPack;
use ui_engine::engine::screen::PaintError;

use super::{
    frame::{self, Btn, Frame, Spec, put},
    pack_drag::DragEntry,
};
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
/// The row of folder buttons under the heading of the list of unused packs.
const TOOLBAR: f32 = 18.0;
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
    /// It is in the selected list.
    active: bool,
    index: usize,
    selected: bool,
    select: MenuAction,
}

/// One row of a list: a pack, or in the list of packs not in use a way into or out of a folder.
enum Row<'a> {
    /// The folder above the open one.
    Back,
    /// A folder inside the open one: its place in [`Snapshot::subfolders`], path and contents.
    Folder {
        index: usize,
        path: &'a str,
        folders: usize,
        packs: usize,
    },
    Pack(Entry<'a>),
}

/// The rows of a list. The list of packs not in use shows the open folder's folders and packs
/// while nothing is searched for, and every pack that matches otherwise.
fn rows<'a>(snapshot: &'a Snapshot, query: &str, active: bool) -> Vec<Row<'a>> {
    let searching = !query.trim().is_empty();
    let mut rows = Vec::new();
    if !active && !searching {
        if !snapshot.folder.is_empty() {
            rows.push(Row::Back);
        }
        for (index, path) in snapshot.subfolders().into_iter().enumerate() {
            let inside = |other: &str| resource_pack::folder_parent(other) == path;
            rows.push(Row::Folder {
                index,
                path,
                folders: snapshot
                    .folders
                    .iter()
                    .filter(|other| inside(other))
                    .count(),
                packs: snapshot
                    .available_folders
                    .iter()
                    .filter(|folder| folder.as_str() == path)
                    .count(),
            });
        }
    }
    rows.extend(
        entries(snapshot, query, active)
            .into_iter()
            .filter(|entry| active || searching || snapshot.in_open_folder(entry.index))
            .map(Row::Pack),
    );
    rows
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
            active,
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
/// `opens_folders` says whether this machine can open the folder packs are kept in.
fn footer(snapshot: &Snapshot, opens_folders: bool) -> (Vec<Btn<'static>>, Vec<Btn<'static>>) {
    let idle = !snapshot.busy;
    let add = Btn::new("Add Pack", idle.then_some(command(Action::Import)));
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
    let folder = Btn::new(
        "Open Folder",
        opens_folders.then_some(MenuAction::SettingsStorage(StorageAction::OpenFolder(
            StorageFolder::ResourcePacks,
        ))),
    );
    (vec![add, second, third], vec![up, down, folder, done])
}

/// Draws the Java Resource Packs screen.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<(), PaintError> {
    let snapshot = &*view.global_resources;
    let (first, second) = footer(snapshot, view.storage.opens_folders);
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
        if let Some(transitions) = canvas.transitions.as_deref_mut() {
            transitions.pack_drag.begin_layout(snapshot.active.len());
        }
        search_row(canvas, view, snapshot, frame)?;
        let width = COLUMN_MAX.min(frame.centre - COLUMN_GAP / 2.0 - 12.0);
        let columns = [
            (false, "Available Packs", -width - COLUMN_GAP / 2.0),
            (true, "Selected Packs", COLUMN_GAP / 2.0),
        ];
        for (active, heading, left) in columns {
            let list = rows(snapshot, &view.pack_query, active);
            let heading = if !active && !snapshot.folder.is_empty() {
                format!("{heading} / {}", snapshot.folder.replace('/', " / "))
            } else {
                heading.to_owned()
            };
            column(
                canvas, view, snapshot, frame, active, &heading, left, width, &list,
            )?;
        }
        held(canvas, snapshot, frame)
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
    list: &[Row<'_>],
) -> Result<(), PaintError> {
    let unit = frame.unit;
    let [x0, x1] = frame.column(left, width);
    // The list reaches a little past its entries so a selected one's box is not cut off.
    let viewport: Bounds = [
        x0 - 3.0 * unit,
        frame.viewport[1],
        x1 + (BAR_GAP + 6.0) * unit,
        frame.viewport[3],
    ];
    // The selected list ends in the base pack, which stays under every other.
    if let Some(transitions) = canvas.transitions.as_deref_mut() {
        transitions
            .pack_drag
            .list(active, [x0, viewport[1], x1, viewport[3]]);
    }
    let rows = list.len() + usize::from(active) + usize::from(list.is_empty() && !active);
    let lead = HEADING + if active { 0.0 } else { TOOLBAR };
    let content = (PADDING + lead + rows as f32 * ENTRY_STEP + PADDING) * unit;
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
            if !active {
                folder_buttons(canvas, view, snapshot, [x0, x1, top + HEADING * unit], unit)?;
            }
            if list.is_empty() && !active {
                let text = if view.pack_query.is_empty() {
                    "Drop packs in, or use Add Pack"
                } else {
                    "No packs match"
                };
                put(
                    canvas,
                    text,
                    [x0 + unit, top + (lead + 4.0) * unit],
                    unit,
                    GREY,
                    width * unit,
                )?;
            }
            for (row, item) in list.iter().enumerate() {
                let y = top + (lead + row as f32 * ENTRY_STEP) * unit;
                let bounds = [x0, y, x1, y + (ENTRY_STEP - 4.0) * unit];
                match item {
                    Row::Pack(entry) => {
                        let subtitle = if active {
                            format!("Priority {}", entry.index + 1)
                        } else {
                            String::new()
                        };
                        pack_entry(canvas, view, snapshot, entry, &subtitle, bounds, unit)?;
                    }
                    Row::Back => {
                        let parent = resource_pack::folder_parent(&snapshot.folder);
                        let caption = if parent.is_empty() {
                            "Back to all packs".to_owned()
                        } else {
                            format!("Back to {}", resource_pack::folder_name(parent))
                        };
                        folder_row(
                            canvas,
                            view,
                            bounds,
                            "..",
                            &caption,
                            true,
                            None,
                            command(Action::FolderUp),
                            unit,
                        )?;
                    }
                    Row::Folder {
                        index,
                        path,
                        folders,
                        packs,
                    } => folder_row(
                        canvas,
                        view,
                        bounds,
                        resource_pack::folder_name(path),
                        &format!("Folders: {folders} / Packs: {packs}"),
                        false,
                        Some(*index),
                        command(Action::OpenFolder(*index)),
                        unit,
                    )?,
                }
            }
            if active {
                let y = top + (lead + list.len() as f32 * ENTRY_STEP) * unit;
                let bounds = [x0, y, x1, y + (ENTRY_STEP - 4.0) * unit];
                base_entry(canvas, view, snapshot.base_selected, bounds, unit)?;
            }
            Ok(())
        },
    )
}

/// The row under the heading of the list of unused packs: make a folder, and inside one rename
/// or delete it. While a folder is being renamed the row is the name field with OK and Cancel.
fn folder_buttons(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    snapshot: &Snapshot,
    [x0, x1, top]: [f32; 3],
    unit: f32,
) -> Result<(), PaintError> {
    let idle = !snapshot.busy;
    let height = 14.0 * unit;
    let gap = 3.0 * unit;
    if view.pack_renaming && !snapshot.folder.is_empty() {
        let small = 36.0 * unit;
        let field_right = x1 - 2.0 * (small + gap);
        crate::widgets::java_field(
            canvas,
            view,
            [x0, top, field_right, top + height],
            &view.pack_folder_name,
            "Folder name",
            view.field == Some(launcher::menu::MenuField::PackFolderName),
            Some(command(Action::FolderName)),
            unit,
        )?;
        let ok = [
            field_right + gap,
            top,
            field_right + gap + small,
            top + height,
        ];
        super::button::draw(
            canvas,
            view,
            ok,
            "OK",
            Some(command(Action::CommitRename)),
            unit,
        )?;
        let cancel = [ok[2] + gap, top, x1, top + height];
        return super::button::draw(
            canvas,
            view,
            cancel,
            "X",
            Some(command(Action::CancelRename)),
            unit,
        );
    }
    let mut buttons = vec![("New Folder", Action::NewFolder)];
    if !snapshot.folder.is_empty() {
        buttons.push(("Rename", Action::RenameFolder));
        buttons.push(("Delete", Action::DeleteFolder));
    }
    let count = buttons.len() as f32;
    let width = (x1 - x0 - gap * (count - 1.0)) / count;
    for (place, (label, action)) in buttons.into_iter().enumerate() {
        let left = x0 + place as f32 * (width + gap);
        super::button::draw(
            canvas,
            view,
            [left, top, left + width, top + height],
            label,
            idle.then_some(command(action)),
            unit,
        )?;
    }
    Ok(())
}

/// Draws a folder with squares: a tab on a body, its front lighter, and when `up` a green arrow.
/// It is the client's own picture, so no game file is needed.
pub(crate) fn folder_icon(
    canvas: &mut Canvas<'_>,
    icon: Bounds,
    up: bool,
) -> Result<(), PaintError> {
    let cell = (icon[2] - icon[0]) / 32.0;
    let at = |left: f32, top: f32, right: f32, bottom: f32| {
        [
            icon[0] + left * cell,
            icon[1] + top * cell,
            icon[0] + right * cell,
            icon[1] + bottom * cell,
        ]
    };
    canvas.fill(at(3.0, 7.0, 15.0, 12.0), [170, 126, 34, 255])?;
    canvas.fill(at(3.0, 10.0, 29.0, 27.0), [170, 126, 34, 255])?;
    canvas.fill(at(4.0, 8.0, 14.0, 11.0), [232, 184, 64, 255])?;
    canvas.fill(at(4.0, 11.0, 28.0, 26.0), [244, 200, 84, 255])?;
    canvas.fill(at(4.0, 15.0, 28.0, 26.0), [252, 220, 120, 255])?;
    if up {
        canvas.fill(at(21.0, 17.0, 25.0, 25.0), [64, 168, 64, 255])?;
        canvas.fill(at(19.0, 17.0, 27.0, 20.0), [64, 168, 64, 255])?;
        canvas.fill(at(20.0, 15.0, 26.0, 18.0), [64, 168, 64, 255])?;
        canvas.fill(at(22.0, 13.0, 24.0, 16.0), [64, 168, 64, 255])?;
    }
    Ok(())
}

/// A folder's row, or the row that goes up one: its icon, name and what it holds. Pressing it
/// opens it, and a pack dragged onto it goes into it.
#[allow(clippy::too_many_arguments)]
fn folder_row(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    bounds: Bounds,
    name: &str,
    caption: &str,
    up: bool,
    target: Option<usize>,
    open: MenuAction,
    unit: f32,
) -> Result<(), PaintError> {
    let over = canvas
        .transitions
        .as_deref()
        .and_then(|transitions| transitions.pack_drag.over_target())
        == Some(target);
    mark(canvas, view, bounds, over, Some(open), unit)?;
    let icon = [
        bounds[0],
        bounds[1],
        bounds[0] + ICON * unit,
        bounds[1] + ICON * unit,
    ];
    folder_icon(canvas, icon, up)?;
    let text_left = bounds[0] + (ICON + 3.0) * unit;
    let room = bounds[2] - text_left;
    put(
        canvas,
        name,
        [text_left, bounds[1] + unit],
        unit,
        TEXT,
        room,
    )?;
    put(
        canvas,
        caption,
        [text_left, bounds[1] + 12.0 * unit],
        unit,
        GREY,
        room,
    )?;
    if let Some(transitions) = canvas.transitions.as_deref_mut() {
        transitions.pack_drag.target(bounds, target);
    }
    canvas.hit(open, bounds)
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
    pack_face(canvas, snapshot, entry.pack, subtitle, bounds, unit)?;
    canvas.hit(entry.select, bounds)?;
    let hot = arrows(canvas, view, snapshot, entry, bounds, unit)?;
    if let Some(transitions) = canvas.transitions.as_deref_mut() {
        transitions.pack_drag.entry(DragEntry {
            active: entry.active,
            index: entry.index,
            bounds,
            select: entry.select,
            hot,
        });
    }
    Ok(())
}

/// Which way an arrow over a pack's icon points, and the sprite that draws it at rest and under
/// the pointer.
#[derive(Clone, Copy)]
enum Arrow {
    Select,
    Unselect,
    Up,
    Down,
}

impl Arrow {
    fn sprites(self) -> [&'static str; 2] {
        match self {
            Self::Select => [
                "sprites/transferable_list/select.png",
                "sprites/transferable_list/select_highlighted.png",
            ],
            Self::Unselect => [
                "sprites/transferable_list/unselect.png",
                "sprites/transferable_list/unselect_highlighted.png",
            ],
            Self::Up => [
                "sprites/transferable_list/move_up.png",
                "sprites/transferable_list/move_up_highlighted.png",
            ],
            Self::Down => [
                "sprites/transferable_list/move_down.png",
                "sprites/transferable_list/move_down_highlighted.png",
            ],
        }
    }
}

/// Draws `arrow` in `zone` with plain squares, for a game or pack that has no picture for it.
fn plain_arrow(
    canvas: &mut Canvas<'_>,
    zone: Bounds,
    arrow: Arrow,
    lit: bool,
    unit: f32,
) -> Result<(), PaintError> {
    let colour = if lit {
        [255, 255, 160, 255]
    } else {
        [255, 255, 255, 255]
    };
    let centre = [(zone[0] + zone[2]) * 0.5, (zone[1] + zone[3]) * 0.5];
    // A triangle of five stripes, two Java pixels thick.
    for step in 0..5 {
        let along = (step as f32 - 2.0) * 2.0 * unit;
        let span = (2.5 - (step as f32 - 2.0).abs()) * 2.0 * unit;
        let stripe = match arrow {
            Arrow::Select => [
                centre[0] + along - unit,
                centre[1] - span,
                centre[0] + along + unit,
                centre[1] + span,
            ],
            Arrow::Unselect => [
                centre[0] - along - unit,
                centre[1] - span,
                centre[0] - along + unit,
                centre[1] + span,
            ],
            Arrow::Up => [
                centre[0] - span,
                centre[1] - along - unit,
                centre[0] + span,
                centre[1] - along + unit,
            ],
            Arrow::Down => [
                centre[0] - span,
                centre[1] + along - unit,
                centre[0] + span,
                centre[1] + along + unit,
            ],
        };
        canvas.fill(stripe, colour)?;
    }
    Ok(())
}

/// The arrows Java shows over a hovered pack's icon: a pack that is not in use can be selected,
/// a selected one deselected, and moved up or down. Each is the game's sprite, or the pack's own
/// when a resource pack restyles it, else a plain arrow. Returns the areas that act on a press,
/// so a drag does not start from them.
fn arrows(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    snapshot: &Snapshot,
    entry: &Entry<'_>,
    bounds: Bounds,
    unit: f32,
) -> Result<[Option<Bounds>; 3], PaintError> {
    let idle = !snapshot.busy;
    let icon = [
        bounds[0],
        bounds[1],
        bounds[0] + ICON * unit,
        bounds[1] + ICON * unit,
    ];
    let half = ICON * unit * 0.5;
    let mut zones: Vec<(Bounds, Arrow, MenuAction)> = Vec::new();
    if !entry.active {
        zones.push((icon, Arrow::Select, command(Action::Activate(entry.index))));
    } else {
        zones.push((
            [icon[0], icon[1], icon[0] + half, icon[3]],
            Arrow::Unselect,
            command(Action::Deactivate(entry.index)),
        ));
        if entry.index > 0 {
            zones.push((
                [icon[0] + half, icon[1], icon[2], icon[1] + half],
                Arrow::Up,
                command(Action::MoveUp(entry.index)),
            ));
        }
        if entry.index + 1 < snapshot.active.len() {
            zones.push((
                [icon[0] + half, icon[1] + half, icon[2], icon[3]],
                Arrow::Down,
                command(Action::MoveDown(entry.index)),
            ));
        }
    }
    let mut hot = [None; 3];
    if !idle {
        return Ok(hot);
    }
    for (slot, (zone, _, action)) in zones.iter().enumerate() {
        canvas.pointer_hit(*action, *zone)?;
        hot[slot] = Some(*zone);
    }
    let hovered = canvas.interaction(view, Some(entry.select)).hovered
        || zones
            .iter()
            .any(|(_, _, action)| canvas.interaction(view, Some(*action)).hovered);
    if !hovered {
        return Ok(hot);
    }
    canvas.fill(icon, [144, 144, 144, 160])?;
    for (zone, arrow, action) in &zones {
        let lit = canvas.interaction(view, Some(*action)).hovered;
        let [rest, highlighted] = arrow.sprites();
        let key = crate::images::java::key(if lit { highlighted } else { rest });
        // The sprite spans the whole icon, with the arrow in its own part of it.
        if !canvas.sprite(&key, icon, [255; 4])? {
            plain_arrow(canvas, *zone, *arrow, lit, unit)?;
        }
    }
    Ok(hot)
}

/// A pack's icon, name, version and description in `bounds`.
fn pack_face(
    canvas: &mut Canvas<'_>,
    snapshot: &Snapshot,
    pack: &InstalledPack,
    subtitle: &str,
    bounds: Bounds,
    unit: f32,
) -> Result<(), PaintError> {
    let icon = [
        bounds[0],
        bounds[1],
        bounds[0] + ICON * unit,
        bounds[1] + ICON * unit,
    ];
    canvas.fill(icon, ICON_FILL)?;
    let art = snapshot
        .icons
        .get(&(pack.id.to_string(), pack.revision))
        .and_then(|path| canvas.artwork?.get(path.as_str()).copied());
    if let Some(art) = art {
        canvas.icon_ref(art, icon)?;
    }
    let text_left = bounds[0] + (ICON + 3.0) * unit;
    let room = bounds[2] - text_left;
    put(
        canvas,
        &pack.name,
        [text_left, bounds[1] + unit],
        unit,
        TEXT,
        room,
    )?;
    let version = pack.version.map(|part| part.to_string()).join(".");
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
    let description: String = pack
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
    Ok(())
}

/// The held pack drawn under the pointer, and the line showing where it would land.
fn held(canvas: &mut Canvas<'_>, snapshot: &Snapshot, frame: &Frame) -> Result<(), PaintError> {
    let Some((entry, pointer, grab, marker)) = canvas.transitions.as_deref().and_then(|t| {
        let (entry, pointer, grab) = t.pack_drag.held()?;
        Some((entry, pointer, grab, t.pack_drag.marker()))
    }) else {
        return Ok(());
    };
    let unit = frame.unit;
    if let Some([left, right, y]) = marker {
        canvas.fill([left, y - unit, right, y + unit], [255, 255, 255, 255])?;
    }
    let packs = if entry.active {
        &snapshot.active
    } else {
        &snapshot.available
    };
    let Some(pack) = packs.get(entry.index) else {
        return Ok(());
    };
    let size = [
        entry.bounds[2] - entry.bounds[0],
        entry.bounds[3] - entry.bounds[1],
    ];
    let at = [pointer.x() - grab[0], pointer.y() - grab[1]];
    let bounds = [at[0], at[1], at[0] + size[0], at[1] + size[1]];
    canvas.fill(bounds, [0, 0, 0, 200])?;
    pack_face(canvas, snapshot, pack, "", bounds, unit)
}

/// The base game's textures, always at the bottom of the selected list. It can be chosen, to see
/// what it is, but not moved or taken out.
fn base_entry(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    selected: bool,
    bounds: Bounds,
    unit: f32,
) -> Result<(), PaintError> {
    let select = command(Action::SelectBase);
    mark(canvas, view, bounds, selected, Some(select), unit)?;
    let icon = [
        bounds[0],
        bounds[1],
        bounds[0] + ICON * unit,
        bounds[1] + ICON * unit,
    ];
    // The game's own picture, else a plain square.
    if !canvas.sprite(&crate::images::java::key(VANILLA_PACK_ICON), icon, [255; 4])? {
        canvas.fill(icon, ICON_FILL)?;
    }
    canvas.hit(select, bounds)?;
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
            let (first, second) = footer(snapshot, true);
            first
                .iter()
                .chain(&second)
                .map(|button| (button.label, button.action.is_some()))
                .collect::<Vec<_>>()
        };
        // Nothing chosen: only adding and leaving work.
        let none = labels(&snapshot);
        assert!(none.contains(&("Add Pack", true)) && none.contains(&("Activate", false)));
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
        assert!(labels(&snapshot).contains(&("Add Pack", false)));
    }
}
