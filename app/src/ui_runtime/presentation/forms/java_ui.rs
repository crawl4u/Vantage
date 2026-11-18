//! Keeps the Java-style menus' files in step with the resource packs the player selected: the
//! font, buttons, panorama and splashes follow the packs as they do in Java.

use std::{path::PathBuf, sync::Arc};

use bevy::prelude::{Local, Res, ResMut};
use client_shell::ui_resources::UiPresentationResource;
use java_resource_pack::menu_files::PACK_PREFIX;
use resource_pack::{LayeredPackView, ValidatedPackStack};

use crate::runtime::network::PackReload;

/// Reads `path` (relative to `assets/minecraft/`) from the selected packs, highest priority first.
pub(crate) fn read_from_packs(view: &LayeredPackView, path: &str) -> Option<Vec<u8>> {
    view.read(&format!("{PACK_PREFIX}{path}"))
        .map(|bytes| bytes.into_vec())
}

/// Where the player's Java files were cached, none when the install cannot be found.
pub(crate) fn cache_dir() -> Option<PathBuf> {
    let layout = launcher::install_layout::InstallLayout::discover().ok()?;
    Some(launcher_host::java_ui::cache_dir(&layout))
}

/// Adds the Java menu files again whenever the selected resource packs change.
pub(crate) fn drive_java_ui(
    reload: Option<Res<PackReload>>,
    mut presentation: ResMut<UiPresentationResource>,
    mut applied: Local<Option<Arc<ValidatedPackStack>>>,
) {
    let Some(reload) = reload else {
        return;
    };
    let stack = reload.global_stack();
    if applied
        .as_ref()
        .is_some_and(|applied| Arc::ptr_eq(applied, stack))
    {
        return;
    }
    // Before the artwork is installed nothing is reloaded; the next frame tries again.
    if presentation.0.ui_assets().is_none() {
        return;
    }
    let view = LayeredPackView::new(stack.clone());
    let from_packs = |path: &str| read_from_packs(&view, path);
    let cache = cache_dir();
    if let Err(reason) = presentation.0.reload_java_ui(&[&from_packs], cache.as_deref()) {
        bevy::log::warn!("Java menu files unusable ({reason})");
    }
    *applied = Some(stack.clone());
}
