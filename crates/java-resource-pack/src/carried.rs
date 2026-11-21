//! The pre-coloured textures Bedrock draws a block with in a hand or inventory slot.
//!
//! Java stores grass, foliage and similar plants in grey and colours them when drawn, with the
//! biome's colour in the world and a fixed default colour in the hand and inventory. Bedrock
//! instead ships a second, already coloured picture of each such block, named `*_carried`.
//! [`outputs`] makes those pictures from the pack's grey ones, so a block held or in the
//! inventory shows the pack's art in the colour Java gives it, whatever the pack.

use crate::texture::{Budget, decode, duplicate, strip_frame};
use image::RgbaImage;

/// How a carried picture is coloured.
#[derive(Clone, Copy)]
enum Tint {
    /// Java's default grass colour.
    Grass,
    /// Java's default foliage colour.
    Foliage,
    /// A fixed colour Java gives the block.
    Fixed([u8; 3]),
}

/// Java block texture, the Bedrock carried texture drawn from it, and its colour.
const CARRIED: &[(&str, &str, Tint)] = &[
    ("grass_block_top", "grass_carried", Tint::Grass),
    ("short_grass", "tallgrass_carried", Tint::Grass),
    ("fern", "fern_carried", Tint::Grass),
    ("tall_grass_top", "double_plant_grass_carried", Tint::Grass),
    ("large_fern_top", "double_plant_fern_carried", Tint::Grass),
    ("oak_leaves", "leaves_oak_carried", Tint::Foliage),
    ("jungle_leaves", "leaves_jungle_carried", Tint::Foliage),
    ("acacia_leaves", "leaves_acacia_carried", Tint::Foliage),
    ("dark_oak_leaves", "leaves_big_oak_carried", Tint::Foliage),
    (
        "spruce_leaves",
        "leaves_spruce_carried",
        Tint::Fixed([0x61, 0x99, 0x61]),
    ),
    (
        "birch_leaves",
        "leaves_birch_carried",
        Tint::Fixed([0x80, 0xA7, 0x55]),
    ),
    (
        "mangrove_leaves",
        "mangrove_leaves_carried",
        Tint::Fixed([0x92, 0xC6, 0x48]),
    ),
    ("vine", "vine_carried", Tint::Foliage),
    (
        "lily_pad",
        "carried_waterlily",
        Tint::Fixed([0x20, 0x80, 0x30]),
    ),
];

/// Java's default grass colour when the pack's colour map names none.
const GRASS_FALLBACK: [u8; 3] = [0x7C, 0xBD, 0x6B];
/// Java's default foliage colour.
const FOLIAGE_DEFAULT: [u8; 3] = [0x48, 0xB5, 0x18];

/// The colour Java's default biome (temperature 0.5, downfall 1.0) gives grass in `map`, the
/// middle of the colour map.
pub(crate) fn default_grass(map: Option<&RgbaImage>) -> [u8; 3] {
    map.filter(|map| map.width() > 0 && map.height() > 0)
        .map(|map| {
            let pixel = map.get_pixel((map.width() - 1) / 2, (map.height() - 1) / 2);
            [pixel[0], pixel[1], pixel[2]]
        })
        .unwrap_or(GRASS_FALLBACK)
}

/// The carried pictures `blocks` (Java block texture stems and PNG bytes) give, as Bedrock paths
/// under `textures/blocks/`, coloured with `grass`.
pub(crate) fn outputs(
    blocks: &[(String, Vec<u8>)],
    grass: [u8; 3],
    budget: &Budget,
) -> Vec<(String, RgbaImage)> {
    CARRIED
        .iter()
        .filter_map(|(java, bedrock, tint)| {
            let bytes = &blocks.iter().find(|(name, _)| name == java)?.1;
            let image = decode(bytes, budget)?;
            // An animated source gives its first frame.
            let image = match strip_frame(image.width(), image.height()) {
                Some(side) => crate::texture::crop(&image, 0, 0, side, side, budget)?,
                None => image,
            };
            let colour = match tint {
                Tint::Grass => grass,
                Tint::Foliage => FOLIAGE_DEFAULT,
                Tint::Fixed(colour) => *colour,
            };
            let mut tinted = duplicate(&image, budget)?;
            for pixel in tinted.pixels_mut() {
                for channel in 0..3 {
                    pixel[channel] =
                        (u16::from(pixel[channel]) * u16::from(colour[channel]) / 255) as u8;
                }
            }
            Some((format!("textures/blocks/{bedrock}.png"), tinted))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture::encode;
    use image::Rgba;

    fn grey(value: u8) -> Vec<u8> {
        encode(&RgbaImage::from_pixel(
            16,
            16,
            Rgba([value, value, value, 255]),
        ))
    }

    #[test]
    fn grey_block_art_comes_out_in_the_colour_java_holds_it_in() {
        let blocks = vec![
            ("grass_block_top".to_owned(), grey(200)),
            ("spruce_leaves".to_owned(), grey(255)),
            ("stone".to_owned(), grey(10)),
        ];
        let made = outputs(&blocks, [100, 200, 50], &Budget::new());
        assert_eq!(made.len(), 2, "only blocks Bedrock carries coloured");
        let grass = made
            .iter()
            .find(|(path, _)| path.ends_with("grass_carried.png"))
            .unwrap();
        assert_eq!(grass.1.get_pixel(0, 0).0, [78, 156, 39, 255]);
        let spruce = made
            .iter()
            .find(|(path, _)| path.ends_with("leaves_spruce_carried.png"))
            .unwrap();
        assert_eq!(spruce.1.get_pixel(3, 3).0, [0x61, 0x99, 0x61, 255]);
    }

    #[test]
    fn the_default_grass_colour_is_the_colour_maps_middle_or_javas_fallback() {
        let mut map = RgbaImage::new(256, 256);
        map.put_pixel(127, 127, Rgba([1, 2, 3, 255]));
        assert_eq!(default_grass(Some(&map)), [1, 2, 3]);
        assert_eq!(default_grass(None), GRASS_FALLBACK);
    }
}
