use serde_json::json;

use super::vanilla_terrain::{base, compile, keys};
use super::*;

/// A pack carrying one Java model bound to every state of the registry's first block.
fn model_pack(block: &str) -> LayeredPackView {
    let model = java_resource_pack::java_model::Model {
        textures: [("side".to_owned(), "block/state".to_owned())].into(),
        elements: vec![java_resource_pack::java_model::Element {
            from: [4.0, 0.0, 4.0],
            to: [12.0, 8.0, 12.0],
            rotation: None,
            faces: [(
                "north",
                java_resource_pack::java_model::Face {
                    texture: "side".to_owned(),
                    uv: None,
                    tint: false,
                    rotation: 0,
                },
            )]
            .into(),
        }],
    };
    let geometry = java_resource_pack::java_model::to_geometry("geometry.java.stub", &model);
    let bindings = json!({ "format_version": 1, "blocks": [{
        "block": block, "properties": {}, "geometry": "geometry.java.stub",
        "rotation": [0, 0, 0], "materials": { "side": "state" } }] });
    let terrain = json!({"texture_data":{"state":{"textures":"textures/base_a"}}});
    pack_zip(vec![
        (
            "textures/terrain_texture.json".to_owned(),
            terrain.to_string().into_bytes(),
        ),
        (
            "textures/base_a.png".to_owned(),
            png(16, 16, |_, _| [9, 90, 200, 255]),
        ),
        (
            "models/blocks/java/stub.geo.json".to_owned(),
            geometry.to_string().into_bytes(),
        ),
        (
            java_resource_pack::block_models::BINDINGS.to_owned(),
            bindings.to_string().into_bytes(),
        ),
    ])
}

/// A resource pack holding `files`.
fn pack_zip(files: Vec<(String, Vec<u8>)>) -> LayeredPackView {
    let id = "00000000-0000-0000-0000-000000000001";
    let manifest = json!({"format_version":2,"header":{"uuid":id,"version":[1,0,0]},
        "modules":[{"type":"resources"}]});
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (path, bytes) in std::iter::once((
        "manifest.json".to_owned(),
        manifest.to_string().into_bytes(),
    ))
    .chain(files)
    {
        writer
            .start_file(path, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&bytes).unwrap();
    }
    LayeredPackView::new(resource_pack::validate_handoff(
        protocol::ResourcePackHandoff::from_archives(vec![
            protocol::ResourcePackArchive::unencrypted(
                id.parse().unwrap(),
                "1.0.0".into(),
                String::new(),
                writer.finish().unwrap().into_inner(),
            ),
        ]),
    ))
}

#[test]
fn a_bound_java_model_redraws_the_states_that_map_to_its_java_block_and_no_other() {
    let records = assets::read_registry_for_protocol(
        assets::pinned_block_registry_bytes(),
        assets::active_content_registry_protocol(),
    )
    .unwrap();
    let java = |index: usize| {
        assets::facts::java_mapping::java_block_state(records[index].sequential_id)
            .map(|state| state.name().to_owned())
    };
    let named = java(0).expect("the first block maps to a Java block");
    assert!(
        java(1).is_none_or(|other| other != named),
        "the fixture needs a second state of another Java block"
    );
    let base = base();
    let overlay = compile(&model_pack(&named), &base, &keys());
    let runtime = base.with_block_overlay(2, &overlay.overlay).unwrap();
    let drawn = runtime.resolve(NetworkIdMode::Sequential, 0);
    assert_eq!(drawn.kind(), VisualKind::Model);
    assert!(drawn.model_template().is_some());
    assert_eq!(
        runtime.resolve(NetworkIdMode::Sequential, 1).kind(),
        VisualKind::Cube
    );
}

/// A base whose only visual is a pane or fence of vanilla's template group: a pane has sixteen
/// mask templates of six quads, a fence a post and sixteen arm templates of eight quads an arm.
fn connected_base(family: u32) -> RuntimeAssets {
    connected_base_blending(family, false)
}

