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

/// Every GUI file the menus draw, as paths relative to `assets/minecraft/`.
fn gui_paths() -> Vec<String> {
    GUI_FILES.iter().map(|file| gui_path(file)).collect()
}

/// The required GUI files only.
fn required_paths() -> Vec<String> {
    REQUIRED_GUI_FILES
        .iter()
        .map(|file| gui_path(file))
        .collect()
}

fn button_path() -> String {
    gui_path("sprites/widget/button.png")
}

/// A version folder with a jar holding `files` (paths relative to `assets/minecraft/`, each with
/// its own path as its bytes) and a JSON of the given type and release time.
fn install_version(root: &Path, id: &str, kind: &str, released: &str, files: &[String]) {
    let dir = root.join("versions").join(id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(format!("{id}.json")),
        format!(r#"{{"id":"{id}","type":"{kind}","releaseTime":"{released}"}}"#),
    )
    .unwrap();
    let mut zip =
        zip::ZipWriter::new(std::fs::File::create(dir.join(format!("{id}.jar"))).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    for file in files {
        zip.start_file(format!("assets/minecraft/{file}"), options)
            .unwrap();
        zip.write_all(format!("{id}:{file}").as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn the_newest_release_wins_over_snapshots_and_older_releases() {
    let game = tempfile::tempdir().unwrap();
    install_version(
        game.path(),
        "1.20.4",
        "release",
        "2023-12-07T00:00:00+00:00",
        &required_paths(),
    );
    install_version(
        game.path(),
        "1.21.8",
        "release",
        "2025-07-17T00:00:00+00:00",
        &required_paths(),
    );
    install_version(
        game.path(),
        "26w01a",
        "snapshot",
        "2026-01-05T00:00:00+00:00",
        &required_paths(),
    );
    let found = find_client_jar(&[game.path().to_owned()]).unwrap();
    assert_eq!(found.version, "1.21.8");
}

#[test]
fn a_snapshot_is_used_when_no_release_exists() {
    let game = tempfile::tempdir().unwrap();
    install_version(
        game.path(),
        "26w01a",
        "snapshot",
        "2026-01-05T00:00:00+00:00",
        &required_paths(),
    );
    assert_eq!(
        find_client_jar(&[game.path().to_owned()]).unwrap().version,
        "26w01a"
    );
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
fn sprites_are_cached_at_javas_own_paths_and_a_complete_install_is_ready() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    install_version(
        game.path(),
        "1.21.8",
        "release",
        "2025-07-17T00:00:00+00:00",
        &gui_paths(),
    );
    let layout = layout(home.path());
    let status = prepare_from(&layout, &[game.path().to_owned()]);
    assert_eq!(
        status,
        Status::Ready {
            version: "1.21.8".to_owned()
        }
    );
    let cached = cache_dir(&layout).join(button_path());
    assert_eq!(
        std::fs::read_to_string(cached).unwrap(),
        format!("1.21.8:{}", button_path())
    );
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
        &[button_path()],
    );
    match prepare_from(&layout(home.path()), &[game.path().to_owned()]) {
        Status::Incomplete { version, missing } => {
            assert_eq!(version, "1.21.8");
            assert!(missing.contains(&gui_path("sprites/widget/button_highlighted.png")));
        }
        other => panic!("expected an incomplete install, got {other:?}"),
    }
}

#[test]
fn with_no_install_nothing_is_ready_and_a_removed_install_keeps_its_cache() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let layout = layout(home.path());
    assert_eq!(
        prepare_from(&layout, &[game.path().to_owned()]),
        Status::NoJavaInstall
    );
    install_version(
        game.path(),
        "1.21.8",
        "release",
        "2025-07-17T00:00:00+00:00",
        &gui_paths(),
    );
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    std::fs::remove_dir_all(game.path().join("versions")).unwrap();
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
}

#[test]
fn an_unchanged_jar_is_not_read_again_but_a_new_version_is() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let layout = layout(home.path());
    install_version(
        game.path(),
        "1.21.8",
        "release",
        "2025-07-17T00:00:00+00:00",
        &gui_paths(),
    );
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    // A cached file that was changed on disk survives a second prepare of the same jar.
    let button = cache_dir(&layout).join(button_path());
    std::fs::write(&button, "edited").unwrap();
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    assert_eq!(std::fs::read_to_string(&button).unwrap(), "edited");
    install_version(
        game.path(),
        "1.21.9",
        "release",
        "2025-09-30T00:00:00+00:00",
        &gui_paths(),
    );
    assert_eq!(
        prepare_from(&layout, &[game.path().to_owned()]),
        Status::Ready {
            version: "1.21.9".to_owned()
        }
    );
    assert_eq!(
        std::fs::read_to_string(&button).unwrap(),
        format!("1.21.9:{}", button_path())
    );
}

#[test]
fn a_cache_from_an_older_format_is_read_from_the_jar_again() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let layout = layout(home.path());
    install_version(
        game.path(),
        "1.21.8",
        "release",
        "2025-07-17T00:00:00+00:00",
        &gui_paths(),
    );
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    let button = cache_dir(&layout).join(button_path());
    std::fs::write(&button, "edited").unwrap();
    // The same jar, but a manifest written under an older format.
    let mut manifest = read_manifest(&cache_dir(&layout)).unwrap();
    manifest.format = 0;
    std::fs::write(
        manifest_path(&cache_dir(&layout)),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    assert_eq!(
        std::fs::read_to_string(&button).unwrap(),
        format!("1.21.8:{}", button_path())
    );
    assert_eq!(
        read_manifest(&cache_dir(&layout)).unwrap().format,
        CACHE_FORMAT
    );
}

/// A game folder with a jar holding what `add` writes, a JSON naming asset index 34, and, when
/// `store` is set, an asset store with the real panorama faces and splash lines.
fn install_with(root: &Path, store: bool, add: impl FnOnce(&mut dyn FnMut(&str, &[u8]))) {
    let dir = root.join("versions/26.3");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("26.3.json"),
        r#"{"type":"release","releaseTime":"2026-09-15","assetIndex":{"id":"34"}}"#,
    )
    .unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(dir.join("26.3.jar")).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    let mut write = |name: &str, bytes: &[u8]| {
        zip.start_file(format!("assets/minecraft/{name}"), options)
            .unwrap();
        zip.write_all(bytes).unwrap();
    };
    for path in gui_paths() {
        write(&path, b"png");
    }
    add(&mut write);
    zip.finish().unwrap();
    if !store {
        return;
    }
    let mut objects = serde_json::Map::new();
    let mut put = |name: &str, hash: &str, bytes: &[u8]| {
        let path = root.join("assets/objects").join(&hash[..2]);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join(hash), bytes).unwrap();
        objects.insert(
            format!("minecraft/{name}"),
            serde_json::json!({ "hash": hash, "size": bytes.len() }),
        );
    };
    for (index, face) in PANORAMA_FACES.iter().enumerate() {
        put(
            face,
            &format!("aa0000000000000000000000000000000000000{index}"),
            &png_header(512),
        );
    }
    put(
        SPLASHES,
        "bb0000000000000000000000000000000000000000",
        b"from the store\n",
    );
    std::fs::create_dir_all(root.join("assets/indexes")).unwrap();
    std::fs::write(
        root.join("assets/indexes/34.json"),
        serde_json::json!({ "objects": objects }).to_string(),
    )
    .unwrap();
}

