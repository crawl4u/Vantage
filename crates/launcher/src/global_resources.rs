//! Pack-management commands and immutable launcher snapshots.
use resource_pack::{ActivePack, InstalledPack};

/// Commands the settings host can submit without owning pack storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    SelectAvailable(usize),
    SelectActive(usize),
    ReadMore(bool, usize),
    Activate(usize),
    Deactivate(usize),
    MoveUp(usize),
    MoveDown(usize),
    Settings(usize),
    CloseSettings,
    Subpack(usize),
    ToggleAvailable,
    ToggleActive,
    Import,
    Apply,
    /// Ask whether to delete the available pack at this index.
    AskDelete(usize),
    /// Drop the question without deleting.
    CancelDelete,
    /// Delete the available pack at this index and its stored files.
    Delete(usize),
    /// Select the base game's textures, which stay in place under every pack.
    SelectBase,
    /// Move a pack by dragging it. `from_active` says which list it came from and `index` its
    /// place there. `to` is where it was let go: a place in the selected list, or none for the
    /// list of packs not in use.
    Drop {
        from_active: bool,
        index: usize,
        to: Option<usize>,
    },
    /// Open the subfolder at this index of [`Snapshot::subfolders`].
    OpenFolder(usize),
    /// Go up to the folder holding the open one.
    FolderUp,
    /// Make a folder inside the open one.
    NewFolder,
    /// Delete the open folder; its packs move up into the folder that held it.
    DeleteFolder,
    /// Start renaming the open folder in the name field.
    RenameFolder,
    /// Rename the open folder to the name field's text.
    CommitRename,
    /// Stop renaming without a change.
    CancelRename,
    /// Focus the folder name field.
    FolderName,
    /// Put the available pack at `index` in the subfolder at `folder` of
    /// [`Snapshot::subfolders`], or in the folder above the open one for none.
    PutInFolder {
        index: usize,
        folder: Option<usize>,
    },
    /// Move the available pack at this index to the next folder, from the top level through
    /// every folder and back.
    CycleFolder(usize),
    /// Step the order the unselected packs are listed in.
    CycleSort,
    /// Focus the pack search field.
    Search,
    /// Empty the pack search field.
    ClearSearch,
}

/// Immutable view of the staged selection and worker status.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// Generation of the indexed pack lists, independent of presentation updates.
    pub revision: u64,
    pub memory_tier: u32,
    pub icons: std::collections::BTreeMap<(String, u64), String>,
    pub available: Vec<InstalledPack>,
    pub active: Vec<InstalledPack>,
    pub selection: Vec<ActivePack>,
    /// Exact last runtime acknowledgement; absent until initial pack preparation completes.
    pub applied_selection: Option<Vec<ActivePack>>,
    pub selected: Option<(bool, usize)>,
    /// The base game's textures are the selected entry.
    pub base_selected: bool,
    pub details_expanded: Option<(bool, usize)>,
    pub settings: Option<usize>,
    /// The available pack the player was asked to confirm deleting.
    pub delete_pending: Option<usize>,
    /// The order `available` is listed in.
    pub sort: resource_pack::PackSort,
    /// Every folder of the library, as paths of names joined by `/`.
    pub folders: Vec<String>,
    /// The folder each pack of `available` is in, an empty path for the top level.
    pub available_folders: Vec<String>,
    /// The folder the list of packs not in use shows, an empty path for the top level.
    pub folder: String,
    pub available_expanded: bool,
    pub active_expanded: bool,
    pub busy: bool,
    pub message: String,
}

impl Snapshot {
    /// The folders directly inside the open one, as full paths in name order.
    #[must_use]
    pub fn subfolders(&self) -> Vec<&str> {
        let mut found: Vec<&str> = self
            .folders
            .iter()
            .map(String::as_str)
            .filter(|folder| resource_pack::folder_parent(folder) == self.folder)
            .collect();
        found.sort_by_key(|folder| folder.to_lowercase());
        found
    }

    /// Whether the pack of `available` at `index` is in the open folder.
    #[must_use]
    pub fn in_open_folder(&self, index: usize) -> bool {
        // A pack with no filing recorded is at the top level.
        self.available_folders
            .get(index)
            .map_or(self.folder.is_empty(), |folder| *folder == self.folder)
    }

    /// Whether the Apply changes action is enabled for the staged selection.
    pub fn can_apply(&self) -> bool {
        !self.busy && self.has_pending_changes()
    }

    /// Reports edits relative to the last acknowledged runtime stack.
    pub fn has_pending_changes(&self) -> bool {
        self.applied_selection
            .as_ref()
            .is_some_and(|applied| *applied != self.selection)
    }
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            memory_tier: 0,
            icons: Default::default(),
            available: Vec::new(),
            active: Vec::new(),
            selection: Vec::new(),
            applied_selection: None,
            selected: None,
            base_selected: false,
            details_expanded: None,
            settings: None,
            delete_pending: None,
            sort: resource_pack::PackSort::default(),
            folders: Vec::new(),
            available_folders: Vec::new(),
            folder: String::new(),
            available_expanded: true,
            active_expanded: true,
            busy: false,
            message: String::new(),
        }
    }
}
