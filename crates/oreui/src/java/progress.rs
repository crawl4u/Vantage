//! The Java-style loading screen, as Java Edition shows a world or server being joined or the
//! terrain loading: the world dimmed behind it, a thin green bar over the title that fills as the
//! whole opening goes on, what is happening in lines under it, and a tile of the chunks round the
//! player that turn white as they arrive. Cancel appears where the join can be called off.
//!
//! One bar runs through every screen of the opening, never backwards: starting or joining takes
//! it to half, and the terrain loading takes it the rest of the way.

use launcher::local_worlds::Stage;
use launcher::menu::{JoinStage, MenuAction, MenuView};
use ui::LoadingGrid;
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
/// The tile under the text: the room for its cells, and the border and padding round them.
/// The cells across, each cell's size and the gap between cells: whole Java pixels, so every cell
/// is alike and the tile is the same size whatever it shows.
const SIDE: usize = 11;
const CELL: f32 = 2.0;
const CELL_GAP: f32 = 1.0;
const TILE_INNER: f32 = SIDE as f32 * (CELL + CELL_GAP) - CELL_GAP;
const TILE_PAD: f32 = 3.0;
const TILE_BORDER: f32 = 1.0;
const TILE_GAP: f32 = 8.0;
/// How far through the bar the menu's part of an opening takes it; the terrain does the rest.
const MENU_PART: f32 = 0.5;
const DIM: [u8; 4] = [0, 0, 0, 130];
const BAR_FILL: [u8; 4] = [88, 200, 88, 255];
const BAR_TRACK: [u8; 4] = [48, 48, 48, 255];
const DETAIL: [u8; 4] = [170, 170, 170, 255];
const EMPTY_CELL: [u8; 4] = [48, 48, 48, 255];
const NEEDED_CELL: [u8; 4] = [70, 70, 70, 255];
const LOADED_CELL: [u8; 4] = [255, 255, 255, 255];

/// What the world loading screen waits on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorldLoad {
    /// The connection to the world is not made yet.
    Joining,
    /// The terrain round the player is arriving.
    Terrain,
    /// The terrain of another dimension is arriving.
    Dimension,
    /// Something that is not the terrain, such as applying resource packs.
    Other,
}

