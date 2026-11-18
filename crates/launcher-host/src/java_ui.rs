//! The Java-style menus' sprites, read from the player's own Java Edition install.
//!
//! Nothing from the game is shipped: this finds the newest Java Edition client jar on this
//! machine and copies the few GUI sprites the menus draw into the local asset folder, the way
//! the Bedrock sample pack is fetched at first run. When no install is found, the Java style
//! reports itself unavailable and the Bedrock look stays.

use std::{
    io::Read,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use launcher::install_layout::InstallLayout;
use serde::{Deserialize, Serialize};

/// Where the sprites sit inside the client jar.
const JAR_GUI_ROOT: &str = "assets/minecraft/textures/gui/";

/// The GUI files the menus draw, relative to the jar's `textures/gui/`. Each may carry a
/// `.mcmeta` beside it that sets its nine-slice borders; that is copied too when present.
pub const SPRITES: &[&str] = &[
    "sprites/widget/button.png",
    "sprites/widget/button_highlighted.png",
    "sprites/widget/button_disabled.png",
    "sprites/widget/slider.png",
    "sprites/widget/slider_highlighted.png",
    "sprites/widget/slider_handle.png",
    "sprites/widget/slider_handle_highlighted.png",
    "sprites/widget/checkbox.png",
    "sprites/widget/checkbox_highlighted.png",
    "sprites/widget/checkbox_selected.png",
    "sprites/widget/checkbox_selected_highlighted.png",
    "sprites/widget/text_field.png",
    "sprites/widget/text_field_highlighted.png",
    "menu_list_background.png",
    "inworld_menu_list_background.png",
    "header_separator.png",
    "footer_separator.png",
    "inworld_header_separator.png",
    "inworld_footer_separator.png",
];

/// Sprites without which the Java style cannot draw its controls.
pub const REQUIRED: &[&str] = &[
    "sprites/widget/button.png",
    "sprites/widget/button_highlighted.png",
    "sprites/widget/button_disabled.png",
];

/// The font definitions copied from the jar, as (jar path, cached name). The bitmap sheets they
/// name are copied too, as `font/<file>`.
const FONT_DEFINITIONS: &[(&str, &str)] = &[
    ("assets/minecraft/font/include/default.json", "font/default.json"),
    ("assets/minecraft/font/include/space.json", "font/space.json"),
];

/// Where the bitmap sheets a font definition names sit in the jar.
const JAR_FONT_TEXTURES: &str = "assets/minecraft/textures/font/";

/// What the cache holds. A cache written under an older number is read from the jar again, so a
/// new kind of file reaches players whose jar has not changed.
const CACHE_FORMAT: u32 = 2;

/// No single sprite is larger than this; anything bigger is not a GUI sprite.
const MAX_SPRITE_BYTES: u64 = 2 * 1024 * 1024;

/// A Java Edition client found on this machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientJar {
    /// The version id, such as `1.21.8`.
    pub version: String,
    pub path: PathBuf,
}

/// What the cache holds, written beside the sprites.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Manifest {
    /// The cache layout this was written under; older caches have none.
    #[serde(default)]
    pub format: u32,
    pub version: String,
    /// Size and modified time of the jar the sprites came from, to notice a changed install.
    pub jar_len: u64,
    pub jar_modified: u64,
    /// The sprites copied, as listed in [`SPRITES`].
    pub files: Vec<String>,
    /// The listed sprites the jar did not hold.
    pub missing: Vec<String>,
    /// The font definitions and bitmap sheets copied, as `font/...` cached names. The Java text
    /// is drawn from them; without them the menus keep the client's own font.
    #[serde(default)]
    pub font_files: Vec<String>,
}

/// Whether the Java style can be drawn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Status {
    /// The sprites are cached for this Java version.
    Ready { version: String },
    /// No Java Edition client was found on this machine.
    NoJavaInstall,
    /// A client was found but lacks sprites the menus need.
    Incomplete { version: String, missing: Vec<String> },
    /// Reading or writing failed.
    Failed(String),
}

impl Status {
    /// Whether the Java style may be shown.
    #[must_use]
    pub fn ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }
}

