//! The settings menu's panel JSON, in the shared Waypoints look (see `tools/menukit.py`).
//!
//! The document is generated once and embedded in the component as its panel bundle (see the
//! pack tool), so the mod never sends it at run time: copying it would cost a callback's worth
//! of fuel. The mod sends `patch`es with its controls' values and which tab is open; `build`
//! is the whole panel, for the bundle and for a changed menu key. The host draws every value,
//! flips switches, drags sliders and opens dropdowns.

use crate::config::Element;
use crate::menu_controls::{
    ALTERNATES, COLLECTION, CONTROLS, DD_PAD, DD_ROW, DD_ROWS, CHUNKS, CHUNK_ENDS, DOC_CHUNKS, DOC_CONTROLS, DOC_LENS, GROWTH, HEIGHT, Kind, MODS, MODS_PAGE, MOVE, SCREEN, SHIFTS, TABS, TAILS, VARIANTS, WIDTH,
};
use crate::crosshair::{self, Ring, Target};
use crate::crosshair_config::CrosshairSettings;
use crate::menu_tabset::TAB_CONTROLS;

/// A control's current value, as the mod sees it.
pub enum Value<'a> {
    Toggle(bool),
    /// Value, minimum, maximum, step.
    Slider(f64, f64, f64, f64),
    /// Selected index and the options.
    Choice(usize, &'a [&'a str]),
    /// The bound key.
    Key(&'a str),
    /// A button's text; `None` keeps the generated label.
    Button(Option<&'a str>),
    /// A native text field's text.
    Text(&'a str),
}


/// A key no keyboard sends, as the panel's own toggle key: Cinnabar needs one, but these menus
/// open from Mods Core, or with a key the player sets.
pub const NO_KEY: &str = "F24";


/// Whether the player set a menu key of their own (F10 belongs to Mods Core).
pub fn has_own_key(key: &str) -> bool {
    !key.is_empty() && key != "None" && key != "F10" && key != NO_KEY
}

/// Which tab an event id opens, if it is a tab.
pub fn tab_event(id: &str) -> Option<usize> {
    id.strip_prefix("tab_").and_then(|n| n.parse().ok()).filter(|n| *n < TABS.len())
}

/// Appends document `n`, already escaped to sit inside a JSON string: the pieces it is made of
/// (`DOC_CHUNKS`), copied from the one text that holds every distinct piece.
pub fn push_document(o: &mut String, n: usize) {
    for &piece in DOC_CHUNKS[n] {
        let end = CHUNK_ENDS[usize::from(piece)] as usize;
        let start = if piece == 0 { 0 } else { CHUNK_ENDS[usize::from(piece) - 1] as usize };
        o.push_str(&CHUNKS[start..end]);
    }
}

/// Document `n` as one string (tests).
#[cfg(test)]
pub fn document_text(n: usize) -> String {
    let mut o = String::with_capacity(DOC_LENS[n]);
    push_document(&mut o, n);
    o
}

/// Which document the host is sent for a page: usually the page's own, but Armor status (which has
/// no box) gets a second document for pages that have one (the Style page), and so do the elements
/// that have cards of their own there (`VARIANTS`: a document holds only the cards of the element
/// edited, instead of every element's, so a page stays within what the host accepts).
pub fn document_index(tab: usize, selected: usize) -> usize {
    if selected == Element::Armor.index() {
        if let Some((_, document)) = ALTERNATES.iter().find(|(page, _)| *page == tab) {
            return *document;
        }
    }
    if let Some((_, _, document)) = VARIANTS.iter().find(|(page, element, _)| *page == tab && *element == selected) {
        return *document;
    }
    tab.min(TABS.len() - 1)
}

/// Settings that show or hide rows of the menu: whether the element being edited has its
/// background on (the Style page's other box options show with it) and whether the hit flash is
/// Custom (its colour and opacity show with it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Flags<'a> {
    pub background: bool,
    /// The edited element's border is on: its thickness, opacity and colour rows show.
    pub border: bool,
    pub hit_custom: bool,
    /// The blur behind the menu, in GUI pixels (Menu blur).
    pub blur: f64,
    /// Brightness and saturation factors of the world behind the menu (1 is unchanged).
    pub backdrop: (f64, f64),
    /// The Crosshair's shape (the one on screen) has straight arms: Gapped or Plus.
    pub lines: bool,
    /// The Crosshair has a shape with a centre dot option (Gapped or Circle).
    pub dots: bool,
    /// The Config page's name and paste fields are empty: their faded hint shows.
    pub name_empty: bool,
    pub paste_empty: bool,
    /// The Mods grid's search box is empty: its faded hint shows.
    pub search_empty: bool,
    pub quick_empty: bool,
    /// The Crosshair's settings, for the page whose window grows with a preview of it.
    pub crosshair: Option<&'a CrosshairSettings>,
    /// The page was just opened from the Crosshair page while its window was wide: the window
    /// starts that wide and eases back.
    pub from_wide: bool,
    /// The Mods grid's filter chip (0 All, 1 On, 2 HUD, 3 Visual, 4 Mechanic, 5 Favorites), its sort, the cards it shows
    /// (indices into `MODS`, in order), how many mods are on, and the chosen saved setup: they light a chip and a slot
    /// and fill the line under the grid.
    pub filter: usize,
    pub sort: usize,
    pub order: &'a [usize],
    pub enabled: usize,
    pub slot: usize,
    /// The Waypoints page: its rows and fields (which rows show, their colours, which fields show their hint).
    pub wp: Option<&'a crate::waypoint_ui::WpUi>,
}

impl Flags<'_> {
    /// How much of the tail rows of `kind` show, from 0 (none) to 1 (all): the flags of `TAIL_KIND` in
    /// menukit.py. The Crosshair's two cards have two tail rows each, of which the second goes first
    /// (half shows): the round centre dot goes while there is no centre dot, and the rounded outline
    /// while the lines have round ends of their own.
    fn tail(self, kind: u8) -> f32 {
        let (dot, round_lines) = self.crosshair.map_or((false, false), |c| (c.dot, c.round_lines));
        let amount = |on: bool| if on { 1.0 } else { 0.0 };
        match kind {
            0 => amount(self.background),
            1 => amount(self.hit_custom),
            4 => amount(self.border),
            // The Waypoints list: a sixteenth for each waypoint shown.
            5 => self.wp.map_or(0.0, |w| w.rows.len().min(16) as f32 / 16.0),
            2 if !self.lines => 0.0,
            2 => if round_lines { 0.5 } else { 1.0 },
            3 if !self.dots => 0.0,
            _ => if dot { 1.0 } else { 0.5 },
        }
    }
}