/// The title as Java writes one: a title shouted in capitals has its words capitalised, and any
/// other is kept as it is.
fn title_case(title: &str) -> String {
    if title.chars().any(char::is_lowercase) {
        return title.to_owned();
    }
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

/// How far a local world's opening has got, from 0.0 to the half the menu's part takes it: the
/// steps run in order and the one under way fills its own share.
fn local_progress(stage: Stage, fraction: Option<f32>) -> f32 {
    // Setting the server up comes first on a first run and takes the first tenth.
    let (start, end) = match stage {
        Stage::CheckingDocker | Stage::PullingImage | Stage::DownloadingServer => (0.0, 0.08),
        Stage::InstallingServer => (0.08, 0.1),
        Stage::StartingServer => (0.1, 0.35),
        Stage::Connecting => (0.35, MENU_PART),
    };
    start + (end - start) * fraction.unwrap_or(0.0).clamp(0.0, 1.0)
}

/// How far a join to a server or Realm has got, within the menu's half.
fn join_progress(stage: JoinStage) -> f32 {
    match stage {
        JoinStage::Realm => 0.05,
        JoinStage::Connecting => 0.12,
        JoinStage::Packs {
            received_bytes,
            total_bytes,
            done,
            total,
        } => {
            let shared = if total_bytes > 0 {
                received_bytes as f32 / total_bytes as f32
            } else if total > 0 {
                done as f32 / total as f32
            } else {
                0.0
            };
            0.12 + (MENU_PART - 0.12 - 0.05) * shared.clamp(0.0, 1.0)
        }
        JoinStage::Generating => MENU_PART - 0.02,
    }
}

/// The bar's fill for an opening shown from the menu.
pub(crate) fn menu_progress(view: &MenuView) -> f32 {
    match &view.local.progress {
        Some(local) => local_progress(local.stage, local.fraction),
        None => join_progress(view.feeds.join.stage),
    }
}

/// The bar's fill for the world loading screen, given how much terrain has arrived.
fn world_progress(load: WorldLoad, terrain: Option<f32>) -> f32 {
    let terrain = terrain.unwrap_or(0.0).clamp(0.0, 1.0);
    match load {
        WorldLoad::Joining => MENU_PART,
        WorldLoad::Terrain => MENU_PART + (1.0 - MENU_PART) * terrain,
        WorldLoad::Dimension | WorldLoad::Other => terrain,
    }
}

/// What the tile shows.
enum Tile<'a> {
    /// The chunks round the player, as the world stream has them.
    Grid(&'a LoadingGrid),
    /// Nothing is known of the chunks yet.
    Empty,
}

/// The cells of `tile` on a square [`SIDE`] across, row by row, as 0 (not needed), 1 (waiting) or
/// 2 (arrived). A smaller grid sits in the middle of it. With no grid known the square shows the
/// round patch of chunks a view is made of, all waiting, so the tile looks the same before and
/// after the chunks are known.
fn cells(tile: &Tile<'_>) -> Vec<u8> {
    let mut square = vec![0_u8; SIDE * SIDE];
    match tile {
        Tile::Grid(grid)
            if grid.side > 0 && grid.side <= SIDE && grid.cells.len() == grid.side * grid.side =>
        {
            let offset = (SIDE - grid.side) / 2;
            for (place, state) in grid.cells.iter().enumerate() {
                let (row, column) = (place / grid.side + offset, place % grid.side + offset);
                square[row * SIDE + column] = *state;
            }
        }
        _ => {
            let middle = (SIDE as f32 - 1.0) / 2.0;
            for (place, state) in square.iter_mut().enumerate() {
                let (row, column) = ((place / SIDE) as f32, (place % SIDE) as f32);
                let distance = (row - middle).hypot(column - middle);
                *state = u8::from(distance <= middle + 0.5);
            }
        }
    }
    square
}

/// Draws the loading screen at `unit` logical pixels to a Java pixel over `size`, with the bar
/// `bar` of the way. `cancel` is the button that calls the join off, with the menu view it reads
/// its hover from.
#[allow(clippy::too_many_arguments, reason = "one screen's drawing state")]
fn paint(
    canvas: &mut Canvas<'_>,
    unit: f32,
    size: [f32; 2],
    title: &str,
    detail: &str,
    bar: f32,
    tile: &Tile<'_>,
    cancel: Option<(&MenuView, MenuAction)>,
) -> Result<(), PaintError> {
    let java_width = (size[0] / unit).ceil();
    let java_height = size[1] / unit;
    let centre = (java_width / 2.0).floor();
    canvas.fill([0.0, 0.0, size[0], size[1]], DIM)?;
    let region =
        canvas.begin_focus_region(focus::SCREEN, [0.0, 0.0, size[0], size[1]], None, true)?;
    canvas.focus_delegate(cancel.map(|(_, action)| action), None);
    // A line that only says the title again is left out.
    let mut lines = wrap_lines(detail, TEXT_WIDTH.min(java_width - 20.0));
    lines.retain(|line| !line.eq_ignore_ascii_case(title));
    let tile_size = TILE_INNER + 2.0 * (TILE_PAD + TILE_BORDER);
    let height = BAR_HEIGHT
        + BAR_GAP
        + LINE * (1.0 + lines.len() as f32)
        + TILE_GAP
        + tile_size
        + cancel.map_or(0.0, |_| CANCEL_HEIGHT + 12.0);
    let mut y = ((java_height - height) / 2.0).max(20.0);
    // The bar over the text: a dark track with the green share done.
    let left = (centre - BAR_WIDTH / 2.0) * unit;
    let track = [
        left,
        y * unit,
        left + BAR_WIDTH * unit,
        (y + BAR_HEIGHT) * unit,
    ];
    canvas.fill(track, BAR_TRACK)?;
    let filled = track[0] + (track[2] - track[0]) * bar.clamp(0.0, 1.0);
    canvas.fill([track[0], track[1], filled, track[3]], BAR_FILL)?;
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
    // The tile: a grey border round black, a cell for each chunk column round the player.
    y += TILE_GAP;
    let outer = [
        (centre - tile_size / 2.0) * unit,
        y * unit,
        (centre + tile_size / 2.0) * unit,
        (y + tile_size) * unit,
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
    let states = cells(tile);
    let (pitch, cell) = (CELL + CELL_GAP, CELL);
    let origin = [
        outer[0] + (TILE_BORDER + TILE_PAD) * unit,
        outer[1] + (TILE_BORDER + TILE_PAD) * unit,
    ];
    for (place, state) in states.iter().enumerate() {
        let colour = match state {
            2 => LOADED_CELL,
            1 => NEEDED_CELL,
            0 => continue,
            _ => EMPTY_CELL,
        };
        let x = origin[0] + (place % SIDE) as f32 * pitch * unit;
        let top = origin[1] + (place / SIDE) as f32 * pitch * unit;
        canvas.fill([x, top, x + cell * unit, top + cell * unit], colour)?;
    }
    y += tile_size;
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
    cancel: Option<MenuAction>,
) -> Result<(), PaintError> {
    let unit = unit(view, canvas, size);
    paint(
        canvas,
        unit,
        size,
        title,
        detail,
        menu_progress(view),
        &Tile::Empty,
        cancel.map(|action| (view, action)),
    )
}

/// Draws the loading screen while the world itself loads, with no menu to read: the player's Java
/// GUI scale (none for Auto) sets its size, `terrain` is how much of the terrain has arrived and
/// `grid` the chunks round the player. The terrain stages say what Java says.
#[allow(clippy::too_many_arguments, reason = "one screen's drawing state")]
pub(crate) fn draw_world(
    canvas: &mut Canvas<'_>,
    scale: Option<u8>,
    size: [f32; 2],
    load: WorldLoad,
    title: &str,
    detail: &str,
    terrain: Option<f32>,
    grid: Option<&LoadingGrid>,
) -> Result<(), PaintError> {
    let unit = unit_for(size, scale, canvas.metrics.dpi_scale.get());
    let (title, detail) = match load {
        WorldLoad::Terrain | WorldLoad::Dimension => ("Loading terrain...", ""),
        WorldLoad::Joining | WorldLoad::Other => (title, detail),
    };
    let tile = grid.map_or(Tile::Empty, Tile::Grid);
    paint(
        canvas,
        unit,
        size,
        title,
        detail,
        world_progress(load, terrain),
        &tile,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shouted_title_is_written_the_way_java_writes_one() {
        assert_eq!(title_case("STARTING WORLD"), "Starting World");
        assert_eq!(title_case("Joining"), "Joining");
        // Java's own sentence case is not changed.
        assert_eq!(title_case("Loading terrain..."), "Loading terrain...");
        assert_eq!(title_case(""), "");
    }

    #[test]
    fn one_bar_runs_through_the_whole_opening_and_never_goes_back() {
        // A local world starting, then joining, then its terrain loading.
        let mut last = -1.0_f32;
        let steps = [
            local_progress(Stage::StartingServer, None),
            local_progress(Stage::Connecting, None),
            world_progress(WorldLoad::Joining, None),
            world_progress(WorldLoad::Terrain, Some(0.0)),
            world_progress(WorldLoad::Terrain, Some(0.5)),
            world_progress(WorldLoad::Terrain, Some(1.0)),
        ];
        for step in steps {
            assert!(step >= last, "{step} follows {last}");
            assert!((0.0..=1.0).contains(&step));
            last = step;
        }
        assert_eq!(last, 1.0);
        // A server join through its packs stays inside the menu's half.
        let packs = join_progress(JoinStage::Packs {
            done: 1,
            total: 2,
            received_bytes: 50,
            total_bytes: 100,
        });
        assert!(
            join_progress(JoinStage::Connecting) < packs
                && packs < join_progress(JoinStage::Generating)
        );
        assert!(join_progress(JoinStage::Generating) <= MENU_PART);
        // The server being set up fills its own share of the start of the bar.
        assert!(
            local_progress(Stage::PullingImage, Some(1.0))
                <= local_progress(Stage::StartingServer, None)
        );
        // A dimension change fills the whole bar by itself.
        assert_eq!(world_progress(WorldLoad::Dimension, Some(0.5)), 0.5);
    }

    #[test]
    fn the_tile_shows_the_chunk_grid_it_is_given_or_a_waiting_square() {
        let grid = LoadingGrid {
            side: 3,
            cells: vec![0, 1, 2, 1, 2, 2, 0, 1, 2],
        };
        // A small grid sits in the middle of the square, whose size never changes.
        let square = cells(&Tile::Grid(&grid));
        assert_eq!(square.len(), SIDE * SIDE);
        let at = |row: usize, column: usize| square[(row + 4) * SIDE + column + 4];
        assert_eq!((at(0, 0), at(0, 1), at(0, 2)), (0, 1, 2));
        assert_eq!(at(2, 2), 2);
        // Round it nothing is needed.
        assert_eq!(square[0], 0);
        // With no grid the square shows the same round patch, all of it waiting.
        let waiting = cells(&Tile::Empty);
        assert_eq!(waiting.len(), SIDE * SIDE);
        assert!(waiting.iter().all(|cell| *cell <= 1));
        assert_eq!(waiting[SIDE * SIDE / 2], 1, "the middle is in the patch");
        assert_eq!(waiting[0], 0, "the corners are not");
        // The patch is as even left to right and top to bottom as it is the other way.
        for row in 0..SIDE {
            for column in 0..SIDE {
                assert_eq!(
                    waiting[row * SIDE + column],
                    waiting[(SIDE - 1 - row) * SIDE + (SIDE - 1 - column)]
                );
                assert_eq!(waiting[row * SIDE + column], waiting[column * SIDE + row]);
            }
        }
        // A grid whose cells do not fill its side is not trusted.
        let broken = LoadingGrid {
            side: 3,
            cells: vec![2; 4],
        };
        assert_eq!(cells(&Tile::Grid(&broken)), waiting);
    }
}
