use std::io::Write;

use launcher::install_layout::{InstallEnvironment, InstallLayout, Platform};

use super::*;

fn layout(root: &Path) -> InstallLayout {
    let platform = if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "macos") {
        Platform::MacOs
    } else {
        Platform::Linux
    };
    InstallLayout::resolve(
        platform,
        &InstallEnvironment {
            executable: root.join("target/debug/cinnabar"),
            user_root: Some(root.to_owned()),
            home: None,
            local_app_data: None,
            xdg_config_home: None,
            xdg_data_home: None,
            xdg_runtime_dir: None,
        },
    )
    .unwrap()
}

/// A version folder with a jar holding `sprites` and a JSON of the given type and release time.
fn install_version(root: &Path, id: &str, kind: &str, released: &str, sprites: &[&str]) {
    let dir = root.join("versions").join(id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(format!("{id}.json")),
        format!(r#"{{"id":"{id}","type":"{kind}","releaseTime":"{released}"}}"#),
    )
    .unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(dir.join(format!("{id}.jar"))).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    for sprite in sprites {
        zip.start_file(format!("assets/{GUI_ROOT}{sprite}"), options).unwrap();
        zip.write_all(format!("{id}:{sprite}").as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn the_newest_release_wins_over_snapshots_and_older_releases() {
    let game = tempfile::tempdir().unwrap();
    install_version(game.path(), "1.20.4", "release", "2023-12-07T00:00:00+00:00", REQUIRED);
    install_version(game.path(), "1.21.8", "release", "2025-07-17T00:00:00+00:00", REQUIRED);
    install_version(game.path(), "26w01a", "snapshot", "2026-01-05T00:00:00+00:00", REQUIRED);
    let found = find_client_jar(&[game.path().to_owned()]).unwrap();
    assert_eq!(found.version, "1.21.8");
}

#[test]
fn a_snapshot_is_used_when_no_release_exists() {
    let game = tempfile::tempdir().unwrap();
    install_version(game.path(), "26w01a", "snapshot", "2026-01-05T00:00:00+00:00", REQUIRED);
    assert_eq!(find_client_jar(&[game.path().to_owned()]).unwrap().version, "26w01a");
}

#[test]
fn a_version_without_its_jar_or_json_is_ignored_and_no_install_is_none() {
    let game = tempfile::tempdir().unwrap();
    assert_eq!(find_client_jar(&[game.path().to_owned()]), None);
    let broken = game.path().join("versions/1.99");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("1.99.json"), "{}").unwrap();
    assert_eq!(find_client_jar(&[game.path().to_owned()]), None);
}

#[test]
fn sprites_are_cached_and_a_complete_install_is_ready() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    install_version(game.path(), "1.21.8", "release", "2025-07-17T00:00:00+00:00", SPRITES);
    let layout = layout(home.path());
    let status = prepare_from(&layout, &[game.path().to_owned()]);
    assert_eq!(status, Status::Ready { version: "1.21.8".to_owned() });
    let button = cache_dir(&layout).join("sprites/widget/button.png");
    assert_eq!(std::fs::read_to_string(button).unwrap(), "1.21.8:sprites/widget/button.png");
    assert_eq!(cached_status(&layout), status);
}

#[test]
fn a_jar_missing_a_required_sprite_is_incomplete() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    install_version(
        game.path(),
        "1.21.8",
        "release",
        "2025-07-17T00:00:00+00:00",
        &["sprites/widget/button.png"],
    );
    match prepare_from(&layout(home.path()), &[game.path().to_owned()]) {
        Status::Incomplete { version, missing } => {
            assert_eq!(version, "1.21.8");
            assert!(missing.contains(&"sprites/widget/button_highlighted.png".to_owned()));
        }
        other => panic!("expected an incomplete install, got {other:?}"),
    }
}

#[test]
fn with_no_install_nothing_is_ready_and_a_removed_install_keeps_its_cache() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let layout = layout(home.path());
    assert_eq!(prepare_from(&layout, &[game.path().to_owned()]), Status::NoJavaInstall);
    install_version(game.path(), "1.21.8", "release", "2025-07-17T00:00:00+00:00", SPRITES);
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    std::fs::remove_dir_all(game.path().join("versions")).unwrap();
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
}