/// The dropdown list the mod draws itself (`dropdown` in `tools/menukit.py`): whether it is drawn,
/// where, which way it opens and which row is the chosen one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Dd {
    /// Drawn and pressable: from opening until its fade out has finished.
    pub shown: bool,
    /// Eased open; false while it fades out.
    pub open: bool,
    /// Top left of the list in window units.
    pub pos: [f32; 2],
    /// Opens upward, so it slides in from below.
    pub up: bool,
    /// How many of its rows exist.
    pub rows: usize,
    /// The first option shown and how many there are (a list longer than its rows scrolls).
    pub first: usize,
    pub total: usize,
    /// The chosen row.
    pub selected: usize,
    /// The key prompt, the number editor, the reset confirmation and the toast (`popups` in menukit.py).
    pub cap: Pop,
    pub ed: Pop,
    /// The colour picker (`popups` in menukit.py): a popup of its own, drawn for a hex colour instead of `ed`.
    pub cp: Pop,
    /// Its hue, saturation and brightness, each from 0 to 1.
    pub hsv: [f32; 3],
    /// The picker's recent and saved colours (red, green, blue, alpha; an empty slot is a faint box).
    pub recent: [[f32; 4]; 6],
    pub saved: [[f32; 4]; 6],
    /// Top left of the number editor in window units.
    pub ed_pos: [f32; 2],
    /// The editor is for a hex colour (a native text field and a swatch of `swatch`), not a number.
    pub hex: bool,
    pub swatch: [f32; 3],
    /// The picker's colour opacity as percent transparent (0 is opaque), for its strip's marker.
    pub transparency: u32,
    pub cf: Pop,
    pub toast: Pop,
    /// The page's scroll bar.
    pub bar: Bar,
    /// Keys go to the mod (a key is being bound or a number typed), not to the host.
    pub typing: bool,
}

/// The colour picker popup's size and the boxes in it, in popup units (`PICKER_*` and `popups` in menukit.py): its saturation /
/// brightness square (left, top, width, height) and its hue strip.
pub const PICKER_W: f32 = 138.0;
pub const PICKER_H: f32 = 206.0;
pub const PICKER_SV: [f32; 4] = [34.0, 16.0, 84.0, 82.0];
pub const PICKER_HUE: [f32; 4] = [34.0, 102.0, 84.0, 8.0];
/// The vertical opacity strip (opaque at the top), and its checkerboard / colour slices.
pub const PICKER_OP: [f32; 4] = [122.0, 16.0, 8.0, 82.0];
pub const PICKER_OP_SLICES: usize = 10;
/// The preset grid: its top left, a cell's side and the gap, in two columns (dark, light) of `PRESETS.len() / 2` rows.
pub const PICKER_PRESET: [f32; 4] = [8.0, 16.0, 10.0, 2.0];
/// The preset colours, a dark and a light one for each hue, row by row (`PRESETS` in menukit.py draws the same ones).
pub const PRESETS: [u32; 14] = [
    0xC6_2828, 0xFF_6B6B, 0xE6_5100, 0xFF_A94D, 0xF2_B600, 0xFF_E066, 0x2E_9E3E, 0x69_DB7C, 0x0E_9AA7, 0x66_D9E8, 0x1C_5FD4, 0x74_A7FF, 0xB5_179E, 0xF0_6ACF,
];

/// The colour of hue `h`, saturation `s` and brightness `v` (each 0 to 1) as red, green and blue from 0 to 1.
pub fn hsv_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = if h.is_finite() { h.clamp(0.0, 1.0) } else { 0.0 };
    let (s, v) = (s.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
    let sector = (h * 6.0).min(5.999_999);
    let f = sector - sector.floor();
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match sector as u32 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// The hue, saturation and brightness (each 0 to 1) of a colour's red, green and blue.
pub fn rgb_hsv(rgb: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = rgb;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d <= 0.0 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    [h, if max <= 0.0 { 0.0 } else { d / max }, max]
}

/// A popup the mod draws: drawn from opening until its fade out has finished, eased open while `open`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pop {
    pub shown: bool,
    pub open: bool,
}

/// The page's scroll bar (see `mod_scroller` in `tools/menukit.py`): how far the page is scrolled,
/// the thumb's place and length along the track, and whether the thumb is held.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bar {
    pub visible: bool,
    pub scroll: f32,
    pub thumb: f32,
    pub thumb_len: f32,
    pub hot: bool,
}

/// How far the list slides while it fades in and out, in window units.
const DD_SLIDE: f32 = 6.0;
/// Empty space above the rows that unroll with a card, inside their clip box (`REVEAL_PAD` in menukit.py).
const REVEAL_PAD: f32 = 1.0;

