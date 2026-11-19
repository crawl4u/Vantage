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

const BEDROCK_TEXT: &str = include_str!("../data/bedrock-textures.tsv");

/// Bedrock texture (relative to `textures/`, no extension) to its vanilla size.
static BEDROCK_TEXTURES: LazyLock<HashMap<&'static str, [u32; 2]>> = LazyLock::new(|| {
    BEDROCK_TEXT
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut parts = line.split('\t');
            let path = parts.next().unwrap();
            let mut side = || parts.next().unwrap().parse::<u32>().unwrap();
            (path, [side(), side()])
        })
        .collect()
});

/// Paths a Java texture is never matched to by name: their layouts differ from Bedrock's
/// namesakes, or they are not mob skins at all.
const NAME_EXCLUDED: [&str; 6] = [
    "entity/banner/",
    "entity/shield/",
    "entity/player/",
    "entity/equipment/",
    "/profession",
    "/type/",
];

/// The Bedrock textures a Java texture at `java` could be: the same path and, for an entity, the
/// bare file name in `entity/`.
fn name_candidates(java: &str) -> impl Iterator<Item = &'static str> + '_ {
    let bare = java.rsplit('/').next().unwrap_or(java);
    let bare_entity = java
        .starts_with("entity/")
        .then(|| format!("entity/{bare}"));
    [Some(java.to_owned()), bare_entity]
        .into_iter()
        .flatten()
        .filter(move |_| !NAME_EXCLUDED.iter().any(|part| java.contains(part)))
        .filter_map(|candidate| {
            BEDROCK_TEXTURES
                .get_key_value(candidate.as_str())
                .map(|(path, _)| *path)
        })
}

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
    TABLE.contains_key(java) || name_candidates(java).next().is_some()
}

/// The Bedrock path (relative to `textures/`, no extension) for the Java texture at `java`
/// (relative to `textures/`, no extension) whose image is `width` by `height`; `None` when the
/// table has no entry or the image is not a whole-number multiple of the vanilla size.
pub(crate) fn bedrock_path(java: &str, width: u32, height: u32) -> Option<&'static str> {
    let scaled = |[vanilla_width, vanilla_height]: [u32; 2]| {
        width.is_multiple_of(vanilla_width)
            && height.is_multiple_of(vanilla_height)
            && width / vanilla_width == height / vanilla_height
            && width >= vanilla_width
    };
    if let Some(target) = TABLE.get(java) {
        return scaled(target.size).then_some(target.bedrock);
    }
    name_candidates(java).find(|candidate| scaled(BEDROCK_TEXTURES[candidate]))
}

/// The Bedrock entity texture a square Java texture at `java` fills with its top half, when
/// Bedrock's namesake is twice as wide as high and the Java width is a whole multiple of it. The
/// modern 64 by 64 humanoid layout keeps the classic 64 by 32 one in its top half, which is all
/// the older Bedrock model reads.
pub(crate) fn top_half_path(java: &str, width: u32, height: u32) -> Option<&'static str> {
    if width != height || TABLE.contains_key(java) || !java.starts_with("entity/") {
        return None;
    }
    name_candidates(java).find(|candidate| {
        let [vanilla_width, vanilla_height] = BEDROCK_TEXTURES[candidate];
        vanilla_width == vanilla_height * 2
            && width.is_multiple_of(vanilla_width)
            && width >= vanilla_width
    })
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
    fn the_texture_listing_matches_the_pinned_sample_pack() {
        let Some(pack) = crate::test_support::sample_pack() else {
            return;
        };
        for (path, [width, height]) in BEDROCK_TEXTURES.iter() {
            let size = image::image_dimensions(pack.join("textures").join(format!("{path}.png")))
                .unwrap_or_else(|error| panic!("{path}: {error}"));
            assert_eq!(size, (*width, *height), "{path}");
        }
    }

    #[test]
    fn older_mob_names_match_bedrock_by_file_name_and_size() {
        // A 1.12-era cow and pig: same layout and size as Bedrock's.
        assert_eq!(
            bedrock_path("entity/cow/cow", 64, 32),
            Some("entity/cow/cow")
        );
        assert_eq!(
            bedrock_path("entity/pig/pig", 128, 64),
            Some("entity/pig/pig")
        );
        // The bare file name in `entity/` also counts.
        assert_eq!(
            bedrock_path("entity/squid/squid", 64, 32),
            Some("entity/squid")
        );
        // A different layout (the modern 64 by 64 zombie) or a villager type is not matched.
        assert_eq!(bedrock_path("entity/zombie/zombie", 64, 64), None);
        assert_eq!(
            top_half_path("entity/zombie/zombie", 128, 128),
            Some("entity/zombie/zombie")
        );
        assert_eq!(top_half_path("entity/zombie/zombie", 128, 64), None);
        assert_eq!(bedrock_path("entity/villager/type/plains", 64, 64), None);
        assert!(!is_listed("entity/banner/base"));
        // Older environment names match Bedrock's by path.
        assert_eq!(
            bedrock_path("environment/sun", 128, 128),
            Some("environment/sun")
        );
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
