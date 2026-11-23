//! Small renders of a mod's own output, drawn as the icon of its card. Each is built from the same drawing
//! primitives as the rest of the menu, so it keeps the menu's fonts and stays sharp at any size.

use std::sync::OnceLock;

use super::{Align, Painter, Primitive, Radii, Rect, Rgba, px};
use crate::config::Element;
use crate::hud;

/// The game's own font: it follows the player's texture pack.
const GAME_FONT: &str = "default";

/// The element whose switch is mod `id`, if the mod is a HUD element.
fn element_of(id: &str) -> Option<Element> {
    Element::ALL.iter().copied().find(|e| e.toggle_id() == id)
}

/// Every element at its defaults with example numbers, worked out once.
fn sample(element: Element) -> Option<&'static hud::Sample> {
    static SAMPLES: OnceLock<Vec<Option<hud::Sample>>> = OnceLock::new();
    SAMPLES.get_or_init(|| Element::ALL.iter().map(|e| hud::sample(*e)).collect())[element.index()].as_ref()
}

/// How far the HUD is enlarged in a card: every element shares it, as in the game, and only one too wide to fit shrinks.
const PREVIEW_SCALE: f32 = 1.15;

/// A row of text in the game, in unscaled GUI pixels, and the room the text keeps from a box's sides.
const ROW: f32 = 10.0;
const PAD: f32 = 5.0;

/// The game font's size for the HUD's text, in unscaled GUI pixels.
const HUD_TEXT: f32 = 9.0;


/// What a mod's preview shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Look {
    /// A HUD box with these lines of text.
    Text(&'static [&'static str]),
    /// A HUD box of lines pushed to the right edge, like the array list.
    Stack(&'static [&'static str]),
    /// The WASD keys.
    Keys,
    /// Four armour slots with their durability bars.
    Armor,
    /// An inventory grid.
    Inventory,
    /// A crosshair.
    Crosshair,
    /// A potion effect: a swatch and its name and time.
    Effect,
}

/// What mod `id`'s preview shows, for a mod that has one.
pub(super) fn look(id: &str) -> Option<Look> {
    Some(match id {
        "hud_keystrokes" => Look::Keys,
        "hud_cps" => Look::Text(&["12 CPS"]),
        "hud_fps" => Look::Text(&["144 FPS"]),
        "hud_armor" => Look::Armor,
        "hud_coords" => Look::Text(&["X 128", "Y 64", "Z -42"]),
        "hud_potion" => Look::Effect,
        "hud_ping" => Look::Text(&["24 ms"]),
        "ch_on" => Look::Crosshair,
        "hud_facing" => Look::Text(&["North"]),
        "hud_food" => Look::Text(&["Sat 5.0"]),
        "hud_speed" => Look::Text(&["4.3 b/s"]),
        "hud_stopwatch" => Look::Text(&["00:12.4"]),
        "hud_playtime" => Look::Text(&["1h 24m"]),
        "hud_clock" => Look::Text(&["12:30"]),
        "hud_day" => Look::Text(&["Day 12"]),
        "hud_viewangles" => Look::Text(&["Yaw 90", "Pitch 12"]),
        "hud_reach" => Look::Text(&["3.2 m"]),
        "hud_combo" => Look::Text(&["x5"]),
        "hud_potioncount" => Look::Text(&["Potions 12"]),
        "hud_totems" => Look::Text(&["Totems 3"]),
        "hud_itemcounter" => Look::Text(&["Items 64"]),
        "hud_inventory" => Look::Inventory,
        "hud_arraylist" => Look::Stack(&["Fullbright", "Zoom", "Sprint"]),
        "hud_chunk" => Look::Text(&["Chunk 8, 4"]),
        "hud_slime" => Look::Text(&["Slime"]),
        "hud_monument" => Look::Text(&["Guardians"]),
        "hud_scoreboard" => Look::Text(&["Scores", "You 12", "Them 9"]),
        "hud_target" => Look::Text(&["Player", "20 / 20"]),
        "hud_waila" => Look::Text(&["Stone"]),
        "hud_height" => Look::Text(&["Y 320"]),
        "hud_subtitles" => Look::Text(&["Footsteps"]),
        "hud_pvp" => Look::Text(&["Hits 4"]),
        _ => return None,
    })
}

