//! Guardian spawns in Bedrock. Guardians are structure spawns there: 25 fixed columns in a
//! 5 × 5 grid, 2, 13, 29, 45 and 55 blocks in from the monument's corner with the smallest X
//! and Z. Each column looks for water from Y 61 down to Y 40, and a block at both Y 61 and
//! Y 59 switches it off.

/// Offsets of the spawn columns from the monument's corner, on both axes.
pub const OFFSETS: [i32; 5] = [2, 13, 29, 45, 55];
pub const TOP: i32 = 61;
pub const BOTTOM: i32 = 40;
/// Footprint of a monument.
pub const SIDE: i32 = 58;
/// The band of heights scanned for the monument's base.
pub const BASE_LOW: i32 = 37;
pub const BASE_HIGH: i32 = 44;
/// How far around the player the base is looked for.
pub const SEARCH_RADIUS: i32 = 48;
pub const TILE: i32 = 16;

/// The 25 column positions for a monument whose corner is `corner` (X, Z).
pub fn columns(corner: [i32; 2]) -> [[i32; 2]; 25] {
    std::array::from_fn(|index| [corner[0] + OFFSETS[index / 5], corner[1] + OFFSETS[index % 5]])
}

/// Bounds of the monument blocks found so far.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bounds {
    pub min: [i32; 2],
    pub max: [i32; 2],
    pub any: bool,
}

impl Bounds {
    pub fn add(&mut self, x: i32, z: i32) {
        if !self.any {
            *self = Self { min: [x, z], max: [x, z], any: true };
            return;
        }
        self.min = [self.min[0].min(x), self.min[1].min(z)];
        self.max = [self.max[0].max(x), self.max[1].max(z)];
    }

    /// The corner, once the blocks span exactly one monument on both axes.
    pub fn corner(&self) -> Option<[i32; 2]> {
        (self.any && self.max[0] - self.min[0] == SIDE - 1 && self.max[1] - self.min[1] == SIDE - 1)
            .then_some(self.min)
    }

    pub fn center(&self) -> [i32; 2] {
        [(self.min[0] + self.max[0]) / 2, (self.min[1] + self.max[1]) / 2]
    }
}

/// The scan tiles (X, Z minimums) covering `radius` around `center`, nearest first.
pub fn tiles(center: [i32; 2], radius: i32) -> Vec<[i32; 2]> {
    let low = [(center[0] - radius).div_euclid(TILE), (center[1] - radius).div_euclid(TILE)];
    let high = [(center[0] + radius).div_euclid(TILE), (center[1] + radius).div_euclid(TILE)];
    let mut tiles = Vec::new();
    for x in low[0]..=high[0] {
        for z in low[1]..=high[1] {
            tiles.push([x * TILE, z * TILE]);
        }
    }
    let middle = [center[0] - TILE / 2, center[1] - TILE / 2];
    tiles.sort_by_key(|tile| (tile[0] - middle[0]).pow(2) + (tile[1] - middle[1]).pow(2));
    tiles
}

/// Whether a column is switched off: something solid at both Y 61 and Y 59.
pub fn blocked(at_61_solid: bool, at_59_solid: bool) -> bool {
    at_61_solid && at_59_solid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_follow_the_wiki_grid() {
        let columns = columns([100, -200]);
        assert_eq!(columns[0], [102, -198]);
        assert_eq!(columns[4], [102, -145]);
        assert_eq!(columns[24], [155, -145]);
        assert_eq!(columns.len(), 25);
    }

    #[test]
    fn a_full_footprint_gives_its_corner() {
        let mut bounds = Bounds::default();
        bounds.add(10, 20);
        assert_eq!(bounds.corner(), None);
        bounds.add(10 + 57, 20 + 57);
        assert_eq!(bounds.corner(), Some([10, 20]));
        bounds.add(9, 20);
        assert_eq!(bounds.corner(), None, "too wide is not a monument");
    }

    #[test]
    fn tiles_cover_the_radius_nearest_first() {
        let tiles = tiles([5, 5], 48);
        assert_eq!(tiles.len(), 49);
        assert_eq!(tiles[0], [0, 0]);
        assert!(blocked(true, true) && !blocked(true, false));
    }
}
