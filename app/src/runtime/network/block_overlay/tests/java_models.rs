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
    let id = "00000000-0000-0000-0000-000000000001";
    let manifest = json!({"format_version":2,"header":{"uuid":id,"version":[1,0,0]},
        "modules":[{"type":"resources"}]});
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (path, bytes) in [
        ("manifest.json", manifest.to_string().into_bytes()),
        (
            "textures/terrain_texture.json",
            terrain.to_string().into_bytes(),
        ),
        ("textures/base_a.png", png(16, 16, |_, _| [9, 90, 200, 255])),
        (
            "models/blocks/java/stub.geo.json",
            geometry.to_string().into_bytes(),
        ),
        (
            java_resource_pack::block_models::BINDINGS,
            bindings.to_string().into_bytes(),
        ),
    ] {
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
