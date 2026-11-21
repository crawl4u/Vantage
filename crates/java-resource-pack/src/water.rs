//! Java's water look as the pack file the client reads to recolour Bedrock biome water.
//!
//! Java draws water from a grey texture whose alpha is the pack's own, tinted by a per-biome
//! colour. Bedrock's biome water colours are brighter and its biome opacity is multiplied into
//! the texture alpha, so a Java pack's water would come out paler and more transparent. A pack
//! that supplies water art therefore also gets [`FILE`], naming Java's biome colours and full
//! biome opacity, so the texture alpha alone decides transparency as in Java.

use serde_json::{Map, Value, json};

/// The pack file the client reads.
pub(crate) const FILE: &str = "cinnabar/water.json";

const TABLE: &str = include_str!("../data/biome-water.tsv");
/// The table key of the colour every unlisted biome has.
const DEFAULT_KEY: &str = "*";

/// The document recolouring water to Java's biome colours with the texture alpha untouched.
pub(crate) fn document() -> Vec<u8> {
    let mut default = None;
    let mut biomes = Map::new();
    for line in TABLE.lines().filter(|line| !line.starts_with('#')) {
        let Some((name, rgb)) = line.split_once('\t') else {
            continue;
        };
        let colour = Value::from(format!("#{}", rgb.trim()));
        if name == DEFAULT_KEY {
            default = Some(colour);
        } else {
            biomes.insert(name.to_owned(), colour);
        }
    }
    let document = json!({ "default": default, "opacity": 1.0, "biomes": biomes });
    serde_json::to_vec(&document).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_document_names_a_default_and_the_biomes_that_differ() {
        let document: Value = serde_json::from_slice(&document()).unwrap();
        assert_eq!(document["default"], "#3f76e4");
        assert_eq!(document["opacity"], 1.0);
        assert_eq!(document["biomes"]["swamp"], "#617b64");
        assert_eq!(document["biomes"]["warm_ocean"], "#43d5ee");
        assert!(document["biomes"]["plains"].is_null());
    }
}
