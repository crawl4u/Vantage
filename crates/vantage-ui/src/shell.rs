//! The menu shell: a window with a sidebar of grouped pages, a header (back and forward, the page
//! title, a search field, close) and a content pane. Pages are placed in the pane; the shell does
//! not know what they hold.
//!
//! All geometry is computed from the tokens in design units and converted to GUI pixels once, so
//! the same shell scales with [`crate::tokens::UNIT`].

use serde_json::{Value, json};

use crate::node::{self, Align};
use crate::tokens::{Palette, RADII, space, text, px};

/// The window, in design units.
pub const WINDOW: [f32; 2] = [1080.0, 545.0];
/// The sidebar's width, in design units.
pub const SIDEBAR_WIDTH: f32 = 210.0;
/// The header's height, in design units.
pub const HEADER_HEIGHT: f32 = 64.0;
/// One sidebar entry's height and the gap below it, in design units.
const ENTRY_HEIGHT: f32 = 36.0;
const ENTRY_GAP: f32 = 4.0;
/// The side margin of entries inside the sidebar, in design units.
const ENTRY_INSET: f32 = 18.0;
/// The brand's distance from the top, in design units.
const BRAND_TOP: f32 = 28.0;
/// Where the first section starts, in design units.
const NAV_TOP: f32 = 84.0;
/// The account row's height, in design units.
const ACCOUNT_HEIGHT: f32 = 72.0;
/// The search field's size, in design units.
const SEARCH: [f32; 2] = [200.0, 28.0];
/// The close button's side, in design units.
const CLOSE: f32 = 28.0;
/// The icon font's scale factors: in the sidebar, in the header's arrows and close button.
const SIDEBAR_ICON: f32 = 1.25;
const HEADER_ICON: f32 = 1.15;

/// How the world behind the menu is blurred and dimmed while it is open: a blur in GUI pixels, and
/// brightness and saturation factors where 1 leaves the picture unchanged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Backdrop {
    pub blur: f32,
    pub brightness: f32,
    pub saturation: f32,
}

impl Backdrop {
    /// The world as it is.
    pub const NONE: Self = Self { blur: 0.0, brightness: 1.0, saturation: 1.0 };

    /// The surface bindings the host reads to blur and dim the world.
    #[must_use]
    pub fn bindings(self) -> serde_json::Map<String, Value> {
        let mut map = serde_json::Map::new();
        map.insert("#world_blur".into(), json!(self.blur.round().clamp(0.0, 32.0)));
        map.insert("#world_brightness".into(), json!(self.brightness.clamp(0.0, 2.0)));
        map.insert("#world_saturation".into(), json!(self.saturation.clamp(0.0, 2.0)));
        map
    }

    /// The easings that fade the effect in when the menu opens and out when it closes.
    #[must_use]
    pub fn transitions() -> Value {
        json!({
            "#world_blur": {"duration": 0.25, "from": 0, "exit": 0},
            "#world_brightness": {"duration": 0.25, "from": 1, "exit": 1},
        })
    }
}

/// One page the sidebar lists.
#[derive(Clone, Copy, Debug)]
pub struct Entry<'a> {
    /// A stable identifier: letters, digits, `_`, `-` and `.`.
    pub id: &'a str,
    pub title: &'a str,
    /// A glyph from [`crate::icons`], or empty for none.
    pub icon: &'a str,
    pub active: bool,
}

/// A group of entries under an optional small heading.
#[derive(Clone, Copy, Debug)]
pub struct Section<'a> {
    pub heading: Option<&'a str>,
    pub entries: &'a [Entry<'a>],
}

/// Everything the shell shows.
#[derive(Clone, Copy, Debug)]
pub struct Shell<'a> {
    pub palette: Palette,
    /// The brand line at the top of the sidebar.
    pub brand: &'a str,
    pub sections: &'a [Section<'a>],
    /// The current page's title in the header.
    pub title: &'a str,
    /// The search field's hint, and the text typed in it.
    pub search_hint: &'a str,
    pub search: &'a str,
    /// The id the search field's control is declared with.
    pub search_id: &'a str,
    /// The id the back button's control is declared with.
    pub back_id: &'a str,
    /// The signed-in player's name, and a short line under it.
    pub account: &'a str,
    pub account_note: &'a str,
    /// The blur and dimming of the world behind the menu.
    pub backdrop: Backdrop,
}

