//! The Java-style Mods screen, laid out like the pack screen: the mods that are off on the left and
//! the active ones on the right, each row a tile with the mod's name, version and authors. Pressing
//! a row chooses it, and the footer then offers what can be done with it: enable or disable, move
//! it up or down the active list, reload and remove it. Permissions and the marketplace stay on the
//! Bedrock-style page, which "More" opens.

use launcher::{
    menu::{JavaSettingsAction, MenuAction, MenuView, settings_sections::SettingSection},
    mods::{Action, InstalledMod, Snapshot},
};
use ui_engine::engine::screen::PaintError;

use super::frame::{self, Btn, Frame, Spec, put};
use crate::{
    paint::{Bounds, Canvas},
    theme::TEXT,
};

/// Sizes in Java pixels.
const COLUMN_MAX: f32 = 200.0;
const COLUMN_GAP: f32 = 8.0;
const ENTRY_STEP: f32 = 36.0;
const ICON: f32 = 32.0;
const HEADING: f32 = 16.0;
const PADDING: f32 = 4.0;
const BAR_GAP: f32 = 2.0;
const GREY: [u8; 4] = [128, 128, 128, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const TILE: [u8; 4] = [38, 38, 38, 255];

fn command(action: Action) -> MenuAction {
    MenuAction::Mods(action)
}

/// What a mod's row says: its name, then its version and authors.
fn lines(item: &InstalledMod) -> (String, String) {
    let name = item
        .metadata
        .name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| item.file.clone());
    let detail = [
        item.metadata.version.as_deref(),
        item.metadata.authors.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    (name, detail)
}

/// The footer's buttons for the chosen mod and whether the library is free to change.
fn footer(snapshot: &Snapshot) -> (Vec<Btn<'static>>, Vec<Btn<'static>>) {
    let library = &snapshot.library;
    let idle = !library.busy;
    let (active, available) = library.groups();
    let chosen = snapshot.details_expanded;
    let on = chosen.and_then(|id| active.iter().position(|item| item.id == id));
    let off = chosen.filter(|id| available.iter().any(|item| item.id == *id));
    let import = Btn::new("Import", idle.then_some(command(Action::Import)));
    let toggle = match (on, off) {
        (Some(_), _) => Btn::new(
            "Disable",
            chosen
                .filter(|_| idle)
                .map(|id| command(Action::Disable(id))),
        ),
        (_, Some(id)) => Btn::new("Enable", idle.then_some(command(Action::Enable(id)))),
        _ => Btn::new("Enable", None),
    };
    let remove = Btn::new(
        "Remove",
        chosen
            .filter(|_| idle)
            .map(|id| command(Action::Remove(id))),
    );
    let up = Btn::new(
        "Move Up",
        on.filter(|place| idle && *place > 0)
            .and(chosen)
            .map(|id| command(Action::MoveUp(id))),
    );
    let down = Btn::new(
        "Move Down",
        on.filter(|place| idle && place + 1 < active.len())
            .and(chosen)
            .map(|id| command(Action::MoveDown(id))),
    );
    let reload = Btn::new(
        "Reload",
        (idle && !active.is_empty()).then_some(command(Action::ReloadAll)),
    );
    let done = Btn::new("Done", Some(MenuAction::AddBack));
    (vec![import, toggle, remove], vec![up, down, reload, done])
}

/// Draws the Java Mods screen.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<(), PaintError> {
    let snapshot = &*view.mods;
    let (first, second) = footer(snapshot);
    let message =
        (!snapshot.library.message.is_empty()).then_some(snapshot.library.message.as_str());
    let more = MenuAction::JavaSettings(JavaSettingsAction::Bedrock(
        SettingSection::Mods.definition().index,
    ));
    let spec = Spec {
        title: "Mods",
        subtitle: message,
        header: frame::HEADER,
        rows: &[&first, &second],
        more: Some(more),
        title_top: None,
    };
    frame::draw(canvas, view, size, &spec, |canvas, frame| {
        let (active, available) = snapshot.library.groups();
        let width = COLUMN_MAX.min(frame.centre - COLUMN_GAP / 2.0 - 12.0);
        for (is_active, heading, left, list) in [
            (
                false,
                "Available Mods",
                -width - COLUMN_GAP / 2.0,
                &available,
            ),
            (true, "Active Mods", COLUMN_GAP / 2.0, &active),
        ] {
            column(
                canvas, view, snapshot, frame, is_active, heading, left, width, list,
            )?;
        }
        Ok(())
    })
}