/// The whole panel: `toggle_key` opens it, `capturing` is the keybind waiting for a key.
/// `selected` is the HUD element the picker dropdowns show. `moving` is the "move with the
/// mouse" mode: the window hides and the HUD shows, with a small bar that is `bar_hidden`
/// while an element is held (it fades out and back).
pub fn build<'a>(
    toggle_key: &str,
    capturing: Option<&str>,
    tab: usize,
    selected: usize,
    moving: bool,
    bar_hidden: bool,
    flags: Flags<'_>,
    dd: Dd,
    value: &dyn Fn(&str) -> Value<'a>,
) -> String {
    let document = document_index(tab, selected);
    let mut o = String::with_capacity(DOC_LENS[document] + 4096);
    // The panel itself opens with no key: Mods Core opens it, and a key the player sets is
    // handled by the mod (see `has_own_key`). The host no longer takes a `toggle_key` field (it
    // rejects the whole screen as an unknown field), so none is sent.
    let _ = toggle_key;
    // The host draws the dropdown list, the slider value editor and the key-capture box; these
    // colours give them the menu's violet look (its own are dark grey with a coral highlight).
    o.push_str("{\"motion\":true,\"widget_colors\":{\"background\":[0.085,0.065,0.15,0.98],");
    o.push_str("\"surface\":[0.26,0.21,0.4,1],\"text\":[0.96,0.95,0.99,1],");
    o.push_str("\"accent\":[0.72,0.55,1,1],\"border\":[0.46,0.39,0.66,1]},\"capture_key\":");
    o.push_str(if capturing.is_some() || dd.typing { "true" } else { "false" });
    o.push_str(",\"show_gameplay_hud\":");
    o.push_str(if moving { "true" } else { "false" });
    o.push_str(",\"reference_size\":[");
    push_u(&mut o, u64::from(WIDTH + 20));
    o.push(',');
    push_u(&mut o, u64::from(HEIGHT + 20));
    o.push_str("],\"controls\":[");
    // Only this page's controls, in the order its document counts them (the host takes 512 per panel).
    for (n, &global) in DOC_CONTROLS[document_index(tab, selected)].iter().enumerate() {
        let (kind, id, label) = &CONTROLS[usize::from(global)];
        if n > 0 {
            o.push(',');
        }
        if let Some(t) = tab_event(id) {
            control(&mut o, "button", id, TABS[t]);
            o.push('}');
            continue;
        }
        match (kind, value(id)) {
            (Kind::Toggle, Value::Toggle(on)) => {
                control(&mut o, "toggle", id, label);
                o.push_str(if on { ",\"value\":true}" } else { ",\"value\":false}" });
            }
            (Kind::Slider, Value::Slider(v, min, max, step)) => {
                control(&mut o, "slider", id, label);
                o.push_str(",\"value\":");
                num(&mut o, v.clamp(min, max));
                o.push_str(",\"min\":");
                num(&mut o, min);
                o.push_str(",\"max\":");
                num(&mut o, max);
                o.push_str(",\"step\":");
                num(&mut o, step);
                o.push('}');
            }
            // A dropdown is a button showing its chosen option: the list it opens is drawn by the
            // menu itself (see `Dd`), not by the host.
            (Kind::Choice, Value::Choice(index, options)) => {
                control(&mut o, "button", id, options.get(index.min(options.len().saturating_sub(1))).copied().unwrap_or(label));
                o.push('}');
            }
            (Kind::Keybind, Value::Key(key)) => {
                control(&mut o, "keybind", id, label);
                o.push_str(",\"key\":");
                q(&mut o, if key.is_empty() { "None" } else { key });
                o.push_str(if capturing == Some(*id) { ",\"capturing\":true}" } else { ",\"capturing\":false}" });
            }
            (Kind::Text, Value::Text(text)) => {
                control(&mut o, "text", id, label);
                o.push_str(",\"value\":");
                q(&mut o, text);
                // The slot name is applied as it is typed (a field commits only on Enter otherwise).
                o.push_str(if *id == "cfg_name" || *id == "hex_in" || *id == "ed_text" || *id == "module_search" || id.starts_with("wp_") { ",\"min_chars\":0,\"max_chars\":256,\"live\":true,\"navigation_keys\":false}" } else { ",\"min_chars\":0,\"max_chars\":256,\"live\":false,\"navigation_keys\":false}" });
            }
            (Kind::Button, Value::Button(text)) => {
                control(&mut o, "button", id, text.unwrap_or(label));
                o.push('}');
            }
            // Aliases, and any value that does not match its control, are plain buttons.
            _ => {
                control(&mut o, "button", id, label);
                o.push('}');
            }
        }
    }
    o.push_str("],\"surface\":{\"screen\":\"");
    o.push_str(SCREEN);
    o.push_str("\",\"document\":\"");
    push_document(&mut o, document);
    o.push_str("\",");
    // The Mods page's grid: its template and entries, and which of them show (the mod filters, searches and sorts).
    if tab == MODS_PAGE {
        o.push_str("\"collections\":{\"modules\":{\"definition\":");
        o.push_str(COLLECTION);
        o.push_str(",\"order\":");
        order_json(&mut o, flags.order);
        o.push_str("}},");
    }
    // The world behind the menu blurs and dims in, and back out when it closes.
    o.push_str("\"transitions\":{\"#world_blur\":{\"duration\":0.25,\"from\":0,\"exit\":0},");
    o.push_str("\"#world_brightness\":{\"duration\":0.25,\"from\":1,\"exit\":1},");
    // The dropdown list eases open and shut: it fades and slides a few units (see `tab_bindings`).
    o.push_str("\"#dd_a\":{\"duration\":0.16,\"from\":0,\"exit\":0,\"easing\":[0.2,0.8,0.2,1]},");
    o.push_str("\"#dd_o\":{\"duration\":0.2,\"from\":[0,-6],\"exit\":[0,-6],\"easing\":[0.2,0.8,0.2,1]},");
    // A page whose window grows eases its edge out (see `growth_bindings`) every time it opens.
    // (Every page has them: the others start wide when they come from the grown page.)
    if let Some(&(_, grow, _)) = GROWTH.first() {
        let ease = ",\"duration\":0.4,\"easing\":[0.2,0.8,0.2,1]},";
        let start = if flags.from_wide { WIDTH + grow } else { WIDTH };
        for (name, from) in [("#xw", f64::from(start)), ("#xw2", f64::from(start) - 2.0), ("#xrule", f64::from(start - WIDTH / 2))] {
            o.push('"');
            o.push_str(name);
            o.push_str("\":{\"from\":");
            num(&mut o, from);
            o.push_str(ease);
        }
        o.push_str("\"#xclose\":{\"from\":[");
        num(&mut o, f64::from(start) - 26.0);
        o.push_str(",7]");
        o.push_str(ease);
        if GROWTH.iter().any(|g| g.0 == tab) {
            o.push_str("\"#xp_w\":{\"from\":0");
            o.push_str(ease);
        }
    }
    // The other popups ease the same way, and the page's scroll bar and content follow the wheel smoothly.
    for p in ["cap", "ed", "cp", "cf", "toast"] {
        o.push_str("\"#");
        o.push_str(p);
        o.push_str("_a\":{\"duration\":0.2,\"from\":0,\"exit\":0,\"easing\":[0.2,0.8,0.2,1]},\"#");
        o.push_str(p);
        o.push_str("_o\":{\"duration\":0.26,\"from\":[0,-6],\"exit\":[0,-6],\"easing\":[0.2,0.8,0.2,1]},");
    }
    o.push_str("\"#scroll_off\":{\"duration\":0.1,\"easing\":[0.2,0.8,0.2,1]},");
    o.push_str("\"#stick_off\":{\"duration\":0.1,\"easing\":[0.2,0.8,0.2,1]},");
    o.push_str("\"#thumb_off\":{\"duration\":0.1,\"easing\":[0.2,0.8,0.2,1]},");
    o.push_str("\"#thumb_hot\":{\"duration\":0.15},");
    // The cards that grow with a flag ease their height, their gradient, their bottom cap and the box of rows that unrolls.
    for (n, (_, name, ..)) in TAILS.iter().enumerate() {
        if TAILS[..n].iter().any(|(_, other, ..)| other == name) {
            continue;
        }
        for part in ["h", "gh", "bot", "clip"] {
            o.push_str("\"#tl_");
            o.push_str(name);
            o.push('_');
            o.push_str(part);
            o.push_str("\":{\"duration\":0.24,\"easing\":[0.2,0.8,0.2,1]},");
        }
    }
    // The cards under a card that gives up its last rows ease up and down with it.
    for (n, (_, name, ..)) in SHIFTS.iter().enumerate() {
        if SHIFTS[..n].iter().any(|(_, other, ..)| other == name) {
            continue;
        }
        o.push_str("\"#tl_");
        o.push_str(name);
        o.push_str("\":{\"duration\":0.24,\"easing\":[0.2,0.8,0.2,1]},");
    }
    // The move bar fades out while an element is held and back in after it is let go.
    o.push_str("\"#move_alpha\":{\"duration\":0.2,\"from\":1,\"exit\":1}},");
    tab_bindings(&mut o, tab, selected, moving, bar_hidden, flags, dd);
    o.push_str("}}");
    o
}

