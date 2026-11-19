//! The 47-tile layout of OptiFine's `ctm` connected-texture method, shared by the pack translation
//! that fills it and the mesher that picks from it.

/// Edge bits of a tile configuration: neighbours continuing the face upwards, downwards, to
/// the left and to the right, as a viewer in front of the face sees them.
pub const UP: u8 = 1;
pub const DOWN: u8 = 2;
pub const LEFT: u8 = 4;
pub const RIGHT: u8 = 8;
/// Corner bits: a corner whose two edges both continue but whose diagonal does not, which the
/// tile marks with a dot in that corner. The diagonal neighbours use the same bits.
pub const DOT_UP_LEFT: u8 = 1;
pub const DOT_UP_RIGHT: u8 = 2;
pub const DOT_DOWN_LEFT: u8 = 4;
pub const DOT_DOWN_RIGHT: u8 = 8;

/// The `(edges, corner dots)` each tile of the `ctm` layout shows, in tile order.
pub const LAYOUT: [(u8, u8); 47] = {
    const U: u8 = UP;
    const D: u8 = DOWN;
    const L: u8 = LEFT;
    const R: u8 = RIGHT;
    const A: u8 = U | D | L | R;
    const TL: u8 = DOT_UP_LEFT;
    const TR: u8 = DOT_UP_RIGHT;
    const BL: u8 = DOT_DOWN_LEFT;
    const BR: u8 = DOT_DOWN_RIGHT;
    [
        (0, 0),
        (R, 0),
        (L | R, 0),
        (L, 0),
        (D | R, BR),
        (D | L, BL),
        (U | D | R, TR | BR),
        (D | L | R, BL | BR),
        (A, TL | BL | BR),
        (A, TL | TR | BL),
        (A, TR | BR),
        (A, BL | BR),
        (D, 0),
        (D | R, 0),
        (D | L | R, 0),
        (D | L, 0),
        (U | R, TR),
        (U | L, TL),
        (U | L | R, TL | TR),
        (U | D | L, TL | BL),
        (A, TR | BL | BR),
        (A, TL | TR | BR),
        (A, TL | TR),
        (A, TL | BL),
        (U | D, 0),
        (U | D | R, 0),
        (A, 0),
        (U | D | L, 0),
        (U | D | R, TR),
        (D | L | R, BR),
        (U | D | R, BR),
        (D | L | R, BL),
        (A, BR),
        (A, BL),
        (A, TL | BR),
        (A, TR | BL),
        (U, 0),
        (U | R, 0),
        (U | L | R, 0),
        (U | L, 0),
        (U | L | R, TL),
        (U | D | L, BL),
        (U | L | R, TR),
        (U | D | L, TL),
        (A, TR),
        (A, TL),
        (A, TL | TR | BL | BR),
    ]
};

/// Tile index by `edges | dots << 4`; `u8::MAX` marks a configuration no tile shows.
const TILE_OF: [u8; 256] = {
    let mut table = [u8::MAX; 256];
    let mut tile = 0;
    while tile < LAYOUT.len() {
        let (edges, dots) = LAYOUT[tile];
        let slot = (edges | dots << 4) as usize;
        assert!(table[slot] == u8::MAX, "two tiles share a configuration");
        table[slot] = tile as u8;
        tile += 1;
    }
    table
};

/// The tile for a face whose edge neighbours `edges` and diagonal neighbours `diagonals`
/// continue it. A diagonal only matters beside both of its edges.
pub fn tile(edges: u8, diagonals: u8) -> u8 {
    let mut dots = 0;
    for (corner, a, b) in [
        (DOT_UP_LEFT, UP, LEFT),
        (DOT_UP_RIGHT, UP, RIGHT),
        (DOT_DOWN_LEFT, DOWN, LEFT),
        (DOT_DOWN_RIGHT, DOWN, RIGHT),
    ] {
        if edges & a != 0 && edges & b != 0 && diagonals & corner == 0 {
            dots |= corner;
        }
    }
    TILE_OF[(edges | dots << 4) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_neighbour_pattern_has_a_tile() {
        for edges in 0..16_u8 {
            for diagonals in 0..16_u8 {
                assert!(usize::from(tile(edges, diagonals)) < LAYOUT.len());
            }
        }
    }

    #[test]
    fn the_layout_matches_its_named_tiles() {
        let all = UP | DOWN | LEFT | RIGHT;
        assert_eq!(tile(0, 0), 0);
        assert_eq!(tile(all, 0b1111), 26);
        assert_eq!(tile(all, 0), 46);
        assert_eq!(tile(RIGHT, 0b1111), 1);
        assert_eq!(tile(DOWN | RIGHT, DOT_UP_LEFT), 4);
        assert_eq!(tile(UP, 0), 36);
    }

    #[test]
    fn a_diagonal_only_counts_beside_both_its_edges() {
        for diagonals in 0..16_u8 {
            assert_eq!(tile(RIGHT, diagonals), 1);
        }
    }
}
