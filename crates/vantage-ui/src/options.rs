//! Option rows: the one layout every settings page is made of.
//!
//! A page is a list of [`Group`]s of [`Opt`]s. Each option is a row with its name (and optional help)
//! on the left and its control on the right; a changed option shows a Reset button. Groups
//! have a heading that collapses them. The host provides every control's live value through
//! `#row_<n>_*` bindings, so the rows are static documents: only layout changes (collapsing,
//! searching) rebuild one.
//!
//! Each option takes consecutive declared controls, in this order for every group: the group's
//! collapse button, then for each option its control (none for [`Kind::Info`]) and its Reset
//! button. Indices stay the same whether or not a group is collapsed.

use serde_json::{Value, json};

use crate::node::{self, Align};
use crate::tokens::{Palette, RADII, TextStyle, px, space, text};

/// What an option edits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind<'a> {
    Toggle { on: bool },
    Slider { value: f32, min: f32, max: f32, step: f32 },
    /// One of a few named values; pressing cycles to the next. The button's label is the current one.
    Choice { options: &'a [&'a str], selected: usize },
    Keybind { key: &'a str },
    Text { value: &'a str },
    /// An action.
    Button { label: &'a str },
    /// Read-only text.
    Info,
}

impl Kind<'_> {
    /// Whether the option has a control of its own.
    fn has_control(&self) -> bool {
        !matches!(self, Self::Info)
    }
}

/// One option.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opt<'a> {
    /// A stable identifier: letters, digits, `_`, `-` and `.`.
    pub id: &'a str,
    pub label: &'a str,
    /// A line of explanation under the name.
    pub help: Option<&'a str>,
    pub kind: Kind<'a>,
    /// Whether the value differs from its default, so the row offers Reset.
    pub changed: bool,
}

/// A titled, collapsible run of options.
#[derive(Clone, Copy, Debug)]
pub struct Group<'a> {
    pub title: &'a str,
    pub collapsed: bool,
    pub options: &'a [Opt<'a>],
}

/// A group reduced to the options a search kept.
#[derive(Clone, Debug)]
pub struct Matched<'a> {
    pub title: &'a str,
    pub collapsed: bool,
    pub options: Vec<Opt<'a>>,
}

/// The groups' options that have every word of `query` in their name or help, in any case. A search
/// drops groups with no match and shows the rest expanded; with no words the groups are kept as given.
#[must_use]
pub fn search<'a>(groups: &[Group<'a>], query: &str) -> Vec<Matched<'a>> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    groups
        .iter()
        .filter_map(|group| {
            let options: Vec<Opt<'a>> = group
                .options
                .iter()
                .copied()
                .filter(|opt| {
                    let haystack = format!("{} {}", opt.label, opt.help.unwrap_or("")).to_lowercase();
                    words.iter().all(|word| haystack.contains(word))
                })
                .collect();
            if words.is_empty() {
                return Some(Matched { title: group.title, collapsed: group.collapsed, options });
            }
            (!options.is_empty()).then_some(Matched { title: group.title, collapsed: false, options })
        })
        .collect()
}

