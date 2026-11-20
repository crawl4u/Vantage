//! Inventory slot textures: Java's `gui/container/inventory.png` and the hotbar selection become
//! the slot cells and selection highlight Bedrock's inventory screens draw, and the cell variants
//! Java has no picture for (dark, inverted, red and locked cells) are made from the pack's own cell
//! so every state of a slot matches the pack.

use crate::texture::{Budget, canvas, crop, duplicate, is_empty, over};
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

/// Single pictures a Java GUI sprite (relative to `textures/gui/sprites/`) replaces: a
/// container's progress and error marks, which Java draws the empty halves of into the
/// container's own atlas, and the backgrounds behind a status effect. Each has its Bedrock
/// texture and, for a nine-slice one, the sidecar describing it.
pub(crate) const SPRITES: [(&str, &str, Option<&[u8]>); 10] = [
    (
        "container/furnace/lit_progress",
        "textures/ui/flame_full_image",
        None,
    ),
    (
        "container/furnace/burn_progress",
        "textures/ui/arrow_active",
        None,
    ),
    (
        "container/brewing_stand/brew_progress",
        "textures/ui/brewing_arrow_full",
        None,
    ),
    (
        "container/brewing_stand/bubbles",
        "textures/ui/bubbles_full",
        None,
    ),
    (
        "container/brewing_stand/fuel_length",
        "textures/ui/brewing_fuel_bar_full",
        None,
    ),
    ("container/anvil/error", "textures/ui/crossout", None),
    (
        "hud/effect_background",
        "textures/ui/hud_mob_effect_background",
        None,
    ),
    (
        "hud/effect_background_ambient",
        "textures/ui/hud_mob_ambient_effect_background",
        None,
    ),
    (
        "container/inventory/effect_background_large",
        "textures/ui/effect_background",
        Some(EFFECT_SIDECAR),
    ),
    (
        "container/inventory/effect_background_large",
        "textures/ui/effect_background_ambient",
        Some(EFFECT_SIDECAR),
    ),
];

/// The sidecar of the effect list's backgrounds: a four-pixel border round a 120 by 32 Java
/// background.
const EFFECT_SIDECAR: &[u8] = br#"{"nineslice_size":4,"base_size":[120,32]}"#;

/// Java's creative category tabs (relative to `textures/gui/sprites/`) and the Bedrock tab
/// textures each one draws: Java's first tab is the left-most, its last the right-most, and the
/// ones between are all the same. Java has no hover picture, so a hovered tab is the plain one.
pub(crate) const TAB_SPRITES: [(&str, &[&str]); 6] = [
    (
        "container/creative_inventory/tab_top_selected_1",
        &[
            "textures/ui/TabTopFrontLeftMost",
            "textures/ui/TabTopFrontLeftMostHover",
        ],
    ),
    (
        "container/creative_inventory/tab_top_selected_2",
        &["textures/ui/TabTopFront", "textures/ui/TabTopFrontHover"],
    ),
    (
        "container/creative_inventory/tab_top_selected_7",
        &[
            "textures/ui/TabTopFrontRightMost",
            "textures/ui/TabTopFrontRightMostHover",
        ],
    ),
    (
        "container/creative_inventory/tab_top_unselected_1",
        &[
            "textures/ui/TabTopBackLeftMost",
            "textures/ui/TabTopBackLeftMostHover",
        ],
    ),
    (
        "container/creative_inventory/tab_top_unselected_2",
        &["textures/ui/TabTopBack", "textures/ui/TabTopBackHover"],
    ),
    (
        "container/creative_inventory/tab_top_unselected_7",
        &[
            "textures/ui/TabTopBackRightMost",
            "textures/ui/TabTopBackRightMostHover",
            "textures/ui/TabTopBackRightMostDark",
            "textures/ui/TabTopBackRightMostDarkHover",
        ],
    ),
];

/// The sidecar of a tab texture: a four-pixel border round a 26 by 32 Java tab.
pub(crate) const TAB_SIDECAR: &[u8] = br#"{"nineslice_size":4,"base_size":[26,32]}"#;

