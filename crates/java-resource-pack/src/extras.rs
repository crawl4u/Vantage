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

const INCOMPATIBLE_TEXT: &str = include_str!("../data/extra-incompatible.txt");

/// Java paths whose picture is laid out differently from their Bedrock namesake's, so copying
/// them would draw a garbled mob; they never map, by table or by name.
static INCOMPATIBLE: LazyLock<std::collections::HashSet<&'static str>> = LazyLock::new(|| {
    INCOMPATIBLE_TEXT
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
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

/// Java textures whose Bedrock namesake is spelled differently: the Java path and the Bedrock one.
const ALIASES: [(&str, &str); 3] = [
    ("entity/bell/bell_body", "entity/bell/bell"),
    ("entity/shulker/shulker", "entity/shulker/shulker_undyed"),
    (
        "entity/shulker/shulker_light_gray",
        "entity/shulker/shulker_silver",
    ),
];

/// The Bedrock textures a Java texture at `java` could be: the same path and, for an entity, the
/// bare file name in `entity/`.
fn name_candidates(java: &str) -> impl Iterator<Item = &'static str> + '_ {
    let bare = java.rsplit('/').next().unwrap_or(java);
    let bare_entity = java
        .starts_with("entity/")
        .then(|| format!("entity/{bare}"));
    let alias = ALIASES
        .iter()
        .find(|(from, _)| *from == java)
        .map(|(_, to)| (*to).to_owned());
    [alias, Some(java.to_owned()), bare_entity]
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
    !INCOMPATIBLE.contains(java)
        && (TABLE.contains_key(java) || name_candidates(java).next().is_some())
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
    if INCOMPATIBLE.contains(java) {
        return None;
    }
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
    if INCOMPATIBLE.contains(java)
        || width != height
        || TABLE.contains_key(java)
        || !java.starts_with("entity/")
    {
        return None;
    }
    name_candidates(java).find(|candidate| {
        let [vanilla_width, vanilla_height] = BEDROCK_TEXTURES[candidate];
        vanilla_width == vanilla_height * 2
            && width.is_multiple_of(vanilla_width)
            && width >= vanilla_width
    })
}

/// A box of a model whose texture Java lays out upside down: its texture origin `(u, v)` and its
/// width, height and depth in texels at the vanilla 64 by 64 size.
pub(crate) type FlippedBox = [u32; 5];

/// The boxes of a Bedrock texture whose Java namesake stores every face turned over. Java draws
/// these models upside down, so against Bedrock's picture its side faces are turned half a
/// revolution with front and back exchanged, and its top and bottom exchange and flip
/// vertically; used as it is, a chest shows its dark underside on top and upside-down sides.
pub(crate) fn flipped_boxes(bedrock: &str) -> Option<&'static [FlippedBox]> {
    /// A single chest's lid, body and latch.
    const CHEST: [FlippedBox; 3] = [[0, 0, 14, 5, 14], [0, 19, 14, 10, 14], [0, 0, 2, 4, 1]];
    const CHEST_TEXTURES: [&str; 7] = [
        "entity/chest/normal",
        "entity/chest/trapped",
        "entity/chest/ender",
        "entity/chest/copper_default",
        "entity/chest/copper_exposed",
        "entity/chest/copper_weathered",
        "entity/chest/copper_oxidized",
    ];
    CHEST_TEXTURES.contains(&bedrock).then_some(&CHEST[..])
}

/// The six face rectangles `(x, y, width, height)` of a box at the vanilla size, in the order
/// right, front, left, back, top, bottom.
fn box_faces([u, v, w, h, d]: FlippedBox) -> [[u32; 4]; 6] {
    [
        [u, v + d, d, h],
        [u + d, v + d, w, h],
        [u + d + w, v + d, d, h],
        [u + 2 * d + w, v + d, w, h],
        [u + d, v, w, d],
        [u + d + w, v, w, d],
    ]
}