/// [`connected_base`] whose faces blend with what is behind them when `blend` is set.
fn connected_base_blending(family: u32, blend: bool) -> RuntimeAssets {
    let counts: Vec<u32> = if family == assets::MODEL_TEMPLATE_FLAG_PANE {
        vec![6; 16]
    } else {
        std::iter::once(6)
            .chain((0..16u32).map(|mask| mask.count_ones() * 8))
            .collect()
    };
    let mut start = 0;
    let templates = counts
        .iter()
        .map(|&quad_count| {
            let template = assets::ModelTemplate {
                quad_start: start,
                quad_count,
                flags: family,
            };
            start += quad_count;
            template
        })
        .collect();
    base_with_shape(0, templates, blend)
}

/// A base of `index` plain cubes then a model drawing the templates from the first, whose
/// quads are blank.
fn base_with_shape(
    index: usize,
    templates: Vec<assets::ModelTemplate>,
    blend: bool,
) -> RuntimeAssets {
    let diagnostic = RuntimeAssets::diagnostic();
    let mut cube = super::super::diagnostic_visual();
    cube.kind = VisualKind::Cube;
    cube.support = VisualSupport::Exact;
    cube.flags = assets::BlockFlags::CUBE_GEOMETRY | assets::BlockFlags::OCCLUDES_FULL_FACE;
    cube.faces = [1; 6];
    let mut shape = super::super::diagnostic_visual();
    shape.kind = VisualKind::Model;
    shape.support = VisualSupport::Exact;
    shape.model_template = 0;
    shape.faces = [1; 6];
    let mut visuals = vec![cube; index];
    visuals.push(shape);
    let quad = assets::ModelQuad {
        positions: [[0; 3]; 4],
        uvs: [[0; 2]; 4],
        material: 1,
        flags: 0,
    };
    let quads: u32 = templates.iter().map(|template| template.quad_count).sum();
    let compiled = assets::CompiledAssets {
        light_properties: vec![assets::LightProperties::OPAQUE_DARK; visuals.len()].into(),
        visuals: visuals.into(),
        hashed: Box::new([]),
        materials: (0..3)
            .map(|index| {
                let mut material = assets::Material::unvaried();
                if blend && index != 0 {
                    material.flags |= assets::MATERIAL_FLAG_ALPHA_BLEND;
                }
                material
            })
            .collect::<Vec<_>>()
            .into(),
        model_templates: templates.into(),
        model_quads: vec![quad; quads as usize].into(),
        animations: Box::new([]),
        animation_frames: Box::new([]),
        texture_pages: diagnostic.texture_pages().to_vec().into(),
        biomes: assets::CompiledBiomeAssets::diagnostic(),
        provenance: assets::BlobProvenance {
            source_manifest_sha256: [1; 32],
            block_registry_sha256: [2; 32],
            light_registry_sha256: [3; 32],
            biome_registry_sha256: [4; 32],
        },
    };
    RuntimeAssets::decode(&assets::encode_blob(&compiled).unwrap()).unwrap()
}

/// Binds a connected block of `family` named by the first registry block and returns the
/// overlay, after checking the runtime accepts it.
fn bound_connected(family: u32) -> super::super::CompiledBlockOverlay {
    let records = assets::read_registry_for_protocol(
        assets::pinned_block_registry_bytes(),
        assets::active_content_registry_protocol(),
    )
    .unwrap();
    let named = assets::facts::java_mapping::java_block_state(records[0].sequential_id)
        .expect("the first block maps to a Java block")
        .name()
        .to_owned();
    // The post is one box; each connection adds one more.
    let box_geometry = |name: &str, index: usize| {
        let element = java_resource_pack::java_model::Element {
            from: [index as f64, 0.0, 7.0],
            to: [index as f64 + 1.0, 16.0, 9.0],
            rotation: None,
            faces: [(
                "north",
                java_resource_pack::java_model::Face {
                    texture: "state".to_owned(),
                    uv: None,
                    tint: false,
                    rotation: 0,
                },
            )]
            .into(),
        };
        let model = java_resource_pack::java_model::Model {
            textures: [("state".to_owned(), "block/state".to_owned())].into(),
            elements: vec![element],
        };
        let document = java_resource_pack::java_model::to_geometry(name, &model);
        (
            format!(
                "models/blocks/java/{}.geo.json",
                name.trim_start_matches("geometry.java.")
            ),
            document.to_string().into_bytes(),
        )
    };
    let mut files = vec![box_geometry("geometry.java.pane_p0", 0)];
    let mut parts = vec![json!({ "geometry": "geometry.java.pane_p0" })];
    for (index, direction) in ["north", "east", "south", "west"].into_iter().enumerate() {
        let name = format!("geometry.java.pane_p{}", index + 1);
        files.push(box_geometry(&name, index + 1));
        parts.push(json!({ "geometry": name, "when": { direction: "true" } }));
    }
    let bindings = json!({ "format_version": 1, "blocks": [{
        "block": named,
        "parts": parts,
        "materials": { "state": "state" } }] });
    files.push((
        java_resource_pack::block_models::BINDINGS.to_owned(),
        bindings.to_string().into_bytes(),
    ));
    files.push((
        "textures/terrain_texture.json".to_owned(),
        json!({"texture_data":{"state":{"textures":"textures/base_a"}}})
            .to_string()
            .into_bytes(),
    ));
    files.push((
        "textures/base_a.png".to_owned(),
        png(16, 16, |_, _| [9, 90, 200, 255]),
    ));
    let base = connected_base(family);
    let overlay = compile(&pack_zip(files), &base, &keys());
    assert_eq!(overlay.overlay.template_overrides, vec![(0, 0)]);
    // The overlay is valid: a group of its own quad counts passes the runtime checks.
    base.with_block_overlay(1, &overlay.overlay).unwrap();
    overlay
}

