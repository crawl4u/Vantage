//! Bounded six-face geometry transport. Pose, lighting and action parity remain unavailable.

use super::*;
use crate::{ViewmodelGeometry, ViewmodelSkin};
use assets::{
    BlockFace, BlockFlags, BlockVisualId, NetworkIdMode, RuntimeAssets, VisualKind, VisualSupport,
};
use sha2::Digest;
use sha2::Sha256;

const TILE_COLUMNS: usize = assets::BLOCK_ITEM_SHEET_GRID[0] as usize;
/// The finest face a held cube draws; finer pack art is filtered down to it.
const MAX_FACE_SIDE: usize = 128;

#[cfg(test)]
#[path = "cube_tests.rs"]
pub(crate) mod tests;

/// The existing held-cube transport pose, shared by cube and validated cube-model geometry.
pub(super) fn opaque_cube_pose() -> Mat4 {
    Mat4::from_translation(Vec3::new(0.56, -0.52, -0.72))
        * Mat4::from_rotation_y(45_f32.to_radians())
        * Mat4::from_scale(Vec3::splat(0.4))
}

impl ViewmodelGeometry {
    /// Builds an ordinary opaque cube from the current validated block carrier.
    /// The existing nearest-only skin transport contains six unmodified tiles.
    pub fn opaque_cube(
        assets: &RuntimeAssets,
        visual: BlockVisualId,
    ) -> Option<(Self, ViewmodelSkin)> {
        if !assets.provenance().is_complete() || visual.0 as usize >= assets.visual_count() {
            return None;
        }
        let block = assets.resolve(NetworkIdMode::Sequential, visual.0);
        let model = assets::gui_item::full_cube_model_quads(assets, visual);
        let ordinary_cube = block.kind() == VisualKind::Cube
            && block
                .flags()
                .contains(BlockFlags::CUBE_GEOMETRY | BlockFlags::OCCLUDES_FULL_FACE)
            && block.model_template().is_none();
        if !block.is_known()
            || (!ordinary_cube && model.is_none())
            || block.support() != VisualSupport::Exact
            || block.animation().is_some()
        {
            return None;
        }
        // Admit the whole source before allocating or copying any pixels.
        let mut sources = [(&[][..], 0usize); 6];
        let mut materials = [0; 6];
        let mut finest = assets::BLOCK_ITEM_FACE_SIDE as usize;
        for (index, face) in BlockFace::ALL.into_iter().enumerate() {
            let id = block.face(face).material_id();
            if id == assets::DIAGNOSTIC_MATERIAL {
                return None;
            }
            let material = assets.materials().get(id as usize)?;
            // World-position rotation leaves carried face pixels and UVs unchanged.
            if material.flags & !assets::MATERIAL_FLAG_ISOTROPIC != 0
                || material.animation != assets::NO_ANIMATION
            {
                return None;
            }
            let page = assets
                .texture_pages()
                .get(material.texture.page() as usize)?;
            let first = page.texture.mips.first()?;
            if !first.size.is_power_of_two() || material.texture.layer() >= page.texture.layers {
                return None;
            }
            finest = finest.max(first.size as usize);
            materials[index] = id;
        }
        // Every face is the finest face's size, within the cap, taken from the mip of that size
        // so a finer source is filtered rather than skipped.
        let side_of_face = finest.min(MAX_FACE_SIDE);
        for (index, id) in materials.into_iter().enumerate() {
            let material = assets.materials().get(id as usize)?;
            let page = assets
                .texture_pages()
                .get(material.texture.page() as usize)?;
            let first = page.texture.mips.first()?.size as usize;
            let level = first
                .ilog2()
                .saturating_sub(side_of_face.ilog2())
                .min(page.texture.mips.len() as u32 - 1);
            let mip = page.texture.mips.get(level as usize)?;
            let size = mip.size as usize;
            let bytes = size.checked_mul(size)?.checked_mul(4)?;
            let start = (material.texture.layer() as usize).checked_mul(bytes)?;
            let tile = mip.rgba8.get(start..start.checked_add(bytes)?)?;
            if !tile.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 255) {
                return None;
            }
            sources[index] = (tile, size);
        }
        let tile_side = side_of_face;
        let tile_pitch = tile_side + 2;
        let side = (TILE_COLUMNS * tile_pitch)
            .next_power_of_two()
            .max(VIEWMODEL_TEXTURE_SIDE as usize);
        let mut pixels = vec![0; side * side * 4];
        for (index, (tile, size)) in sources.into_iter().enumerate() {
            let [x, y] = [
                (index % TILE_COLUMNS) * tile_pitch,
                (index / TILE_COLUMNS) * tile_pitch,
            ];
            for row in 0..tile_pitch {
                for column in 0..tile_pitch {
                    // A one-texel border repeats the edge so sampling never bleeds a neighbour.
                    let (face_y, face_x) = (
                        row.saturating_sub(1).min(tile_side - 1) * size / tile_side,
                        column.saturating_sub(1).min(tile_side - 1) * size / tile_side,
                    );
                    let source = (face_y * size + face_x) * 4;
                    let target = ((y + row) * side + x + column) * 4;
                    pixels[target..target + 4].copy_from_slice(&tile[source..source + 4]);
                }
            }
        }
        // Same face corners and UV directions as the existing world cube pipeline.
        let corners = [
            [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]],
            [[1., 0., 0.], [1., 1., 0.], [1., 1., 1.], [1., 0., 1.]],
            [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
            [[0., 1., 0.], [0., 1., 1.], [1., 1., 1.], [1., 1., 0.]],
            [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]],
            [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]],
        ];
        let horizontal = [[0., 0.], [1., 0.], [1., 1.], [0., 1.]];
        let transposed = [[0., 0.], [0., 1.], [1., 1.], [1., 0.]];
        let vertical = [[0., 1.], [1., 1.], [1., 0.], [0., 0.]];
        let vertical_transposed = [[0., 1.], [0., 0.], [1., 0.], [1., 1.]];
        let transform = opaque_cube_pose();
        let mut vertices = Vec::with_capacity(36);
        for (face, positions) in corners.into_iter().enumerate() {
            let uv = if let Some(quads) = model {
                quads[face]
                    .uvs
                    .map(|uv| uv.map(|coordinate| f32::from(coordinate) / 4096.0))
            } else {
                match face {
                    0 | 5 => vertical,
                    1 | 4 => vertical_transposed,
                    3 => transposed,
                    _ => horizontal,
                }
            };
            for corner in [0, 1, 2, 0, 2, 3] {
                let p = transform
                    .transform_point3(Vec3::from_array(positions[corner]) - Vec3::splat(0.5));
                vertices.push(HandVertex {
                    position: p.to_array(),
                    uv: [
                        ((face % TILE_COLUMNS * tile_pitch + 1) as f32
                            + uv[corner][0] * tile_side as f32)
                            / side as f32,
                        ((face / TILE_COLUMNS * tile_pitch + 1) as f32
                            + uv[corner][1] * tile_side as f32)
                            / side as f32,
                    ],
                });
            }
        }
        let pixel_identity: [u8; 32] = Sha256::digest(&pixels).into();
        let mut digest = Sha256::new();
        digest.update(b"opaque-cube-static-v1");
        digest.update(assets.provenance().source_manifest_sha256);
        digest.update(assets.provenance().block_registry_sha256);
        digest.update(visual.0.to_le_bytes());
        for material in materials {
            digest.update(material.to_le_bytes());
        }
        digest.update(pixel_identity);
        digest.update(bytemuck::cast_slice(&vertices));
        let geometry = Self {
            vertices: vertices.into(),
            identity: digest.finalize().into(),
            allowed_rigs: Arc::from([]),
            cube_origin: true,
            world: None,
        };
        let skin = ViewmodelSkin::with_side(pixels.into(), pixel_identity, side as u32)?;
        Some((geometry, skin))
    }
}
