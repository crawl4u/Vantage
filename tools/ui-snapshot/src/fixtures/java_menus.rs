//! The Java-style menus, drawn from the sprites cached out of a Java Edition install.

use super::{Arc, Fixture, GUI_SCALE, SIZE, SMALL_SIZE, State};
use launcher::menu::{
    JavaSettingsView, MenuScreen, MenuView,
    auth::AuthState,
    settings_options::{SettingId, menu_style::MenuStyle},
    settings_sections::SettingSection,
};

/// The Java-style title screen at the usual and the smallest window sizes.
pub(super) const STATES: &[State] = &[
    State {
        name: "java-home-scale-2",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: || java_home_at(Some(2)),
    },
    State {
        name: "java-home-small",
        size: SMALL_SIZE,
        gui_scale: 1,
        fixture: java_home,
    },
    State {
        name: "java-pause",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: java_pause,
    },
    State {
        name: "java-options",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: || java_settings(JavaSettingsView::Hub, SettingSection::Video),
    },
    State {
        name: "java-options-video",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: || java_settings(JavaSettingsView::Page, SettingSection::Video),
    },
    State {
        name: "java-options-sound",
        size: SMALL_SIZE,
        gui_scale: 1,
        fixture: || java_settings(JavaSettingsView::Page, SettingSection::Sound),
    },
    State {
        name: "java-servers",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: java_servers,
    },
    State {
        name: "java-packs",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: java_packs,
    },
    State {
        name: "java-packs-folder",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: java_pack_folder,
    },
    State {
        name: "java-options-controls",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: || java_settings(JavaSettingsView::Page, SettingSection::KeyboardAndMouse),
    },
    State {
        name: "java-options-account",
        size: SMALL_SIZE,
        gui_scale: 1,
        fixture: || java_settings(JavaSettingsView::Page, SettingSection::Account),
    },
    State {
        name: "java-mods",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: java_mods,
    },
    State {
        name: "java-storage",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: java_storage,
    },
    State {
        name: "java-language",
        size: SMALL_SIZE,
        gui_scale: 1,
        fixture: java_language,
    },
    State {
        name: "java-profile",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: java_profile,
    },
    State {
        name: "java-skins",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: java_skins,
    },
    State {
        name: "java-home",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: java_home,
    },
    State {
        name: "java-worlds",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: java_worlds,
    },
    State {
        name: "java-world-create",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: || java_world_form(launcher::local_worlds::Screen::Create),
    },
    State {
        name: "java-world-importing",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: || java_world_form(launcher::local_worlds::Screen::Importing),
    },
    State {
        name: "java-world-delete",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: || java_world_form(launcher::local_worlds::Screen::ConfirmDelete),
    },
    State {
        name: "java-loading",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: java_loading,
    },
    State {
        name: "java-shaders",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: java_shaders,
    },
    State {
        name: "java-servers-featured",
        size: SMALL_SIZE,
        gui_scale: 1,
        fixture: java_featured_servers,
    },
];

/// The Resource Packs page inside a folder that is being renamed.
fn java_pack_folder() -> Fixture {
    let Fixture::Menu(mut view) = java_packs() else {
        unreachable!("the packs fixture is a menu");
    };
    let mut snapshot = (*view.global_resources).clone();
    snapshot.folder = "PVP".to_owned();
    view.global_resources = Arc::new(snapshot);
    view.pack_renaming = true;
    view.pack_folder_name = "PVP".to_owned();
    Fixture::Menu(view)
}

/// A Java page of `section`, in the Java style, ready for the caller to fill in.
fn java_page(section: SettingSection) -> Box<MenuView> {
    let mut view = MenuView::new(true, "Player".into());
    view.screen = MenuScreen::Settings;
    view.auth_state = AuthState::Authenticated;
    view.settings.section = section.definition().index;
    view.settings.java_view = JavaSettingsView::Page;
    let options = Arc::make_mut(&mut view.settings_options);
    assert!(options.set(SettingId::MenuStyle.index(), MenuStyle::Java.value()));
    Box::new(view)
}

/// The Mods page with three installed mods, one active and one chosen.
fn java_mods() -> Fixture {
    let mut view = java_page(SettingSection::Mods);
    let installed = |id: u64, name: &str| launcher::mods::InstalledMod {
        id,
        file: format!("{name}.wasm"),
        bytes: 1,
        permissions: Vec::new(),
        metadata: ui::mod_metadata::ModMetadata {
            name: Some(name.to_owned()),
            version: Some("1.0".to_owned()),
            authors: Some("Someone".to_owned()),
            ..Default::default()
        },
    };
    let mut snapshot = launcher::mods::Snapshot::default();
    snapshot.library.installed = vec![
        installed(1, "Vantage"),
        installed(2, "Zoom"),
        installed(3, "Keystrokes"),
    ];
    snapshot.library.active = vec![launcher::mods::ActiveMod {
        id: 1,
        status: launcher::mods::Status::Running,
    }];
    snapshot.details_expanded = Some(2);
    view.mods = Arc::new(snapshot);
    Fixture::Menu(view)
}

