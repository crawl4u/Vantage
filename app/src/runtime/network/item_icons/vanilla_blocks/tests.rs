use super::*;
use crate::runtime::network::block_overlay::{CarriedBlock, OverlayGaps};
use assets::{
    BlockOverlay, BlockVisual, Material, ModelQuad, ModelTemplate, NO_ANIMATION, TextureRef,
};

/// Original cube and half-height model fixtures share the same texture contract.
fn block(model: bool) -> RuntimeAssets {
    let diagnostic = RuntimeAssets::diagnostic();
    let quads = assets::BlockFace::ALL.map(|face| {
        let (corners, uvs) = assets::gui_item::cube_face(face);
        ModelQuad {
            positions: corners.map(|point| {
                [
                    (point[0] * 256.0) as i16,
                    (point[1] * 128.0) as i16,
                    (point[2] * 256.0) as i16,
                ]
            }),
            uvs: uvs.map(|uv| uv.map(|value| (value * 4096.0) as u16)),
            material: 0,
            flags: face.model_quad_face_id(),
        }
    });
    let overlay = BlockOverlay {
        visuals: vec![BlockVisual {
            faces: [0; 6],
            flags: if model {
                assets::BlockFlags::empty()
            } else {
                assets::BlockFlags::CUBE_GEOMETRY
            },
            kind: if model {
                VisualKind::Model
            } else {
                VisualKind::Cube
            },
            support: assets::VisualSupport::Exact,
            contributor_role: assets::ContributorRole::Primary,
            model_template: if model { 0 } else { assets::NO_MODEL_TEMPLATE },
            animation: NO_ANIMATION,
            variant: 0,
            native_material: assets::native_material::NativeMaterialFacts::Unresolved,
            destroy: assets::compiled::block_destroy::BlockDestroyFacts::Unresolved,
        }],
        light_properties: vec![assets::LightProperties::OPAQUE_DARK],
        materials: vec![Material {
            texture: TextureRef::new(1, 0).unwrap(),
            ..Material::unvaried()
        }],
        model_templates: if model {
            vec![ModelTemplate {
                quad_start: 0,
                quad_count: quads.len() as u32,
                flags: 0,
            }]
        } else {
            vec![]
        },
        model_quads: if model { quads.into() } else { vec![] },
        textures: vec![diagnostic.texture_array().clone()],
        ..Default::default()
    };
    diagnostic.with_block_overlay(1, &overlay).unwrap()
}

/// Only the optional carried art changes; the slab's geometry comes from the base.
fn textures() -> CompiledBlockOverlay {
    let mut texture = RuntimeAssets::diagnostic().texture_array().clone();
    for mip in &mut texture.mips {
        mip.rgba8 = [80, 150, 220, 255]
            .repeat((mip.size * mip.size) as usize)
            .into();
    }
    CompiledBlockOverlay {
        overlay: BlockOverlay {
            materials: vec![
                Material::unvaried(),
                Material {
                    texture: TextureRef::new(1, 0).unwrap(),
                    ..Material::unvaried()
                },
            ],
            textures: vec![texture],
            ..Default::default()
        },
        gaps: OverlayGaps::default(),
        vanilla_carried: vec![CarriedBlock {
            visual: 1,
            faces: [1; 6],
        }],
    }
}

#[test]
fn model_routes_never_publish_rebound_inventory_or_held_art() {
    let base = block(true);
    let mut output = BlockIcons::default();
    let route = Route {
        identifier: "minecraft:fixture_slab".into(),
        metadata: 7,
        visual: 1,
    };
    append_routes(&base, &textures(), &mut output, &[route], None);
    assert!(output.icons.is_empty());
    assert!(output.block_sheets.is_empty());
}

#[test]
fn plain_cube_routes_publish_both_thumbnail_and_held_sheet() {
    let mut output = BlockIcons::default();
    let route = Route {
        identifier: "minecraft:fixture_cube".into(),
        metadata: 7,
        visual: 1,
    };
    append_routes(&block(false), &textures(), &mut output, &[route], None);
    assert_eq!(output.icons.len(), 1);
    assert_eq!(output.block_sheets.len(), 1);
    assert_eq!(output.icons[0].metadata, 7);
    assert_eq!(output.block_sheets[0].metadata, 7);
}