#[test]
fn a_bound_java_pane_redraws_its_template_group_with_a_shape_for_each_mask() {
    let family = assets::MODEL_TEMPLATE_FLAG_PANE;
    let templates = bound_connected(family).overlay.model_templates;
    assert_eq!(templates.len(), 16);
    for (mask, template) in templates.iter().enumerate() {
        assert_eq!(template.flags, family);
        assert_eq!(
            template.quad_count,
            1 + (mask as u32).count_ones(),
            "mask {mask}"
        );
    }
}

#[test]
fn a_bound_java_fence_redraws_its_post_and_one_arm_template_for_each_mask() {
    let family = assets::MODEL_TEMPLATE_FLAG_FENCE_WOOD;
    let templates = bound_connected(family).overlay.model_templates;
    assert_eq!(templates.len(), 17);
    assert!(templates.iter().all(|template| template.flags == family));
    assert_eq!(templates[0].quad_count, 1, "post");
    for mask in 0..16u32 {
        let arms = &templates[1 + mask as usize];
        assert_eq!(arms.quad_count, mask.count_ones(), "mask {mask}");
    }
}

#[test]
fn a_redrawn_block_blends_where_the_vanilla_block_does() {
    let family = assets::MODEL_TEMPLATE_FLAG_PANE;
    let method = |blend: bool| {
        let base = connected_base_blending(family, blend);
        super::super::java_models::render_method(&base, base.resolve(NetworkIdMode::Sequential, 0))
    };
    assert_eq!(method(true), protocol_ids::BlockRenderMethod::Blend);
    assert_eq!(method(false), protocol_ids::BlockRenderMethod::AlphaTest);
}

/// The first registry state whose Java state has a property, with the Java block and that
/// property.
fn first_state_with_a_property() -> (usize, String, (String, String)) {
    let records = assets::read_registry_for_protocol(
        assets::pinned_block_registry_bytes(),
        assets::active_content_registry_protocol(),
    )
    .unwrap();
    records
        .iter()
        .enumerate()
        .find_map(|(index, record)| {
            let java = assets::facts::java_mapping::java_block_state(record.sequential_id)?;
            let (name, value) = java.properties().next()?;
            Some((
                index,
                java.name().to_owned(),
                (name.to_owned(), value.to_owned()),
            ))
        })
        .expect("some Bedrock state maps to a Java state with properties")
}

/// One box of `tall` pixels reaching up from the floor, as a geometry file.
fn tall_box(name: &str, tall: f64) -> (String, Vec<u8>) {
    let element = java_resource_pack::java_model::Element {
        from: [4.0, 0.0, 4.0],
        to: [12.0, tall, 12.0],
        rotation: None,
        faces: ["north", "south", "east", "west"]
            .into_iter()
            .map(|face| {
                (
                    face,
                    java_resource_pack::java_model::Face {
                        texture: "state".to_owned(),
                        uv: None,
                        tint: false,
                        rotation: 0,
                    },
                )
            })
            .collect(),
    };
    let model = java_resource_pack::java_model::Model {
        textures: [("state".to_owned(), "block/state".to_owned())].into(),
        elements: vec![element],
    };
    (
        format!(
            "models/blocks/java/{}.geo.json",
            name.trim_start_matches("geometry.java.")
        ),
        java_resource_pack::java_model::to_geometry(name, &model)
            .to_string()
            .into_bytes(),
    )
}

