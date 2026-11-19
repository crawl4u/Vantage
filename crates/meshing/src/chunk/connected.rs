//! Neighbour-aware connected textures: which of the 47 tiles of OptiFine's `ctm` layout a block
//! face shows, given which blocks around it continue the same material.

use std::cell::OnceCell;

use assets::BlockFlags;

use super::models::PaletteResolutionContext;
use crate::{
    Face, SIDE,
    contributors::{PaletteFacts, ResolvedPaletteEntry},
};

use pack_json::connected_layout::{
    DOT_DOWN_LEFT, DOT_DOWN_RIGHT, DOT_UP_LEFT, DOT_UP_RIGHT, DOWN, LEFT, RIGHT, UP, tile,
};

/// The block-grid direction a viewer in front of `face` sees as right and as down, matching the
/// texture orientation the chunk shader draws.
const fn screen_axes(face: Face) -> ([i32; 3], [i32; 3]) {
    match face {
        Face::NegativeX => ([0, 0, 1], [0, -1, 0]),
        Face::PositiveX => ([0, 0, -1], [0, -1, 0]),
        Face::NegativeY | Face::PositiveY => ([1, 0, 0], [0, 0, 1]),
        Face::NegativeZ => ([-1, 0, 0], [0, -1, 0]),
        Face::PositiveZ => ([1, 0, 0], [0, -1, 0]),
    }
}

const fn normal(face: Face) -> [i32; 3] {
    match face {
        Face::NegativeX => [-1, 0, 0],
        Face::PositiveX => [1, 0, 0],
        Face::NegativeY => [0, -1, 0],
        Face::PositiveY => [0, 1, 0],
        Face::NegativeZ => [0, 0, -1],
        Face::PositiveZ => [0, 0, 1],
    }
}

/// Reads blocks anywhere in the three-by-three-by-three sections around the one being meshed.
pub(crate) struct ConnectedSampler<'refs, 'assets, 'chunks> {
    context: PaletteResolutionContext<'assets, 'chunks>,
    center: &'refs PaletteFacts<'chunks>,
    sections: [OnceCell<Option<PaletteFacts<'chunks>>>; 27],
}

impl<'refs, 'assets, 'chunks> ConnectedSampler<'refs, 'assets, 'chunks> {
    pub(crate) fn new(
        context: PaletteResolutionContext<'assets, 'chunks>,
        center: &'refs PaletteFacts<'chunks>,
    ) -> Self {
        Self {
            context,
            center,
            sections: std::array::from_fn(|_| OnceCell::new()),
        }
    }

    /// The block at `coordinate`, in blocks relative to the meshed section's origin; open space
    /// outside the loaded snapshot.
    fn entry(&self, coordinate: [i32; 3]) -> ResolvedPaletteEntry {
        let side = SIDE as i32;
        let offset = coordinate.map(|value| value.div_euclid(side));
        let local = coordinate.map(|value| value.rem_euclid(side) as usize);
        if offset == [0, 0, 0] {
            return self.center.at(local[0], local[1], local[2]);
        }
        if offset.iter().any(|value| !(-1..=1).contains(value)) {
            return ResolvedPaletteEntry::AIR;
        }
        let slot = ((offset[0] + 1) * 9 + (offset[1] + 1) * 3 + offset[2] + 1) as usize;
        self.sections[slot]
            .get_or_init(|| {
                self.context
                    .neighbourhood
                    .sub_chunk(offset.map(|value| value as i8))
                    .map(|sub_chunk| {
                        PaletteFacts::new(
                            self.context.classifier,
                            self.context.visuals,
                            self.context.network_id_mode,
                            sub_chunk,
                        )
                    })
            })
            .as_ref()
            .map_or(ResolvedPaletteEntry::AIR, |facts| {
                facts.at(local[0], local[1], local[2])
            })
    }

    /// Whether the block at `coordinate` continues `material` on `face`: it shows the same
    /// material there and nothing opaque covers that face.
    fn continues(&self, coordinate: [i32; 3], face: Face, material: u32) -> bool {
        if self.entry(coordinate).faces[face.index()] != material {
            return false;
        }
        let front = normal(face);
        let front = [0, 1, 2].map(|axis| coordinate[axis] + front[axis]);
        !self
            .entry(front)
            .flags
            .contains(BlockFlags::OCCLUDES_FULL_FACE)
    }

    /// The tile of a face of `material` when that material is a connected-texture set the
    /// neighbouring blocks choose from, else `None`.
    pub(crate) fn connection(
        &self,
        materials: &[assets::Material],
        coordinate: [usize; 3],
        face: Face,
        material: u32,
    ) -> Option<u8> {
        let selector = materials.get(material as usize)?;
        (selector.variation_count == u32::from(assets::CONNECTED_TILE_COUNT)
            && selector.mask_strength >> 16 == assets::CONNECTED_TAG)
            .then(|| self.tile(coordinate, face, material))
    }

    /// The `ctm` tile index of `face` of the block at `coordinate`, whose face shows `material`.
    pub(crate) fn tile(&self, coordinate: [usize; 3], face: Face, material: u32) -> u8 {
        let origin = coordinate.map(|value| value as i32);
        let (right, down) = screen_axes(face);
        let at = |dx: i32, dy: i32| {
            [0, 1, 2].map(|axis| origin[axis] + dx * right[axis] + dy * down[axis])
        };
        let mut edges = 0;
        for (bit, dx, dy) in [(UP, 0, -1), (DOWN, 0, 1), (LEFT, -1, 0), (RIGHT, 1, 0)] {
            if self.continues(at(dx, dy), face, material) {
                edges |= bit;
            }
        }
        let mut diagonals = 0;
        for (bit, dx, dy) in [
            (DOT_UP_LEFT, -1, -1),
            (DOT_UP_RIGHT, 1, -1),
            (DOT_DOWN_LEFT, -1, 1),
            (DOT_DOWN_RIGHT, 1, 1),
        ] {
            if self.continues(at(dx, dy), face, material) {
                diagonals |= bit;
            }
        }
        tile(edges, diagonals)
    }
}
