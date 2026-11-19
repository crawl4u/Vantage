//! Serial disk work keeps staged selection and import results in submission order.

use bevy::prelude::Resource;
use launcher::global_resources::{Action, Snapshot};
use launcher_host::desktop::picker::{self, Filter};
use resource_pack::{
    GlobalPackLibrary, LibraryError, PACK_IMPORT_EXTENSIONS, PackSort, ValidatedPackStack,
};
use std::{path::PathBuf, sync::Arc};

const PACKS: Filter = Filter {
    title: "Import resource pack",
    name: "Resource packs",
    extensions: &PACK_IMPORT_EXTENSIONS,
    unavailable_hint: "Drop a pack onto the window to import it.",
    max_bytes: Some(resource_pack::MAX_ARCHIVE_BYTES as u64),
};

pub(super) enum Command {
    MinecraftImport(Box<launcher_host::minecraft_import::Request>),
    Action { action: Action, revision: u64 },
    Import(PathBuf),
    Commit(Vec<resource_pack::ActivePack>),
    /// Rename the open folder to this name.
    RenameFolder(String),
}
pub(super) enum Event {
    MinecraftImportStatus(String),
    MinecraftImport(
        launcher::minecraft_import::Snapshot,
        Option<Box<launcher_host::minecraft_import::UserState>>,
    ),
    Snapshot(Snapshot),
    Apply(
        Arc<ValidatedPackStack>,
        Vec<resource_pack::ActivePack>,
        ApplyOrigin,
    ),
}

/// Identifies whether a pack publication came from persisted startup state or a user action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ApplyOrigin {
    Startup,
    User,
}

#[derive(Resource)]
pub(super) struct Worker {
    commands: crossbeam_channel::Sender<Command>,
    pub events: crossbeam_channel::Receiver<Event>,
    pub snapshot: Snapshot,
    pub awaiting: Option<(u64, Vec<resource_pack::ActivePack>)>,
    pub minecraft_import_pending: bool,
    /// The menu admits one request until the worker publishes its result.
    pub minecraft_import_request: Option<launcher::minecraft_import::Action>,
}

impl Worker {
    /// Starts one worker, shared by initial imports, file drops and settings actions.
    pub fn new(root: PathBuf, config_root: PathBuf, files: Vec<PathBuf>) -> Self {
        let (commands, incoming) = crossbeam_channel::unbounded();
        let (outgoing, events) = crossbeam_channel::unbounded();
        std::thread::spawn(move || run(root, config_root, files, incoming, outgoing));
        Self {
            commands,
            events,
            snapshot: Snapshot::default(),
            awaiting: None,
            minecraft_import_pending: false,
            minecraft_import_request: None,
        }
    }

    /// Queues a command without blocking the frame.
    pub fn send(&mut self, command: Command) {
        let _ = self.commands.send(command);
    }

    /// Reports whether the explicit import request reached the serial worker.
    pub fn send_minecraft_import(
        &mut self,
        request: launcher_host::minecraft_import::Request,
    ) -> bool {
        let action = request.action;
        let sent = self
            .commands
            .send(Command::MinecraftImport(Box::new(request)))
            .is_ok();
        if sent {
            self.minecraft_import_request = Some(action);
        }
        sent
    }
}

