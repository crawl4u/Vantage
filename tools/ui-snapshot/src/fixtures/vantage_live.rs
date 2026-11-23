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

/// A settings page with one choice's list of values open.
pub(super) fn choice(tab: usize, id: &str) -> Fixture {
    let mut settings = Settings::default();
    let mut state = PanelState::default();
    state.event(&mut settings, &format!("tab_{tab}"), 1.0);
    state.event(&mut settings, &format!("choose_{id}"), 1.0);
    let page = menu_v2::page(&settings, &state).expect("a rebuilt page");
    let panel: Panel = serde_json::from_str(&page).expect("the page is a valid panel");
    Fixture::ModPanel(Box::new(panel), None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The host rejects an invalid page and keeps drawing the old one, so a page that fails here would
    /// look frozen in the game.
    fn assert_valid(page: &str, what: &str) {
        let panel: Panel = serde_json::from_str(page).unwrap_or_else(|e| panic!("{what} is not a panel: {e}"));
        panel.validate().unwrap_or_else(|e| panic!("{what} is invalid: {e}"));
    }

    #[test]
    fn every_page_the_redesign_draws_is_a_valid_panel_to_the_host() {
        for tab in 0..vantage::menu_controls::TABS.len() {
            if !menu_v2::handles(tab) {
                continue;
            }
            let mut settings = Settings::default();
            let mut state = PanelState::default();
            state.event(&mut settings, &format!("tab_{tab}"), 1.0);
            let page = menu_v2::page(&settings, &state).expect("a handled page");
            assert_valid(&page, &format!("page {tab}"));
        }
    }

    #[test]
    fn the_colour_picker_and_a_choice_list_are_valid_panels_to_the_host() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.event(&mut settings, "tab_10", 1.0);
        state.event(&mut settings, "choose_hit_color", 1.0);
        assert_valid(&menu_v2::page(&settings, &state).unwrap(), "a choice list");
        state.event(&mut settings, &format!("pick_{}", vantage::config::CUSTOM_ROW), 1.0);
        assert_valid(&menu_v2::page(&settings, &state).unwrap(), "the colour picker");
    }

    #[test]
    fn the_search_results_page_is_a_valid_panel_to_the_host() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.field(&mut settings, "module_search", "opacity", true);
        assert_valid(&menu_v2::mods_panel(&settings, &state), "the search results");
    }

    #[test]
    fn the_general_page_is_valid_with_each_keybind_section_open() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.event(&mut settings, "tab_6", 1.0);
        let page: serde_json::Value = serde_json::from_str(&menu_v2::page(&settings, &state).unwrap()).unwrap();
        let headings: Vec<String> = page["controls"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|c| c["id"].as_str())
            .filter(|id| id.starts_with("group_"))
            .map(str::to_owned)
            .collect();
        assert!(headings.len() > 3);
        for heading in headings {
            let mut settings = Settings::default();
            let mut state = PanelState::default();
            state.event(&mut settings, "tab_6", 1.0);
            state.event(&mut settings, &heading, 1.0);
            assert_valid(&menu_v2::page(&settings, &state).unwrap(), &format!("General with {heading} open"));
        }
    }
}
