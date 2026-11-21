//! The held cube drawn from the world's own texture pages, so it shows the same art, at the
//! same resolution, as the block placed in the world. A pack's grey grass and foliage are
//! coloured with the fixed default colour a hand or inventory slot uses.

use super::*;
use crate::{ViewmodelGeometry, ViewmodelSkin};
use assets::{BlockFace, BlockFlags, BlockVisualId, NetworkIdMode, RuntimeAssets, VisualKind};
use sha2::{Digest, Sha256};

/// Java's default grass colour, used when the pack names none.
pub const DEFAULT_GRASS: [u8; 3] = [0x7C, 0xBD, 0x6B];
const DEFAULT_FOLIAGE: [u8; 3] = [0x48, 0xB5, 0x18];
const BIRCH_FOLIAGE: [u8; 3] = [0x80, 0xA7, 0x55];
const EVERGREEN_FOLIAGE: [u8; 3] = [0x61, 0x99, 0x61];

/// Material flags a world-page held face may carry; any other flag asks for behaviour the hand
/// does not reproduce, so such a block keeps its baked picture.
const ALLOWED_FLAGS: u32 = assets::MATERIAL_FLAG_ISOTROPIC
    | assets::MATERIAL_FLAG_TINT_MASK
    | assets::MATERIAL_FLAG_OVERLAY_MASK
    | assets::MATERIAL_FLAG_ALPHA_CUTOUT
    | assets::MATERIAL_FLAG_FOLIAGE_CLASS_MASK;

/// The colour a hand or inventory slot gives a face with these material flags, or `None` when
/// the face is not tinted. `grass` is the pack's default grass colour.
pub fn held_tint(flags: u32, grass: [u8; 3]) -> Option<Option<[u8; 3]>> {
    match flags & assets::MATERIAL_FLAG_TINT_MASK {
        0 => Some(None),
        assets::MATERIAL_FLAG_GRASS_TINT => Some(Some(grass)),
        assets::MATERIAL_FLAG_FOLIAGE_TINT => Some(Some(
            match flags & assets::MATERIAL_FLAG_FOLIAGE_CLASS_MASK {
                assets::MATERIAL_FLAG_BIRCH_FOLIAGE => BIRCH_FOLIAGE,
                assets::MATERIAL_FLAG_EVERGREEN_FOLIAGE => EVERGREEN_FOLIAGE,
                _ => DEFAULT_FOLIAGE,
            },
        )),
        // Water has no held colour.
        _ => None,
    }
}

impl ViewmodelGeometry {
    /// Builds an ordinary cube whose faces reference the world's texture pages, with the
    /// placeholder skin the hand's identity checks expect. Returns `None` for anything the
    /// page-sampling hand cannot draw exactly: animated, unknown or non-cube faces.
    pub fn opaque_world_cube(
        assets: &RuntimeAssets,
        visual: BlockVisualId,
        grass: [u8; 3],
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
            || block.support() != assets::VisualSupport::Exact
            || block.animation().is_some()
        {
            return None;
        }
        let mut faces = [(0u32, 0u32, 0u32); 6];
        for (index, face) in BlockFace::ALL.into_iter().enumerate() {
            let id = block.face(face).material_id();
            if id == assets::DIAGNOSTIC_MATERIAL {
                return None;
            }
            let material = assets.materials().get(id as usize)?;
            if material.flags & !ALLOWED_FLAGS != 0
                || material.animation != assets::NO_ANIMATION
                || material.flags & assets::MATERIAL_FLAG_UV_MASK != 0
            {
                return None;
            }
            let page = material.texture.page();
            let source = assets.texture_pages().get(page as usize)?;
            if material.texture.layer() >= source.texture.layers {
                return None;
            }
            let mut tint = 0;
            if let Some(colour) = held_tint(material.flags, grass)? {
                tint = u32::from_le_bytes([colour[0], colour[1], colour[2], 0]) | WORLD_HAND_TINTED;
                if material.flags & assets::MATERIAL_FLAG_OVERLAY_MASK != 0 {
                    tint |= WORLD_HAND_OVERLAY_MASK;
                }
            }
            faces[index] = (page, material.texture.layer(), tint);
        }
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
        let transform = cube::opaque_cube_pose();
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
            let (page, layer, tint) = faces[face];
            for corner in [0, 1, 2, 0, 2, 3] {
                let p = transform
                    .transform_point3(Vec3::from_array(positions[corner]) - Vec3::splat(0.5));
                vertices.push(WorldHandVertex {
                    position: p.to_array(),
                    uv: uv[corner],
                    page,
                    layer,
                    tint,
                });
            }
        }
        let carrier = std::ptr::from_ref(assets) as usize;
        let mut digest = Sha256::new();
        digest.update(b"opaque-world-cube-v1");
        digest.update(assets.provenance().source_manifest_sha256);
        digest.update(assets.provenance().block_registry_sha256);
        digest.update(visual.0.to_le_bytes());
        digest.update(carrier.to_le_bytes());
        digest.update(bytemuck::cast_slice(&vertices));
        let geometry = Self {
            vertices: Arc::from([]),
            identity: digest.finalize().into(),
            allowed_rigs: Arc::from([]),
            cube_origin: true,
            world: Some(WorldCube {
                vertices: vertices.into(),
                carrier,
            }),
        };
        // The pixels live on the GPU; the skin only carries the identity the gates compare.
        let placeholder: Arc<[u8]> = vec![0; VIEWMODEL_TEXTURE_BYTES].into();
        let skin = ViewmodelSkin::new(placeholder, Sha256::digest(geometry.identity).into())?;
        Some((geometry, skin))
    }
}
