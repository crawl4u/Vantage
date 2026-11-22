//! The redesigned menu, built from the mod's own tables and settings (see `vantage-ui`).
//!
//! The pages are pure functions of the settings: they return the panel description the host draws.
//! Their controls are declared with the ids the old menu's handlers already know
//! (`PanelState::event`, `PanelState::field`), so presses and typed text reach the same settings.

use std::sync::OnceLock;

use vantage_ui::{
    mods::{self, Card, Filter, Ids, Sort, State},
    options::{self, Group, Opt},
    shell::{Backdrop, Entry, Section, Shell},
    tokens::Palette,
};

use crate::config::Settings;
use crate::menu::Value;
use crate::menu_controls::{COLLECTION, CONTROLS, DOC_CONTROLS, Kind, MODS, MODS_PAGE, TABS};
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

/// The id of each card's open button, in `MODS` order: the control that opens the mod's settings, or an id
/// nothing handles for a mod with no settings page. A page of its own (`tab_<n>`) always opens; an element's
/// options (`opt_<n>`) only when the old card showed a settings button.
fn open_ids() -> &'static [String] {
    static IDS: OnceLock<Vec<String>> = OnceLock::new();
    IDS.get_or_init(|| {
        collection_items()
            .iter()
            .enumerate()
            .map(|(i, item)| {
                // The collection numbers controls within the Mods page's own document.
                let local = item["controls"]["options"].as_u64().unwrap_or(0) as usize;
                let id = DOC_CONTROLS[MODS_PAGE]
                    .get(local)
                    .and_then(|global| CONTROLS.get(usize::from(*global)))
                    .map_or("", |control| control.1);
                let gear = item["values"]["#item_gear"].as_bool() == Some(true);
                if id.starts_with("tab_") || (gear && !id.is_empty()) {
                    id.to_owned()
                } else {
                    format!("none_{}", MODS.get(i).map_or("", |m| m.0))
                }
            })
            .collect()
    })
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
            open: open.get(i).map_or("", String::as_str),
        })
        .collect()
}

/// The pages the sidebar lists, as `(tab, title)`, in groups: where the sidebar goes when a page is open.
const SIDEBAR: [(&str, &[(usize, &str)]); 2] = [
    ("Mods and options", &[(0, "Mods"), (7, "Config"), (26, "Sharing")]),
    ("Menu", &[(6, "General"), (8, "Credits")]),
];

/// The sidebar entries, each pressing its old tab button, with `active` lit. While an element's Style or
/// Layout page is open a last group switches between the two.
fn entries(active: usize) -> Vec<Vec<Entry<'static>>> {
    let mut groups: Vec<Vec<Entry<'static>>> = SIDEBAR
        .iter()
        .map(|(_, pages)| {
            pages
                .iter()
                .map(|&(tab, title)| Entry { id: TAB_IDS[tab], title, active: tab == active })
                .collect()
        })
        .collect();
    if matches!(active, 1 | 2) {
        groups.push(vec![
            Entry { id: TAB_IDS[2], title: "Style", active: active == 2 },
            Entry { id: TAB_IDS[1], title: "Layout", active: active == 1 },
        ]);
    }
    groups
}

/// The sidebar sections for `entries`: the fixed groups, then the element's own when it has one.
fn sections_of<'a>(entries: &'a [Vec<Entry<'static>>], element: &'a str) -> Vec<Section<'a>> {
    entries
        .iter()
        .enumerate()
        .map(|(i, list)| Section { heading: Some(SIDEBAR.get(i).map_or(element, |group| group.0)), entries: list })
        .collect()
}

