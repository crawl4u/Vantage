//! The Mods page: category chips, a sort button and a grid of mod cards.
//!
//! [`visible`] decides which cards show and in what order (a pure function, tested alone). The
//! card is a native collection template: the host lays out and scrolls the grid, and each card's
//! toggle, favourite and open buttons press declared controls through item roles.

use serde_json::{Value, json};

use crate::node::{self, Align};
use crate::shell::{self, Rect};
use crate::tokens::{Palette, RADII, space, text, px};

/// One mod's card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Card<'a> {
    /// A stable identifier for the card's declared controls.
    pub id: &'a str,
    pub name: &'a str,
    pub category: &'a str,
    pub on: bool,
    pub favorite: bool,
    pub hidden: bool,
    /// The card's glyph from [`crate::icons`], or empty to show the name's first letter.
    pub icon: &'a str,
    /// The id of the card's open button.
    pub open: &'a str,
}

/// The ids the page declares its controls with, so a caller can reuse the ids its own handlers know.
pub struct Ids<'a> {
    /// The prefix of the sort segments' ids (`<sort>_<n>`, `n` indexing [`Sort::ALL`]) and the search field.
    pub sort: &'a str,
    pub search: &'a str,
    /// A filter chip's id.
    pub chip: &'a dyn Fn(Filter<'_>) -> String,
    /// A card's switch and favourite ids, from the card's `id`.
    pub toggle: &'a dyn Fn(&str) -> String,
    pub favorite: &'a dyn Fn(&str) -> String,
    /// A card's hide switch id.
    pub hide: &'a dyn Fn(&str) -> String,
    /// A card's drag handle id; the handle's event value is the card's new place in the shown order.
    pub reorder: &'a dyn Fn(&str) -> String,
}

impl Ids<'static> {
    /// Self-contained ids for pages with no handlers of their own: `chip_<label>`, `<id>_on`, `<id>_fav`.
    #[must_use]
    pub fn plain() -> Self {
        fn chip(filter: Filter<'_>) -> String {
            match filter {
                Filter::All => "chip_all".into(),
                Filter::Enabled => "chip_enabled".into(),
                Filter::Favorites => "chip_favorites".into(),
                Filter::Hidden => "chip_hidden".into(),
                Filter::Category(name) => format!("chip_{}", name.to_lowercase()),
            }
        }
        fn toggle(id: &str) -> String {
            format!("{id}_on")
        }
        fn favorite(id: &str) -> String {
            format!("{id}_fav")
        }
        fn hide(id: &str) -> String {
            format!("{id}_hide")
        }
        fn reorder(id: &str) -> String {
            format!("{id}_move")
        }
        Self { sort: "sort", search: "search", chip: &chip, toggle: &toggle, favorite: &favorite, hide: &hide, reorder: &reorder }
    }
}

/// Which cards a chip selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter<'a> {
    All,
    Enabled,
    Favorites,
    Category(&'a str),
    Hidden,
}

/// How the visible cards are ordered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sort {
    /// The order the cards were given in.
    #[default]
    Default,
    /// By name, ignoring case.
    Name,
    /// Switched-on mods first, then the given order.
    EnabledFirst,
}

impl Sort {
    /// Every mode, in the order the segment control lists them.
    pub const ALL: [Sort; 3] = [Sort::Default, Sort::Name, Sort::EnabledFirst];

    /// The segment's text.
    #[must_use]
    pub fn short(self) -> &'static str {
        match self {
            Self::Default => "Custom",
            Self::Name => "A-Z",
            Self::EnabledFirst => "On",
        }
    }

    /// The next mode when the sort button is pressed.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Default => Self::Name,
            Self::Name => Self::EnabledFirst,
            Self::EnabledFirst => Self::Default,
        }
    }

    /// The button's text.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "Sort: Default",
            Self::Name => "Sort: A to Z",
            Self::EnabledFirst => "Sort: On first",
        }
    }
}

/// What the page shows besides the cards.
#[derive(Clone, Copy, Debug)]
pub struct State<'a> {
    pub filter: Filter<'a>,
    pub sort: Sort,
    /// The search text; a card matches when its name contains every word, ignoring case.
    pub query: &'a str,
    /// The Custom order: each card's place, by card index; empty keeps the given order.
    pub custom: &'a [usize],
}