/// One scrolling list of mods under its heading.
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
    list: &[InstalledMod],
) -> Result<(), PaintError> {
    let unit = frame.unit;
    let [x0, x1] = frame.column(left, width);
    let viewport: Bounds = [
        x0 - 3.0 * unit,
        frame.viewport[1],
        x1 + (BAR_GAP + 6.0) * unit,
        frame.viewport[3],
    ];
    let content = (PADDING + HEADING + list.len().max(1) as f32 * ENTRY_STEP + PADDING) * unit;
    let key = if active {
        "java_mods/active"
    } else {
        "java_mods/available"
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
            if list.is_empty() {
                let text = if active {
                    "Enable mods from the left"
                } else {
                    "No mods installed"
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
            for (row, item) in list.iter().enumerate() {
                let y = top + (HEADING + row as f32 * ENTRY_STEP) * unit;
                let bounds = [x0, y, x1, y + (ENTRY_STEP - 4.0) * unit];
                entry(
                    canvas,
                    view,
                    snapshot,
                    item,
                    active.then_some(row + 1),
                    bounds,
                    unit,
                )?;
            }
            Ok(())
        },
    )
}

/// One mod's row: a tile, its name, and its version and authors, or its priority when active.
fn entry(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    snapshot: &Snapshot,
    item: &InstalledMod,
    priority: Option<usize>,
    bounds: Bounds,
    unit: f32,
) -> Result<(), PaintError> {
    let select = command(Action::ReadMore(item.id));
    let selected = snapshot.details_expanded == Some(item.id);
    let state = canvas.interaction(view, Some(select));
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
    let tile = [
        bounds[0],
        bounds[1],
        bounds[0] + ICON * unit,
        bounds[1] + ICON * unit,
    ];
    canvas.fill(tile, TILE)?;
    let (name, detail) = lines(item);
    if let Some(initial) = name.chars().next() {
        put(
            canvas,
            &initial.to_uppercase().to_string(),
            [tile[0] + 13.0 * unit, tile[1] + 12.0 * unit],
            unit,
            TEXT,
            ICON * unit,
        )?;
    }
    let text_left = bounds[0] + (ICON + 3.0) * unit;
    let room = bounds[2] - text_left;
    put(
        canvas,
        &name,
        [text_left, bounds[1] + unit],
        unit,
        TEXT,
        room,
    )?;
    let detail = match priority {
        Some(place) if detail.is_empty() => format!("Priority {place}"),
        Some(place) => format!("Priority {place} · {detail}"),
        None => detail,
    };
    put(
        canvas,
        &detail,
        [text_left, bounds[1] + 12.0 * unit],
        unit,
        GREY,
        room,
    )?;
    canvas.hit(select, bounds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher::mods::Library;

    fn installed(id: u64, name: &str) -> InstalledMod {
        InstalledMod {
            id,
            file: format!("{name}.wasm"),
            bytes: 1,
            permissions: Vec::new(),
            metadata: ui::mod_metadata::ModMetadata {
                name: (!name.is_empty()).then(|| name.to_owned()),
                version: Some("1.2".to_owned()),
                authors: Some("Someone".to_owned()),
                ..Default::default()
            },
        }
    }

    #[test]
    fn a_row_names_the_mod_with_its_version_and_authors_or_falls_back_to_its_file() {
        assert_eq!(
            lines(&installed(1, "Vantage")),
            ("Vantage".to_owned(), "1.2 · Someone".to_owned())
        );
        assert_eq!(lines(&installed(2, "")).0, ".wasm");
    }

    #[test]
    fn the_footer_offers_what_the_chosen_mod_allows() {
        let mut snapshot = Snapshot {
            library: Library {
                installed: vec![installed(1, "A"), installed(2, "B")],
                ..Default::default()
            },
            ..Default::default()
        };
        let find = |snapshot: &Snapshot, label: &str| {
            let (first, second) = footer(snapshot);
            first
                .into_iter()
                .chain(second)
                .find(|button| button.label == label)
                .and_then(|button| button.action)
        };
        // Nothing chosen: only importing works.
        assert!(find(&snapshot, "Import").is_some());
        assert!(find(&snapshot, "Enable").is_none() && find(&snapshot, "Remove").is_none());
        // A mod that is off can be enabled or removed.
        snapshot.details_expanded = Some(2);
        assert_eq!(find(&snapshot, "Enable"), Some(command(Action::Enable(2))));
        assert_eq!(find(&snapshot, "Remove"), Some(command(Action::Remove(2))));
        // A busy library accepts nothing.
        snapshot.library.busy = true;
        assert!(find(&snapshot, "Import").is_none() && find(&snapshot, "Enable").is_none());
    }
}
