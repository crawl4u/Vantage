//! The redesigned menu, built from the mod's own tables and settings (see `vantage-ui`).
//!
//! The pages are pure functions of the settings: they return the panel description the host draws.
//! Their controls are declared with the ids the old menu's handlers already know
//! (`PanelState::event`, `PanelState::field`), so presses and typed text reach the same settings.

use std::sync::OnceLock;

use vantage_ui::{
    icons,
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

/// Each card's icon, in `MODS` order, from the generated collection.
fn card_icons() -> Vec<&'static str> {
    collection_items()
        .iter()
        .map(|item| item["values"]["#item_icon"].as_str().unwrap_or(""))
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
    let icon_names = card_icons();
    let open = open_ids();
    MODS.iter()
        .enumerate()
        .map(|(i, &(id, lower, category))| Card {
            id,
            name: names.get(i).copied().filter(|name| !name.is_empty()).unwrap_or(lower),
            category: CATEGORIES[usize::from(category).min(CATEGORIES.len() - 1)],
            on: matches!(crate::panel::value(settings, state, id), Value::Toggle(true)),
            favorite: settings.favorites.iter().any(|f| f == id),
            hidden: settings.hidden_mods.iter().any(|h| h == id),
            icon: icon_names.get(i).copied().unwrap_or(""),
            open: open.get(i).map_or("", String::as_str),
        })
        .collect()
}