/// Opens storage and processes commands until the app drops the sender.
fn run(
    root: PathBuf,
    config_root: PathBuf,
    files: Vec<PathBuf>,
    incoming: crossbeam_channel::Receiver<Command>,
    outgoing: crossbeam_channel::Sender<Event>,
) {
    let mut minecraft_import = launcher_host::minecraft_import::Service::new(config_root);
    let mut import_commit_pending = None;
    let mut snapshot = Snapshot::default();
    let icon_root = root.join("icons");
    let mut library = match GlobalPackLibrary::open(root, engine_version()) {
        Ok(library) => library,
        Err(error) => {
            snapshot.message = error.to_string();
            let _ = outgoing.send(Event::Snapshot(snapshot));
            return;
        }
    };
    library.set_device_memory(launcher_host::memory::physical_bytes());
    super::icons::refresh(&library, &mut snapshot, &icon_root);
    // Outlives the startup apply, whose acknowledgement would otherwise clear it unseen.
    let mut missing_notice = (!library.missing_packs().is_empty()).then(|| {
        format!(
            "These packs' files are missing, so they were removed: {}. Import them again to use them.",
            library.missing_packs().join(", ")
        )
    });
    snapshot.message = missing_notice.clone().unwrap_or_default();
    match library.preview() {
        Ok(stack) => {
            let _ = outgoing.send(Event::Apply(
                stack,
                library.active().to_vec(),
                ApplyOrigin::Startup,
            ));
        }
        Err(error) => snapshot.message = error.to_string(),
    }
    refresh(&library, &mut snapshot);
    let _ = outgoing.send(Event::Snapshot(snapshot.clone()));
    let mut initial: std::collections::VecDeque<Command> =
        files.into_iter().map(Command::Import).collect();
    let mut watch = Watch::default();
    loop {
        let command = match initial.pop_front() {
            Some(command) => command,
            None => match incoming.recv_timeout(SCAN_INTERVAL) {
                Ok(command) => command,
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    // Between commands the library's folder is checked for packs put in or taken
                    // out by hand.
                    let (changed, selection) = scan_folder(&mut library, &mut snapshot, &mut watch);
                    if changed {
                        snapshot.revision += 1;
                        super::icons::refresh(&library, &mut snapshot, &icon_root);
                        refresh(&library, &mut snapshot);
                        if selection {
                            match library.preview() {
                                Ok(stack) => {
                                    let _ = outgoing.send(Event::Apply(
                                        stack,
                                        library.active().to_vec(),
                                        ApplyOrigin::User,
                                    ));
                                }
                                Err(error) => snapshot.message = error.to_string(),
                            }
                        }
                        let _ = outgoing.send(Event::Snapshot(snapshot.clone()));
                    }
                    continue;
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
            },
        };
        if let Command::Action { action, revision } = &command
            && indexed(*action)
            && *revision != snapshot.revision
        {
            snapshot.message = "Resource pack list changed; select the pack again.".into();
            let _ = outgoing.send(Event::Snapshot(snapshot.clone()));
            continue;
        }
        let mut changes_lists = matches!(
            &command,
            Command::MinecraftImport(_)
                | Command::Import(_)
                | Command::RenameFolder(_)
                | Command::Action {
                    action: Action::Activate(_)
                        | Action::Deactivate(_)
                        | Action::MoveUp(_)
                        | Action::MoveDown(_)
                        | Action::Subpack(_)
                        | Action::Import
                        | Action::Delete(_)
                        | Action::Drop { .. }
                        | Action::NewFolder
                        | Action::DeleteFolder
                        | Action::PutInFolder { .. }
                        | Action::CycleFolder(_)
                        | Action::CycleSort,
                    ..
                }
        );
        if let Command::Commit(selection) = &command {
            snapshot.applied_selection = Some(selection.clone());
        }
        let performs_work = matches!(
            &command,
            Command::MinecraftImport(_)
                | Command::Import(_)
                | Command::Commit(_)
                | Command::Action {
                    action: Action::Import | Action::Apply,
                    ..
                }
        );
        if performs_work {
            snapshot.busy = true;
            snapshot.message = "Updating resource packs…".into();
            let _ = outgoing.send(Event::Snapshot(snapshot.clone()));
        }
        let refresh_icons = |library: &GlobalPackLibrary, snapshot: &mut Snapshot| {
            super::icons::refresh(library, snapshot, &icon_root);
        };
        let result = match command {
            Command::MinecraftImport(request) => {
                let (outcome, changed) = import_minecraft_data(
                    &mut minecraft_import,
                    *request,
                    &mut library,
                    &mut snapshot,
                    refresh_icons,
                );
                changes_lists = changed;
                if let Some(stack) = outcome.stack {
                    import_commit_pending = Some(library.active().to_vec());
                    let _ = outgoing.send(Event::Apply(
                        stack,
                        library.active().to_vec(),
                        ApplyOrigin::User,
                    ));
                }
                let _ = outgoing.send(Event::MinecraftImport(
                    outcome.snapshot,
                    outcome.state.map(Box::new),
                ));
                snapshot.message.clear();
                Ok(())
            }
            Command::Commit(selection) => {
                let result = commit_selection(
                    &mut library,
                    &selection,
                    &mut import_commit_pending,
                    &outgoing,
                );
                snapshot.message = missing_notice.take().unwrap_or_default();
                result
            }
            Command::Import(path) => {
                import_files(&mut library, vec![path], &mut snapshot, refresh_icons)
            }
            Command::RenameFolder(name) => {
                let open = snapshot.folder.clone();
                library.rename_folder(&open, &name).map(|renamed| {
                    snapshot.folder = renamed;
                    snapshot.message.clear();
                })
            }
            Command::Action {
                action: Action::Import,
                ..
            } => match picker::pick_files(&PACKS) {
                Ok(paths) => import_files(
                    &mut library,
                    paths
                        .iter()
                        .map(|file| file.as_ref().to_path_buf())
                        .collect(),
                    &mut snapshot,
                    refresh_icons,
                ),
                Err(error) => {
                    snapshot.message = error;
                    Ok(())
                }
            },
            Command::Action { action, .. } => act(&mut library, &mut snapshot, action, &outgoing),
        };
        if let Err(error) = result {
            snapshot.message = error.to_string();
        }
        snapshot.busy = false;
        if changes_lists {
            snapshot.revision += 1;
        }
        refresh(&library, &mut snapshot);
        let _ = outgoing.send(Event::Snapshot(snapshot.clone()));
    }
}

