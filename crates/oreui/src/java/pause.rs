//! The Java-style pause menu, laid out as Java Edition lays its game menu out: the title 40 Java
//! pixels down and the buttons in 24-pixel rows from a quarter of the height plus 8, the first and
//! last 204 pixels wide and the ones between in pairs 98 pixels wide. Java's own rows for
//! advancements, statistics and feedback have no counterpart here, so the rows hold what this
//! menu offers: resume, wardrobe, options and save and quit. The world shows dimmed behind it.

use launcher::menu::{MenuAction, MenuScreen, MenuView};
use ui_engine::engine::screen::PaintError;

use super::{button, home, unit};
use crate::{focus, paint::Canvas, theme::TEXT};

/// Sizes in Java pixels.
const FULL: f32 = 204.0;
const HALF: f32 = 98.0;
const HEIGHT: f32 = 20.0;
const STEP: f32 = 24.0;
/// Space between the two buttons of a pair.
const PAIR_GAP: f32 = 8.0;
/// Where the title's text starts, and how tall Java's text is.
const TITLE_TOP: f32 = 40.0;
const TEXT_HEIGHT: f32 = 8.0;
/// The darkening of the world behind the menu, as Java's pause background.
const DIM: [u8; 4] = [16, 16, 16, 200];

type Rect = [f32; 4];

/// Where every control sits for a window `width` by `height` Java pixels.
struct Layout {
    title: Rect,
    resume: Rect,
    wardrobe: Rect,
    options: Rect,
    quit: Rect,
    toggle: Rect,
}

impl Layout {
    fn new(width: f32, height: f32) -> Self {
        let centre = (width / 2.0).floor();
        let top = (height / 4.0).floor() + 8.0;
        let rect = |x: f32, row: f32, w: f32| {
            let y = top + STEP * row;
            [x, y, x + w, y + HEIGHT]
        };
        Self {
            title: [0.0, TITLE_TOP, width, TITLE_TOP + TEXT_HEIGHT],
            resume: rect(centre - FULL / 2.0, 0.0, FULL),
            wardrobe: rect(centre - PAIR_GAP / 2.0 - HALF, 1.0, HALF),
            options: rect(centre + PAIR_GAP / 2.0, 1.0, HALF),
            quit: rect(centre - FULL / 2.0, 2.0, FULL),
            toggle: [width - 2.0 - 120.0, 2.0, width - 2.0, 2.0 + HEIGHT],
        }
    }
}

/// Draws the Java pause menu.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<(), PaintError> {
    let unit = unit(view, canvas, size);
    let layout = Layout::new((size[0] / unit).ceil(), (size[1] / unit).ceil());
    let px = |rect: Rect| rect.map(|value| value * unit);
    canvas.fill([0.0, 0.0, size[0], size[1]], DIM)?;
    let screen =
        canvas.begin_focus_region(focus::SCREEN, [0.0, 0.0, size[0], size[1]], None, true)?;
    canvas.focus_delegate(Some(MenuAction::PauseResume), None);
    button::label_centred(canvas, "Game Menu", px(layout.title), unit, TEXT)?;
    for (rect, label, action) in [
        (layout.resume, "Back to Game", MenuAction::PauseResume),
        (
            layout.wardrobe,
            "Wardrobe",
            MenuAction::Navigate(MenuScreen::DressingRoom),
        ),
        (layout.options, "Options...", MenuAction::PauseSettings),
        (
            layout.quit,
            "Save and Quit to Title",
            MenuAction::PauseDisconnect,
        ),
    ] {
        button::draw(canvas, view, px(rect), label, Some(action), unit)?;
    }
    home::button_toggle(canvas, view, px(layout.toggle), unit)?;
    canvas.end_focus_region(screen);
    Ok(())
}
