//! Which texture a Java item draws, as its item definition and model say.
//!
//! A pack decides an item's look through `items/<name>.json` (a definition that picks a model)
//! and the model's `layer0` texture, not through the texture's file name: a pack may draw
//! `diamond_sword` from `item/ruby_blade.png`. [`redirects`] follows each item to its texture,
//! so the translator can write that texture under the item's own name.

use crate::{Layout, java_model};
use serde_json::Value;
use std::{cell::RefCell, collections::BTreeMap};

const ITEM_DEFINITIONS: &str = "assets/minecraft/items/";
const MODELS: &str = "assets/minecraft/models/";
const TEXTURES: &str = "assets/minecraft/textures/";

/// The most items followed in one pack.
const MAX_ITEMS: usize = 4096;

/// The model an item definition draws when no condition says otherwise: a plain model, or the
/// fallback or `on_false` branch of a condition, range dispatch or selection.
fn default_model(node: &Value) -> Option<&str> {
    let node = node
        .get("model")
        .filter(|inner| inner.is_object())
        .unwrap_or(node);
    match node
        .get("type")
        .and_then(Value::as_str)?
        .trim_start_matches("minecraft:")
    {
        "model" => node.get("model")?.as_str(),
        "condition" => default_model(node.get("on_false")?),
        "range_dispatch" | "select" => default_model(node.get("fallback")?),
        _ => None,
    }
}

/// `(item, texture path)` for every item whose model draws a texture other than the one its
/// own name gives. The texture path is the pack file the item draws. Items are named by their
/// definition or, without one, by a model in `models/item/`.
pub(crate) fn redirects(
    layout: Layout,
    paths: &[String],
    mut read: impl FnMut(&str) -> Option<Vec<u8>>,
) -> Vec<(String, String)> {
    if layout != Layout::Flattened {
        return Vec::new();
    }
    let mut items: BTreeMap<&str, Option<String>> = BTreeMap::new();
    for path in paths {
        if let Some(name) = path
            .strip_prefix(MODELS)
            .and_then(|rest| rest.strip_prefix("item/"))
            .and_then(|rest| rest.strip_suffix(".json"))
            .filter(|name| !name.contains('/'))
        {
            items
                .entry(name)
                .or_insert_with(|| Some(format!("item/{name}")));
        }
    }
    for path in paths {
        if let Some(name) = path
            .strip_prefix(ITEM_DEFINITIONS)
            .and_then(|rest| rest.strip_suffix(".json"))
            .filter(|name| !name.contains('/'))
        {
            let model = read(path)
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .and_then(|definition| default_model(&definition).map(str::to_owned));
            items.insert(
                name,
                model.map(|model| model.trim_start_matches("minecraft:").to_owned()),
            );
        }
    }
    let source = RefCell::new((&mut read, BTreeMap::<String, Option<Value>>::new()));
    let load = |name: &str| -> Option<Value> {
        let mut source = source.borrow_mut();
        let (read, cache) = &mut *source;
        cache
            .entry(name.to_owned())
            .or_insert_with(|| serde_json::from_slice(&read(&format!("{MODELS}{name}.json"))?).ok())
            .clone()
    };
    let exists = |texture: &str| {
        paths
            .binary_search_by(|path| path.as_str().cmp(texture))
            .is_ok()
    };
    let mut result = Vec::new();
    for (name, model) in items.into_iter().take(MAX_ITEMS) {
        let Some(model) = model.and_then(|model| java_model::resolve(&model, &load)) else {
            continue;
        };
        let Some(texture) = model.texture("layer0") else {
            continue;
        };
        // A model drawn in layers, such as a potion's bottle over its tinted liquid, is more than
        // its first layer: one texture would lose the rest, so the item keeps its own picture.
        if model.texture("layer1").is_some() {
            continue;
        }
        let texture = texture.trim_start_matches("minecraft:");
        let path = format!("{TEXTURES}{texture}.png");
        let own = format!("{TEXTURES}item/{name}.png");
        if path != own && exists(&path) {
            result.push((name.to_owned(), path));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run(files: &[(&str, Value)]) -> Vec<(String, String)> {
        let mut paths: Vec<String> = files.iter().map(|(path, _)| (*path).to_owned()).collect();
        paths.sort();
        redirects(Layout::Flattened, &paths, |path| {
            files
                .iter()
                .find(|(name, _)| *name == path)
                .map(|(_, value)| serde_json::to_vec(value).unwrap())
        })
    }

    const PNG: &str = "assets/minecraft/textures/item/ruby_blade.png";

    #[test]
    fn an_item_draws_the_texture_its_model_names_not_its_own_file_name() {
        let files = [
            (PNG, json!(null)),
            (
                "assets/minecraft/models/item/diamond_sword.json",
                json!({ "parent": "item/handheld", "textures": { "layer0": "item/ruby_blade" } }),
            ),
        ];
        assert_eq!(run(&files), [("diamond_sword".to_owned(), PNG.to_owned())]);
    }

    #[test]
    fn a_definition_picks_the_model_and_conditions_fall_back_to_their_default() {
        let files = [
            (PNG, json!(null)),
            (
                "assets/minecraft/items/bow.json",
                json!({ "model": { "type": "minecraft:condition",
                    "on_true": { "type": "minecraft:model", "model": "minecraft:item/pulling" },
                    "on_false": { "type": "minecraft:model", "model": "minecraft:item/ruby" } } }),
            ),
            (
                "assets/minecraft/models/item/ruby.json",
                json!({ "parent": "minecraft:item/generated",
                    "textures": { "layer0": "minecraft:item/ruby_blade" } }),
            ),
        ];
        // The model the definition names is also a model file, so it reads as an item of its own.
        assert!(run(&files).contains(&("bow".to_owned(), PNG.to_owned())));
    }

    #[test]
    fn models_naming_their_own_texture_or_a_missing_one_redirect_nothing() {
        let files = [
            ("assets/minecraft/textures/item/apple.png", json!(null)),
            (
                "assets/minecraft/models/item/apple.json",
                json!({ "parent": "item/generated", "textures": { "layer0": "item/apple" } }),
            ),
            (
                "assets/minecraft/models/item/stick.json",
                json!({ "parent": "item/generated", "textures": { "layer0": "item/absent" } }),
            ),
        ];
        assert!(run(&files).is_empty());
    }

    #[test]
    fn a_model_drawn_in_layers_redirects_nothing_because_one_texture_would_lose_the_rest() {
        let files = [
            (PNG, json!(null)),
            (
                "assets/minecraft/models/item/splash_potion.json",
                json!({ "parent": "item/generated",
                    "textures": { "layer0": "item/ruby_blade", "layer1": "item/splash_potion" } }),
            ),
        ];
        assert!(run(&files).is_empty());
    }
}
