//! The Java-style Storage screen: what the game keeps on disk, as a scrolling list. Worlds and the
//! server packs it downloaded are rows with their size and date, and pressing one selects it for
//! Delete. Under the list sit the buttons that open the folders, and clear all downloaded server
//! packs or the screenshots.

use launcher::menu::{
    JavaSettingsAction, MenuAction, MenuView,
    settings_sections::SettingSection,
    settings_storage::{StorageAction, StorageFolder, StorageItem},
};
use ui_engine::engine::screen::PaintError;

use super::frame::{self, Btn, Frame, Spec, put};
use crate::{
    paint::{Bounds, Canvas},
    theme::TEXT,
};

/// Sizes in Java pixels.
const ROW_WIDTH: f32 = 305.0;
const ITEM_HEIGHT: f32 = 22.0;
const ITEM_STEP: f32 = 26.0;
const HEADING_STEP: f32 = 18.0;
const PADDING: f32 = 6.0;
const BAR_OFFSET: f32 = 164.0;
const GREY: [u8; 4] = [128, 128, 128, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

/// One row of the list.
enum Row {
    Heading(String),
    Note(String),
    Item {
        title: String,
        detail: String,
        selected: bool,
        select: Option<MenuAction>,
    },
}

impl Row {
    fn step(&self) -> f32 {
        match self {
            Self::Heading(_) | Self::Note(_) => HEADING_STEP,
            Self::Item { .. } => ITEM_STEP,
        }
    }
}

/// `bytes` as megabytes or gigabytes with two decimals.
fn size_text(bytes: u64) -> String {
    if bytes >= 1 << 30 {
        format!("{:.2} GB", bytes as f64 / f64::from(1_u32 << 30))
    } else {
        format!("{:.2} MB", bytes as f64 / f64::from(1_u32 << 20))
    }
}

/// What a category holds in a line: how many and how large.
fn summary(items: &[StorageItem]) -> String {
    let bytes = items
        .iter()
        .fold(0_u64, |total, item| total.saturating_add(item.bytes));
    format!("{} · {}", items.len(), size_text(bytes))
}

/// The rows: the saved worlds, then the downloaded server packs.
fn rows(view: &MenuView) -> Vec<Row> {
    let storage = &view.storage;
    let mut rows = Vec::new();
    if storage.loading {
        rows.push(Row::Note("Loading storage...".to_owned()));
    }
    rows.push(Row::Heading(format!(
        "Worlds ({})",
        summary(&storage.worlds)
    )));
    if storage.worlds.is_empty() && !storage.loading {
        rows.push(Row::Note("No worlds saved".to_owned()));
    }
    for (index, item) in storage.worlds.iter().enumerate() {
        rows.push(Row::Item {
            title: item.name.clone(),
            detail: format!("{} · {}", size_text(item.bytes), item.date),
            selected: storage.selected_world == Some(index),
            select: (!storage.loading).then_some(MenuAction::SettingsStorage(
                StorageAction::SelectWorld(index),
            )),
        });
    }
    rows.push(Row::Heading(format!(
        "Downloaded server packs ({})",
        summary(&storage.cached)
    )));
    if storage.cached.is_empty() && !storage.loading {
        rows.push(Row::Note("No downloaded packs".to_owned()));
    }
    for (index, item) in storage.cached.iter().enumerate() {
        if !item.matches_query(&view.storage_query) {
            continue;
        }
        let version = if item.version.is_empty() {
            "Metadata unavailable"
        } else {
            &item.version
        };
        rows.push(Row::Item {
            title: item.name.clone(),
            detail: format!("{version} · {}", size_text(item.bytes)),
            selected: storage.selected == Some(index),
            select: (!storage.loading)
                .then_some(MenuAction::SettingsStorage(StorageAction::Select(index))),
        });
    }
    rows
}

/// The footer's buttons.
fn footer(view: &MenuView) -> (Vec<Btn<'static>>, Vec<Btn<'static>>) {
    let storage = &view.storage;
    let idle = !storage.loading;
    let open = |folder: StorageFolder| {
        storage
            .opens_folders
            .then_some(MenuAction::SettingsStorage(StorageAction::OpenFolder(
                folder,
            )))
    };
    let chosen = storage.selected.is_some() || storage.selected_world.is_some();
    let first = vec![
        Btn::new(
            "Delete Selected",
            (idle && chosen).then_some(MenuAction::SettingsStorage(StorageAction::RequestDelete)),
        ),
        Btn::new("Open Worlds", open(StorageFolder::Worlds)),
        Btn::new("Open Packs", open(StorageFolder::ResourcePacks)),
    ];
    let second = vec![
        Btn::new(
            "Clear Packs",
            (idle && !storage.cached.is_empty())
                .then_some(MenuAction::SettingsStorage(StorageAction::RequestClear)),
        ),
        Btn::new(
            "Clear Shots",
            idle.then_some(MenuAction::SettingsStorage(
                StorageAction::RequestScreenshots,
            )),
        ),
        Btn::new("Done", Some(MenuAction::AddBack)),
    ];
    (first, second)
}

/// Draws the Java Storage screen.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<(), PaintError> {
    let (first, second) = footer(view);
    let more = MenuAction::JavaSettings(JavaSettingsAction::Bedrock(
        SettingSection::StorageManagement.definition().index,
    ));
    let spec = Spec {
        title: "Storage",
        subtitle: None,
        header: frame::HEADER,
        rows: &[&first, &second],
        more: Some(more),
    };
    frame::draw(canvas, view, size, &spec, |canvas, frame| {
        list(canvas, view, frame)
    })
}

