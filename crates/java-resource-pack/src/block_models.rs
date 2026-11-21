//! Java blockstates and block models as the pack file that rebinds vanilla block geometry.
//!
//! [`translate`] reads a flattened pack's `blockstates/*.json` and the models they name and writes
//! one Bedrock geometry file per model plus [`BINDINGS`], which lists for each Java block state
//! the geometry, its texture variables and its whole-block turn. Entries name the Java block and
//! its Java properties; the client finds the Bedrock states that map to them through the game's
//! Java mapping, so no name or state table is kept here. A block built from `multipart` keeps
//! its vanilla shape unless its parts depend only on the four horizontal connections, as a
//! fence's or a pane's do ([`multipart`]).

use crate::{Layout, java_model, multipart, names};
use serde_json::{Map, Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

/// The bindings document the client reads to redraw vanilla block states.
pub const BINDINGS: &str = "cinnabar/block_models.json";

/// Where the geometry of every translated model is written.
const GEOMETRY_FOLDER: &str = "models/blocks/java/";

const BLOCKSTATES: &str = "assets/minecraft/blockstates/";
const MODELS: &str = "assets/minecraft/models/";

/// The most blockstate files and distinct variants read from one pack.
const MAX_BLOCKS: usize = 4096;
const MAX_ENTRIES: usize = 16384;

/// Parses a variant key such as `facing=north,half=top` into Java properties.
fn variant_properties(key: &str) -> Result<Map<String, Value>, ()> {
    let mut properties = Map::new();
    for pair in key.split(',').filter(|pair| !pair.is_empty()) {
        let (property, value) = pair.split_once('=').ok_or(())?;
        properties.insert(property.to_owned(), json!(value));
    }
    Ok(properties)
}

/// A blockstate's variant: the first of a weighted list, as a model name and Java `x`/`y` turns.
fn variant(value: &Value) -> Option<(&str, i64, i64)> {
    let value = match value {
        Value::Array(list) => list.first()?,
        other => other,
    };
    let turn = |axis: &str| value.get(axis).and_then(Value::as_i64).unwrap_or(0);
    Some((value.get("model")?.as_str()?, turn("x"), turn("y")))
}

/// The turns a Java variant asks for as the whole-block rotation, or `None` when they are not
/// quarter turns.
fn rotation(x: i64, y: i64) -> Option<[i32; 3]> {
    (x % 90 == 0 && y % 90 == 0).then(|| [x.rem_euclid(360) as i32, y.rem_euclid(360) as i32, 0])
}

/// The files a pack's blockstates and models translate to.
#[derive(Debug, Default)]
pub(crate) struct Output {
    pub files: Vec<(String, Vec<u8>)>,
}

/// Translates a flattened pack's blockstates. `has` tells whether the pack holds a path and
/// `read` returns its bytes.
pub(crate) fn translate(
    layout: Layout,
    paths: &[String],
    mut read: impl FnMut(&str) -> Option<Vec<u8>>,
) -> Output {
    if layout != Layout::Flattened {
        return Output::default();
    }
    let documents: Vec<(&str, &str)> = paths
        .iter()
        .filter_map(|path| {
            let name = path.strip_prefix(BLOCKSTATES)?.strip_suffix(".json")?;
            (!name.contains('/')).then_some((path.as_str(), name))
        })
        .take(MAX_BLOCKS)
        .collect();
    if documents.is_empty() {
        return Output::default();
    }
    // Models are read once however many blocks name them.
    let source = RefCell::new((&mut read, BTreeMap::<String, Option<Value>>::new()));
    let load = |name: &str| -> Option<Value> {
        let mut source = source.borrow_mut();
        let (read, cache) = &mut *source;
        cache
            .entry(name.to_owned())
            .or_insert_with(|| {
                let bytes = read(&format!("{MODELS}{name}.json"))?;
                serde_json::from_slice(&bytes).ok()
            })
            .clone()
    };
    let mut entries = Vec::new();
    let mut geometries: BTreeMap<String, Value> = BTreeMap::new();
    for (path, block) in documents {
        let document = {
            let mut source = source.borrow_mut();
            (source.0)(path).and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        };
        if let Some(parts) = document
            .as_ref()
            .and_then(|document| document.get("multipart")?.as_array())
        {
            if entries.len() < MAX_ENTRIES
                && let Some((entry, files)) = connection_entry(layout, block, parts, &load)
            {
                entries.push(entry);
                geometries.extend(files);
            }
            continue;
        }
        let Some(variants) = document
            .as_ref()
            .and_then(|document| document.get("variants")?.as_object())
        else {
            continue;
        };
        let mut block_entries = Vec::new();
        let complete = variants.iter().all(|(key, value)| {
            let Some((model, x, y)) = variant(value) else {
                return false;
            };
            let (Ok(properties), Some(turn)) = (variant_properties(key), rotation(x, y)) else {
                return false;
            };
            let Some(resolved) = java_model::resolve(model, &load) else {
                return false;
            };
            // Whole cubes keep the texture pipeline's occluding cubes, and a turned element's
            // Java rescaling is not translated, so those stay vanilla.
            if resolved
                .elements
                .iter()
                .any(|element| element.rotation.is_some())
            {
                return false;
            }
            if is_full_cube(&resolved) {
                return true;
            }
            let Some((identifier, materials)) = geometry(layout, model, &resolved) else {
                return false;
            };
            geometries
                .entry(identifier.clone())
                .or_insert_with(|| java_model::to_geometry(&identifier, &resolved));
            block_entries.push(json!({
                "block": format!("minecraft:{block}"),
                "properties": properties,
                "geometry": identifier,
                "rotation": turn,
                "materials": materials,
            }));
            true
        });
        if complete && entries.len() + block_entries.len() <= MAX_ENTRIES {
            entries.extend(block_entries);
        }
    }
    if entries.is_empty() {
        return Output::default();
    }
    let mut files: Vec<(String, Vec<u8>)> = geometries
        .into_iter()
        .filter_map(|(identifier, document)| {
            let file = identifier.trim_start_matches("geometry.java.");
            Some((
                format!("{GEOMETRY_FOLDER}{file}.geo.json"),
                serde_json::to_vec(&document).ok()?,
            ))
        })
        .collect();
    let bindings = json!({ "format_version": 1, "blocks": entries });
    if let Ok(bytes) = serde_json::to_vec(&bindings) {
        files.push((BINDINGS.to_owned(), bytes));
    }
    Output { files }
}

/// The binding of a fence or pane: the geometry that always draws, one geometry per connection
/// mask, and the Bedrock texture key each merged texture variable names. `None` when the block
/// is not drawn from connections alone, or a texture is not a block texture.
fn connection_entry(
    layout: Layout,
    block: &str,
    parts: &[Value],
    load: &dyn Fn(&str) -> Option<Value>,
) -> Option<(Value, BTreeMap<String, Value>)> {
    let connected = multipart::connected(parts, load)?;
    let mut geometries = BTreeMap::new();
    let mut materials = Map::new();
    let mut ids = Vec::new();
    for (suffix, model) in std::iter::once(("always".to_owned(), &connected.always)).chain(
        connected
            .masks
            .iter()
            .enumerate()
            .map(|(mask, model)| (format!("m{mask}"), model)),
    ) {
        for (variable, texture) in &model.textures {
            materials.insert(variable.clone(), json!(block_texture_key(layout, texture)?));
        }
        let identifier = format!("geometry.java.{block}_{suffix}");
        geometries.insert(
            identifier.clone(),
            java_model::to_geometry(&identifier, model),
        );
        ids.push(identifier);
    }
    let always = ids.remove(0);
    let entry = json!({
        "block": format!("minecraft:{block}"),
        "connections": { "always": always, "masks": ids },
        "materials": materials,
    });
    Some((entry, geometries))
}

/// The Bedrock terrain key of a Java block texture, or `None` for another kind of texture.
/// The translator keeps block textures by file name, whatever folder they sit in.
fn block_texture_key(layout: Layout, texture: &str) -> Option<String> {
    let name = texture.strip_prefix("block/")?;
    let path = names::block_path(layout, name.rsplit('/').next().unwrap_or(name));
    Some(path.strip_prefix("blocks/").unwrap_or(&path).to_owned())
}

/// Whether the model is one box filling the block.
fn is_full_cube(model: &java_model::Model) -> bool {
    matches!(model.elements.as_slice(), [element]
        if element.from == [0.0; 3] && element.to == [16.0; 3])
}

/// The geometry identifier of a model and the Bedrock texture key each variable it draws names;
/// `None` when a face names a texture the model never defines.
fn geometry(
    layout: Layout,
    model: &str,
    resolved: &java_model::Model,
) -> Option<(String, Map<String, Value>)> {
    let variables: BTreeSet<&str> = resolved
        .elements
        .iter()
        .flat_map(|element| element.faces.values())
        .map(|face| face.texture.as_str())
        .collect();
    let mut materials = Map::new();
    for variable in variables {
        let texture = resolved.texture(variable)?;
        materials.insert(
            variable.to_owned(),
            json!(block_texture_key(layout, texture)?),
        );
    }
    let name = model
        .strip_prefix("minecraft:")
        .unwrap_or(model)
        .replace('/', "_");
    Some((format!("geometry.java.{name}"), materials))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(files: &[(&str, Value)]) -> Output {
        let paths: Vec<String> = files.iter().map(|(path, _)| (*path).to_owned()).collect();
        translate(Layout::Flattened, &paths, |path| {
            files
                .iter()
                .find(|(name, _)| *name == path)
                .map(|(_, value)| serde_json::to_vec(value).unwrap())
        })
    }

    fn bindings(output: &Output) -> Value {
        let bytes = &output
            .files
            .iter()
            .find(|(path, _)| path == BINDINGS)
            .unwrap()
            .1;
        serde_json::from_slice(bytes).unwrap()
    }

    fn lantern_files() -> Vec<(&'static str, Value)> {
        vec![
            (
                "assets/minecraft/blockstates/lantern.json",
                json!({ "variants": {
                    "hanging=false": { "model": "block/lantern" },
                    "hanging=true": { "model": "block/hanging_lantern", "y": 90 } } }),
            ),
            (
                "assets/minecraft/models/block/lantern.json",
                json!({ "textures": { "lantern": "block/lantern" }, "elements": [{
                    "from": [5, 0, 5], "to": [11, 7, 11],
                    "faces": { "north": { "texture": "#lantern" } } }] }),
            ),
            (
                "assets/minecraft/models/block/hanging_lantern.json",
                json!({ "parent": "block/lantern" }),
            ),
        ]
    }

    #[test]
    fn variants_keep_their_java_properties_with_their_geometry_and_turn() {
        let output = run(&lantern_files());
        let document = bindings(&output);
        let blocks = document["blocks"].as_array().unwrap();
        assert_eq!(blocks.len(), 2);
        let hanging = blocks
            .iter()
            .find(|entry| entry["properties"]["hanging"] == "true")
            .unwrap();
        assert_eq!(hanging["block"], "minecraft:lantern");
        assert_eq!(hanging["rotation"], json!([0, 90, 0]));
        assert_eq!(hanging["geometry"], "geometry.java.block_hanging_lantern");
        assert_eq!(hanging["materials"]["lantern"], "lantern");
        assert!(
            output
                .files
                .iter()
                .any(|(path, _)| path == "models/blocks/java/block_hanging_lantern.geo.json")
        );
    }

    #[test]
    fn an_unmapped_looking_property_is_passed_through_for_the_client_to_match() {
        let mut files = lantern_files();
        files[0].1 = json!({ "variants": { "damage=3": { "model": "block/lantern" } } });
        let blocks = bindings(&run(&files));
        assert_eq!(blocks["blocks"][0]["properties"]["damage"], "3");
    }

    #[test]
    fn multipart_blocks_and_missing_models_are_left_to_vanilla() {
        let mut files = lantern_files();
        files[0].1 = json!({ "multipart": [{ "apply": { "model": "block/lantern" } }] });
        assert!(run(&files).files.is_empty());
        files[0].1 = json!({ "variants": { "": { "model": "block/absent" } } });
        assert!(run(&files).files.is_empty());
    }

    #[test]
    fn whole_cubes_and_turned_elements_stay_vanilla() {
        let cube = json!({ "textures": { "all": "block/stone" }, "elements": [{
            "from": [0, 0, 0], "to": [16, 16, 16],
            "faces": { "north": { "texture": "#all" } } }] });
        let files = |model: Value| {
            vec![
                (
                    "assets/minecraft/blockstates/stone.json",
                    json!({ "variants": { "": [{ "model": "block/stone" }, { "model": "block/stone", "y": 90 }] } }),
                ),
                ("assets/minecraft/models/block/stone.json", model),
            ]
        };
        assert!(run(&files(cube)).files.is_empty());
        let turned = json!({ "textures": { "all": "block/stone" }, "elements": [{
            "from": [2, 0, 2], "to": [14, 16, 14],
            "rotation": { "origin": [8, 8, 8], "axis": "y", "angle": 45 },
            "faces": { "north": { "texture": "#all" } } }] });
        assert!(run(&files(turned)).files.is_empty());
    }

    #[test]
    fn a_texture_in_a_subfolder_is_named_by_its_file() {
        let mut files = lantern_files();
        files[1].1 = json!({ "textures": { "lantern": "block/extra/lantern" }, "elements": [{
            "from": [5, 0, 5], "to": [11, 7, 11],
            "faces": { "north": { "texture": "#lantern" } } }] });
        let blocks = bindings(&run(&files));
        assert_eq!(blocks["blocks"][0]["materials"]["lantern"], "lantern");
    }
    #[test]
    fn only_quarter_turns_of_a_whole_block_are_applied() {
        assert_eq!(rotation(0, 270), Some([0, 270, 0]));
        assert_eq!(rotation(-90, 0), Some([270, 0, 0]));
        assert_eq!(rotation(0, 45), None);
    }

    #[test]
    fn a_fence_binds_a_post_and_a_geometry_for_each_connection_mask() {
        let mut parts = vec![json!({ "apply": { "model": "block/oak_fence_post" } })];
        for (turn, direction) in ["north", "east", "south", "west"].iter().enumerate() {
            parts.push(
                json!({ "apply": { "model": "block/oak_fence_side", "y": turn * 90 },
                "when": { *direction: "true" } }),
            );
        }
        let files = vec![
            (
                "assets/minecraft/blockstates/oak_fence.json",
                json!({ "multipart": parts }),
            ),
            (
                "assets/minecraft/models/block/oak_fence_post.json",
                json!({ "parent": "block/fence_post", "textures": { "texture": "block/oak_planks" } }),
            ),
            (
                "assets/minecraft/models/block/oak_fence_side.json",
                json!({ "parent": "block/fence_side", "textures": { "texture": "block/oak_planks" } }),
            ),
        ];
        let output = run(&files);
        let document = bindings(&output);
        let entry = &document["blocks"][0];
        assert_eq!(entry["block"], "minecraft:oak_fence");
        assert_eq!(
            entry["connections"]["always"],
            "geometry.java.oak_fence_always"
        );
        assert_eq!(entry["connections"]["masks"].as_array().unwrap().len(), 16);
        assert!(entry["materials"].as_object().unwrap().len() == 1);
        // The post, plus one geometry per mask.
        assert_eq!(
            output
                .files
                .iter()
                .filter(|(path, _)| path.starts_with("models/blocks/java/oak_fence_"))
                .count(),
            17
        );
    }
}
