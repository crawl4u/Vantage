//! Authored carried block faces shared by inventory baking and held cube rendering.

use super::{AssetError, IconSprite, invalid};
use crate::BlockVisualId;

pub const BLOCK_ITEM_FACE_SIDE: u16 = 16;
pub const BLOCK_ITEM_SHEET_GRID: [u16; 2] = [3, 2];
pub const BLOCK_ITEM_SHEET_SIZE: [u16; 2] = [
    BLOCK_ITEM_FACE_SIDE * BLOCK_ITEM_SHEET_GRID[0],
    BLOCK_ITEM_FACE_SIDE * BLOCK_ITEM_SHEET_GRID[1],
];
pub const MAX_ICON_BLOCK_SHEETS: usize = 2048;
/// The largest face a carried sheet holds; a pack's finer faces are filtered down to it.
pub const MAX_BLOCK_ITEM_FACE_SIDE: u16 = 64;

/// The face side of a carried sheet of `width` by `height` texels: three faces across and two
/// down, each a power of two from the base side up to [`MAX_BLOCK_ITEM_FACE_SIDE`].
#[must_use]
pub const fn block_item_face_side(width: u32, height: u32) -> Option<u32> {
    let side = width / BLOCK_ITEM_SHEET_GRID[0] as u32;
    if side >= BLOCK_ITEM_FACE_SIDE as u32
        && side <= MAX_BLOCK_ITEM_FACE_SIDE as u32
        && side.is_power_of_two()
        && width == side * BLOCK_ITEM_SHEET_GRID[0] as u32
        && height == side * BLOCK_ITEM_SHEET_GRID[1] as u32
    {
        Some(side)
    } else {
        None
    }
}

/// The size of a carried sheet whose faces are `side` texels across.
#[must_use]
pub const fn block_item_sheet_size(side: u16) -> [u16; 2] {
    [
        side * BLOCK_ITEM_SHEET_GRID[0],
        side * BLOCK_ITEM_SHEET_GRID[1],
    ]
}

/// A six-face carried sheet in the ordinary sprite pool, indexed by compiled block visual.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IconBlockSheet {
    pub visual: BlockVisualId,
    pub sprite: u32,
}

/// A thumbnail bound to the world state whose GUI quads the runtime draws at display size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IconBlockModel {
    pub sprite: u32,
    pub visual: BlockVisualId,
    /// Independent carried artwork when the GUI model replaces a flat item preview.
    pub held_sprite: Option<u32>,
}

/// Checks that model bindings are bounded, sorted and reference existing sprites.
pub(super) fn validate_models(
    models: &[IconBlockModel],
    sprites: &[IconSprite],
) -> Result<(), AssetError> {
    if models.len() > super::MAX_ICON_SPRITES {
        return Err(invalid("block model thumbnail count exceeds bound"));
    }
    let mut previous = None;
    for model in models {
        if previous.is_some_and(|sprite| sprite >= model.sprite) {
            return Err(invalid("block model thumbnails are not strictly sorted"));
        }
        if model.sprite as usize >= sprites.len() {
            return Err(invalid("block model thumbnail references a missing sprite"));
        }
        if model
            .held_sprite
            .is_some_and(|sprite| sprite as usize >= sprites.len())
        {
            return Err(invalid("held block sprite references a missing sprite"));
        }
        previous = Some(model.sprite);
    }
    Ok(())
}

pub(super) fn validate(
    sheets: &[IconBlockSheet],
    sprites: &[IconSprite],
) -> Result<(), AssetError> {
    if sheets.len() > MAX_ICON_BLOCK_SHEETS {
        return Err(invalid("carried block sheet count exceeds bound"));
    }
    let mut previous = None;
    for sheet in sheets {
        if previous.is_some_and(|visual| visual >= sheet.visual.0) {
            return Err(invalid("carried block sheets are not strictly sorted"));
        }
        let sprite = sprites
            .get(sheet.sprite as usize)
            .ok_or_else(|| invalid("carried block sheet references a missing sprite"))?;
        if block_item_face_side(u32::from(sprite.width), u32::from(sprite.height)).is_none() {
            return Err(invalid("carried block sheet has invalid dimensions"));
        }
        previous = Some(sheet.visual.0);
    }
    Ok(())
}

/// Packs faces in `BlockFace::ALL` order without tinting, reshading or changing alpha. Every
/// face is the same valid square side. Color/mask resolution belongs to the authored
/// carried-texture compiler.
#[must_use]
pub fn compose_block_item_sheet(tiles: &[IconSprite; 6]) -> Option<IconSprite> {
    let side16 = tiles[0].width;
    let side = usize::from(side16);
    if block_item_face_side(u32::from(side16) * 3, u32::from(side16) * 2).is_none()
        || tiles.iter().any(|tile| {
            tile.width != side16 || tile.height != side16 || tile.rgba8.len() != side * side * 4
        })
    {
        return None;
    }
    let [width, height] = block_item_sheet_size(side16);
    let mut rgba8 = vec![0; usize::from(width) * usize::from(height) * 4];
    let columns = usize::from(BLOCK_ITEM_SHEET_GRID[0]);
    for (face, tile) in tiles.iter().enumerate() {
        let (x, y) = (face % columns * side, face / columns * side);
        for row in 0..side {
            let target = ((y + row) * usize::from(width) + x) * 4;
            rgba8[target..target + side * 4]
                .copy_from_slice(&tile.rgba8[row * side * 4..(row + 1) * side * 4]);
        }
    }
    Some(IconSprite {
        width,
        height,
        rgba8: rgba8.into(),
    })
}
