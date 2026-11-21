//! A detected Java Edition install, read-only, as the lowest layer under every Java pack.
//!
//! A Java pack ships art only for what it restyles, and Java draws the rest from its own
//! vanilla assets. Without a base layer every omitted texture would fall back to Bedrock's
//! art, so those items and blocks look unchanged beside the pack's. The base supplies the
//! missing block, item and colour-map textures from the newest installed release; the pack's
//! own files always win. Its contents are read in memory during an import, never copied into the
//! repository or shipped.

use super::read_entry;
use std::{
    collections::HashMap,
    io::Cursor,
    path::{Path, PathBuf},
};
use zip::ZipArchive;

/// Environment override: `off` or `0` disables the layer; any other value names the folder to
/// read instead of the platform's default Minecraft folder.
pub(super) const OVERRIDE_VARIABLE: &str = "CINNABAR_JAVA_BASE";

/// Folders of the base that fill gaps, relative to the archive root.
///
/// Entity textures stay out: Java and Bedrock lay many mobs' pictures out differently, so a Java
/// texture is only safe where the translator knows the conversion.
const LAYERED_FOLDERS: [&str; 3] = [
    "assets/minecraft/textures/block/",
    "assets/minecraft/textures/item/",
    "assets/minecraft/textures/colormap/",
];

/// The folder whose presence marks a flattened (1.13 and later) pack or jar.
const FLATTENED_MARKER: &str = "assets/minecraft/textures/block/";

/// The largest client jar the layer reads.
const MAX_JAR_BYTES: u64 = 512 * 1024 * 1024;

/// The most a base texture or its animation file may inflate to.
const MAX_ENTRY_BYTES: u64 = 16 * 1024 * 1024;

/// An opened client jar and its indexed paths.
pub(super) struct JavaBase {
    archive: ZipArchive<Cursor<Vec<u8>>>,
    paths: Vec<(usize, String)>,
}

impl JavaBase {
    /// The base's file at `index`, or `None` when it cannot be read within its limit.
    pub(super) fn read(&mut self, index: usize) -> Option<Vec<u8>> {
        read_entry(&mut self.archive, index, MAX_ENTRY_BYTES).ok()
    }
}

/// Opens the newest installed release, or `None` when the layer is disabled or none is found.
pub(super) fn open() -> Option<JavaBase> {
    let folder = match std::env::var(OVERRIDE_VARIABLE) {
        Ok(value) if matches!(value.trim(), "off" | "0" | "") => return None,
        Ok(value) => PathBuf::from(value),
        Err(_) => default_folder()?,
    };
    open_in(&folder)
}

/// Opens the newest flattened release under `minecraft`'s `versions` folder.
pub(super) fn open_in(minecraft: &Path) -> Option<JavaBase> {
    let mut versions: Vec<(Vec<u32>, PathBuf)> = std::fs::read_dir(minecraft.join("versions"))
        .ok()?
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            let key = version_key(&name)?;
            Some((
                key,
                minecraft
                    .join("versions")
                    .join(&name)
                    .join(format!("{name}.jar")),
            ))
        })
        .collect();
    versions.sort();
    // Newest first, skipping a jar that is unreadable or predates the flattened layout.
    versions
        .into_iter()
        .rev()
        .find_map(|(_, jar)| open_jar(&jar))
}

/// Opens one client jar when it holds flattened block textures.
fn open_jar(jar: &Path) -> Option<JavaBase> {
    if std::fs::metadata(jar).ok()?.len() > MAX_JAR_BYTES {
        return None;
    }
    let mut archive = ZipArchive::new(Cursor::new(std::fs::read(jar).ok()?)).ok()?;
    let mut paths = Vec::new();
    for index in 0..archive.len() {
        let file = archive.by_index_raw(index).ok()?;
        if file.is_file() {
            paths.push((index, file.name().to_owned()));
        }
    }
    paths
        .iter()
        .any(|(_, path)| path.starts_with(FLATTENED_MARKER))
        .then_some(JavaBase { archive, paths })
}

/// A release's numeric version, such as `26.3` or `1.21.4`; snapshots and other names have none.
fn version_key(name: &str) -> Option<Vec<u32>> {
    name.split('.')
        .map(|part| part.parse().ok())
        .collect::<Option<Vec<u32>>>()
        .filter(|key| !key.is_empty())
}

/// The platform's default Minecraft folder.
fn default_folder() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|root| PathBuf::from(root).join(".minecraft"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join("Library/Application Support/minecraft"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".minecraft"))
    }
}