/// The face of a HUD box: dark and see-through, like the real element.
const BOX: Rgba = [0.0, 0.0, 0.0, 0.5];
/// A key, slot or swatch on that box.
const PART: Rgba = [1.0, 1.0, 1.0, 0.16];

/// Draws the element at its defaults as it looks in the game: its own box, colours and text, in the game's font.
fn paint_sample(d: &mut Painter, sample: &hud::Sample, area: Rect) {
    let [bw, bh] = sample.size;
    // One scale for every element, so they keep their sizes relative to each other as in the game; only one too big for
    // its card shrinks to fit.
    let k = (area.w * 0.78 / bw.max(1.0)).min(area.h * 0.8 / bh.max(1.0)).min(PREVIEW_SCALE);
    let r = Rect::new(area.centre().x - bw * k / 2.0, area.centre().y - bh * k / 2.0, bw * k, bh * k);
    let clip = d.at(r);
    d.scene.push(Primitive::PushClip { rect: clip, radii: Radii::default() });
    if sample.background[3] > 0.0 {
        d.rect(r, sample.background, Radii::all(if sample.rounded { 3.0 * k } else { 0.0 }));
    }
    let s = sample.scale.max(0.1);
    let row = ROW * s * k;
    // The rows are centred in the box, and the whole block of them shrinks if its widest line would not fit.
    let line_text = |line: &hud::Line| -> Vec<(String, Rgba)> {
        let mut pieces = Vec::new();
        if let Some((head, color)) = &line.head {
            pieces.push((head.clone(), *color));
        }
        pieces.push((line.text.clone(), line.color));
        if let Some((tail, color)) = &line.tail {
            pieces.push((tail.clone(), *color));
        }
        pieces
    };
    let gap = 3.0 * s * k;
    let line_width = |pieces: &[(String, Rgba)]| pieces.iter().map(|(t, _)| hud::text_width(t) * s * k).sum::<f32>() + gap * pieces.len().saturating_sub(1) as f32;
    let widest = sample.lines.iter().map(|l| line_width(&line_text(l))).fold(0.0_f32, f32::max);
    let fit = if widest > 0.0 { ((r.w - 2.0 * PAD * s * k) / widest).min(1.0) } else { 1.0 };
    let size = HUD_TEXT * s * k * fit;
    let block = row * sample.lines.len() as f32;
    let top = r.y + (r.h - block) / 2.0;
    for (i, line) in sample.lines.iter().enumerate() {
        let y = top + i as f32 * row + (row - size * 1.5) / 2.0;
        let cell = |x: f32, w: f32| Rect::new(x, y, w, size * 1.5);
        let mut put = |d: &mut Painter, rect: Rect, text: &str, color: Rgba, align: Align| {
            if sample.shadow {
                d.text_in(Rect::new(rect.x + 0.6 * s * k, rect.y + 0.6 * s * k, rect.w, rect.h), text, size, [0.0, 0.0, 0.0, 0.55], align, GAME_FONT);
            }
            d.text_in(rect, text, size, color, align, GAME_FONT);
        };
        if let (Some([bx, by, bw, bh]), 0) = (sample.bar, i) {
            // CPS: the separator sits in the middle of the gap, and each count is set against it, so the bar stays
            // between them whatever the font's widths are.
            let (left, right) = line.text.split_once("  ").unwrap_or((line.text.as_str(), ""));
            let lw = hud::text_width(left) * s * k * fit;
            let rw = hud::text_width(right) * s * k * fit;
            let space = 4.0 * s * k * fit;
            let total = lw + space + rw;
            let bar_x = r.x + (r.w - total) / 2.0 + lw + space / 2.0;
            let _ = bx;
            d.rect(Rect::new(bar_x - (bw * k).max(1.0) / 2.0, r.y + by * k * fit.max(0.5), (bw * k).max(1.0), bh * k * fit), line.color, Radii::default());
            put(d, cell(bar_x - space / 2.0 - lw - 2.0, lw + 2.0), left, line.color, Align::End);
            put(d, cell(bar_x + space / 2.0, rw + 4.0 * s * k), right, line.color, Align::Start);
            continue;
        }
        let pieces = line_text(line);
        let widths: Vec<f32> = pieces.iter().map(|(t, _)| hud::text_width(t) * s * k * fit).collect();
        let total = widths.iter().sum::<f32>() + gap * fit * pieces.len().saturating_sub(1) as f32;
        let mut x = r.x + (r.w - total) / 2.0;
        for ((text, color), width) in pieces.iter().zip(widths) {
            put(d, cell(x, width + 4.0 * s * k), text, *color, Align::Start);
            x += width + gap * fit;
        }
    }
    d.scene.push(Primitive::PopClip);
}