/// Bedrock's panel, which every container screen and dialog draws behind its controls.
pub(crate) const PANEL: &str = "textures/ui/dialog_background_opaque.png";

/// Textures every screen shares, each with the copy only container screens draw instead. The
/// copies keep a see-through pack's frames, cells and player box see-through where the shared
/// ones would make every dialog hard to read. Keys have no extension.
pub const CONTAINER_SWAPS: [(&str, &str); 9] = [
    (
        "textures/ui/dialog_background_opaque",
        "textures/ui/dialog_background_container",
    ),
    (
        "textures/ui/recipe_book_pane_bg",
        "textures/ui/recipe_book_pane_bg_container",
    ),
    (
        "textures/ui/recipe_book_item_bg",
        "textures/ui/recipe_book_item_bg_container",
    ),
    ("textures/ui/Black", "textures/ui/black_container"),
    (
        "textures/ui/player_preview_border",
        "textures/ui/player_preview_border_container",
    ),
    (
        "textures/ui/recipe_book_light_button",
        "textures/ui/recipe_book_light_button_container",
    ),
    (
        "textures/ui/recipe_book_dark_button",
        "textures/ui/recipe_book_dark_button_container",
    ),
    (
        "textures/ui/recipe_book_dark_button_pressed",
        "textures/ui/recipe_book_dark_button_pressed_container",
    ),
    (
        "textures/ui/recipe_book_red_button",
        "textures/ui/recipe_book_red_button_container",
    ),
];

/// The container-only panel's file.
pub(crate) const CONTAINER_PANEL: &str = "textures/ui/dialog_background_container.png";

/// The panel's sidecar: a four-pixel frame round a flat middle, over the Java panel's own 176 by
/// 166 size.
pub(crate) const PANEL_SIDECAR: &[u8] = br#"{"nineslice_size":4,"base_size":[176,166]}"#;

/// The Java container panel's size, and how wide its frame is kept when the middle is cleared.
const PANEL_SIZE: [u32; 2] = [176, 166];
const PANEL_FRAME: u32 = 4;
/// The fill of a classic Java panel, used when a pack's own panel has none.
const CLASSIC_FILL: [u8; 4] = [198, 198, 198, 255];

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

/// The Bedrock panel for a Java container atlas: its frame, round a flat middle. With `filled`, the
/// middle is opaque: the pack's own fill when its panel has one, else Java's classic grey, so a
/// pack whose containers are see-through keeps its frame without making every dialog hard to
/// read. Without it the middle is the pack's own, see-through when the pack's is. Slots and
/// pictures drawn inside the panel are cleared. `None` when the atlas has no panel.
pub(crate) fn panel(atlas: &RgbaImage, budget: &Budget, filled: bool) -> Option<RgbaImage> {
    let scale = (atlas.width() / 256).max(1);
    let frame = PANEL_FRAME * scale;
    let [width, height] = PANEL_SIZE.map(|side| side * scale);
    let region = crop(atlas, 0, 0, width, height, budget)?;
    if is_empty(&region) {
        return None;
    }
    let sample = region.get_pixel(frame, frame).0;
    let fill = if sample[3] >= 250 {
        sample
    } else if filled {
        CLASSIC_FILL
    } else {
        [0; 4]
    };
    // The fill goes under everything but the clear pixels that reach the panel's outer edge:
    // under a frame with rounded or notched corners it would show as square corners behind them.
    let outside = clear_outside(&region, frame);
    let mut panel = canvas(width, height, budget)?;
    for (x, y, pixel) in panel.enumerate_pixels_mut() {
        if !outside[(y * width + x) as usize] {
            pixel.0 = fill;
        }
    }
    // Only the frame is kept; the middle stays the flat fill.
    let mut ring = duplicate(&region, budget)?;
    for y in frame..height - frame {
        for x in frame..width - frame {
            ring.put_pixel(x, y, image::Rgba([0; 4]));
        }
    }
    over(&panel, &ring, budget)
}

