//! Java Edition art that an import filled in from the installed game, kept below every pack.
//!
//! A Java pack ships art only for what it restyles, and Java draws the rest from its own game
//! files. An import therefore adds the game's art for every texture the pack omits and lists those
//! files in a marker. Left inside the pack, they would cover the packs beneath it, so a pack that
//! only restyles blocks would hide another pack's swords and armour. The stack instead puts those
//! files under all packs, where the game's own art belongs.

use std::collections::HashSet;

use crate::pack::{ValidatedPack, ValidatedPackStack};

/// The file in an imported Java pack naming the files its import filled in from the game: a JSON
/// object with a `paths` list.
pub(crate) const JAVA_BASE_FILL_MARKER: &str = "cinnabar/java_base_fill.json";

/// The shape of [`JAVA_BASE_FILL_MARKER`].
#[derive(serde::Deserialize)]
struct Marker {
    paths: Vec<String>,
}

/// The sub-pack name of a split-off fill layer, so it never shares an identity with its pack.
const FILL_LAYER: &str = "java-base";

impl ValidatedPack {
    /// The files this pack's import filled in from the game, when it names any.
    fn java_base_fill_paths(&self) -> Option<HashSet<String>> {
        let bytes = self.read_file(JAVA_BASE_FILL_MARKER).ok()??;
        let marker: Marker = serde_json::from_slice(&bytes).ok()?;
        let paths: HashSet<String> = marker.paths.into_iter().collect();
        (!paths.is_empty()).then_some(paths)
    }

    /// Splits the filled-in files from the pack's own: `(fill, own)`. Both share the archive.
    fn split_java_base_fill(&self) -> Option<(Self, Self)> {
        let filled = self.java_base_fill_paths()?;
        let keep = |pack: &mut Self, wanted: bool| {
            pack.files.retain(|path, _| filled.contains(&**path) == wanted);
            pack.folded
                .retain(|_, exact| filled.contains(&**exact) == wanted);
            pack.file_order = pack
                .file_order
                .iter()
                .filter(|path| filled.contains(&***path) == wanted)
                .cloned()
                .collect();
        };
        let mut fill = self.clone();
        keep(&mut fill, true);
        let mut own = self.clone();
        keep(&mut own, false);
        fill.physical_entry_count = fill.files.len();
        own.physical_entry_count = self.physical_entry_count.saturating_sub(fill.files.len());
        // The archive is counted once, with the pack's own layer.
        fill.declared_bytes = 0;
        fill.sub_pack_name = FILL_LAYER.into();
        Some((fill, own))
    }
}

impl ValidatedPackStack {
    /// The stack with every imported Java pack's filled-in files moved under all the packs, in
    /// their original order, and the packs' own files above them.
    #[must_use]
    pub(crate) fn with_java_base_fills_below(self) -> Self {
        let mut fills = Vec::new();
        let mut layers = Vec::with_capacity(self.packs.len());
        for pack in self.packs.iter() {
            match pack.split_java_base_fill() {
                Some((fill, own)) => {
                    fills.push(fill);
                    layers.push(own);
                }
                None => layers.push(pack.clone()),
            }
        }
        if fills.is_empty() {
            return self;
        }
        fills.extend(layers);
        Self {
            packs: fills.into_boxed_slice(),
            rejections: self.rejections,
            fingerprint: Default::default(),
        }
    }
}

#[cfg(all(test, feature = "handoff"))]
mod tests {
    use super::*;
    use crate::test_support::layered_pack_view;

    fn file(path: &str, bytes: &[u8]) -> (String, Vec<u8>) {
        (path.to_owned(), bytes.to_vec())
    }

    /// The marker for a pack whose import filled in `paths`.
    fn marker(paths: &[&str]) -> (String, Vec<u8>) {
        file(
            JAVA_BASE_FILL_MARKER,
            serde_json::json!({ "paths": paths }).to_string().as_bytes(),
        )
    }

    #[test]
    fn a_packs_filled_in_files_sit_under_the_packs_below_it_and_its_own_stay_above() {
        let lower = [file("textures/items/sword.png", b"lower sword")];
        let upper = [
            file("textures/items/sword.png", b"base sword"),
            file("textures/blocks/dirt.png", b"upper dirt"),
            marker(&["textures/items/sword.png"]),
        ];
        let view = layered_pack_view(&[&lower, &upper]);
        // Left alone, the upper pack's filled-in sword covers the lower pack's.
        assert_eq!(
            &*view.read("textures/items/sword.png").unwrap(),
            b"base sword"
        );
        let hoisted = crate::LayeredPackView::new(std::sync::Arc::new(
            ValidatedPackStack {
                packs: view.stack().packs.clone(),
                rejections: Box::default(),
                fingerprint: Default::default(),
            }
            .with_java_base_fills_below(),
        ));
        assert_eq!(
            &*hoisted.read("textures/items/sword.png").unwrap(),
            b"lower sword"
        );
        assert_eq!(
            &*hoisted.read("textures/blocks/dirt.png").unwrap(),
            b"upper dirt"
        );
    }

    #[test]
    fn filled_in_files_still_show_when_no_pack_draws_them() {
        let only = [
            file("textures/items/sword.png", b"base sword"),
            marker(&["textures/items/sword.png"]),
        ];
        let view = layered_pack_view(&[&only]);
        let hoisted = ValidatedPackStack {
            packs: view.stack().packs.clone(),
            rejections: Box::default(),
            fingerprint: Default::default(),
        }
        .with_java_base_fills_below();
        let view = crate::LayeredPackView::new(std::sync::Arc::new(hoisted));
        assert_eq!(
            &*view.read("textures/items/sword.png").unwrap(),
            b"base sword"
        );
    }

    #[test]
    fn a_stack_without_markers_is_unchanged() {
        let plain = [file("textures/items/sword.png", b"sword")];
        let view = layered_pack_view(&[&plain]);
        let before = view.stack().packs.len();
        let after = ValidatedPackStack {
            packs: view.stack().packs.clone(),
            rejections: Box::default(),
            fingerprint: Default::default(),
        }
        .with_java_base_fills_below();
        assert_eq!(after.packs.len(), before);
    }
}
