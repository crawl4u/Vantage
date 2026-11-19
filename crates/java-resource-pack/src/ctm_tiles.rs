//! Expands the tile sets of OptiFine's smaller connected-texture methods to the full 47-tile
//! `ctm` layout the client picks from, so every method draws the same way.

use image::{GenericImage, GenericImageView, RgbaImage};
use pack_json::connected_layout::{
    DOT_DOWN_LEFT, DOT_DOWN_RIGHT, DOT_UP_LEFT, DOT_UP_RIGHT, DOWN, LAYOUT, LEFT, RIGHT, UP,
};

use crate::ctm::Connected;

/// Compact tile numbers: the unconnected outline, the plain interior, left and right borders
/// only (a vertical run), top and bottom borders only (a horizontal run), and the four corner
/// dots.
const ISOLATED: usize = 0;
const PLAIN: usize = 1;
const VERTICAL_RUN: usize = 2;
const HORIZONTAL_RUN: usize = 3;
const DOTTED: usize = 4;

/// The compact tile a corner quarter is cut from, given whether the neighbour along the
/// quarter's vertical edge, along its horizontal edge, and the diagonal one continue the face.
const fn compact_source(vertical: bool, horizontal: bool, dotted: bool) -> usize {
    match (vertical, horizontal) {
        (false, false) => ISOLATED,
        (true, false) => VERTICAL_RUN,
        (false, true) => HORIZONTAL_RUN,
        (true, true) if dotted => DOTTED,
        (true, true) => PLAIN,
    }
}

/// The 47 tiles of `tiles`, which follow `layout`; `None` when `tiles` lists too few or differ in
/// size where they must match.
pub(crate) fn expand(layout: Connected, tiles: &[RgbaImage]) -> Option<Vec<RgbaImage>> {
    if tiles.len() < layout.tile_count() {
        return None;
    }
    let (width, height) = tiles[0].dimensions();
    if tiles[..layout.tile_count()]
        .iter()
        .any(|tile| tile.dimensions() != (width, height))
    {
        return None;
    }
    LAYOUT
        .iter()
        .enumerate()
        .map(|(index, &(edges, dots))| {
            let has = |bit: u8| edges & bit != 0;
            match layout {
                Connected::Full => Some(tiles[index].clone()),
                Connected::Horizontal => Some(
                    tiles[match (has(LEFT), has(RIGHT)) {
                        (true, true) => 1,
                        (false, true) => 0,
                        (true, false) => 2,
                        (false, false) => 3,
                    }]
                    .clone(),
                ),
                Connected::Vertical => Some(
                    tiles[match (has(UP), has(DOWN)) {
                        (true, true) => 1,
                        (true, false) => 0,
                        (false, true) => 2,
                        (false, false) => 3,
                    }]
                    .clone(),
                ),
                Connected::Compact => {
                    let (half_width, half_height) = (width / 2, height / 2);
                    let mut out = RgbaImage::new(width, height);
                    for (x, y, vertical, horizontal, dot) in [
                        (0, 0, UP, LEFT, DOT_UP_LEFT),
                        (width - half_width, 0, UP, RIGHT, DOT_UP_RIGHT),
                        (0, height - half_height, DOWN, LEFT, DOT_DOWN_LEFT),
                        (
                            width - half_width,
                            height - half_height,
                            DOWN,
                            RIGHT,
                            DOT_DOWN_RIGHT,
                        ),
                    ] {
                        let source =
                            compact_source(has(vertical), has(horizontal), dots & dot != 0);
                        let quarter = tiles[source].view(x, y, half_width, half_height);
                        out.copy_from(&*quarter, x, y).ok()?;
                    }
                    Some(out)
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn solid(tone: u8) -> RgbaImage {
        RgbaImage::from_pixel(4, 4, Rgba([tone, 0, 0, 255]))
    }

    fn tile_with_edges(edges: u8) -> usize {
        LAYOUT
            .iter()
            .position(|&(e, d)| e == edges && d == 0)
            .unwrap()
    }

    #[test]
    fn horizontal_runs_pick_by_left_and_right_neighbours() {
        let tiles: Vec<_> = (0..4).map(solid).collect();
        let full = expand(Connected::Horizontal, &tiles).unwrap();
        let tone = |index: usize| full[index].get_pixel(0, 0)[0];
        assert_eq!(tone(tile_with_edges(0)), 3);
        assert_eq!(tone(tile_with_edges(RIGHT)), 0);
        assert_eq!(tone(tile_with_edges(LEFT | RIGHT)), 1);
        assert_eq!(tone(tile_with_edges(LEFT)), 2);
        // Up and down neighbours change nothing.
        assert_eq!(tone(tile_with_edges(UP | LEFT | RIGHT)), 1);
    }

    #[test]
    fn vertical_runs_pick_by_top_and_bottom_neighbours() {
        let tiles: Vec<_> = (0..4).map(solid).collect();
        let full = expand(Connected::Vertical, &tiles).unwrap();
        let tone = |index: usize| full[index].get_pixel(0, 0)[0];
        assert_eq!(tone(tile_with_edges(UP)), 0);
        assert_eq!(tone(tile_with_edges(UP | DOWN)), 1);
        assert_eq!(tone(tile_with_edges(DOWN)), 2);
        assert_eq!(tone(tile_with_edges(0)), 3);
    }

    #[test]
    fn compact_tiles_fill_each_quarter_by_its_own_neighbours() {
        let tiles: Vec<_> = (0..5).map(|tone| solid(tone * 10)).collect();
        let full = expand(Connected::Compact, &tiles).unwrap();
        let quarters = |index: usize| {
            let tile = &full[index];
            [(0, 0), (3, 0), (0, 3), (3, 3)].map(|(x, y)| tile.get_pixel(x, y)[0])
        };
        // Isolated: every quarter is the outline tile. Fully surrounded: every quarter plain.
        assert_eq!(quarters(0), [0; 4]);
        assert_eq!(quarters(26), [10; 4]);
        // Surrounded with every diagonal missing: dotted corners.
        assert_eq!(quarters(46), [40; 4]);
        // A neighbour only below: both lower quarters continue vertically, the upper do not.
        assert_eq!(quarters(tile_with_edges(DOWN)), [0, 0, 20, 20]);
        // A neighbour only to the right: right quarters continue horizontally.
        assert_eq!(quarters(tile_with_edges(RIGHT)), [0, 30, 0, 30]);
    }

    #[test]
    fn a_short_or_mismatched_set_is_refused() {
        let tiles: Vec<_> = (0..4).map(solid).collect();
        assert!(expand(Connected::Compact, &tiles).is_none());
        let mut mixed: Vec<_> = (0..4).map(solid).collect();
        mixed[2] = RgbaImage::new(2, 2);
        assert!(expand(Connected::Horizontal, &mixed).is_none());
    }
}
