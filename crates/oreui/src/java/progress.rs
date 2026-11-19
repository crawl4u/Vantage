//! The Java-style loading screen, as Java Edition shows a world or server being joined or the
//! terrain loading: the world dimmed behind it, a thin green bar over the title, what is happening
//! in lines under it, and a tile that fills with white squares from its middle as the terrain
//! arrives. The bar and the tile follow the real progress; while it is not known the bar sweeps
//! to and fro so something always shows. Cancel appears where the join can be called off.

use launcher::menu::{MenuAction, MenuView};
use ui_engine::engine::screen::PaintError;

use super::{
    button,
    frame::{Btn, wrap_lines},
    unit, unit_for,
};
use crate::{focus, paint::Canvas, theme::TEXT};

/// Sizes in Java pixels.
const TEXT_WIDTH: f32 = 300.0;
const LINE: f32 = 10.0;
const CANCEL_WIDTH: f32 = 200.0;
const CANCEL_HEIGHT: f32 = 20.0;
/// The thin bar over the text: its width, height and the gap under it.
const BAR_WIDTH: f32 = 100.0;
const BAR_HEIGHT: f32 = 2.0;
const BAR_GAP: f32 = 6.0;
/// The tile under the text, its cells across, each cell's size and the border round it.
const CELLS: usize = 5;
const CELL: f32 = 5.0;
const CELL_GAP: f32 = 1.0;
const TILE_PAD: f32 = 3.0;
const TILE_BORDER: f32 = 1.0;
const TILE_GAP: f32 = 8.0;
const DIM: [u8; 4] = [0, 0, 0, 130];
const BAR_FILL: [u8; 4] = [88, 200, 88, 255];
const BAR_TRACK: [u8; 4] = [48, 48, 48, 255];
const DETAIL: [u8; 4] = [170, 170, 170, 255];
const EMPTY_CELL: [u8; 4] = [40, 40, 40, 255];
const LOADED_CELL: [u8; 4] = [255, 255, 255, 255];

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

/// Which cells of the tile are loaded when `fraction` of the terrain is: they fill from the
/// middle outward, as the chunks around the player do.
fn loaded_cells(fraction: f32) -> [[bool; CELLS]; CELLS] {
    let mut order: Vec<(usize, usize)> = (0..CELLS)
        .flat_map(|row| (0..CELLS).map(move |column| (row, column)))
        .collect();
    let middle = (CELLS as f32 - 1.0) / 2.0;
    let distance = |(row, column): (usize, usize)| {
        (row as f32 - middle).powi(2) + (column as f32 - middle).powi(2)
    };
    // Cells at one distance fill in turn round the middle, so the blob grows evenly.
    let angle = |(row, column): (usize, usize)| (row as f32 - middle).atan2(column as f32 - middle);
    order.sort_by(|a, b| {
        distance(*a)
            .total_cmp(&distance(*b))
            .then(angle(*a).total_cmp(&angle(*b)))
    });
    let count = (fraction.clamp(0.0, 1.0) * (CELLS * CELLS) as f32).round() as usize;
    let mut loaded = [[false; CELLS]; CELLS];
    for (row, column) in order.into_iter().take(count) {
        loaded[row][column] = true;
    }
    loaded
}

/// Where an unknown amount's bar segment is, as a share of the bar's width, `seconds` in: a
/// third of the bar sweeping out and back.
fn sweep(seconds: f64) -> [f32; 2] {
    let wave = (seconds * 1.4).sin() as f32 * 0.5 + 0.5;
    let start = wave * (1.0 - 1.0 / 3.0);
    [start, start + 1.0 / 3.0]
}