/// The Storage page with worlds and downloaded packs.
fn java_storage() -> Fixture {
    let mut view = java_page(SettingSection::StorageManagement);
    let item = |name: &str, bytes: u64| launcher::menu::settings_storage::StorageItem {
        name: name.to_owned(),
        bytes,
        date: "2026-11-02".to_owned(),
        version: "1.21.4".to_owned(),
        ..Default::default()
    };
    let storage = Arc::make_mut(&mut view.storage);
    storage.worlds = vec![item("Home Base", 48 << 20), item("Creative", 5 << 30)];
    storage.cached = vec![item("Server Textures", 12 << 20)];
    storage.selected_world = Some(0);
    storage.opens_folders = true;
    Fixture::Menu(view)
}

/// The Language page with a handful of languages.
fn java_language() -> Fixture {
    let mut view = java_page(SettingSection::Language);
    view.language_choices = [
        ("en_US", "English (US)"),
        ("de_DE", "Deutsch"),
        ("fr_FR", "FranÃ§ais"),
        ("es_ES", "EspaÃ±ol"),
    ]
    .iter()
    .map(|(code, name)| ((*code).to_owned(), (*name).to_owned()))
    .collect();
    Fixture::Menu(view)
}

/// The create screen, or a message over the world list, in the Java style.
fn java_world_form(screen: launcher::local_worlds::Screen) -> Fixture {
    let mut view = MenuView::new(true, "Player".into());
    view.screen = MenuScreen::Play;
    view.auth_state = AuthState::Authenticated;
    view.local.screen = screen;
    view.local.tab = launcher::local_worlds::Tab::General;
    let options = Arc::make_mut(&mut view.settings_options);
    assert!(options.set(SettingId::MenuStyle.index(), MenuStyle::Java.value()));
    Fixture::Menu(Box::new(view))
}

/// A world being started, in the Java style, with a known amount done.
fn java_loading() -> Fixture {
    let mut view = MenuView::new(true, "Player".into());
    view.screen = MenuScreen::Play;
    view.auth_state = AuthState::Authenticated;
    view.local.progress = Some(launcher::local_worlds::Progress {
        stage: launcher::local_worlds::Stage::Connecting,
        fraction: Some(0.4),
        detail: "My World".to_owned(),
    });
    let options = Arc::make_mut(&mut view.settings_options);
    assert!(options.set(SettingId::MenuStyle.index(), MenuStyle::Java.value()));
    Fixture::Menu(Box::new(view))
}

/// The profile's overview with the totals and a few achievements.
fn java_profile() -> Fixture {
    let mut view = MenuView::new(true, "ExamplePlayer".into());
    view.screen = MenuScreen::Profile;
    view.auth_state = AuthState::Authenticated;
    let profile = &mut view.feeds.profile;
    profile.loaded = true;
    profile.avatar_loaded = true;
    profile.featured_screenshot_loaded = true;
    profile.achievements_loaded = true;
    profile.gamertag = "ExamplePlayer".into();
    profile.presence = "Online".into();
    profile.friends = Some(12);
    profile.followers = Some(3);
    let entry = |name: &str, about: &str, locked: bool| bridge::ProfileAchievement {
        id: name.to_owned(),
        name: name.to_owned(),
        description: about.to_owned(),
        gamerscore: Some(20),
        locked,
        date_unlocked: if locked {
            String::new()
        } else {
            "2026-11-01".to_owned()
        },
        suggested_order: locked.then_some(1),
        ..Default::default()
    };
    profile.achievements = Some(bridge::ProfileAchievements {
        unlocked: 21,
        total: 62,
        current_gamerscore: Some(420),
        max_gamerscore: Some(1240),
        entries: vec![
            entry("Taking Inventory", "Open your inventory.", false),
            entry(
                "Getting Wood",
                "Punch a tree until a block of wood pops out.",
                false,
            ),
            entry("Benchmarking", "Craft a workbench with four planks.", true),
        ],
    });
    let options = Arc::make_mut(&mut view.settings_options);
    assert!(options.set(SettingId::MenuStyle.index(), MenuStyle::Java.value()));
    Fixture::Menu(Box::new(view))
}

