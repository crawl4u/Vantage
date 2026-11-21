use super::*;
use crate::{
    hud::{LEGACY_ICON_RECTS, Rect},
    texture::{Budget, encode},
};
use image::{Rgba, RgbaImage};
use std::collections::BTreeMap;

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];

/// A `width` x `height` PNG filled with `color`.
fn solid(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    encode(&RgbaImage::from_pixel(width, height, Rgba(color)))
}

/// A transparent `side`-pixel PNG with one `color` pixel in its top-left corner.
fn corner_dot(side: u32, color: [u8; 4]) -> Vec<u8> {
    let mut image = RgbaImage::new(side, side);
    image.put_pixel(0, 0, Rgba(color));
    encode(&image)
}

/// A transparent `size`-pixel atlas with each region, given in 256-pixel coordinates and scaled
/// with the atlas, filled with its colour.
fn marked_atlas(size: u32, regions: &[(Rect, [u8; 4])]) -> Vec<u8> {
    let scale = size / 256;
    let mut image = RgbaImage::new(size, size);
    for (region, color) in regions {
        for y in region.y * scale..(region.y + region.h) * scale {
            for x in region.x * scale..(region.x + region.w) * scale {
                image.put_pixel(x, y, Rgba(*color));
            }
        }
    }
    encode(&image)
}

/// The legacy icon region named `name`.
fn icon(name: &str) -> Rect {
    LEGACY_ICON_RECTS
        .iter()
        .find(|(icon, _)| *icon == name)
        .unwrap()
        .1
}

/// The runtime texture source limit the tests translate under.
const TEXTURE_LIMIT: u64 = 4 * 1024 * 1024;

/// Translates in-memory pack files.
fn run(files: &BTreeMap<String, Vec<u8>>, name: &str) -> Result<Translation, TranslateError> {
    let paths: Vec<String> = files.keys().cloned().collect();
    translate(&paths, |path| files.get(path).cloned(), name, TEXTURE_LIMIT)
}

/// Translated files keyed by path.
fn output(translation: &Translation) -> BTreeMap<&str, &[u8]> {
    translation
        .files
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
        .collect()
}

/// The size of a translated texture and its pixel at (`x`, `y`).
fn pixel(out: &BTreeMap<&str, &[u8]>, path: &str, x: u32, y: u32) -> ((u32, u32), [u8; 4]) {
    let bytes = out.get(path).unwrap_or_else(|| panic!("{path} missing"));
    let format = if path.ends_with(".tga") {
        image::ImageFormat::Tga
    } else {
        image::ImageFormat::Png
    };
    let image = image::load_from_memory_with_format(bytes, format)
        .unwrap()
        .into_rgba8();
    (image.dimensions(), image.get_pixel(x, y).0)
}

/// Builds a file map from string paths.
fn pack(files: Vec<(&str, Vec<u8>)>) -> BTreeMap<String, Vec<u8>> {
    files
        .into_iter()
        .map(|(path, bytes)| (path.to_owned(), bytes))
        .collect()
}

#[test]
fn a_legacy_pack_translates_textures_hud_potions_and_sky() {
    let icons = marked_atlas(
        512,
        &[
            (icon("crosshair"), WHITE),
            (icon("heart/full"), RED),
            (icon("food_full"), YELLOW),
        ],
    );
    let region = |x, y, w, h| Rect { x, y, w, h };
    let widgets = marked_atlas(
        256,
        &[
            (region(0, 0, 1, 22), RED),
            (region(1, 0, 20, 22), GREEN),
            (region(161, 0, 20, 22), BLUE),
            (region(0, 22, 24, 24), WHITE),
        ],
    );
    let textures = "assets/minecraft/textures/";
    let files = pack(vec![
        (
            "pack.mcmeta",
            "{\"pack\":{\"pack_format\":1,\"description\":\"\u{a7}6Legacy \u{a7}rPvP\"}}".into(),
        ),
        ("pack.png", solid(16, 16, BLUE)),
        (
            &format!("{textures}blocks/planks_oak.png"),
            solid(16, 16, GREEN),
        ),
        (
            &format!("{textures}blocks/destroy_stage_3.png"),
            solid(16, 16, GREEN),
        ),
        (
            &format!("{textures}items/fish_cod_raw.png"),
            solid(16, 16, BLUE),
        ),
        (
            &format!("{textures}items/potion_overlay.png"),
            solid(16, 16, WHITE),
        ),
        (
            &format!("{textures}items/potion_bottle_splash.png"),
            encode(&RgbaImage::new(16, 16)),
        ),
        (
            &format!("{textures}models/armor/diamond_layer_1.png"),
            solid(64, 32, BLUE),
        ),
        (
            &format!("{textures}models/armor/chainmail_layer_2.png"),
            solid(64, 32, RED),
        ),
        (&format!("{textures}gui/icons.png"), icons),
        (&format!("{textures}gui/widgets.png"), widgets),
        (
            &format!("{textures}particle/particles.png"),
            solid(128, 128, RED),
        ),
        (
            "assets/minecraft/mcpatcher/sky/world0/sky1.properties",
            b"source=./sky1.png\n".to_vec(),
        ),
        (
            "assets/minecraft/mcpatcher/sky/world0/sky1.png",
            solid(48, 32, BLUE),
        ),
        ("assets/minecraft/sounds/ignored.ogg", vec![1, 2, 3]),
    ]);
    let translation = run(&files, "Legacy PvP").unwrap();
    assert_eq!(translation.layout, Layout::Legacy);
    let out = output(&translation);

    let manifest: serde_json::Value = serde_json::from_slice(out["manifest.json"]).unwrap();
    assert_eq!(manifest["format_version"], 2);
    assert_eq!(manifest["header"]["name"], "Legacy PvP");
    assert_eq!(manifest["header"]["description"], "Legacy PvP");
    assert_eq!(
        manifest["header"]["min_engine_version"],
        serde_json::json!(MIN_ENGINE_VERSION)
    );
    assert_eq!(manifest["modules"][0]["type"], "resources");
    for path in [
        "pack_icon.png",
        "textures/blocks/planks_oak.png",
        "textures/environment/destroy_stage_3.png",
        "textures/items/fish_raw.png",
        "textures/models/armor/diamond_1.png",
        "textures/models/armor/chain_2.png",
        "textures/gui/icons.png",
        "textures/particle/particles.png",
        "textures/environment/overworld_cubemap/cubemap_0.png",
        "textures/environment/overworld_cubemap/cubemap_5.png",
    ] {
        assert!(out.contains_key(path), "{path} missing");
    }
    assert!(!out.keys().any(|path| path.contains("sounds")));

    let (_, color) = pixel(&out, "textures/items/potion_bottle_splash_heal.png", 8, 8);
    assert_eq!(
        color,
        [245, 35, 34, 255],
        "white overlay tinted with the heal colour"
    );
    assert!(!out.contains_key("textures/items/potion_bottle_heal.png"));

    assert_eq!(pixel(&out, "textures/ui/heart.png", 0, 0), ((18, 18), RED));
    assert_eq!(pixel(&out, "textures/ui/hunger_full.png", 5, 5).1, YELLOW);
    assert!(
        !out.contains_key("textures/ui/freeze_heart.png"),
        "empty atlas region exported"
    );

    // The hotbar goes out whole, as Java draws it, with its pieces blank beneath.
    assert_eq!(
        pixel(&out, "textures/ui/java_hotbar.png", 0, 0),
        ((182, 22), RED)
    );
    assert_eq!(pixel(&out, "textures/ui/java_hotbar.png", 1, 0).1, GREEN);
    assert_eq!(pixel(&out, "textures/ui/java_hotbar.png", 180, 21).1, BLUE);
    for (piece, size) in [
        ("hotbar_start_cap", (1, 22)),
        ("hotbar_0", (20, 22)),
        ("hotbar_8", (20, 22)),
        ("hotbar_end_cap", (1, 22)),
    ] {
        let (found, color) = pixel(&out, &format!("textures/ui/{piece}.png"), 0, 0);
        assert_eq!((found, color[3]), (size, 0), "{piece}");
    }
    assert_eq!(
        pixel(&out, "textures/ui/selected_hotbar_slot.png", 0, 0).0,
        (24, 24)
    );
}

