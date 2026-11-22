//! The redesigned Mods page built from the Vantage mod's own mod list and default settings.

use ui::mod_panel::Panel;
use vantage::{config::Settings, menu_v2, panel::PanelState};

use super::Fixture;

/// The page for the default settings, with `favorite` starred and `enabled` filtering when asked.
pub(super) fn fixture(filter: usize, query: &str) -> Fixture {
    let mut settings = Settings::default();
    settings.favorites.push("hud_keystrokes".to_owned());
    settings.favorites.push("zoom_on".to_owned());
    let mut state = PanelState::default();
    state.event(&mut settings, &format!("flt_{filter}"), 1.0);
    state.field(&mut settings, "module_search", query, true);
    let page = menu_v2::mods_panel(&settings, &state);
    let panel: Panel = serde_json::from_str(&page).expect("the live page is a valid panel");
    Fixture::ModPanel(Box::new(panel), None, None)
}

/// A rebuilt settings page of the Vantage menu, opened by its tab.
pub(super) fn settings(tab: usize) -> Fixture {
    let mut settings = Settings::default();
    let mut state = PanelState::default();
    state.event(&mut settings, &format!("tab_{tab}"), 1.0);
    let page = menu_v2::page(&settings, &state).expect("a rebuilt page");
    let panel: Panel = serde_json::from_str(&page).expect("the page is a valid panel");
    Fixture::ModPanel(Box::new(panel), None, None)
}
