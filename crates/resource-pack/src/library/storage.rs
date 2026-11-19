//! Retires immutable archives only after their replacement has been acknowledged.
use super::{
    ActivePack, CATALOG_FILE, Catalog, GlobalPackLibrary, InstalledPack, LibraryError,
    atomic_write, encode_catalog,
};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

/// How deep inside the library's folder folders and archives are looked for.
pub const SCAN_DEPTH: usize = 4;

/// Folders of the library's own that are not pack folders.
const KEPT_FOLDERS: [&str; 2] = ["imported", "icons"];

/// The directory of the folder at `folder` inside `root`; `root` itself for the top level.
pub(super) fn folder_dir(root: &Path, folder: &str) -> PathBuf {
    folder
        .split('/')
        .filter(|name| !name.is_empty())
        .fold(root.to_path_buf(), |path, name| path.join(name))
}

/// Where `pack`'s archive is: in the folder its pack is filed in.
pub(super) fn archive_in(root: &Path, catalog: &Catalog, pack: &InstalledPack) -> PathBuf {
    let folder = catalog.placement.get(&pack.id).map_or("", String::as_str);
    folder_dir(root, folder).join(pack.filename())
}

/// Moves a library that kept every archive in one folder into the layout where each archive sits
/// in the folder its pack is filed in, making the folders as it goes.
pub(super) fn migrate(root: &Path, catalog: &mut Catalog) {
    if catalog.layout >= 1 {
        return;
    }
    for folder in &catalog.folders {
        let _ = fs::create_dir_all(folder_dir(root, folder));
    }
    for pack in catalog.available.iter().chain(&catalog.retained) {
        let target = archive_in(root, catalog, pack);
        let old = root.join(pack.filename());
        if target != old && !target.exists() && old.exists() {
            if let Some(parent) = target.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::rename(&old, &target);
        }
    }
    catalog.layout = 1;
    if let Ok(bytes) = encode_catalog(catalog) {
        let _ = atomic_write(&root.join(CATALOG_FILE), &bytes);
    }
}

/// Every pack archive under `root`, by file name, with the folder holding it.
fn find_archives(root: &Path) -> HashMap<String, String> {
    fn walk(dir: &Path, relative: &str, depth: usize, found: &mut HashMap<String, String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                let skipped = name.starts_with('.')
                    || (relative.is_empty() && KEPT_FOLDERS.contains(&name.as_str()))
                    || depth >= SCAN_DEPTH;
                if !skipped {
                    let inner = if relative.is_empty() {
                        name
                    } else {
                        format!("{relative}/{name}")
                    };
                    walk(&entry.path(), &inner, depth + 1, found);
                }
            } else if name.ends_with(".mcpack") {
                found.entry(name).or_insert_with(|| relative.to_owned());
            }
        }
    }
    let mut found = HashMap::new();
    walk(root, "", 0, &mut found);
    found
}

/// Follows archives moved by hand to another folder: a pack whose archive is not where its
/// folder says, but is in another folder, is filed there now. Returns whether anything moved.
pub(super) fn relocate(root: &Path, catalog: &mut Catalog) -> bool {
    let missing = |catalog: &Catalog, pack: &InstalledPack| {
        matches!(archive_in(root, catalog, pack).try_exists(), Ok(false))
    };
    if !catalog
        .available
        .iter()
        .chain(&catalog.retained)
        .any(|pack| missing(catalog, pack))
    {
        return false;
    }
    let located = find_archives(root);
    let mut moves = Vec::new();
    for pack in catalog.available.iter().chain(&catalog.retained) {
        if missing(catalog, pack)
            && let Some(folder) = located.get(&pack.filename())
        {
            moves.push((pack.id, folder.clone()));
        }
    }
    let changed = !moves.is_empty();
    for (id, folder) in moves {
        let mut current = String::new();
        for name in folder.split('/').filter(|name| !name.is_empty()) {
            if !current.is_empty() {
                current.push('/');
            }
            current.push_str(name);
            if !catalog.folders.contains(&current) {
                catalog.folders.push(current.clone());
            }
        }
        if folder.is_empty() {
            catalog.placement.remove(&id);
        } else {
            catalog.placement.insert(id, folder);
        }
    }
    if changed {
        catalog.folders.sort();
        if let Ok(bytes) = encode_catalog(catalog) {
            let _ = atomic_write(&root.join(CATALOG_FILE), &bytes);
        }
    }
    changed
}

