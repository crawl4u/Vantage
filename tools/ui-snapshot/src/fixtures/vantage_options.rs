//! A Vantage settings page: grouped option rows of every kind in the menu shell.

use ui::mod_panel::Panel;
use vantage_ui::{
    options::{self, Group, Kind, Opt},
    shell::{Backdrop, Entry, Section, Shell},
    tokens::Palette,
};

use super::Fixture;

const ALIGN: [&str; 3] = ["Left", "Center", "Right"];

/// The Keystrokes page with its groups and a search applied.
pub(super) fn fixture(collapsed: bool, query: &'static str) -> Fixture {
    const ENTRIES: [Entry<'static>; 2] = [
        Entry { id: "mods", title: "Mods", active: false },
        Entry { id: "keystrokes", title: "Keystrokes", active: true },
    ];
    const SECTIONS: [Section<'static>; 1] =
        [Section { heading: Some("Mods and options"), entries: &ENTRIES }];
    let display = [
        Opt { id: "show", label: "Show on screen", help: Some("Draws the pressed keys in the corner."), kind: Kind::Toggle { on: true }, changed: false },
        Opt { id: "scale", label: "Size", help: None, kind: Kind::Slider { value: 1.25, min: 0.5, max: 3.0, step: 0.01 }, changed: true },
        Opt { id: "opacity", label: "Background opacity", help: None, kind: Kind::Slider { value: 0.4, min: 0.0, max: 1.0, step: 0.01 }, changed: false },
        Opt { id: "align", label: "Alignment", help: None, kind: Kind::Choice { options: &ALIGN, selected: 1 }, changed: false },
    ];
    let behaviour = [
        Opt { id: "mouse", label: "Show mouse buttons", help: None, kind: Kind::Toggle { on: false }, changed: false },
        Opt { id: "space", label: "Show the space bar", help: Some("A wide key under the movement keys."), kind: Kind::Toggle { on: true }, changed: false },
        Opt { id: "toggle_key", label: "Toggle key", help: None, kind: Kind::Keybind { key: "KeyK" }, changed: true },
        Opt { id: "title", label: "Title", help: None, kind: Kind::Text { value: "Keys" }, changed: false },
        Opt { id: "reset", label: "Defaults", help: None, kind: Kind::Button { label: "Restore" }, changed: false },
        Opt { id: "note", label: "Keys follow your own bindings.", help: None, kind: Kind::Info, changed: false },
    ];
    let groups = [
        Group { title: "Display", collapsed, options: &display },
        Group { title: "Behavior", collapsed: false, options: &behaviour },
    ];
    let matched = options::search(&groups, query);
    let shown = options::groups_of(&matched);
    let shell = Shell {
        palette: Palette::dark(),
        brand: "Vantage",
        sections: &SECTIONS,
        title: "Keystrokes",
        search_hint: "Search options...",
        search: query,
        search_id: "search",
        back_id: "back",
        account: "Player",
        account_note: "Online",
        backdrop: Backdrop::NONE,
    };
    let panel = options::panel(&shell, &shown, None);
    let panel: Panel = serde_json::from_value(panel).expect("the page is a valid panel");
    Fixture::ModPanel(Box::new(panel), None, None)
}