/// Whether `card` belongs to `filter`. Hidden cards belong only to [`Filter::Hidden`].
fn selected(card: &Card<'_>, filter: Filter<'_>) -> bool {
    match filter {
        Filter::Hidden => card.hidden,
        _ if card.hidden => false,
        Filter::All => true,
        Filter::Enabled => card.on,
        Filter::Favorites => card.favorite,
        Filter::Category(name) => card.category == name,
    }
}

/// Whether `card`'s name has every word of `query`, in any case.
fn matches(card: &Card<'_>, query: &str) -> bool {
    let name = card.name.to_lowercase();
    query
        .split_whitespace()
        .all(|word| name.contains(&word.to_lowercase()))
}

/// The indices of the cards to show, in display order.
#[must_use]
pub fn visible(cards: &[Card<'_>], state: &State<'_>) -> Vec<usize> {
    let mut shown: Vec<usize> = (0..cards.len())
        .filter(|&i| selected(&cards[i], state.filter) && matches(&cards[i], state.query))
        .collect();
    match state.sort {
        Sort::Default if state.custom.len() == cards.len() => shown.sort_by_key(|&i| state.custom[i]),
        Sort::Default => {}
        Sort::Name => shown.sort_by_key(|&i| cards[i].name.to_lowercase()),
        Sort::EnabledFirst => shown.sort_by_key(|&i| !cards[i].on),
    }
    shown
}

/// A filter chip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chip<'a> {
    pub label: &'a str,
    pub filter: Filter<'a>,
}

/// The chips to show, in order: All, Enabled, Favorites, then every category that has a card
/// (in `categories` order), then Hidden when anything is hidden. The chip of the active filter always
/// shows, even when nothing belongs to it, so it can be left again by pressing it.
#[must_use]
pub fn chips<'a>(
    cards: &[Card<'_>],
    categories: &[&'a str],
    active: Filter<'a>,
) -> Vec<Chip<'a>> {
    let mut chips = vec![
        Chip { label: "All", filter: Filter::All },
        Chip { label: "Enabled", filter: Filter::Enabled },
        Chip { label: "Favorites", filter: Filter::Favorites },
    ];
    for &category in categories {
        let present = cards.iter().any(|card| !card.hidden && card.category == category);
        if present || active == Filter::Category(category) {
            chips.push(Chip { label: category, filter: Filter::Category(category) });
        }
    }
    if cards.iter().any(|card| card.hidden) || active == Filter::Hidden {
        chips.push(Chip { label: "Hidden", filter: Filter::Hidden });
    }
    chips
}

/// The help shown in place of an empty grid, as a title and a line under it.
#[must_use]
pub fn empty_message(state: &State<'_>) -> (&'static str, &'static str) {
    if !state.query.trim().is_empty() {
        return ("No matching mods", "Try a different name.");
    }
    match state.filter {
        Filter::Favorites => ("No favorite mods", "Star a mod on its card to keep it here."),
        Filter::Hidden => ("No hidden mods", "Mods you hide from their card show up here."),
        Filter::Enabled => ("No mods are on", "Switch a mod on from the bar on its card."),
        _ => ("No mods here", "Nothing is listed in this group."),
    }
}

// ---- Geometry (GUI pixels) --------------------------------------------------------------------

/// The space between the pane's edge and what it holds, in design units.
const PAD: f32 = space::XL;
/// A chip's height and the gap after the row, in design units.
const CHIP_HEIGHT: f32 = 28.0;
const ROW_GAP: f32 = space::L;
/// Grid columns.
pub const COLUMNS: usize = 4;
/// A card cell's height, in design units (the plate plus the gap below it).
const CELL_HEIGHT: f32 = 132.0;
/// The gap between plates, in design units.
const GAP: f32 = space::M;
/// The bottom bar's height, in design units.
const BAR: f32 = 26.0;
/// The side of a card's corner and handle controls, in design units: a little larger than their glyphs.
const TARGET: f32 = 24.0;
/// The icon font's scale factors: a card's icon and its favourite star.
const CARD_ICON: f32 = 3.0;
const STAR_ICON: f32 = 1.3;