/// Draws the Keystrokes element at its defaults: its keys at their real places.
fn paint_keys(d: &mut Painter, area: Rect) {
    let (keys, idle) = hud::sample_keys();
    let right = keys.iter().map(|k| k.rect[0] + k.rect[2]).max().unwrap_or(1) as f32;
    let bottom = keys.iter().map(|k| k.rect[1] + k.rect[3]).max().unwrap_or(1) as f32;
    let k = (area.w * 0.62 / right).min(area.h * 0.9 / bottom).min(PREVIEW_SCALE);
    let (x0, y0) = (area.centre().x - right * k / 2.0, area.centre().y - bottom * k / 2.0);
    for key in keys {
        let [x, y, w, h] = key.rect.map(|v| v as f32 * k);
        let r = Rect::new(x0 + x, y0 + y, w, h);
        d.rect(r, idle, Radii::all(2.5 * k));
        let size = HUD_TEXT * k;
        if key.name == "Space" {
            // A short bar across the middle of the key, like the game draws it.
            let bar = Rect::new(r.x + r.w / 4.0, r.y + r.h / 2.0 - 0.5 * k, r.w / 2.0, k.max(1.0));
            d.rect(bar, [1.0, 1.0, 1.0, 1.0], Radii::default());
            continue;
        }
        d.text_in(Rect::new(r.x, r.y + (r.h - size * 1.5) / 2.0, r.w, size * 1.5), &key.label, size, [1.0, 1.0, 1.0, 1.0], Align::Centre, GAME_FONT);
    }
}

