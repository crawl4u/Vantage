//! The Java-style menus, drawn from the sprites cached out of a Java Edition install.

use super::{Arc, Fixture, GUI_SCALE, SIZE, SMALL_SIZE, State};
use launcher::menu::{
    MenuScreen, MenuView,
    auth::AuthState,
    settings_options::{SettingId, menu_style::MenuStyle},
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
        name: "java-home-small",
        size: SMALL_SIZE,
        gui_scale: 1,
        fixture: java_home,
    },
];

/// A signed-in home with the Java style chosen.
fn java_home() -> Fixture {
    let mut view = MenuView::new(true, "Player".into());
    view.screen = MenuScreen::Home;
    view.auth_state = AuthState::Authenticated;
    let options = Arc::make_mut(&mut view.settings_options);
    assert!(options.set(SettingId::MenuStyle.index(), MenuStyle::Java.value()));
    Fixture::Menu(Box::new(view))
}
