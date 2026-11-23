//! The colour picker: a card over a page with a saturation and brightness square, a hue strip, an
//! opacity strip, a hex field, saved colours, recent colours and presets.
//!
//! Everything is drawn for the colour the card is given, so a change republishes the card. The square
//! drags two sliders at once, the strips drag one each, and the hex field opens the host's text editor.

use serde_json::{Value, json};

use crate::node::{self, Align};
use crate::tokens::{Palette, RADII, Rgba, space, text, px};

/// The ids the card presses, which the page's owner answers.
pub const CLOSE: &str = "ed_no";
pub const APPLY: &str = "ed_ok";
pub const SAVE: &str = "cp_star";
pub const SAT: &str = "pk_x";
pub const VALUE: &str = "pk_y";
pub const HUE: &str = "pk_hue";
pub const OPACITY: &str = "pk_op";
pub const HEX: &str = "hex_in";

/// The card's size in design units.
const WIDTH: f32 = 300.0;
const PAD: f32 = 16.0;
const SQUARE: [f32; 2] = [180.0, 112.0];
const STRIP: f32 = 14.0;
const SWATCH: [f32; 2] = [26.0, 18.0];
const SWATCH_GAP: f32 = 4.0;
/// How many presets share a row.
const PRESETS_PER_ROW: usize = 7;

/// What the card shows.
#[derive(Clone, Copy, Debug)]
pub struct Picker<'a> {
    /// The setting being picked, as the card's title.
    pub title: &'a str,
    /// Hue, saturation and brightness, each from 0 to 1.
    pub hsv: [f32; 3],
    /// The opacity in percent, or `None` for a colour with no opacity of its own.
    pub opacity: Option<u32>,
    /// The six hex digits in the field.
    pub hex: &'a str,
    /// Whether the colour is among the saved ones.
    pub saved: bool,
    /// Recently applied colours, saved colours and presets, as 24-bit colours.
    pub recent: &'a [u32],
    pub favorites: &'a [u32],
    pub presets: &'a [u32],
    /// Converts a hue, saturation and brightness to red, green and blue (0 to 1).
    pub to_rgb: fn(f32, f32, f32) -> [f32; 3],
}

/// The ids of the swatch buttons: `cp_r<n>` recent, `cp_f<n>` saved, `cp_p<n>` preset.
fn swatch_id(kind: char, n: usize) -> String {
    format!("cp_{kind}{n}")
}

fn rgb24(rgb: u32) -> Rgba {
    [(rgb >> 16 & 255) as f32 / 255.0, (rgb >> 8 & 255) as f32 / 255.0, (rgb & 255) as f32 / 255.0, 1.0]
}

fn line(style: crate::tokens::TextStyle) -> f32 {
    px(style.size) * 1.6
}

fn scale(style: crate::tokens::TextStyle) -> f32 {
    px(style.size) / 9.0
}

/// The controls the card declares, in the order [`controls`] numbers them from `first`: the close
/// button behind it, the three sliders and the opacity strip's, the hex field, Apply, Save, then the swatches.
#[must_use]
pub fn declared(picker: &Picker<'_>) -> Vec<Value> {
    let slider = |id: &str, value: f32, max: f32, step: f32| {
        json!({"kind": "slider", "id": id, "label": id, "value": value, "min": 0.0, "max": max, "step": step})
    };
    let button = |id: &str, label: &str| json!({"kind": "button", "id": id, "label": label});
    let [h, s, v] = picker.hsv;
    let mut out = vec![
        button(CLOSE, "Close"),
        slider(SAT, s, 1.0, 0.005),
        // The vertical axis grows downwards, so it holds the brightness the other way round.
        slider(VALUE, 1.0 - v, 1.0, 0.005),
        slider(HUE, h, 1.0, 0.002),
        slider(OPACITY, picker.opacity.unwrap_or(100) as f32, 100.0, 1.0),
        json!({"kind": "text", "id": HEX, "label": "Hex", "value": picker.hex, "min_chars": 0, "max_chars": 16, "live": true}),
        button(APPLY, "Apply"),
        button(SAVE, "Save"),
    ];
    for (kind, list) in [('r', picker.recent), ('f', picker.favorites), ('p', picker.presets)] {
        for n in 0..list.len() {
            out.push(button(&swatch_id(kind, n), "Colour"));
        }
    }
    out
}

/// A vertical fade.
fn fade_down(name: &str, at: [f32; 2], size: [f32; 2], from: Rgba, to: Rgba) -> Value {
    let mut control = node::fade(name, at, size, from, to);
    control[name]["gradient_direction"] = json!("vertical");
    control
}