/// The pages the sidebar lists, as `(tab, title)`, in groups: where the sidebar goes when a page is open.
const SIDEBAR: [(&str, &[(usize, &str)]); 2] = [
    ("Mods and options", &[(0, "Mods"), (17, "Waypoints"), (7, "Profiles"), (26, "Sharing")]),
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
                .map(|&(tab, title)| Entry { id: TAB_IDS[tab], title, icon: icon_of(tab), active: tab == active })
                .collect()
        })
        .collect();
    if matches!(active, 1 | 2) {
        groups.push(vec![
            Entry { id: TAB_IDS[2], title: "Style", icon: icons::PALETTE, active: active == 2 },
            Entry { id: TAB_IDS[1], title: "Layout", icon: icons::GRID_3X3, active: active == 1 },
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

/// A page's sidebar icon.
fn icon_of(tab: usize) -> &'static str {
    match tab {
        0 => icons::LAYERS,
        6 => icons::SETTINGS,
        7 => icons::PACKAGE,
        8 => icons::HEART,
        17 => icons::MAP_PIN,
        26 => icons::CLOUD,
        _ => "",
    }
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
        6 => Filter::Hidden,
        _ => Filter::All,
    }
}

/// The chip number a filter is pressed as: the old menu's `flt_<n>` ids.
fn chip_number(filter: Filter<'_>) -> usize {
    match filter {
        Filter::Enabled => 1,
        Filter::Category(name) => CATEGORIES.iter().position(|c| *c == name).map_or(0, |i| i + 2),
        Filter::Favorites => 5,
        Filter::Hidden => 6,
        Filter::All => 0,
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

/// The sidebar for the open page: each group's heading and entries as `(button id, title, icon, active)`.
#[must_use]
pub fn sidebar(active: usize) -> Vec<(&'static str, Vec<(&'static str, &'static str, &'static str, bool)>)> {
    entries(active)
        .into_iter()
        .enumerate()
        .map(|(i, list)| (SIDEBAR.get(i).map_or("", |group| group.0), list.into_iter().map(|e| (e.id, e.title, e.icon, e.active)).collect()))
        .collect()
}

/// What the Mods grid shows now: every card, the indices of those shown in order, the filter chips as
/// `(label, number)` with the active one's number, and the sort in use.
#[must_use]
pub fn grid<'a>(settings: &'a Settings, panel_state: &'a PanelState) -> (Vec<Card<'a>>, Vec<usize>, Vec<(&'static str, usize)>, usize, usize) {
    let cards = cards(settings, panel_state);
    let custom = crate::panel::custom_ranks(settings);
    let filter: Filter<'static> = filter_of(panel_state.grid_filter());
    let state = State { filter, sort: sort_of(panel_state.grid_sort()), query: panel_state.search_text(), custom: &custom };
    let order = mods::visible(&cards, &state);
    let chips: Vec<(&'static str, usize)> = mods::chips(&cards, &CATEGORIES, filter).iter().map(|c| (c.label, chip_number(c.filter))).collect();
    (cards, order, chips, panel_state.grid_filter(), panel_state.grid_sort())
}

/// Whether the Mods page is drawn by the Vantage GUI toolkit (`gui.rs`) rather than as a JSON document.
pub const GUI_MENU: bool = true;

/// Whether `tab` is drawn by the toolkit right now: the Mods page, and the settings pages that have no text
/// fields, while no colour picker is open (those two still use the JSON menu).
#[must_use]
pub fn uses_scene(settings: &Settings, state: &PanelState, tab: usize) -> bool {
    if !GUI_MENU || !settings.new_menu {
        return false;
    }
    if tab == MODS_PAGE {
        return true;
    }
    (CONVERTED.contains(&tab) || tab == WAYPOINTS_PAGE || tab == CREDITS_PAGE) && state.picker_view().is_none()
}

/// The panel the toolkit's pages draw in: one node the host paints the published scene into, sized to fit the
/// screen like the JSON pages are.
#[must_use]
pub fn scene_panel(settings: &Settings, state: &PanelState, tab: usize) -> String {
    let (search_id, search_value) = if tab == MODS_PAGE { ("module_search", state.search_text()) } else { ("opt_search", state.option_query()) };
    // The search pill is a transparent button over the painted one that opens the host's text editor.
    let [ww, wh] = vantage_ui::shell::window_px();
    let pill = crate::gui::search_rect();
    let offset = [pill.x + pill.w / 2.0 - ww / 2.0, pill.y + pill.h / 2.0 - wh / 2.0];
    serde_json::json!({
        "reference_size": vantage_ui::shell::reference_size(),
        "fill": true,
        "controls": [{"kind": "text", "id": search_id, "label": "Search", "value": search_value, "max_chars": 48, "live": true}],
        "surface": {
            "screen": "vantage.scene",
            "document": serde_json::json!({
                "namespace": "vantage",
                "scene": {"type": "panel", "size": ["100%", "100%"], "controls": [
                    {"draw": {"type": "custom", "renderer": "vantage_scene", "size": ["100%", "100%"]}},
                    {"search": {"type": "button", "size": [pill.w, pill.h], "offset": offset, "anchor_from": "center", "anchor_to": "center",
                        "controls": [],
                        "button_mappings": [{"from_button_id": "button.menu_select", "to_button_id": "mod.edit:0", "mapping_type": "pressed"}]}}
                ]},
            }).to_string(),
            "bindings": backdrop(settings).bindings(),
            "transitions": vantage_ui::shell::Backdrop::transitions(),
        },
    })
    .to_string()
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
        palette: palette(settings),
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
    let custom = crate::panel::custom_ranks(settings);
    let state = State {
        filter: filter_of(panel_state.grid_filter()),
        sort: sort_of(panel_state.grid_sort()),
        query,
        custom: &custom,
    };
    let chip = |filter: Filter<'_>| format!("flt_{}", chip_number(filter));
    let toggle = |id: &str| id.to_owned();
    let favorite = |id: &str| format!("fav_{id}");
    let hide = |id: &str| format!("hide_{id}");
    let reorder = |id: &str| format!("move_{id}");
    let ids = Ids { sort: "sort", search: "module_search", chip: &chip, toggle: &toggle, favorite: &favorite, hide: &hide, reorder: &reorder };
    // A search no mod answers looks through the options of every page instead.
    if !query.trim().is_empty() && mods::visible(&cards, &state).is_empty() {
        let hits = option_hits(settings, panel_state, query);
        if !hits.is_empty() {
            return option_results_panel(&shell, &hits);
        }
    }
    mods::panel(&shell, &cards, &CATEGORIES, &state, &ids).to_string()
}

/// The pages whose options a search looks through: those not about one chosen element or one long list.
fn searchable(tab: usize) -> bool {
    CONVERTED.contains(&tab) && !matches!(tab, 1 | 2 | 7 | 26)
}

/// A page with options matching `query`, with the names of the first few that do.
struct PageHit {
    tab: usize,
    matches: Vec<&'static str>,
}

/// The pages with an option whose name has every word of `query`, in page order.
fn option_hits(settings: &Settings, state: &PanelState, query: &str) -> Vec<PageHit> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let defaults = Settings::default();
    let mut hits = Vec::new();
    for tab in (0..TABS.len()).filter(|tab| searchable(*tab)) {
        let matches: Vec<&'static str> = rows_of(settings, state, &defaults, tab)
            .filter(|row| {
                let name = row.label.to_lowercase();
                words.iter().all(|word| name.contains(word.as_str()))
            })
            .map(|row| row.label)
            .collect();
        if !matches.is_empty() {
            hits.push(PageHit { tab, matches });
        }
    }
    hits
}

/// How many matching option names a result lists before it says there are more.
const RESULT_NAMES: usize = 3;

/// The results of a search that found options but no mod: one row per page, opening that page.
fn option_results_panel(shell: &Shell<'_>, hits: &[PageHit]) -> String {
    let ids: Vec<String> = hits.iter().map(|hit| TAB_IDS[hit.tab].to_owned()).collect();
    let notes: Vec<String> = hits
        .iter()
        .map(|hit| {
            let mut names = hit.matches.iter().take(RESULT_NAMES).copied().collect::<Vec<_>>().join(", ");
            if hit.matches.len() > RESULT_NAMES {
                names.push_str(", ...");
            }
            names
        })
        .collect();
    let opts: Vec<Opt<'_>> = hits
        .iter()
        .zip(&ids)
        .zip(&notes)
        .map(|((hit, id), note)| Opt {
            id,
            label: TABS.get(hit.tab).copied().unwrap_or("Settings"),
            help: Some(note),
            kind: options::Kind::Button { label: "Open" },
            changed: false,
        })
        .collect();
    let group = [Group { title: "Options", collapsed: false, options: &opts }];
    let matched = options::search(&group, "");
    let shown = options::groups_of(&matched);
    let shell = Shell { title: "Search results", ..*shell };
    options::panel(&shell, &shown, None, None, None).to_string()
}

/// The Waypoints page.
const WAYPOINTS_PAGE: usize = 17;

/// The Waypoints page: the view buttons, each waypoint's switch, Edit and Delete, and the editor's fields.
/// Builds the Waypoints page and hands it to `f`.
fn waypoints_view<'a, R>(settings: &'a Settings, state: &'a PanelState, f: &mut dyn FnMut(&PageView<'_>) -> R) -> R {
    use crate::menu::Value as V;
    let wp = &state.wp;
    // Before the page has been synced with the world its texts are empty, which a control label may not be.
    let or = |text: &'a str, fallback: &'static str| if text.is_empty() { fallback } else { text };
    let value = |id: &str| crate::panel::value(settings, state, id);
    // The ids of the waypoint rows' controls, kept so each Opt can borrow its own.
    let ids: Vec<[String; 3]> = (0..wp.rows.len()).map(|n| [format!("wp_vis_{n}"), format!("wp_edit_{n}"), format!("wp_del_{n}")]).collect();
    let labels: Vec<[String; 2]> = wp.rows.iter().map(|row| [format!("Edit {}", row.name), format!("Delete {}", row.name)]).collect();
    let mut list: Vec<Opt<'_>> = Vec::new();
    for (n, row) in wp.rows.iter().enumerate() {
        list.push(Opt { id: &ids[n][0], label: &row.name, help: Some(&row.pos), kind: options::Kind::Toggle { on: row.visible }, changed: false });
        list.push(Opt { id: &ids[n][1], label: &labels[n][0], help: None, kind: options::Kind::Button { label: "Edit" }, changed: false });
        list.push(Opt { id: &ids[n][2], label: &labels[n][1], help: None, kind: options::Kind::Button { label: "Delete" }, changed: false });
    }
    if list.is_empty() {
        list.push(Opt { id: "wp_empty", label: or(&wp.info, "No waypoints here yet."), help: None, kind: options::Kind::Info, changed: false });
    }
    let button = |id: &'static str, label: &'static str, shown: &'a str| Opt { id, label, help: None, kind: options::Kind::Button { label: shown }, changed: false };
    let view = [
        button("wp_fdim", "Show", or(&wp.dim_label, "Show: Here")),
        button("wp_fsort", "Sort", or(&wp.sort_label, "Sort: Nearest")),
        button("wp_fgroup", "Group", or(&wp.group_label, "Group: All")),
    ];
    let text = |id: &'static str, label: &'static str, shown: &'a str| Opt { id, label, help: None, kind: options::Kind::Text { value: shown }, changed: false };
    let mut editor = vec![
        text("wp_name", "Name", &wp.name),
        text("wp_x", "X", &wp.x),
        text("wp_y", "Y", &wp.y),
        text("wp_z", "Z", &wp.z),
        text("wp_group", "Group", &wp.group),
    ];
    if let V::Choice(selected, names) = value("gp_wp_edit_color") {
        editor.push(Opt { id: "choose_gp_wp_edit_color", label: "Colour", help: None, kind: options::Kind::Choice { options: names, selected }, changed: false });
    }
    let status = if wp.error.is_empty() { wp.info.as_str() } else { wp.error.as_str() };
    editor.push(Opt { id: "wp_status", label: "Status", help: Some(status), kind: options::Kind::Info, changed: false });
    editor.push(button("wp_save", "Save", "Save"));
    editor.push(button("wp_cancel", "Cancel", "Cancel"));
    editor.push(button("wp_undo", "Undo", or(&wp.undo_label, "Nothing to undo")));
    let confirm = Opt { id: "gp_wp_confirm", label: "Confirm waypoint deletion", help: None, kind: options::Kind::Toggle { on: matches!(value("gp_wp_confirm"), V::Toggle(true)) }, changed: false };
    editor.push(confirm);
    let groups = [
        Group { title: "View", collapsed: false, options: &view },
        Group { title: "Your waypoints", collapsed: false, options: &list },
        Group { title: or(&wp.mode, "Add waypoint"), collapsed: false, options: &editor },
    ];
    let matched = options::search(&groups, state.option_query());
    let shown = options::groups_of(&matched);
    let open = state.open_choice().and_then(|id| match crate::panel::value(settings, state, id) {
        V::Choice(selected, names) => Some(options::Popup { title: "Colour", options: names, selected: selected.min(names.len().saturating_sub(1)) }),
        _ => None,
    });
    f(&PageView { title: "Waypoints", groups: &shown, popup: open.as_ref(), picker: None, element: "", has_text: true })
}