#[test]
fn a_current_pack_translates_flattened_names_strips_sprites_and_equipment() {
    let sprites = "assets/minecraft/textures/gui/sprites/hud/";
    let selection = RgbaImage::from_fn(24, 23, |_, y| match y {
        0 => Rgba(WHITE),
        22 => Rgba(RED),
        _ => Rgba([0; 4]),
    });
    let strip = encode(&RgbaImage::from_fn(16, 64, |_, y| {
        Rgba(if y < 16 { RED } else { BLUE })
    }));
    let blocks = "assets/minecraft/textures/block/";
    let files = pack(vec![
        (
            "pack.mcmeta",
            br#"{"pack":{"description":{"text":"Current ","extra":[{"text":"Pack"}]},"min_format":[97,1]}}"#
                .to_vec(),
        ),
        (&format!("{blocks}oak_planks.png"), solid(16, 16, GREEN)),
        (&format!("{blocks}fire_0.png"), strip.clone()),
        (&format!("{blocks}stone.png"), strip),
        ("assets/minecraft/textures/item/golden_apple.png", solid(16, 16, YELLOW)),
        (&format!("{sprites}heart/full.png"), solid(9, 9, RED)),
        (&format!("{sprites}crosshair.png"), solid(15, 15, WHITE)),
        (&format!("{sprites}hotbar.png"), solid(182, 22, GREEN)),
        (&format!("{sprites}hotbar_selection.png"), encode(&selection)),
        (
            "assets/minecraft/textures/entity/equipment/humanoid_leggings/iron.png",
            solid(64, 32, WHITE),
        ),
        (
            "assets/minecraft/textures/entity/equipment/humanoid/turtle_scute.png",
            solid(64, 32, GREEN),
        ),
    ]);
    let translation = run(&files, "Current").unwrap();
    assert_eq!(translation.layout, Layout::Flattened);
    let out = output(&translation);
    let manifest: serde_json::Value = serde_json::from_slice(out["manifest.json"]).unwrap();
    assert_eq!(manifest["header"]["description"], "Current Pack");

    assert!(out.contains_key("textures/blocks/planks_oak.png"));
    assert!(out.contains_key("textures/items/apple_golden.png"));
    assert_eq!(
        pixel(&out, "textures/blocks/fire_0.png", 0, 63),
        ((16, 64), BLUE)
    );
    assert_eq!(
        pixel(&out, "textures/blocks/stone.png", 0, 0),
        ((16, 16), RED)
    );
    assert_eq!(pixel(&out, "textures/ui/heart.png", 0, 0), ((9, 9), RED));
    assert_eq!(
        pixel(&out, "textures/ui/java_hotbar.png", 90, 10),
        ((182, 22), GREEN)
    );
    let (size, top) = pixel(&out, "textures/ui/selected_hotbar_slot.png", 0, 23);
    assert_eq!(
        (size, top),
        ((24, 24), WHITE),
        "padded with the top border row"
    );
    assert_eq!(
        pixel(&out, "textures/gui/icons.png", 0, 0),
        ((256, 256), WHITE)
    );
    assert!(out.contains_key("textures/models/armor/iron_2.png"));
    assert!(out.contains_key("textures/models/armor/turtle_1.png"));
}

/// A vertical strip of `frames` square frames, each filled with a colour derived from its index.
fn strip(side: u32, frames: u32) -> Vec<u8> {
    encode(&RgbaImage::from_fn(side, side * frames, |_, y| {
        Rgba([(y / side) as u8, 0, 0, 255])
    }))
}

#[test]
fn pre_1_9_clock_and_compass_strips_become_the_bedrock_atlases() {
    let items = "assets/minecraft/textures/items/";
    let files = pack(vec![
        (
            "pack.mcmeta",
            br#"{"pack":{"pack_format":1,"description":"1.7"}}"#.to_vec(),
        ),
        (&format!("{items}clock.png"), strip(16, 64)),
        (&format!("{items}compass.png"), strip(16, 32)),
        (&format!("{items}bed.png"), solid(16, 16, RED)),
    ]);
    let translation = run(&files, "Legacy").unwrap();
    let out = output(&translation);
    assert_eq!(
        pixel(&out, "textures/items/watch_atlas.png", 0, 63 * 16),
        ((16, 1024), [63, 0, 0, 255])
    );
    assert_eq!(
        pixel(&out, "textures/items/compass_atlas.png", 0, 31 * 16),
        ((16, 512), [31, 0, 0, 255])
    );
    assert!(!out.contains_key("textures/items/clock.png"));
    assert!(!out.contains_key("textures/items/compass.png"));
    assert!(out.contains_key("textures/items/bed_red.png"));
}

#[test]
fn strips_with_another_frame_count_are_not_used_as_atlases() {
    let files = pack(vec![(
        "assets/minecraft/textures/items/clock.png",
        strip(16, 8),
    )]);
    let out_files = run(&files, "Legacy").unwrap();
    let out = output(&out_files);
    assert!(!out.contains_key("textures/items/watch_atlas.png"));
    let flattened = pack(vec![(
        "assets/minecraft/textures/item/clock.png",
        strip(16, 64),
    )]);
    let translation = run(&flattened, "Current").unwrap();
    assert!(!output(&translation).contains_key("textures/items/watch_atlas.png"));
}