/// Where the sprites are cached for `layout`.
#[must_use]
pub fn cache_dir(layout: &InstallLayout) -> PathBuf {
    layout.compiled_assets.join("java-ui")
}

fn manifest_path(cache: &Path) -> PathBuf {
    cache.join("manifest.json")
}

/// The folders a Java Edition launcher keeps its game in on this machine, most likely first. An
/// explicit `VANTAGE_JAVA_DIR` comes before the usual per-platform locations.
#[must_use]
pub fn default_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = std::env::var_os("VANTAGE_JAVA_DIR").filter(|dir| !dir.is_empty()) {
        roots.push(PathBuf::from(dir));
    }
    if cfg!(windows) {
        if let Some(roaming) = std::env::var_os("APPDATA") {
            roots.push(PathBuf::from(roaming).join(".minecraft"));
        }
    } else if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        if cfg!(target_os = "macos") {
            roots.push(home.join("Library/Application Support/minecraft"));
        }
        roots.push(home.join(".minecraft"));
    }
    roots
}

/// The newest release client among the `versions` folders of `roots`, else the newest of any
/// kind. A version counts only when its jar and its JSON are both there.
#[must_use]
pub fn find_client_jar(roots: &[PathBuf]) -> Option<ClientJar> {
    struct Candidate {
        jar: ClientJar,
        release: bool,
        released_at: String,
    }
    let mut best: Option<Candidate> = None;
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root.join("versions")) else {
            continue;
        };
        for entry in entries.flatten() {
            let id = entry.file_name().to_string_lossy().into_owned();
            let dir = entry.path();
            let (jar, json) = (dir.join(format!("{id}.jar")), dir.join(format!("{id}.json")));
            if !jar.is_file() {
                continue;
            }
            let Some(meta) = std::fs::read(&json)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            else {
                continue;
            };
            let candidate = Candidate {
                release: meta["type"] == "release",
                released_at: meta["releaseTime"].as_str().unwrap_or_default().to_owned(),
                jar: ClientJar {
                    version: id,
                    path: jar,
                },
            };
            let better = best.as_ref().is_none_or(|best| {
                (candidate.release, &candidate.released_at) > (best.release, &best.released_at)
            });
            if better {
                best = Some(candidate);
            }
        }
    }
    best.map(|best| best.jar)
}

/// Copies the sprites of `jar` into `cache` and writes the manifest. Missing sprites are
/// recorded, not errors; the caller judges them against [`REQUIRED`].
pub fn extract(jar: &ClientJar, cache: &Path) -> Result<Manifest, String> {
    let meta = std::fs::metadata(&jar.path).map_err(|error| error.to_string())?;
    let file = std::fs::File::open(&jar.path).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(cache).map_err(|error| error.to_string())?;
    let mut manifest = Manifest {
        format: CACHE_FORMAT,
        version: jar.version.clone(),
        jar_len: meta.len(),
        jar_modified: meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |elapsed| elapsed.as_secs()),
        ..Manifest::default()
    };
    for sprite in SPRITES {
        let Some(bytes) = read_entry(&mut archive, &format!("{JAR_GUI_ROOT}{sprite}"))? else {
            manifest.missing.push((*sprite).to_owned());
            continue;
        };
        write_cached(cache, sprite, &bytes)?;
        manifest.files.push((*sprite).to_owned());
        let meta_name = format!("{sprite}.mcmeta");
        if let Some(bytes) = read_entry(&mut archive, &format!("{JAR_GUI_ROOT}{meta_name}"))? {
            write_cached(cache, &meta_name, &bytes)?;
        }
    }
    extract_font(&mut archive, cache, &mut manifest)?;
    let json = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    std::fs::write(manifest_path(cache), json).map_err(|error| error.to_string())?;
    Ok(manifest)
}