/// The Waypoints page as a JSON document.
fn waypoints_panel<'a>(settings: &'a Settings, state: &'a PanelState) -> String {
    waypoints_view(settings, state, &mut |page| {
        let entries = entries(WAYPOINTS_PAGE);
        let sections = sections_of(&entries, "");
        let shell = Shell {
            palette: palette(settings),
            brand: "Vantage",
            sections: &sections,
            title: page.title,
            search_hint: "Search waypoints...",
            search: state.option_query(),
            search_id: "opt_search",
            back_id: "back",
            account: "Player",
            account_note: "",
            backdrop: backdrop(settings),
        };
        options::panel(&shell, page.groups, page.popup, None, None).to_string()
    })
}

/// The Credits page.
const CREDITS_PAGE: usize = 8;

/// The credits, as sections of lines. The licence notices and attributions are kept as the project states them.
/// The longest line a credit row holds before the row clips it.
const CREDIT_LINE: usize = 44;
const CREDITS: [(&str, &[&str]); 5] = [
    ("Vantage", &["Made by Crawl.", "HUD elements and world tools, each placed", "and styled on its own."]),
    (
        "Based on",
        &[
            "Technic Tweaks and Keystrokes by Emil Chill",
            "(MIT licence), and Zoomify by isXander,",
            "ported by Emil Chill (LGPL-3.0-or-later).",
            "Many thanks to Emil Chill for the original",
            "mods and menu kit.",
        ],
    ),
    ("Built with", &["Claude AI, with Emil Chill's original work,", "for Cinnabar."]),
    ("Inspired by", &["Lunar Client's HUD look and Crosshair.", "Vantage is not affiliated with Lunar Client."]),
    ("Platform", &["Cinnabar, with Mods Core for its mod list", "and menu hand-over."]),
];

/// Builds the Credits page and hands it to `f`.
fn credits_view<R>(state: &PanelState, f: &mut dyn FnMut(&PageView<'_>) -> R) -> R {
    let opts: Vec<Vec<Opt<'_>>> = CREDITS
        .iter()
        .map(|(_, lines)| lines.iter().map(|line| Opt { id: "credit", label: line, help: None, kind: options::Kind::Info, changed: false }).collect())
        .collect();
    let groups: Vec<Group<'_>> = CREDITS.iter().zip(&opts).map(|((title, _), opts)| Group { title, collapsed: false, options: opts }).collect();
    let matched = options::search(&groups, state.option_query());
    let shown = options::groups_of(&matched);
    f(&PageView { title: "Credits", groups: &shown, popup: None, picker: None, element: "", has_text: false })
}

/// The Credits page as a JSON document.
fn credits_panel(settings: &Settings, state: &PanelState) -> String {
    credits_view(state, &mut |page| {
        let entries = entries(CREDITS_PAGE);
        let sections = sections_of(&entries, "");
        let shell = Shell {
            palette: palette(settings),
            brand: "Vantage",
            sections: &sections,
            title: page.title,
            search_hint: "Search credits...",
            search: state.option_query(),
            search_id: "opt_search",
            back_id: "back",
            account: "Player",
            account_note: "",
            backdrop: backdrop(settings),
        };
        options::panel(&shell, page.groups, None, None, None).to_string()
    })
}

/// The settings pages the redesign has rebuilt, by tab. The others keep their old design until they are.
const CONVERTED: [usize; 24] = [1, 2, 7, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14, 15, 16, 18, 19, 20, 21, 22, 23, 24, 25, 26];

/// Whether the redesign draws this tab (the Mods grid and the rebuilt settings pages).
#[must_use]
pub fn handles(tab: usize) -> bool {
    tab == MODS_PAGE || tab == CREDITS_PAGE || tab == WAYPOINTS_PAGE || CONVERTED.contains(&tab)
}

/// The page for the open tab, when the redesign draws it.
#[must_use]
pub fn page(settings: &Settings, state: &PanelState) -> Option<String> {
    let tab = state.tab_number();
    if uses_scene(settings, state, tab) {
        return Some(scene_panel(settings, state, tab));
    }
    page_json(settings, state)
}

/// The page for the open tab as a JSON document, whether or not the toolkit draws it: the pages the toolkit
/// does not draw use it, and it is what the offline previews and checks of the documents build.
#[must_use]
pub fn page_json(settings: &Settings, state: &PanelState) -> Option<String> {
    let tab = state.tab_number();
    if tab == MODS_PAGE {
        return Some(mods_panel(settings, state));
    }
    if tab == CREDITS_PAGE {
        return Some(credits_panel(settings, state));
    }
    if tab == WAYPOINTS_PAGE {
        return Some(waypoints_panel(settings, state));
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

/// Whether `id` is the menu's own furniture on page `tab`, so it is not listed as an option there. The
/// Profiles page (the old Config page) is made of the `cfg_` controls, which are its options.
fn is_furniture(tab: usize, id: &str) -> bool {
    is_chrome(id) && !(tab == 7 && id.starts_with("cfg_")) && !(tab == LAYOUT_PAGE && id == "move")
}

/// Controls that are the menu's own furniture (tabs, setups, the HUD mover, popups), not options of a page.
fn is_chrome(id: &str) -> bool {
    const PREFIXES: [&str; 21] = [
        "tab_", "cfg_", "move", "reset_", "edit_", "ed_", "cf_", "dd_", "hex_", "cap_", "cp_", "toast_", "opt_", "lay_", "card_",
        "flt_", "fav_", "hide_", "move_", "pk_", "sort",
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
    /// The section the row sits in when the page's own table does not say (the General page's keybinds).
    section: Option<&'static str>,
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
        section: None,
    });
    switch
        .into_iter()
        .chain(rows_of(settings, state, defaults, tab))
        .chain((tab == KEYBINDS_PAGE).then(|| appearance_row(settings, state, defaults)))
        .chain((tab == KEYBINDS_PAGE).then(|| keybind_rows(settings, state, defaults)).into_iter().flatten())
        .collect()
}

/// The Layout page.
const LAYOUT_PAGE: usize = 1;

/// The General page's theme switch.
fn appearance_row<'a>(settings: &'a Settings, state: &'a PanelState, defaults: &'a Settings) -> Row<'a> {
    let id = "menu_light";
    let value = crate::panel::value(settings, state, id);
    let changed = !same(&value, &crate::panel::value(defaults, state, id));
    Row { id, label: "Light theme", value, changed, section: Some(APPEARANCE) }
}