/// Which pixels of `image` are fully clear and joined to its edge through clear pixels within
/// `frame` pixels of it, row by row. The middle never counts, so a panel with no frame art along an
/// edge still has its fill.
fn clear_outside(image: &RgbaImage, frame: u32) -> Vec<bool> {
    let (width, height) = image.dimensions();
    let mut outside = vec![false; (width * height) as usize];
    let mut pending: Vec<(u32, u32)> = Vec::new();
    for x in 0..width {
        pending.extend([(x, 0), (x, height - 1)]);
    }
    for y in 0..height {
        pending.extend([(0, y), (width - 1, y)]);
    }
    while let Some((x, y)) = pending.pop() {
        let slot = (y * width + x) as usize;
        let in_frame = x < frame || y < frame || x + frame >= width || y + frame >= height;
        if !in_frame || outside[slot] || image.get_pixel(x, y).0[3] != 0 {
            continue;
        }
        outside[slot] = true;
        if x > 0 {
            pending.push((x - 1, y));
        }
        if y > 0 {
            pending.push((x, y - 1));
        }
        if x + 1 < width {
            pending.push((x + 1, y));
        }
        if y + 1 < height {
            pending.push((x, y + 1));
        }
    }
    outside
}

/// The container-only copies a Java container atlas yields, each with its path and, for the nine-slice
/// ones, the sidecar describing it. They follow the container panel: where the pack's is see-through
/// so are the recipe list's backdrop and the player's box, and the recipe list's item cell is the
/// pack's own slot cell.
pub(crate) fn container_swaps(
    atlas: &RgbaImage,
    budget: &Budget,
) -> Vec<(String, RgbaImage, Option<&'static [u8]>)> {
    let scale = (atlas.width() / 256).max(1);
    let mut found = Vec::new();
    let paths = |index: usize| format!("{}.png", CONTAINER_SWAPS[index].1);
    if let Some(panel) = panel(atlas, budget, false) {
        let middle = *panel.get_pixel(panel.width() / 2, panel.height() / 2);
        if let Some(mut pane) = canvas(1, 1, budget) {
            pane.put_pixel(0, 0, middle);
            found.push((paths(1), pane, None));
        }
    }
    if let Some(cell) = slot_cell(atlas, budget) {
        found.push((paths(2), cell, Some(CELL_SIDECAR)));
    }
    // The recipe list's buttons (lit, dark, pressed and red) are the pack's own slot. Java has no
    // pictures for the states, and the list's "+" already marks a group, so none is washed or
    // tinted: a pack that wants them to differ ships its own pictures.
    if let Some(cell) = slot_cell(atlas, budget) {
        for index in 5..=8 {
            if let Some(button) = duplicate(&cell, budget) {
                found.push((paths(index), button, Some(CELL_SIDECAR)));
            }
        }
    }
    // The box the player stands in: its middle decides what Black becomes, its edge is the frame.
    // Bedrock draws Black 52 by 70 inside the box, but a Java box may be narrower (vanilla's is 49
    // wide), so Black is as big as Bedrock draws it and filled only where the pack's box is.
    let [box_x, box_y, box_width, box_height] = PLAYER_BOX.map(|value| value * scale);
    if let Some(player) = crop(atlas, box_x, box_y, box_width, box_height, budget) {
        let middle = *player.get_pixel(box_width / 2, box_height / 2);
        let (inner_width, inner_height) = (box_width - 2 * scale, box_height - 2 * scale);
        // A see-through box makes Black clear; otherwise it is the middle where the pack's box is.
        let size = if middle.0[3] >= 250 {
            (inner_width, inner_height)
        } else {
            (1, 1)
        };
        if let Some(mut black) = canvas(size.0, size.1, budget) {
            if middle.0[3] >= 250 {
                for y in 0..inner_height {
                    for x in 0..inner_width {
                        if *player.get_pixel(x + scale, y + scale) == middle {
                            black.put_pixel(x, y, middle);
                        }
                    }
                }
            }
            found.push((paths(3), black, None));
        }
        // The offhand slot's cell begins inside this crop's bottom right corner; it is not the
        // box's frame, and the player's Black no longer covers it.
        let mut player = player;
        for y in OFFHAND_CELL[1].saturating_sub(PLAYER_BOX[1]) * scale..box_height {
            for x in OFFHAND_CELL[0].saturating_sub(PLAYER_BOX[0]) * scale..box_width {
                player.put_pixel(x, y, image::Rgba([0; 4]));
            }
        }
        if !is_empty(&player) {
            found.push((paths(4), player, Some(PLAYER_BOX_SIDECAR)));
        }
    }
    found
}