/// Drops folders whose directory was deleted, and those inside them. Returns no names: the packs
/// that were in them are noticed missing, or found elsewhere, afterwards.
pub(super) fn forget_folders(root: &Path, catalog: &mut Catalog) -> Vec<String> {
    let before = catalog.folders.len();
    let gone: Vec<String> = catalog
        .folders
        .iter()
        .filter(|folder| !folder_dir(root, folder).is_dir())
        .cloned()
        .collect();
    catalog.folders.retain(|folder| {
        !gone
            .iter()
            .any(|dead| folder == dead || folder.starts_with(&format!("{dead}/")))
    });
    if catalog.folders.len() != before
        && let Ok(bytes) = encode_catalog(catalog)
    {
        let _ = atomic_write(&root.join(CATALOG_FILE), &bytes);
    }
    Vec::new()
}

/// Drops entries whose archive is gone, as Bedrock ignores a configured pack it cannot find, and
/// names the packs the player loses. A failed save only repeats this on the next open.
pub(super) fn forget_missing(root: &Path, catalog: &mut Catalog) -> Vec<String> {
    let placement = catalog.placement.clone();
    let gone = |pack: &InstalledPack| {
        let folder = placement.get(&pack.id).map_or("", String::as_str);
        matches!(
            folder_dir(root, folder).join(pack.filename()).try_exists(),
            Ok(false)
        )
    };
    let selects = |active: &ActivePack, pack: &InstalledPack| {
        active.id == pack.id && active.revision == pack.revision
    };
    let lost: Vec<InstalledPack> = catalog
        .available
        .iter()
        .chain(&catalog.retained)
        .filter(|pack| gone(pack))
        .cloned()
        .collect();
    if lost.is_empty() {
        return Vec::new();
    }
    let mut names: Vec<String> = lost
        .iter()
        .filter(|pack| {
            catalog.available.contains(pack)
                || catalog.active.iter().any(|active| selects(active, pack))
        })
        .map(|pack| pack.name.clone())
        .collect();
    names.sort();
    names.dedup();
    catalog.available.retain(|pack| !lost.contains(pack));
    catalog.retained.retain(|pack| !lost.contains(pack));
    let installed = |active: &ActivePack| !lost.iter().any(|pack| selects(active, pack));
    catalog.active.retain(installed);
    catalog.restore.retain(installed);
    if let Ok(bytes) = encode_catalog(catalog) {
        let _ = atomic_write(&root.join(CATALOG_FILE), &bytes);
    }
    names
}

/// Removes obsolete revisions after the newest preview has been acknowledged.
pub(super) fn prune(root: &Path, catalog: &mut Catalog) -> Result<(), LibraryError> {
    let mut candidate = catalog.clone();
    let mut failure = None;
    candidate.retained.retain(|pack| {
        if candidate
            .active
            .iter()
            .chain(&candidate.restore)
            .any(|active| active.id == pack.id && active.revision == pack.revision)
        {
            return true;
        }
        match fs::remove_file(archive_in(root, catalog, pack)) {
            Ok(()) => false,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => {
                failure.get_or_insert(error);
                true
            }
        }
    });
    atomic_write(&root.join(CATALOG_FILE), &encode_catalog(&candidate)?)?;
    *catalog = candidate;
    match failure {
        Some(error) => Err(error.into()),
        None => Ok(()),
    }
}

impl GlobalPackLibrary {
    /// Preserves a durable restore selection's archive revisions until a new backup replaces it.
    pub fn retain_for_restore(&mut self, selection: &[ActivePack]) -> Result<(), LibraryError> {
        if selection.len() > crate::MAX_PACKS {
            return Err(crate::AdmissionError::TooManyPacks.into());
        }
        if self.catalog.restore == selection {
            return Ok(());
        }
        let previous = std::mem::replace(&mut self.catalog.restore, selection.to_vec());
        if let Err(error) = self.persist() {
            self.catalog.restore = previous;
            return Err(error);
        }
        Ok(())
    }
}