/// The section of the General page that holds it.
const APPEARANCE: &str = "Appearance";

/// The menu's colours: the light palette when the setting asks for it.
fn palette(settings: &Settings) -> Palette {
    if settings.menu_light { Palette::light() } else { Palette::dark() }
}

/// The page that also lists every keybind of every mod, in a section of its own.
const KEYBINDS_PAGE: usize = 6;

/// The name of the page that lists keybind `id` among its options, or a catch-all.
fn keybind_owner(settings: &Settings, state: &PanelState, defaults: &Settings, id: &str) -> &'static str {
    CONVERTED
        .iter()
        .copied()
        .filter(|tab| *tab != KEYBINDS_PAGE)
        .find(|tab| rows_of(settings, state, defaults, *tab).any(|row| row.id == id))
        .and_then(|tab| TABS.get(tab).copied())
        .unwrap_or(OTHER_KEYS)
}

/// The section of keybinds no rebuilt page lists; it is split in several, each opened on its own, so the
/// page stays within the host's size limit.
const OTHER_KEYS: &str = "Other keys";
const OTHER_KEY_SECTIONS: [&str; 10] = [
    "Other keys 1", "Other keys 2", "Other keys 3", "Other keys 4", "Other keys 5", "Other keys 6", "Other keys 7", "Other keys 8", "Other keys 9", "Other keys 10",
];
/// How many keybinds one of those sections holds.
const OTHER_KEYS_PER_SECTION: usize = 10;

/// Every keybind the menu has that the page does not already list: one row each, grouped by the page that owns it.
fn keybind_rows<'a>(settings: &'a Settings, state: &'a PanelState, defaults: &'a Settings) -> Vec<Row<'a>> {
    let listed: Vec<&str> = rows_of(settings, state, defaults, KEYBINDS_PAGE).map(|row| row.id).collect();
    let mut rows: Vec<Row<'a>> = CONTROLS
        .iter()
        .filter(|(kind, id, _)| *kind == Kind::Keybind && !listed.contains(id) && !is_furniture(KEYBINDS_PAGE, id))
        .map(|&(_, id, label)| {
            let value = crate::panel::value(settings, state, id);
            let changed = !same(&value, &crate::panel::value(defaults, state, id));
            Row { id, label, value, changed, section: Some(keybind_owner(settings, state, defaults, id)) }
        })
        .collect();
    // Rows of one owner sit together, owners in page order and the catch-all last.
    let order = |row: &Row<'_>| match row.section {
        Some(name) if name != OTHER_KEYS => TABS.iter().position(|t| *t == name).unwrap_or(usize::MAX - 1),
        _ => usize::MAX,
    };
    rows.sort_by_key(order);
    for (others, row) in rows.iter_mut().filter(|row| row.section == Some(OTHER_KEYS)).enumerate() {
        let part = (others / OTHER_KEYS_PER_SECTION).min(OTHER_KEY_SECTIONS.len() - 1);
        row.section = Some(OTHER_KEY_SECTIONS[part]);
    }
    rows
}