/// The pane's inner rectangle (inside the padding), in GUI pixels.
#[must_use]
pub fn inner() -> Rect {
    let [x, y, w, h] = shell::content_rect();
    [x + px(PAD), y + px(PAD), w - px(PAD) * 2.0, h - px(PAD) * 2.0]
}

/// The grid's viewport (width, height) in GUI pixels, below the chip row.
#[must_use]
pub fn viewport() -> [f32; 2] {
    let [_, _, w, h] = inner();
    [w, h - px(CHIP_HEIGHT + ROW_GAP)]
}

/// One cell's size in GUI pixels: the viewport's width shared by the columns, rounded down so the
/// columns always fit.
#[must_use]
pub fn cell() -> [f32; 2] {
    let [w, _] = viewport();
    [((w - 8.0) / COLUMNS as f32).floor(), px(CELL_HEIGHT)]
}

/// Approximate width of a chip with `label`, in GUI pixels.
fn chip_width(label: &str) -> f32 {
    label.chars().count() as f32 * px(text::LABEL.size) * 0.56 + px(space::XL * 1.25)
}

fn scale(style: crate::tokens::TextStyle) -> f32 {
    px(style.size) / 9.0
}

/// The card template: a plate with an icon tile, the mod's name on a bar that switches it, a star
/// that favourites it, and a press anywhere else that opens its settings. The host binds
/// `#item_name`, `#item_initial`, and the animated `#item_toggle_toggle_progress`,
/// `#item_favorite_toggle_progress` and `#item_open_hover`.
#[must_use]
pub fn template(p: &Palette, icons: bool) -> Value {
    let [cw, ch] = cell();
    let gap = px(GAP);
    let (w, h) = (cw - gap, ch - gap);
    let bar = px(BAR);
    let alpha = |name: &str| json!([{"binding_name": name, "binding_name_override": "#alpha"}]);
    let press = |name: &str, role: &str, offset: [f32; 2], size: [f32; 2]| {
        node::button(name, offset, size, &format!("mod.item:{role}"), vec![])
    };
    let with_bindings = |mut control: Value, bindings: Value| {
        let name = control.as_object().and_then(|m| m.keys().next().cloned()).unwrap();
        control[name.as_str()]["bindings"] = bindings;
        control
    };
    let tile = px(48.0);
    let controls = vec![
        node::rounded("plate", [0.0, 0.0], [w, h], px(RADII.md), p.raised),
        node::outline("edge", [0.0, 0.0], [w, h], px(RADII.md), 1.0, p.border),
        with_bindings(
            node::rounded("hover", [0.0, 0.0], [w, h], px(RADII.md), [1.0, 1.0, 1.0, 0.07]),
            alpha("#item_open_hover"),
        ),
        // Below the strip that holds the hide, drag and favourite controls, so a press there is theirs alone.
        press("open", "open", [0.0, px(TARGET + space::S * 2.0)], [w, h - bar - px(TARGET + space::S * 2.0)]),
        node::rounded(
            "tile",
            [(w - tile) / 2.0, (h - bar - tile) / 2.0 - px(space::XS)],
            [tile, tile],
            px(RADII.sm),
            p.dim,
        ),
        {
            // The icon, or the name's first letter when the cards have none, centred in the tile.
            let top = (h - bar - tile) / 2.0 - px(space::XS);
            if icons {
                node::bound_icon("initial", "#item_icon", [(w - tile) / 2.0, top + (tile - px(40.0)) / 2.0], [tile, px(40.0)], p.text, CARD_ICON)
            } else {
                let mut initial = node::label(
                    "initial",
                    "#item_initial",
                    [(w - tile) / 2.0, top + (tile - px(text::DISPLAY.size) * 1.6) / 2.0],
                    [tile, px(text::DISPLAY.size) * 1.6],
                    p.text,
                    scale(text::DISPLAY),
                    Align::Center,
                );
                initial["initial"]["bindings"] = json!([{"binding_name": "#item_initial"}]);
                initial["initial"]["text"] = json!("#item_initial");
                initial
            }
        },
        // The bar: neutral when off, accent when on (cross-faded by the toggle's progress).
        node::rounded("bar_off", [0.0, h - bar], [w, bar], px(RADII.md), p.dim),
        with_bindings(
            node::rounded("bar_on", [0.0, h - bar], [w, bar], px(RADII.md), p.accent),
            alpha("#item_toggle_toggle_progress"),
        ),
        {
            let mut name = node::label(
                "name_off",
                "#item_name",
                [px(space::S), h - bar + (bar - px(text::LABEL.size) * 1.6) / 2.0],
                [w - px(space::S) * 2.0, px(text::LABEL.size) * 1.6],
                p.text,
                scale(text::LABEL),
                Align::Center,
            );
            name["name_off"]["bindings"] = json!([{"binding_name": "#item_name"}]);
            name
        },
        {
            let mut name = node::label(
                "name_on",
                "#item_name",
                [px(space::S), h - bar + (bar - px(text::LABEL.size) * 1.6) / 2.0],
                [w - px(space::S) * 2.0, px(text::LABEL.size) * 1.6],
                p.on_accent,
                scale(text::LABEL),
                Align::Center,
            );
            name["name_on"]["bindings"] = json!([
                {"binding_name": "#item_name"},
                {"binding_name": "#item_toggle_toggle_progress", "binding_name_override": "#alpha"},
            ]);
            name
        },
        press("switch", "toggle", [0.0, h - bar], [w, bar]),
        // The favourite star: an outline glyph that fills with the gold when set.
        node::icon("star_off", crate::icons::STAR, [w - px(space::XXL), px(space::S)], [px(20.0), px(20.0)], p.muted, STAR_ICON),
        node::bound(
            node::icon("star_on", crate::icons::STAR, [w - px(space::XXL), px(space::S)], [px(20.0), px(20.0)], p.star, STAR_ICON),
            alpha("#item_favorite_toggle_progress"),
        ),
        // The hide control: an eye in the opposite corner.
        node::icon("eye", crate::icons::EYE, [px(space::S), px(space::S)], [px(20.0), px(20.0)], p.muted, STAR_ICON),
        press("hide", "hide", [px(space::S + 4.0), px(space::S)], [px(TARGET), px(TARGET)]),
        // The drag handle, top centre: press it and drag the card to another place in the grid.
        node::icon("grip", crate::icons::LIST, [(w - px(20.0)) / 2.0, px(space::S)], [px(20.0), px(20.0)], p.muted, STAR_ICON),
        press("handle", "reorder", [(w - px(TARGET)) / 2.0 + px(8.0), px(space::S)], [px(TARGET), px(TARGET)]),
        press(
            "star",
            "favorite",
            [w - px(TARGET), px(space::S)],
            [px(TARGET), px(TARGET)],
        ),
    ];
    json!({
        "screen": "vantage.card",
        "document": json!({
            "namespace": "vantage",
            "card": {"type": "panel", "size": [cw, ch], "controls": controls},
        }).to_string(),
    })
}

