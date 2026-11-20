//! Java blockstates and block models as the pack file that rebinds vanilla block geometry.
//!
//! [`translate`] reads a flattened pack's `blockstates/*.json` and the models they name and writes
//! one Bedrock geometry file per model plus [`BINDINGS`], which lists for each vanilla block
//! state the geometry, its texture variables and its whole-block turn. Only what maps to Bedrock
//! exactly is written: a block whose variants name a property with no Bedrock equivalent, and a
//! block built from `multipart`, keep their vanilla shape.

use crate::{Layout, java_model, names};
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

/// Java block properties and the Bedrock block state each one is, with the Java values the
/// Bedrock state accepts. A property absent here has no exact Bedrock equivalent.
pub const STATE_ALIASES: &[(&str, &str, StateKind)] = &[
    ("axis", "pillar_axis", StateKind::Text),
    ("half", "minecraft:vertical_half", StateKind::Text),
    (
        "facing",
        "minecraft:cardinal_direction",
        StateKind::Cardinal,
    ),
    ("open", "open_bit", StateKind::Flag),
    ("lit", "lit", StateKind::Flag),
    ("hanging", "hanging", StateKind::Flag),
    ("powered", "powered_bit", StateKind::Flag),
];

/// Java properties that carry no Bedrock state and never change which model draws.
const IGNORED: &[&str] = &["waterlogged"];

/// How a Java property's value becomes the Bedrock state's value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateKind {
    /// The same string.
    Text,
    /// `true` and `false` as booleans.
    Flag,
    /// The four horizontal directions only; `up` and `down` have no cardinal value.
    Cardinal,
}

/// Maps one `property=value` of a Java variant key to its Bedrock state, `Ok(None)` for a
/// property that never changes the model, and `Err` when Bedrock has no equivalent.
fn bedrock_state(property: &str, value: &str) -> Result<Option<(&'static str, Value)>, ()> {
    if IGNORED.contains(&property) {
        return Ok(None);
    }
    let (_, name, kind) = STATE_ALIASES
        .iter()
        .find(|(java, ..)| *java == property)
        .ok_or(())?;
    let value = match kind {
        StateKind::Text => json!(value),
        StateKind::Flag => json!(match value {
            "true" => true,
            "false" => false,
            _ => return Err(()),
        }),
        StateKind::Cardinal => match value {
            "north" | "south" | "east" | "west" => json!(value),
            _ => return Err(()),
        },
    };
    Ok(Some((name, value)))
}

/// Parses a variant key such as `facing=north,half=top` into Bedrock states.
fn variant_states(key: &str) -> Result<Map<String, Value>, ()> {
    let mut states = Map::new();
    for pair in key.split(',').filter(|pair| !pair.is_empty()) {
        let (property, value) = pair.split_once('=').ok_or(())?;
        if let Some((name, value)) = bedrock_state(property, value)? {
            states.insert(name.to_owned(), value);
        }
    }
    Ok(states)
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
            let (Ok(states), Some(turn)) = (variant_states(key), rotation(x, y)) else {
                return false;
            };
            let Some(resolved) = java_model::resolve(model, &load) else {
                return false;
            };
            let Some((identifier, materials)) = geometry(layout, model, &resolved) else {
                return false;
            };
            geometries
                .entry(identifier.clone())
                .or_insert_with(|| java_model::to_geometry(&identifier, &resolved));
            block_entries.push(json!({
                "block": format!("minecraft:{block}"),
                "states": states,
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
        let name = texture.strip_prefix("block/")?;
        let path = names::block_path(layout, name);
        materials.insert(
            variable.to_owned(),
            json!(path.strip_prefix("blocks/").unwrap_or(&path)),
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
    fn variants_become_bedrock_states_with_their_geometry_and_turn() {
        let output = run(&lantern_files());
        let document = bindings(&output);
        let blocks = document["blocks"].as_array().unwrap();
        assert_eq!(blocks.len(), 2);
        let hanging = blocks
            .iter()
            .find(|entry| entry["states"]["hanging"] == json!(true))
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
    fn a_block_with_a_property_bedrock_lacks_keeps_its_vanilla_shape() {
        let mut files = lantern_files();
        files[0].1 = json!({ "variants": {
            "hanging=false": { "model": "block/lantern" },
            "damage=3": { "model": "block/lantern" } } });
        assert!(run(&files).files.is_empty());
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
    fn only_quarter_turns_of_a_whole_block_are_applied() {
        assert_eq!(rotation(0, 270), Some([0, 270, 0]));
        assert_eq!(rotation(-90, 0), Some([270, 0, 0]));
        assert_eq!(rotation(0, 45), None);
    }

    #[test]
    fn every_state_alias_is_a_distinct_java_property_and_converts_its_values() {
        let mut seen = BTreeSet::new();
        for (java, _, _) in STATE_ALIASES {
            assert!(seen.insert(*java), "{java} listed twice");
        }
        assert_eq!(
            bedrock_state("open", "true").unwrap(),
            Some(("open_bit", json!(true)))
        );
        assert!(bedrock_state("facing", "up").is_err());
        assert_eq!(bedrock_state("waterlogged", "true").unwrap(), None);
    }
}