/// The card over `area` (GUI pixels), its controls numbered from `first`.
#[must_use]
pub fn controls(p: &Palette, picker: &Picker<'_>, area: [f32; 4], first: usize) -> Vec<Value> {
    let [ax, ay, aw, ah] = area;
    let rows = 1 + usize::from(!picker.favorites.is_empty()) + usize::from(!picker.recent.is_empty());
    let preset_rows = picker.presets.len().div_ceil(PRESETS_PER_ROW);
    let swatch_h = SWATCH[1] + SWATCH_GAP;
    let strips = if picker.opacity.is_some() { 2.0 } else { 1.0 };
    let top = 44.0;
    let strips_y = top + SQUARE[1] + 10.0;
    let swatches_y = strips_y + strips * (STRIP + 8.0) + 6.0;
    let swatch_rows = (rows - 1) as f32 + preset_rows as f32;
    let height = (swatches_y + swatch_rows * swatch_h + 56.0).max(0.0);
    let (w, h) = (px(WIDTH).min(aw - px(space::XL * 2.0)), px(height).min(ah - px(space::XL * 2.0)));
    let (x, y) = (ax + (aw - w) / 2.0, ay + (ah - h) / 2.0);
    // A point inside the card, in design units from its top left.
    let at = |dx: f32, dy: f32| [px(dx), px(dy)];
    let [hue, sat, val] = picker.hsv;
    let hue_rgb = (picker.to_rgb)(hue, 1.0, 1.0);
    let rgb = (picker.to_rgb)(hue, sat, val);
    let opaque = |c: [f32; 3]| [c[0], c[1], c[2], 1.0];
    let mut c: Vec<Value> = Vec::new();

    // The square: white to the hue left to right, then clear to black top to bottom.
    c.push(node::rounded("sv_base", at(PAD, top), at(SQUARE[0], SQUARE[1]), px(RADII.xs), opaque(hue_rgb)));
    c.push(node::fade("sv_white", at(PAD, top), at(SQUARE[0], SQUARE[1]), [1.0, 1.0, 1.0, 1.0], [hue_rgb[0], hue_rgb[1], hue_rgb[2], 0.0]));
    c.push(fade_down("sv_black", at(PAD, top), at(SQUARE[0], SQUARE[1]), [0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]));
    c.push(node::outline("sv_edge", at(PAD, top), at(SQUARE[0], SQUARE[1]), px(RADII.xs), 1.0, p.border));
    let marker = px(9.0);
    let mx = px(PAD) + px(SQUARE[0]) * sat - marker / 2.0;
    let my = px(top) + px(SQUARE[1]) * (1.0 - val) - marker / 2.0;
    c.push(node::outline("sv_mark_dark", [mx, my], [marker, marker], marker / 2.0, 2.0, [0.0, 0.0, 0.0, 1.0]));
    c.push(node::outline("sv_mark", [mx + 1.0, my + 1.0], [marker - 2.0, marker - 2.0], marker / 2.0, 1.0, [1.0, 1.0, 1.0, 1.0]));
    c.push(node::button(
        "sv_drag",
        at(PAD, top),
        at(SQUARE[0], SQUARE[1]),
        &format!("mod.vector:{}:{}", first + 1, first + 2),
        vec![],
    ));

    // The hue strip: six fades through the colour wheel.
    let stops = [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 0.0, 0.0]];
    let seg = SQUARE[0] / 6.0;
    for (i, pair) in stops.windows(2).enumerate() {
        c.push(node::fade(
            &format!("hue_{i}"),
            at(PAD + seg * i as f32, strips_y),
            at(seg + 0.5, STRIP),
            opaque(pair[0]),
            opaque(pair[1]),
        ));
    }
    c.push(node::outline("hue_edge", at(PAD, strips_y), at(SQUARE[0], STRIP), px(RADII.xs), 1.0, p.border));
    c.push(node::rounded("hue_mark", [px(PAD) + px(SQUARE[0]) * hue - px(2.0), px(strips_y) - px(2.0)], [px(4.0), px(STRIP + 4.0)], px(1.0), [1.0, 1.0, 1.0, 1.0]));
    c.push(node::button("hue_drag", at(PAD, strips_y), at(SQUARE[0], STRIP), &format!("mod.axis:{}:x", first + 3), vec![]));

    // The opacity strip: clear to the colour.
    if let Some(opacity) = picker.opacity {
        let oy = strips_y + STRIP + 8.0;
        c.push(node::rounded("op_back", at(PAD, oy), at(SQUARE[0], STRIP), px(RADII.xs), p.dim));
        c.push(node::fade("op_fade", at(PAD, oy), at(SQUARE[0], STRIP), [rgb[0], rgb[1], rgb[2], 0.0], opaque(rgb)));
        c.push(node::outline("op_edge", at(PAD, oy), at(SQUARE[0], STRIP), px(RADII.xs), 1.0, p.border));
        let frac = opacity as f32 / 100.0;
        c.push(node::rounded("op_mark", [px(PAD) + px(SQUARE[0]) * frac - px(2.0), px(oy) - px(2.0)], [px(4.0), px(STRIP + 4.0)], px(1.0), [1.0, 1.0, 1.0, 1.0]));
        c.push(node::button("op_drag", at(PAD, oy), at(SQUARE[0], STRIP), &format!("mod.axis:{}:x", first + 4), vec![]));
    }

    // The right column: the colour, its hex field and Save.
    let col = PAD + SQUARE[0] + 14.0;
    let col_w = WIDTH - col - PAD;
    c.push(node::rounded("preview", at(col, top), at(col_w, 40.0), px(RADII.xs), opaque(rgb)));
    c.push(node::outline("preview_edge", at(col, top), at(col_w, 40.0), px(RADII.xs), 1.0, p.border));
    let field_h = 26.0;
    c.push(node::button(
        "hex",
        at(col, top + 50.0),
        at(col_w, field_h),
        &format!("mod.edit:{}", first + 5),
        vec![
            node::rounded("fill", [0.0, 0.0], at(col_w, field_h), px(RADII.xs), p.field),
            node::outline("edge", [0.0, 0.0], at(col_w, field_h), px(RADII.xs), 1.0, p.border),
            node::label(
                "text",
                picker.hex,
                [0.0, (px(field_h) - line(text::SMALL)) / 2.0],
                [px(col_w), line(text::SMALL)],
                p.text,
                scale(text::SMALL),
                Align::Center,
            ),
        ],
    ));
    let (fill, ink, label) = if picker.saved { (p.accent, p.on_accent, "Saved") } else { (p.raised, p.text, "Save") };
    c.push(node::button(
        "save",
        at(col, top + 50.0 + field_h + 8.0),
        at(col_w, field_h),
        &format!("mod.control:{}", first + 7),
        vec![
            node::rounded("fill", [0.0, 0.0], at(col_w, field_h), px(RADII.xs), fill),
            node::label(
                "text",
                label,
                [0.0, (px(field_h) - line(text::SMALL)) / 2.0],
                [px(col_w), line(text::SMALL)],
                ink,
                scale(text::SMALL),
                Align::Center,
            ),
        ],
    ));

    // Swatch rows: recent, saved, then the presets, numbered in the order they were declared.
    let mut row_y = swatches_y;
    let recent_base = first + 8;
    let favorite_base = recent_base + picker.recent.len();
    let preset_base = favorite_base + picker.favorites.len();
    let mut place = |kind: char, list: &[u32], base: usize, per_row: usize, row_y: &mut f32, c: &mut Vec<Value>| {
        for (n, rgb) in list.iter().enumerate() {
            let (col, row) = (n % per_row, n / per_row);
            let sx = PAD + col as f32 * (SWATCH[0] + SWATCH_GAP);
            let sy = *row_y + row as f32 * swatch_h;
            c.push(node::button(
                &swatch_id(kind, n),
                at(sx, sy),
                at(SWATCH[0], SWATCH[1]),
                &format!("mod.control:{}", base + n),
                vec![
                    node::rounded("fill", [0.0, 0.0], at(SWATCH[0], SWATCH[1]), px(RADII.xs), rgb24(*rgb)),
                    node::outline("edge", [0.0, 0.0], at(SWATCH[0], SWATCH[1]), px(RADII.xs), 1.0, p.border),
                ],
            ));
        }
        *row_y += list.len().div_ceil(per_row).max(1) as f32 * swatch_h;
    };
    if !picker.recent.is_empty() {
        place('r', picker.recent, recent_base, 6, &mut row_y, &mut c);
    }
    if !picker.favorites.is_empty() {
        place('f', picker.favorites, favorite_base, 6, &mut row_y, &mut c);
    }
    place('p', picker.presets, preset_base, PRESETS_PER_ROW, &mut row_y, &mut c);

    // Apply and Cancel along the bottom.
    let by = height - 40.0;
    let bw = 92.0;
    c.push(node::button(
        "apply",
        at(WIDTH - PAD - bw, by),
        at(bw, 26.0),
        &format!("mod.control:{}", first + 6),
        vec![
            node::rounded("fill", [0.0, 0.0], at(bw, 26.0), px(RADII.xs), p.accent),
            node::label(
                "text",
                "Apply",
                [0.0, (px(26.0) - line(text::SMALL)) / 2.0],
                [px(bw), line(text::SMALL)],
                p.on_accent,
                scale(text::SMALL),
                Align::Center,
            ),
        ],
    ));
    c.push(node::button(
        "cancel",
        at(WIDTH - PAD - bw * 2.0 - 8.0, by),
        at(bw, 26.0),
        &format!("mod.control:{first}"),
        vec![
            node::rounded("fill", [0.0, 0.0], at(bw, 26.0), px(RADII.xs), p.raised),
            node::label(
                "text",
                "Cancel",
                [0.0, (px(26.0) - line(text::SMALL)) / 2.0],
                [px(bw), line(text::SMALL)],
                p.text,
                scale(text::SMALL),
                Align::Center,
            ),
        ],
    ));

    vec![
        // Everything behind the card dims, and a press there cancels the picker.
        node::button(
            "picker_dim",
            [ax, ay],
            [aw, ah],
            &format!("mod.control:{first}"),
            vec![node::rounded("dim", [0.0, 0.0], [aw, ah], px(RADII.md), [0.0, 0.0, 0.0, 0.55])],
        ),
        node::rounded("picker_card", [x, y], [w, h], px(RADII.md), p.raised),
        node::outline("picker_edge", [x, y], [w, h], px(RADII.md), 1.0, p.border),
        node::label(
            "picker_title",
            picker.title,
            [x + px(PAD), y + px(10.0)],
            [w - px(PAD * 2.0), line(text::HEADING)],
            p.highlight,
            scale(text::HEADING),
            Align::Left,
        ),
        node::panel("picker_body", [x, y], [w, h], c),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
        [h * s, v, 0.0]
    }

    fn picker<'a>(recent: &'a [u32], favorites: &'a [u32], presets: &'a [u32], opacity: Option<u32>) -> Picker<'a> {
        Picker { title: "Colour", hsv: [0.5, 0.5, 0.5], opacity, hex: "336699", saved: false, recent, favorites, presets, to_rgb: rgb }
    }

    fn presses(value: &Value, out: &mut Vec<String>) {
        let node = node::body(value);
        if let Some(press) = node["button_mappings"][0]["to_button_id"].as_str() {
            out.push(press.to_owned());
        }
        for child in node["controls"].as_array().into_iter().flatten() {
            presses(child, out);
        }
    }

    #[test]
    fn every_press_names_a_declared_control_and_every_swatch_is_pressed() {
        let (recent, saved, presets) = ([0x112233, 0x445566], [0x778899], [1u32, 2, 3, 4, 5, 6, 7, 8, 9]);
        let p = picker(&recent, &saved, &presets, Some(80));
        let declared = declared(&p);
        let first = 20;
        let mut found = Vec::new();
        for control in controls(&Palette::dark(), &p, [0.0, 0.0, 600.0, 400.0], first) {
            presses(&control, &mut found);
        }
        for press in &found {
            let number = |prefix: &str| press.strip_prefix(prefix).and_then(|n| n.split(':').next()?.parse::<usize>().ok());
            let index = number("mod.control:").or_else(|| number("mod.axis:")).or_else(|| number("mod.edit:")).or_else(|| number("mod.vector:"));
            let index = index.unwrap_or_else(|| panic!("{press} is not a control press"));
            assert!(index >= first && index - first < declared.len(), "{press} is outside the declared controls");
        }
        // Close, Apply, Save, and the swatches (2 + 1 + 9) are all pressable.
        let controls_pressed = found.iter().filter_map(|p| p.strip_prefix("mod.control:")).count();
        assert!(controls_pressed >= 3 + recent.len() + saved.len() + presets.len());
    }

    #[test]
    fn the_declared_controls_match_the_swatches_in_number_and_order() {
        let p = picker(&[1, 2], &[3], &[4, 5, 6], None);
        let ids: Vec<String> = declared(&p).iter().map(|c| c["id"].as_str().unwrap().to_owned()).collect();
        assert_eq!(&ids[8..], ["cp_r0", "cp_r1", "cp_f0", "cp_p0", "cp_p1", "cp_p2"]);
        assert_eq!(&ids[..8], [CLOSE, SAT, VALUE, HUE, OPACITY, HEX, APPLY, SAVE]);
    }

    #[test]
    fn a_colour_without_opacity_draws_no_opacity_strip() {
        let none = controls(&Palette::dark(), &picker(&[], &[], &[], None), [0.0, 0.0, 600.0, 400.0], 0);
        let some = controls(&Palette::dark(), &picker(&[], &[], &[], Some(100)), [0.0, 0.0, 600.0, 400.0], 0);
        assert!(!serde_json::to_string(&none).unwrap().contains("op_drag"));
        assert!(serde_json::to_string(&some).unwrap().contains("op_drag"));
    }
}