/// The header of a PNG `side` texels square: enough for the size check, not a picture.
fn png_header(side: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend(side.to_be_bytes());
    bytes.extend(side.to_be_bytes());
    bytes
}

#[test]
fn the_font_definitions_and_the_sheets_they_name_are_cached() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    install_with(game.path(), false, |write| {
        write(
            "font/include/default.json",
            br#"{"providers":[
                {"type":"bitmap","file":"minecraft:font/ascii.png","ascent":7,"chars":["ab"]},
                {"type":"bitmap","file":"minecraft:font/../../secret.png","ascent":7,"chars":["a"]},
                {"type":"bitmap","file":"minecraft:font/sub/dir.png","ascent":7,"chars":["a"]},
                {"type":"ttf","file":"minecraft:font/x.ttf"}]}"#,
        );
        write(
            "font/include/space.json",
            br#"{"providers":[{"type":"space","advances":{" ":4}}]}"#,
        );
        write("textures/font/ascii.png", b"sheet");
        write("textures/secret.png", b"no");
    });
    let layout = layout(home.path());
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    let cache = cache_dir(&layout);
    let manifest = read_manifest(&cache).unwrap();
    for file in [
        "font/include/default.json",
        "font/include/space.json",
        "textures/font/ascii.png",
    ] {
        assert!(
            manifest.files.iter().any(|cached| cached == file),
            "{file} not cached"
        );
        assert!(cache.join(file).is_file());
    }
    assert_eq!(
        std::fs::read(cache.join("textures/font/ascii.png")).unwrap(),
        b"sheet"
    );
    // Names that would leave the font folder, and non-bitmap providers, are never fetched.
    assert!(
        !cache.join("textures/secret.png").exists() && !cache.join("textures/font/x.ttf").exists()
    );
}

