//! Inventory slot textures: Java's `gui/container/inventory.png` and the hotbar selection become
//! the slot cells and selection highlight Bedrock's inventory screens draw, and the cell variants
//! Java has no picture for (dark, inverted, red and locked cells) are made from the pack's own cell
//! so every state of a slot matches the pack.

use crate::texture::{Budget, crop, duplicate, is_empty};
use image::RgbaImage;

/// Where the first slot of the player's inventory sits in a 256-pixel legacy container atlas, and
/// the side of a slot.
const SLOT_X: u32 = 7;
const SLOT_Y: u32 = 83;
const SLOT: u32 = 18;

/// Bedrock's slot cell and the variants it draws for other slot states.
pub(crate) const CELL: &str = "textures/ui/cell_image.png";
pub(crate) const CELL_NORMAL: &str = "textures/ui/cell_image_normal.png";
pub(crate) const CELL_DARK: &str = "textures/ui/cell_image_darkgrey.png";
pub(crate) const CELL_INVERT: &str = "textures/ui/cell_image_invert.png";
pub(crate) const CELL_RED: &str = "textures/ui/cell_image_red.png";
pub(crate) const CELL_LOCK: &str = "textures/ui/cell_image_lock.png";
/// The highlight Bedrock draws round the selected slot of its pocket inventory.
pub(crate) const SLOT_HIGHLIGHT: &str = "textures/ui/pocket_ui_highlight_selected_slot.png";

/// Every slot cell texture, each with a nine-slice sidecar.
pub(crate) const CELL_TEXTURES: [&str; 6] = [
    CELL,
    CELL_NORMAL,
    CELL_DARK,
    CELL_INVERT,
    CELL_RED,
    CELL_LOCK,
];

/// The sidecar of a cell texture. The cell is an 18-pixel Java slot whose one-pixel outline stays
/// at its own width however large the cell is drawn; `base_size` is the slot's size before the
/// atlas's scale, as the experience bar's sidecar describes its image.
pub(crate) const CELL_SIDECAR: &[u8] = br#"{"nineslice_size":1,"base_size":[18,18]}"#;

/// How much of each channel a darkened cell keeps.
const DARK_SHARE: u32 = 55;
/// How much of each channel a locked cell keeps.
const LOCK_SHARE: u32 = 35;
/// What a red cell adds to its red channel, and how much of its other channels it keeps.
const RED_BOOST: u32 = 90;
const RED_KEEP: u32 = 60;

/// The slot cell cut from a container atlas; `None` when the atlas has no artwork there.
pub(crate) fn slot_cell(atlas: &RgbaImage, budget: &Budget) -> Option<RgbaImage> {
    let scale = (atlas.width() / 256).max(1);
    let cell = crop(
        atlas,
        SLOT_X * scale,
        SLOT_Y * scale,
        SLOT * scale,
        SLOT * scale,
        budget,
    )?;
    (!is_empty(&cell)).then_some(cell)
}

/// Rewrites the colour of every opaque pixel with `map`, keeping alpha.
fn recolour(
    cell: &RgbaImage,
    budget: &Budget,
    map: impl Fn([u8; 3]) -> [u8; 3],
) -> Option<RgbaImage> {
    let mut out = duplicate(cell, budget)?;
    for pixel in out.pixels_mut() {
        let [r, g, b, a] = pixel.0;
        if a != 0 {
            let [r, g, b] = map([r, g, b]);
            pixel.0 = [r, g, b, a];
        }
    }
    Some(out)
}

/// Scales a channel to `share` percent.
fn share(channel: u8, share: u32) -> u8 {
    (u32::from(channel) * share / 100) as u8
}