/// The Skin Presets list with the starter skins and one of the player's own.
fn java_skins() -> Fixture {
    let mut view = MenuView::new(true, "Player".into());
    view.screen = MenuScreen::DressingRoom;
    view.auth_state = AuthState::Authenticated;
    use launcher::dressing_room::{
        DressingRoomSkin, DressingRoomView, STARTER_SKIN_NAMES, SkinModel,
    };
    let skin = |index: usize, name: String, imported: bool| DressingRoomSkin {
        engine_version: render_api::DEFAULT_SKIN_GEOMETRY_ENGINE_VERSION.into(),
        id: index.to_string(),
        name,
        path: String::new(),
        imported,
        model: if index == 1 {
            SkinModel::Slim
        } else {
            SkinModel::Classic
        },
        skin: render_api::StandardSkin {
            width: render_api::CLASSIC_SKIN_SIDE as u32,
            height: render_api::CLASSIC_SKIN_SIDE as u32,
            rgba8: vec![160; render_api::CLASSIC_SKIN_SIDE.pow(2) * 4].into(),
            cape: None,
            geometry: None,
        },
    };
    let mut skins: Vec<DressingRoomSkin> = STARTER_SKIN_NAMES
        .iter()
        .enumerate()
        .map(|(index, name)| skin(index, (*name).to_owned(), false))
        .collect();
    skins.push(skin(skins.len(), "Backroom Warden".to_owned(), true));
    view.dressing_room = Arc::new(DressingRoomView {
        skins: skins.into(),
        selected: Some(1),
        ..Default::default()
    });
    let options = Arc::make_mut(&mut view.settings_options);
    assert!(options.set(SettingId::MenuStyle.index(), MenuStyle::Java.value()));
    Fixture::Menu(Box::new(view))
}

/// The world list with three worlds, the second chosen.
fn java_worlds() -> Fixture {
    let mut view = MenuView::new(true, "Player".into());
    view.screen = MenuScreen::Play;
    view.auth_state = AuthState::Authenticated;
    let card = |name: &str, mode: &str, size: &str| launcher::menu::LocalWorldCard {
        name: name.to_owned(),
        game_mode: mode.to_owned(),
        world_type: "Default".to_owned(),
        server: String::new(),
        date: "11/2/2026".to_owned(),
        size: size.to_owned(),
        thumbnail: None,
    };
    view.local_worlds = vec![
        card("Home Base", "Survival", "48.0 MB"),
        card("Creative Plots", "Creative", "1.2 GB"),
        card("Parkour", "Adventure", "6.5 MB"),
    ]
    .into();
    view.feeds.selected_play_world = Some(1);
    let options = Arc::make_mut(&mut view.settings_options);
    assert!(options.set(SettingId::MenuStyle.index(), MenuStyle::Java.value()));
    Fixture::Menu(Box::new(view))
}

/// The Shader Packs page with two packs, one active.
fn java_shaders() -> Fixture {
    let mut view = MenuView::new(true, "Player".into());
    view.screen = MenuScreen::Settings;
    view.auth_state = AuthState::Authenticated;
    view.settings.section = SettingSection::Shaders.definition().index;
    view.settings.java_view = JavaSettingsView::Page;
    view.shader_packs = vec!["BSL v8.zip".to_owned(), "Complementary".to_owned()].into();
    view.shader_packs_revision = 1;
    view.render_mode = ui::RenderMode::ShaderPack;
    view.renderer = Some(launcher::menu::RendererChoice::ShaderPack(0, 1));
    let options = Arc::make_mut(&mut view.settings_options);
    assert!(options.set(SettingId::MenuStyle.index(), MenuStyle::Java.value()));
    Fixture::Menu(Box::new(view))
}

/// The Resource Packs page with packs in both lists, one chosen.
fn java_packs() -> Fixture {
    let mut view = MenuView::new(true, "Player".into());
    view.screen = MenuScreen::Settings;
    view.auth_state = AuthState::Authenticated;
    view.settings.section = SettingSection::GlobalTexturePack.definition().index;
    view.settings.java_view = JavaSettingsView::Page;
    let pack = |name: &str, description: &str| resource_pack::InstalledPack {
        id: Default::default(),
        version: [1, 2, 0],
        name: name.to_owned(),
        description: description.to_owned(),
        min_engine_version: None,
        subpacks: Vec::new(),
        revision: 0,
    };
    view.global_resources = Arc::new(launcher::global_resources::Snapshot {
        available: vec![
            pack(
                "Faithful 64x",
                "Doubles the detail of every block and item.",
            ),
            pack("Bare Bones", "A flat, minimal look."),
            pack("Old Library", ""),
        ],
        active: vec![pack("Vonpack", "Dark menus and blue buttons.")],
        selected: Some((false, 1)),
        message: "Texture packs applied.".to_owned(),
        folders: vec!["PVP".to_owned(), "Textures".to_owned()],
        available_folders: vec!["PVP".to_owned(), String::new(), String::new()],
        ..Default::default()
    });
    let options = Arc::make_mut(&mut view.settings_options);
    assert!(options.set(SettingId::MenuStyle.index(), MenuStyle::Java.value()));
    Fixture::Menu(Box::new(view))
}