/// The scrolling list.
fn list(canvas: &mut Canvas<'_>, view: &MenuView, frame: &Frame) -> Result<(), PaintError> {
    let unit = frame.unit;
    let rows = rows(view);
    let content = (PADDING * 2.0 + rows.iter().map(Row::step).sum::<f32>()) * unit;
    let left = (frame.centre - (ROW_WIDTH / 2.0).floor()) * unit;
    frame::scrolled(
        canvas,
        "java_storage",
        frame.viewport,
        content,
        (frame.centre + BAR_OFFSET) * unit,
        unit,
        |canvas, offset| {
            let mut y = frame.viewport[1] - offset + PADDING * unit;
            for row in &rows {
                let width = ROW_WIDTH * unit;
                match row {
                    Row::Heading(text) => {
                        put(canvas, text, [left, y + 5.0 * unit], unit, TEXT, width)?
                    }
                    Row::Note(text) => put(
                        canvas,
                        text,
                        [left + 4.0 * unit, y + 5.0 * unit],
                        unit,
                        GREY,
                        width,
                    )?,
                    Row::Item {
                        title,
                        detail,
                        selected,
                        select,
                    } => {
                        let bounds = [left, y, left + width, y + ITEM_HEIGHT * unit];
                        item(
                            canvas, view, title, detail, *selected, *select, bounds, unit,
                        )?;
                    }
                }
                y += row.step() * unit;
            }
            Ok(())
        },
    )
}

/// One item: boxed while chosen, lit under the pointer, its name over its size and date.
#[allow(clippy::too_many_arguments)]
fn item(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    title: &str,
    detail: &str,
    selected: bool,
    select: Option<MenuAction>,
    bounds: Bounds,
    unit: f32,
) -> Result<(), PaintError> {
    let state = canvas.interaction(view, select);
    if selected {
        canvas.fill(bounds, GREY)?;
        canvas.fill(
            [
                bounds[0] + unit,
                bounds[1] + unit,
                bounds[2] - unit,
                bounds[3] - unit,
            ],
            BLACK,
        )?;
    } else if state.hovered || state.focused {
        canvas.fill(bounds, [255, 255, 255, 24])?;
    }
    let room = bounds[2] - bounds[0] - 8.0 * unit;
    put(
        canvas,
        title,
        [bounds[0] + 4.0 * unit, bounds[1] + 3.0 * unit],
        unit,
        TEXT,
        room,
    )?;
    put(
        canvas,
        detail,
        [bounds[0] + 4.0 * unit, bounds[1] + 12.0 * unit],
        unit,
        GREY,
        room,
    )?;
    if let Some(action) = select {
        canvas.hit(action, bounds)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(name: &str) -> StorageItem {
        StorageItem {
            name: name.to_owned(),
            bytes: 3 << 20,
            date: "2026-01-02".to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn sizes_read_in_megabytes_and_gigabytes() {
        assert_eq!(size_text(3 << 20), "3.00 MB");
        assert_eq!(size_text(3 << 30), "3.00 GB");
        assert_eq!(summary(&[world("a"), world("b")]), "2 · 6.00 MB");
    }

    #[test]
    fn worlds_and_packs_are_rows_and_the_chosen_one_is_marked() {
        let mut view = MenuView::new(true, "Player".into());
        std::sync::Arc::make_mut(&mut view.storage).worlds = vec![world("Home"), world("Away")];
        std::sync::Arc::make_mut(&mut view.storage).selected_world = Some(1);
        let listed = rows(&view);
        let items: Vec<(&str, bool)> = listed
            .iter()
            .filter_map(|row| match row {
                Row::Item {
                    title, selected, ..
                } => Some((title.as_str(), *selected)),
                _ => None,
            })
            .collect();
        assert_eq!(items, [("Home", false), ("Away", true)]);
        // Delete waits for something to be chosen.
        let (first, _) = footer(&view);
        assert!(first[0].action.is_some());
        std::sync::Arc::make_mut(&mut view.storage).selected_world = None;
        assert!(footer(&view).0[0].action.is_none());
        // While the lists load nothing can be chosen or cleared.
        std::sync::Arc::make_mut(&mut view.storage).loading = true;
        let listed = rows(&view);
        assert!(
            listed
                .iter()
                .any(|row| matches!(row, Row::Note(text) if text.contains("Loading")))
        );
        assert!(footer(&view).1[1].action.is_none());
    }
}