/// The Bedrock slot textures for a Java container atlas: the cell and the variants made from it,
/// each paired with its path. Empty when the atlas has no slot artwork.
pub(crate) fn cell_textures(atlas: &RgbaImage, budget: &Budget) -> Vec<(String, RgbaImage)> {
    let Some(cell) = slot_cell(atlas, budget) else {
        return Vec::new();
    };
    let dark = recolour(&cell, budget, |[r, g, b]| {
        [
            share(r, DARK_SHARE),
            share(g, DARK_SHARE),
            share(b, DARK_SHARE),
        ]
    });
    let invert = recolour(&cell, budget, |[r, g, b]| [255 - r, 255 - g, 255 - b]);
    let red = recolour(&cell, budget, |[r, g, b]| {
        [
            (u32::from(r) + RED_BOOST).min(255) as u8,
            share(g, RED_KEEP),
            share(b, RED_KEEP),
        ]
    });
    let lock = recolour(&cell, budget, |[r, g, b]| {
        [
            share(r, LOCK_SHARE),
            share(g, LOCK_SHARE),
            share(b, LOCK_SHARE),
        ]
    });
    let normal = duplicate(&cell, budget);
    [
        (CELL, Some(cell)),
        (CELL_NORMAL, normal),
        (CELL_DARK, dark),
        (CELL_INVERT, invert),
        (CELL_RED, red),
        (CELL_LOCK, lock),
    ]
    .into_iter()
    .filter_map(|(path, image)| Some((path.to_owned(), image?)))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// A 256-pixel atlas whose slot has a grey outline over a transparent middle.
    fn atlas() -> RgbaImage {
        let mut atlas = RgbaImage::new(256, 256);
        for i in 0..SLOT {
            for (x, y) in [(i, 0), (i, SLOT - 1), (0, i), (SLOT - 1, i)] {
                atlas.put_pixel(SLOT_X + x, SLOT_Y + y, Rgba([100, 120, 140, 255]));
            }
        }
        atlas
    }

    #[test]
    fn an_atlas_with_a_slot_gives_the_cell_and_every_variant() {
        let budget = Budget::new();
        let textures = cell_textures(&atlas(), &budget);
        let paths: Vec<&str> = textures.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(paths, CELL_TEXTURES);
        for (_, image) in &textures {
            assert_eq!(image.dimensions(), (SLOT, SLOT));
        }
    }

    #[test]
    fn variants_recolour_the_outline_and_keep_the_middle_clear() {
        let budget = Budget::new();
        let textures = cell_textures(&atlas(), &budget);
        let outline = |path: &str| {
            textures
                .iter()
                .find(|(candidate, _)| candidate == path)
                .map(|(_, image)| image.get_pixel(0, 0).0)
                .unwrap()
        };
        assert_eq!(outline(CELL), [100, 120, 140, 255]);
        assert_eq!(outline(CELL_INVERT), [155, 135, 115, 255]);
        let dark = outline(CELL_DARK);
        assert!(dark[0] < 100 && dark[1] < 120 && dark[2] < 140 && dark[3] == 255);
        let red = outline(CELL_RED);
        assert!(red[0] > 100 && red[1] < 120 && red[2] < 140);
        assert!(outline(CELL_LOCK)[0] < dark[0]);
        for (_, image) in &textures {
            assert_eq!(image.get_pixel(9, 9).0[3], 0, "the middle stays clear");
        }
    }

    #[test]
    fn an_atlas_scales_the_cell_with_its_resolution() {
        let budget = Budget::new();
        let mut big = RgbaImage::new(512, 512);
        big.put_pixel(SLOT_X * 2, SLOT_Y * 2, Rgba([1, 2, 3, 255]));
        let cell = slot_cell(&big, &budget).unwrap();
        assert_eq!(cell.dimensions(), (SLOT * 2, SLOT * 2));
    }

    #[test]
    fn an_atlas_without_slot_artwork_gives_nothing() {
        let budget = Budget::new();
        assert!(cell_textures(&RgbaImage::new(256, 256), &budget).is_empty());
        assert!(slot_cell(&RgbaImage::new(64, 64), &budget).is_none());
    }
}