/// Puts a Java texture's boxes in Bedrock's orientation: for each of `boxes` (scaled to `image`),
/// right and left turn half a revolution, front and back exchange and turn, and top and bottom
/// exchange and flip vertically.
pub(crate) fn reorient_boxes(image: &mut image::RgbaImage, boxes: &[FlippedBox]) {
    let scale = image.width() / 64;
    if scale == 0 || image.width() != image.height() {
        return;
    }
    let source = image.clone();
    // Each Bedrock face (by index) takes the Java face named here, turned as flagged.
    const TAKES: [(usize, bool, bool); 6] = [
        (0, true, true),
        (3, true, true),
        (2, true, true),
        (1, true, true),
        (5, false, true),
        (4, false, true),
    ];
    for boxed in boxes {
        let faces = box_faces(boxed.map(|value| value * scale));
        for (target, &(from, flip_x, flip_y)) in faces.iter().zip(&TAKES) {
            let origin = faces[from];
            if origin[2] != target[2] || origin[3] != target[3] {
                continue;
            }
            let fits = |r: [u32; 4]| r[0] + r[2] <= image.width() && r[1] + r[3] <= image.height();
            if !fits(*target) || !fits(origin) {
                continue;
            }
            for y in 0..target[3] {
                for x in 0..target[2] {
                    let sx = if flip_x { target[2] - 1 - x } else { x };
                    let sy = if flip_y { target[3] - 1 - y } else { y };
                    image.put_pixel(
                        target[0] + x,
                        target[1] + y,
                        *source.get_pixel(origin[0] + sx, origin[1] + sy),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A chest unwrap in which every face of every box is its own flat colour, so a face's
    /// destination shows where it came from.
    fn chest_unwrap(scale: u32) -> image::RgbaImage {
        let mut image = image::RgbaImage::new(64 * scale, 64 * scale);
        let mut colour = 10u8;
        for boxed in flipped_boxes("entity/chest/normal").unwrap() {
            for [x, y, w, h] in box_faces(*boxed) {
                colour += 10;
                for dy in 0..h * scale {
                    for dx in 0..w * scale {
                        image.put_pixel(
                            x * scale + dx,
                            y * scale + dy,
                            image::Rgba([colour, 0, 0, 255]),
                        );
                    }
                }
            }
        }
        image
    }

    #[test]
    fn a_java_chest_has_each_face_moved_and_turned_to_bedrocks_place() {
        for scale in [1, 2] {
            let before = chest_unwrap(scale);
            let mut image = before.clone();
            reorient_boxes(&mut image, flipped_boxes("entity/chest/normal").unwrap());
            for boxed in &flipped_boxes("entity/chest/normal").unwrap()[..2] {
                let faces = box_faces(boxed.map(|value| value * scale));
                let red = |image: &image::RgbaImage, face: usize| {
                    image.get_pixel(faces[face][0] + 1, faces[face][1] + 1)[0]
                };
                // Bedrock's right, front, left, back, top and bottom hold Java's right, back,
                // left, front, bottom and top.
                for (bedrock, java) in [(0, 0), (1, 3), (2, 2), (3, 1), (4, 5), (5, 4)] {
                    assert_eq!(
                        red(&image, bedrock),
                        red(&before, java),
                        "scale {scale} face {bedrock}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_marked_row_ends_up_on_the_far_edge_of_its_turned_face() {
        let mut image = image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 255]));
        // Java's lid front face: mark its top row.
        let lid = flipped_boxes("entity/chest/normal").unwrap()[0];
        let java_front = box_faces(lid)[1];
        for dx in 0..java_front[2] {
            image.put_pixel(
                java_front[0] + dx,
                java_front[1],
                image::Rgba([255, 0, 0, 255]),
            );
        }
        reorient_boxes(&mut image, &[lid]);
        // Bedrock's back face holds it, turned: the marked row is now its bottom row.
        let back = box_faces(lid)[3];
        assert_eq!(image.get_pixel(back[0], back[1] + back[3] - 1)[0], 255);
        assert_eq!(image.get_pixel(back[0], back[1])[0], 0);
    }

    #[test]
    fn reorienting_twice_restores_the_original_and_other_textures_are_untouched() {
        let original = chest_unwrap(1);
        let mut image = original.clone();
        let boxes = flipped_boxes("entity/chest/trapped").unwrap();
        reorient_boxes(&mut image, boxes);
        assert_ne!(image, original);
        reorient_boxes(&mut image, boxes);
        assert_eq!(image, original);
        assert!(flipped_boxes("entity/zombie/zombie").is_none());
        assert!(flipped_boxes("entity/boat/chest_boat_oak").is_none());
    }

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
    fn every_alias_names_a_bedrock_texture() {
        for (java, bedrock) in ALIASES {
            assert!(BEDROCK_TEXTURES.contains_key(bedrock), "{bedrock}");
            assert!(!TABLE.contains_key(java), "{java} is already in the table");
        }
        assert_eq!(
            bedrock_path("entity/shulker/shulker", 128, 128),
            Some("entity/shulker/shulker_undyed")
        );
        assert_eq!(
            bedrock_path("entity/bell/bell_body", 64, 64),
            Some("entity/bell/bell")
        );
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
                ["png", "tga"]
                    .iter()
                    .any(|extension| texture.with_extension(extension).is_file()),
                "{} is not a sample-pack texture",
                target.bedrock
            );
        }
    }
}
