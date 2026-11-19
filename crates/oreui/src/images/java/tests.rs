use std::{collections::HashMap, io::Cursor};

use java_resource_pack::menu_files::{FONT_DEFINITIONS, GUI_FILES, gui_path};

use super::*;

/// A PNG `width` by `height` texels, opaque where `ink` says and clear elsewhere.
fn png(width: u32, height: u32, ink: impl Fn(u32, u32) -> bool) -> Vec<u8> {
    let image = image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([255, 255, 255, if ink(x, y) { 255 } else { 0 }])
    });
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

fn solid(width: u32, height: u32) -> Vec<u8> {
    png(width, height, |_, _| true)
}

/// A cache folder holding every GUI file as a 2 by 2 picture, with the button's borders, and the
/// game's font: a default font that references `include/default`, whose sheet has `x` and `y`.
fn game_cache() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let write = |path: &str, bytes: &[u8]| {
        let target = dir.path().join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, bytes).unwrap();
    };
    for file in GUI_FILES {
        write(&gui_path(file), &solid(2, 2));
    }
    write(
        &mcmeta(&gui_path("sprites/widget/button.png")),
        br#"{"gui":{"scaling":{"type":"nine_slice","width":2,"height":2,"border":1}}}"#,
    );
    write(
        "font/default.json",
        br#"{"providers":[{"type":"reference","id":"minecraft:include/default"}]}"#,
    );
    write(
        "font/include/default.json",
        br#"{"providers":[{"type":"bitmap","file":"minecraft:font/game.png","ascent":7,"chars":["xy"]}]}"#,
    );
    // `x` is 3 texels wide, `y` is 5.
    write(
        "textures/font/game.png",
        &png(16, 8, |x, y| y == 2 && matches!(x, 0..=2 | 8..=12)),
    );
    write("texts/splashes.txt", b"from the game\n\nsecond\n");
    dir
}

fn resolve_with(packs: &[ReadFile<'_>], cache: &Path) -> Option<Resolved> {
    resolve(&Layers {
        packs,
        cache: Some(cache),
    })
}

fn pack_of(files: Vec<(&str, Vec<u8>)>) -> impl Fn(&str) -> Option<Vec<u8>> {
    let files: HashMap<String, Vec<u8>> = files
        .into_iter()
        .map(|(path, bytes)| (path.to_owned(), bytes))
        .collect();
    move |path| files.get(path).cloned()
}

fn dimensions(resolved: &Resolved, name: &str) -> Option<(u32, u32)> {
    resolved.art.iter().find_map(|(key_, art)| match art {
        Art::Still(width, height, _) if *key_ == key(name) => Some((*width, *height)),
        _ => None,
    })
}

#[test]
fn the_required_sprites_must_come_from_some_layer() {
    let empty = tempfile::tempdir().unwrap();
    assert!(resolve_with(&[], empty.path()).is_none());
    // The cache alone is enough, as is a pack alone with no cache.
    let cache = game_cache();
    assert!(resolve_with(&[], cache.path()).is_some());
    let buttons: Vec<(String, Vec<u8>)> = ["button", "button_highlighted", "button_disabled"]
        .iter()
        .map(|name| (gui_path(&format!("sprites/widget/{name}.png")), solid(4, 4)))
        .collect();
    let pack = pack_of(
        buttons
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.clone()))
            .collect(),
    );
    assert!(resolve_with(&[&pack], empty.path()).is_some());
}

#[test]
fn a_packs_copy_of_a_sprite_replaces_the_games() {
    let cache = game_cache();
    let path = gui_path("sprites/widget/button.png");
    let pack = pack_of(vec![(path.as_str(), solid(8, 4))]);
    let resolved = resolve_with(&[&pack], cache.path()).unwrap();
    assert_eq!(
        dimensions(&resolved, "sprites/widget/button.png"),
        Some((8, 4))
    );
    // A sprite the pack leaves alone still comes from the game.
    assert_eq!(
        dimensions(&resolved, "sprites/widget/slider.png"),
        Some((2, 2))
    );
    // Without the pack the game's copy shows.
    let plain = resolve_with(&[], cache.path()).unwrap();
    assert_eq!(
        dimensions(&plain, "sprites/widget/button.png"),
        Some((2, 2))
    );
}

#[test]
fn the_higher_pack_wins_over_the_lower() {
    let cache = game_cache();
    let path = gui_path("sprites/widget/button.png");
    let high = pack_of(vec![(path.as_str(), solid(6, 6))]);
    let low = pack_of(vec![(path.as_str(), solid(10, 10))]);
    let resolved = resolve_with(&[&high, &low], cache.path()).unwrap();
    assert_eq!(
        dimensions(&resolved, "sprites/widget/button.png"),
        Some((6, 6))
    );
}