impl GlobalPackLibrary {
    /// Deletes an imported pack and every stored revision of it. A pack the player has selected,
    /// staged or applied, or that a restore point keeps, stays until it is deselected and the
    /// change applied.
    pub fn remove(&mut self, id: uuid::Uuid) -> Result<(), LibraryError> {
        let selected = self
            .active
            .iter()
            .chain(&self.catalog.active)
            .chain(&self.catalog.restore)
            .any(|active| active.id == id);
        if selected {
            return Err(LibraryError::PackInUse);
        }
        if !self.catalog.available.iter().any(|pack| pack.id == id) {
            return Err(LibraryError::UnknownPack);
        }
        let mut candidate = self.catalog.clone();
        let doomed: Vec<PathBuf> = candidate
            .available
            .iter()
            .chain(&candidate.retained)
            .filter(|pack| pack.id == id)
            .map(|pack| archive_in(&self.root, &candidate, pack))
            .collect();
        candidate.available.retain(|pack| pack.id != id);
        candidate.retained.retain(|pack| pack.id != id);
        candidate.placement.remove(&id);
        // The catalog stops naming the pack first, so a failed deletion leaves only an orphan file.
        atomic_write(&self.root.join(CATALOG_FILE), &encode_catalog(&candidate)?)?;
        self.catalog = candidate;
        let mut failure = None;
        for path in doomed {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        match failure {
            Some(error) => Err(error.into()),
            None => Ok(()),
        }
    }
}

/// The longest a folder's name may be, in characters.
pub const MAX_FOLDER_NAME_CHARS: usize = 48;

/// The most folders a library keeps.
pub const MAX_FOLDERS: usize = 256;

/// The parent of the folder at `path`: the folder holding it, or the top level (an empty path).
#[must_use]
pub fn folder_parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// The last part of `path`, the folder's own name.
#[must_use]
pub fn folder_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// `name` as a folder name: trimmed, without slashes or control characters, and not too long.
fn clean_folder_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|character| {
            !matches!(
                *character,
                '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*'
            ) && !character.is_control()
        })
        .collect::<String>()
        .trim()
        .chars()
        .take(MAX_FOLDER_NAME_CHARS)
        .collect();
    // A name may not end in a dot or space, and a few are reserved, on Windows.
    let cleaned = cleaned.trim_end_matches(['.', ' ']).to_owned();
    let reserved = ["CON", "PRN", "AUX", "NUL"];
    let stem = cleaned.split('.').next().unwrap_or("").to_ascii_uppercase();
    let numbered = stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && stem.ends_with(|digit: char| digit.is_ascii_digit());
    if reserved.contains(&stem.as_str()) || numbered {
        format!("{cleaned} folder")
    } else {
        cleaned
    }
}

impl GlobalPackLibrary {
    /// Every folder, as paths of names joined by `/`. Folders only organise the library: a pack
    /// in one is installed, selected and applied exactly as one outside any.
    pub fn folders(&self) -> &[String] {
        &self.catalog.folders
    }

    /// The folder holding the pack `id`, an empty path for the top level.
    pub fn folder_of(&self, id: uuid::Uuid) -> &str {
        self.catalog.placement.get(&id).map_or("", String::as_str)
    }

    /// Makes a folder named `name` inside `parent` and returns its path. A name already used
    /// there is numbered, and an empty one is called "New Folder".
    pub fn create_folder(&mut self, parent: &str, name: &str) -> Result<String, LibraryError> {
        if !parent.is_empty() && !self.catalog.folders.iter().any(|folder| folder == parent) {
            return Err(LibraryError::UnknownFolder);
        }
        if self.catalog.folders.len() >= MAX_FOLDERS {
            return Err(LibraryError::TooManyFolders);
        }
        let base = match clean_folder_name(name) {
            name if name.is_empty() => "New Folder".to_owned(),
            name => name,
        };
        let join = |name: &str| {
            if parent.is_empty() {
                name.to_owned()
            } else {
                format!("{parent}/{name}")
            }
        };
        let mut path = join(&base);
        let mut number = 2;
        while self.catalog.folders.contains(&path) {
            path = join(&format!("{base} {number}"));
            number += 1;
        }
        fs::create_dir_all(folder_dir(&self.root, &path))?;
        let mut candidate = self.catalog.clone();
        candidate.folders.push(path.clone());
        candidate.folders.sort();
        self.save_catalog(candidate)?;
        Ok(path)
    }