#[test]
fn an_unchanged_jar_is_not_read_again_but_a_new_version_is() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let layout = layout(home.path());
    install_version(game.path(), "1.21.8", "release", "2025-07-17T00:00:00+00:00", SPRITES);
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    // A cached file that was changed on disk survives a second prepare of the same jar.
    let button = cache_dir(&layout).join("sprites/widget/button.png");
    std::fs::write(&button, "edited").unwrap();
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    assert_eq!(std::fs::read_to_string(&button).unwrap(), "edited");
    install_version(game.path(), "1.21.9", "release", "2025-09-30T00:00:00+00:00", SPRITES);
    assert_eq!(
        prepare_from(&layout, &[game.path().to_owned()]),
        Status::Ready { version: "1.21.9".to_owned() }
    );
    assert_eq!(std::fs::read_to_string(&button).unwrap(), "1.21.9:sprites/widget/button.png");
}

/// Runs against a real Java Edition folder named by `VANTAGE_JAVA_DIR`; skipped without one.
#[test]
fn a_real_java_install_caches_every_menu_sprite() {
    let Some(game) = std::env::var_os("VANTAGE_JAVA_DIR").map(PathBuf::from) else {
        eprintln!("skipped: missing fixture, set VANTAGE_JAVA_DIR to a Java Edition folder");
        return;
    };
    let home = tempfile::tempdir().unwrap();
    let layout = layout(home.path());
    let status = prepare_from(&layout, &[game]);
    assert!(status.ready(), "{status:?}");
    let manifest = read_manifest(&cache_dir(&layout)).unwrap();
    assert!(manifest.missing.is_empty(), "missing: {:?}", manifest.missing);
    for sprite in SPRITES {
        let bytes = std::fs::read(cache_dir(&layout).join(sprite)).unwrap();
        assert!(bytes.starts_with(b"\x89PNG"), "{sprite} is not a PNG");
    }
    // The Java font's definitions and its bitmap sheets come along.
    for file in ["font/default.json", "font/space.json", "font/ascii.png"] {
        assert!(manifest.font_files.iter().any(|cached| cached == file), "{file} not cached");
        assert!(cache_dir(&layout).join(file).is_file());
    }
    // The title's real panorama faces, from the launcher's asset store, and the splash lines.
    for file in ["title/panorama_0.png", "title/panorama_5.png", "texts/splashes.txt"] {
        assert!(manifest.data_files.iter().any(|cached| cached == file), "{file} not cached");
    }
    assert!(png_size(&std::fs::read(cache_dir(&layout).join("title/panorama_0.png")).unwrap())
        .is_some_and(|(width, height)| width >= MIN_PANORAMA_SIDE && height >= MIN_PANORAMA_SIDE));
    // The widget sprites carry nine-slice borders.
    assert!(cache_dir(&layout).join("sprites/widget/button.png.mcmeta").is_file());
}

#[test]
fn the_font_definitions_and_the_sheets_they_name_are_cached() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let dir = game.path().join("versions/1.21.8");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("1.21.8.json"), r#"{"type":"release","releaseTime":"2025-07-17"}"#).unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(dir.join("1.21.8.jar")).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    let mut add = |name: &str, bytes: &[u8]| {
        zip.start_file(name, options).unwrap();
        zip.write_all(bytes).unwrap();
    };
    for sprite in SPRITES {
        add(&format!("assets/{GUI_ROOT}{sprite}"), b"png");
    }
    add(
        "assets/minecraft/font/include/default.json",
        br#"{"providers":[
            {"type":"bitmap","file":"minecraft:font/ascii.png","ascent":7,"chars":["ab"]},
            {"type":"bitmap","file":"minecraft:font/../../secret.png","ascent":7,"chars":["a"]},
            {"type":"bitmap","file":"minecraft:font/sub/dir.png","ascent":7,"chars":["a"]},
            {"type":"ttf","file":"minecraft:font/x.ttf"}]}"#,
    );
    add(
        "assets/minecraft/font/include/space.json",
        br#"{"providers":[{"type":"space","advances":{" ":4}}]}"#,
    );
    add("assets/minecraft/textures/font/ascii.png", b"sheet");
    add("assets/minecraft/textures/secret.png", b"no");
    zip.finish().unwrap();

    let layout = layout(home.path());
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    let manifest = read_manifest(&cache_dir(&layout)).unwrap();
    assert_eq!(
        manifest.font_files,
        ["font/default.json", "font/space.json", "font/ascii.png"]
    );
    let cache = cache_dir(&layout);
    assert_eq!(std::fs::read(cache.join("font/ascii.png")).unwrap(), b"sheet");
    // Names that would leave the font folder, and non-bitmap providers, are never fetched.
    assert!(!cache.join("secret.png").exists() && !cache.join("font/x.ttf").exists());
}

