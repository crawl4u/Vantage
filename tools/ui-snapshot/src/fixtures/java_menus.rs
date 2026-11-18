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
        name: "java-home",
        size: SIZE,
        gui_scale: GUI_SCALE,
        fixture: java_home,
    },
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