/// For each row, the other keybinds that use the same key, as a line of help (none when it is alone on its key).
fn conflict_notes(rows: &[Row<'_>]) -> Vec<Option<String>> {
    fn key<'r>(row: &'r Row<'_>) -> Option<&'r str> {
        match row.value {
            Value::Key(key) if !key.is_empty() && key != "None" => Some(key),
            _ => None,
        }
    }
    rows.iter()
        .map(|row| {
            let mine = key(row)?;
            let others: Vec<&str> = rows.iter().filter(|other| other.id != row.id && key(other) == Some(mine)).map(|other| other.label).collect();
            (!others.is_empty()).then(|| format!("Also used by {}", others.join(", ")))
        })
        .collect()
}

/// Whether the switch `row` depends on is off, so the row is left out.
fn switched_off(settings: &Settings, state: &PanelState, id: &str) -> bool {
    DEPENDS
        .iter()
        .find(|(option, _)| *option == id)
        .is_some_and(|(_, switch)| matches!(crate::panel::value(settings, state, switch), Value::Toggle(false)))
}

/// Options that matter only while a switch above them is on: `(option, its switch)`. The page leaves them
/// out while the switch is off, as the reference menu does.
const DEPENDS: [(&str, &str); 17] = [
    ("chroma_speed", "chroma_on"),
    ("chroma_type", "chroma_on"),
    ("border_width", "box_border"),
    ("border_opacity", "box_border"),
    ("border_color", "box_border"),
    ("zoom_steps", "zoom_steps_on"),
    ("zoom_per_step", "zoom_steps_on"),
    ("ch_dot_round", "ch_dot"),
    ("ch_target_scale", "ch_target"),
    ("ch_target_opacity", "ch_target"),
    ("ch_player_color", "ch_players"),
    ("ch_mob_color", "ch_mobs"),
    ("ch_weapon_shape", "ch_weapon"),
    ("ch_weapon_size", "ch_weapon"),
    ("ch_outline_color", "ch_outline"),
    ("ch_outline_width", "ch_outline"),
    ("ch_outline_round", "ch_outline"),
];

/// Titled sections of the busier pages: `(tab, [(the option a section starts at, its title)])`. A page not
/// listed is one section.
const SECTIONS: [(usize, &[(&str, &str)]); 5] = [
    (7, &[("cfg_slot_0", "Profiles"), ("cfg_name", "Selected profile"), ("cfg_export", "Share")]),
    (1, &[("pos_x", "Position"), ("scale", "Size"), ("armor_horizontal", "Armor"), ("show_chunk", "Chunk info")]),
    (2, &[("bg_color", "Box"), ("text_color", "Text"), ("labels", "Display"), ("chroma_on", "Chroma"), ("background", "Extras"), ("box_border", "Border")]),
    (4, &[("zoom_initial", "Amount"), ("zoom_step_in", "Scroll steps"), ("zoom_smooth", "Smoothing"), ("zoom_sens", "Controls"), ("zoom_p_default", "Presets")]),
    (5, &[("ch_shape", "Shape"), ("ch_dot", "Dot"), ("ch_target", "Targets"), ("ch_weapon", "Weapon"), ("ch_color", "Colour"), ("ch_outline", "Outline")]),
];

/// The section title of every row of `rows`, in order: the last section start at or before it.
fn section_titles<'a>(tab: usize, rows: &[Row<'_>]) -> Vec<&'a str> {
    let starts: &[(&str, &str)] = SECTIONS.iter().find(|s| s.0 == tab).map_or(&[], |s| s.1);
    let mut current = "";
    rows.iter()
        .map(|row| {
            if let Some((_, title)) = starts.iter().find(|(id, _)| *id == row.id) {
                current = title;
            }
            if let Some(section) = row.section {
                return section;
            }
            current
        })
        .collect()
}

/// The options a page's old document pressed.
fn rows_of<'a>(settings: &'a Settings, state: &'a PanelState, defaults: &'a Settings, tab: usize) -> impl Iterator<Item = Row<'a>> {
    referenced(crate::menu::document_index(tab, state.element().index()))
        .into_iter()
        .filter_map(move |index| {
            let (kind, id, label) = CONTROLS[index];
            if is_furniture(tab, id) || !matches!(kind, Kind::Toggle | Kind::Slider | Kind::Choice | Kind::Keybind | Kind::Button | Kind::Text) {
                return None;
            }
            let value = crate::panel::value(settings, state, id);
            let changed = !same(&value, &crate::panel::value(defaults, state, id));
            Some(Row { id, label, value, changed, section: None })
        })
}

/// What a settings page shows: its title, its groups of options, the open choice's list, the open colour
/// picker and the element being edited.
pub struct PageView<'a> {
    pub title: &'a str,
    pub groups: &'a [Group<'a>],
    pub popup: Option<&'a options::Popup<'a>>,
    pub picker: Option<&'a vantage_ui::picker::Picker<'a>>,
    pub element: &'a str,
    /// Whether any option is a text field, which only the JSON menu can edit.
    pub has_text: bool,
}