/// The values of the open page's controls and which page is open, for `panel.update-content`.
pub fn patch<'a>(
    capturing: Option<&str>,
    tab: usize,
    selected: usize,
    moving: bool,
    bar_hidden: bool,
    flags: Flags<'_>,
    dd: Dd,
    value: &dyn Fn(&str) -> Value<'a>,
) -> String {
    let mut o = String::with_capacity(64 + 24 * 48);
    o.push_str("{\"capture_key\":");
    o.push_str(if capturing.is_some() || dd.typing { "true" } else { "false" });
    o.push_str(",\"show_gameplay_hud\":");
    o.push_str(if moving { "true" } else { "false" });
    o.push_str(",\"controls\":[");
    let mut first = true;
    // Only the open page's controls: the others are not on screen, and the page that replaces
    // this one is sent whole. (Page buttons and picker chips never change their text and are
    // not in the list.)
    for index in TAB_CONTROLS[document_index(tab, selected)] {
        let (kind, id, label) = &CONTROLS[usize::from(*index)];
        let v = value(id);
        let start = o.len();
        if !first {
            o.push(',');
        }
        o.push_str("{\"id\":");
        q(&mut o, id);
        match (kind, v) {
            (Kind::Toggle, Value::Toggle(on)) => o.push_str(if on { ",\"value\":true}" } else { ",\"value\":false}" }),
            (Kind::Slider, Value::Slider(v, min, max, _)) => {
                o.push_str(",\"value\":");
                num(&mut o, v.clamp(min, max));
                o.push('}');
            }
            (Kind::Choice, Value::Choice(index, options)) => {
                o.push_str(",\"label\":");
                q(&mut o, options.get(index.min(options.len().saturating_sub(1))).copied().unwrap_or(label));
                o.push('}');
            }
            (Kind::Keybind, Value::Key(key)) => {
                o.push_str(",\"key\":");
                q(&mut o, if key.is_empty() { "None" } else { key });
                o.push_str(if capturing == Some(*id) { ",\"capturing\":true}" } else { ",\"capturing\":false}" });
            }
            (Kind::Text, Value::Text(text)) => {
                o.push_str(",\"value\":");
                q(&mut o, text);
                o.push('}');
            }
            (Kind::Button, Value::Button(Some(text))) => {
                o.push_str(",\"label\":");
                q(&mut o, text);
                o.push('}');
            }
            (Kind::Button, Value::Button(None)) => {
                o.push_str(",\"label\":");
                q(&mut o, label);
                o.push('}');
            }
            // Aliases and mismatched values change nothing.
            _ => {
                o.truncate(start);
                continue;
            }
        }
        first = false;
    }
    o.push_str("],");
    if tab == MODS_PAGE {
        o.push_str("\"collection_order\":{\"modules\":");
        order_json(&mut o, flags.order);
        o.push_str("},");
    }
    tab_bindings(&mut o, tab, selected, moving, bar_hidden, flags, dd);
    o.push('}');
    o
}

/// The labels of the Mods grid's sort button, in the order of `Flags::sort`.
pub const SORTS: [&str; 3] = ["Sort: Default", "Sort: A to Z", "Sort: On first"];

/// The filter chips of the Mods grid (`FILTERS` in tools/menukit.py): All, On, HUD, Visual, Mechanic, Favorites.
pub const FILTER_CHIPS: usize = 6;

/// `[3,0,7]`: the cards the grid shows, as indices into its entries.
fn order_json(o: &mut String, order: &[usize]) {
    o.push('[');
    for (n, i) in order.iter().enumerate() {
        if n > 0 {
            o.push(',');
        }
        push_u(o, *i as u64);
    }
    o.push(']');
}

