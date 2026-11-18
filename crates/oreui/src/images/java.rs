//! The Java-style menus' files, resolved as Java resolves them: the selected resource packs first,
//! highest priority first, then the player's own game, cached by `launcher_host::java_ui`. A pack's
//! copy of a button, an icon, a background or the font replaces the game's. The sprites join the
//! OreUI texture pages under `java/` keys; without the files the Java style stays unavailable.

use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, RwLock},
};

use java_resource_pack::menu_files::{
    FONT_DEFINITIONS, FONT_TEXTURES, GUI_FILES, REQUIRED_GUI_FILES, SPLASHES, gui_path, mcmeta,
};
use serde_json::Value;

use super::{
    Art,
    java_font::{JavaFont, Sheet},
};

/// Key prefix of the Java sprites in the OreUI sprite map.
pub const KEY_PREFIX: &str = "java/";

/// The sprite key of a GUI file (relative to `textures/gui/`) or a font sheet (`font/<name>`).
#[must_use]
pub fn key(name: &str) -> String {
    format!("{KEY_PREFIX}{name}")
}

/// Reads a file by its path relative to `assets/minecraft/`, none when it is not there.
pub type ReadFile<'a> = &'a dyn Fn(&str) -> Option<Vec<u8>>;

/// Where files are read from, highest priority first: each pack in turn, then the cache folder.
pub struct Layers<'a> {
    pub packs: &'a [ReadFile<'a>],
    pub cache: Option<&'a Path>,
}

impl Layers<'_> {
    fn count(&self) -> usize {
        self.packs.len() + 1
    }

    /// The file `path` from one layer: a pack, or the cache for the last.
    fn read_at(&self, layer: usize, path: &str) -> Option<Vec<u8>> {
        match self.packs.get(layer) {
            Some(read) => read(path),
            None => std::fs::read(self.cache?.join(path)).ok(),
        }
    }

    /// The winning copy of `path` and the layer it came from.
    fn read(&self, path: &str) -> Option<(usize, Vec<u8>)> {
        (0..self.count()).find_map(|layer| Some((layer, self.read_at(layer, path)?)))
    }
}

/// What the menus read once resolved: the font, the sprites' nine-slice borders and the splashes.
struct Active {
    font: Arc<JavaFont>,
    borders: Arc<HashMap<String, [u16; 4]>>,
    splashes: Arc<Vec<String>>,
}

static ACTIVE: RwLock<Option<Active>> = RwLock::new(None);

/// Java's font, when its files resolved.
#[must_use]
pub fn font() -> Option<Arc<JavaFont>> {
    let active = ACTIVE.read().ok()?;
    active
        .as_ref()
        .map(|active| active.font.clone())
        .filter(|font| !font.is_empty())
}

/// The nine-slice borders of a sprite, as top, right, bottom and left texels; none for a sprite
/// that is stretched whole.
#[must_use]
pub fn border(key: &str) -> Option<[u16; 4]> {
    ACTIVE.read().ok()?.as_ref()?.borders.get(key).copied()
}

/// The title's splash lines: the packs' and the game's together, as Java combines them.
#[must_use]
pub fn splashes() -> Arc<Vec<String>> {
    ACTIVE
        .read()
        .ok()
        .and_then(|active| active.as_ref().map(|active| active.splashes.clone()))
        .unwrap_or_default()
}

/// The resolved files: the sprites and sheets to pack into texture pages, with the rest kept for
/// [`install`].
pub(super) struct Resolved {
    pub(super) art: Vec<(String, Art)>,
    active: Active,
}

impl Resolved {
    /// Makes the font, the borders and the splashes the menus read.
    pub(super) fn install(self) -> Vec<(String, Art)> {
        if let Ok(mut active) = ACTIVE.write() {
            *active = Some(self.active);
        }
        self.art
    }
}

/// Forgets what was installed, so the Java style is unavailable.
pub(super) fn clear() {
    if let Ok(mut active) = ACTIVE.write() {
        *active = None;
    }
}

fn decode(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let rgba = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some((width, height, rgba.into_raw()))
}

