//! The Java-style Language screen, as Java Edition lists its languages: a scrolling list with one
//! row for each language, the chosen one boxed, and Done under it. Pressing a row chooses that
//! language at once.

use launcher::menu::{JavaSettingsAction, MenuAction, MenuView, settings_sections::SettingSection};
use ui_engine::engine::screen::PaintError;

use super::frame::{self, Btn, Frame, Spec, put};
use crate::{
    paint::{Bounds, Canvas},
    theme::TEXT,
};

/// Sizes in Java pixels.
const ROW_WIDTH: f32 = 305.0;
const ROW_HEIGHT: f32 = 18.0;
const ROW_STEP: f32 = 20.0;
const PADDING: f32 = 6.0;
const BAR_OFFSET: f32 = 164.0;
const GREY: [u8; 4] = [128, 128, 128, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

/// One language of the list.
struct Row<'a> {
    name: &'a str,
    selected: bool,
    select: MenuAction,
}

/// The languages in the order the game lists them, with the one in use marked. With no choice
/// saved the first, the game's own default, is the one in use.
fn rows(view: &MenuView) -> Vec<Row<'_>> {
    let chosen = view.settings_options.language();
    view.language_choices
        .iter()
        .enumerate()
        .filter_map(|(index, (code, name))| {
            Some(Row {
                name,
                selected: chosen.map_or(index == 0, |chosen| chosen == code),
                select: MenuAction::SettingsLanguage(u16::try_from(index).ok()?),
            })
        })
        .collect()
}

/// Draws the Java Language screen.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<(), PaintError> {
    let done = [Btn::new("Done", Some(MenuAction::AddBack))];
    let more = MenuAction::JavaSettings(JavaSettingsAction::Bedrock(
        SettingSection::Language.definition().index,
    ));
    let spec = Spec {
        title: "Language",
        subtitle: None,
        header: frame::HEADER,
        rows: &[&done],
        more: Some(more),
    };
    frame::draw(canvas, view, size, &spec, |canvas, frame| {
        list(canvas, view, frame)
    })
}

/// The scrolling list of languages.
fn list(canvas: &mut Canvas<'_>, view: &MenuView, frame: &Frame) -> Result<(), PaintError> {
    let unit = frame.unit;
    let rows = rows(view);
    let content = (PADDING * 2.0 + rows.len() as f32 * ROW_STEP) * unit;
    let left = (frame.centre - (ROW_WIDTH / 2.0).floor()) * unit;
    frame::scrolled(
        canvas,
        "java_language",
        frame.viewport,
        content,
        (frame.centre + BAR_OFFSET) * unit,
        unit,
        |canvas, offset| {
            let top = frame.viewport[1] - offset + PADDING * unit;
            for (index, row) in rows.iter().enumerate() {
                let y = top + index as f32 * ROW_STEP * unit;
                let bounds = [left, y, left + ROW_WIDTH * unit, y + ROW_HEIGHT * unit];
                entry(canvas, view, row, bounds, unit)?;
            }
            Ok(())
        },
    )
}

/// One row: boxed while chosen, lit under the pointer.
fn entry(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    row: &Row<'_>,
    bounds: Bounds,
    unit: f32,
) -> Result<(), PaintError> {
    let state = canvas.interaction(view, Some(row.select));
    if row.selected {
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
    put(
        canvas,
        row.name,
        [bounds[0] + 4.0 * unit, bounds[1] + 5.0 * unit],
        unit,
        TEXT,
        bounds[2] - bounds[0] - 8.0 * unit,
    )?;
    canvas.hit(row.select, bounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view_with(codes: &[(&str, &str)]) -> MenuView {
        let mut view = MenuView::new(true, "Player".into());
        view.language_choices = codes
            .iter()
            .map(|(code, name)| ((*code).to_owned(), (*name).to_owned()))
            .collect();
        view
    }

    #[test]
    fn the_first_language_is_in_use_until_one_is_chosen() {
        let view = view_with(&[("en_US", "English"), ("de_DE", "Deutsch")]);
        let listed = rows(&view);
        assert_eq!(listed.len(), 2);
        assert!(listed[0].selected && !listed[1].selected);
        assert_eq!(listed[1].select, MenuAction::SettingsLanguage(1));
    }

    #[test]
    fn the_saved_language_is_the_one_marked() {
        let mut view = view_with(&[("en_US", "English"), ("de_DE", "Deutsch")]);
        std::sync::Arc::make_mut(&mut view.settings_options).set_language("de_DE");
        let listed = rows(&view);
        assert!(!listed[0].selected && listed[1].selected);
    }
}