#[test]
fn a_jar_without_font_files_has_no_java_font_but_still_works() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    install_version(game.path(), "1.21.8", "release", "2025-07-17T00:00:00+00:00", SPRITES);
    let layout = layout(home.path());
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    assert!(read_manifest(&cache_dir(&layout)).unwrap().font_files.is_empty());
}

#[test]
fn a_cache_from_an_older_format_is_read_from_the_jar_again() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let layout = layout(home.path());
    install_version(game.path(), "1.21.8", "release", "2025-07-17T00:00:00+00:00", SPRITES);
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    let button = cache_dir(&layout).join("sprites/widget/button.png");
    std::fs::write(&button, "edited").unwrap();
    // The same jar, but a manifest written before the format number existed.
    let mut manifest = read_manifest(&cache_dir(&layout)).unwrap();
    manifest.format = 0;
    std::fs::write(
        manifest_path(&cache_dir(&layout)),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    assert_eq!(std::fs::read_to_string(&button).unwrap(), "1.21.8:sprites/widget/button.png");
    assert_eq!(read_manifest(&cache_dir(&layout)).unwrap().format, CACHE_FORMAT);
}

/// The header of a PNG `side` texels square: enough for the size check, not a picture.
fn png_header(side: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend(side.to_be_bytes());
    bytes.extend(side.to_be_bytes());
    bytes
}

/// A game folder whose jar holds one-pixel panorama faces and sprites, and whose asset store holds
/// the real faces and the splash lines when `store` is set.
fn install_with_store(root: &Path, store: bool) {
    let dir = root.join("versions/26.3");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("26.3.json"),
        r#"{"type":"release","releaseTime":"2026-09-15","assetIndex":{"id":"34"}}"#,
    )
    .unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(dir.join("26.3.jar")).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    for sprite in SPRITES {
        zip.start_file(format!("assets/{GUI_ROOT}{sprite}"), options).unwrap();
        zip.write_all(b"png").unwrap();
    }
    for face in 0..6 {
        zip.start_file(format!("assets/{PANORAMA_FACES}{face}.png"), options).unwrap();
        zip.write_all(&png_header(1)).unwrap();
    }
    zip.start_file("assets/minecraft/texts/splashes.txt", options).unwrap();
    zip.write_all(b"from the jar\n").unwrap();
    zip.finish().unwrap();
    if !store {
        return;
    }
    let mut objects = serde_json::Map::new();
    let mut put = |name: &str, hash: &str, bytes: &[u8]| {
        let path = root.join("assets/objects").join(&hash[..2]);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join(hash), bytes).unwrap();
        objects.insert(name.to_owned(), serde_json::json!({ "hash": hash, "size": bytes.len() }));
    };
    for face in 0..6 {
        put(&format!("{PANORAMA_FACES}{face}.png"), &format!("aa00000000000000000000000000000000000{face}00"), &png_header(512));
    }
    put("minecraft/texts/splashes.txt", "bb0000000000000000000000000000000000000000", b"from the store\n");
    std::fs::create_dir_all(root.join("assets/indexes")).unwrap();
    std::fs::write(
        root.join("assets/indexes/34.json"),
        serde_json::json!({ "objects": objects }).to_string(),
    )
    .unwrap();
}

#[test]
fn the_asset_store_comes_before_the_jar_so_real_faces_win_over_placeholders() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    install_with_store(game.path(), true);
    let layout = layout(home.path());
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    let manifest = read_manifest(&cache_dir(&layout)).unwrap();
    for face in 0..6 {
        let cached = format!("title/panorama_{face}.png");
        assert!(manifest.data_files.contains(&cached), "{cached} not cached");
        let bytes = std::fs::read(cache_dir(&layout).join(&cached)).unwrap();
        assert_eq!(png_size(&bytes), Some((512, 512)));
    }
    let splashes = std::fs::read_to_string(cache_dir(&layout).join("texts/splashes.txt")).unwrap();
    assert_eq!(splashes, "from the store\n");
}

#[test]
fn placeholder_faces_in_the_jar_are_not_taken_but_the_splashes_still_are() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    install_with_store(game.path(), false);
    let layout = layout(home.path());
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    let manifest = read_manifest(&cache_dir(&layout)).unwrap();
    assert_eq!(manifest.data_files, ["texts/splashes.txt"]);
    assert!(!cache_dir(&layout).join("title").exists());
}

#[test]
fn a_png_size_comes_from_its_header_and_other_bytes_have_none() {
    assert_eq!(png_size(&png_header(1024)), Some((1024, 1024)));
    assert_eq!(png_size(b"GIF89a not a png at all......"), None);
    assert_eq!(png_size(b"\x89PNG"), None);
}