/// Draws the loading screen at `unit` logical pixels to a Java pixel over `size`. `cancel` is the
/// button that calls the join off, with the menu view it reads its hover from.
fn paint(
    canvas: &mut Canvas<'_>,
    unit: f32,
    size: [f32; 2],
    title: &str,
    detail: &str,
    fraction: Option<f32>,
    cancel: Option<(&MenuView, MenuAction)>,
) -> Result<(), PaintError> {
    let java_width = (size[0] / unit).ceil();
    let java_height = size[1] / unit;
    let centre = (java_width / 2.0).floor();
    canvas.fill([0.0, 0.0, size[0], size[1]], DIM)?;
    let region =
        canvas.begin_focus_region(focus::SCREEN, [0.0, 0.0, size[0], size[1]], None, true)?;
    canvas.focus_delegate(cancel.map(|(_, action)| action), None);
    let lines = wrap_lines(detail, TEXT_WIDTH.min(java_width - 20.0));
    let tile =
        CELLS as f32 * CELL + (CELLS as f32 - 1.0) * CELL_GAP + 2.0 * (TILE_PAD + TILE_BORDER);
    let height = BAR_HEIGHT
        + BAR_GAP
        + LINE * (1.0 + lines.len() as f32)
        + TILE_GAP
        + tile
        + cancel.map_or(0.0, |_| CANCEL_HEIGHT + 12.0);
    let mut y = ((java_height - height) / 2.0).max(20.0);
    // The bar, over the text. Without a known amount a segment of it sweeps to and fro.
    let left = (centre - BAR_WIDTH / 2.0) * unit;
    let bar = [
        left,
        y * unit,
        left + BAR_WIDTH * unit,
        (y + BAR_HEIGHT) * unit,
    ];
    canvas.fill(bar, BAR_TRACK)?;
    let width = bar[2] - bar[0];
    let [from, to] = match fraction {
        Some(fraction) => [0.0, fraction.clamp(0.0, 1.0)],
        None => {
            if let Some(transitions) = canvas.transitions.as_deref_mut() {
                transitions.marquee_drawn = true;
            }
            sweep(canvas.seconds)
        }
    };
    canvas.fill(
        [bar[0] + width * from, bar[1], bar[0] + width * to, bar[3]],
        BAR_FILL,
    )?;
    y += BAR_HEIGHT + BAR_GAP;
    let line = |canvas: &mut Canvas<'_>, text: &str, y: f32, colour: [u8; 4]| {
        button::label_centred(
            canvas,
            text,
            [0.0, y * unit, size[0], (y + 8.0) * unit],
            unit,
            colour,
        )
    };
    line(canvas, &title_case(title), y, TEXT)?;
    y += LINE;
    for text in &lines {
        line(canvas, text, y, DETAIL)?;
        y += LINE;
    }
    // The tile: a grey border round black, the loaded cells white and the rest dark.
    y += TILE_GAP;
    let outer = [
        (centre - tile / 2.0) * unit,
        y * unit,
        (centre + tile / 2.0) * unit,
        (y + tile) * unit,
    ];
    canvas.fill(outer, [110, 110, 110, 255])?;
    canvas.fill(
        [
            outer[0] + TILE_BORDER * unit,
            outer[1] + TILE_BORDER * unit,
            outer[2] - TILE_BORDER * unit,
            outer[3] - TILE_BORDER * unit,
        ],
        [0, 0, 0, 255],
    )?;
    let loaded = loaded_cells(fraction.unwrap_or(0.0));
    let origin = [
        outer[0] + (TILE_BORDER + TILE_PAD) * unit,
        outer[1] + (TILE_BORDER + TILE_PAD) * unit,
    ];
    for (row, cells) in loaded.iter().enumerate() {
        for (column, on) in cells.iter().enumerate() {
            let x = origin[0] + column as f32 * (CELL + CELL_GAP) * unit;
            let top = origin[1] + row as f32 * (CELL + CELL_GAP) * unit;
            canvas.fill(
                [x, top, x + CELL * unit, top + CELL * unit],
                if *on { LOADED_CELL } else { EMPTY_CELL },
            )?;
        }
    }
    y += tile;
    if let Some((view, action)) = cancel {
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

/// Draws the loading screen of a join from the menu, with a Cancel where the join allows one.
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
    paint(
        canvas,
        unit,
        size,
        title,
        detail,
        fraction,
        cancel.map(|action| (view, action)),
    )
}

/// Draws the loading screen while the world itself loads, with no menu to read: the player's Java
/// GUI scale (none for Auto) sets its size. The terrain stage says what Java says.
pub(crate) fn draw_world(
    canvas: &mut Canvas<'_>,
    scale: Option<u8>,
    size: [f32; 2],
    terrain: bool,
    title: &str,
    detail: &str,
    fraction: Option<f32>,
) -> Result<(), PaintError> {
    let unit = unit_for(size, scale, canvas.metrics.dpi_scale.get());
    let (title, detail) = if terrain {
        ("Loading terrain...", "")
    } else {
        (title, detail)
    };
    paint(canvas, unit, size, title, detail, fraction, None)
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

    #[test]
    fn the_tile_fills_from_its_middle_outward() {
        let count = |fraction| {
            loaded_cells(fraction)
                .iter()
                .flatten()
                .filter(|cell| **cell)
                .count()
        };
        assert_eq!(count(0.0), 0);
        assert_eq!(count(1.0), CELLS * CELLS);
        let few = loaded_cells(0.04);
        assert!(
            few[CELLS / 2][CELLS / 2],
            "the first cell is the middle one"
        );
        assert!(!few[0][0]);
        // More terrain never unloads a cell.
        let (some, more) = (loaded_cells(0.3), loaded_cells(0.6));
        for row in 0..CELLS {
            for column in 0..CELLS {
                assert!(!some[row][column] || more[row][column]);
            }
        }
    }

    #[test]
    fn an_unknown_amount_sweeps_a_third_of_the_bar_and_stays_inside_it() {
        for step in 0..100 {
            let [from, to] = sweep(f64::from(step) * 0.1);
            assert!(0.0 <= from && to <= 1.0 + 1e-6 && (to - from - 1.0 / 3.0).abs() < 1e-6);
        }
    }
}