/// The first letter of a mod's name, upper case, for its icon tile.
#[must_use]
pub fn initial(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default()
}

/// The page's own controls (chips, the sort button and the grid's placeholder), placed in the pane.
#[must_use]
pub fn page_controls(
    p: &Palette,
    chips: &[Chip<'_>],
    state: &State<'_>,
    shown: usize,
    first_control: usize,
) -> Vec<Value> {
    let [px0, py0, _, _] = inner();
    let mut out = Vec::new();
    let mut x = px0;
    for (i, chip) in chips.iter().enumerate() {
        let width = chip_width(chip.label);
        let active = chip.filter == state.filter;
        let (fill, ink) = if active { (p.accent, p.on_accent) } else { (p.raised, p.text) };
        out.push(node::button(
            &format!("chip_{i}"),
            [x, py0],
            [width, px(CHIP_HEIGHT)],
            &format!("mod.control:{}", first_control + i),
            vec![
                node::rounded("fill", [0.0, 0.0], [width, px(CHIP_HEIGHT)], px(RADII.xs), fill),
                node::label(
                    "text",
                    chip.label,
                    [0.0, (px(CHIP_HEIGHT) - px(text::LABEL.size) * 1.6) / 2.0],
                    [width, px(text::LABEL.size) * 1.6],
                    ink,
                    scale(text::LABEL),
                    Align::Center,
                ),
            ],
        ));
        x += width + px(space::XS);
    }
    // The sort segments: one joined bar, the active mode in the accent.
    let [_, _, iw, _] = inner();
    let widths: Vec<f32> = Sort::ALL.iter().map(|mode| chip_width(mode.short()) - px(space::S)).collect();
    let mut sx = px0 + iw - widths.iter().sum::<f32>();
    for (n, mode) in Sort::ALL.iter().enumerate() {
        let width = widths[n];
        let active = *mode == state.sort;
        let (fill, ink) = if active { (p.accent, p.on_accent) } else { (p.raised, p.muted) };
        out.push(node::button(
            &format!("sort_{n}"),
            [sx, py0],
            [width, px(CHIP_HEIGHT)],
            &format!("mod.control:{}", first_control + chips.len() + n),
            vec![
                node::rounded("fill", [0.0, 0.0], [width, px(CHIP_HEIGHT)], px(RADII.xs), fill),
                node::label(
                    "text",
                    mode.short(),
                    [0.0, (px(CHIP_HEIGHT) - px(text::LABEL.size) * 1.6) / 2.0],
                    [width, px(text::LABEL.size) * 1.6],
                    ink,
                    scale(text::LABEL),
                    Align::Center,
                ),
            ],
        ));
        sx += width;
    }
    let grid_y = py0 + px(CHIP_HEIGHT + ROW_GAP);
    if shown == 0 {
        let (title, line) = empty_message(state);
        let [_, vh] = viewport();
        out.push(node::label(
            "empty_title",
            title,
            [px0, grid_y + vh / 2.0 - px(text::HEADING.size) * 1.6],
            [iw, px(text::HEADING.size) * 1.6],
            p.highlight,
            scale(text::HEADING),
            Align::Center,
        ));
        out.push(node::label(
            "empty_line",
            line,
            [px0, grid_y + vh / 2.0 + px(space::XS)],
            [iw, px(text::SMALL.size) * 1.6],
            p.muted,
            scale(text::SMALL),
            Align::Center,
        ));
    } else {
        out.push(json!({"grid": {"type": "panel", "cinnabar_collection": "mods", "offset": [px0, grid_y]}}));
    }
    out
}

/// The controls each card declares: switch, favourite, hide, drag handle and open.
pub const PER_CARD: usize = 5;

/// The control ids the page declares after the shell's: one chip button per chip, the sort button,
/// then [`PER_CARD`] per card (switch, favourite, hide, drag handle, open).
#[must_use]
pub fn declared_controls(cards: &[Card<'_>], chips: &[Chip<'_>], ids: &Ids<'_>) -> Vec<Value> {
    let mut declared: Vec<Value> = chips
        .iter()
        .map(|chip| json!({"kind": "button", "id": (ids.chip)(chip.filter), "label": chip.label}))
        .collect();
    for (n, mode) in Sort::ALL.iter().enumerate() {
        declared.push(json!({"kind": "button", "id": format!("{}_{n}", ids.sort), "label": mode.short()}));
    }
    for card in cards {
        declared.push(json!({"kind": "toggle", "id": (ids.toggle)(card.id), "label": card.name, "value": card.on}));
        declared.push(json!({"kind": "toggle", "id": (ids.favorite)(card.id), "label": card.name, "value": card.favorite}));
        declared.push(json!({"kind": "toggle", "id": (ids.hide)(card.id), "label": card.name, "value": card.hidden}));
        declared.push(json!({"kind": "button", "id": (ids.reorder)(card.id), "label": card.name}));
        declared.push(json!({"kind": "button", "id": card.open, "label": card.name}));
    }
    declared
}

/// The grid's collection: every card as an item and `order` as the cards to show.
#[must_use]
pub fn collection(p: &Palette, cards: &[Card<'_>], order: &[usize], first_card_control: usize) -> Value {
    let [vw, vh] = viewport();
    let items: Vec<Value> = cards
        .iter()
        .enumerate()
        .map(|(i, card)| {
            let base = first_card_control + i * PER_CARD;
            json!({
                "values": {"#item_name": card.name, "#item_initial": initial(card.name), "#item_icon": card.icon},
                "controls": {"toggle": base, "favorite": base + 1, "hide": base + 2, "reorder": base + 3, "open": base + 4},
            })
        })
        .collect();
    json!({
        "definition": {"template": template(p, cards.iter().all(|card| !card.icon.is_empty())), "columns": COLUMNS, "viewport": [vw, vh], "items": items},
        "order": order,
    })
}

/// The whole Mods page on top of the shell, as a native panel description.
#[must_use]
pub fn panel(
    shell_state: &shell::Shell<'_>,
    cards: &[Card<'_>],
    categories: &[&str],
    state: &State<'_>,
    ids: &Ids<'_>,
) -> Value {
    let p = shell_state.palette;
    let p_backdrop = shell_state.backdrop;
    let order = visible(cards, state);
    let chips = chips(cards, categories, state.filter);
    let shell_controls = shell::declared_controls(shell_state);
    let first_page = shell_controls.len();
    let mut controls = shell::controls(shell_state);
    controls.extend(page_controls(&p, &chips, state, order.len(), first_page));
    let mut declared = shell_controls;
    declared.extend(declared_controls(cards, &chips, ids));
    let [w, h] = shell::window_px();
    let mut collections = serde_json::Map::new();
    if !order.is_empty() {
        collections.insert(
            "mods".into(),
            collection(&p, cards, &order, first_page + chips.len() + Sort::ALL.len()),
        );
    }
    json!({
        "reference_size": crate::shell::reference_size(),
        "fill": true,
        "controls": declared,
        "surface": {
            "screen": "vantage.root",
            "document": json!({
                "namespace": "vantage",
                "root": crate::shell::root([w, h], controls),
            }).to_string(),
            "collections": collections,
            "bindings": p_backdrop.bindings(),
            "transitions": shell::Backdrop::transitions(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card<'a>(id: &'a str, name: &'a str, category: &'a str) -> Card<'a> {
        Card { id, name, category, on: false, favorite: false, hidden: false, icon: "", open: "" }
    }

    fn cards() -> Vec<Card<'static>> {
        vec![
            Card { on: true, ..card("fps", "FPS", "HUD") },
            Card { favorite: true, ..card("zoom", "Zoom", "Mechanic") },
            card("cps", "CPS", "HUD"),
            Card { hidden: true, ..card("fog", "Fog", "Visual") },
            Card { on: true, favorite: true, ..card("sky", "Sky layers", "Visual") },
        ]
    }

    fn state<'a>(filter: Filter<'a>, sort: Sort, query: &'a str) -> State<'a> {
        State { filter, sort, query, custom: &[] }
    }

    #[test]
    fn all_shows_every_card_that_is_not_hidden_in_the_given_order() {
        assert_eq!(visible(&cards(), &state(Filter::All, Sort::Default, "")), [0, 1, 2, 4]);
    }

    #[test]
    fn each_filter_selects_its_own_cards_and_hidden_cards_only_show_under_hidden() {
        let c = cards();
        assert_eq!(visible(&c, &state(Filter::Enabled, Sort::Default, "")), [0, 4]);
        assert_eq!(visible(&c, &state(Filter::Favorites, Sort::Default, "")), [1, 4]);
        assert_eq!(visible(&c, &state(Filter::Category("HUD"), Sort::Default, "")), [0, 2]);
        assert_eq!(visible(&c, &state(Filter::Category("Visual"), Sort::Default, "")), [4]);
        assert_eq!(visible(&c, &state(Filter::Hidden, Sort::Default, "")), [3]);
    }

    #[test]
    fn search_needs_every_word_in_the_name_in_any_case() {
        let c = cards();
        assert_eq!(visible(&c, &state(Filter::All, Sort::Default, "SKY lay")), [4]);
        assert_eq!(visible(&c, &state(Filter::All, Sort::Default, "ps")), [0, 2]);
        assert!(visible(&c, &state(Filter::All, Sort::Default, "nope")).is_empty());
    }

    #[test]
    fn the_custom_order_arranges_the_cards_only_under_the_custom_sort() {
        let c = cards();
        let custom = [4, 3, 2, 1, 0];
        let with = |sort| visible(&c, &State { filter: Filter::All, sort, query: "", custom: &custom });
        assert_eq!(with(Sort::Default), [4, 2, 1, 0]);
        assert_eq!(with(Sort::Name), [2, 0, 4, 1]);
    }

    #[test]
    fn sorting_by_name_ignores_case_and_enabled_first_keeps_the_given_order_within_each_group() {
        let c = cards();
        assert_eq!(visible(&c, &state(Filter::All, Sort::Name, "")), [2, 0, 4, 1]);
        assert_eq!(visible(&c, &state(Filter::All, Sort::EnabledFirst, "")), [0, 4, 1, 2]);
    }

    #[test]
    fn the_sort_button_cycles_through_every_mode_and_back() {
        assert_eq!(Sort::Default.next().next().next(), Sort::Default);
        assert_eq!(Sort::Name.label(), "Sort: A to Z");
    }

    #[test]
    fn chips_list_only_groups_with_cards_and_always_the_active_one() {
        let c = cards();
        let categories = ["HUD", "Visual", "Mechanic", "Combat"];
        fn labels<'a>(chips: &[Chip<'a>]) -> Vec<&'a str> {
            chips.iter().map(|chip| chip.label).collect()
        }
        assert_eq!(
            labels(&chips(&c, &categories, Filter::All)),
            ["All", "Enabled", "Favorites", "HUD", "Visual", "Mechanic", "Hidden"]
        );
        // An empty category shows while it is the active one, so it can be left again.
        assert!(labels(&chips(&c, &categories, Filter::Category("Combat"))).contains(&"Combat"));
        // Nothing hidden: no Hidden chip.
        let none_hidden: Vec<_> = c.iter().copied().map(|card| Card { hidden: false, ..card }).collect();
        assert!(!labels(&chips(&none_hidden, &categories, Filter::All)).contains(&"Hidden"));
    }

    #[test]
    fn an_empty_grid_explains_itself_for_the_search_and_for_each_filter() {
        let s = |filter, query| empty_message(&state(filter, Sort::Default, query)).0;
        assert_eq!(s(Filter::All, "zzz"), "No matching mods");
        assert_eq!(s(Filter::Favorites, ""), "No favorite mods");
        assert_eq!(s(Filter::Hidden, ""), "No hidden mods");
        assert_eq!(s(Filter::Enabled, ""), "No mods are on");
    }

    #[test]
    fn the_icon_tile_shows_the_first_letter_in_upper_case() {
        assert_eq!(initial("keystrokes"), "K");
        assert_eq!(initial("  3D view"), "3");
        assert_eq!(initial(""), "");
    }

    #[test]
    fn the_columns_always_fit_the_viewport_with_the_hosts_margin() {
        let [vw, _] = viewport();
        let [cw, ch] = cell();
        assert!(cw * COLUMNS as f32 + 8.0 <= vw, "{cw} x {COLUMNS} does not fit {vw}");
        assert!(ch > px(BAR) * 2.0);
    }

    #[test]
    fn every_page_control_is_declared_and_the_card_roles_match_the_template() {
        let c = cards();
        let shell = shell::Shell {
            palette: Palette::dark(),
            brand: "Vantage",
            sections: &[],
            title: "Mods",
            search_hint: "Search mods...",
            search: "",
            search_id: "search",
            back_id: "back",
            account: "Player",
            account_note: "Online",
            backdrop: shell::Backdrop::NONE,
        };
        let state = state(Filter::All, Sort::Default, "");
        let panel = panel(&shell, &c, &["HUD", "Visual", "Mechanic"], &state, &Ids::plain());
        let declared = panel["controls"].as_array().unwrap().len();
        // The shell's back, forward and search, 7 chips, the sort segments, and the cards' own.
        assert_eq!(declared, 3 + 7 + Sort::ALL.len() + c.len() * PER_CARD);
        let template: Value = serde_json::from_str(
            panel["surface"]["collections"]["mods"]["definition"]["template"]["document"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        let mut roles = Vec::new();
        for control in template["card"]["controls"].as_array().unwrap() {
            if let Some(press) = node::body(control)["button_mappings"][0]["to_button_id"].as_str() {
                roles.push(press.to_owned());
            }
        }
        roles.sort();
        assert_eq!(roles, ["mod.item:favorite", "mod.item:hide", "mod.item:open", "mod.item:reorder", "mod.item:toggle"]);
        for item in panel["surface"]["collections"]["mods"]["definition"]["items"].as_array().unwrap() {
            for index in item["controls"].as_object().unwrap().values() {
                assert!((index.as_u64().unwrap() as usize) < declared);
            }
        }
    }
}
