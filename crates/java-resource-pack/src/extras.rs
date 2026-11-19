//! Textures outside blocks, items and the GUI: mobs and other entities, paintings, maps, the
//! colour maps, the sky's sun and moon, and a few particles and misc pictures. A Java texture goes
//! where [`TABLE`] says, when the pack's copy is the same picture at a whole-number multiple of
//! its vanilla size, so an HD pack's mob skins land at their Bedrock paths at full resolution.
//!
//! The table comes from `tools/java-pack-tables/derive_extras.py`, which matches Java Edition's
//! textures against the pinned Bedrock sample pack by pixels, else by path and size.

use std::{collections::HashMap, sync::LazyLock};

const TABLE_TEXT: &str = include_str!("../data/extra-names.tsv");

/// Where one Java texture goes: its Bedrock path and its vanilla size.
#[derive(Clone, Copy)]
struct Target {
    bedrock: &'static str,
    size: [u32; 2],
}

/// Java path (relative to `textures/`, no extension) to its target.
static TABLE: LazyLock<HashMap<&'static str, Target>> = LazyLock::new(|| {
    TABLE_TEXT
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut parts = line.split('\t');
            let (java, bedrock) = (parts.next().unwrap(), parts.next().unwrap());
            let mut side = || parts.next().unwrap().parse::<u32>().unwrap();
            let size = [side(), side()];
            (java, Target { bedrock, size })
        })
        .collect()
});

/// The folders of `textures/` the table covers.
pub(crate) const FOLDERS: [&str; 8] = [
    "entity/",
    "painting/",
    "environment/",
    "misc/",
    "map/",
    "colormap/",
    "particle/",
    "trims/",
];

/// Whether the table has an entry for the Java texture at `java` (relative to `textures/`, no
/// extension).
pub(crate) fn is_listed(java: &str) -> bool {
    TABLE.contains_key(java)
}

/// The Bedrock path (relative to `textures/`, no extension) for the Java texture at `java`
/// (relative to `textures/`, no extension) whose image is `width` by `height`; `None` when the
/// table has no entry or the image is not a whole-number multiple of the vanilla size.
pub(crate) fn bedrock_path(java: &str, width: u32, height: u32) -> Option<&'static str> {
    let target = TABLE.get(java)?;
    let [vanilla_width, vanilla_height] = target.size;
    let scaled = width.is_multiple_of(vanilla_width)
        && height.is_multiple_of(vanilla_height)
        && width / vanilla_width == height / vanilla_height
        && width >= vanilla_width;
    scaled.then_some(target.bedrock)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_is_well_formed_and_every_java_path_is_listed_once() {
        let lines = TABLE_TEXT
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .count();
        assert_eq!(TABLE.len(), lines, "duplicate Java path");
        for (java, target) in TABLE.iter() {
            assert!(
                FOLDERS.iter().any(|folder| java.starts_with(folder)),
                "{java} is outside the covered folders"
            );
            assert!(!target.bedrock.ends_with(".png"), "{}", target.bedrock);
            assert!(target.size[0] > 0 && target.size[1] > 0, "{java}");
        }
    }

    #[test]
    fn a_whole_number_multiple_of_the_vanilla_size_maps_and_anything_else_does_not() {
        let (java, target) = TABLE.iter().next().expect("the table has entries");
        let [width, height] = target.size;
        assert_eq!(bedrock_path(java, width, height), Some(target.bedrock));
        assert_eq!(
            bedrock_path(java, width * 4, height * 4),
            Some(target.bedrock)
        );
        assert_eq!(bedrock_path(java, width * 4, height * 2), None);
        assert_eq!(bedrock_path(java, width + 1, height), None);
        assert_eq!(bedrock_path("entity/not_a_texture", width, height), None);
    }

    #[test]
    fn mobs_whose_java_and_bedrock_names_differ_still_map() {
        assert_eq!(
            bedrock_path("entity/horse/mule", 128, 128),
            Some("entity/horse2/mule")
        );
        assert_eq!(
            bedrock_path("entity/wither/wither", 64, 64),
            Some("entity/wither_boss/wither")
        );
        // Banner and shield patterns, skins and professions are laid out differently in Bedrock.
        for java in [
            "entity/banner/base",
            "entity/shield/base",
            "entity/player/wide/steve",
            "entity/villager/profession/farmer",
        ] {
            assert!(!is_listed(java), "{java}");
        }
    }

    #[test]
    fn the_table_matches_the_pinned_sample_pack() {
        let Some(pack) = crate::test_support::sample_pack() else {
            return;
        };
        for target in TABLE.values() {
            let texture = pack.join("textures").join(target.bedrock);
            assert!(
                texture.with_extension("png").is_file(),
                "{} is not a sample-pack texture",
                target.bedrock
            );
        }
    }
}