/// `tab_<n>` for each page, as static text.
const TAB_IDS: [&str; 27] = [
    "tab_0", "tab_1", "tab_2", "tab_3", "tab_4", "tab_5", "tab_6", "tab_7", "tab_8", "tab_9", "tab_10", "tab_11", "tab_12",
    "tab_13", "tab_14", "tab_15", "tab_16", "tab_17", "tab_18", "tab_19", "tab_20", "tab_21", "tab_22", "tab_23", "tab_24",
    "tab_25", "tab_26",
];

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
    let entries = entries(MODS_PAGE);
    let sections = sections_of(&entries, "");
    let query = panel_state.search_text();
    let shell = Shell {
        palette: Palette::dark(),
        brand: "Vantage",
        sections: &sections,
        title: "Mods",
        search_hint: "Search mods...",
        search: query,
        search_id: "module_search",
        back_id: "back",
        account: "Player",
        account_note: "",
        backdrop: backdrop(settings),
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

/// The settings pages the redesign has rebuilt, by tab. The others keep their old design until they are.
const CONVERTED: [usize; 22] = [1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14, 15, 16, 19, 20, 21, 22, 23, 24, 25, 26];

/// Whether the redesign draws this tab (the Mods grid and the rebuilt settings pages).
#[must_use]
pub fn handles(tab: usize) -> bool {
    tab == MODS_PAGE || CONVERTED.contains(&tab)
}

/// The page for the open tab, when the redesign draws it.
#[must_use]
pub fn page(settings: &Settings, state: &PanelState) -> Option<String> {
    let tab = state.tab_number();
    if tab == MODS_PAGE {
        return Some(mods_panel(settings, state));
    }
    CONVERTED.contains(&tab).then(|| settings_panel(settings, state, tab))
}

/// The controls a page's baked document presses, in the order it first mentions them, as indices into `CONTROLS`.
fn referenced(document: usize) -> Vec<usize> {
    let mut text = String::new();
    crate::menu::push_document(&mut text, document);
    let mut found: Vec<usize> = Vec::new();
    for marker in ["mod.control:", "mod.axis:", "mod.drag_axis:", "mod.edit:"] {
        let mut from = 0;
        while let Some(at) = text[from..].find(marker) {
            let start = from + at + marker.len();
            let digits: String = text[start..].chars().take_while(char::is_ascii_digit).collect();
            from = start;
            if let Some(global) = digits.parse::<usize>().ok().and_then(|local| DOC_CONTROLS[document].get(local)) {
                found.push(usize::from(*global));
            }
        }
    }
    // First mention first: the document lists a page top to bottom.
    let mut order: Vec<usize> = Vec::new();
    for index in found {
        if !order.contains(&index) {
            order.push(index);
        }
    }
    order
}

/// Controls that are the menu's own furniture (tabs, setups, the HUD mover, popups), not options of a page.
fn is_chrome(id: &str) -> bool {
    const PREFIXES: [&str; 18] = [
        "tab_", "cfg_", "move", "reset_", "edit_", "ed_", "cf_", "dd_", "hex_", "cap_", "cp_", "toast_", "opt_", "lay_", "card_",
        "flt_", "fav_", "sort",
    ];
    PREFIXES.iter().any(|prefix| id.starts_with(prefix)) || id == "module_search"
}

/// The mod whose settings page is `tab`: its switch id, when a card's settings button opens that tab.
fn mod_of_tab(tab: usize) -> Option<&'static str> {
    let wanted = TAB_IDS.get(tab)?;
    open_ids().iter().position(|id| id == wanted).map(|i| MODS[i].0)
}

/// One option of a page: the control's id, its label and how it is edited, from the old tables and the settings.
struct Row<'a> {
    id: &'static str,
    label: &'static str,
    value: Value<'a>,
    changed: bool,
}

/// Whether two values are the same setting.
fn same(a: &Value<'_>, b: &Value<'_>) -> bool {
    match (a, b) {
        (Value::Toggle(a), Value::Toggle(b)) => a == b,
        (Value::Slider(a, ..), Value::Slider(b, ..)) => (a - b).abs() < 1.0e-9,
        (Value::Choice(a, _), Value::Choice(b, _)) => a == b,
        (Value::Key(a), Value::Key(b)) => a == b,
        _ => true,
    }
}

/// The rows of `tab`'s page: the mod's own switch first, when the page belongs to one mod, then the
/// options its old page showed.
fn rows<'a>(settings: &'a Settings, state: &'a PanelState, defaults: &'a Settings, tab: usize) -> Vec<Row<'a>> {
    let switch = mod_of_tab(tab).map(|id| Row {
        id,
        label: "Enabled",
        value: crate::panel::value(settings, state, id),
        changed: false,
    });
    switch
        .into_iter()
        .chain(rows_of(settings, state, defaults, tab))
        .collect()
}

/// The options a page's old document pressed.
fn rows_of<'a>(settings: &'a Settings, state: &'a PanelState, defaults: &'a Settings, tab: usize) -> impl Iterator<Item = Row<'a>> {
    referenced(crate::menu::document_index(tab, state.element().index()))
        .into_iter()
        .filter_map(|index| {
            let (kind, id, label) = CONTROLS[index];
            if is_chrome(id) || !matches!(kind, Kind::Toggle | Kind::Slider | Kind::Choice | Kind::Keybind | Kind::Button) {
                return None;
            }
            let value = crate::panel::value(settings, state, id);
            let changed = !same(&value, &crate::panel::value(defaults, state, id));
            Some(Row { id, label, value, changed })
        })
}