#[test]
fn hd_carried_sheets_survive_session_publication_beside_a_reduced_thumbnail() {
    let mut compiled = textures();
    let side = 256;
    let color = [80, 150, 220, 255];
    compiled.overlay.textures.first_mut().unwrap().mips =
        assets::build_legacy_terrain_mip_chain(&color.repeat((side * side) as usize), side)
            .unwrap()
            .into();
    let mut output = BlockIcons::default();
    let route = Route {
        identifier: "minecraft:fixture_cube".into(),
        metadata: 7,
        visual: 1,
    };
    append_routes(&block(false), &compiled, &mut output, &[route], None);
    // A tile finer than the thumbnail cap draws its thumbnail from the largest mip within it.
    assert_eq!(output.icons.len(), 1);
    assert!(output.misses.is_empty());
    assert_eq!(output.block_sheets.len(), 1);
    let expected = output.block_sheets[0].rgba8.clone();
    let icons = super::super::compile_session_icons(
        &super::super::tests::view(),
        &[("minecraft:fixture_cube".into(), "test:gem".into())],
        output,
    )
    .unwrap();
    assert_eq!(icons.icons.len(), 1);
    assert!(icons.misses.is_empty());
    assert_eq!(icons.block_sheets.len(), 1);
    assert_eq!(icons.block_sheets[0].metadata, 7);
    assert_eq!(icons.block_sheets[0].rgba8, expected);
    assert_eq!(icons.block_material_flags["minecraft:fixture_cube"], 0);
}

#[test]
fn full_thumbnail_capacity_does_not_discard_available_carried_sheets() {
    let mut output = BlockIcons {
        icons: (0..MAX_SESSION_ICONS)
            .map(|_| SessionIcon {
                identifier: "test:other".into(),
                metadata: 0,
                width: 1,
                height: 1,
                rgba8: vec![255; 4].into(),
            })
            .collect(),
        ..Default::default()
    };
    let route = Route {
        identifier: "minecraft:fixture_cube".into(),
        metadata: 7,
        visual: 1,
    };
    append_routes(&block(false), &textures(), &mut output, &[route], None);
    assert_eq!(output.icons.len(), MAX_SESSION_ICONS);
    assert_eq!(output.block_sheets.len(), 1);
    assert_eq!(output.block_sheets[0].metadata, 7);
}

/// A carried cube whose faces are masked grass-side art: half the texels are dirt (alpha 0) and
/// half grey overlay (alpha 255), as a pack's grass side is stored.
fn masked_carried() -> CompiledBlockOverlay {
    let mut compiled = textures();
    let side = 16usize;
    let texels = (0..side * side)
        .flat_map(|index| {
            if index < side * side / 2 {
                [200u8, 200, 200, 255]
            } else {
                [100, 60, 20, 0]
            }
        })
        .collect::<Vec<_>>();
    compiled.overlay.textures.first_mut().unwrap().mips =
        assets::build_legacy_terrain_mip_chain(&texels, side as u32)
            .unwrap()
            .into();
    compiled.overlay.materials[1].flags =
        assets::MATERIAL_FLAG_GRASS_TINT | assets::MATERIAL_FLAG_OVERLAY_MASK;
    compiled
}

#[test]
fn masked_tinted_carried_faces_take_the_default_grass_colour_in_thumbnail_and_sheet() {
    let route = || Route {
        identifier: "minecraft:fixture_cube".into(),
        metadata: 0,
        visual: 1,
    };
    let mut output = BlockIcons::default();
    append_routes(
        &block(false),
        &masked_carried(),
        &mut output,
        &[route()],
        Some([100, 200, 50]),
    );
    assert_eq!(output.icons.len(), 1, "the thumbnail draws");
    assert_eq!(output.block_sheets.len(), 1, "the held sheet draws");
    let sheet = &output.block_sheets[0];
    // The first face's top row is overlay: grey 200 tinted by the colour; no texel is
    // transparent, and the dirt keeps its own colour.
    let texel = |x: usize, y: usize| {
        let offset = (y * sheet.width as usize + x) * 4;
        <[u8; 4]>::try_from(&sheet.rgba8[offset..offset + 4]).unwrap()
    };
    assert_eq!(texel(2, 2), [78, 156, 39, 255]);
    assert_eq!(texel(2, 14), [100, 60, 20, 255]);

    let mut untinted = BlockIcons::default();
    append_routes(
        &block(false),
        &masked_carried(),
        &mut untinted,
        &[route()],
        None,
    );
    assert!(
        untinted.icons.is_empty() && untinted.block_sheets.is_empty(),
        "a tint with no colour leaves the startup art in place"
    );
}
