//! Bedrock's particle atlas from Java's particle sprites. Bedrock draws most particles from one
//! 128-pixel atlas of 8-pixel cells, Java 1.20+ keeps a file per frame. A translated pack carries
//! an atlas holding only the cells a Java sprite fills, with a marker listing them, so the client
//! lays those cells over its own atlas instead of blanking the rest.

use crate::texture::{Budget, canvas, paste};
use image::{RgbaImage, imageops};
use std::{borrow::Cow, collections::HashMap, sync::LazyLock};

const TABLE_TEXT: &str = include_str!("../data/particle-cells.tsv");

/// The side of Bedrock's atlas and of one cell, in pixels at vanilla scale.
pub const ATLAS_SIDE: u32 = 128;
pub const CELL: u32 = 8;

/// The atlas texture, and the file beside it listing the cells a pack supplies.
pub const ATLAS: &str = "textures/particle/particles";
pub const MARKER: &str = "textures/particle/particles.overlay.json";

/// Java sprite name (relative to `textures/particle/`, no extension) and the cell it fills.
static CELLS: LazyLock<Vec<(&'static str, [u32; 2])>> = LazyLock::new(|| {
    TABLE_TEXT
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut parts = line.split('\t');
            let name = parts.next().unwrap();
            let mut index = || parts.next().unwrap().parse::<u32>().unwrap();
            (name, [index(), index()])
        })
        .collect()
});

/// The Java sprite names the table reads, for loading them from a pack.
pub(crate) fn sprite_names() -> impl Iterator<Item = &'static str> {
    CELLS.iter().map(|(name, _)| *name)
}

/// The finest scale a composed atlas takes (2048 pixels a side), so a pack of 256-pixel sprites
/// does not need a 4096-pixel atlas; larger sprites are shrunk to it.
const MAX_SCALE: u32 = 16;

/// Whether a sprite is a square picture of at least a cell's width.
fn usable(sprite: &RgbaImage) -> bool {
    sprite.width() == sprite.height() && sprite.width() >= CELL
}

/// The atlas of the cells `sprites` fill and the marker listing those cells. Its scale is the
/// finest sprite's, up to [`MAX_SCALE`]; each sprite is enlarged by whole pixels or resampled to
/// its cell. `None` when no sprite is a usable square.
pub(crate) fn compose(
    sprites: &HashMap<String, RgbaImage>,
    budget: &Budget,
) -> Option<(RgbaImage, Vec<u8>)> {
    let scale = CELLS
        .iter()
        .filter_map(|(name, _)| sprites.get(*name).filter(|sprite| usable(sprite)))
        .map(|sprite| sprite.width().div_ceil(CELL))
        .max()?
        .min(MAX_SCALE);
    let mut atlas = canvas(ATLAS_SIDE * scale, ATLAS_SIDE * scale, budget)?;
    let mut filled = Vec::new();
    let side = CELL * scale;
    for (name, [column, row]) in CELLS.iter() {
        let Some(sprite) = sprites.get(*name).filter(|sprite| usable(sprite)) else {
            continue;
        };
        let cell = if sprite.width() == side {
            Cow::Borrowed(sprite)
        } else {
            let filter = if side.is_multiple_of(sprite.width()) {
                imageops::Nearest
            } else {
                imageops::Triangle
            };
            Cow::Owned(imageops::resize(sprite, side, side, filter))
        };
        paste(&mut atlas, &cell, column * side, row * side);
        filled.push([*column, *row]);
    }
    let marker = serde_json::to_vec(&serde_json::json!({ "cells": filled })).ok()?;
    Some((atlas, marker))
}

/// An RGBA image: width, height and row-major pixels.
pub type Pixels<'a> = (u32, u32, &'a [u8]);

/// The cells listed in an overlay marker; `None` when it is not one.
fn parse_marker(marker: &[u8]) -> Option<Vec<[u32; 2]>> {
    let value: serde_json::Value = serde_json::from_slice(marker).ok()?;
    value["cells"]
        .as_array()?
        .iter()
        .map(|cell| {
            let [column, row] = [cell[0].as_u64()?, cell[1].as_u64()?];
            Some([u32::try_from(column).ok()?, u32::try_from(row).ok()?])
        })
        .collect()
}

