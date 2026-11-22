//! The Vantage Mods page: category chips, the sort button and the card grid on the menu shell.

use ui::mod_panel::Panel;
use vantage_ui::{
    mods::{self, Card, Filter, Ids, Sort, State},
    shell::{Backdrop, Entry, Section, Shell},
    tokens::Palette,
};

use super::Fixture;

const NAMES: [(&str, &str, &str, bool, bool); 14] = [
    ("keystrokes", "Keystrokes", "HUD", true, true),
    ("cps", "CPS", "HUD", true, false),
    ("fps", "FPS", "HUD", false, false),
    ("armor", "Armor status", "HUD", true, false),
    ("coords", "Coordinates", "HUD", false, true),
    ("zoom", "Zoom", "Mechanic", true, false),
    ("sprint", "Toggle sprint", "Mechanic", false, false),
    ("crosshair", "Crosshair", "Visual", true, false),
    ("fullbright", "Fullbright", "Visual", false, false),
    ("time", "Time changer", "Visual", false, true),
    ("weather", "Weather changer", "Visual", false, false),
    ("hitbox", "Hitboxes", "Visual", false, false),
    ("blur", "Menu blur", "Visual", true, false),
    ("hive", "Hive requeue", "Mechanic", false, false),
];

/// The page with `filter` and `query` applied.
pub(super) fn fixture(filter: Filter<'static>, query: &'static str) -> Fixture {
    const MODS: [Entry<'static>; 3] = [
        Entry { id: "mods", title: "Mods", active: true },
        Entry { id: "profiles", title: "Profiles", active: false },
        Entry { id: "keybinds", title: "Keybinds", active: false },
    ];
    const LOOK: [Entry<'static>; 2] = [
        Entry { id: "themes", title: "Themes", active: false },
        Entry { id: "preferences", title: "Preferences", active: false },
    ];
    const SECTIONS: [Section<'static>; 2] = [
        Section { heading: Some("Mods and options"), entries: &MODS },
        Section { heading: Some("Personalization"), entries: &LOOK },
    ];
    let cards: Vec<Card<'static>> = NAMES
        .iter()
        .map(|&(id, name, category, on, favorite)| Card { id, name, category, on, favorite, hidden: false, open: "" })
        .collect();
    let shell = Shell {
        palette: Palette::dark(),
        brand: "Vantage",
        sections: &SECTIONS,
        title: "Mods",
        search_hint: "Search mods...",
        search: query,
        search_id: "search",
        back_id: "back",
        account: "Player",
        account_note: "Online",
        backdrop: Backdrop::NONE,
    };
    let state = State { filter, sort: Sort::Default, query };
    let panel = mods::panel(&shell, &cards, &["HUD", "Visual", "Mechanic"], &state, &Ids::plain());
    let panel: Panel = serde_json::from_value(panel).expect("the page is a valid panel");
    Fixture::ModPanel(Box::new(panel), None, None)
}
