//! The redesigned Mods page built from the Vantage mod's own mod list and default settings.

use ui::mod_panel::Panel;
use vantage::{config::Settings, menu_v2, panel::PanelState};
use vantage_ui::mods::{Filter, Sort};

use super::Fixture;

/// The page for the default settings, with `favorite` starred and `enabled` filtering when asked.
pub(super) fn fixture(filter: Filter<'static>, query: &str) -> Fixture {
    let mut settings = Settings::default();
    settings.favorites.push("hud_keystrokes".to_owned());
    settings.favorites.push("zoom_on".to_owned());
    let page = menu_v2::mods_panel(&settings, &PanelState::default(), filter, Sort::Default, query);
    let panel: Panel = serde_json::from_str(&page).expect("the live page is a valid panel");
    Fixture::ModPanel(Box::new(panel), None, None)
}
