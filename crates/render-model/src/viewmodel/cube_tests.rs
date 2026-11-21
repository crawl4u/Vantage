use super::*;
use assets::{BlockVisualId, CompiledAssets, RegistryRecord};
use std::fs;

/// Compiles real barrel selectors with original opaque sprite artwork for held-consumer checks.
pub(crate) fn barrels() -> (CompiledAssets, Vec<RegistryRecord>) {
    barrels_with_size(assets::TILE_SIZE)
}

/// Compiles original sprites at the requested source resolution.
pub(crate) fn barrels_with_size(side: u32) -> (CompiledAssets, Vec<RegistryRecord>) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::create_dir_all(root.join("biomes")).unwrap();
    fs::create_dir_all(root.join("behavior/biomes")).unwrap();
    fs::create_dir_all(root.join("textures/colormap")).unwrap();
    fs::write(
        root.join("biomes/plain.client_biome.json"),
        r#"{"minecraft:client_biome":{"description":{"identifier":"plain"},"components":{}}}"#,
    )
    .unwrap();
    fs::write(root.join("behavior/biomes/plain.biome.json"),r#"{"minecraft:biome":{"description":{"identifier":"plain"},"components":{"minecraft:climate":{"temperature":0.8,"downfall":0.4}}}}"#).unwrap();
    fs::write(root.join("blocks.json"), r#"{"barrel":{"textures":"art"}}"#).unwrap();
    fs::write(
        root.join("textures/terrain_texture.json"),
        r#"{"texture_data":{"art":{"textures":"textures/art"}}}"#,
    )
    .unwrap();
    fs::write(root.join("textures/flipbook_textures.json"), "[]").unwrap();
    image::RgbaImage::from_fn(side, side, |x, y| {
        image::Rgba([20, (x * 192 / side) as u8, (y * 192 / side) as u8, 255])
    })
    .save(root.join("textures/art.png"))
    .unwrap();
    for map in assets::TintMapId::ALL {
        image::RgbaImage::from_pixel(
            assets::TINT_MAP_SIZE,
            assets::TINT_MAP_SIZE,
            image::Rgba([255; 4]),
        )
        .save(root.join(format!("textures/colormap/{}.png", map.source_name())))
        .unwrap();
    }
    let protocol = assets::active_content_registry_protocol();
    let records =
        assets::read_registry_for_protocol(assets::pinned_block_registry_bytes(), protocol)
            .unwrap()
            .into_vec()
            .into_iter()
            .filter(|r| matches!(r.name.as_ref(), "minecraft:air" | "minecraft:barrel"))
            .collect::<Vec<_>>();
    let span = records
        .iter()
        .map(|r| r.sequential_id as usize + 1)
        .max()
        .unwrap();
    let compiled = pack_compiler::compile_pack_with_biomes(
        root,
        &root.join("behavior"),
        &records,
        &[],
        &vec![assets::LightProperties::default(); span],
        protocol,
    )
    .unwrap();
    (compiled, records)
}

#[test]
fn opaque_barrel_models_remain_available_in_first_person() {
    let (compiled, records) = barrels();
    let runtime = RuntimeAssets::decode(&assets::encode_blob(&compiled).unwrap()).unwrap();
    let mut count = 0;
    for record in records
        .iter()
        .filter(|r| r.name.as_ref() == "minecraft:barrel")
    {
        let (geometry, skin) =
            ViewmodelGeometry::opaque_cube(&runtime, BlockVisualId(record.sequential_id))
                .expect("opaque full-cube barrel models retain held geometry");
        assert_eq!(geometry.vertices.len(), 36);
        assert!(geometry.cube_origin);
        assert!(
            skin.rgba8
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] == 255)
        );
        let visual = runtime.resolve(assets::NetworkIdMode::Sequential, record.sequential_id);
        let template = runtime.model_templates()[visual.model_template().unwrap() as usize];
        let quads =
            &runtime.model_quads()[template.quad_start as usize..][..template.quad_count as usize];
        let tile_side = usize::from(assets::BLOCK_ITEM_FACE_SIDE);
        let tile_pitch = tile_side + 2;
        for vertex in geometry.vertices.iter() {
            let pixel = vertex.uv.map(|coordinate| coordinate * skin.side as f32);
            let slot = pixel.map(|coordinate| coordinate as usize / tile_pitch);
            let face = slot[1] * TILE_COLUMNS + slot[0];
            let uv = std::array::from_fn::<_, 2, _>(|axis| {
                ((pixel[axis] - (slot[axis] * tile_pitch + 1) as f32) / tile_side as f32 * 4096.0)
                    .round() as u16
            });
            let position = (opaque_cube_pose()
                .inverse()
                .transform_point3(Vec3::from_array(vertex.position))
                + Vec3::splat(0.5))
            .to_array()
            .map(|component| (component * 256.0).round() as i16);
            let corner = quads[face]
                .positions
                .iter()
                .position(|&world| world == position)
                .expect("held vertex remains on its world cube face");
            assert_eq!(
                uv, quads[face].uvs[corner],
                "held UVs preserve the sprite at the same geometric corner"
            );
        }
        count += 1;
    }
    assert_eq!(count, 12);
}

