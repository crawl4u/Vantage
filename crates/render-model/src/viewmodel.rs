//! Validated first-person meshes and their texture data.
use glam::{Mat4, Vec3};
use std::sync::Arc;
pub(crate) mod cube;
pub mod geometry;
pub const VIEWMODEL_TEXTURE_SIDE: u32 = 64;

/// The largest texture a held cube's faces are packed into.
pub const MAX_VIEWMODEL_TEXTURE_SIDE: u32 = 1024;

pub const VIEWMODEL_TEXTURE_BYTES: usize =
    (VIEWMODEL_TEXTURE_SIDE * VIEWMODEL_TEXTURE_SIDE * 4) as usize;

#[derive(Clone, Debug)]
pub struct ViewmodelSkin {
    pub rgba8: Arc<[u8]>,
    pub identity: [u8; 32],
    /// Texels across the square texture: the skin's 64, or a held cube's power of two.
    pub side: u32,
}

impl ViewmodelSkin {
    pub fn identity(&self) -> [u8; 32] {
        self.identity
    }
    pub fn new(rgba8: Arc<[u8]>, identity: [u8; 32]) -> Option<Self> {
        Self::with_side(rgba8, identity, VIEWMODEL_TEXTURE_SIDE)
    }

    /// A square texture `side` texels across, a power of two from the skin's 64 up to
    /// [`MAX_VIEWMODEL_TEXTURE_SIDE`], with fully opaque or fully transparent texels only.
    pub fn with_side(rgba8: Arc<[u8]>, identity: [u8; 32], side: u32) -> Option<Self> {
        (identity != [0; 32]
            && side.is_power_of_two()
            && (VIEWMODEL_TEXTURE_SIDE..=MAX_VIEWMODEL_TEXTURE_SIDE).contains(&side)
            && rgba8.len() == (side * side * 4) as usize
            && rgba8
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| matches!(pixel[3], 0 | 255)))
        .then_some(Self {
            rgba8,
            identity,
            side,
        })
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HandVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
}

const _: () = assert!(size_of::<HandVertex>() == 20);

#[derive(Clone, Debug)]
pub struct ViewmodelGeometry {
    pub vertices: Arc<[HandVertex]>,
    pub identity: [u8; 32],
    pub allowed_rigs: Arc<[u32]>,
    pub cube_origin: bool,
}

impl ViewmodelGeometry {
    pub fn from_runtime(
        assets: &assets::RuntimeEntityAssets,
        artwork: &crate::ActorArtworkPages,
    ) -> Option<Self> {
        let candidates = assets.geometry_candidates("geometry.humanoid.custom");
        let [geometry] = candidates else {
            return None;
        };
        let mut result = geometry::validated_geometry(geometry, artwork.entity_identity)?;
        result.allowed_rigs = assets
            .rig_geometries()
            .iter()
            .enumerate()
            .filter_map(|(index, binding)| {
                let candidate = assets.geometries().get(binding.geometry as usize)?;
                (candidate == geometry).then_some(index as u32)
            })
            .collect::<Vec<_>>()
            .into();
        (!result.allowed_rigs.is_empty()).then_some(result)
    }
    pub fn accepts_rig(&self, rig: u32) -> bool {
        self.allowed_rigs.contains(&rig)
    }
}
