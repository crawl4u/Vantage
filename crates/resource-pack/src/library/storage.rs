//! Retires immutable archives only after their replacement has been acknowledged.
use super::{
    ActivePack, CATALOG_FILE, Catalog, GlobalPackLibrary, InstalledPack, LibraryError,
    atomic_write, encode_catalog,
};
use std::{fs, path::Path};

/// Drops entries whose archive is gone, as Bedrock ignores a configured pack it cannot find, and
/// names the packs the player loses. A failed save only repeats this on the next open.
pub(super) fn forget_missing(root: &Path, catalog: &mut Catalog) -> Vec<String> {
    let gone = |pack: &InstalledPack| matches!(root.join(pack.filename()).try_exists(), Ok(false));
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
        match fs::remove_file(root.join(pack.filename())) {
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
        let doomed: Vec<InstalledPack> = candidate
            .available
            .iter()
            .chain(&candidate.retained)
            .filter(|pack| pack.id == id)
            .cloned()
            .collect();
        candidate.available.retain(|pack| pack.id != id);
        candidate.retained.retain(|pack| pack.id != id);
        candidate.placement.remove(&id);
        // The catalog stops naming the pack first, so a failed deletion leaves only an orphan file.
        atomic_write(&self.root.join(CATALOG_FILE), &encode_catalog(&candidate)?)?;
        self.catalog = candidate;
        let mut failure = None;
        for pack in doomed {
            match fs::remove_file(self.root.join(pack.filename())) {
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
    name.chars()
        .filter(|character| !matches!(*character, '/' | '\\') && !character.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(MAX_FOLDER_NAME_CHARS)
        .collect()
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
        let mut candidate = self.catalog.clone();
        candidate.folders.push(path.clone());
        candidate.folders.sort();
        self.save_catalog(candidate)?;
        Ok(path)
    }

    /// Deletes the folder at `path` and the folders inside it. Their packs are not deleted; they
    /// move up into the folder that held `path`.
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
        self.save_catalog(candidate)
    }

    /// Puts the pack `id` in the folder at `folder`, or at the top level for an empty path.
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
        self.save_catalog(candidate)
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
        let lost = forget_missing(&self.root, &mut self.catalog);
        if lost.is_empty() {
            return lost;
        }
        let installed: Vec<uuid::Uuid> = self.catalog.available.iter().map(|pack| pack.id).collect();
        self.active.retain(|active| installed.contains(&active.id));
        self.catalog.placement.retain(|id, _| installed.contains(id));
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