/// `"bindings":{...}`: which page is open (`#tab_N_on` lights the sidebar item and shows the
/// page), which dropdown of the element picker shows (`#grp_N`), which of the Style page's per-element cards show
/// (`#sec_keys`, `#sec_counter`, `#sec_cps`, `#sec_border`, `#sec_border_keys`, `#sec_chunk`, `#sec_status`, `#sec_speed`, `#sec_facing`, `#sec_reach`, `#sec_sat`, `#sec_stopwatch`, `#sec_playtime`), the opposite of `#sec_keys` (`#sec_other`, for the
/// cards that are drawn taller while Keystrokes is edited), and the window or the move bar.
fn tab_bindings(o: &mut String, tab: usize, selected: usize, moving: bool, bar_hidden: bool, flags: Flags<'_>, dd: Dd) {
    o.push_str("\"bindings\":{");
    // No blur or dimming while HUD elements are dragged, so the layout is seen as it is.
    o.push_str("\"#world_blur\":");
    push_u(o, if moving { 0 } else { flags.blur.round().clamp(0.0, 32.0) as u64 });
    o.push_str(",\"#world_brightness\":");
    o.push_str(&if moving { "1".to_owned() } else { flags.backdrop.0.clamp(0.0, 2.0).to_string() });
    o.push_str(",\"#world_saturation\":");
    o.push_str(&if moving { "1".to_owned() } else { flags.backdrop.1.clamp(0.0, 2.0).to_string() });
    o.push(',');
    for t in 0..TABS.len() {
        if t > 0 {
            o.push(',');
        }
        o.push_str("\"#tab_");
        push_u(o, t as u64);
        o.push_str("_on\":");
        o.push_str(if t == tab { "true" } else { "false" });
    }
    // The Style page's cards for one element's own options.
    let (keys, cps, fps) =
        (selected == Element::Keystrokes.index(), selected == Element::Cps.index(), selected == Element::Fps.index());
    // Which of the picker's element dropdowns shows (a dropdown holds at most eight options, so
    // there is one for each group), and the Style page's armor cards in place of its usual ones.
    let in_group = Element::from_index(selected).group_position().0;
    // The Crosshair page (the live preview binds about seventy names) publishes only what its document uses: the host
    // refuses a surface with too many bindings ("surface has too many bindings").
    let lean = GROWTH.iter().any(|g| g.0 == tab);
    for g in 0..if lean { 0 } else { Element::GROUPS.len() } {
        o.push_str(",\"#grp_");
        push_u(o, g as u64);
        o.push_str(if g == in_group { "\":true" } else { "\":false" });
    }
    // Armor status has no box: the Layout page leaves its box rows out for it.
    let armor = selected == Element::Armor.index();
    let arrows = [Element::Ping, Element::Speed, Element::Reach, Element::Combo, Element::ItemCounter].iter().any(|e| selected == e.index());
    if !lean {
        o.push_str(if flags.name_empty { ",\"#hint_cfg_name\":true" } else { ",\"#hint_cfg_name\":false" });
        o.push_str(if flags.paste_empty { ",\"#hint_cfg_in\":true" } else { ",\"#hint_cfg_in\":false" });
        o.push_str(if flags.quick_empty { ",\"#hint_gp_wp_quick\":true" } else { ",\"#hint_gp_wp_quick\":false" });
        o.push_str(if flags.search_empty { ",\"#hint_module_search\":true" } else { ",\"#hint_module_search\":false" });
        // The Waypoints page: which rows show and their colours, and the faded hints of its fields.
        let none = crate::waypoint_ui::WpUi::default();
        let wp = flags.wp.unwrap_or(&none);
        for n in 0..crate::waypoint_ui::ROWS {
            o.push_str(",\"#wp_row_");
            push_u(o, n as u64);
            o.push_str(if n < wp.rows.len() { "\":true" } else { "\":false" });
            o.push_str(",\"#wp_c");
            push_u(o, n as u64);
            o.push_str("\":[");
            let [r, g, b] = crate::waypoints::rgb(wp.rows.get(n).map_or(0, |row| row.color));
            for (k, c) in [r, g, b, 1.0].into_iter().enumerate() {
                if k > 0 {
                    o.push(',');
                }
                num(o, f64::from(c));
            }
            o.push(']');
        }
        for (id, empty) in [("wp_name", wp.name.is_empty()), ("wp_x", wp.x.is_empty()), ("wp_y", wp.y.is_empty()), ("wp_z", wp.z.is_empty()), ("wp_group", wp.group.is_empty())] {
            o.push_str(",\"#hint_");
            o.push_str(id);
            o.push_str(if empty { "\":true" } else { "\":false" });
        }
        o.push_str(if flags.background { ",\"#bg_on\":true,\"#bg_off\":false" } else { ",\"#bg_on\":false,\"#bg_off\":true" });
        o.push_str(if flags.border { ",\"#border_on\":true,\"#border_off\":false" } else { ",\"#border_on\":false,\"#border_off\":true" });
        o.push_str(if flags.hit_custom { ",\"#hit_custom\":true,\"#hit_other\":false" } else { ",\"#hit_custom\":false,\"#hit_other\":true" });
        o.push_str(if flags.dots { ",\"#xh_dot\":true,\"#xh_nodot\":false" } else { ",\"#xh_dot\":false,\"#xh_nodot\":true" });
        o.push_str(if flags.lines { ",\"#xh_lines\":true,\"#xh_other\":false" } else { ",\"#xh_lines\":false,\"#xh_other\":true" });
        for n in 0..FILTER_CHIPS {
            o.push_str(",\"#flt_");
            push_u(o, n as u64);
            o.push_str(if n == flags.filter { "_on\":true" } else { "_on\":false" });
        }
        // The line under the grid, and the note while the search finds nothing.
        o.push_str(",\"#count_text\":");
        q(o, &format!("{} of {} enabled", flags.enabled, MODS.len()));
        o.push_str(",\"#sort_text\":");
        q(o, SORTS[flags.sort.min(SORTS.len() - 1)]);
        o.push_str(if flags.order.is_empty() { ",\"#empty\":true" } else { ",\"#empty\":false" });
    }
    for n in 0..crate::setups::SLOTS {
        o.push_str(",\"#slot_");
        push_u(o, n as u64);
        o.push_str(if n == flags.slot { "_on\":true" } else { "_on\":false" });
    }
    growth_bindings(o, tab, flags);
    if !lean {
        o.push_str(if armor { ",\"#sec_armor\":true" } else { ",\"#sec_armor\":false" });
        o.push_str(if armor { ",\"#sec_box\":false,\"#sec_nobox\":true" } else { ",\"#sec_box\":true,\"#sec_nobox\":false" });
        o.push_str(if keys { ",\"#sec_keys\":true" } else { ",\"#sec_keys\":false" });
        o.push_str(if keys { ",\"#sec_other\":false" } else { ",\"#sec_other\":true" });
        o.push_str(if cps || fps || arrows { ",\"#sec_counter\":true" } else { ",\"#sec_counter\":false" });
        o.push_str(if cps { ",\"#sec_cps\":true" } else { ",\"#sec_cps\":false" });
        // The Border card sits lower for Keystrokes, whose Box card is taller.
        let bordered = crate::config::Settings::has_border(Element::from_index(selected));
        o.push_str(if bordered && !keys { ",\"#sec_border\":true" } else { ",\"#sec_border\":false" });
        o.push_str(if bordered && keys { ",\"#sec_border_keys\":true" } else { ",\"#sec_border_keys\":false" });
        o.push_str(if selected == Element::Chunk.index() { ",\"#sec_chunk\":true" } else { ",\"#sec_chunk\":false" });
        let status = selected == Element::Slime.index() || selected == Element::Monument.index();
        o.push_str(if status { ",\"#sec_status\":true" } else { ",\"#sec_status\":false" });
        // Speed, Reach, Stopwatch and Playtime each have a card of their own options.
        o.push_str(if selected == Element::Speed.index() { ",\"#sec_speed\":true" } else { ",\"#sec_speed\":false" });
        o.push_str(if selected == Element::Facing.index() { ",\"#sec_facing\":true" } else { ",\"#sec_facing\":false" });
        o.push_str(if selected == Element::Clock.index() { ",\"#sec_clock\":true" } else { ",\"#sec_clock\":false" });
        o.push_str(if selected == Element::Reach.index() { ",\"#sec_reach\":true" } else { ",\"#sec_reach\":false" });
        o.push_str(if selected == Element::Food.index() { ",\"#sec_sat\":true" } else { ",\"#sec_sat\":false" });
        o.push_str(if selected == Element::Coords.index() { ",\"#sec_coords\":true" } else { ",\"#sec_coords\":false" });
        o.push_str(if selected == Element::Potion.index() { ",\"#sec_potion\":true" } else { ",\"#sec_potion\":false" });
        // Item counter's item, and Array list's brackets.
        o.push_str(if selected == Element::ItemCounter.index() { ",\"#sec_itemcounter\":true" } else { ",\"#sec_itemcounter\":false" });
        o.push_str(if selected == Element::ArrayList.index() { ",\"#sec_arraylist\":true" } else { ",\"#sec_arraylist\":false" });
        o.push_str(if selected == Element::Scoreboard.index() { ",\"#sec_scoreboard\":true" } else { ",\"#sec_scoreboard\":false" });
        o.push_str(if selected == Element::Waila.index() { ",\"#sec_waila\":true" } else { ",\"#sec_waila\":false" });
        o.push_str(if selected == Element::ViewAngles.index() { ",\"#sec_va\":true" } else { ",\"#sec_va\":false" });
        o.push_str(if selected == Element::Inventory.index() { ",\"#sec_inv\":true" } else { ",\"#sec_inv\":false" });
        o.push_str(if selected == Element::PvpInfo.index() { ",\"#sec_pvp\":true" } else { ",\"#sec_pvp\":false" });
        o.push_str(if selected == Element::Stopwatch.index() { ",\"#sec_stopwatch\":true" } else { ",\"#sec_stopwatch\":false" });
        o.push_str(if selected == Element::Playtime.index() { ",\"#sec_playtime\":true" } else { ",\"#sec_playtime\":false" });
    }
    // The dropdown list: drawn while `shown`, faded and slid in by `#dd_a` and `#dd_o` (eased by the
    // host, see `transitions` in `build`), its highlight on the chosen row, and the rows it has.
    o.push_str(if dd.shown { ",\"#dd_v\":true,\"#dd_a\":" } else { ",\"#dd_v\":false,\"#dd_a\":" });
    o.push_str(if dd.open { "1" } else { "0" });
    o.push_str(",\"#dd_pos\":[");
    num(o, f64::from(dd.pos[0]));
    o.push(',');
    num(o, f64::from(dd.pos[1]));
    o.push_str("],\"#dd_o\":[0,");
    let slide = if dd.open { 0.0 } else if dd.up { DD_SLIDE } else { -DD_SLIDE };
    num(o, f64::from(slide));
    // The card's height follows the rows the open list has (see `dropdown` in menukit.py).
    let height = 2.0 * DD_PAD + dd.rows.max(1) as f32 * DD_ROW;
    o.push_str("],\"#dd_h\":");
    num(o, f64::from(height));
    o.push_str(",\"#dd_gh\":");
    num(o, f64::from(height - 14.0));
    o.push_str(",\"#dd_bot\":[1,");
    num(o, f64::from(height - 13.0));
    o.push_str("],\"#dd_sel\":[");
    num(o, f64::from(DD_PAD));
    o.push(',');
    // The chosen option's highlight, out of sight while it is scrolled away.
    let at = dd.selected as f32 - dd.first as f32;
    let in_view = at >= 0.0 && at < dd.rows as f32;
    num(o, if in_view { f64::from(DD_PAD + at * DD_ROW) } else { f64::from(DD_PAD) });
    o.push_str(if in_view { "],\"#dd_sel_v\":true" } else { "],\"#dd_sel_v\":false" });
    // The list's small scroll bar, shown only while there are more options than rows.
    let long = dd.total > dd.rows && dd.rows > 0;
    let track = dd.rows as f32 * DD_ROW;
    let thumb = if long { (track * dd.rows as f32 / dd.total as f32).max(8.0) } else { 0.0 };
    let travel = (track - thumb).max(0.0);
    let last = dd.total.saturating_sub(dd.rows).max(1) as f32;
    o.push_str(if long { ",\"#dd_bar_v\":true,\"#dd_thumb_h\":" } else { ",\"#dd_bar_v\":false,\"#dd_thumb_h\":" });
    num(o, f64::from(thumb.max(1.0)));
    o.push_str(",\"#dd_thumb_o\":[");
    num(o, 80.5);
    o.push(',');
    num(o, f64::from(DD_PAD + travel * dd.first as f32 / last));
    o.push(']');
    for i in 0..DD_ROWS {
        o.push_str(",\"#dd_v");
        push_u(o, i as u64);
        o.push_str(if i < dd.rows { "\":true" } else { "\":false" });
        // Hover shows only while the list is open, so it never lingers while the list fades.
        o.push_str(",\"#dd_hv");
        push_u(o, i as u64);
        o.push_str(if dd.open && i < dd.rows { "\":true" } else { "\":false" });
    }
    // The open page's cards that grow with a flag: their heights for the flag as it is now.
    for (document, name, x, y, base, th, kind) in TAILS {
        if document != document_index(tab, selected) {
            continue;
        }
        let shown = th * flags.tail(kind);
        let height = base + shown;
        // The rows' clip box is `REVEAL_PAD` taller than the rows (menukit.py); shut it holds only empty space.
        for (part, first) in [("h", height), ("gh", height - 14.0), ("clip", if shown > 0.0 { shown + REVEAL_PAD } else { 0.5 })] {
            o.push_str(",\"#tl_");
            o.push_str(name);
            o.push('_');
            o.push_str(part);
            o.push_str("\":");
            num(o, f64::from(first));
        }
        o.push_str(",\"#tl_");
        o.push_str(name);
        o.push_str("_bot\":[");
        num(o, f64::from(x + 1.0));
        o.push(',');
        num(o, f64::from(y + height - 13.0));
        o.push(']');
    }
    for (document, name, th, kind) in SHIFTS {
        if document != document_index(tab, selected) {
            continue;
        }
        let hidden = th * (1.0 - flags.tail(kind));
        o.push_str(",\"#tl_");
        o.push_str(name);
        o.push_str("\":[0,");
        num(o, -f64::from(hidden));
        o.push(']');
    }
    for (name, pop) in [("cap", dd.cap), ("ed", dd.ed), ("cp", dd.cp), ("cf", dd.cf), ("toast", dd.toast)] {
        o.push_str(",\"#");
        o.push_str(name);
        o.push_str(if pop.shown { "_v\":true,\"#" } else { "_v\":false,\"#" });
        o.push_str(name);
        o.push_str(if pop.open { "_a\":1,\"#" } else { "_a\":0,\"#" });
        o.push_str(name);
        o.push_str(if pop.open { "_o\":[0,0]" } else { "_o\":[0,-6]" });
    }
    o.push_str(",\"#ed_pos\":[");
    num(o, f64::from(dd.ed_pos[0]));
    o.push(',');
    num(o, f64::from(dd.ed_pos[1]));
    o.push_str("],\"#cp_pos\":[");
    num(o, f64::from(dd.ed_pos[0]));
    o.push(',');
    num(o, f64::from(dd.ed_pos[1]));
    o.push(']');
    for (name, colours) in [("r", dd.recent), ("f", dd.saved)] {
        for (n, c) in colours.iter().enumerate() {
            o.push_str(",\"#cp_");
            o.push_str(name);
            push_u(o, n as u64);
            o.push_str("\":[");
            for (i, v) in c.iter().enumerate() {
                if i > 0 {
                    o.push(',');
                }
                num(o, f64::from(*v));
            }
            o.push(']');
        }
    }
    let [h, s, v] = dd.hsv;
    let hue = hsv_rgb(h, 1.0, 1.0);
    o.push_str(",\"#cp_hue\":[");
    for c in hue {
        num(o, f64::from(c));
        o.push(',');
    }
    o.push_str("1],\"#cp_sv\":[");
    num(o, f64::from(PICKER_SV[0] + s.clamp(0.0, 1.0) * PICKER_SV[2] - 3.5));
    o.push(',');
    num(o, f64::from(PICKER_SV[1] + (1.0 - v.clamp(0.0, 1.0)) * PICKER_SV[3] - 3.5));
    o.push_str("],\"#cp_hu\":[");
    num(o, f64::from(PICKER_HUE[0] + h.clamp(0.0, 1.0) * PICKER_HUE[2] - 1.5));
    o.push(',');
    num(o, f64::from(PICKER_HUE[1] - 2.0));
    o.push(']');
    // The opacity strip: its slices fade the picked colour out, and a marker shows where the opacity sits.
    for i in 0..PICKER_OP_SLICES {
        o.push_str(",\"#cp_o");
        push_u(o, i as u64);
        o.push_str("\":[");
        for c in dd.swatch {
            num(o, f64::from(c));
            o.push(',');
        }
        num(o, 1.0 - (i as f64 + 0.5) / PICKER_OP_SLICES as f64);
        o.push(']');
    }
    o.push_str(",\"#cp_ot\":[");
    num(o, f64::from(PICKER_OP[0] - 2.0));
    o.push(',');
    num(o, f64::from(PICKER_OP[1] + dd.transparency.min(100) as f32 / 100.0 * PICKER_OP[3] - 1.5));
    o.push_str(if dd.hex { "],\"#ed_hex\":true,\"#ed_num\":false,\"#ed_sw\":[" } else { "],\"#ed_hex\":false,\"#ed_num\":true,\"#ed_sw\":[" });
    for (n, c) in dd.swatch.iter().enumerate() {
        if n > 0 {
            o.push(',');
        }
        num(o, f64::from(*c));
    }
    o.push_str(",1],\"#scroll_off\":[0,");
    num(o, -f64::from(dd.bar.scroll));
    o.push_str("],\"#stick_off\":[0,");
    num(o, f64::from(dd.bar.scroll));
    o.push_str("],\"#thumb_off\":[0,");
    num(o, f64::from(dd.bar.thumb));
    o.push_str("],\"#thumb_h\":");
    num(o, f64::from(dd.bar.thumb_len));
    o.push_str(if dd.bar.visible { ",\"#bar_v\":true" } else { ",\"#bar_v\":false" });
    o.push_str(if dd.bar.hot { ",\"#thumb_hot\":1" } else { ",\"#thumb_hot\":0" });
    if MOVE {
        o.push_str(if moving { ",\"#v_window\":false,\"#v_move\":true" } else { ",\"#v_window\":true,\"#v_move\":false" });
        // The move bar's opacity; the host eases it (see `transitions` in `build`).
        o.push_str(if bar_hidden { ",\"#move_alpha\":0" } else { ",\"#move_alpha\":1" });
    }
    o.push('}');
}

