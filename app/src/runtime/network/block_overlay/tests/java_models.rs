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
    let diagnostic = RuntimeAssets::diagnostic();
    let mut pane = super::super::diagnostic_visual();
    pane.kind = VisualKind::Model;
    pane.support = VisualSupport::Exact;
    pane.model_template = 0;
    pane.faces = [1; 6];
    let counts: Vec<u32> = if family == assets::MODEL_TEMPLATE_FLAG_PANE {
        vec![6; 16]
    } else {
        std::iter::once(6)
            .chain((0..16u32).map(|mask| mask.count_ones() * 8))
            .collect()
    };
    let mut start = 0;
    let templates: Vec<assets::ModelTemplate> = counts
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
    let quad = assets::ModelQuad {
        positions: [[0; 3]; 4],
        uvs: [[0; 2]; 4],
        material: 1,
        flags: 0,
    };
    let compiled = assets::CompiledAssets {
        visuals: vec![pane].into(),
        light_properties: vec![assets::LightProperties::OPAQUE_DARK; 1].into(),
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
        model_quads: vec![quad; start as usize].into(),
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
    // The post is one box; mask `m` adds one box for each connection it names.
    let model = |boxes: usize| {
        let element = |index: usize| java_resource_pack::java_model::Element {
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
        java_resource_pack::java_model::Model {
            textures: [("state".to_owned(), "block/state".to_owned())].into(),
            elements: (0..boxes).map(element).collect(),
        }
    };
    let geometry = |name: &str, boxes: usize| {
        let document = java_resource_pack::java_model::to_geometry(name, &model(boxes));
        (
            format!(
                "models/blocks/java/{}.geo.json",
                name.trim_start_matches("geometry.java.")
            ),
            document.to_string().into_bytes(),
        )
    };
    let masks: Vec<String> = (0..16)
        .map(|mask| format!("geometry.java.pane_m{mask}"))
        .collect();
    let mut files = vec![geometry("geometry.java.pane_always", 1)];
    files.extend(
        masks
            .iter()
            .enumerate()
            .map(|(mask, name)| geometry(name, (mask as u32).count_ones() as usize)),
    );
    let bindings = json!({ "format_version": 1, "blocks": [{
        "block": named,
        "connections": { "always": "geometry.java.pane_always", "masks": masks },
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