/// Resolves every Java menu file; none when the required sprites are in no layer.
pub(super) fn resolve(layers: &Layers<'_>) -> Option<Resolved> {
    if REQUIRED_GUI_FILES
        .iter()
        .any(|file| layers.read(&gui_path(file)).is_none())
    {
        return None;
    }
    let mut art = Vec::new();
    let mut borders = HashMap::new();
    for file in GUI_FILES {
        let path = gui_path(file);
        let Some((layer, bytes)) = layers.read(&path) else {
            continue;
        };
        let Some((width, height, pixels)) = decode(&bytes) else {
            tracing::warn!("Java menu sprite {file} is unreadable");
            continue;
        };
        // The borders come from the layer that supplied the texture. A texture that declares none
        // takes the next layer down's, scaled to its size: Java would stretch it whole, which
        // distorts a pack's button on any narrow or square control.
        if let Some(border) = (layer..layers.count())
            .find_map(|below| layers.read_at(below, &mcmeta(&path)))
            .and_then(|meta| parse_border(&meta, [width, height]))
        {
            borders.insert(key(file), border);
        }
        art.push((key(file), Art::Still(width, height, pixels)));
    }
    let (font, sheets) = resolve_font(layers);
    art.extend(sheets);
    let splashes = (0..layers.count())
        .filter_map(|layer| layers.read_at(layer, SPLASHES))
        .flat_map(|bytes| {
            String::from_utf8_lossy(&bytes)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect();
    Some(Resolved {
        art,
        active: Active {
            font: Arc::new(font),
            borders: Arc::new(borders),
            splashes: Arc::new(splashes),
        },
    })
}

/// The font providers of `path` from every layer, highest priority first, with references to
/// other font files replaced by their providers.
fn providers(layers: &Layers<'_>, path: &str, depth: u8) -> Vec<Value> {
    if depth > 4 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for layer in 0..layers.count() {
        let Some(json) = layers
            .read_at(layer, path)
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        else {
            continue;
        };
        for provider in json["providers"].as_array().into_iter().flatten() {
            if provider["type"] == "reference" {
                let id = provider["id"].as_str().unwrap_or_default();
                let id = id.strip_prefix("minecraft:").unwrap_or(id);
                if !id.is_empty() && !id.contains(['\\', ':']) && !id.contains("..") {
                    out.extend(providers(layers, &format!("font/{id}.json"), depth + 1));
                }
            } else {
                out.push(provider.clone());
            }
        }
    }
    out
}

/// The font and the bitmap sheets it draws from, as sprites. The default font's providers come
/// from every pack's `font/default.json`, then the game's, so a pack's glyphs win and the game's
/// fill the characters a pack leaves out.
fn resolve_font(layers: &Layers<'_>) -> (JavaFont, Vec<(String, Art)>) {
    let mut providers = providers(layers, FONT_DEFINITIONS[0], 0);
    if providers.is_empty() {
        // A game whose default font has no reference file: read its definitions directly.
        for definition in &FONT_DEFINITIONS[1..] {
            providers.extend(self::providers(layers, definition, 0));
        }
    }
    let mut sheets = HashMap::new();
    let mut art = Vec::new();
    for provider in &providers {
        let Some(name) = provider["file"]
            .as_str()
            .filter(|_| provider["type"] == "bitmap")
            .and_then(|file| file.strip_prefix("minecraft:font/"))
        else {
            continue;
        };
        if sheets.contains_key(name)
            || !name.ends_with(".png")
            || name.contains(['/', '\\'])
            || name.contains("..")
        {
            continue;
        }
        let Some((width, height, pixels)) = layers
            .read(&format!("{FONT_TEXTURES}{name}"))
            .and_then(|(_, bytes)| decode(&bytes))
        else {
            tracing::warn!("Java font sheet {name} is unreadable");
            continue;
        };
        art.push((
            key(&format!("font/{name}")),
            Art::Still(width, height, pixels.clone()),
        ));
        sheets.insert(
            name.to_owned(),
            Sheet {
                width,
                height,
                rgba: pixels,
            },
        );
    }
    let definition = serde_json::json!({ "providers": providers }).to_string();
    (JavaFont::build(&[definition.as_bytes()], &sheets), art)
}

/// The nine-slice border a Java `.mcmeta` declares, as top, right, bottom and left texels of a
/// texture `dimensions` wide and high. A border is written for the sprite's nominal size, so a
/// texture drawn at a higher resolution has it scaled up.
fn parse_border(bytes: &[u8], dimensions: [u32; 2]) -> Option<[u16; 4]> {
    let meta: Value = serde_json::from_slice(bytes).ok()?;
    let scaling = &meta["gui"]["scaling"];
    if scaling["type"] != "nine_slice" {
        return None;
    }
    let number = |value: &Value| value.as_f64().filter(|value| *value >= 0.0);
    let (nominal_w, nominal_h) = (
        number(&scaling["width"]).filter(|w| *w > 0.0),
        number(&scaling["height"]).filter(|h| *h > 0.0),
    );
    let scale = [
        nominal_w.map_or(1.0, |nominal| f64::from(dimensions[0]) / nominal),
        nominal_h.map_or(1.0, |nominal| f64::from(dimensions[1]) / nominal),
    ];
    let border = &scaling["border"];
    let (top, right, bottom, left) = match number(border) {
        Some(all) => (all, all, all, all),
        None => (
            number(&border["top"])?,
            number(&border["right"])?,
            number(&border["bottom"])?,
            number(&border["left"])?,
        ),
    };
    let texels = |value: f64, scale: f64| (value * scale).round().min(f64::from(u16::MAX)) as u16;
    Some([
        texels(top, scale[1]),
        texels(right, scale[0]),
        texels(bottom, scale[1]),
        texels(left, scale[0]),
    ])
}

#[cfg(test)]
mod tests;