/// The surface's screen name, as a panel bundle names its route.
pub const ROUTE: &str = SCREEN;

fn control(o: &mut String, kind: &str, id: &str, label: &str) {
    o.push_str("{\"kind\":\"");
    o.push_str(kind);
    o.push_str("\",\"id\":");
    q(o, id);
    o.push_str(",\"label\":");
    q(o, label);
}

/// Whether `bytes` needs no escaping in a JSON string: no control character, quote or backslash.
/// Looks at eight bytes at a time (a whole menu is thousands of label bytes, and one byte at a time
/// cost a good part of a page's fuel); the answer is the same as checking every byte.
fn plain(bytes: &[u8]) -> bool {
    const ONES: u64 = 0x0101_0101_0101_0101;
    const HIGH: u64 = 0x8080_8080_8080_8080;
    // Each test sets a byte's high bit exactly when some byte matches (for the word as a whole):
    // a byte below 0x20, or a byte equal to '"' or '\\' (a zero byte after the XOR).
    let flagged = |w: u64| {
        let below = w.wrapping_sub(ONES * 0x20) & !w;
        let quote = w ^ (ONES * u64::from(b'"'));
        let slash = w ^ (ONES * u64::from(b'\\'));
        (below | (quote.wrapping_sub(ONES) & !quote) | (slash.wrapping_sub(ONES) & !slash)) & HIGH != 0
    };
    let mut chunks = bytes.chunks_exact(8);
    for chunk in &mut chunks {
        let mut word = [0u8; 8];
        word.copy_from_slice(chunk);
        if flagged(u64::from_le_bytes(word)) {
            return false;
        }
    }
    chunks.remainder().iter().all(|&b| b >= 0x20 && b != b'"' && b != b'\\')
}