#[test]
fn textures_in_subfolders_never_replace_vanilla_textures() {
    let files = pack(vec![
        (
            "assets/minecraft/textures/block/stone.png",
            solid(16, 16, RED),
        ),
        (
            "assets/minecraft/textures/block/variants/stone.png",
            solid(16, 16, BLUE),
        ),
        (
            "assets/minecraft/textures/item/custom/apple.png",
            solid(16, 16, BLUE),
        ),
    ]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    assert_eq!(pixel(&out, "textures/blocks/stone.png", 0, 0).1, RED);
    assert!(!out.contains_key("textures/items/apple.png"));
}

#[test]
fn partially_transparent_pixels_keep_their_alpha() {
    let translucent = encode(&RgbaImage::from_pixel(16, 16, Rgba([10, 20, 30, 254])));
    let files = pack(vec![
        (
            "assets/minecraft/textures/item/apple.png",
            translucent.clone(),
        ),
        (
            "assets/minecraft/textures/gui/icons.png",
            encode(&RgbaImage::from_pixel(256, 256, Rgba([1, 2, 3, 200]))),
        ),
    ]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    assert_eq!(out["textures/items/apple.png"], translucent.as_slice());
    assert_eq!(pixel(&out, "textures/ui/heart.png", 0, 0).1, [1, 2, 3, 200]);
}

#[test]
fn hostile_hud_sprites_cannot_request_huge_canvases() {
    let sprites = "assets/minecraft/textures/gui/sprites/hud/";
    let files = pack(vec![
        (&format!("{sprites}crosshair.png"), solid(16_380, 1, WHITE)),
        (
            &format!("{sprites}hotbar_selection.png"),
            solid(16_380, 1, WHITE),
        ),
        (
            "assets/minecraft/textures/block/stone.png",
            solid(16, 16, RED),
        ),
    ]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    assert!(!out.contains_key("textures/gui/icons.png"));
    assert!(!out.contains_key("textures/ui/selected_hotbar_slot.png"));
    assert!(out.contains_key("textures/blocks/stone.png"));
}

#[test]
fn textures_vanilla_ships_as_tga_are_written_as_tga() {
    let files = pack(vec![
        (
            "assets/minecraft/textures/block/oak_leaves.png",
            solid(16, 16, GREEN),
        ),
        (
            "assets/minecraft/textures/item/leather_helmet.png",
            solid(16, 16, RED),
        ),
        (
            "assets/minecraft/textures/block/stone.png",
            solid(16, 16, RED),
        ),
    ]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    assert_eq!(
        pixel(&out, "textures/blocks/leaves_oak.tga", 0, 0),
        ((16, 16), GREEN)
    );
    assert!(!out.contains_key("textures/blocks/leaves_oak.png"));
    assert!(
        !out.keys().any(|path| path.contains("leather_helmet")),
        "dye masks are left alone"
    );
}

#[test]
fn grass_sides_become_tint_masks() {
    let overlay = encode(&RgbaImage::from_fn(16, 16, |_, y| {
        Rgba(if y < 4 { [150, 150, 150, 255] } else { [0; 4] })
    }));
    let blocks = "assets/minecraft/textures/block/";
    let files = pack(vec![
        (
            &format!("{blocks}grass_block_side.png"),
            solid(16, 16, [120, 80, 50, 255]),
        ),
        (
            &format!("{blocks}grass_block_side_overlay.png"),
            overlay.clone(),
        ),
        (
            &format!("{blocks}grass_block_snow.png"),
            solid(16, 16, WHITE),
        ),
    ]);
    let out_files = run(&files, "Current").unwrap();
    let out = output(&out_files);
    assert_eq!(
        pixel(&out, "textures/blocks/grass_side.tga", 0, 0).1,
        [150, 150, 150, 255]
    );
    assert_eq!(
        pixel(&out, "textures/blocks/grass_side.tga", 0, 8).1,
        [120, 80, 50, 0]
    );
    assert_eq!(
        pixel(&out, "textures/blocks/grass_side_snowed.tga", 3, 3).1,
        [255, 255, 255, 0]
    );
    assert!(out.contains_key("textures/blocks/grass_side_carried.png"));

    let legacy_side_only = pack(vec![
        (
            "assets/minecraft/textures/blocks/grass_side.png",
            solid(16, 16, [120, 80, 50, 255]),
        ),
        (
            "assets/minecraft/textures/blocks/stone.png",
            solid(16, 16, RED),
        ),
    ]);
    let translation = run(&legacy_side_only, "Legacy").unwrap();
    let out = output(&translation);
    assert!(
        !out.keys()
            .any(|path| path.starts_with("textures/blocks/grass_side."))
    );
}

#[test]
fn default_potion_bottles_are_filled_with_water() {
    let items = "assets/minecraft/textures/item/";
    let files = pack(vec![
        (&format!("{items}potion_overlay.png"), solid(16, 16, WHITE)),
        (
            &format!("{items}potion.png"),
            encode(&RgbaImage::new(16, 16)),
        ),
    ]);
    let translation = run(&files, "Current").unwrap();
    let out = output(&translation);
    assert_eq!(
        pixel(&out, "textures/items/potion_bottle_drinkable.png", 8, 8).1,
        [56, 92, 196, 255]
    );
    assert_eq!(
        pixel(&out, "textures/items/potion_bottle_heal.png", 8, 8).1,
        [245, 35, 34, 255]
    );
}

#[test]
fn translations_beyond_the_pixel_budget_fail() {
    let files = pack(vec![
        (
            "assets/minecraft/textures/item/potion_overlay.png",
            solid(64, 64, WHITE),
        ),
        (
            "assets/minecraft/textures/item/potion.png",
            solid(64, 64, RED),
        ),
    ]);
    let paths: Vec<String> = files.keys().cloned().collect();
    let read = |path: &str| files.get(path).cloned();
    let small = Budget::with_pixels(64 * 64 * 10);
    assert_eq!(
        translate::translate_within(&paths, read, "Pack", TEXTURE_LIMIT, &small).unwrap_err(),
        TranslateError::TooLarge
    );
    assert!(run(&files, "Pack").is_ok());
}

#[test]
fn modern_crosshairs_replace_the_legacy_atlas_crosshair() {
    let textures = "assets/minecraft/textures/";
    let icons = marked_atlas(
        256,
        &[(icon("crosshair"), WHITE), (icon("heart/full"), RED)],
    );
    let files = pack(vec![
        (&format!("{textures}gui/icons.png"), icons),
        (
            &format!("{textures}gui/sprites/hud/crosshair.png"),
            solid(15, 15, BLUE),
        ),
    ]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    assert_eq!(
        pixel(&out, "textures/gui/icons.png", 0, 0),
        ((256, 256), BLUE)
    );
    assert_eq!(
        pixel(&out, "textures/gui/icons.png", 52, 0).1,
        RED,
        "the rest of the atlas stays"
    );
}

#[test]
fn bottles_without_an_overlay_are_not_published_as_filled_bottles() {
    let files = pack(vec![
        (
            "assets/minecraft/textures/item/potion.png",
            encode(&RgbaImage::new(16, 16)),
        ),
        (
            "assets/minecraft/textures/block/stone.png",
            solid(16, 16, RED),
        ),
    ]);
    let translation = run(&files, "Pack").unwrap();
    assert!(!output(&translation).contains_key("textures/items/potion_bottle_drinkable.png"));
}

#[test]
fn generated_textures_over_the_runtime_limit_are_left_out() {
    let files = pack(vec![
        (
            "assets/minecraft/textures/block/oak_leaves.png",
            solid(16, 16, GREEN),
        ),
        (
            "assets/minecraft/textures/block/stone.png",
            solid(16, 16, RED),
        ),
    ]);
    let paths: Vec<String> = files.keys().cloned().collect();
    let translation = translate(&paths, |path| files.get(path).cloned(), "Pack", 8).unwrap();
    let out = output(&translation);
    assert!(!out.contains_key("textures/blocks/leaves_oak.tga"));
    assert!(
        out.contains_key("textures/blocks/stone.png"),
        "copied sources are kept"
    );
    assert_eq!(translation.skipped, 1);

    let only_oversized = pack(vec![(
        "assets/minecraft/textures/block/oak_leaves.png",
        solid(16, 16, GREEN),
    )]);
    let paths: Vec<String> = only_oversized.keys().cloned().collect();
    assert_eq!(
        translate(&paths, |path| only_oversized.get(path).cloned(), "Pack", 8).unwrap_err(),
        TranslateError::NothingToTranslate
    );
}

#[test]
fn experience_bars_reach_the_hud_textures_as_plain_images() {
    let sprites = "assets/minecraft/textures/gui/sprites/hud/";
    let files = pack(vec![
        (
            &format!("{sprites}experience_bar_background.png"),
            solid(182, 5, GREEN),
        ),
        (
            &format!("{sprites}experience_bar_progress.png"),
            solid(182, 5, YELLOW),
        ),
    ]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    assert_eq!(
        pixel(&out, "textures/ui/experiencebarempty.png", 181, 4),
        ((182, 5), GREEN)
    );
    assert_eq!(
        pixel(&out, "textures/ui/experiencebarfull.png", 0, 0),
        ((182, 5), YELLOW)
    );
    for sidecar in [
        "textures/ui/experiencebarempty.json",
        "textures/ui/experiencebarfull.json",
    ] {
        let value: serde_json::Value = serde_json::from_slice(out[sidecar]).unwrap();
        assert_eq!(value, serde_json::json!({"base_size": [182, 5]}));
    }
    assert_eq!(pixel(&out, "textures/ui/experiencenub.png", 0, 0).1, [0; 4]);
}

#[test]
fn animated_potion_layers_compose_their_first_frame() {
    let items = "assets/minecraft/textures/item/";
    let overlay = encode(&RgbaImage::from_fn(16, 32, |_, y| {
        Rgba(if y < 16 { WHITE } else { BLUE })
    }));
    let files = pack(vec![
        (&format!("{items}potion_overlay.png"), overlay),
        (
            &format!("{items}potion.png"),
            encode(&RgbaImage::new(16, 16)),
        ),
    ]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    assert_eq!(
        pixel(&out, "textures/items/potion_bottle_heal.png", 8, 8),
        ((16, 16), [245, 35, 34, 255])
    );
}

#[test]
fn legacy_regions_left_transparent_stay_transparent() {
    let icons = marked_atlas(256, &[(icon("heart/full"), RED)]);
    let files = pack(vec![
        ("assets/minecraft/textures/gui/icons.png", icons),
        (
            "assets/minecraft/textures/gui/sprites/hud/heart/vehicle_full.png",
            solid(9, 9, BLUE),
        ),
    ]);
    let translation = run(&files, "Legacy").unwrap();
    let out = output(&translation);
    assert_eq!(pixel(&out, "textures/ui/hunger_full.png", 4, 4).1, [0; 4]);
    assert!(
        !out.contains_key("textures/ui/freeze_heart.png"),
        "frozen hearts predate 1.17 packs"
    );
    assert_eq!(pixel(&out, "textures/ui/horse_heart.png", 0, 0).1, BLUE);
}

#[test]
fn experience_bar_companions_follow_only_surviving_bars() {
    let sprites = "assets/minecraft/textures/gui/sprites/hud/";
    let noisy = encode(&RgbaImage::from_fn(182, 5, |x, y| {
        Rgba([(x * 7) as u8, (y * 31 + x) as u8, (x ^ y) as u8, 255])
    }));
    let files = pack(vec![
        (&format!("{sprites}experience_bar_background.png"), noisy),
        (
            "assets/minecraft/textures/block/stone.png",
            solid(16, 16, RED),
        ),
    ]);
    let paths: Vec<String> = files.keys().cloned().collect();
    let translation = translate(&paths, |path| files.get(path).cloned(), "Pack", 1024).unwrap();
    let out = output(&translation);
    assert!(!out.contains_key("textures/ui/experiencebarempty.png"));
    assert!(!out.contains_key("textures/ui/experiencebarempty.json"));
    assert!(!out.contains_key("textures/ui/experiencenub.png"));
}

#[test]
fn crosshairs_of_any_resolution_reach_the_icons_atlas() {
    let sprites = "assets/minecraft/textures/gui/sprites/hud/";
    for (side, atlas) in [(16, 256), (32, 512), (15, 256)] {
        let files = pack(vec![(
            &format!("{sprites}crosshair.png"),
            solid(side, side, BLUE),
        )]);
        let translation = run(&files, "Pack").unwrap();
        let out = output(&translation);
        assert_eq!(
            pixel(&out, "textures/gui/icons.png", 0, 0),
            ((atlas, atlas), BLUE)
        );
        assert_eq!(
            pixel(&out, "textures/gui/icons.png", atlas / 256 * 15, 0).1,
            [0; 4]
        );
    }
    let files = pack(vec![
        (
            "assets/minecraft/textures/gui/icons.png",
            marked_atlas(256, &[(icon("crosshair"), WHITE)]),
        ),
        (&format!("{sprites}crosshair.png"), solid(16, 16, BLUE)),
    ]);
    let translation = run(&files, "Pack").unwrap();
    assert_eq!(
        pixel(&output(&translation), "textures/gui/icons.png", 14, 14).1,
        BLUE
    );
}

#[test]
fn pre_1_9_boats_are_the_oak_boat() {
    let files = pack(vec![(
        "assets/minecraft/textures/items/boat.png",
        solid(16, 16, BLUE),
    )]);
    let translation = run(&files, "Legacy").unwrap();
    assert!(output(&translation).contains_key("textures/items/boat_oak.png"));
}

#[test]
fn translating_the_same_content_keeps_the_pack_identity() {
    let files = pack(vec![(
        "assets/minecraft/textures/block/stone.png",
        solid(16, 16, RED),
    )]);
    let identity = |files: &BTreeMap<String, Vec<u8>>| {
        let translation = run(files, "Pack").unwrap();
        let out = output(&translation);
        let manifest: serde_json::Value = serde_json::from_slice(out["manifest.json"]).unwrap();
        manifest["header"]["uuid"].as_str().unwrap().to_owned()
    };
    let first = identity(&files);
    assert_eq!(first, identity(&files));
    let changed = pack(vec![(
        "assets/minecraft/textures/block/stone.png",
        solid(16, 16, BLUE),
    )]);
    assert_ne!(first, identity(&changed));
}

#[test]
fn unreadable_and_invalid_files_are_skipped() {
    let files = pack(vec![
        (
            "assets/minecraft/textures/block/stone.png",
            solid(16, 16, RED),
        ),
        (
            "assets/minecraft/textures/block/dirt.png",
            b"not a png".to_vec(),
        ),
        (
            "assets/minecraft/textures/item/apple.png",
            solid(16, 16, RED),
        ),
    ]);
    let paths: Vec<String> = files.keys().cloned().collect();
    let translation = translate(
        &paths,
        |path| (!path.ends_with("apple.png")).then(|| files[path].clone()),
        "Pack",
        TEXTURE_LIMIT,
    )
    .unwrap();
    assert_eq!(translation.skipped, 2);
    let out = output(&translation);
    assert!(out.contains_key("textures/blocks/stone.png"));
    assert!(!out.contains_key("textures/blocks/dirt.png"));
}

#[test]
fn a_pack_without_usable_textures_is_rejected() {
    let files = pack(vec![
        ("pack.mcmeta", br#"{"pack":{"pack_format":15}}"#.to_vec()),
        ("assets/minecraft/sounds.json", b"{}".to_vec()),
    ]);
    assert_eq!(
        run(&files, "Sounds").unwrap_err(),
        TranslateError::NothingToTranslate
    );
}

#[test]
fn roots_are_found_at_the_shallowest_marker() {
    assert_eq!(
        find_root(["pack.mcmeta", "x/pack.mcmeta"]).as_deref(),
        Some("")
    );
    assert_eq!(
        find_root(["Faithful/pack.mcmeta", "Faithful/assets/minecraft/a.png"]).as_deref(),
        Some("Faithful/")
    );
    assert_eq!(
        find_root(["p/assets/minecraft/textures/a.png"]).as_deref(),
        Some("p/")
    );
    assert_eq!(
        find_root([
            "data/minecraft/datapacks/extra/pack.mcmeta",
            "assets/minecraft/textures/block/stone.png",
        ])
        .as_deref(),
        Some(""),
        "a nested data pack never wins"
    );
    assert_eq!(
        find_root(["data/minecraft/datapacks/extra/pack.mcmeta"]).as_deref(),
        Some("data/minecraft/datapacks/extra/")
    );
    assert_eq!(
        find_root(["manifest.json", "textures/a.png", "notpack.mcmeta"]),
        None
    );
}

#[test]
fn the_menus_files_are_kept_as_the_pack_has_them() {
    let button = solid(8, 4, RED);
    let meta =
        br#"{"gui":{"scaling":{"type":"nine_slice","width":8,"height":4,"border":1}}}"#.to_vec();
    let files = pack(vec![
        (
            "assets/minecraft/textures/block/stone.png",
            solid(16, 16, GREEN),
        ),
        (
            "assets/minecraft/textures/gui/sprites/widget/button.png",
            button.clone(),
        ),
        (
            "assets/minecraft/textures/gui/sprites/widget/button.png.mcmeta",
            meta.clone(),
        ),
        ("assets/minecraft/font/default.json", b"{}".to_vec()),
        ("assets/minecraft/texts/splashes.txt", b"Hi!".to_vec()),
        // Files the menus never read stay out.
        (
            "assets/minecraft/textures/gui/title/minecraft.png",
            solid(4, 4, BLUE),
        ),
        ("assets/minecraft/font/../sneaky.json", b"{}".to_vec()),
    ]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    let kept = |path: &str| {
        out.get(format!("{}{path}", menu_files::PACK_PREFIX).as_str())
            .copied()
    };
    assert_eq!(
        kept("textures/gui/sprites/widget/button.png"),
        Some(button.as_slice())
    );
    assert_eq!(
        kept("textures/gui/sprites/widget/button.png.mcmeta"),
        Some(meta.as_slice())
    );
    assert_eq!(kept("font/default.json"), Some(b"{}".as_slice()));
    assert_eq!(kept("texts/splashes.txt"), Some(b"Hi!".as_slice()));
    assert!(kept("textures/gui/title/minecraft.png").is_none());
    assert!(kept("font/../sneaky.json").is_none());
}

#[test]
fn a_pack_that_only_restyles_the_menus_is_still_a_pack() {
    let files = pack(vec![(
        "assets/minecraft/textures/gui/sprites/widget/button.png",
        solid(8, 4, RED),
    )]);
    assert!(run(&files, "Menus").is_ok());
}

#[test]
fn the_inventory_slot_becomes_every_bedrock_cell_with_sidecars() {
    let slot = Rect {
        x: 7,
        y: 83,
        w: 18,
        h: 18,
    };
    let files = pack(vec![(
        "assets/minecraft/textures/gui/container/inventory.png",
        marked_atlas(512, &[(slot, GREEN)]),
    )]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    for cell in [
        "textures/ui/cell_image.png",
        "textures/ui/cell_image_normal.png",
        "textures/ui/cell_image_darkgrey.png",
        "textures/ui/cell_image_invert.png",
        "textures/ui/cell_image_red.png",
        "textures/ui/cell_image_lock.png",
    ] {
        assert_eq!(pixel(&out, cell, 0, 0).0, (36, 36), "{cell}");
        let sidecar = cell.replace(".png", ".json");
        let value: serde_json::Value = serde_json::from_slice(out[sidecar.as_str()]).unwrap();
        assert_eq!(
            value,
            serde_json::json!({"nineslice_size": 1, "base_size": [18, 18]})
        );
    }
    assert_eq!(pixel(&out, "textures/ui/cell_image.png", 0, 0).1, GREEN);
}

#[test]
fn the_hotbar_selection_also_highlights_the_selected_inventory_slot() {
    let sprites = "assets/minecraft/textures/gui/sprites/hud/";
    let files = pack(vec![(
        &format!("{sprites}hotbar_selection.png"),
        solid(24, 23, YELLOW),
    )]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    assert_eq!(
        pixel(
            &out,
            "textures/ui/pocket_ui_highlight_selected_slot.png",
            0,
            0
        ),
        ((24, 24), YELLOW)
    );
}

#[test]
fn the_inventory_frame_becomes_the_bedrock_panel_with_a_sidecar() {
    let files = pack(vec![(
        "assets/minecraft/textures/gui/container/inventory.png",
        marked_atlas(
            256,
            &[(
                Rect {
                    x: 0,
                    y: 0,
                    w: 176,
                    h: 2,
                },
                GREEN,
            )],
        ),
    )]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    // The frame row is kept and everything else is the opaque grey, so the panel stays readable.
    assert_eq!(
        pixel(&out, "textures/ui/dialog_background_opaque.png", 10, 0),
        ((176, 166), GREEN)
    );
    assert_eq!(
        pixel(&out, "textures/ui/dialog_background_opaque.png", 88, 83).1,
        [198, 198, 198, 255]
    );
    let value: serde_json::Value =
        serde_json::from_slice(out["textures/ui/dialog_background_opaque.json"]).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"nineslice_size": 4, "base_size": [176, 166]})
    );
}

#[test]
fn the_container_panel_keeps_a_see_through_pack_see_through_beside_the_filled_one() {
    let files = pack(vec![(
        "assets/minecraft/textures/gui/container/inventory.png",
        marked_atlas(
            256,
            &[(
                Rect {
                    x: 0,
                    y: 0,
                    w: 176,
                    h: 2,
                },
                GREEN,
            )],
        ),
    )]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    assert_eq!(
        pixel(&out, "textures/ui/dialog_background_container.png", 10, 0),
        ((176, 166), GREEN)
    );
    assert_eq!(
        pixel(&out, "textures/ui/dialog_background_container.png", 88, 83).1[3],
        0
    );
    assert!(out.contains_key("textures/ui/dialog_background_container.json"));
}

#[test]
fn mob_skins_and_colour_maps_reach_their_bedrock_paths_at_any_whole_multiple_of_their_size() {
    let textures = "assets/minecraft/textures/";
    let files = pack(vec![
        (
            "pack.mcmeta",
            br#"{"pack":{"pack_format":34,"description":"x"}}"#.to_vec(),
        ),
        (
            &format!("{textures}entity/creeper/creeper.png"),
            solid(256, 128, GREEN),
        ),
        (
            &format!("{textures}colormap/grass.png"),
            solid(256, 256, YELLOW),
        ),
        // Not the vanilla proportions: left out rather than mapped wrongly.
        (
            &format!("{textures}entity/creeper/creeper_armor.png"),
            solid(100, 32, GREEN),
        ),
    ]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    assert_eq!(
        pixel(&out, "textures/entity/creeper/creeper.png", 0, 0),
        ((256, 128), GREEN)
    );
    assert_eq!(
        pixel(&out, "textures/colormap/grass.png", 0, 0),
        ((256, 256), YELLOW)
    );
    assert!(!out.contains_key("textures/entity/creeper/creeper_armor.png"));
}

#[test]
fn a_see_through_pack_gets_see_through_recipe_list_item_and_player_box_copies_for_containers() {
    let slot = Rect {
        x: 7,
        y: 83,
        w: 18,
        h: 18,
    };
    let player = Rect {
        x: 25,
        y: 7,
        w: 54,
        h: 1,
    };
    let frame = Rect {
        x: 0,
        y: 0,
        w: 176,
        h: 1,
    };
    let files = pack(vec![(
        "assets/minecraft/textures/gui/container/inventory.png",
        marked_atlas(256, &[(slot, GREEN), (player, YELLOW), (frame, GREEN)]),
    )]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    // The recipe list's backdrop and the player's background are clear, as the pack's panel is.
    assert_eq!(
        pixel(&out, "textures/ui/recipe_book_pane_bg_container.png", 0, 0),
        ((1, 1), [0; 4])
    );
    assert_eq!(
        pixel(&out, "textures/ui/black_container.png", 0, 0),
        ((1, 1), [0; 4])
    );
    // The recipe item cell is the pack's slot, the player frame the pack's frame.
    assert_eq!(
        pixel(&out, "textures/ui/recipe_book_item_bg_container.png", 0, 0),
        ((18, 18), GREEN)
    );
    assert_eq!(
        pixel(
            &out,
            "textures/ui/player_preview_border_container.png",
            0,
            0
        ),
        ((54, 72), YELLOW)
    );
    for sidecar in [
        "textures/ui/recipe_book_item_bg_container.json",
        "textures/ui/player_preview_border_container.json",
    ] {
        assert!(out.contains_key(sidecar), "{sidecar}");
    }
}

#[test]
fn container_progress_sprites_replace_their_bedrock_textures_untouched() {
    let sprites = "assets/minecraft/textures/gui/sprites/container/";
    let flame = solid(14, 14, RED);
    let files = pack(vec![
        (&format!("{sprites}furnace/lit_progress.png"), flame.clone()),
        (&format!("{sprites}anvil/error.png"), solid(28, 21, GREEN)),
    ]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    assert_eq!(
        pixel(&out, "textures/ui/flame_full_image.png", 0, 0),
        ((14, 14), RED)
    );
    assert_eq!(
        pixel(&out, "textures/ui/crossout.png", 0, 0),
        ((28, 21), GREEN)
    );
    assert!(!out.contains_key("textures/ui/arrow_active.png"));
}

#[test]
fn creative_tab_sprites_become_every_bedrock_tab_with_a_sidecar() {
    let sprites = "assets/minecraft/textures/gui/sprites/container/creative_inventory/";
    let files = pack(vec![
        (
            &format!("{sprites}tab_top_selected_2.png"),
            solid(26, 32, RED),
        ),
        (
            &format!("{sprites}tab_top_unselected_7.png"),
            solid(26, 32, GREEN),
        ),
    ]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    for tab in ["TabTopFront", "TabTopFrontHover"] {
        assert_eq!(
            pixel(&out, &format!("textures/ui/{tab}.png"), 0, 0),
            ((26, 32), RED)
        );
    }
    for tab in ["TabTopBackRightMost", "TabTopBackRightMostDarkHover"] {
        assert_eq!(
            pixel(&out, &format!("textures/ui/{tab}.png"), 0, 0),
            ((26, 32), GREEN)
        );
    }
    let sidecar: serde_json::Value =
        serde_json::from_slice(out["textures/ui/TabTopFront.json"]).unwrap();
    assert_eq!(
        sidecar,
        serde_json::json!({"nineslice_size": 4, "base_size": [26, 32]})
    );
    assert!(!out.contains_key("textures/ui/TabTopBack.png"));
}

#[test]
fn effect_backgrounds_replace_the_hud_and_list_pictures() {
    let files = pack(vec![
        (
            "assets/minecraft/textures/gui/sprites/hud/effect_background.png",
            solid(24, 24, RED),
        ),
        (
            "assets/minecraft/textures/gui/sprites/container/inventory/effect_background_large.png",
            solid(120, 32, GREEN),
        ),
    ]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    assert_eq!(
        pixel(&out, "textures/ui/hud_mob_effect_background.png", 0, 0),
        ((24, 24), RED)
    );
    for list in ["effect_background", "effect_background_ambient"] {
        assert_eq!(
            pixel(&out, &format!("textures/ui/{list}.png"), 0, 0),
            ((120, 32), GREEN)
        );
        assert!(out.contains_key(format!("textures/ui/{list}.json").as_str()));
    }
}

#[test]
fn particle_sprites_become_an_atlas_overlay_unless_the_pack_has_its_own_atlas() {
    let sprite = "assets/minecraft/textures/particle/flame.png";
    let files = pack(vec![(sprite, solid(8, 8, RED))]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    assert_eq!(
        pixel(&out, "textures/particle/particles.png", 0, 3 * 8),
        ((128, 128), RED)
    );
    assert_eq!(
        pixel(&out, "textures/particle/particles.png", 0, 0).1,
        [0; 4]
    );
    let marker: serde_json::Value =
        serde_json::from_slice(out["textures/particle/particles.overlay.json"]).unwrap();
    assert_eq!(marker, serde_json::json!({"cells": [[0, 3]]}));

    let own = pack(vec![
        (sprite, solid(8, 8, RED)),
        (
            "assets/minecraft/textures/particle/particles.png",
            solid(128, 128, GREEN),
        ),
    ]);
    let translation = run(&own, "Pack").unwrap();
    let out = output(&translation);
    assert_eq!(
        pixel(&out, "textures/particle/particles.png", 0, 3 * 8).1,
        GREEN
    );
    assert!(!out.contains_key("textures/particle/particles.overlay.json"));
}

#[test]
fn leather_items_get_the_undyed_tint_baked_in_and_need_an_overlay_to_be_translated() {
    let items = "assets/minecraft/textures/item/";
    let files = pack(vec![
        (&format!("{items}leather_helmet.png"), solid(16, 16, WHITE)),
        (
            &format!("{items}leather_helmet_overlay.png"),
            solid(16, 16, RED),
        ),
        (
            &format!("{items}leather_chestplate.png"),
            solid(16, 16, WHITE),
        ),
        (
            &format!("{items}leather_chestplate_overlay.png"),
            corner_dot(16, RED),
        ),
        (&format!("{items}leather_boots.png"), solid(16, 16, WHITE)),
    ]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    // The overlay covers the whole helmet, so every pixel is fixed and keeps its colour.
    let (size, helmet) = pixel(&out, "textures/items/leather_helmet.png", 0, 0);
    assert_eq!(size, (16, 16));
    assert!(
        helmet[0] >= 250 && helmet[1] <= 4 && helmet[3] == 255,
        "{helmet:?}"
    );
    // Elsewhere the dyed base is browned.
    let (_, brown) = pixel(&out, "textures/items/leather_chestplate.png", 5, 5);
    assert!(
        brown[0] > brown[1] && brown[1] > brown[2] && brown[3] == 255,
        "{brown:?}"
    );
    assert!(!out.contains_key("textures/items/leather_helmet.tga"));
    // With no overlay nothing separates the dyed layer, so vanilla's icon stays.
    assert!(!out.contains_key("textures/items/leather_boots.png"));
    assert!(!out.contains_key("textures/items/leather_boots.tga"));
}

#[test]
fn worn_leather_armor_becomes_a_dye_mask_texture() {
    let layer = "assets/minecraft/textures/entity/equipment/humanoid/";
    let files = pack(vec![
        (&format!("{layer}leather.png"), solid(64, 32, WHITE)),
        (&format!("{layer}leather_overlay.png"), solid(64, 32, RED)),
    ]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    assert_eq!(
        pixel(&out, "textures/models/armor/leather_1.tga", 0, 0),
        ((64, 32), [255, 0, 0, 3])
    );
}

#[test]
fn a_modern_square_zombie_fills_bedrocks_half_height_layout_with_its_top_half() {
    let mut image = RgbaImage::new(128, 128);
    for pixel in image.pixels_mut() {
        *pixel = Rgba(RED);
    }
    for y in 64..128 {
        for x in 0..128 {
            image.put_pixel(x, y, Rgba(GREEN));
        }
    }
    let files = pack(vec![(
        "assets/minecraft/textures/entity/zombie/zombie.png",
        encode(&image),
    )]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    assert_eq!(
        pixel(&out, "textures/entity/zombie/zombie.png", 5, 63),
        ((128, 64), RED)
    );
}

#[test]
fn a_huge_hotbar_is_shrunk_so_every_slot_fits_the_clients_texture_pages() {
    let sprites = "assets/minecraft/textures/gui/sprites/hud/";
    let files = pack(vec![(
        &format!("{sprites}hotbar.png"),
        solid(182 * 32, 22 * 32, WHITE),
    )]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    for slot in 0..9 {
        let (size, _) = pixel(&out, &format!("textures/ui/hotbar_{slot}.png"), 0, 0);
        assert_eq!(size, (20 * 8, 22 * 8), "slot {slot}");
    }
}

#[test]
fn a_pack_animation_keeps_its_strip_and_becomes_a_flipbook_patch() {
    let blocks = "assets/minecraft/textures/block/";
    let files = pack(vec![
        (&format!("{blocks}lava_still.png"), solid(16, 64, RED)),
        (
            &format!("{blocks}lava_still.png.mcmeta"),
            br#"{"animation":{"frametime":3,"interpolate":true}}"#.to_vec(),
        ),
        (&format!("{blocks}diamond_ore.png"), solid(16, 48, GREEN)),
        (
            &format!("{blocks}diamond_ore.png.mcmeta"),
            br#"{"animation":{"frames":[2,1,0]}}"#.to_vec(),
        ),
        (&format!("{blocks}stone.png"), solid(16, 32, WHITE)),
    ]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    // The animated textures keep every frame; the unanimated strip keeps its first.
    assert_eq!(
        pixel(&out, "textures/blocks/lava_still.png", 0, 0).0,
        (16, 64)
    );
    assert_eq!(
        pixel(&out, "textures/blocks/diamond_ore.png", 0, 0).0,
        (16, 48)
    );
    assert_eq!(pixel(&out, "textures/blocks/stone.png", 0, 0).0, (16, 16));
    let patches: serde_json::Value =
        serde_json::from_slice(out["textures/flipbook_textures.json"]).unwrap();
    let patches = patches.as_array().unwrap();
    assert_eq!(patches.len(), 2);
    let lava = patches
        .iter()
        .find(|patch| patch["atlas_tile"] == "still_lava")
        .unwrap();
    assert_eq!(lava["ticks_per_frame"], 3);
    assert_eq!(lava["blend_frames"], true);
    assert_eq!(lava["frames"], serde_json::json!([0, 1, 2, 3]));
    let ore = patches
        .iter()
        .find(|patch| patch["atlas_tile"] == "diamond_ore")
        .unwrap();
    assert_eq!(ore["frames"], serde_json::json!([2, 1, 0]));
}

#[test]
fn an_optifine_random_rule_becomes_a_weighted_texture_list_for_the_blocks_tile() {
    let root = "assets/minecraft/";
    let files = pack(vec![
        (
            &format!("{root}textures/block/dirt.png"),
            solid(16, 16, RED),
        ),
        (
            &format!("{root}optifine/ctm/dirt/dirt.properties"),
            b"method=random\nmatchTiles=dirt\ntiles=<default> 1-2\nweights=3 1 1\n".to_vec(),
        ),
        (
            &format!("{root}optifine/ctm/dirt/1.png"),
            solid(16, 16, GREEN),
        ),
        (
            &format!("{root}optifine/ctm/dirt/2.png"),
            solid(16, 16, BLUE),
        ),
        // Another rule that depends on the biome is left alone.
        (
            &format!("{root}optifine/ctm/stone/stone.properties"),
            b"method=random\nmatchTiles=stone\ntiles=1-2\nbiomes=plains\n".to_vec(),
        ),
        (
            &format!("{root}textures/block/stone.png"),
            solid(16, 16, WHITE),
        ),
    ]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    let atlas: serde_json::Value =
        serde_json::from_slice(out["textures/terrain_texture.json"]).unwrap();
    let data = atlas["texture_data"].as_object().unwrap();
    // The dirt picture fills several single-variant tiles, the block's own `flattened_dirt` among
    // them; the biome-bound stone rule is left alone.
    assert!(data.contains_key("flattened_dirt"));
    assert!(!data.contains_key("stone") && !data.contains_key("flattened_stone"));
    assert!(data.values().all(|tile| tile == &data["flattened_dirt"]));
    let choices = data["flattened_dirt"]["textures"][0]["variations"]
        .as_array()
        .unwrap();
    assert_eq!(choices.len(), 3);
    assert_eq!(choices[0]["weight"], 3.0);
    assert_eq!(choices[2]["path"], "textures/blocks/dirt_random2");
    assert_eq!(
        pixel(&out, "textures/blocks/dirt_random0.png", 0, 0).1,
        RED,
        "the default choice is the block's own texture"
    );
    assert_eq!(
        pixel(&out, "textures/blocks/dirt_random1.png", 0, 0).1,
        GREEN
    );
    assert_eq!(
        pixel(&out, "textures/blocks/dirt_random2.png", 0, 0).1,
        BLUE
    );
}

#[test]
fn an_optifine_repeat_rule_becomes_a_declared_tile_pattern() {
    let root = "assets/minecraft/";
    let mut files = vec![
        (
            format!("{root}textures/block/stone.png"),
            solid(16, 16, WHITE),
        ),
        (
            format!("{root}optifine/ctm/stone/stone.properties"),
            b"method=repeat\nmatchTiles=stone\nwidth=2\nheight=2\ntiles=1-4\n".to_vec(),
        ),
        // A pattern with a tile missing is left alone.
        (
            format!("{root}textures/block/sand.png"),
            solid(16, 16, WHITE),
        ),
        (
            format!("{root}optifine/ctm/sand/sand.properties"),
            b"method=repeat\nmatchTiles=sand\nwidth=2\nheight=2\ntiles=1-4\n".to_vec(),
        ),
    ];
    for (index, color) in [RED, GREEN, BLUE, YELLOW].into_iter().enumerate() {
        files.push((
            format!("{root}optifine/ctm/stone/{}.png", index + 1),
            solid(16, 16, color),
        ));
    }
    files.push((format!("{root}optifine/ctm/sand/1.png"), solid(16, 16, RED)));
    let files = files
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes.clone()))
        .collect();
    let translation = run(&pack(files), "Pack").unwrap();
    let out = output(&translation);
    let atlas: serde_json::Value =
        serde_json::from_slice(out["textures/terrain_texture.json"]).unwrap();
    let data = atlas["texture_data"].as_object().unwrap();
    let tile = data
        .values()
        .find(|tile| tile.get("cinnabar_repeat").is_some())
        .expect("the stone pattern");
    assert_eq!(
        tile["cinnabar_repeat"],
        serde_json::json!({"width": 2, "height": 2})
    );
    let choices = tile["textures"][0]["variations"].as_array().unwrap();
    assert_eq!(choices.len(), 4);
    for (index, color) in [RED, GREEN, BLUE, YELLOW].into_iter().enumerate() {
        let path = choices[index]["path"].as_str().unwrap();
        assert_eq!(pixel(&out, &format!("{path}.png"), 0, 0).1, color, "{path}");
    }
    assert!(
        data.keys().all(|key| !key.contains("sand")),
        "the incomplete pattern writes nothing"
    );
}

#[test]
fn an_optifine_compact_rule_becomes_the_full_declared_tile_set() {
    let root = "assets/minecraft/";
    let mut files = vec![
        (
            format!("{root}textures/block/stone.png"),
            solid(16, 16, WHITE),
        ),
        (
            format!("{root}optifine/ctm/stone/stone.properties"),
            b"method=ctm_compact\nmatchBlocks=stone\ntiles=0-4\n".to_vec(),
        ),
    ];
    let tones = [RED, GREEN, BLUE, YELLOW, WHITE];
    for (index, color) in tones.into_iter().enumerate() {
        files.push((
            format!("{root}optifine/ctm/stone/{index}.png"),
            solid(16, 16, color),
        ));
    }
    let files = files
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes.clone()))
        .collect();
    let translation = run(&pack(files), "Pack").unwrap();
    let out = output(&translation);
    let atlas: serde_json::Value =
        serde_json::from_slice(out["textures/terrain_texture.json"]).unwrap();
    let tile = atlas["texture_data"]
        .as_object()
        .unwrap()
        .values()
        .find(|tile| tile.get("cinnabar_connected").is_some())
        .expect("the stone tile set");
    let choices = tile["textures"][0]["variations"].as_array().unwrap();
    assert_eq!(choices.len(), 47);
    let colour = |index: usize| {
        let path = choices[index]["path"].as_str().unwrap();
        pixel(&out, &format!("{path}.png"), 0, 0).1
    };
    // Tile 0 has no neighbours and tile 26 all of them; a quarter keeps its own source.
    assert_eq!(colour(0), RED);
    assert_eq!(colour(26), GREEN);
    assert_eq!(colour(46), WHITE);
}

#[test]
fn a_packs_ascii_font_becomes_the_bedrock_sheet_with_its_marker() {
    let files = pack(vec![(
        "assets/minecraft/textures/font/ascii.png",
        solid(128, 128, WHITE),
    )]);
    let translation = run(&files, "Pack").unwrap();
    let out = output(&translation);
    // Rows the font leaves for control characters are cleared, the ASCII rows kept.
    assert_eq!(pixel(&out, "font/default8.png", 4, 4).1[3], 0);
    assert_eq!(pixel(&out, "font/default8.png", 4, 40), ((128, 128), WHITE));
    let marker: serde_json::Value = serde_json::from_slice(out["font/cinnabar_font.json"]).unwrap();
    assert_eq!(marker["blank_cells_keep_base"], true);
}

#[test]
fn a_pack_with_its_own_water_art_also_gets_javas_water_colours() {
    let block = "assets/minecraft/textures/block/";
    let with_water = pack(vec![(
        &format!("{block}water_still.png"),
        solid(16, 16, [160, 160, 160, 180]),
    )]);
    let translation = run(&with_water, "Current").unwrap();
    assert!(output(&translation).contains_key(crate::water::FILE));

    let without = pack(vec![(&format!("{block}stone.png"), solid(16, 16, WHITE))]);
    let translation = run(&without, "Current").unwrap();
    assert!(!output(&translation).contains_key(crate::water::FILE));
}

#[test]
fn an_item_model_decides_which_texture_the_item_draws() {
    let items = "assets/minecraft/textures/item/";
    let files = pack(vec![
        (
            &format!("{items}ruby_blade.png"),
            solid(16, 16, [200, 20, 40, 255]),
        ),
        (
            &format!("{items}diamond_sword.png"),
            solid(16, 16, [10, 10, 10, 255]),
        ),
        (
            "assets/minecraft/models/item/diamond_sword.json",
            br#"{"parent":"item/handheld","textures":{"layer0":"item/ruby_blade"}}"#.to_vec(),
        ),
    ]);
    let translation = run(&files, "Current").unwrap();
    let out = output(&translation);
    assert_eq!(
        pixel(&out, "textures/items/diamond_sword.png", 8, 8).1,
        [200, 20, 40, 255],
        "the model's texture, not the file with the item's own name"
    );
}