/// Copies the font definitions and the bitmap sheets the default font names. A jar without them
/// (an older version) just has no Java font.
fn extract_font(
    archive: &mut zip::ZipArchive<std::fs::File>,
    cache: &Path,
    manifest: &mut Manifest,
) -> Result<(), String> {
    let mut sheets = Vec::new();
    for (jar_name, cached) in FONT_DEFINITIONS {
        let Some(bytes) = read_entry(archive, jar_name)? else {
            continue;
        };
        write_cached(cache, cached, &bytes)?;
        manifest.font_files.push((*cached).to_owned());
        sheets.extend(bitmap_sheets(&bytes));
    }
    for sheet in sheets {
        let cached = format!("font/{sheet}");
        if manifest.font_files.contains(&cached) {
            continue;
        }
        if let Some(bytes) = read_entry(archive, &format!("{JAR_FONT_TEXTURES}{sheet}"))? {
            write_cached(cache, &cached, &bytes)?;
            manifest.font_files.push(cached);
        }
    }
    Ok(())
}

/// The file names of the bitmap sheets a font definition names, such as `ascii.png`.
fn bitmap_sheets(definition: &[u8]) -> Vec<String> {
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(definition) else {
        return Vec::new();
    };
    json["providers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|provider| provider["type"] == "bitmap")
        .filter_map(|provider| provider["file"].as_str())
        .filter_map(|file| file.strip_prefix("minecraft:font/"))
        // Only plain sheet names: nothing may reach outside the font folder.
        .filter(|name| name.ends_with(".png") && !name.contains(['/', '\\']) && !name.contains(".."))
        .map(str::to_owned)
        .collect()
}

/// One archive entry's bytes, none when absent. An oversized entry is an error.
fn read_entry(
    archive: &mut zip::ZipArchive<std::fs::File>,
    name: &str,
) -> Result<Option<Vec<u8>>, String> {
    let entry = match archive.by_name(name) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if entry.size() > MAX_SPRITE_BYTES {
        return Err(format!("{name} is larger than a GUI sprite"));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .take(MAX_SPRITE_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(Some(bytes))
}

/// Writes one cached file under `cache`; `name` is always one of this module's fixed names.
fn write_cached(cache: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    let path = cache.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, bytes).map_err(|error| error.to_string())
}

/// What a manifest says about whether the Java style can be drawn.
fn judge(manifest: &Manifest) -> Status {
    let missing: Vec<String> = REQUIRED
        .iter()
        .filter(|name| !manifest.files.iter().any(|file| file == *name))
        .map(|name| (*name).to_owned())
        .collect();
    if missing.is_empty() {
        Status::Ready {
            version: manifest.version.clone(),
        }
    } else {
        Status::Incomplete {
            version: manifest.version.clone(),
            missing,
        }
    }
}

fn read_manifest(cache: &Path) -> Option<Manifest> {
    serde_json::from_slice(&std::fs::read(manifest_path(cache)).ok()?).ok()
}

/// The status of the cache as it stands, without looking for an install. `NoJavaInstall` when
/// nothing is cached.
#[must_use]
pub fn cached_status(layout: &InstallLayout) -> Status {
    read_manifest(&cache_dir(layout)).map_or(Status::NoJavaInstall, |manifest| judge(&manifest))
}

/// Finds the Java install under `roots`, caches its sprites unless the cache already matches that
/// jar, and returns the resulting status.
#[must_use]
pub fn prepare_from(layout: &InstallLayout, roots: &[PathBuf]) -> Status {
    let cache = cache_dir(layout);
    let Some(jar) = find_client_jar(roots) else {
        // A cache from an install that has since been removed keeps working.
        return cached_status(layout);
    };
    if let (Some(manifest), Ok(meta)) = (read_manifest(&cache), std::fs::metadata(&jar.path)) {
        let modified = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |elapsed| elapsed.as_secs());
        if manifest.format == CACHE_FORMAT
            && manifest.version == jar.version
            && manifest.jar_len == meta.len()
            && manifest.jar_modified == modified
        {
            return judge(&manifest);
        }
    }
    match extract(&jar, &cache) {
        Ok(manifest) => judge(&manifest),
        Err(error) => Status::Failed(error),
    }
}

/// [`prepare_from`] over this machine's usual Java Edition folders.
#[must_use]
pub fn prepare(layout: &InstallLayout) -> Status {
    prepare_from(layout, &default_roots())
}

#[cfg(test)]
mod tests;
