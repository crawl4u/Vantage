//! The redesigned menu, built from the mod's own tables and settings (see `vantage-ui`).
//!
//! The pages are pure functions of the settings: they return the panel description the host draws.
//! A control's declared id is the id the old menu's handlers already know (`PanelState::event`), so
//! presses and values reach the same settings.

use std::sync::OnceLock;

use vantage_ui::{
    mods::{self, Card, Filter, Sort, State},
    shell::{Entry, Section, Shell},
    tokens::Palette,
};

use crate::config::Settings;
use crate::menu::Value;
use crate::menu_controls::{COLLECTION, MODS};
use crate::panel::PanelState;

/// The Mods page's category names, by the category number in `MODS`.
pub const CATEGORIES: [&str; 3] = ["HUD", "Visual", "Mechanic"];

/// The cards' display names, in `MODS` order, read once from the generated collection.
fn names() -> &'static [String] {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let collection: serde_json::Value = serde_json::from_str(COLLECTION).unwrap_or_default();
        collection["items"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|item| item["values"]["#item_name"].as_str().unwrap_or("").to_owned())
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// One card per mod, with its switch and favourite taken from `settings`.
#[must_use]
pub fn cards<'a>(settings: &'a Settings, state: &'a PanelState) -> Vec<Card<'a>> {
    let names = names();
    MODS.iter()
        .enumerate()
        .map(|(i, &(id, lower, category))| Card {
            id,
            name: names.get(i).map_or(lower, String::as_str),
            category: CATEGORIES[usize::from(category).min(CATEGORIES.len() - 1)],
            on: matches!(crate::panel::value(settings, state, id), Value::Toggle(true)),
            favorite: settings.favorites.iter().any(|f| f == id),
            hidden: false,
        })
        .collect()
}

/// The sidebar the redesigned menu shows.
fn sections() -> [Section<'static>; 1] {
    const ENTRIES: [Entry<'static>; 1] = [Entry { id: "mods", title: "Mods", active: true }];
    [Section { heading: Some("Mods and options"), entries: &ENTRIES }]
}

/// The Mods page for the current settings, as the panel JSON the host takes.
#[must_use]
pub fn mods_panel(settings: &Settings, panel_state: &PanelState, filter: Filter<'_>, sort: Sort, query: &str) -> String {
    let cards = cards(settings, panel_state);
    let sections = sections();
    let shell = Shell {
        palette: Palette::dark(),
        brand: "Vantage",
        sections: &sections,
        title: "Mods",
        search_hint: "Search mods...",
        search: query,
        account: "Player",
        account_note: "",
    };
    let state = State { filter, sort, query };
    mods::panel(&shell, &cards, &CATEGORIES, &state).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mod_has_a_card_with_a_display_name_and_a_known_category() {
        let settings = Settings::default();
        let state = PanelState::default();
        let cards = cards(&settings, &state);
        assert_eq!(cards.len(), MODS.len());
        for card in &cards {
            assert!(!card.name.is_empty(), "{} has no name", card.id);
            assert!(CATEGORIES.contains(&card.category));
        }
    }

    #[test]
    fn a_favourite_in_the_settings_is_a_favourite_on_its_card() {
        let mut settings = Settings::default();
        let state = PanelState::default();
        let id = MODS[3].0;
        settings.favorites.push(id.to_owned());
        let cards = cards(&settings, &state);
        assert!(cards[3].favorite);
        assert_eq!(cards.iter().filter(|c| c.favorite).count(), 1);
    }

    #[test]
    fn the_page_is_valid_json_with_a_card_per_mod_and_a_control_per_press() {
        let page = mods_panel(&Settings::default(), &PanelState::default(), Filter::All, Sort::Default, "");
        let value: serde_json::Value = serde_json::from_str(&page).unwrap();
        let items = value["surface"]["collections"]["mods"]["definition"]["items"].as_array().unwrap();
        assert_eq!(items.len(), MODS.len());
        let order = value["surface"]["collections"]["mods"]["order"].as_array().unwrap();
        assert_eq!(order.len(), MODS.len());
        assert!(value["controls"].as_array().unwrap().len() >= MODS.len() * 3);
    }
}