/// The Multiplayer screen's featured tab, with one featured and one creator server.
fn java_featured_servers() -> Fixture {
    let Fixture::Menu(mut view) = java_servers() else {
        unreachable!("the servers fixture is a menu");
    };
    let card = |name: &str, address: &str, caption: &str| launcher::menu::MenuServerCard {
        name: name.into(),
        address: address.into(),
        caption: caption.into(),
        image_path: String::new(),
        icon: None,
    };
    view.featured = vec![
        card("Featured Realm", "featured.example.test", "Weekly events"),
        card("Maker Hub", "maker.example.test", "By a creator"),
    ]
    .into();
    view.feeds.details.insert(
        "maker.example.test".to_owned(),
        launcher::menu::ServerDetails {
            group: "creator".to_owned(),
            ..Default::default()
        },
    );
    view.feeds.java_server_group = launcher::menu::server_list::ServerGroup::Featured;
    Fixture::Menu(view)
}

/// The Multiplayer screen in the Java style, with saved servers, one answering and one not.
fn java_servers() -> Fixture {
    let mut view = MenuView::new(true, "Player".into());
    view.screen = MenuScreen::Servers;
    view.auth_state = AuthState::Authenticated;
    let saved = |name: &str, address: &str| launcher::menu::SavedServer {
        name: name.to_owned(),
        address: address.to_owned(),
        favorite: false,
        last_joined_unix: 0,
    };
    view.servers = vec![
        saved("Survival Realm", "play.example.test:19132"),
        saved("Creative Plots", "creative.example.test:19132"),
        saved("Old Server", "gone.example.test:19132"),
    ]
    .into();
    view.feeds.pings.insert(
        "play.example.test:19132".to_owned(),
        launcher::menu::PingInfo {
            motd: "A friendly survival server with events every weekend and a long message".into(),
            online: true,
            players: 12,
            max_players: 100,
            ping_ms: 48,
        },
    );
    view.feeds.pings.insert(
        "creative.example.test:19132".to_owned(),
        launcher::menu::PingInfo {
            motd: "Build with friends".into(),
            online: true,
            players: 3,
            max_players: 20,
            ping_ms: 420,
        },
    );
    view.feeds.pings.insert(
        "gone.example.test:19132".to_owned(),
        launcher::menu::PingInfo::default(),
    );
    view.feeds.select_saved(0);
    let options = Arc::make_mut(&mut view.settings_options);
    assert!(options.set(SettingId::MenuStyle.index(), MenuStyle::Java.value()));
    Fixture::Menu(Box::new(view))
}

/// Settings in the Java style, on the list of pages or on the page of `section`.
fn java_settings(view_kind: JavaSettingsView, section: SettingSection) -> Fixture {
    let mut view = MenuView::new(true, "Player".into());
    view.screen = MenuScreen::Settings;
    view.auth_state = AuthState::Authenticated;
    view.settings.section = section.definition().index;
    view.settings.java_view = view_kind;
    let options = Arc::make_mut(&mut view.settings_options);
    assert!(options.set(SettingId::MenuStyle.index(), MenuStyle::Java.value()));
    Fixture::Menu(Box::new(view))
}

/// The pause menu with the Java style chosen.
fn java_pause() -> Fixture {
    let mut view = MenuView::new(true, "Player".into());
    view.screen = MenuScreen::Pause;
    view.auth_state = AuthState::Authenticated;
    let options = Arc::make_mut(&mut view.settings_options);
    assert!(options.set(SettingId::MenuStyle.index(), MenuStyle::Java.value()));
    Fixture::Menu(Box::new(view))
}

/// A signed-in home with the Java style chosen and Java's Auto GUI scale.
fn java_home() -> Fixture {
    java_home_at(None)
}

/// The same with a fixed Java GUI scale, or Auto for none.
fn java_home_at(scale: Option<i32>) -> Fixture {
    let mut view = MenuView::new(true, "Player".into());
    view.screen = MenuScreen::Home;
    view.auth_state = AuthState::Authenticated;
    let options = Arc::make_mut(&mut view.settings_options);
    assert!(options.set(SettingId::MenuStyle.index(), MenuStyle::Java.value()));
    if let Some(scale) = scale {
        assert!(options.set(SettingId::JavaGuiScale.index(), scale));
    }
    Fixture::Menu(Box::new(view))
}
