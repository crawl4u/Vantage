//! Pack skybox presentation, separate from the required atmosphere carrier.

use render_scene::SkyboxSettings;

use assets::{SkyBlend, SkyRotation, SkyboxDimension};
use bevy::math::{Mat3, Vec3, Vec4};
use bevy::render::render_resource::ShaderType;
use bytemuck::{Pod, Zeroable};

use render_scene::{AtmosphereFrame, SkyKind};

/// Bedrock translates the cube before scaling its authored half-unit extent.
const NATIVE_SKYBOX_TRANSLATION: f32 = 256.0;
const NATIVE_SKYBOX_SCALE: f32 = 2000.0;
const SKYBOX_HALF_EXTENT: f32 = 0.5;

/// Translation in cube half-extents, face mask, visibility, and radius.
#[repr(C, align(16))]
#[derive(Clone, Copy, Pod, Zeroable, ShaderType)]
pub(crate) struct SkyboxUniform {
    pub(crate) parameters: Vec4,
    /// Whether the cube follows the time of day (x); the rest is reserved.
    pub(crate) time: Vec4,
    /// The rows of the matrix that turns a view ray into the layer's own frame.
    pub(crate) rotation: [Vec4; 3],
    /// A Java layer's opacity (x) and blend code (y, zero for a Bedrock cube).
    pub(crate) layer: Vec4,
}

impl SkyboxUniform {
    /// Builds the active dimension's transform without using the camera's position.
    pub(crate) fn new(frame: AtmosphereFrame, settings: SkyboxSettings, mask: u32) -> Self {
        let overworld = frame.sky_kind() == SkyKind::Overworld;
        let offset = if overworld && settings.native_vertical_offset {
            NATIVE_SKYBOX_TRANSLATION / (NATIVE_SKYBOX_SCALE * SKYBOX_HALF_EXTENT)
        } else {
            0.0
        };
        Self {
            parameters: Vec4::new(
                offset,
                mask as f32,
                u8::from(settings.visible) as f32,
                if overworld {
                    NATIVE_SKYBOX_SCALE * SKYBOX_HALF_EXTENT
                } else {
                    8.0
                },
            ),
            time: Vec4::ZERO,
            rotation: IDENTITY_ROWS,
            layer: Vec4::ZERO,
        }
    }

    /// A Java sky layer: `mask` names its source's faces, `alpha` is its opacity now and the
    /// layer turns by `rotation` (see [`rotation_rows`]).
    pub(crate) fn layer(
        frame: AtmosphereFrame,
        settings: SkyboxSettings,
        mask: u32,
        blend: SkyBlend,
        alpha: f32,
        rotation: [Vec4; 3],
    ) -> Self {
        Self {
            rotation,
            layer: Vec4::new(alpha.clamp(0.0, 1.0), f32::from(blend_code(blend)), 0.0, 0.0),
            ..Self::new(frame, settings, mask)
        }
    }

    /// Dims the cube with the time of day when `follow`.
    pub(crate) fn following_time(mut self, follow: bool) -> Self {
        self.time.x = f32::from(u8::from(follow));
        self
    }
}

const IDENTITY_ROWS: [Vec4; 3] = [
    Vec4::new(1.0, 0.0, 0.0, 0.0),
    Vec4::new(0.0, 1.0, 0.0, 0.0),
    Vec4::new(0.0, 0.0, 1.0, 0.0),
];

/// The code the shader and the pipeline's blend state share for a Java layer's blend.
pub(crate) const fn blend_code(blend: SkyBlend) -> u8 {
    match blend {
        SkyBlend::Alpha => 1,
        SkyBlend::Add => 2,
        SkyBlend::Replace => 3,
        SkyBlend::Multiply => 4,
        SkyBlend::Screen => 5,
        SkyBlend::Subtract => 6,
    }
}