/// The matched groups as [`Group`]s, for [`rows`] and [`declared`].
#[must_use]
pub fn groups_of<'a>(matched: &'a [Matched<'a>]) -> Vec<Group<'a>> {
    matched
        .iter()
        .map(|m| Group { title: m.title, collapsed: m.collapsed, options: &m.options })
        .collect()
}

// ---- Geometry (design units) ------------------------------------------------------------------

/// A row's height without and with a help line.
const ROW: f32 = 40.0;
const ROW_HELP: f32 = 52.0;
/// A group heading's height.
const HEADING: f32 = 36.0;
/// The space above each group after the first.
const GROUP_GAP: f32 = space::L;
/// Control sizes.
const TOGGLE: [f32; 2] = [44.0, 24.0];
const SLIDER: f32 = 160.0;
const PILL: [f32; 2] = [124.0, 26.0];
const KEY: [f32; 2] = [90.0, 26.0];
const FIELD: [f32; 2] = [150.0, 26.0];
const BUTTON: [f32; 2] = [110.0, 26.0];
const VALUE_WIDTH: f32 = 48.0;
/// The Reset button's size and its gap from the control.
const RESET: [f32; 2] = [46.0, 18.0];

fn row_height(opt: &Opt<'_>) -> f32 {
    if opt.help.is_some() { ROW_HELP } else { ROW }
}

fn scale(style: TextStyle) -> f32 {
    px(style.size) / 9.0
}

fn line(style: TextStyle) -> f32 {
    px(style.size) * 1.6
}

/// The binding name that shows an option's Reset button.
#[must_use]
pub fn changed_binding(id: &str) -> String {
    let safe: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("#opt_changed_{safe}")
}

/// The number of declared controls the groups take.
#[must_use]
pub fn control_count(groups: &[Group<'_>]) -> usize {
    groups
        .iter()
        .map(|group| {
            1 + group
                .options
                .iter()
                .map(|opt| if opt.kind.has_control() { 2 } else { 0 })
                .sum::<usize>()
        })
        .sum()
}

/// The declared controls, in the order [`rows`] numbers them (from `first`).
#[must_use]
pub fn declared(groups: &[Group<'_>]) -> Vec<Value> {
    let mut out = Vec::new();
    for (g, group) in groups.iter().enumerate() {
        out.push(json!({"kind": "button", "id": format!("group_{g}"), "label": group.title}));
        for opt in group.options {
            match opt.kind {
                Kind::Info => continue,
                Kind::Toggle { on } => out.push(json!({
                    "kind": "toggle", "id": opt.id, "label": opt.label, "value": on,
                })),
                Kind::Slider { value, min, max, step } => out.push(json!({
                    "kind": "slider", "id": opt.id, "label": opt.label,
                    "value": value, "min": min, "max": max, "step": step,
                })),
                Kind::Choice { options, selected } => out.push(json!({
                    "kind": "button", "id": opt.id,
                    "label": options.get(selected).copied().unwrap_or(""),
                })),
                Kind::Keybind { key } => out.push(json!({
                    "kind": "keybind", "id": opt.id, "label": opt.label, "key": key,
                })),
                Kind::Text { value } => out.push(json!({
                    "kind": "text", "id": opt.id, "label": opt.label, "value": value,
                })),
                Kind::Button { label } => out.push(json!({
                    "kind": "button", "id": opt.id, "label": label,
                })),
            }
            out.push(json!({"kind": "button", "id": format!("{}_reset", opt.id), "label": "Reset"}));
        }
    }
    out
}

/// The rows laid out: their controls (offsets from the content's top left) and the content height.
#[derive(Clone, Debug)]
pub struct Rows {
    pub controls: Vec<Value>,
    /// In GUI pixels.
    pub height: f32,
}

/// Lays the groups out in a column `width` GUI pixels wide, numbering their controls from `first`.
#[must_use]
pub fn rows(p: &Palette, groups: &[Group<'_>], width: f32, first: usize) -> Rows {
    let mut controls = Vec::new();
    let mut y = 0.0;
    let mut index = first;
    for (g, group) in groups.iter().enumerate() {
        if g > 0 {
            y += px(GROUP_GAP);
        }
        // Heading: a button that collapses the group.
        let chevron = if group.collapsed { ">" } else { "v" };
        controls.push(node::button(
            &format!("group_{g}"),
            [0.0, y],
            [width, px(HEADING)],
            &format!("mod.control:{index}"),
            vec![
                node::label(
                    "title",
                    &group.title.to_uppercase(),
                    [0.0, (px(HEADING) - line(text::EYEBROW)) / 2.0],
                    [width - px(space::XXL), line(text::EYEBROW)],
                    p.muted,
                    scale(text::EYEBROW),
                    Align::Left,
                ),
                node::label(
                    "chevron",
                    chevron,
                    [width - px(space::XXL), (px(HEADING) - line(text::SMALL)) / 2.0],
                    [px(space::XXL), line(text::SMALL)],
                    p.muted,
                    scale(text::SMALL),
                    Align::Center,
                ),
            ],
        ));
        index += 1;
        y += px(HEADING);
        for opt in group.options {
            let own = index;
            if opt.kind.has_control() {
                index += 2;
            }
            if group.collapsed {
                continue;
            }
            let height = row_height(opt);
            controls.extend(row(p, opt, own, y, width, height));
            y += px(height);
        }
    }
    Rows { controls, height: y + px(space::XL) }
}

/// One option's controls at vertical offset `y`, with its control at declared index `own`.
fn row(p: &Palette, opt: &Opt<'_>, own: usize, y: f32, width: f32, height: f32) -> Vec<Value> {
    let h = px(height);
    let mut out = vec![node::rounded(
        &format!("rule_{own}"),
        [0.0, y + h - 1.0],
        [width, 1.0],
        0.0,
        p.border,
    )];
    // Name and help.
    let name_y = if opt.help.is_some() { px(7.0) } else { (h - line(text::LABEL)) / 2.0 };
    out.push(node::label(
        &format!("name_{own}"),
        opt.label,
        [0.0, y + name_y],
        [width * 0.55, line(text::LABEL)],
        p.text,
        scale(text::LABEL),
        Align::Left,
    ));
    if let Some(help) = opt.help {
        out.push(node::label(
            &format!("help_{own}"),
            help,
            [0.0, y + px(26.0)],
            [width * 0.6, line(text::CAPTION)],
            p.muted,
            scale(text::CAPTION),
            Align::Left,
        ));
    }
    if !opt.kind.has_control() {
        return out;
    }

    // The control, right-aligned.
    let [cw, ch] = control_size(&opt.kind);
    let x = width - px(cw);
    let cy = y + (h - px(ch)) / 2.0;
    let reset = own + 1;
    let press = format!("mod.control:{own}");
    let control: Vec<Value> = match opt.kind {
        Kind::Toggle { .. } => toggle(p, own, [x, cy], &press),
        Kind::Slider { .. } => slider(p, own, [x, cy]),
        Kind::Choice { .. } => pill(p, [x, cy], [px(cw), px(ch)], &press, &format!("#row_{own}_label"), true),
        Kind::Keybind { .. } => pill(p, [x, cy], [px(cw), px(ch)], &press, &format!("#row_{own}_value"), true),
        Kind::Text { .. } => field(p, own, [x, cy]),
        Kind::Button { .. } => action(p, own, [x, cy], &press),
        Kind::Info => Vec::new(),
    };
    out.extend(control);
    // Reset, only while the option differs from its default.
    let binding = changed_binding(opt.id);
    out.push(node::bound(
        node::button(
            &format!("reset_{own}"),
            [x - px(RESET[0] + space::M), y + (h - px(RESET[1])) / 2.0],
            [px(RESET[0]), px(RESET[1])],
            &format!("mod.control:{reset}"),
            vec![node::label(
                "text",
                "Reset",
                [0.0, (px(RESET[1]) - line(text::CAPTION)) / 2.0],
                [px(RESET[0]), line(text::CAPTION)],
                p.text,
                scale(text::CAPTION),
                Align::Center,
            )],
        ),
        json!([{"binding_name": binding, "binding_name_override": "#visible"}]),
    ));
    out
}

/// A control's size in design units.
fn control_size(kind: &Kind<'_>) -> [f32; 2] {
    match kind {
        Kind::Toggle { .. } => TOGGLE,
        Kind::Slider { .. } => [SLIDER + VALUE_WIDTH + space::M, 26.0],
        Kind::Choice { .. } => PILL,
        Kind::Keybind { .. } => KEY,
        Kind::Text { .. } => FIELD,
        Kind::Button { .. } => BUTTON,
        Kind::Info => [0.0, 0.0],
    }
}

/// A switch: a pill that fills with the accent, and a knob the host slides.
fn toggle(p: &Palette, n: usize, at: [f32; 2], press: &str) -> Vec<Value> {
    let size = [px(TOGGLE[0]), px(TOGGLE[1])];
    let knob = px(16.0);
    vec![
        node::rounded(&format!("track_{n}"), at, size, size[1] / 2.0, p.dim),
        node::bound(
            node::rounded(&format!("on_{n}"), at, size, size[1] / 2.0, p.accent),
            json!([{"binding_name": format!("#row_{n}_toggle_progress"), "binding_name_override": "#alpha"}]),
        ),
        // The host slides the knob inside a box over the track: its offset is from that box.
        node::panel(
            &format!("knobs_{n}"),
            at,
            size,
            vec![node::bound(
                node::rounded(&format!("knob_{n}"), [0.0, 0.0], [knob, knob], knob / 2.0, p.highlight),
                json!([{"binding_name": format!("#row_{n}_knob_offset"), "binding_name_override": "#offset"}]),
            )],
        ),
        node::button(&format!("hit_{n}"), at, size, press, vec![]),
    ]
}

/// A slider: a track with an accent fill, dragged from anywhere on it, and its value, which opens
/// the host's value editor.
fn slider(p: &Palette, n: usize, at: [f32; 2]) -> Vec<Value> {
    let track = px(SLIDER);
    let bar = px(6.0);
    let top = at[1] + (px(26.0) - bar) / 2.0;
    vec![
        node::rounded(&format!("track_{n}"), [at[0], top], [track, bar], bar / 2.0, p.dim),
        node::bound(
            node::rounded(&format!("fill_{n}"), [at[0], top], [track, bar], bar / 2.0, p.accent),
            json!([{
                "binding_name": format!("(#row_{n}_fill * {track})"),
                "binding_name_override": "#size_binding_x_absolute",
            }]),
        ),
        node::button(
            &format!("drag_{n}"),
            [at[0], at[1]],
            [track, px(26.0)],
            &format!("mod.axis:{n}:x"),
            vec![],
        ),
        node::bound_label(
            &format!("value_{n}"),
            &format!("#row_{n}_value"),
            [at[0] + track + px(space::M), at[1] + (px(26.0) - line(text::SMALL)) / 2.0],
            [px(VALUE_WIDTH), line(text::SMALL)],
            p.text,
            scale(text::SMALL),
            Align::Right,
        ),
        node::button(
            &format!("edit_{n}"),
            [at[0] + track + px(space::M), at[1]],
            [px(VALUE_WIDTH), px(26.0)],
            &format!("mod.edit:{n}"),
            vec![],
        ),
    ]
}

/// A pill-shaped button showing the host binding `shown`.
fn pill(p: &Palette, at: [f32; 2], size: [f32; 2], press: &str, shown: &str, outlined: bool) -> Vec<Value> {
    let _ = outlined;
    vec![node::button(
        &format!("pill_{}", shown.trim_start_matches('#')),
        at,
        size,
        press,
        vec![
            node::rounded("fill", [0.0, 0.0], size, px(RADII.xs), p.raised),
            node::outline("edge", [0.0, 0.0], size, px(RADII.xs), 1.0, p.border),
            node::bound_label(
                "text",
                shown,
                [0.0, (size[1] - line(text::SMALL)) / 2.0],
                [size[0], line(text::SMALL)],
                p.text,
                scale(text::SMALL),
                Align::Center,
            ),
        ],
    )]
}

/// A text field that opens the host's editor.
fn field(p: &Palette, n: usize, at: [f32; 2]) -> Vec<Value> {
    let size = [px(FIELD[0]), px(FIELD[1])];
    vec![node::button(
        &format!("field_{n}"),
        at,
        size,
        &format!("mod.edit:{n}"),
        vec![
            node::rounded("fill", [0.0, 0.0], size, px(RADII.xs), p.field),
            node::outline("edge", [0.0, 0.0], size, px(RADII.xs), 1.0, p.border),
            node::bound_label(
                "text",
                &format!("#row_{n}_value"),
                [px(space::M), (size[1] - line(text::SMALL)) / 2.0],
                [size[0] - px(space::L), line(text::SMALL)],
                p.text,
                scale(text::SMALL),
                Align::Left,
            ),
        ],
    )]
}

/// An action button with its own label.
fn action(p: &Palette, n: usize, at: [f32; 2], press: &str) -> Vec<Value> {
    let size = [px(BUTTON[0]), px(BUTTON[1])];
    vec![node::button(
        &format!("action_{n}"),
        at,
        size,
        press,
        vec![
            node::rounded("fill", [0.0, 0.0], size, px(RADII.xs), p.accent),
            node::bound_label(
                "text",
                &format!("#row_{n}_label"),
                [0.0, (size[1] - line(text::SMALL)) / 2.0],
                [size[0], line(text::SMALL)],
                p.on_accent,
                scale(text::SMALL),
                Align::Center,
            ),
        ],
    )]
}

/// The surface bindings the rows read: which options show Reset.
#[must_use]
pub fn surface_bindings(groups: &[Group<'_>]) -> Value {
    let mut map = serde_json::Map::new();
    for group in groups {
        for opt in group.options {
            map.insert(changed_binding(opt.id), json!(opt.changed));
        }
    }
    Value::Object(map)
}

/// A list of every value of one choice, drawn over the page.
#[derive(Clone, Copy, Debug)]
pub struct Popup<'a> {
    /// The option's name, shown as the list's title.
    pub title: &'a str,
    pub options: &'a [&'a str],
    pub selected: usize,
}

/// The id a list's values press: `pick_<n>`; and the one that closes it.
pub const PICK_CLOSE: &str = "pick_close";

/// The controls a list declares: its close button, then one button per value.
#[must_use]
pub fn popup_declared(popup: &Popup<'_>) -> Vec<Value> {
    let mut out = vec![json!({"kind": "button", "id": PICK_CLOSE, "label": "Close"})];
    for (n, name) in popup.options.iter().enumerate() {
        out.push(json!({"kind": "button", "id": format!("pick_{n}"), "label": *name}));
    }
    out
}

/// The list over `area` (GUI pixels): a dimmer that closes it, a card with the title and the values.
#[must_use]
pub fn popup_controls(p: &Palette, popup: &Popup<'_>, area: [f32; 4], first: usize) -> Vec<Value> {
    let [ax, ay, aw, ah] = area;
    let row = px(32.0);
    let width = px(260.0).min(aw - px(space::XL * 2.0));
    let title_h = px(44.0);
    let want = title_h + row * popup.options.len() as f32 + px(space::M);
    let height = want.min(ah - px(space::XL * 2.0));
    let (x, y) = (ax + (aw - width) / 2.0, ay + (ah - height) / 2.0);
    let list_h = height - title_h - px(space::M);
    let content_h = row * popup.options.len() as f32;
    let buttons: Vec<Value> = popup
        .options
        .iter()
        .enumerate()
        .map(|(n, name)| {
            let on = n == popup.selected;
            node::button(
                &format!("opt_{n}"),
                [0.0, row * n as f32],
                [width - px(space::XL * 2.0) - 8.0, row - px(2.0)],
                &format!("mod.control:{}", first + 1 + n),
                vec![
                    node::rounded(
                        "fill",
                        [0.0, 0.0],
                        [width - px(space::XL * 2.0) - 8.0, row - px(2.0)],
                        px(RADII.xs),
                        if on { p.accent } else { p.dim },
                    ),
                    node::label(
                        "text",
                        name,
                        [px(space::M), (row - px(2.0) - line(text::SMALL)) / 2.0],
                        [width - px(space::XL * 2.0) - 8.0 - px(space::XL), line(text::SMALL)],
                        if on { p.on_accent } else { p.text },
                        scale(text::SMALL),
                        Align::Left,
                    ),
                ],
            )
        })
        .collect();
    vec![
        // Everything behind the card dims, and a press there closes the list.
        node::button(
            "pick_dim",
            [ax, ay],
            [aw, ah],
            &format!("mod.control:{first}"),
            vec![node::rounded("dim", [0.0, 0.0], [aw, ah], px(RADII.md), [0.0, 0.0, 0.0, 0.55])],
        ),
        node::rounded("pick_card", [x, y], [width, height], px(RADII.md), p.raised),
        node::outline("pick_edge", [x, y], [width, height], px(RADII.md), 1.0, p.border),
        node::label(
            "pick_title",
            popup.title,
            [x + px(space::XL), y + (title_h - line(text::HEADING)) / 2.0],
            [width - px(space::XL * 2.0), line(text::HEADING)],
            p.highlight,
            scale(text::HEADING),
            Align::Left,
        ),
        node::scroll_view(
            "pick_list",
            [x + px(space::XL), y + title_h],
            [width - px(space::XL * 2.0), list_h],
            buttons,
            content_h,
        ),
    ]
}

/// The settings page: the shell with the groups as scrolling rows in its pane.
#[must_use]
pub fn panel(shell_state: &crate::shell::Shell<'_>, groups: &[Group<'_>], popup: Option<&Popup<'_>>) -> Value {
    let p = shell_state.palette;
    let shell_declared = crate::shell::declared_controls(shell_state);
    let first = shell_declared.len();
    let [x, y, w, h] = crate::shell::content_rect();
    let pad = px(space::XL);
    let (view_w, view_h) = (w - pad * 2.0, h - pad * 2.0);
    let laid = rows(&p, groups, view_w - 8.0, first);
    let mut controls = crate::shell::controls(shell_state);
    controls.push(node::scroll_view(
        "rows",
        [x + pad, y + pad],
        [view_w, view_h],
        laid.controls,
        laid.height.max(view_h),
    ));
    let mut declared = shell_declared;
    declared.extend(self::declared(groups));
    if let Some(popup) = popup {
        // The list's controls follow every row's: its first index is the next one declared.
        let first = declared.len();
        controls.extend(popup_controls(&p, popup, [x, y, w, h], first));
        declared.extend(popup_declared(popup));
    }
    let [ww, wh] = crate::shell::window_px();
    let mut bindings = surface_bindings(groups);
    if let Some(map) = bindings.as_object_mut() {
        map.extend(shell_state.backdrop.bindings());
    }
    json!({
        "reference_size": [ww, wh],
        "controls": declared,
        "surface": {
            "screen": "vantage.root",
            "document": json!({
                "namespace": "vantage",
                "root": {"type": "panel", "size": [ww, wh], "controls": controls},
            }).to_string(),
            "bindings": bindings,
            "transitions": crate::shell::Backdrop::transitions(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHOICES: [&str; 3] = ["Left", "Center", "Right"];

    fn options() -> [Opt<'static>; 6] {
        [
            Opt { id: "show", label: "Show it", help: Some("Draws it on the screen."), kind: Kind::Toggle { on: true }, changed: false },
            Opt { id: "scale", label: "Scale", help: None, kind: Kind::Slider { value: 1.0, min: 0.5, max: 2.0, step: 0.01 }, changed: true },
            Opt { id: "align", label: "Align", help: None, kind: Kind::Choice { options: &CHOICES, selected: 1 }, changed: false },
            Opt { id: "key", label: "Key", help: None, kind: Kind::Keybind { key: "KeyZ" }, changed: false },
            Opt { id: "note", label: "Note", help: None, kind: Kind::Info, changed: false },
            Opt { id: "name", label: "Name", help: None, kind: Kind::Text { value: "x" }, changed: false },
        ]
    }

    fn group<'a>(opts: &'a [Opt<'a>], collapsed: bool) -> Group<'a> {
        Group { title: "Display", collapsed, options: opts }
    }

    #[test]
    fn every_option_with_a_control_takes_two_controls_and_info_takes_none() {
        let opts = options();
        let groups = [group(&opts, false)];
        // The heading, then control + Reset for five options.
        assert_eq!(control_count(&groups), 1 + 5 * 2);
        assert_eq!(declared(&groups).len(), control_count(&groups));
    }

    #[test]
    fn collapsing_a_group_keeps_every_control_index() {
        let opts = options();
        let open = [group(&opts, false)];
        let shut = [group(&opts, true)];
        assert_eq!(control_count(&open), control_count(&shut));
        assert_eq!(declared(&open).len(), declared(&shut).len());
        // The collapsed group draws its heading only.
        let p = Palette::dark();
        assert_eq!(rows(&p, &shut, 300.0, 0).controls.len(), 1);
        assert!(rows(&p, &open, 300.0, 0).controls.len() > 10);
    }

    #[test]
    fn rows_press_only_declared_controls_and_stay_inside_their_column() {
        let opts = options();
        let groups = [group(&opts, false)];
        let first = 4;
        let declared_total = first + control_count(&groups);
        let width = 320.0;
        let laid = rows(&Palette::dark(), &groups, width, first);
        fn visit(value: &Value, declared: usize, first: usize) {
            let node = node::body(value);
            if let Some(press) = node["button_mappings"][0]["to_button_id"].as_str() {
                let n = press
                    .strip_prefix("mod.control:")
                    .or_else(|| press.strip_prefix("mod.edit:"))
                    .or_else(|| press.strip_prefix("mod.axis:").map(|r| r.split(':').next().unwrap()))
                    .map(|n| n.parse::<usize>().unwrap());
                let n = n.unwrap_or_else(|| panic!("{press} is not a control route"));
                assert!(n >= first && n < declared, "{press} is not one of the rows' controls");
            }
        }
        for control in &laid.controls {
            visit(control, declared_total, first);
            let node = node::body(control);
            if let (Some(off), Some(size)) = (node["offset"].as_array(), node["size"].as_array()) {
                let x = off[0].as_f64().unwrap() as f32 + size[0].as_f64().unwrap() as f32;
                assert!(x <= width + 0.01, "{control} spills past the column");
            }
        }
    }

    #[test]
    fn a_slider_fill_is_bound_to_its_controls_fraction() {
        let opts = options();
        let laid = rows(&Palette::dark(), &[group(&opts, false)], 320.0, 0);
        // The slider is the second option: heading 0, toggle 1 and its Reset 2, slider 3.
        let fill = laid.controls.iter().find(|c| node::body(c).get("bindings").is_some_and(|b| b.to_string().contains("#row_3_fill")));
        assert!(fill.is_some(), "no fill bound to #row_3_fill");
    }

    #[test]
    fn reset_shows_only_for_changed_options_through_surface_bindings() {
        let opts = options();
        let bindings = surface_bindings(&[group(&opts, false)]);
        assert_eq!(bindings["#opt_changed_scale"], json!(true));
        assert_eq!(bindings["#opt_changed_show"], json!(false));
    }

    #[test]
    fn search_keeps_matching_options_expanded_and_drops_empty_groups() {
        let opts = options();
        let other = [Opt { id: "x", label: "Opacity", help: None, kind: Kind::Toggle { on: false }, changed: false }];
        let groups = [group(&opts, true), Group { title: "Other", collapsed: false, options: &other }];
        let found = search(&groups, "SHOW screen");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].options.len(), 1);
        assert!(!found[0].collapsed, "a search expands the groups it keeps");
        assert!(search(&groups, "zzz").is_empty());
        let all = search(&groups, "  ");
        assert_eq!(all.len(), 2, "no words, no filtering");
        assert!(all[0].collapsed, "and no change to what is collapsed");
        assert_eq!(groups_of(&all).len(), 2);
    }

    #[test]
    fn a_choice_list_declares_a_close_button_and_a_button_per_value_and_presses_only_those() {
        let names = ["Red", "Green", "Blue"];
        let popup = Popup { title: "Color", options: &names, selected: 1 };
        let declared = popup_declared(&popup);
        assert_eq!(declared.len(), 4);
        assert_eq!(declared[0]["id"], PICK_CLOSE);
        assert_eq!(declared[3]["id"], "pick_2");
        let first = 10;
        let controls = popup_controls(&Palette::dark(), &popup, [0.0, 0.0, 300.0, 200.0], first);
        let mut presses = Vec::new();
        fn collect(value: &Value, presses: &mut Vec<String>) {
            let node = node::body(value);
            if let Some(press) = node["button_mappings"][0]["to_button_id"].as_str() {
                presses.push(press.to_owned());
            }
            if let Some(children) = node["controls"].as_array() {
                for child in children {
                    collect(child, presses);
                }
            }
            if let Some(children) = node["controls"].as_array().and_then(|c| c.first()).and_then(|c| c.as_object()) {
                let _ = children;
            }
        }
        for control in &controls {
            collect(control, &mut presses);
        }
        for n in 0..4 {
            assert!(presses.contains(&format!("mod.control:{}", first + n)), "control {n} is not pressed");
        }
    }

    #[test]
    fn changed_bindings_are_safe_names_whatever_the_id_holds() {
        assert_eq!(changed_binding("hud.fps-scale"), "#opt_changed_hud_fps_scale");
    }
}
