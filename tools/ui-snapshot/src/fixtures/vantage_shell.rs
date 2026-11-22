//! The Vantage menu shell with a sample sidebar: the frame every menu page is placed in.

use ui::mod_panel::Panel;
use vantage_ui::{
    shell::{self, Backdrop, Entry, Section, Shell},
    tokens::Palette,
};

use super::Fixture;

/// The shell on the dark theme, with the first page active.
pub(super) fn fixture() -> Fixture {
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
    let panel = shell::panel(&Shell {
        palette: Palette::dark(),
        brand: "Vantage",
        sections: &SECTIONS,
        title: "Mods",
        search_hint: "Search mods...",
        search: "",
        search_id: "search",
        account: "Player",
        account_note: "Online",
        backdrop: Backdrop::NONE,
    });
    let panel: Panel = serde_json::from_value(panel).expect("the shell is a valid panel");
    Fixture::ModPanel(Box::new(panel), None, None)
}