/// A JSON string.
pub fn q(o: &mut String, s: &str) {
    o.push('"');
    // Nearly every string here is a plain label or option name. One cheap scan and one copy
    // replace the per-character loop, which cost most of a frame's fuel over a whole menu.
    if plain(s.as_bytes()) {
        o.push_str(s);
    } else {
        for ch in s.chars() {
            match ch {
                '"' => o.push_str("\\\""),
                '\\' => o.push_str("\\\\"),
                c if (c as u32) < 0x20 => o.push(' '),
                c => o.push(c),
            }
        }
    }
    o.push('"');
}

pub fn push_u(o: &mut String, mut n: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    for b in &buf[i..] {
        o.push(*b as char);
    }
}

/// A number with at most three decimals, without float formatting machinery.
pub fn num(o: &mut String, v: f64) {
    let v = if v.is_finite() { v } else { 0.0 };
    let scaled = (v.abs() * 1000.0).round() as u64;
    if v < 0.0 && scaled != 0 {
        o.push('-');
    }
    push_u(o, scaled / 1000);
    let frac = scaled % 1000;
    if frac != 0 {
        o.push('.');
        let digits = [frac / 100, frac / 10 % 10, frac % 10];
        let last = if frac % 100 == 0 { 1 } else if frac % 10 == 0 { 2 } else { 3 };
        for d in &digits[..last] {
            o.push((b'0' + *d as u8) as char);
        }
    }
}

/// How the window of a page that grows is sized (`#xw` and what hangs off its right edge), and the
/// preview's bindings. The window is `WIDTH` wide, plus the room for the preview while the crosshair
/// is on; the host eases these (see `build`), so the whole window grows when the crosshair is switched on.
fn growth_bindings(o: &mut String, tab: usize, flags: Flags<'_>) {
    if GROWTH.is_empty() {
        return;
    }
    // Pages without the preview are the usual width.
    let (c, grow, card) = match (GROWTH.iter().find(|g| g.0 == tab), flags.crosshair) {
        (Some(&(_, grow, card)), Some(c)) => (Some(c), grow, card),
        _ => (None, 0, 0),
    };
    let on = c.is_some_and(|c| c.enabled);
    let (wide, shown) = if on { (WIDTH + grow, card) } else { (WIDTH, 0) };
    o.push_str(",\"#xw\":");
    push_u(o, u64::from(wide));
    o.push_str(",\"#xw2\":");
    push_u(o, u64::from(wide - 2));
    o.push_str(",\"#xclose\":[");
    push_u(o, u64::from(wide - 26));
    o.push_str(",7],\"#xrule\":");
    push_u(o, u64::from(wide - WIDTH / 2));
    if let Some(c) = c {
        o.push_str(",\"#xp_w\":");
        push_u(o, u64::from(shown));
        // Always published, so that nothing in the card is ever unbound (it is clipped away while shut).
        preview_bindings(o, c, (card.saturating_sub(16) / 2) as f32);
    }
}