/// A redesigned settings page: the page's options as rows in the menu shell.
#[must_use]
pub fn settings_panel(settings: &Settings, state: &PanelState, tab: usize) -> String {
    let defaults = Settings::default();
    let rows = rows(settings, state, &defaults, tab);
    // A choice steps through its values on press, so its control is declared under its own id.
    let ids: Vec<String> = rows
        .iter()
        .map(|row| if matches!(row.value, Value::Choice(..)) { format!("cycle_{}", row.id) } else { row.id.to_owned() })
        .collect();
    let opts: Vec<Opt<'_>> = rows
        .iter()
        .zip(&ids)
        .map(|(row, id)| Opt {
            id,
            label: row.label,
            help: None,
            kind: match &row.value {
                Value::Toggle(on) => options::Kind::Toggle { on: *on },
                Value::Slider(value, min, max, step) => {
                    options::Kind::Slider { value: *value as f32, min: *min as f32, max: *max as f32, step: *step as f32 }
                }
                Value::Choice(selected, names) => options::Kind::Choice { options: names, selected: *selected },
                Value::Key(key) => options::Kind::Keybind { key },
                Value::Button(label) => options::Kind::Button { label: label.unwrap_or("Apply") },
                _ => options::Kind::Info,
            },
            changed: row.changed,
        })
        .collect();
    let element = state.element().label();
    let page = TABS.get(tab).copied().unwrap_or("Settings");
    let title = if matches!(tab, 1 | 2) { format!("{page}: {element}") } else { page.to_owned() };
    let group = [Group { title: &title, collapsed: false, options: &opts }];
    let matched = options::search(&group, state.option_query());
    let shown = options::groups_of(&matched);
    let entries = entries(tab);
    let sections = sections_of(&entries, element);
    let shell = Shell {
        palette: Palette::dark(),
        brand: "Vantage",
        sections: &sections,
        title: &title,
        search_hint: "Search options...",
        search: state.option_query(),
        search_id: "opt_search",
        back_id: "back",
        account: "Player",
        account_note: "",
        backdrop: backdrop(settings),
    };
    options::panel(&shell, &shown).to_string()
}

/// The blur and dimming of the world behind the menu.
fn backdrop(settings: &Settings) -> Backdrop {
    Backdrop {
        blur: settings.gameplay.menu_blur() as f32,
        brightness: settings.gameplay.menu_backdrop().0 as f32,
        saturation: settings.gameplay.menu_backdrop().1 as f32,
    }
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

    #[test]
    fn every_rebuilt_page_has_options_and_declares_only_valid_unique_ids() {
        for &tab in &CONVERTED {
            let mut settings = Settings::default();
            let mut state = PanelState::default();
            state.event(&mut settings, &format!("tab_{tab}"), 1.0);
            assert_eq!(state.tab_number(), tab);
            let page: serde_json::Value = serde_json::from_str(&page(&settings, &state).expect("a rebuilt page")).unwrap();
            let mut seen = std::collections::HashSet::new();
            let controls = page["controls"].as_array().unwrap();
            for control in controls {
                let id = control["id"].as_str().unwrap();
                assert!(
                    !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b)),
                    "tab {tab}: {id:?} is not a valid id"
                );
                assert!(seen.insert(id.to_owned()), "tab {tab}: {id} is declared twice");
            }
            // The sidebar's five pages, back, forward and search, then the page's own heading and rows.
            assert!(controls.len() > 5 + 3 + 1, "tab {tab} lists no options");
        }
    }

    #[test]
    fn a_choice_steps_to_its_next_value_and_reset_restores_the_default() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.event(&mut settings, "tab_16", 1.0);
        // Menu blur: change a slider, then reset it.
        let before = settings.gameplay.blur_amount;
        state.event(&mut settings, "gp_blur", before as f32 + 3.0);
        assert_ne!(settings.gameplay.blur_amount, before);
        state.event(&mut settings, "gp_blur_reset", 0.0);
        assert_eq!(settings.gameplay.blur_amount, before);
        // Time changer: its choice steps by its own cycle id.
        state.event(&mut settings, "tab_11", 1.0);
        let (at, count) = match crate::panel::value(&settings, &state, "gp_time") {
            Value::Choice(at, names) => (at, names.len()),
            _ => panic!("gp_time is a choice"),
        };
        state.event(&mut settings, "cycle_gp_time", 1.0);
        let Value::Choice(next, _) = crate::panel::value(&settings, &state, "gp_time") else { panic!() };
        assert_eq!(next, (at + 1) % count);
    }

    #[test]
    fn page_furniture_is_never_listed_as_an_option() {
        let settings = Settings::default();
        let state = PanelState::default();
        let defaults = Settings::default();
        for &tab in &CONVERTED {
            for row in rows(&settings, &state, &defaults, tab) {
                assert!(!is_chrome(row.id), "{} is furniture", row.id);
            }
        }
    }
}