    /// Deletes the folder at `path` and the folders inside it. Their packs are not deleted; they
    /// move up into the folder that held `path`, archives and all. A folder with something else
    /// in it stays on disk, and shows again as a folder.
    pub fn delete_folder(&mut self, path: &str) -> Result<(), LibraryError> {
        if !self.catalog.folders.iter().any(|folder| folder == path) {
            return Err(LibraryError::UnknownFolder);
        }
        let inside = |folder: &str| folder == path || folder.starts_with(&format!("{path}/"));
        let parent = folder_parent(path).to_owned();
        let mut candidate = self.catalog.clone();
        candidate.folders.retain(|folder| !inside(folder));
        for place in candidate.placement.values_mut() {
            if inside(place) {
                place.clone_from(&parent);
            }
        }
        candidate.placement.retain(|_, place| !place.is_empty());
        let mut doomed: Vec<String> = self
            .catalog
            .folders
            .iter()
            .filter(|folder| inside(folder))
            .cloned()
            .collect();
        self.rehome(candidate)?;
        // The deepest folders go first, so each is empty by the time it is removed.
        doomed.sort_by_key(|folder| std::cmp::Reverse(folder.len()));
        let mut stuck = false;
        for folder in doomed {
            stuck |= fs::remove_dir(folder_dir(&self.root, &folder)).is_err();
        }
        if stuck {
            return Err(LibraryError::FolderNotEmpty);
        }
        Ok(())
    }

    /// Puts the pack `id` in the folder at `folder`, or at the top level for an empty path. Its
    /// archives move into that folder on disk.
    pub fn place_pack(&mut self, id: uuid::Uuid, folder: &str) -> Result<(), LibraryError> {
        if !folder.is_empty() && !self.catalog.folders.iter().any(|known| known == folder) {
            return Err(LibraryError::UnknownFolder);
        }
        if !self.catalog.available.iter().any(|pack| pack.id == id) {
            return Err(LibraryError::UnknownPack);
        }
        let mut candidate = self.catalog.clone();
        if folder.is_empty() {
            candidate.placement.remove(&id);
        } else {
            candidate.placement.insert(id, folder.to_owned());
        }
        self.rehome(candidate)
    }

    /// Renames the folder at `path` to `name`, on disk and in the library, with everything in it
    /// following. Returns the new path. A name another folder beside it has is refused.
    pub fn rename_folder(&mut self, path: &str, name: &str) -> Result<String, LibraryError> {
        if !self.catalog.folders.iter().any(|folder| folder == path) {
            return Err(LibraryError::UnknownFolder);
        }
        let name = clean_folder_name(name);
        if name.is_empty() {
            return Err(LibraryError::InvalidFolderName);
        }
        let parent = folder_parent(path);
        let renamed = if parent.is_empty() {
            name.clone()
        } else {
            format!("{parent}/{name}")
        };
        if renamed == path {
            return Ok(renamed);
        }
        let taken = self
            .catalog
            .folders
            .iter()
            .any(|folder| folder != path && folder.eq_ignore_ascii_case(&renamed));
        if taken {
            return Err(LibraryError::FolderExists);
        }
        let moved = |folder: &str| -> Option<String> {
            if folder == path {
                Some(renamed.clone())
            } else {
                folder
                    .strip_prefix(&format!("{path}/"))
                    .map(|rest| format!("{renamed}/{rest}"))
            }
        };
        let mut candidate = self.catalog.clone();
        for folder in &mut candidate.folders {
            if let Some(new) = moved(folder) {
                *folder = new;
            }
        }
        for place in candidate.placement.values_mut() {
            if let Some(new) = moved(place) {
                *place = new;
            }
        }
        candidate.folders.sort();
        let (from, to) = (
            folder_dir(&self.root, path),
            folder_dir(&self.root, &renamed),
        );
        fs::rename(&from, &to)?;
        if let Err(error) = self.save_catalog(candidate) {
            let _ = fs::rename(&to, &from);
            return Err(error);
        }
        Ok(renamed)
    }