/// The rows of the matrix turning a view ray into a layer's frame. A layer follows the sun: with
/// `rotation` it has turned by the celestial angle (in turns, zero at noon) times its speed about
/// its axis, as the sun does about +Z, so a ray looks up the layer where the layer was `angle` ago.
pub(crate) fn rotation_rows(rotation: Option<SkyRotation>, celestial_angle: f32) -> [Vec4; 3] {
    let Some(SkyRotation { speed, axis }) = rotation else {
        return IDENTITY_ROWS;
    };
    let axis = Vec3::from_array(axis);
    if !axis.is_finite() || axis.length_squared() < 1e-12 {
        return IDENTITY_ROWS;
    }
    let turned = Mat3::from_axis_angle(axis.normalize(), celestial_angle * std::f32::consts::TAU * speed);
    // The inverse of a rotation is its transpose, whose rows are the matrix's columns.
    let inverse = turned.transpose();
    [
        inverse.row(0).extend(0.0),
        inverse.row(1).extend(0.0),
        inverse.row(2).extend(0.0),
    ]
}

/// Selects the numbered texture namespace independently of pass admission.
pub(crate) fn dimension(frame: AtmosphereFrame) -> SkyboxDimension {
    match frame.sky_kind() {
        SkyKind::Overworld => SkyboxDimension::Overworld,
        SkyKind::Nether => SkyboxDimension::Nether,
        SkyKind::End => SkyboxDimension::End,
    }
}

#[cfg(test)]
mod rotation_tests {
    use super::*;

    fn apply(rows: [Vec4; 3], ray: Vec3) -> Vec3 {
        Vec3::new(rows[0].truncate().dot(ray), rows[1].truncate().dot(ray), rows[2].truncate().dot(ray))
    }

    #[test]
    fn a_layer_that_does_not_rotate_keeps_every_ray() {
        let ray = Vec3::new(0.3, 0.5, -0.8).normalize();
        assert!(apply(rotation_rows(None, 0.37), ray).abs_diff_eq(ray, 1e-6));
        assert!(apply(rotation_rows(Some(SkyRotation { speed: 1.0, axis: [0.0, 0.0, 1.0] }), 0.0), ray)
            .abs_diff_eq(ray, 1e-6));
    }

    #[test]
    fn a_layer_pinned_to_the_sun_is_looked_up_where_it_was() {
        // A quarter turn about +Z takes the zenith to the west, as it does the sun at dusk, so a
        // ray at the west horizon finds what the layer held at the zenith.
        let rows = rotation_rows(Some(SkyRotation { speed: 1.0, axis: [0.0, 0.0, 1.0] }), 0.25);
        assert!(apply(rows, Vec3::new(-1.0, 0.0, 0.0)).abs_diff_eq(Vec3::Y, 1e-5));
        assert!(apply(rows, Vec3::Y).abs_diff_eq(Vec3::X, 1e-5));
    }

    #[test]
    fn speed_scales_the_turn_and_a_zero_axis_is_ignored() {
        let slow = rotation_rows(Some(SkyRotation { speed: 0.5, axis: [0.0, 0.0, 1.0] }), 0.5);
        let fast = rotation_rows(Some(SkyRotation { speed: 1.0, axis: [0.0, 0.0, 1.0] }), 0.25);
        for (left, right) in slow.iter().zip(fast) {
            assert!(left.abs_diff_eq(right, 1e-5));
        }
        let broken = rotation_rows(Some(SkyRotation { speed: 1.0, axis: [0.0; 3] }), 0.3);
        assert_eq!(broken, IDENTITY_ROWS);
    }

    #[test]
    fn every_blend_has_its_own_nonzero_code() {
        let codes: std::collections::BTreeSet<u8> = [
            SkyBlend::Alpha,
            SkyBlend::Add,
            SkyBlend::Replace,
            SkyBlend::Multiply,
            SkyBlend::Screen,
            SkyBlend::Subtract,
        ]
        .into_iter()
        .map(blend_code)
        .collect();
        assert_eq!(codes.len(), 6);
        assert!(!codes.contains(&0));
    }
}

#[cfg(test)]
#[path = "pack_skybox/tests.rs"]
mod tests;