/// Builds the page for `tab` and hands it to `f`.
pub fn with_page<R>(settings: &Settings, state: &PanelState, tab: usize, f: &mut dyn FnMut(&PageView<'_>) -> R) -> R {
    if tab == WAYPOINTS_PAGE {
        return waypoints_view(settings, state, f);
    }
    if tab == CREDITS_PAGE {
        return credits_view(state, f);
    }
    let defaults = Settings::default();
    let all = rows(settings, state, &defaults, tab);
    let titles = section_titles(tab, &all);
    let notes = conflict_notes(&all);
    let (mut rows, mut kept_titles, mut kept_notes) = (Vec::new(), Vec::new(), Vec::new());
    for ((row, title), note) in all.into_iter().zip(titles).zip(notes) {
        if !switched_off(settings, state, row.id) {
            rows.push(row);
            kept_titles.push(title);
            kept_notes.push(note);
        }
    }
    let (titles, notes) = (kept_titles, kept_notes);
    // A choice opens its list of values on press, so its control is declared under its own id.
    let ids: Vec<String> = rows
        .iter()
        .map(|row| if matches!(row.value, Value::Choice(..)) { format!("choose_{}", row.id) } else { row.id.to_owned() })
        .collect();
    let opts: Vec<Opt<'_>> = rows
        .iter()
        .zip(&ids)
        .zip(&notes)
        .map(|((row, id), note)| Opt {
            id,
            label: row.label,
            help: note.as_deref(),
            kind: match &row.value {
                Value::Toggle(on) => options::Kind::Toggle { on: *on },
                Value::Slider(value, min, max, step) => {
                    options::Kind::Slider { value: *value as f32, min: *min as f32, max: *max as f32, step: *step as f32 }
                }
                Value::Choice(selected, names) => options::Kind::Choice { options: names, selected: (*selected).min(names.len().saturating_sub(1)) },
                Value::Key(key) => options::Kind::Keybind { key },
                Value::Button(label) => options::Kind::Button { label: label.unwrap_or(row.label) },
                Value::Text(value) => options::Kind::Text { value },
            },
            changed: row.changed,
        })
        .collect();
    let element = state.element().label();
    let page = if tab == 7 { "Profiles" } else { TABS.get(tab).copied().unwrap_or("Settings") };
    let title = if matches!(tab, 1 | 2) { format!("{page}: {element}") } else { page.to_owned() };
    // Consecutive rows of one section make a group; a page with no sections is one group named for the page.
    let mut group: Vec<Group<'_>> = Vec::new();
    let mut start = 0;
    for end in 1..=opts.len() {
        if end == opts.len() || titles[end] != titles[start] {
            let heading = if titles[start].is_empty() { title.as_str() } else { titles[start] };
            // The keybind sections are many and long: one is open at a time, and none starts open.
            let keybinds = tab == KEYBINDS_PAGE && rows[start].section.is_some_and(|section| section != APPEARANCE);
            let id = options::group_id(heading);
            let collapsed = if keybinds { !state.open_section(tab, &id) } else { state.section_collapsed(tab, &id, false) };
            group.push(Group { title: heading, collapsed, options: &opts[start..end] });
            start = end;
        }
    }
    if group.is_empty() {
        group.push(Group { title: &title, collapsed: false, options: &opts });
    }
    let matched = options::search(&group, state.option_query());
    let shown = options::groups_of(&matched);
    let element_label = state.element().label();
    let has_text = opts.iter().any(|o| matches!(o.kind, options::Kind::Text { .. }));
    // The open choice's list of values.
    let open = state.open_choice().and_then(|id| match crate::panel::value(settings, state, id) {
        Value::Choice(selected, names) => {
            let label = rows.iter().find(|row| row.id == id).map_or("", |row| row.label);
            Some((label, names, selected.min(names.len().saturating_sub(1))))
        }
        _ => None,
    });
    let popup = open.map(|(label, names, selected)| options::Popup { title: label, options: names, selected });
    let view = state.picker_view();
    let picker = view.map(|(title, hsv, opacity, hex)| {
        let rgb = u32::from_str_radix(hex, 16).unwrap_or(0);
        crate::menu_v2::picker_data(settings, title, hsv, opacity, hex, rgb)
    });
    f(&PageView { title: &title, groups: &shown, popup: popup.as_ref(), picker: picker.as_ref(), element: element_label, has_text })
}

/// A redesigned settings page: the page's options as rows in the menu shell.
#[must_use]
pub fn settings_panel(settings: &Settings, state: &PanelState, tab: usize) -> String {
    with_page(settings, state, tab, &mut |page| {
        let element = page.element;
        let entries = entries(tab);
        let sections = sections_of(&entries, element);
        let shell = Shell {
            palette: palette(settings),
            brand: "Vantage",
            sections: &sections,
            title: page.title,
            search_hint: "Search options...",
            search: state.option_query(),
            search_id: "opt_search",
            back_id: "back",
            account: "Player",
            account_note: "",
            backdrop: backdrop(settings),
        };
        let preview = (tab == 5).then(|| crosshair_preview(settings));
        options::panel(&shell, page.groups, page.popup, page.picker, preview.as_ref()).to_string()
    })
}

/// The crosshair as it looks at rest, drawn from the settings.
fn crosshair_preview(settings: &Settings) -> options::Side<'static> {
    use crate::crosshair::{self, Target};
    use vantage_ui::{node, tokens::px};
    const BOX: f32 = 112.0;
    let c = &settings.crosshair;
    let frame = c.frame(Target::None, false);
    let look = c.look(false, false);
    let outline = if c.outline && frame.outline[3] > 0.0 { crosshair::snap(c.outline_width as f32, 1) } else { 0.0 };
    let half = BOX / 2.0;
    let p = crosshair::preview_with(&look, outline, c.outline_round, half);
    // The side card is 160 units wide: the drawing is centred in it, below the title.
    let (left, top) = (px(24.0), px(36.0));
    let at = |x: f32, y: f32| [left + px(x), top + px(y)];
    let mut body = vec![node::rounded("backdrop", at(0.0, 0.0), [px(BOX), px(BOX)], px(vantage_ui::tokens::RADII.xs), [0.0, 0.0, 0.0, 0.35])];
    let circle = |name: &str, ring: Option<crosshair::Ring>, centre: [f32; 2], colour: [f32; 4], body: &mut Vec<serde_json::Value>| {
        if let Some(r) = ring {
            let outer = r.radius + r.width / 2.0;
            let inner = (r.radius - r.width / 2.0).max(0.0);
            // A ring is a disc of its outer radius with a hole; the disc of the Dot shape has none.
            body.push(node::rounded(name, at(centre[0] - outer, centre[1] - outer), [px(outer * 2.0), px(outer * 2.0)], px(outer), colour));
            if inner > 0.0 {
                body.push(node::rounded(&format!("{name}_hole"), at(centre[0] - inner, centre[1] - inner), [px(inner * 2.0), px(inner * 2.0)], px(inner), [0.0, 0.0, 0.0, 0.0]));
            }
        }
    };
    let middle = [half, half];
    circle("ring_edge", p.ring_edge, middle, frame.outline, &mut body);
    circle("disc_edge", p.disc_edge, p.disc_at, frame.outline, &mut body);
    for (i, (rect, radius)) in p.edges.iter().zip(&p.edge_radius).enumerate() {
        body.push(node::rounded(&format!("edge_{i}"), at(rect[0], rect[1]), [px(rect[2]), px(rect[3])], px(*radius), frame.outline));
    }
    circle("ring", p.ring, middle, frame.fill, &mut body);
    circle("disc", p.disc, p.disc_at, frame.fill, &mut body);
    for (i, (rect, radius)) in p.fill.iter().zip(&p.fill_radius).enumerate() {
        body.push(node::rounded(&format!("bar_{i}"), at(rect[0], rect[1]), [px(rect[2]), px(rect[3])], px(*radius), frame.fill));
    }
    options::Side { title: "Preview", size: [160.0, 160.0], body }
}

/// One shape of the crosshair preview, in design units inside a box [`PREVIEW_BOX`] across.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PreviewShape {
    /// A bar, as `[x, y, width, height]`, with a corner radius.
    Bar { rect: [f32; 4], radius: f32, color: [f32; 4] },
    /// A ring (or a disc when `inner` is zero) around `centre`.
    Ring { centre: [f32; 2], outer: f32, inner: f32, color: [f32; 4] },
}

/// The side of the crosshair preview's box, in design units.
pub const PREVIEW_BOX: f32 = 112.0;