/// `base` with the cells `marker` lists replaced by the same cells of `over`, both atlases
/// square multiples of the vanilla side. The result has the finer of the two scales, the coarser
/// atlas enlarged by whole pixels. `None` when either atlas is not such a multiple or the scales
/// do not divide, so the caller keeps its own.
#[must_use]
pub fn overlay(base: Pixels, over: Pixels, marker: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let cells = parse_marker(marker)?;
    let scale_of = |(width, height, pixels): Pixels| {
        (width == height
            && width >= ATLAS_SIDE
            && width.is_multiple_of(ATLAS_SIDE)
            && pixels.len() == (width as usize) * (height as usize) * 4)
            .then_some(width / ATLAS_SIDE)
    };
    let (base_scale, over_scale) = (scale_of(base)?, scale_of(over)?);
    let scale = base_scale.max(over_scale);
    if !scale.is_multiple_of(base_scale) || !scale.is_multiple_of(over_scale) {
        return None;
    }
    let side = ATLAS_SIDE * scale;
    let enlarge = |(width, height, pixels): Pixels| {
        let image = RgbaImage::from_raw(width, height, pixels.to_vec())?;
        Some(if width == side {
            image
        } else {
            imageops::resize(&image, side, side, imageops::Nearest)
        })
    };
    let mut out = enlarge(base)?;
    let over = enlarge(over)?;
    let cell = CELL * scale;
    for [column, row] in cells {
        if (u64::from(column) + 1) * u64::from(cell) > u64::from(side)
            || (u64::from(row) + 1) * u64::from(cell) > u64::from(side)
        {
            continue;
        }
        let part = imageops::crop_imm(&over, column * cell, row * cell, cell, cell).to_image();
        paste(&mut out, &part, column * cell, row * cell);
    }
    Some((side, side, out.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn sprites(side: u32, names: &[&str]) -> HashMap<String, RgbaImage> {
        names
            .iter()
            .map(|name| {
                (
                    (*name).to_owned(),
                    RgbaImage::from_pixel(side, side, Rgba([9, 8, 7, 255])),
                )
            })
            .collect()
    }

    #[test]
    fn every_cell_is_inside_the_atlas_and_listed_once() {
        let mut cells: Vec<[u32; 2]> = CELLS.iter().map(|(_, cell)| *cell).collect();
        let count = cells.len();
        cells.sort_unstable();
        cells.dedup();
        assert_eq!(cells.len(), count);
        assert!(
            cells
                .iter()
                .all(|[column, row]| *column.max(row) < ATLAS_SIDE / CELL)
        );
    }

    #[test]
    fn a_sprite_fills_only_its_cell_at_the_finest_scale() {
        let budget = Budget::new();
        let mut found = sprites(8, &["flame"]);
        found.extend(sprites(16, &["heart"]));
        let (atlas, marker) = compose(&found, &budget).unwrap();
        assert_eq!(atlas.dimensions(), (256, 256));
        // Flame is cell (0, 3) and was doubled; heart is (0, 5).
        assert_eq!(atlas.get_pixel(0, 3 * 16).0, [9, 8, 7, 255]);
        assert_eq!(atlas.get_pixel(31, 3 * 16 + 31).0, [0; 4]);
        assert_eq!(atlas.get_pixel(0, 5 * 16).0, [9, 8, 7, 255]);
        assert_eq!(atlas.get_pixel(0, 0).0, [0; 4]);
        assert_eq!(parse_marker(&marker).unwrap(), vec![[0, 3], [0, 5]]);
    }

    #[test]
    fn huge_and_odd_sized_sprites_are_resampled_to_a_bounded_atlas() {
        let budget = Budget::new();
        let mut found = sprites(256, &["flame"]);
        found.extend(sprites(55, &["heart"]));
        let (atlas, _) = compose(&found, &budget).unwrap();
        assert_eq!(atlas.dimensions(), (2048, 2048));
        assert_eq!(atlas.get_pixel(0, 3 * 128).0, [9, 8, 7, 255]);
        assert_eq!(atlas.get_pixel(64, 5 * 128 + 64).0, [9, 8, 7, 255]);
    }

    #[test]
    fn no_usable_sprite_gives_no_atlas() {
        let budget = Budget::new();
        assert!(compose(&sprites(8, &["not_a_particle"]), &budget).is_none());
        let mut wide = HashMap::new();
        wide.insert("flame".to_owned(), RgbaImage::new(8, 16));
        assert!(compose(&wide, &budget).is_none());
        assert!(compose(&sprites(4, &["flame"]), &budget).is_none());
        assert!(compose(&HashMap::new(), &budget).is_none());
    }

    #[test]
    fn the_overlay_replaces_listed_cells_and_keeps_the_rest_of_the_base() {
        let base = vec![1u8; 128 * 128 * 4];
        let mut over = vec![0u8; 256 * 256 * 4];
        for (index, byte) in over.iter_mut().enumerate() {
            *byte = [200, 100, 50, 255][index % 4];
        }
        let (width, height, pixels) = overlay(
            (128, 128, &base),
            (256, 256, &over),
            br#"{"cells":[[2,1]]}"#,
        )
        .unwrap();
        assert_eq!((width, height), (256, 256));
        let at = |x: u32, y: u32| {
            let offset = ((y * 256 + x) * 4) as usize;
            pixels[offset..offset + 4].to_vec()
        };
        assert_eq!(at(2 * 16, 16), [200, 100, 50, 255]);
        assert_eq!(at(2 * 16 + 15, 16 + 15), [200, 100, 50, 255]);
        assert_eq!(at(0, 0), [1; 4]);
        assert_eq!(at(3 * 16, 16), [1; 4]);
    }

    #[test]
    fn atlases_that_do_not_divide_or_markers_that_are_not_markers_are_refused() {
        let base = vec![1u8; 128 * 128 * 4];
        let three = vec![1u8; 384 * 384 * 4];
        let two = vec![1u8; 256 * 256 * 4];
        assert!(overlay((256, 256, &two), (384, 384, &three), br#"{"cells":[]}"#).is_none());
        assert!(overlay((128, 128, &base), (128, 128, &base), b"nope").is_none());
        assert!(overlay((128, 128, &base), (128, 128, &base), br#"{"cells":[]}"#).is_some());
    }
}
