//! The redesigned menu, built from the mod's own tables and settings (see `vantage-ui`).
//!
//! The pages are pure functions of the settings: they return the panel description the host draws.
//! Their controls are declared with the ids the old menu's handlers already know
//! (`PanelState::event`, `PanelState::field`), so presses and typed text reach the same settings.

use std::sync::OnceLock;

use vantage_ui::{
    mods::{self, Card, Filter, Ids, Sort, State},
    shell::{Backdrop, Entry, Section, Shell},
    tokens::Palette,
};

use crate::config::Settings;
use crate::menu::Value;
use crate::menu_controls::{COLLECTION, CONTROLS, DOC_CONTROLS, MODS, MODS_PAGE};
use crate::panel::PanelState;

/// The Mods page's category names, by the category number in `MODS`.
pub const CATEGORIES: [&str; 3] = ["HUD", "Visual", "Mechanic"];

/// The generated collection's items, parsed once.
fn collection_items() -> &'static [serde_json::Value] {
    static ITEMS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    ITEMS.get_or_init(|| {
        let collection: serde_json::Value = serde_json::from_str(COLLECTION).unwrap_or_default();
        collection["items"].as_array().cloned().unwrap_or_default()
    })
}

/// The cards' display names, in `MODS` order.
fn names() -> Vec<&'static str> {
    collection_items()
        .iter()
        .map(|item| item["values"]["#item_name"].as_str().unwrap_or(""))
        .collect()
}

/// The id of each card's open button, in `MODS` order.
fn open_ids() -> Vec<&'static str> {
    collection_items()
        .iter()
        .map(|item| {
            // The collection numbers controls within the Mods page's own document.
            let local = item["controls"]["open"].as_u64().unwrap_or(0) as usize;
            DOC_CONTROLS[MODS_PAGE]
                .get(local)
                .and_then(|global| CONTROLS.get(usize::from(*global)))
                .map_or("", |control| control.1)
        })
        .collect()
}

/// One card per mod, with its switch and favourite taken from `settings`.
#[must_use]
pub fn cards<'a>(settings: &'a Settings, state: &'a PanelState) -> Vec<Card<'a>> {
    let names = names();
    let open = open_ids();
    MODS.iter()
        .enumerate()
        .map(|(i, &(id, lower, category))| Card {
            id,
            name: names.get(i).copied().filter(|name| !name.is_empty()).unwrap_or(lower),
            category: CATEGORIES[usize::from(category).min(CATEGORIES.len() - 1)],
            on: matches!(crate::panel::value(settings, state, id), Value::Toggle(true)),
            favorite: settings.favorites.iter().any(|f| f == id),
            hidden: false,
            open: open.get(i).copied().unwrap_or(""),
        })
        .collect()
}

/// The sidebar the redesigned menu shows.
fn sections() -> [Section<'static>; 1] {
    const ENTRIES: [Entry<'static>; 1] = [Entry { id: "mods", title: "Mods", active: true }];
    [Section { heading: Some("Mods and options"), entries: &ENTRIES }]
}

/// The filter the Mods grid's chip number selects (see `PanelState::grid_filter`).
fn filter_of(number: usize) -> Filter<'static> {
    match number {
        1 => Filter::Enabled,
        2..=4 => Filter::Category(CATEGORIES[number - 2]),
        5 => Filter::Favorites,
        _ => Filter::All,
    }
}

/// The chip number a filter is pressed as: the old menu's `flt_<n>` ids.
fn chip_number(filter: Filter<'_>) -> usize {
    match filter {
        Filter::Enabled => 1,
        Filter::Category(name) => CATEGORIES.iter().position(|c| *c == name).map_or(0, |i| i + 2),
        Filter::Favorites => 5,
        Filter::All | Filter::Hidden => 0,
    }
}

/// The sort the Mods grid's sort number selects.
fn sort_of(number: usize) -> Sort {
    match number {
        1 => Sort::Name,
        2 => Sort::EnabledFirst,
        _ => Sort::Default,
    }
}

/// The Mods page for the current settings and grid state, as the panel JSON the host takes. Its
/// controls carry the ids the old menu's handlers use: a mod's own id for its switch, `fav_<id>`,
/// `flt_<n>`, `sort` and `module_search`.
#[must_use]
pub fn mods_panel(settings: &Settings, panel_state: &PanelState) -> String {
    let cards = cards(settings, panel_state);
    let sections = sections();
    let query = panel_state.search_text();
    let shell = Shell {
        palette: Palette::dark(),
        brand: "Vantage",
        sections: &sections,
        title: "Mods",
        search_hint: "Search mods...",
        search: query,
        search_id: "module_search",
        account: "Player",
        account_note: "",
        backdrop: Backdrop {
            blur: settings.gameplay.menu_blur() as f32,
            brightness: settings.gameplay.menu_backdrop().0 as f32,
            saturation: settings.gameplay.menu_backdrop().1 as f32,
        },
    };
    let state = State {
        filter: filter_of(panel_state.grid_filter()),
        sort: sort_of(panel_state.grid_sort()),
        query,
    };
    let chip = |filter: Filter<'_>| format!("flt_{}", chip_number(filter));
    let toggle = |id: &str| id.to_owned();
    let favorite = |id: &str| format!("fav_{id}");
    let ids = Ids { sort: "sort", search: "module_search", chip: &chip, toggle: &toggle, favorite: &favorite };
    mods::panel(&shell, &cards, &CATEGORIES, &state, &ids).to_string()
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
    fn the_page_is_valid_json_with_a_card_per_mod_and_the_old_menus_ids() {
        let page = mods_panel(&Settings::default(), &PanelState::default());
        let value: serde_json::Value = serde_json::from_str(&page).unwrap();
        let items = value["surface"]["collections"]["mods"]["definition"]["items"].as_array().unwrap();
        assert_eq!(items.len(), MODS.len());
        let ids: Vec<&str> = value["controls"].as_array().unwrap().iter().filter_map(|c| c["id"].as_str()).collect();
        for id in ["module_search", "sort", "flt_0", "flt_1", MODS[0].0, "fav_hud_keystrokes"] {
            assert!(ids.contains(&id), "{id} is not declared");
        }
    }

    #[test]
    fn every_chip_presses_the_filter_the_old_menu_names_and_every_open_button_has_an_id() {
        for number in 0..6 {
            assert_eq!(chip_number(filter_of(number)), number);
        }
        let open = open_ids();
        assert_eq!(open.len(), MODS.len());
        assert!(open.iter().all(|id| !id.is_empty()), "a card has no open control");
    }

    #[test]
    fn the_grids_filter_state_selects_the_cards_shown() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        settings.favorites.push(MODS[3].0.to_owned());
        state.event(&mut settings, "flt_5", 1.0);
        let page: serde_json::Value = serde_json::from_str(&mods_panel(&settings, &state)).unwrap();
        let order = page["surface"]["collections"]["mods"]["order"].as_array().unwrap();
        assert_eq!(order.len(), 1);
        assert_eq!(order[0], 3);
    }

    #[test]
    fn every_declared_control_id_is_valid_and_unique() {
        let page = mods_panel(&Settings::default(), &PanelState::default());
        let value: serde_json::Value = serde_json::from_str(&page).unwrap();
        let mut seen = std::collections::HashSet::new();
        for control in value["controls"].as_array().unwrap() {
            let id = control["id"].as_str().unwrap();
            assert!(
                !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b)),
                "{id:?} is not a valid control id"
            );
            assert!(seen.insert(id.to_owned()), "{id} is declared twice");
        }
    }
}
