//! The Java-style menus' sprites, cached from the player's own Java Edition install by
//! `launcher_host::java_ui`. They join the OreUI texture pages under `java/` keys when the cache
//! exists; without it the Java style stays unavailable.

use std::{collections::HashMap, path::Path, sync::OnceLock};

use serde::Deserialize;

use super::{
    Art,
    java_font::{JavaFont, Sheet},
};

/// Folder of the cache inside the compiled assets directory.
const CACHE_DIR: &str = "java-ui";

/// Key prefix of the cached sprites in the OreUI sprite map.
pub const KEY_PREFIX: &str = "java/";

/// The sprite key of a cached GUI file such as `sprites/widget/button.png`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("{KEY_PREFIX}{name}")
}

/// Nine-slice borders (top, right, bottom, left, in texels) of the cached sprites that have them.
static BORDERS: OnceLock<HashMap<String, [u16; 4]>> = OnceLock::new();

/// The nine-slice borders of the cached sprite `key`, as top, right, bottom and left texels.
#[must_use]
pub fn border(key: &str) -> Option<[u16; 4]> {
    BORDERS.get()?.get(key).copied()
}

/// Java's bitmap font, built from the cached definitions and sheets; none without them.
static FONT: OnceLock<Option<JavaFont>> = OnceLock::new();

/// Java's font, when the cache holds it.
#[must_use]
pub fn font() -> Option<&'static JavaFont> {
    FONT.get()?.as_ref()
}

/// The title screen's splash lines from Java's `splashes.txt`; none without the cache.
static SPLASHES: OnceLock<Vec<String>> = OnceLock::new();

/// Java's splash lines, one per entry, when the cache holds them.
#[must_use]
pub fn splashes() -> &'static [String] {
    SPLASHES.get().map_or(&[], Vec::as_slice)
}

#[derive(Deserialize)]
struct Manifest {
    files: Vec<String>,
    #[serde(default)]
    font_files: Vec<String>,
    #[serde(default)]
    data_files: Vec<String>,
}

/// Decodes every cached sprite; an absent or unreadable cache gives none. Remembers the borders
/// the sprites' `.mcmeta` files declare.
pub(super) fn read(compiled_dir: &Path) -> Vec<(String, Art)> {
    let cache = compiled_dir.join(CACHE_DIR);
    let Some(manifest) = std::fs::read(cache.join("manifest.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Manifest>(&bytes).ok())
    else {
        return Vec::new();
    };
    let mut art = Vec::new();
    let mut borders = HashMap::new();
    for name in manifest.files {
        let Some(image) = std::fs::read(cache.join(&name))
            .ok()
            .and_then(|bytes| image::load_from_memory(&bytes).ok())
        else {
            tracing::warn!("Java menu sprite {name} is unreadable");
            continue;
        };
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        if let Some(border) = std::fs::read(cache.join(format!("{name}.mcmeta")))
            .ok()
            .and_then(|bytes| parse_border(&bytes))
        {
            borders.insert(key(&name), border);
        }
        art.push((key(&name), Art::Still(width, height, rgba.into_raw())));
    }
    let _ = BORDERS.set(borders);
    if manifest.data_files.iter().any(|file| file == "texts/splashes.txt") {
        let lines = std::fs::read_to_string(cache.join("texts/splashes.txt")).unwrap_or_default();
        let _ = SPLASHES.set(
            lines
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect(),
        );
    }
    let (font, sheets) = read_font(&cache, &manifest.font_files);
    let _ = FONT.set((!font.is_empty()).then_some(font));
    art.extend(sheets);
    art
}

/// The font from the cached definitions, and the sheets it draws from as sprites. The definitions
/// are read in the order the manifest lists them, which is their priority.
fn read_font(cache: &Path, files: &[String]) -> (JavaFont, Vec<(String, Art)>) {
    let mut sheets = HashMap::new();
    let mut art = Vec::new();
    for file in files.iter().filter(|file| file.ends_with(".png")) {
        let Some(image) = std::fs::read(cache.join(file))
            .ok()
            .and_then(|bytes| image::load_from_memory(&bytes).ok())
        else {
            tracing::warn!("Java font sheet {file} is unreadable");
            continue;
        };
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        let name = file.strip_prefix("font/").unwrap_or(file).to_owned();
        art.push((key(file), Art::Still(width, height, rgba.as_raw().clone())));
        sheets.insert(
            name,
            Sheet {
                width,
                height,
                rgba: rgba.into_raw(),
            },
        );
    }
    // The game lists the space definition first, so spaces win over any sheet's glyph.
    let mut names: Vec<&String> = files.iter().filter(|file| file.ends_with(".json")).collect();
    names.sort_by_key(|file| !file.ends_with("space.json"));
    let definitions: Vec<Vec<u8>> = names
        .into_iter()
        .filter_map(|file| std::fs::read(cache.join(file)).ok())
        .collect();
    let definitions: Vec<&[u8]> = definitions.iter().map(Vec::as_slice).collect();
    (JavaFont::build(&definitions, &sheets), art)
}

/// The nine-slice border a Java `.mcmeta` declares, as top, right, bottom and left texels.
fn parse_border(bytes: &[u8]) -> Option<[u16; 4]> {
    let meta: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let scaling = &meta["gui"]["scaling"];
    if scaling["type"] != "nine_slice" {
        return None;
    }
    let texels = |value: &serde_json::Value| u16::try_from(value.as_u64()?).ok();
    let border = &scaling["border"];
    if let Some(all) = texels(border) {
        return Some([all; 4]);
    }
    Some([
        texels(&border["top"])?,
        texels(&border["right"])?,
        texels(&border["bottom"])?,
        texels(&border["left"])?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_border_applies_to_every_side() {
        let meta = br#"{"gui":{"scaling":{"type":"nine_slice","width":200,"height":20,"border":3}}}"#;
        assert_eq!(parse_border(meta), Some([3; 4]));
    }

    #[test]
    fn per_side_borders_come_out_as_top_right_bottom_left() {
        let meta = br#"{"gui":{"scaling":{"type":"nine_slice","width":8,"height":20,
            "border":{"left":2,"top":1,"right":4,"bottom":3}}}}"#;
        assert_eq!(parse_border(meta), Some([1, 4, 3, 2]));
    }

    #[test]
    fn other_scaling_or_bad_files_have_no_border() {
        assert_eq!(parse_border(br#"{"gui":{"scaling":{"type":"stretch"}}}"#), None);
        assert_eq!(parse_border(b"not json"), None);
    }
}