/// Draws mod `id`'s preview centred in `area` in colour `ink`; false when the mod has none.
pub(super) fn paint(d: &mut Painter, id: &str, area: Rect, ink: Rgba) -> bool {
    if let Some(element) = element_of(id) {
        if element == Element::Keystrokes {
            paint_keys(d, area);
            return true;
        }
        if let Some(sample) = sample(element) {
            paint_sample(d, sample, area);
            return true;
        }
    }
    if id == "ch_on" {
        // The crosshair as it is drawn at its default settings, at the size it has in the game.
        let shapes = crate::menu_v2::crosshair_shapes(&crate::config::Settings::default());
        let side = px(crate::menu_v2::PREVIEW_BOX);
        let k = (area.h * 0.9 / side).min(1.0);
        d.preview_shapes(area.centre().x - side * k / 2.0, area.centre().y - side * k / 2.0, k, &shapes);
        return true;
    }
    let Some(look) = look(id) else { return false };
    let c = area.centre();
    let size = px(14.0);
    match look {
        Look::Text(lines) | Look::Stack(lines) => {
            let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f32;
            let w = (longest * size * 0.72 + px(18.0)).min(area.w - px(10.0));
            let line_h = size * 1.3;
            let h = lines.len() as f32 * line_h + px(10.0);
            let r = Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h);
            d.rect(r, BOX, Radii::all(px(8.0)));
            for (i, line) in lines.iter().enumerate() {
                let row = Rect::new(r.x + px(8.0), r.y + px(5.0) + i as f32 * line_h, r.w - px(16.0), line_h);
                let align = if matches!(look, Look::Stack(_)) { Align::End } else { Align::Start };
                d.line(row, line, size, ink, align);
            }
        }
        Look::Keys => {
            let key = px(26.0);
            let gap = px(4.0);
            let top = c.y - key - gap / 2.0;
            let mid = c.x - key / 2.0;
            for (letter, x, y) in [("W", mid, top), ("A", mid - key - gap, top + key + gap), ("S", mid, top + key + gap), ("D", mid + key + gap, top + key + gap)] {
                let r = Rect::new(x, y, key, key);
                d.rect(r, BOX, Radii::all(px(6.0)));
                d.ring(r, Radii::all(px(6.0)), 0.3, [1.0, 1.0, 1.0, 0.22]);
                d.line(r, letter, size, ink, Align::Centre);
            }
        }
        Look::Armor => {
            let slot = px(26.0);
            let gap = px(5.0);
            let x0 = c.x - (4.0 * slot + 3.0 * gap) / 2.0;
            for (i, wear) in [0.9_f32, 0.7, 0.5, 0.8].iter().enumerate() {
                let r = Rect::new(x0 + i as f32 * (slot + gap), c.y - slot / 2.0 - px(3.0), slot, slot);
                d.rect(r, BOX, Radii::all(px(5.0)));
                d.ring(r, Radii::all(px(5.0)), 0.3, [1.0, 1.0, 1.0, 0.22]);
                d.rect(Rect::new(r.x + px(5.0), r.y + px(6.0), slot - px(10.0), slot - px(16.0)), PART, Radii::all(px(3.0)));
                let bar = Rect::new(r.x + px(4.0), r.bottom() + px(2.0), slot - px(8.0), px(3.0));
                d.rect(bar, [1.0, 1.0, 1.0, 0.18], Radii::all(px(1.5)));
                d.rect(Rect::new(bar.x, bar.y, bar.w * wear, bar.h), [0.35, 0.85, 0.45, 1.0], Radii::all(px(1.5)));
            }
        }
        Look::Inventory => {
            let slot = px(17.0);
            let gap = px(3.0);
            let (cols, rows) = (6.0_f32, 3.0_f32);
            let x0 = c.x - (cols * slot + (cols - 1.0) * gap) / 2.0;
            let y0 = c.y - (rows * slot + (rows - 1.0) * gap) / 2.0;
            for row in 0..3 {
                for col in 0..6 {
                    let r = Rect::new(x0 + col as f32 * (slot + gap), y0 + row as f32 * (slot + gap), slot, slot);
                    d.rect(r, BOX, Radii::all(px(3.0)));
                    if (row + col) % 3 == 0 {
                        d.rect(Rect::new(r.x + px(4.0), r.y + px(4.0), slot - px(8.0), slot - px(8.0)), PART, Radii::all(px(2.0)));
                    }
                }
            }
        }
        Look::Crosshair => {
            let arm = px(15.0);
            let gap = px(5.0);
            let t = px(3.0);
            for r in [
                Rect::new(c.x - t / 2.0, c.y - gap - arm, t, arm),
                Rect::new(c.x - t / 2.0, c.y + gap, t, arm),
                Rect::new(c.x - gap - arm, c.y - t / 2.0, arm, t),
                Rect::new(c.x + gap, c.y - t / 2.0, arm, t),
            ] {
                d.rect(r, ink, Radii::all(t / 2.0));
            }
        }
        Look::Effect => {
            let w = px(94.0);
            let h = px(30.0);
            let r = Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h);
            d.rect(r, BOX, Radii::all(px(8.0)));
            let swatch = Rect::new(r.x + px(6.0), r.y + px(6.0), h - px(12.0), h - px(12.0));
            d.rect(swatch, [0.45, 0.8, 0.95, 0.85], Radii::all(px(4.0)));
            d.line(Rect::new(swatch.right() + px(6.0), r.y + px(3.0), r.w - h, px(14.0)), "Speed II", px(11.0), ink, Align::Start);
            d.line(Rect::new(swatch.right() + px(6.0), r.y + px(15.0), r.w - h, px(12.0)), "0:30", px(10.0), [ink[0], ink[1], ink[2], 0.7], Align::Start);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mod_icons::{KINDS, Kind};

    #[test]
    fn a_preview_exists_for_exactly_the_mods_marked_as_previews() {
        for (id, kind) in KINDS {
            assert_eq!(look(id).is_some(), kind == Kind::Preview, "{id}: its look and its kind in mod_icons::KINDS disagree");
        }
    }
}
