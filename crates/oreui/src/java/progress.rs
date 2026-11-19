//! The Java-style loading screen, as Java Edition shows a world or server being joined: the
//! screen dimmed, a title with what is happening in lines under it, a thin bar when the progress
//! is known, and Cancel where the join can be called off.

use launcher::menu::{MenuAction, MenuView};
use ui_engine::engine::screen::PaintError;

use super::{
    button,
    frame::{Btn, wrap_lines},
    unit,
};
use crate::{focus, paint::Canvas, theme::TEXT};

/// Sizes in Java pixels.
const TEXT_WIDTH: f32 = 300.0;
const LINE: f32 = 10.0;
/// The thin bar over the text: its width, height and the gap under it.
const BAR_WIDTH: f32 = 100.0;
const BAR_HEIGHT: f32 = 2.0;
const BAR_GAP: f32 = 6.0;
/// The black tile under the text, as Java shows the terrain being loaded, and the white square in it.
const TILE: f32 = 24.0;
const TILE_MARK: f32 = 8.0;
const TILE_GAP: f32 = 8.0;
const CANCEL_WIDTH: f32 = 200.0;
const CANCEL_HEIGHT: f32 = 20.0;
const DIM: [u8; 4] = [0, 0, 0, 110];
const BAR_FILL: [u8; 4] = [88, 200, 88, 255];
const BAR_TRACK: [u8; 4] = [48, 48, 48, 255];
const DETAIL: [u8; 4] = [170, 170, 170, 255];

/// The title as Java writes one: words capitalised, not shouting in capitals.
fn title_case(title: &str) -> String {
    let lower = title.to_lowercase();
    lower
        .split(' ')
        .map(|word| {
            let mut letters = word.chars();
            letters.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + letters.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Draws the loading screen over the dimmed menu.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
    title: &str,
    detail: &str,
    fraction: Option<f32>,
    cancel: Option<MenuAction>,
) -> Result<(), PaintError> {
    let unit = unit(view, canvas, size);
    let java_width = (size[0] / unit).ceil();
    let java_height = size[1] / unit;
    let centre = (java_width / 2.0).floor();
    canvas.fill([0.0, 0.0, size[0], size[1]], DIM)?;
    let region =
        canvas.begin_focus_region(focus::SCREEN, [0.0, 0.0, size[0], size[1]], None, true)?;
    canvas.focus_delegate(cancel, None);
    let lines = wrap_lines(detail, TEXT_WIDTH.min(java_width - 20.0));
    let height = LINE * (1.0 + lines.len() as f32)
        + fraction.map_or(0.0, |_| BAR_HEIGHT + BAR_GAP)
        + TILE
        + TILE_GAP
        + cancel.map_or(0.0, |_| CANCEL_HEIGHT + 12.0);
    let mut y = ((java_height - height) / 2.0).max(20.0);
    let line = |canvas: &mut Canvas<'_>, text: &str, y: f32, colour: [u8; 4]| {
        button::label_centred(
            canvas,
            text,
            [0.0, y * unit, size[0], (y + 8.0) * unit],
            unit,
            colour,
        )
    };
    // The bar sits over the text, the way Java draws the terrain loading.
    if let Some(fraction) = fraction {
        let left = (centre - BAR_WIDTH / 2.0) * unit;
        let bar = [
            left,
            y * unit,
            left + BAR_WIDTH * unit,
            (y + BAR_HEIGHT) * unit,
        ];
        canvas.fill(bar, BAR_TRACK)?;
        let filled = bar[0] + (bar[2] - bar[0]) * fraction.clamp(0.0, 1.0);
        canvas.fill([bar[0], bar[1], filled, bar[3]], BAR_FILL)?;
        y += BAR_HEIGHT + BAR_GAP;
    }
    line(canvas, &title_case(title), y, TEXT)?;
    y += LINE;
    for text in &lines {
        line(canvas, text, y, DETAIL)?;
        y += LINE;
    }
    // A black tile with a white square in it stands for the terrain taking shape.
    y += TILE_GAP;
    let tile = [
        (centre - TILE / 2.0) * unit,
        y * unit,
        (centre + TILE / 2.0) * unit,
        (y + TILE) * unit,
    ];
    canvas.fill(tile, [0, 0, 0, 255])?;
    let mark = [
        (centre - TILE_MARK / 2.0) * unit,
        (y + (TILE - TILE_MARK) / 2.0) * unit,
        (centre + TILE_MARK / 2.0) * unit,
        (y + (TILE + TILE_MARK) / 2.0) * unit,
    ];
    canvas.fill(mark, [255, 255, 255, 255])?;
    y += TILE;
    if let Some(action) = cancel {
        y += 12.0;
        let item = Btn::new("Cancel", Some(action));
        let bounds = [
            (centre - CANCEL_WIDTH / 2.0) * unit,
            y * unit,
            (centre + CANCEL_WIDTH / 2.0) * unit,
            (y + CANCEL_HEIGHT) * unit,
        ];
        button::draw(canvas, view, bounds, item.label, item.action, unit)?;
    }
    canvas.end_focus_region(region);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shouted_title_is_written_the_way_java_writes_one() {
        assert_eq!(title_case("STARTING WORLD"), "Starting World");
        assert_eq!(title_case("Joining"), "Joining");
        assert_eq!(title_case(""), "");
    }
}