/// The base files a pack lacks, as `path -> index` into the base: only for a flattened pack,
/// since a legacy pack names its folders and textures differently, and never a path the pack
/// itself holds.
pub(super) fn missing_from<'a>(
    pack: &HashMap<String, usize>,
    base_paths: impl IntoIterator<Item = &'a (usize, String)>,
) -> HashMap<String, usize> {
    let flattened = pack.keys().any(|path| {
        path.starts_with(FLATTENED_MARKER) || path.starts_with("assets/minecraft/textures/item/")
    });
    if !flattened {
        return HashMap::new();
    }
    base_paths
        .into_iter()
        .filter(|(_, path)| {
            LAYERED_FOLDERS
                .iter()
                .any(|folder| path.starts_with(folder))
                && (path.ends_with(".png") || path.ends_with(".png.mcmeta"))
                && !pack.contains_key(path)
        })
        .map(|(index, path)| (path.clone(), *index))
        .collect()
}

impl JavaBase {
    /// The base files `pack` lacks; see [`missing_from`].
    pub(super) fn missing_from(&self, pack: &HashMap<String, usize>) -> HashMap<String, usize> {
        missing_from(pack, &self.paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::{ZipWriter, write::SimpleFileOptions};

    /// Writes `versions/<name>/<name>.jar` holding `files` under `minecraft`.
    fn install(minecraft: &Path, name: &str, files: &[&str]) {
        let folder = minecraft.join("versions").join(name);
        std::fs::create_dir_all(&folder).unwrap();
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for file in files {
            writer
                .start_file(*file, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(name.as_bytes()).unwrap();
        }
        let bytes = writer.finish().unwrap().into_inner();
        std::fs::write(folder.join(format!("{name}.jar")), bytes).unwrap();
    }

    const FLAT: &str = "assets/minecraft/textures/block/stone.png";
    const LEGACY: &str = "assets/minecraft/textures/blocks/stone.png";

    #[test]
    fn the_newest_numeric_release_with_flattened_textures_is_read() {
        let directory = tempfile::tempdir().unwrap();
        install(directory.path(), "1.21.4", &[FLAT]);
        install(directory.path(), "26.3", &[FLAT]);
        install(directory.path(), "26.4-snapshot-10", &[FLAT]);
        install(directory.path(), "1.7.10", &[LEGACY]);
        let mut base = open_in(directory.path()).expect("a release is installed");
        let (index, _) = base.paths.iter().find(|(_, path)| path == FLAT).unwrap();
        assert_eq!(base.read(*index).unwrap(), b"26.3", "26.3 outranks 1.21.4");
        let newer_legacy = tempfile::tempdir().unwrap();
        install(newer_legacy.path(), "1.21.4", &[FLAT]);
        install(newer_legacy.path(), "27.0", &[LEGACY]);
        let mut base = open_in(newer_legacy.path()).unwrap();
        let (index, _) = base.paths.iter().find(|(_, path)| path == FLAT).unwrap();
        assert_eq!(
            base.read(*index).unwrap(),
            b"1.21.4",
            "a jar without flattened textures is skipped"
        );
        assert!(open_in(&directory.path().join("absent")).is_none());
    }

    #[test]
    fn a_flattened_pack_gains_only_the_files_it_lacks() {
        let base = [
            (0, "assets/minecraft/textures/item/goat_horn.png".to_owned()),
            (1, "assets/minecraft/textures/item/bucket.png".to_owned()),
            (
                2,
                "assets/minecraft/textures/block/lantern.png.mcmeta".to_owned(),
            ),
            (3, "assets/minecraft/textures/colormap/grass.png".to_owned()),
            (
                4,
                "assets/minecraft/textures/entity/chest/normal.png".to_owned(),
            ),
            (5, "assets/minecraft/lang/en_us.json".to_owned()),
            (6, "assets/minecraft/textures/painting/kebab.png".to_owned()),
            (7, "assets/minecraft/textures/gui/widgets.png".to_owned()),
        ];
        let pack: HashMap<String, usize> = [
            "assets/minecraft/textures/item/bucket.png",
            "assets/minecraft/textures/block/stone.png",
        ]
        .into_iter()
        .enumerate()
        .map(|(index, path)| (path.to_owned(), index))
        .collect();
        let missing = missing_from(&pack, &base);
        let mut names: Vec<_> = missing.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "assets/minecraft/textures/block/lantern.png.mcmeta",
                "assets/minecraft/textures/colormap/grass.png",
                "assets/minecraft/textures/item/goat_horn.png",
            ],
            "the pack's own bucket wins; only block, item and colour-map files are layered"
        );
    }

    #[test]
    fn a_legacy_pack_gains_nothing() {
        let base = [(0, "assets/minecraft/textures/item/goat_horn.png".to_owned())];
        let pack: HashMap<String, usize> =
            [("assets/minecraft/textures/blocks/stone.png".to_owned(), 0)].into();
        assert!(missing_from(&pack, &base).is_empty());
    }
}