    /// Moves every archive to where `candidate`'s filing puts it, then keeps `candidate`. A
    /// failure puts the archives that moved back.
    fn rehome(&mut self, candidate: Catalog) -> Result<(), LibraryError> {
        let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
        let mut failure = None;
        for pack in self.catalog.available.iter().chain(&self.catalog.retained) {
            let from = archive_in(&self.root, &self.catalog, pack);
            let to = archive_in(&self.root, &candidate, pack);
            if from == to || !from.exists() {
                continue;
            }
            let moved = to
                .parent()
                .map_or(Ok(()), fs::create_dir_all)
                .and_then(|()| fs::rename(&from, &to));
            match moved {
                Ok(()) => done.push((from, to)),
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            }
        }
        let saved = match failure {
            Some(error) => Err(LibraryError::from(error)),
            None => self.save_catalog(candidate),
        };
        if saved.is_err() {
            for (from, to) in done.into_iter().rev() {
                let _ = fs::rename(to, from);
            }
        }
        saved
    }

    /// Writes `candidate` and keeps it, or keeps the catalog as it was when writing fails.
    fn save_catalog(&mut self, candidate: Catalog) -> Result<(), LibraryError> {
        atomic_write(&self.root.join(CATALOG_FILE), &encode_catalog(&candidate)?)?;
        self.catalog = candidate;
        Ok(())
    }
}

impl GlobalPackLibrary {
    /// The folder the library keeps its archives in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Whether `file_name`, a file directly in the library's folder, is an archive the library
    /// wrote and keeps, as opposed to a pack someone put there to be imported.
    pub fn manages(&self, file_name: &str) -> bool {
        self.catalog
            .available
            .iter()
            .chain(&self.catalog.retained)
            .any(|pack| pack.filename() == file_name)
    }

    /// Notices archives deleted from the library's folder: those packs leave the library and any
    /// selection, as they do when it opens. Returns the names of the packs that went.
    pub fn reconcile(&mut self) -> Vec<String> {
        let folders = self.catalog.folders.len();
        forget_folders(&self.root, &mut self.catalog);
        relocate(&self.root, &mut self.catalog);
        let lost = forget_missing(&self.root, &mut self.catalog);
        if lost.is_empty() {
            // Folders that vanished from disk, or packs found in another, are changes too.
            if self.catalog.folders.len() != folders {
                self.catalog
                    .placement
                    .retain(|_, folder| self.catalog.folders.contains(folder));
            }
            return lost;
        }
        let installed: Vec<uuid::Uuid> =
            self.catalog.available.iter().map(|pack| pack.id).collect();
        self.active.retain(|active| installed.contains(&active.id));
        self.catalog
            .placement
            .retain(|id, _| installed.contains(id));
        lost
    }

    /// Makes the folder at `path`, and those above it, when they are not there yet.
    pub fn ensure_folder(&mut self, path: &str) -> Result<(), LibraryError> {
        let mut candidate = self.catalog.clone();
        let mut current = String::new();
        for name in path.split('/').filter(|name| !name.is_empty()) {
            if !current.is_empty() {
                current.push('/');
            }
            current.push_str(&clean_folder_name(name));
            if !candidate.folders.contains(&current) {
                if candidate.folders.len() >= MAX_FOLDERS {
                    return Err(LibraryError::TooManyFolders);
                }
                candidate.folders.push(current.clone());
            }
        }
        if candidate.folders.len() == self.catalog.folders.len() {
            return Ok(());
        }
        candidate.folders.sort();
        self.save_catalog(candidate)
    }
}