/// The crosshair at rest as shapes, back to front, from the settings.
#[must_use]
pub fn crosshair_shapes(settings: &Settings) -> Vec<PreviewShape> {
    use crate::crosshair::{self, Target};
    let c = &settings.crosshair;
    let frame = c.frame(Target::None, false);
    let look = c.look(false, false);
    let outline = if c.outline && frame.outline[3] > 0.0 { crosshair::snap(c.outline_width as f32, 1) } else { 0.0 };
    let half = PREVIEW_BOX / 2.0;
    let p = crosshair::preview_with(&look, outline, c.outline_round, half);
    let ring = |r: Option<crosshair::Ring>, centre: [f32; 2], color: [f32; 4]| {
        r.map(|r| PreviewShape::Ring { centre, outer: r.radius + r.width / 2.0, inner: (r.radius - r.width / 2.0).max(0.0), color })
    };
    let middle = [half, half];
    let mut out: Vec<PreviewShape> = Vec::new();
    out.extend(ring(p.ring_edge, middle, frame.outline));
    out.extend(ring(p.disc_edge, p.disc_at, frame.outline));
    out.extend(p.edges.iter().zip(&p.edge_radius).map(|(rect, radius)| PreviewShape::Bar { rect: *rect, radius: *radius, color: frame.outline }));
    out.extend(ring(p.ring, middle, frame.fill));
    out.extend(ring(p.disc, p.disc_at, frame.fill));
    out.extend(p.fill.iter().zip(&p.fill_radius).map(|(rect, radius)| PreviewShape::Bar { rect: *rect, radius: *radius, color: frame.fill }));
    out
}