/// A pack binding `blocks` with `geometries` and one flat texture.
fn pack_with_bindings(geometries: Vec<(String, Vec<u8>)>, blocks: serde_json::Value) -> LayeredPackView {
    let mut files = geometries;
    files.push((
        java_resource_pack::block_models::BINDINGS.to_owned(),
        json!({ "format_version": 1, "blocks": blocks })
            .to_string()
            .into_bytes(),
    ));
    files.push((
        "textures/terrain_texture.json".to_owned(),
        json!({"texture_data":{"state":{"textures":"textures/base_a"}}})
            .to_string()
            .into_bytes(),
    ));
    files.push((
        "textures/base_a.png".to_owned(),
        png(16, 16, |_, _| [9, 90, 200, 255]),
    ));
    pack_zip(files)
}

#[test]
fn a_bound_java_wall_state_draws_the_parts_that_hold_for_its_java_state() {
    let (index, named, (property, value)) = first_state_with_a_property();
    // A post, an arm for this state's property value and an arm for a value it never has.
    let files = vec![
        tall_box("geometry.java.wall_p0", 16.0),
        tall_box("geometry.java.wall_p1", 14.0),
        tall_box("geometry.java.wall_p2", 12.0),
    ];
    let blocks = json!([{ "block": named, "materials": { "state": "state" }, "parts": [
        { "geometry": "geometry.java.wall_p0" },
        { "geometry": "geometry.java.wall_p1", "when": { property.clone(): value } },
        { "geometry": "geometry.java.wall_p2", "when": { property: "no such value" } }] }]);
    let wall = assets::MODEL_TEMPLATE_FLAG_WALL;
    let base = base_with_shape(
        index,
        vec![assets::ModelTemplate {
            quad_start: 0,
            quad_count: 6,
            flags: wall,
        }],
        false,
    );
    let overlay = compile(&pack_with_bindings(files, blocks), &base, &keys());
    let templates = &overlay.overlay.model_templates;
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].flags, wall);
    // Four side faces of the post and of the arm that holds.
    assert_eq!(templates[0].quad_count, 8);
    assert_eq!(overlay.overlay.template_overrides, vec![(index as u32, 0)]);
    base.with_block_overlay(index as u32 + 1, &overlay.overlay)
        .unwrap();
}

#[test]
fn a_bound_java_gate_model_redraws_the_gates_chain_of_parts_keeping_its_axis() {
    let (index, named, _) = first_state_with_a_property();
    let files = vec![tall_box("geometry.java.gate", 16.0)];
    let blocks = json!([{ "block": named, "properties": {}, "geometry": "geometry.java.gate",
        "rotation": [0, 0, 0], "materials": { "state": "state" } }]);
    let axis = assets::MODEL_TEMPLATE_FLAG_COMPOUND_NEXT | assets::MODEL_TEMPLATE_FLAG_GATE_AXIS_X;
    let base = base_with_shape(
        index,
        vec![
            assets::ModelTemplate {
                quad_start: 0,
                quad_count: 6,
                flags: axis,
            },
            assets::ModelTemplate {
                quad_start: 6,
                quad_count: 6,
                flags: 0,
            },
        ],
        false,
    );
    let overlay = compile(&pack_with_bindings(files, blocks), &base, &keys());
    let templates = &overlay.overlay.model_templates;
    assert_eq!(
        templates.len(),
        2,
        "four quads split over a head and a tail"
    );
    assert_eq!(templates[0].flags, axis);
    assert_eq!(templates[1].flags, 0);
    assert_eq!(
        templates
            .iter()
            .map(|template| template.quad_count)
            .sum::<u32>(),
        4
    );
    assert_eq!(overlay.overlay.template_overrides, vec![(index as u32, 0)]);
    base.with_block_overlay(index as u32 + 1, &overlay.overlay)
        .unwrap();
}