/// The index of the first control after the sidebar entries: back, forward, then search.
#[must_use]
pub fn first_extra_control(sections: &[Section<'_>]) -> usize {
    sections.iter().map(|section| section.entries.len()).sum()
}

/// The controls the panel declares, in the order the buttons press them: one per entry, then
/// back, forward and the search field.
#[must_use]
pub fn declared_controls(shell: &Shell<'_>) -> Vec<Value> {
    let mut declared: Vec<Value> = shell
        .sections
        .iter()
        .flat_map(|section| section.entries)
        .map(|entry| json!({"kind": "button", "id": entry.id, "label": entry.title}))
        .collect();
    declared.push(json!({"kind": "button", "id": shell.back_id, "label": "Back"}));
    declared.push(json!({"kind": "button", "id": "forward", "label": "Forward"}));
    declared.push(json!({
        "kind": "text", "id": shell.search_id, "label": "Search", "value": shell.search,
        "max_chars": 40, "live": true,
    }));
    declared
}

/// A rectangle in GUI pixels: `[x, y, width, height]`.
pub type Rect = [f32; 4];

/// The window's size in GUI pixels.
#[must_use]
pub fn window_px() -> [f32; 2] {
    [px(WINDOW[0]), px(WINDOW[1])]
}

/// The size the host fits the window to: the window with a margin all round, so it scales up to nearly fill
/// the screen and no further.
#[must_use]
pub fn reference_size() -> [f32; 2] {
    let [w, h] = window_px();
    [w * SCREEN_MARGIN, h * SCREEN_MARGIN]
}

/// The room left around the window when it fills the screen, as a factor of its size.
const SCREEN_MARGIN: f32 = 1.12;

/// The root of a page: a panel filling the viewport, with the window centred in it.
#[must_use]
pub fn root(size: [f32; 2], controls: Vec<Value>) -> Value {
    json!({
        "type": "panel",
        "size": ["100%", "100%"],
        "controls": [{"window": {
            "type": "panel",
            "size": size,
            "anchor_from": "center",
            "anchor_to": "center",
            "controls": controls,
        }}],
    })
}

/// Where a page is drawn: the content pane, in GUI pixels.
#[must_use]
pub fn content_rect() -> Rect {
    let x = SIDEBAR_WIDTH + space::GUTTER;
    let y = HEADER_HEIGHT;
    [
        px(x),
        px(y),
        px(WINDOW[0] - x - space::GUTTER),
        px(WINDOW[1] - y - space::GUTTER),
    ]
}

/// The sidebar entries' rectangles, in GUI pixels, in section order, without the headings.
#[must_use]
pub fn entry_rects(sections: &[Section<'_>]) -> Vec<Rect> {
    let mut rects = Vec::new();
    let mut y = NAV_TOP;
    for section in sections {
        if section.heading.is_some() {
            y += 26.0;
        }
        for _ in section.entries {
            rects.push([
                px(ENTRY_INSET),
                px(y),
                px(SIDEBAR_WIDTH - ENTRY_INSET * 2.0),
                px(ENTRY_HEIGHT),
            ]);
            y += ENTRY_HEIGHT + ENTRY_GAP;
        }
        y += space::L;
    }
    rects
}

/// A label's font scale for a type style: the panel font is drawn at 9 GUI pixels at scale 1, so a style of N design units is N * UNIT / 9.
fn scale(style: crate::tokens::TextStyle) -> f32 {
    px(style.size) / 9.0
}

/// A label box for one line of `style` text.
fn line_box(style: crate::tokens::TextStyle, width: f32) -> [f32; 2] {
    [px(width), px(style.size) * 1.6]
}

/// The shell's controls, ready to sit under a document root of the window's size.
#[must_use]
pub fn controls(shell: &Shell<'_>) -> Vec<Value> {
    let p = &shell.palette;
    let [w, h] = window_px();
    let mut out = vec![
        node::rounded("window", [0.0, 0.0], [w, h], px(RADII.lg), p.panel),
    ];

    // Brand: the logo, the width of the sidebar less its margins.
    out.extend(crate::logo::logo([px(space::XXL + space::XS), px(BRAND_TOP)], px(SIDEBAR_WIDTH - 72.0)));

    // Sections and entries.
    let mut y = NAV_TOP;
    let mut index = 0;
    let rects = entry_rects(shell.sections);
    for section in shell.sections {
        if let Some(heading) = section.heading {
            out.push(node::label(
                &format!("heading_{index}"),
                &heading.to_uppercase(),
                [px(space::XXL + space::XS), px(y + 4.0)],
                line_box(text::EYEBROW, SIDEBAR_WIDTH - 60.0),
                p.muted,
                scale(text::EYEBROW),
                Align::Left,
            ));
            y += 26.0;
        }
        for entry in section.entries {
            let rect = rects[index];
            let (fill, ink) = if entry.active {
                (Some(p.accent), p.on_accent)
            } else {
                (None, p.text)
            };
            let mut parts = Vec::new();
            if let Some(fill) = fill {
                parts.push(node::rounded(
                    "fill",
                    [0.0, 0.0],
                    [rect[2], rect[3]],
                    px(RADII.sm),
                    fill,
                ));
            }
            let text_x = if entry.icon.is_empty() { space::XL } else { 42.0 };
            if !entry.icon.is_empty() {
                parts.push(node::icon(
                    "icon",
                    entry.icon,
                    [px(space::M), (rect[3] - px(24.0)) / 2.0],
                    [px(28.0), px(24.0)],
                    ink,
                    SIDEBAR_ICON,
                ));
            }
            parts.push(node::label(
                "title",
                entry.title,
                [px(text_x), (rect[3] - px(text::LABEL.size) * 1.6) / 2.0],
                line_box(text::LABEL, SIDEBAR_WIDTH - ENTRY_INSET * 2.0 - 56.0),
                ink,
                scale(text::LABEL),
                Align::Left,
            ));
            out.push(node::button(
                &format!("entry_{index}"),
                [rect[0], rect[1]],
                [rect[2], rect[3]],
                &format!("mod.control:{index}"),
                parts,
            ));
            y += ENTRY_HEIGHT + ENTRY_GAP;
            index += 1;
        }
        y += space::L;
    }

    // Account row.
    let row_y = WINDOW[1] - ACCOUNT_HEIGHT;
    out.push(node::rounded(
        "account_rule",
        [px(ENTRY_INSET), px(row_y)],
        [px(SIDEBAR_WIDTH - ENTRY_INSET * 2.0), 1.0],
        0.0,
        p.border,
    ));
    out.push(node::rounded(
        "avatar",
        [px(ENTRY_INSET), px(row_y + 18.0)],
        [px(36.0), px(36.0)],
        px(RADII.sm),
        p.raised,
    ));
    out.push(node::label(
        "account",
        shell.account,
        [px(ENTRY_INSET + 48.0), px(row_y + 18.0)],
        line_box(text::LABEL, SIDEBAR_WIDTH - 90.0),
        p.text,
        scale(text::LABEL),
        Align::Left,
    ));
    out.push(node::label(
        "account_note",
        shell.account_note,
        [px(ENTRY_INSET + 48.0), px(row_y + 38.0)],
        line_box(text::CAPTION, SIDEBAR_WIDTH - 90.0),
        p.muted,
        scale(text::CAPTION),
        Align::Left,
    ));

    // Header.
    let gutter = space::GUTTER;
    let head_y = (HEADER_HEIGHT - CLOSE) / 2.0;
    let extra = first_extra_control(shell.sections);
    let arrow = |name: &str, x: f32, glyph: &str, control: usize, is_icon: bool| {
        node::button(
            name,
            [px(x), px(head_y)],
            [px(CLOSE), px(CLOSE)],
            &format!("mod.control:{control}"),
            vec![if is_icon {
                node::icon("g", glyph, [0.0, px(CLOSE - 24.0) / 2.0], [px(CLOSE), px(24.0)], p.muted, HEADER_ICON)
            } else {
                node::label(
                    "g",
                    glyph,
                    [0.0, px(CLOSE - text::HEADING.size * 1.6) / 2.0],
                    [px(CLOSE), px(text::HEADING.size) * 1.6],
                    p.muted,
                    scale(text::HEADING),
                    Align::Center,
                )
            }],
        )
    };
    out.push(arrow("back", SIDEBAR_WIDTH + gutter - space::S, crate::icons::CHEVRON_LEFT, extra, true));
    out.push(arrow("forward", SIDEBAR_WIDTH + gutter + CLOSE - space::S, ">", extra + 1, false));
    out.push(node::label(
        "title",
        shell.title,
        [px(SIDEBAR_WIDTH + gutter + CLOSE * 2.0 + space::M), px(head_y + 2.0)],
        line_box(text::HEADING, 320.0),
        p.highlight,
        scale(text::HEADING),
        Align::Left,
    ));
    let close_x = WINDOW[0] - gutter - CLOSE;
    let search_x = close_x - space::M - SEARCH[0];
    out.push(node::rounded(
        "search",
        [px(search_x), px(head_y)],
        [px(SEARCH[0]), px(SEARCH[1])],
        px(RADII.xs),
        p.field,
    ));
    out.push(node::outline(
        "search_edge",
        [px(search_x), px(head_y)],
        [px(SEARCH[0]), px(SEARCH[1])],
        px(RADII.xs),
        1.0,
        p.border,
    ));
    let (shown, ink) = if shell.search.is_empty() {
        (shell.search_hint, p.muted)
    } else {
        (shell.search, p.text)
    };
    out.push(node::button(
        "search_box",
        [px(search_x), px(head_y)],
        [px(SEARCH[0]), px(SEARCH[1])],
        &format!("mod.edit:{}", extra + 2),
        vec![node::label(
            "text",
            shown,
            [px(space::L), px(4.0)],
            line_box(text::SMALL, SEARCH[0] - 24.0),
            ink,
            scale(text::SMALL),
            Align::Left,
        )],
    ));
    out.push(node::button(
        "close",
        [px(close_x), px(head_y)],
        [px(CLOSE), px(CLOSE)],
        "mod.close",
        vec![
            node::rounded("fill", [0.0, 0.0], [px(CLOSE), px(CLOSE)], px(RADII.xs), p.raised),
            node::icon("x", crate::icons::X, [0.0, px(CLOSE - 24.0) / 2.0], [px(CLOSE), px(24.0)], p.text, HEADER_ICON),
        ],
    ));

    out
}

/// The shell as a panel document: `{"namespace", "root"}`.
#[must_use]
pub fn document(shell: &Shell<'_>) -> Value {
    let [w, h] = window_px();
    json!({
        "namespace": "vantage",
        "root": root([w, h], controls(shell)),
    })
}

/// The shell as a native panel description: the document as a surface at the window's size.
#[must_use]
pub fn panel(shell: &Shell<'_>) -> Value {
    json!({
        "reference_size": reference_size(),
        "fill": true,
        "controls": declared_controls(shell),
        "surface": {
            "screen": "vantage.root",
            "document": document(shell).to_string(),
            "bindings": shell.backdrop.bindings(),
            "transitions": Backdrop::transitions(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRIES: [Entry<'static>; 3] = [
        Entry { id: "mods", title: "Mods", icon: "", active: true },
        Entry { id: "profiles", title: "Profiles", icon: "", active: false },
        Entry { id: "keybinds", title: "Keybinds", icon: "", active: false },
    ];
    const SECTIONS: [Section<'static>; 2] = [
        Section { heading: Some("Mods & options"), entries: &ENTRIES },
        Section { heading: None, entries: &ENTRIES },
    ];

    fn shell() -> Shell<'static> {
        Shell {
            palette: Palette::dark(),
            brand: "Vantage",
            sections: &SECTIONS,
            title: "Mods",
            search_hint: "Search mods...",
            search: "",
            search_id: "search",
            back_id: "back",
            account: "Player",
            account_note: "Online",
            backdrop: Backdrop::NONE,
        }
    }

    #[test]
    fn the_content_pane_sits_right_of_the_sidebar_inside_the_window_with_the_gutter_around_it() {
        let [x, y, w, h] = content_rect();
        let [ww, wh] = window_px();
        assert!(x >= px(SIDEBAR_WIDTH));
        assert!(y >= px(HEADER_HEIGHT));
        assert!((x + w - (ww - px(space::GUTTER))).abs() < 1.0e-3);
        assert!((y + h - (wh - px(space::GUTTER))).abs() < 1.0e-3);
    }

    #[test]
    fn every_entry_fits_in_the_sidebar_above_the_account_row() {
        let [_, wh] = window_px();
        let rects = entry_rects(&SECTIONS);
        assert_eq!(rects.len(), 6);
        for rect in &rects {
            assert!(rect[0] >= 0.0 && rect[0] + rect[2] <= px(SIDEBAR_WIDTH));
            assert!(rect[1] + rect[3] <= wh - px(ACCOUNT_HEIGHT), "{rect:?} overlaps the account");
        }
        for pair in rects.windows(2) {
            assert!(pair[1][1] >= pair[0][1] + pair[0][3], "entries overlap");
        }
    }

    #[test]
    fn every_button_presses_a_declared_control() {
        let panel = panel(&shell());
        let declared = panel["controls"].as_array().unwrap().len();
        // Six entries, back, forward and the search field.
        assert_eq!(declared, 9);
        let document: Value =
            serde_json::from_str(panel["surface"]["document"].as_str().unwrap()).unwrap();
        let controls = document["root"]["controls"][0]["window"]["controls"].as_array().unwrap();
        let presses: Vec<&str> = controls
            .iter()
            .filter_map(|control| node::body(control)["button_mappings"][0]["to_button_id"].as_str())
            .collect();
        for press in &presses {
            let index = press
                .strip_prefix("mod.control:")
                .or_else(|| press.strip_prefix("mod.edit:"))
                .map(|index| index.parse::<usize>().unwrap());
            match index {
                Some(index) => assert!(index < declared, "{press} is not declared"),
                None => assert_eq!(*press, "mod.close"),
            }
        }
        for index in 0..6 {
            assert!(presses.contains(&format!("mod.control:{index}").as_str()));
        }
        assert!(presses.contains(&"mod.edit:8"), "the search field opens its editor");
    }

    #[test]
    fn the_active_entry_alone_is_filled_with_the_accent() {
        let controls = controls(&shell());
        let accent = Palette::dark().accent;
        let filled = controls
            .iter()
            .filter(|control| {
                node::body(control)["controls"].as_array().is_some_and(|parts| {
                    parts.iter().any(|part| node::body(part)["color"] == json!(accent))
                })
            })
            .count();
        assert_eq!(filled, 2, "the first section and the second each list one active entry");
    }

    #[test]
    fn every_control_stays_inside_the_window() {
        let [w, h] = window_px();
        for control in controls(&shell()) {
            let node = node::body(&control);
            let (Some(size), Some(offset)) = (node["size"].as_array(), node["offset"].as_array())
            else {
                continue;
            };
            let value = |list: &Vec<Value>, i: usize| list[i].as_f64().unwrap() as f32;
            assert!(value(offset, 0) + value(size, 0) <= w + 0.01, "{control} spills right");
            assert!(value(offset, 1) + value(size, 1) <= h + 0.01, "{control} spills down");
        }
    }
}