#[test]
fn held_barrel_cube_admission_rejects_inset_or_folded_models() {
    let (compiled, records) = barrels();
    let record = records
        .iter()
        .find(|r| r.name.as_ref() == "minecraft:barrel")
        .unwrap();
    let id = assets::BlockVisualId(record.sequential_id);
    let template =
        compiled.model_templates[compiled.visuals[id.0 as usize].model_template as usize];
    for folded in [false, true] {
        let mut invalid = compiled.clone();
        let quad = &mut invalid.model_quads[template.quad_start as usize];
        if folded {
            quad.uvs[1] = quad.uvs[0];
        } else {
            for point in &mut quad.positions {
                point[0] = 16;
            }
        }
        let runtime = RuntimeAssets::decode(&assets::encode_blob(&invalid).unwrap()).unwrap();
        assert!(ViewmodelGeometry::opaque_cube(&runtime, id).is_none());
    }
}

/// A held cube packs faces of whatever resolution its art has, up to the cap, into a texture
/// sized to hold them, and its UVs address the same texels.
#[test]
fn a_held_cube_texture_grows_with_the_resolution_of_its_faces() {
    for source in [16, 32, 64] {
        let (compiled, records) = barrels_with_size(source);
        let runtime = RuntimeAssets::decode(&assets::encode_blob(&compiled).unwrap()).unwrap();
        let record = records
            .iter()
            .find(|r| r.name.as_ref() == "minecraft:barrel")
            .unwrap();
        let visual = BlockVisualId(record.sequential_id);
        let block = runtime.resolve(assets::NetworkIdMode::Sequential, visual.0);
        let material = runtime
            .materials()
            .get(block.face(BlockFace::Up).material_id() as usize)
            .unwrap();
        let face = runtime.texture_pages()[material.texture.page() as usize]
            .texture
            .mips[0]
            .size as usize;
        let (geometry, skin) = ViewmodelGeometry::opaque_cube(&runtime, visual)
            .unwrap_or_else(|| panic!("a {source}-texel barrel is held"));
        let face = face.clamp(16, 128);
        let expected = (TILE_COLUMNS * (face + 2))
            .next_power_of_two()
            .max(VIEWMODEL_TEXTURE_SIDE as usize);
        assert_eq!(skin.side as usize, expected, "source {source}");
        assert_eq!(skin.rgba8.len(), expected * expected * 4);
        assert!(
            geometry
                .vertices
                .iter()
                .all(|vertex| vertex.uv.iter().all(|uv| (0.0..=1.0).contains(uv)))
        );
    }
}

/// A cube drawn from world pages names each face's own page and layer at any resolution, keeps
/// the geometry of the baked cube, and copies no pixels.
#[test]
fn a_world_cube_names_the_pages_and_layers_of_its_faces() {
    for source in [16, 64] {
        let (compiled, records) = barrels_with_size(source);
        let runtime = RuntimeAssets::decode(&assets::encode_blob(&compiled).unwrap()).unwrap();
        let record = records
            .iter()
            .find(|r| r.name.as_ref() == "minecraft:barrel")
            .unwrap();
        let visual = BlockVisualId(record.sequential_id);
        let (baked, _) = ViewmodelGeometry::opaque_cube(&runtime, visual).unwrap();
        let (cube, skin) =
            ViewmodelGeometry::opaque_world_cube(&runtime, visual, world_cube::DEFAULT_GRASS)
                .expect("a plain cube is drawn from world pages");
        let world = cube.world.as_ref().unwrap();
        assert!(cube.vertices.is_empty() && cube.cube_origin);
        assert_eq!(world.carrier, std::ptr::from_ref(&runtime) as usize);
        assert_eq!(world.vertices.len(), baked.vertices.len());
        assert_ne!(skin.identity, [0; 32]);
        let block = runtime.resolve(assets::NetworkIdMode::Sequential, visual.0);
        for (face, quad) in BlockFace::ALL.into_iter().zip(world.vertices.chunks(6)) {
            let material = runtime
                .materials()
                .get(block.face(face).material_id() as usize)
                .unwrap();
            for (vertex, baked) in quad.iter().zip(&baked.vertices[face as usize * 6..]) {
                assert_eq!(vertex.page, material.texture.page());
                assert_eq!(vertex.layer, material.texture.layer());
                assert_eq!(vertex.position, baked.position);
                assert!(vertex.uv.iter().all(|uv| (0.0..=1.0).contains(uv)));
                assert_eq!(vertex.tint, 0);
            }
        }
    }
}

/// Every flag that tints a face gives its default hand colour; water, which has none, is refused.
#[test]
fn held_tints_follow_the_material_flags() {
    use world_cube::{DEFAULT_GRASS, held_tint};
    let grass = [1, 2, 3];
    assert_eq!(held_tint(0, grass), Some(None));
    assert_eq!(
        held_tint(assets::MATERIAL_FLAG_GRASS_TINT, grass),
        Some(Some(grass))
    );
    let foliage = |class| held_tint(assets::MATERIAL_FLAG_FOLIAGE_TINT | class, DEFAULT_GRASS);
    let colours = [
        foliage(0),
        foliage(assets::MATERIAL_FLAG_BIRCH_FOLIAGE),
        foliage(assets::MATERIAL_FLAG_EVERGREEN_FOLIAGE),
    ];
    assert!(colours.iter().all(|c| c.is_some_and(|c| c.is_some())));
    assert_eq!(
        colours
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        3,
        "each foliage class has its own colour"
    );
    assert_eq!(held_tint(assets::MATERIAL_FLAG_WATER_TINT, grass), None);
}
