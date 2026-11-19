use std::cell::OnceCell;

use assets::{
    BLOCK_VISUAL_VARIANT_COVERED_GRASS, BLOCK_VISUAL_VARIANT_MATERIAL_MASK,
    BLOCK_VISUAL_VARIANT_NONSEASONAL_LEAF, BLOCK_VISUAL_VARIANT_SEASONAL_LEAF,
    BLOCK_VISUAL_VARIANT_SNOW_COVER, BLOCK_VISUAL_VARIANT_TOP_SNOW, SEASONAL_LEAF_DEEP_OFFSET,
    SEASONAL_LEAF_EXPOSED_OFFSET,
};

use super::models::{PaletteResolutionContext, adjacent_palette_entry};
use crate::{
    Face,
    contributors::{PaletteFacts, ResolvedPaletteEntry},
};

pub(crate) struct CubeMaterialResolver<'refs, 'assets, 'chunks> {
    pub(crate) context: PaletteResolutionContext<'assets, 'chunks>,
    pub(crate) facts: &'refs PaletteFacts<'chunks>,
    pub(crate) neighbour_facts: &'refs [OnceCell<PaletteFacts<'chunks>>; Face::ALL.len()],
    pub(crate) seasonal_coverage: Option<&'refs super::seasonal_foliage::SeasonalCoverage>,
    pub(crate) leaves: &'refs super::leaves::LeafOcclusion<'assets, 'chunks>,
    pub(crate) connected: &'refs super::connected::ConnectedSampler<'refs, 'assets, 'chunks>,
}

impl CubeMaterialResolver<'_, '_, '_> {
    /// The connected-texture tile of a face of `material` when that material is a tile set the
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
            .then(|| self.connected.tile(coordinate, face, material))
    }

    /// Vanilla grass samples only the block directly above:
    /// every TopSnow height, Snow, or PowderSnow selects its snowy
    /// side. The carried face table and the top/bottom materials stay unchanged.
    pub(crate) fn face_material(
        &self,
        coordinate: [usize; 3],
        entry: ResolvedPaletteEntry,
        face: Face,
    ) -> u32 {
        if entry.variant
            & (BLOCK_VISUAL_VARIANT_SEASONAL_LEAF | BLOCK_VISUAL_VARIANT_NONSEASONAL_LEAF)
            != 0
        {
            let exposed = entry.variant & BLOCK_VISUAL_VARIANT_SEASONAL_LEAF != 0
                && self
                    .seasonal_coverage
                    .is_some_and(|coverage| coverage.exposed(coordinate));
            return (entry.variant & BLOCK_VISUAL_VARIANT_MATERIAL_MASK)
                + face.index() as u32
                + if exposed {
                    SEASONAL_LEAF_EXPOSED_OFFSET
                } else {
                    0
                }
                + if self.leaves.is_deep(coordinate.map(|value| value as i32)) {
                    SEASONAL_LEAF_DEEP_OFFSET
                } else {
                    0
                };
        }
        if entry.variant & BLOCK_VISUAL_VARIANT_COVERED_GRASS != 0
            && !matches!(face, Face::NegativeY | Face::PositiveY)
        {
            let above = adjacent_palette_entry(
                self.context,
                self.facts,
                self.neighbour_facts,
                coordinate,
                Face::PositiveY,
            );
            if above.variant & (BLOCK_VISUAL_VARIANT_TOP_SNOW | BLOCK_VISUAL_VARIANT_SNOW_COVER)
                != 0
            {
                return entry.variant & BLOCK_VISUAL_VARIANT_MATERIAL_MASK;
            }
        }
        entry.faces[face.index()]
    }
}