/// The player's box in a 256-pixel container atlas: its left, top, width and height, and the
/// sidecar of its frame.
const PLAYER_BOX: [u32; 4] = [25, 7, 54, 72];
/// Where the offhand slot's cell starts in the atlas, which the player box's crop overlaps.
const OFFHAND_CELL: [u32; 2] = [76, 61];
const PLAYER_BOX_SIDECAR: &[u8] = br#"{"nineslice_size":1,"base_size":[54,72]}"#;

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

    /// A 256-pixel atlas with a one-pixel frame round a panel and a slot cell inside it.
    fn panelled(fill: [u8; 4]) -> RgbaImage {
        let mut atlas = atlas();
        for i in 0..PANEL_SIZE[0] {
            atlas.put_pixel(i, 0, Rgba([0, 0, 0, 255]));
            atlas.put_pixel(i, PANEL_SIZE[1] - 1, Rgba([0, 0, 0, 255]));
        }
        for i in 0..PANEL_SIZE[1] {
            atlas.put_pixel(0, i, Rgba([0, 0, 0, 255]));
            atlas.put_pixel(PANEL_SIZE[0] - 1, i, Rgba([0, 0, 0, 255]));
        }
        if fill[3] != 0 {
            for y in 1..PANEL_SIZE[1] - 1 {
                for x in 1..PANEL_SIZE[0] - 1 {
                    if atlas.get_pixel(x, y).0[3] == 0 {
                        atlas.put_pixel(x, y, Rgba(fill));
                    }
                }
            }
        }
        atlas
    }

    #[test]
    fn an_opaque_panel_keeps_its_frame_and_fill_and_loses_what_was_drawn_inside_it() {
        let budget = Budget::new();
        let panel = panel(&panelled([150, 160, 170, 255]), &budget, true).unwrap();
        assert_eq!(panel.dimensions(), (176, 166));
        assert_eq!(panel.get_pixel(0, 0).0, [0, 0, 0, 255]);
        // The slot outline inside the panel is gone, replaced by the pack's fill.
        assert_eq!(panel.get_pixel(SLOT_X, SLOT_Y).0, [150, 160, 170, 255]);
        assert_eq!(panel.get_pixel(88, 83).0, [150, 160, 170, 255]);
    }

    #[test]
    fn a_see_through_panel_keeps_its_frame_over_java_grey() {
        let budget = Budget::new();
        let panel = panel(&panelled([0; 4]), &budget, true).unwrap();
        assert_eq!(panel.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(panel.get_pixel(88, 83).0, CLASSIC_FILL);
        for pixel in panel.pixels() {
            assert_eq!(pixel.0[3], 255, "a dialog behind text is never see-through");
        }
    }

    #[test]
    fn rounded_corners_stay_clear_instead_of_showing_the_fill_square() {
        let budget = Budget::new();
        let mut atlas = panelled([150, 160, 170, 255]);
        // Round the top left corner: the corner pixel and its neighbours are clear.
        for (x, y) in [(0, 0), (1, 0), (0, 1)] {
            atlas.put_pixel(x, y, Rgba([0; 4]));
        }
        // An opaque panel and the container's own copy both keep the corner clear.
        for filled in [true, false] {
            let panel = panel(&atlas, &budget, filled).unwrap();
            assert_eq!(panel.get_pixel(0, 0).0[3], 0, "corner, filled {filled}");
            assert_eq!(panel.get_pixel(1, 0).0[3], 0);
            assert_eq!(panel.get_pixel(2, 2).0, [150, 160, 170, 255]);
            assert_eq!(panel.get_pixel(5, 0).0, [0, 0, 0, 255]);
        }
    }

    #[test]
    fn a_player_box_narrower_than_bedrocks_draws_black_only_where_it_is() {
        let budget = Budget::new();
        let mut atlas = RgbaImage::new(256, 256);
        // Vanilla's box: black 49 wide and 70 tall at 26, 8, inside a lighter frame.
        for y in 6..80 {
            for x in 24..80 {
                atlas.put_pixel(x, y, Rgba([198, 198, 198, 255]));
            }
        }
        for y in 8..78 {
            for x in 26..75 {
                atlas.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        // The offhand slot's cell, whose corner the box's crop overlaps.
        atlas.put_pixel(77, 70, Rgba([90, 90, 90, 255]));
        let swaps = container_swaps(&atlas, &budget);
        let frame = swaps
            .iter()
            .find(|(path, ..)| path.ends_with("player_preview_border_container.png"))
            .map(|(_, image, _)| image)
            .unwrap();
        assert_eq!(
            frame.get_pixel(77 - 25, 70 - 7).0[3],
            0,
            "not part of the frame"
        );
        assert_eq!(
            frame.get_pixel(1, 1).0,
            [0, 0, 0, 255],
            "the box's own pixels stay"
        );
        let black = swaps
            .iter()
            .find(|(path, ..)| path.ends_with("black_container.png"))
            .map(|(_, image, _)| image)
            .unwrap();
        assert_eq!(black.dimensions(), (52, 70));
        assert_eq!(black.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(black.get_pixel(48, 69).0, [0, 0, 0, 255]);
        assert_eq!(black.get_pixel(49, 30).0[3], 0, "past the pack's own box");
        assert_eq!(black.get_pixel(51, 69).0[3], 0);
    }

    #[test]
    fn the_container_panel_stays_see_through_where_the_packs_is() {
        let budget = Budget::new();
        let open = panel(&panelled([0; 4]), &budget, false).unwrap();
        assert_eq!(open.get_pixel(0, 0).0, [0, 0, 0, 255], "the frame is kept");
        assert_eq!(open.get_pixel(88, 83).0[3], 0, "the middle is see-through");
        let opaque = panel(&panelled([150, 160, 170, 255]), &budget, false).unwrap();
        assert_eq!(opaque.get_pixel(88, 83).0, [150, 160, 170, 255]);
    }

    #[test]
    fn every_recipe_button_state_is_the_slot_itself() {
        let budget = Budget::new();
        let swaps = container_swaps(&panelled([0; 4]), &budget);
        let cell = slot_cell(&panelled([0; 4]), &budget).unwrap();
        for name in [
            "recipe_book_light_button_container.png",
            "recipe_book_dark_button_container.png",
            "recipe_book_dark_button_pressed_container.png",
            "recipe_book_red_button_container.png",
        ] {
            let button = swaps
                .iter()
                .find(|(path, ..)| path.ends_with(name))
                .map(|(_, image, _)| image)
                .unwrap();
            assert_eq!(
                button.as_raw(),
                cell.as_raw(),
                "{name} is not washed or tinted"
            );
        }
    }

    #[test]
    fn sprites_name_distinct_bedrock_paths_in_the_ui_folder() {
        let mut bedrock: Vec<&str> = SPRITES.iter().map(|(_, to, _)| *to).collect();
        bedrock.sort_unstable();
        bedrock.dedup();
        assert_eq!(bedrock.len(), SPRITES.len());
        for (_, to, _) in SPRITES {
            assert!(to.starts_with("textures/ui/"));
        }
    }

    #[test]
    fn every_tab_texture_is_drawn_by_one_sprite() {
        let mut paths: Vec<&str> = TAB_SPRITES
            .iter()
            .flat_map(|(_, to)| to.iter().copied())
            .collect();
        let count = paths.len();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(paths.len(), count);
        assert!(
            paths
                .iter()
                .all(|path| path.starts_with("textures/ui/TabTop"))
        );
    }

    #[test]
    fn an_atlas_without_a_panel_gives_none() {
        assert!(panel(&RgbaImage::new(256, 256), &Budget::new(), true).is_none());
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