#[test]
fn a_jar_without_font_files_has_no_java_font_but_still_works() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    install_version(
        game.path(),
        "1.21.8",
        "release",
        "2025-07-17T00:00:00+00:00",
        &gui_paths(),
    );
    let layout = layout(home.path());
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    let manifest = read_manifest(&cache_dir(&layout)).unwrap();
    assert!(manifest.files.iter().all(|file| !file.starts_with("font/")));
}

#[test]
fn the_asset_store_comes_before_the_jar_so_real_faces_win_over_placeholders() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    install_with(game.path(), true, |write| {
        for face in PANORAMA_FACES {
            write(face, &png_header(1));
        }
        write(SPLASHES, b"from the jar\n");
    });
    let layout = layout(home.path());
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    let cache = cache_dir(&layout);
    for face in PANORAMA_FACES {
        let bytes = std::fs::read(cache.join(face)).unwrap();
        assert_eq!(png_size(&bytes), Some((512, 512)), "{face}");
    }
    assert_eq!(
        std::fs::read_to_string(cache.join(SPLASHES)).unwrap(),
        "from the store\n"
    );
}

#[test]
fn placeholder_faces_in_the_jar_are_not_taken_but_the_splashes_still_are() {
    let game = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    install_with(game.path(), false, |write| {
        for face in PANORAMA_FACES {
            write(face, &png_header(1));
        }
        write(SPLASHES, b"from the jar\n");
    });
    let layout = layout(home.path());
    assert!(prepare_from(&layout, &[game.path().to_owned()]).ready());
    let cache = cache_dir(&layout);
    assert!(!cache.join(PANORAMA_FACES[0]).exists());
    assert_eq!(
        std::fs::read_to_string(cache.join(SPLASHES)).unwrap(),
        "from the jar\n"
    );
}

#[test]
fn a_png_size_comes_from_its_header_and_other_bytes_have_none() {
    assert_eq!(png_size(&png_header(1024)), Some((1024, 1024)));
    assert_eq!(png_size(b"GIF89a not a png at all......"), None);
    assert_eq!(png_size(b"\x89PNG"), None);
}

#[test]
fn only_plain_minecraft_bitmap_sheets_are_named() {
    let definition = br#"{"providers":[
        {"type":"bitmap","file":"minecraft:font/ascii.png"},
        {"type":"bitmap","file":"other:font/ascii.png"},
        {"type":"bitmap","file":"minecraft:font/a/b.png"},
        {"type":"ttf","file":"minecraft:font/f.ttf"}]}"#;
    assert_eq!(bitmap_sheets(definition), ["ascii.png"]);
    assert!(bitmap_sheets(b"not json").is_empty());
}

/// Runs against a real Java Edition folder named by `VANTAGE_JAVA_DIR`; skipped without one.
#[test]
fn a_real_java_install_caches_every_menu_file() {
    let Some(game) = std::env::var_os("VANTAGE_JAVA_DIR").map(PathBuf::from) else {
        eprintln!("skipped: missing fixture, set VANTAGE_JAVA_DIR to a Java Edition folder");
        return;
    };
    let home = tempfile::tempdir().unwrap();
    let layout = layout(home.path());
    let status = prepare_from(&layout, &[game]);
    assert!(status.ready(), "{status:?}");
    let cache = cache_dir(&layout);
    let manifest = read_manifest(&cache).unwrap();
    assert!(
        manifest.missing.is_empty(),
        "missing: {:?}",
        manifest.missing
    );
    for path in gui_paths() {
        let bytes = std::fs::read(cache.join(&path)).unwrap();
        assert!(bytes.starts_with(b"\x89PNG"), "{path} is not a PNG");
    }
    // The font's definitions and its bitmap sheets come along.
    for file in [
        "font/include/default.json",
        "font/include/space.json",
        "textures/font/ascii.png",
    ] {
        assert!(
            manifest.files.iter().any(|cached| cached == file),
            "{file} not cached"
        );
        assert!(cache.join(file).is_file());
    }
    // The title's real panorama faces, from the launcher's asset store, and the splash lines.
    for file in [PANORAMA_FACES[0], PANORAMA_FACES[5], SPLASHES] {
        assert!(
            manifest.files.iter().any(|cached| cached == file),
            "{file} not cached"
        );
    }
    assert!(
        png_size(&std::fs::read(cache.join(PANORAMA_FACES[0])).unwrap()).is_some_and(
            |(width, height)| width >= MIN_PANORAMA_SIDE && height >= MIN_PANORAMA_SIDE
        )
    );
    // The widget sprites carry nine-slice borders.
    assert!(cache.join(mcmeta(&button_path())).is_file());
}