/// How often the library's folder is checked for packs put in or taken out by hand.
const SCAN_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1500);

/// Subfolders of the library's folder the scan leaves alone: where imported originals are kept,
/// and the pack icons.
const KEPT_FOLDERS: [&str; 2] = ["imported", "icons"];

/// How deep inside the library's folder the scan looks for packs and folders.
const SCAN_DEPTH: usize = 4;

/// What the scan of the library's folder remembers between checks.
#[derive(Default)]
struct Watch {
    /// A file's size and time when last seen, so a file still being copied in waits a round.
    seen: std::collections::HashMap<PathBuf, (u64, Option<std::time::SystemTime>)>,
    /// Files that failed to import as they were, which are not tried again until they change.
    rejected: std::collections::HashMap<PathBuf, (u64, Option<std::time::SystemTime>)>,
}

/// The packs and folders found under `dir`, as paths with the folder each is in, relative to the
/// library's folder.
fn scan_tree(
    library: &GlobalPackLibrary,
    dir: &std::path::Path,
    relative: &str,
    depth: usize,
    packs: &mut Vec<(PathBuf, String)>,
    folders: &mut Vec<String>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            let hidden = name.starts_with('.');
            let kept = relative.is_empty() && KEPT_FOLDERS.contains(&name.as_str());
            if hidden || kept || depth >= SCAN_DEPTH {
                continue;
            }
            let inner = if relative.is_empty() {
                name.clone()
            } else {
                format!("{relative}/{name}")
            };
            folders.push(inner.clone());
            scan_tree(library, &path, &inner, depth + 1, packs, folders);
        } else if kind.is_file()
            && resource_pack::is_pack_import_path(&path)
            && !library.manages(&name)
        {
            packs.push((path, relative.to_owned()));
        }
    }
}