#[test]
fn borders_come_from_the_layer_that_supplied_the_texture() {
    let cache = game_cache();
    let path = gui_path("sprites/widget/button.png");
    let plain = resolve_with(&[], cache.path()).unwrap();
    assert_eq!(
        plain.active.borders.get(&key("sprites/widget/button.png")),
        Some(&[1; 4])
    );
    // A pack texture without its own `.mcmeta` takes the game's borders, scaled to its size, so a
    // narrow or square control does not stretch the pack's edges.
    let bare = pack_of(vec![(path.as_str(), solid(8, 8))]);
    let resolved = resolve_with(&[&bare], cache.path()).unwrap();
    assert_eq!(
        resolved
            .active
            .borders
            .get(&key("sprites/widget/button.png")),
        Some(&[4; 4])
    );
    // A pack's own borders are written for the nominal size and scale up with the texture.
    let meta_path = mcmeta(&path);
    let hd = pack_of(vec![
        (path.as_str(), solid(400, 40)),
        (
            meta_path.as_str(),
            br#"{"gui":{"scaling":{"type":"nine_slice","width":200,"height":20,"border":3}}}"#
                .to_vec(),
        ),
    ]);
    let resolved = resolve_with(&[&hd], cache.path()).unwrap();
    // A button twice Java's 200 pixels puts the pack at two texels per Java pixel; the game's own
    // tiny stand-in counts as Java's size.
    assert_eq!(resolved.active.scale, 2.0);
    assert_eq!(plain.active.scale, 1.0);
    assert_eq!(
        resolved
            .active
            .borders
            .get(&key("sprites/widget/button.png")),
        Some(&[6; 4])
    );
}

#[test]
fn a_border_is_read_per_side_scaled_and_other_scaling_has_none() {
    let all = br#"{"gui":{"scaling":{"type":"nine_slice","width":200,"height":20,"border":3}}}"#;
    assert_eq!(parse_border(all, [200, 20]), Some(([3; 4], [3.0; 4])));
    let sides = br#"{"gui":{"scaling":{"type":"nine_slice","width":8,"height":20,
        "border":{"left":2,"top":1,"right":4,"bottom":3}}}}"#;
    assert_eq!(
        parse_border(sides, [8, 20]),
        Some(([1, 4, 3, 2], [1.0, 4.0, 3.0, 2.0]))
    );
    // Twice the nominal width and height doubles every side in texels, and none in Java pixels.
    assert_eq!(
        parse_border(sides, [16, 40]),
        Some(([2, 8, 6, 4], [1.0, 4.0, 3.0, 2.0]))
    );
    assert_eq!(
        parse_border(br#"{"gui":{"scaling":{"type":"stretch"}}}"#, [8, 8]),
        None
    );
    assert_eq!(parse_border(b"not json", [8, 8]), None);
}

#[test]
fn the_games_font_resolves_through_its_references() {
    let cache = game_cache();
    let resolved = resolve_with(&[], cache.path()).unwrap();
    let font = &resolved.active.font;
    assert_eq!(font.glyph('x').unwrap().advance, 4.0);
    assert_eq!(font.glyph('y').unwrap().advance, 6.0);
    assert!(
        resolved
            .art
            .iter()
            .any(|(name, _)| *name == key("font/game.png"))
    );
}

#[test]
fn a_packs_glyphs_win_and_the_games_fill_what_the_pack_leaves_out() {
    let cache = game_cache();
    // The pack defines `x` only, in a sheet where it is 7 texels wide.
    let pack = pack_of(vec![
        (
            FONT_DEFINITIONS[0],
            br#"{"providers":[{"type":"bitmap","file":"minecraft:font/pack.png","ascent":7,"chars":["x"]}]}"#.to_vec(),
        ),
        ("textures/font/pack.png", png(8, 8, |x, y| y == 2 && x <= 6)),
    ]);
    let resolved = resolve_with(&[&pack], cache.path()).unwrap();
    let font = &resolved.active.font;
    assert_eq!(font.glyph('x').unwrap().advance, 8.0, "the pack's x wins");
    assert_eq!(font.glyph('x').unwrap().sheet.as_deref(), Some("pack.png"));
    assert_eq!(
        font.glyph('y').unwrap().advance,
        6.0,
        "the game's y fills in"
    );
    for sheet in ["font/pack.png", "font/game.png"] {
        assert!(
            resolved.art.iter().any(|(name, _)| *name == key(sheet)),
            "{sheet}"
        );
    }
}

#[test]
fn unsupported_providers_and_unsafe_names_are_ignored() {
    let cache = game_cache();
    let pack = pack_of(vec![(
        FONT_DEFINITIONS[0],
        br#"{"providers":[
            {"type":"ttf","file":"minecraft:font/f.ttf"},
            {"type":"bitmap","file":"minecraft:font/../x.png","ascent":7,"chars":["z"]},
            {"type":"bitmap","file":"minecraft:font/sub/x.png","ascent":7,"chars":["z"]},
            {"type":"reference","id":"minecraft:../secret"}]}"#
            .to_vec(),
    )]);
    let resolved = resolve_with(&[&pack], cache.path()).unwrap();
    assert!(resolved.active.font.glyph('z').is_none());
    assert!(resolved.active.font.glyph('x').is_some());
}

#[test]
fn a_font_that_references_itself_does_not_loop() {
    let cache = game_cache();
    let pack = pack_of(vec![
        (
            FONT_DEFINITIONS[0],
            br#"{"providers":[{"type":"reference","id":"minecraft:loop"}]}"#.to_vec(),
        ),
        (
            "font/loop.json",
            br#"{"providers":[{"type":"reference","id":"minecraft:loop"}]}"#.to_vec(),
        ),
    ]);
    assert!(resolve_with(&[&pack], cache.path()).is_some());
}

#[test]
fn splash_lines_of_every_layer_are_combined() {
    let cache = game_cache();
    let pack = pack_of(vec![(SPLASHES, b"from the pack\n".to_vec())]);
    let resolved = resolve_with(&[&pack], cache.path()).unwrap();
    assert_eq!(
        *resolved.active.splashes,
        ["from the pack", "from the game", "second"]
    );
}