fn rgba(o: &mut String, c: [f32; 4]) {
    o.push('[');
    for (i, v) in c.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        num(o, f64::from(*v));
    }
    o.push(']');
}

/// `"#pv_<name>_<part>":value`.
fn pv(o: &mut String, name: &str, part: &str) {
    o.push_str(",\"#pv_");
    o.push_str(name);
    o.push('_');
    o.push_str(part);
    o.push_str("\":");
}

/// The corner radii (in box pixels) the preview's rectangles can have: the menu draws each one
/// with a fixed radius, and every rectangle of one kind (the bars, or their outlines) shows in the
/// set that comes nearest to the radius they want. (One flag per set, not per rectangle: a page can
/// bind only so many names.)
/// The most rectangles a look has: four arms (a centre dot is a disc).
pub const PREVIEW_RECTS: usize = 4;
pub const PREVIEW_RADII: [f32; 4] = [0.0, 2.0, 4.0, 7.0];

/// One rectangle of the preview: where it is and how big. One that is not wanted sits far outside
/// the box, which clips it. (The host only takes numbers, vectors, text and flags as bindings.)
fn rect_node(o: &mut String, name: &str, r: Option<[f32; 4]>) {
    let [x, y, w, h] = r.unwrap_or([-100.0, -100.0, 1.0, 1.0]);
    pv(o, name, "o");
    o.push('[');
    num(o, f64::from(x));
    o.push(',');
    num(o, f64::from(y));
    o.push(']');
    pv(o, name, "w");
    num(o, f64::from(w));
    pv(o, name, "h");
    num(o, f64::from(h));
}

/// Which corner set of `kind` ("f" for the bars, "e" for the outlines) shows: the one nearest the
/// radius the rectangles want, as far as the shortest of them has room for.
fn radius_sets(o: &mut String, kind: &str, rects: &[[f32; 4]], radii: &[f32]) {
    let want = radii.first().copied().unwrap_or(0.0);
    let room = rects.iter().map(|r| r[2].min(r[3]) / 2.0 + 0.5).fold(f32::MAX, f32::min);
    let mut best = 0;
    for (k, level) in PREVIEW_RADII.iter().enumerate() {
        if *level <= room && (level - want).abs() < (PREVIEW_RADII[best] - want).abs() {
            best = k;
        }
    }
    for k in 0..PREVIEW_RADII.len() {
        pv(o, &format!("{kind}l{k}"), "v");
        o.push_str(if !rects.is_empty() && k == best { "true" } else { "false" });
    }
}

/// The ring of the preview: a fixed unit circle in the menu, seen through a view box that is moved
/// and scaled so the circle lands where `ring` is, with the stroke in the unit circle's units.
fn ring_node(o: &mut String, name: &str, ring: Option<(Ring, [f32; 2])>, side: f32) {
    let (r, at) = ring.map_or((Ring { radius: 1.0, width: 1.0 }, [0.0, 0.0]), |(r, at)| (r, at));
    let radius = r.radius.max(0.5);
    pv(o, name, "vb");
    o.push('[');
    num(o, f64::from(-at[0] / radius));
    o.push(',');
    num(o, f64::from(-at[1] / radius));
    o.push(',');
    num(o, f64::from(side / radius));
    o.push(',');
    num(o, f64::from(side / radius));
    o.push(']');
    pv(o, name, "sw");
    num(o, f64::from(r.width / radius));
    pv(o, name, "v");
    o.push_str(if ring.is_some() { "true" } else { "false" });
}

/// The preview of the crosshair at rest, drawn in a box `half * 2` across (`preview_card` in menukit.py).
fn preview_bindings(o: &mut String, c: &CrosshairSettings, half: f32) {
    let frame = c.frame(Target::None, false);
    let look = c.look(false, false);
    let outline = if c.outline && frame.outline[3] > 0.0 { crosshair::snap(c.outline_width as f32, 1) } else { 0.0 };
    let p = crosshair::preview_with(&look, outline, c.outline_round, half);
    o.push_str(",\"#pv_name\":");
    q(o, crosshair::SHAPES[c.shape.min(crosshair::SHAPES.len() - 1)]);
    o.push_str(",\"#pv_fc\":");
    rgba(o, frame.fill);
    o.push_str(",\"#pv_ec\":");
    rgba(o, frame.outline);
    for i in 0..PREVIEW_RECTS {
        rect_node(o, &format!("f{i}"), p.fill.get(i).copied());
        rect_node(o, &format!("e{i}"), p.edges.get(i).copied());
    }
    radius_sets(o, "f", &p.fill, &p.fill_radius);
    radius_sets(o, "e", &p.edges, &p.edge_radius);
    let middle = [half, half];
    ring_node(o, "re", p.ring_edge.map(|r| (r, middle)), half * 2.0);
    ring_node(o, "rf", p.ring.map(|r| (r, middle)), half * 2.0);
    ring_node(o, "de", p.disc_edge.map(|r| (r, p.disc_at)), half * 2.0);
    ring_node(o, "df", p.disc.map(|r| (r, p.disc_at)), half * 2.0);
}

#[cfg(test)]
mod plain_tests {
    use super::plain;

    fn slow(bytes: &[u8]) -> bool {
        bytes.iter().all(|&b| b >= 0x20 && b != b'"' && b != b'\\')
    }

    #[test]
    fn the_word_check_agrees_with_the_byte_check() {
        // Every byte value at every place of strings that span whole words and a remainder.
        for len in 0..26 {
            for at in 0..len {
                for b in 0..=255u8 {
                    let mut bytes = vec![b'a'; len];
                    bytes[at] = b;
                    assert_eq!(plain(&bytes), slow(&bytes), "len {len} at {at} byte {b}");
                }
            }
        }
        // Mixed text, including UTF-8 (bytes of 0x80 and over).
        let mut seed = 0x1234_5678_u32;
        for _ in 0..20_000 {
            let len = (seed % 40) as usize;
            let bytes: Vec<u8> = (0..len)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    [b'a', b' ', b'"', b'\\', 0x1f, 0x20, 0x7f, 0x80, 0xc3, 0xff, b'0', 0x00][(seed % 12) as usize]
                })
                .collect();
            assert_eq!(plain(&bytes), slow(&bytes), "{bytes:?}");
        }
    }
}

#[cfg(test)]
mod control_limit_tests {
    use crate::menu_controls::{CONTROLS, DOC_CONTROLS};

    #[test]
    fn every_page_fits_the_hosts_control_limit() {
        for (n, controls) in DOC_CONTROLS.iter().enumerate() {
            assert!(controls.len() <= 512, "document {n} has {} controls", controls.len());
            assert!(controls.iter().all(|g| usize::from(*g) < CONTROLS.len()));
            assert!(controls.windows(2).all(|w| w[0] < w[1]), "document {n} lists controls out of order");
        }
    }
}