/// Checks the library's folder: archives deleted from it leave the library, packs put into it
/// are imported at once, into the folder they were put in, and folders made in it appear as
/// folders. The originals are kept in `imported`. Returns whether the lists changed and whether
/// the selection did.
fn scan_folder(
    library: &mut GlobalPackLibrary,
    snapshot: &mut Snapshot,
    watch: &mut Watch,
) -> (bool, bool) {
    let mut changed = false;
    let before = library.active().len();
    let layout = |library: &GlobalPackLibrary| -> (Vec<String>, Vec<String>) {
        (
            library.folders().to_vec(),
            library
                .available()
                .iter()
                .map(|pack| library.folder_of(pack.id).to_owned())
                .collect(),
        )
    };
    let arranged = layout(library);
    let lost = library.reconcile();
    let selection = library.active().len() != before;
    if !lost.is_empty() {
        snapshot.message = format!("Removed {}.", lost.join(", "));
        changed = true;
    }
    // Folders made, deleted, renamed or filled by hand show up as they are.
    changed |= layout(library) != arranged;
    let root = library.root().to_path_buf();
    let (mut packs, mut folders) = (Vec::new(), Vec::new());
    scan_tree(library, &root, "", 0, &mut packs, &mut folders);
    for folder in folders {
        let known = library.folders().contains(&folder);
        if !known && library.ensure_folder(&folder).is_ok() {
            changed = true;
        }
    }
    packs.retain(|(path, _)| {
        let Ok(meta) = std::fs::metadata(path) else {
            return false;
        };
        let stamp = (meta.len(), meta.modified().ok());
        if watch.rejected.get(path) == Some(&stamp) {
            return false;
        }
        // Wait for a file to stop changing before reading it.
        if watch.seen.get(path) != Some(&stamp) {
            watch.seen.insert(path.clone(), stamp);
            return false;
        }
        true
    });
    for (path, folder) in packs {
        let stamp = std::fs::metadata(&path)
            .ok()
            .map(|meta| (meta.len(), meta.modified().ok()));
        let label = path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        match library.import(&path) {
            Ok(report) if !report.imported.is_empty() => {
                if !folder.is_empty() {
                    let _ = library.ensure_folder(&folder);
                    for pack in &report.imported {
                        let _ = library.place_pack(pack.id, &folder);
                    }
                }
                snapshot.message = format!(
                    "Imported {}.",
                    report
                        .imported
                        .iter()
                        .map(|pack| pack.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                watch.seen.remove(&path);
                if !keep_original(&root, &path) {
                    watch.rejected.extend(stamp.map(|stamp| (path.clone(), stamp)));
                }
                changed = true;
            }
            result => {
                watch.rejected.extend(stamp.map(|stamp| (path.clone(), stamp)));
                snapshot.message = match result {
                    Err(error) => format!("{label}: {error}"),
                    _ => format!("{label} has no resource pack in it."),
                };
                changed = true;
            }
        }
    }
    (changed, selection)
}

/// Moves an imported original into the library's `imported` folder, under a free name; whether
/// it was moved.
fn keep_original(root: &std::path::Path, path: &std::path::Path) -> bool {
    let kept = root.join(KEPT_FOLDERS[0]);
    if std::fs::create_dir_all(&kept).is_err() {
        return false;
    }
    let name = path.file_name().map_or_else(
        || std::ffi::OsString::from("pack"),
        std::ffi::OsStr::to_os_string,
    );
    let mut target = kept.join(&name);
    let mut number = 2;
    while target.exists() {
        target = kept.join(format!("{number}-{}", name.to_string_lossy()));
        number += 1;
    }
    std::fs::rename(path, target).is_ok()
}

/// Reports an import acknowledgement against its submitted selection even after staging changes.
fn commit_selection(
    library: &mut GlobalPackLibrary,
    selection: &[resource_pack::ActivePack],
    pending: &mut Option<Vec<resource_pack::ActivePack>>,
    outgoing: &crossbeam_channel::Sender<Event>,
) -> Result<(), LibraryError> {
    let result = library.commit_selection(selection);
    if pending.as_deref() == Some(selection) {
        *pending = None;
        let status = match &result {
            Ok(()) => "Resource packs applied and saved.".to_owned(),
            Err(error) => format!("Resource pack preferences could not be saved: {error}"),
        };
        let _ = outgoing.send(Event::MinecraftImportStatus(status));
    }
    result
}

/// Returns the import result and whether the pack library changed, refreshing artwork only then.
fn import_minecraft_data(
    service: &mut launcher_host::minecraft_import::Service,
    request: launcher_host::minecraft_import::Request,
    library: &mut GlobalPackLibrary,
    snapshot: &mut Snapshot,
    refresh_icons: impl FnOnce(&GlobalPackLibrary, &mut Snapshot),
) -> (launcher_host::minecraft_import::Outcome, bool) {
    let catalog = library.available().to_vec();
    let selection = library.active().to_vec();
    let outcome = service.act(request, library);
    let changed = library.available() != catalog || library.active() != selection;
    if changed {
        clear_indexed(snapshot);
        refresh_icons(library, snapshot);
    }
    (outcome, changed)
}

/// Imports each selection and keeps every batch result visible, then refreshes icons once.
/// A single selection preserves its original status and error handling. Cancellation and
/// entirely failed imports leave the icons unchanged.
fn import_files(
    library: &mut GlobalPackLibrary,
    mut paths: Vec<PathBuf>,
    snapshot: &mut Snapshot,
    refresh_icons: impl FnOnce(&GlobalPackLibrary, &mut Snapshot),
) -> Result<(), LibraryError> {
    if paths.len() == 1 {
        import(library, paths.remove(0), snapshot)?;
        refresh_icons(library, snapshot);
        return Ok(());
    }
    let mut messages = Vec::with_capacity(paths.len());
    let mut imported = false;
    snapshot.message.clear();
    for path in paths {
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        match import(library, path, snapshot) {
            Ok(()) => imported = true,
            Err(error) => snapshot.message = error.to_string(),
        }
        messages.push(format!("{name}: {}", snapshot.message));
    }
    snapshot.message = messages.join("\n");
    if imported {
        refresh_icons(library, snapshot);
    }
    Ok(())
}

/// Only actions that refer to published list indices require that list generation.
fn indexed(action: Action) -> bool {
    !matches!(
        action,
        Action::ToggleAvailable
            | Action::ToggleActive
            | Action::CloseSettings
            | Action::Import
            | Action::Apply
            | Action::CancelDelete
            | Action::CycleSort
            | Action::SelectBase
            | Action::RenameFolder
            | Action::CommitRename
            | Action::CancelRename
            | Action::FolderName
            | Action::FolderUp
            | Action::NewFolder
            | Action::DeleteFolder
            | Action::Search
            | Action::ClearSearch
    )
}

/// Uses the protocol's single game-version constant for compatibility checks.
fn engine_version() -> [u32; 3] {
    let mut parts = protocol::GAME_VERSION
        .split('.')
        .map(|part| part.parse().unwrap_or(0));
    std::array::from_fn(|_| parts.next().unwrap_or(0))
}

/// Rebuilds the UI lists from the library's highest-priority-first selection.
fn refresh(library: &GlobalPackLibrary, snapshot: &mut Snapshot) {
    snapshot.memory_tier = library.device_memory_tier();
    snapshot.selection = library.active().to_vec();
    snapshot.active = library
        .active()
        .iter()
        .filter_map(|active| library.metadata(active).cloned())
        .collect();
    snapshot.sort = library.sort();
    snapshot.folders = library.folders().to_vec();
    if !snapshot.folder.is_empty() && !snapshot.folders.contains(&snapshot.folder) {
        snapshot.folder.clear();
    }
    snapshot.available = library
        .available()
        .iter()
        .filter(|pack| !library.active().iter().any(|active| active.id == pack.id))
        .cloned()
        .collect();
    match snapshot.sort {
        PackSort::Added => {}
        PackSort::Name => snapshot
            .available
            .sort_by_cached_key(|pack| pack.name.to_lowercase()),
        PackSort::NameReverse => {
            snapshot
                .available
                .sort_by_cached_key(|pack| pack.name.to_lowercase());
            snapshot.available.reverse();
        }
    }
    snapshot.available_folders = snapshot
        .available
        .iter()
        .map(|pack| library.folder_of(pack.id).to_owned())
        .collect();
}

/// Imports all resource halves and reports skipped behavior or unsupported entries.
fn import(
    library: &mut GlobalPackLibrary,
    path: PathBuf,
    snapshot: &mut Snapshot,
) -> Result<(), LibraryError> {
    let report = library.import(&path)?;
    // A pack added while a folder is open goes into it.
    if !snapshot.folder.is_empty() {
        for pack in &report.imported {
            library.place_pack(pack.id, &snapshot.folder)?;
        }
    }
    clear_indexed(snapshot);
    snapshot.message = format!(
        "Imported {} resource packs; skipped {} behavior packs. {}",
        report.imported.len(),
        report.skipped_behavior,
        report.rejected.join("; ")
    );
    Ok(())
}

/// Moves a dragged pack to where it was let go: into the selected list at a place, back out of
/// it, or to another place in it. Letting go anywhere else changes nothing.
fn drop_pack(
    library: &mut GlobalPackLibrary,
    snapshot: &mut Snapshot,
    from_active: bool,
    index: usize,
    to: Option<usize>,
) -> Result<(), LibraryError> {
    match (from_active, to) {
        (false, Some(place)) => {
            let Some(pack) = snapshot.available.get(index) else {
                return Ok(());
            };
            let id = pack.id;
            library.activate(id)?;
            // Activating puts the pack in the list; `place` counts the packs that were there.
            let place = place.min(library.active().len().saturating_sub(1));
            library.move_pack(id, place)?;
            clear_indexed(snapshot);
            snapshot.selected = Some((true, place));
        }
        (true, None) => {
            let Some(pack) = snapshot.active.get(index) else {
                return Ok(());
            };
            library.deactivate(pack.id);
            clear_indexed(snapshot);
        }
        (true, Some(place)) => {
            let Some(pack) = snapshot.active.get(index) else {
                return Ok(());
            };
            // Letting go below the pack's own place counts the pack itself too.
            let target = if place > index { place - 1 } else { place };
            let target = target.min(snapshot.active.len().saturating_sub(1));
            library.move_pack(pack.id, target)?;
            clear_indexed(snapshot);
            snapshot.selected = Some((true, target));
        }
        (false, None) => {}
    }
    Ok(())
}

fn clear_indexed(snapshot: &mut Snapshot) {
    snapshot.base_selected = false;
    snapshot.delete_pending = None;
    snapshot.selected = None;
    snapshot.details_expanded = None;
    snapshot.settings = None;
}

/// Applies an indexed UI action against the last published lists.
fn act(
    library: &mut GlobalPackLibrary,
    snapshot: &mut Snapshot,
    action: Action,
    outgoing: &crossbeam_channel::Sender<Event>,
) -> Result<(), LibraryError> {
    if !matches!(
        action,
        Action::SelectAvailable(_)
            | Action::SelectActive(_)
            | Action::ReadMore(..)
            | Action::Settings(_)
            | Action::CloseSettings
            | Action::ToggleAvailable
            | Action::ToggleActive
            | Action::AskDelete(_)
            | Action::CancelDelete
            | Action::SelectBase
            | Action::Search
            | Action::ClearSearch
    ) {
        snapshot.message.clear();
    }
    match action {
        Action::SelectAvailable(index) => {
            snapshot.selected = Some((false, index));
            snapshot.base_selected = false;
        }
        Action::SelectActive(index) => {
            snapshot.selected = Some((true, index));
            snapshot.base_selected = false;
        }
        Action::OpenFolder(index) => {
            if let Some(folder) = snapshot.subfolders().get(index) {
                snapshot.folder = (*folder).to_owned();
                clear_indexed(snapshot);
            }
        }
        Action::FolderUp => {
            snapshot.folder = resource_pack::folder_parent(&snapshot.folder).to_owned();
            clear_indexed(snapshot);
        }
        Action::NewFolder => {
            library.create_folder(&snapshot.folder, "")?;
        }
        Action::DeleteFolder => {
            if !snapshot.folder.is_empty() {
                library.delete_folder(&snapshot.folder)?;
                snapshot.folder = resource_pack::folder_parent(&snapshot.folder).to_owned();
                clear_indexed(snapshot);
            }
        }
        Action::PutInFolder { index, folder } => {
            let target = match folder {
                Some(folder) => snapshot.subfolders().get(folder).map(|path| (*path).to_owned()),
                None => Some(resource_pack::folder_parent(&snapshot.folder).to_owned()),
            };
            if let (Some(pack), Some(target)) = (snapshot.available.get(index), target) {
                library.place_pack(pack.id, &target)?;
                clear_indexed(snapshot);
            }
        }
        Action::CycleFolder(index) => {
            if let Some(pack) = snapshot.available.get(index) {
                // The top level, then every folder in name order, round again.
                let mut places = vec![String::new()];
                places.extend(snapshot.folders.iter().cloned());
                let current = library.folder_of(pack.id);
                let at = places.iter().position(|place| place == current).unwrap_or(0);
                let target = places[(at + 1) % places.len()].clone();
                library.place_pack(pack.id, &target)?;
                clear_indexed(snapshot);
            }
        }
        Action::SelectBase => {
            snapshot.selected = None;
            snapshot.delete_pending = None;
            snapshot.base_selected = true;
        }
        Action::Drop {
            from_active,
            index,
            to,
        } => drop_pack(library, snapshot, from_active, index, to)?,
        Action::ReadMore(active, index) => {
            let next = Some((active, index));
            snapshot.details_expanded = if snapshot.details_expanded == next {
                None
            } else {
                next
            };
        }
        Action::Activate(index) => {
            if let Some(pack) = snapshot.available.get(index) {
                library.activate(pack.id)?;
                clear_indexed(snapshot);
            }
        }
        Action::Deactivate(index) => {
            if let Some(pack) = snapshot.active.get(index) {
                library.deactivate(pack.id);
                clear_indexed(snapshot);
            }
        }
        Action::MoveUp(index) | Action::MoveDown(index) => {
            if let Some(pack) = snapshot.active.get(index) {
                let to = if matches!(action, Action::MoveUp(_)) {
                    index.saturating_sub(1)
                } else {
                    (index + 1).min(snapshot.active.len() - 1)
                };
                library.move_pack(pack.id, to)?;
                let moved = |item: usize| {
                    if item == index {
                        to
                    } else if index < to && item > index && item <= to {
                        item - 1
                    } else if to < index && item >= to && item < index {
                        item + 1
                    } else {
                        item
                    }
                };
                snapshot.details_expanded = snapshot
                    .details_expanded
                    .map(|(active, item)| (active, if active { moved(item) } else { item }));
                snapshot.settings = snapshot.settings.map(moved);
                snapshot.selected = Some((true, to));
            }
        }
        Action::AskDelete(index) => {
            snapshot.delete_pending = (index < snapshot.available.len()).then_some(index);
        }
        Action::CancelDelete => snapshot.delete_pending = None,
        // The query and the folder name live in the menu; the worker only sees these when a
        // remote control sends them.
        Action::Search
        | Action::ClearSearch
        | Action::RenameFolder
        | Action::CommitRename
        | Action::CancelRename
        | Action::FolderName => {}
        Action::CycleSort => {
            library.set_sort(library.sort().next())?;
            clear_indexed(snapshot);
        }
        Action::Delete(index) => {
            snapshot.delete_pending = None;
            if let Some(pack) = snapshot.available.get(index) {
                let name = pack.name.clone();
                library.remove(pack.id)?;
                clear_indexed(snapshot);
                snapshot.message = format!("Deleted {name}.");
            }
        }
        Action::Settings(index) => snapshot.settings = Some(index),
        Action::CloseSettings => snapshot.settings = None,
        Action::Subpack(index) => {
            if let Some(pack) = snapshot
                .settings
                .and_then(|index| snapshot.active.get(index))
            {
                let folder = pack.subpacks.get(index).map(|pack| pack.folder.as_str());
                if let Some(folder) = folder {
                    library.select_subpack(pack.id, folder)?;
                    snapshot.settings = None;
                }
            }
        }
        Action::ToggleAvailable => snapshot.available_expanded = !snapshot.available_expanded,
        Action::ToggleActive => snapshot.active_expanded = !snapshot.active_expanded,
        Action::Apply => {
            let stack = {
                let _zone = bevy::log::info_span!("pack_apply.preview").entered();
                library.preview()?
            };
            let _ = outgoing.send(Event::Apply(
                stack,
                library.active().to_vec(),
                ApplyOrigin::User,
            ));
            snapshot.message = "Preparing resource packs…".into();
        }
        Action::Import => unreachable!("file picker handled by worker"),
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod disconnect_tests;

#[cfg(test)]
mod drop_tests;