/// The picker card for the colour being edited.
fn picker_data<'a>(settings: &'a Settings, title: &'a str, hsv: [f32; 3], opacity: Option<u32>, hex: &'a str, rgb: u32) -> vantage_ui::picker::Picker<'a> {
    vantage_ui::picker::Picker {
        title,
        hsv,
        opacity,
        hex,
        saved: settings.gameplay.favorite_colors.contains(&rgb),
        recent: &settings.gameplay.recent_colors,
        favorites: &settings.gameplay.favorite_colors,
        presets: &crate::menu::PRESETS,
        to_rgb: crate::menu::hsv_rgb,
    }
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
        for id in ["module_search", "sort_0", "flt_0", "flt_1", MODS[0].0, "fav_hud_keystrokes"] {
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
            let page: serde_json::Value = serde_json::from_str(&settings_panel(&settings, &state, tab)).unwrap();
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
    fn a_choice_is_picked_from_its_list_and_reset_restores_the_default() {
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
        state.event(&mut settings, "choose_gp_time", 1.0);
        assert_eq!(state.open_choice(), Some("gp_time"));
        let pick = (at + 1) % count;
        state.event(&mut settings, &format!("pick_{pick}"), 1.0);
        let Value::Choice(next, _) = crate::panel::value(&settings, &state, "gp_time") else { panic!() };
        assert_eq!(next, pick);
        assert_eq!(state.open_choice(), None, "picking closes the list");
    }

    #[test]
    fn page_furniture_is_never_listed_as_an_option() {
        let settings = Settings::default();
        let state = PanelState::default();
        let defaults = Settings::default();
        for &tab in &CONVERTED {
            for row in rows(&settings, &state, &defaults, tab) {
                assert!(!is_furniture(tab, row.id), "{} is furniture", row.id);
            }
        }
    }

    #[test]
    fn a_search_no_mod_answers_lists_the_pages_whose_options_match() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.field(&mut settings, "module_search", "thickness", true);
        let hits = option_hits(&settings, &state, "thickness");
        assert!(hits.iter().any(|hit| hit.tab == 5), "the Crosshair page has a thickness option");
        let page: serde_json::Value = serde_json::from_str(&mods_panel(&settings, &state)).unwrap();
        let ids: Vec<&str> = page["controls"].as_array().unwrap().iter().filter_map(|c| c["id"].as_str()).collect();
        assert!(ids.contains(&TAB_IDS[5]) && ids.contains(&"module_search"));
        // A search a mod answers keeps the grid.
        state.field(&mut settings, "module_search", "zoom", true);
        let page: serde_json::Value = serde_json::from_str(&mods_panel(&settings, &state)).unwrap();
        assert!(page["surface"]["collections"]["mods"].is_object());
    }

    #[test]
    fn every_credit_line_fits_its_row() {
        for (title, lines) in CREDITS {
            for line in lines {
                assert!(line.chars().count() <= CREDIT_LINE, "{title}: {line:?} is too long");
            }
        }
    }

    #[test]
    fn dependent_options_and_section_starts_name_real_options_and_switches() {
        let settings = Settings::default();
        let state = PanelState::default();
        for (option, switch) in DEPENDS {
            let control = |id: &str| CONTROLS.iter().find(|c| c.1 == id);
            assert!(control(option).is_some(), "{option} is not a control");
            assert!(matches!(control(switch), Some((Kind::Toggle, ..))), "{switch} is not a switch");
            assert!(matches!(crate::panel::value(&settings, &state, switch), Value::Toggle(_)));
        }
        for (tab, starts) in SECTIONS {
            let listed: Vec<&str> = rows_of(&settings, &state, &Settings::default(), tab).map(|row| row.id).collect();
            for (id, _) in starts {
                assert!(listed.contains(id), "{id} is not an option of page {tab}");
            }
        }
    }

    #[test]
    fn an_option_hides_while_its_switch_is_off_and_returns_when_it_is_on() {
        let mut settings = Settings::default();
        settings.crosshair.dot = false;
        let state = PanelState::default();
        assert!(switched_off(&settings, &state, "ch_dot_round"));
        settings.crosshair.dot = true;
        assert!(!switched_off(&settings, &state, "ch_dot_round"));
        assert!(!switched_off(&settings, &state, "ch_dot"));
    }

    #[test]
    fn a_sectioned_page_declares_exactly_the_rows_it_shows() {
        let settings = Settings::default();
        let state = PanelState::default();
        let page: serde_json::Value = serde_json::from_str(&settings_panel(&settings, &state, 5)).unwrap();
        let ids: Vec<&str> = page["controls"].as_array().unwrap().iter().filter_map(|c| c["id"].as_str()).collect();
        assert!(!ids.contains(&"ch_dot_round") && !ids.contains(&"ch_outline_color"));
        assert!(ids.contains(&"ch_dot") && ids.contains(&"ch_outline"));
    }

    #[test]
    fn the_custom_row_opens_the_picker_and_its_sliders_set_the_colour_that_apply_keeps() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.event(&mut settings, "choose_hit_color", 1.0);
        state.event(&mut settings, &format!("pick_{}", crate::config::CUSTOM_ROW), 1.0);
        assert!(state.picker_view().is_some(), "the Custom row opens the picker");
        // Full saturation and brightness at hue 0 is pure red.
        state.event(&mut settings, "pk_hue", 0.0);
        state.event(&mut settings, "pk_x", 1.0);
        state.event(&mut settings, "pk_y", 0.0);
        assert_eq!(state.picker_view().unwrap().3, "FF0000");
        state.event(&mut settings, "pk_op", 40.0);
        assert_eq!(state.picker_view().unwrap().2, Some(40));
        let before = settings.clone();
        state.event(&mut settings, "ed_ok", 1.0);
        assert!(state.picker_view().is_none(), "Apply closes the picker");
        assert_ne!(settings.hit_color, before.hit_color);
        assert_eq!(crate::config::color_rgb(settings.hit_color), 0xFF0000);
    }

    #[test]
    fn a_preset_picked_in_the_picker_is_kept_by_apply() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.event(&mut settings, "choose_hit_color", 1.0);
        state.event(&mut settings, &format!("pick_{}", crate::config::CUSTOM_ROW), 1.0);
        state.event(&mut settings, "cp_p0", 1.0);
        state.event(&mut settings, "cp_star", 1.0);
        state.event(&mut settings, "ed_ok", 1.0);
        assert!(state.picker_view().is_none());
        assert_eq!(crate::config::color_rgb(settings.hit_color), 0xC62828);
        // A custom colour is past the named ones: the page still labels every control.
        let page: serde_json::Value = serde_json::from_str(&settings_panel(&settings, &state, 10)).unwrap();
        for control in page["controls"].as_array().unwrap() {
            assert!(!control["label"].as_str().unwrap_or("x").is_empty(), "{} has no label", control["id"]);
        }
    }

    #[test]
    fn keybinds_on_the_same_key_name_each_other_and_unbound_ones_never_conflict() {
        let row = |id: &'static str, label: &'static str, key: &'static str| Row { id, label, value: Value::Key(key), changed: false, section: None };
        let rows = [row("a", "Alpha", "KeyZ"), row("b", "Beta", "KeyZ"), row("c", "Gamma", "KeyX"), row("d", "Delta", ""), row("e", "Eps", "")];
        let notes = conflict_notes(&rows);
        assert_eq!(notes[0].as_deref(), Some("Also used by Beta"));
        assert_eq!(notes[1].as_deref(), Some("Also used by Alpha"));
        assert_eq!((notes[2].as_ref(), notes[3].as_ref(), notes[4].as_ref()), (None, None, None));
    }

    #[test]
    fn the_general_page_lists_every_keybind_once_in_its_own_section() {
        let settings = Settings::default();
        let state = PanelState::default();
        let defaults = Settings::default();
        let all = rows(&settings, &state, &defaults, KEYBINDS_PAGE);
        let keys: Vec<&str> = all.iter().filter(|r| matches!(r.value, Value::Key(_))).map(|r| r.id).collect();
        for id in &keys {
            assert_eq!(keys.iter().filter(|k| *k == id).count(), 1, "{id} is listed twice");
        }
        let expected = CONTROLS.iter().filter(|(kind, ..)| *kind == Kind::Keybind).count();
        assert_eq!(keys.len(), expected);
    }

    #[test]
    fn every_rebuilt_page_fits_the_panel_size_limit() {
        // The host's limits (`ui::mod_panel::MAX_PANEL_BYTES` and `MAX_PANEL_CONTROLS`), which this crate cannot import.
        const MAX_PANEL_BYTES: usize = 320 * 1024;
        const MAX_PANEL_CONTROLS: usize = 512;
        const MAX_SURFACE_BINDINGS: usize = 256;
        const MAX_SURFACE_BYTES: usize = 192 * 1024;
        let settings = Settings::default();
        let state = PanelState::default();
        for &tab in &CONVERTED {
            let page = settings_panel(&settings, &state, tab);
            assert!(page.len() <= MAX_PANEL_BYTES, "page {tab} is {} bytes", page.len());
            let document = serde_json::from_str::<serde_json::Value>(&page).unwrap()["surface"]["document"].as_str().unwrap().len();
            assert!(document <= MAX_SURFACE_BYTES, "page {tab}'s document is {document} bytes");
            let controls = serde_json::from_str::<serde_json::Value>(&page).unwrap()["controls"].as_array().unwrap().len();
            assert!(controls <= MAX_PANEL_CONTROLS, "page {tab} declares {controls} controls");
            let bindings = serde_json::from_str::<serde_json::Value>(&page).unwrap()["surface"]["bindings"].as_object().unwrap().len();
            assert!(bindings <= MAX_SURFACE_BINDINGS, "page {tab} binds {bindings} values");
        }
    }

    #[test]
    fn each_keybind_section_fits_the_surface_limit_when_it_is_opened_alone() {
        let defaults = Settings::default();
        let mut sections: Vec<&str> = rows(&defaults, &PanelState::default(), &defaults, KEYBINDS_PAGE).iter().filter_map(|row| row.section).collect();
        sections.dedup();
        assert!(sections.len() > 2, "the keybinds are grouped by owner");
        for section in sections {
            let mut settings = Settings::default();
            let mut state = PanelState::default();
            state.event(&mut settings, "tab_6", 1.0);
            state.event(&mut settings, &options::group_id(section), 1.0);
            let page: serde_json::Value = serde_json::from_str(&settings_panel(&settings, &state, KEYBINDS_PAGE)).unwrap();
            let document = page["surface"]["document"].as_str().unwrap().len();
            assert!(document <= 192 * 1024, "{section} opens to a {document} byte document");
        }
    }

    #[test]
    fn the_layout_page_starts_the_hud_mover_and_the_mover_hides_the_page_until_it_is_done() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.event(&mut settings, "tab_1", 1.0);
        let ids = |page: &str| -> Vec<String> {
            let value: serde_json::Value = serde_json::from_str(page).unwrap();
            value["controls"].as_array().unwrap().iter().filter_map(|c| c["id"].as_str().map(str::to_owned)).collect()
        };
        assert!(ids(&settings_panel(&settings, &state, 1)).contains(&"move".to_owned()));
        state.event(&mut settings, "move", 1.0);
        assert!(state.moving);
        // The original document, with its layout bar, is what is shown while moving.
        assert!(ids(&crate::panel::build(&settings, &state)).contains(&"move_done".to_owned()));
        state.event(&mut settings, "move_done", 1.0);
        state.event(&mut settings, "move_done", 1.0);
        assert!(!state.moving);
        assert!(ids(&settings_panel(&settings, &state, 1)).contains(&"move".to_owned()));
    }
}
