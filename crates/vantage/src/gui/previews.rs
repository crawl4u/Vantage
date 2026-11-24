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

/// How far the inventory is enlarged in a card.
const INVENTORY_SCALE: f32 = 1.9;

/// How far the keys are enlarged in a card: more than the other HUDs, whose own sizes are right as they are.
const KEYS_SCALE: f32 = 1.75;

/// A row of text in the game, in unscaled GUI pixels, and the room the text keeps from a box's sides.
const ROW: f32 = 10.0;
const PAD: f32 = 5.0;
const PAD_X: f32 = 5.0;

/// The game font's size for the HUD's text, in unscaled GUI pixels.
const HUD_TEXT: f32 = 9.0;


/// How far, in text sizes, a line is lowered inside its box so the glyphs (which sit high in their line box) are
/// centred.
const GLYPH_DROP: f32 = 0.27;

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
        "gp_wp_on" => Look::Text(&["Waypoint 1 [20m]"]),
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
    if let Some((colour, width)) = sample.border {
        d.ring(r, Radii::all(if sample.rounded { 3.0 * k } else { 0.0 }), (width * k).max(0.5), colour);
    }
    if let Some(marks) = &sample.compass {
        // The strip as the game draws it: a tick over each mark, the marks fading towards the ends of the window.
        let step = 22.0 * s * k;
        let ink = sample.lines.first().map_or([1.0, 1.0, 1.0, 1.0], |l| l.color);
        let ink = if sample.lines.is_empty() { [1.0, 1.0, 1.0, 1.0] } else { ink };
        let size = HUD_TEXT * s * k;
        for (n, (text, alpha)) in marks.iter().enumerate() {
            let cx = r.x + r.w / 2.0 + (n as f32 - 4.0) * step;
            let colour = [ink[0], ink[1], ink[2], ink[3] * alpha];
            let tick_top = r.y + (sample.pad_y + 2.0) * s * k;
            d.rect(Rect::new(cx - 0.5 * s * k, tick_top, (1.0 * s * k).max(1.0), 5.0 * s * k), colour, Radii::default());
            d.text_in(Rect::new(cx - step / 2.0, tick_top + 6.0 * s * k, step, size * 1.5), text, size, colour, Align::Centre, GAME_FONT);
        }
        d.scene.push(Primitive::PopClip);
        return;
    }
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
    let icon_room = sample.icon[0] * s * k;
    let fit = if widest > 0.0 { ((r.w - 2.0 * PAD * s * k - icon_room) / widest).min(1.0) } else { 1.0 };
    if icon_room > 0.0 {
        let side = sample.icon[1] * s * k;
        if let Some(item) = &sample.item {
            put_item(d, Rect::new(r.x + PAD_X * s * k, r.y + (r.h - side) / 2.0, side, side), item);
        }
    }
    if let Some(tool) = &sample.tool {
        let side = 12.0 * s * k;
        put_item(d, Rect::new(r.right() - PAD_X * s * k - side, r.y + sample.pad_y * s * k + (ROW * s * k - side) / 2.0 + 1.0 * s * k, side, side), tool);
    }
    let size = HUD_TEXT * s * k * fit;
    let block = row * sample.lines.len() as f32;
    let top = r.y + (r.h - block) / 2.0;
    for (i, line) in sample.lines.iter().enumerate() {
        // The game's glyphs sit high in their line box, so the line drops a little to be centred in its row.
        let y = top + i as f32 * row + (row - size * 1.5) / 2.0 + size * GLYPH_DROP;
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
            // The bar is centred on the text's row and a little taller than the digits, so it shows above and below.
            let _ = by;
            let bar_h = (size * 1.15).max(bh * k * fit);
            d.rect(Rect::new(bar_x - (bw * k).max(1.0) / 2.0, top + i as f32 * row + (row - bar_h) / 2.0, (bw * k).max(1.0), bar_h), line.color, Radii::default());
            put(d, cell(bar_x - space / 2.0 - lw - 2.0, lw + 2.0), left, line.color, Align::End);
            put(d, cell(bar_x + space / 2.0, rw + 4.0 * s * k), right, line.color, Align::Start);
            continue;
        }
        if line.head.is_some() {
            // Coordinates: the label and number sit at the left of the box, and the heading marks stand in a column
            // at its right edge, as the game lays them out.
            let (head, head_color) = line.head.clone().unwrap_or_default();
            let hw = hud::text_width(&head) * s * k * fit;
            let tw = hud::text_width(&line.text) * s * k * fit;
            let left = r.x + PAD_X * s * k;
            put(d, cell(left, hw + 4.0 * s * k), &head, head_color, Align::Start);
            put(d, cell(left + hw + gap * fit, tw + 4.0 * s * k), &line.text, line.color, Align::Start);
            if let Some((tail, color)) = &line.tail {
                put(d, cell(r.right() - PAD_X * s * k - 40.0 * s * k, 40.0 * s * k), tail, *color, Align::End);
            }
            continue;
        }
        let pieces = line_text(line);
        if icon_room > 0.0 {
            // Beside an icon the lines read from the left, from just after it.
            let left = r.x + PAD_X * s * k + icon_room;
            let mut x = left;
            for (text, color) in &pieces {
                let width = hud::text_width(text) * s * k * fit;
                put(d, cell(x, width + 4.0 * s * k), text, *color, Align::Start);
                x += width + gap * fit;
            }
            continue;
        }
        if let [(text, color)] = pieces.as_slice() {
            // A single piece is centred in the whole box by the text itself, so the font's own widths decide the middle.
            put(d, Rect::new(r.x, y, r.w, size * 1.5), text, *color, Align::Centre);
            continue;
        }
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

/// Draws the game's icon of item `id` in `rect`, placed with the menu's own position and zoom like every other shape.
fn put_item(d: &mut Painter, rect: Rect, id: &str) {
    let rect = d.at(rect);
    d.scene.push(Primitive::Item { rect, id: id.to_owned() });
}

/// The armour card's pieces, helmet to boots: the item whose icon is drawn from the loaded pack, and a durability left.
const ARMOR_PIECES: [(&str, &str); 4] = [
    ("minecraft:diamond_helmet", "344"),
    ("minecraft:diamond_chestplate", "383"),
    ("minecraft:diamond_leggings", "286"),
    ("minecraft:diamond_boots", "347"),
];

/// Draws the Armor element: each piece's durability and its item icon, in a row, as the game shows them. The icons are
/// the game's own, so they follow the player's pack.
fn paint_armor(d: &mut Painter, area: Rect) {
    const ICON: f32 = 16.0;
    const GAP: f32 = 3.0;
    const BETWEEN: f32 = 5.0;
    let total: f32 = ARMOR_PIECES.iter().map(|(_, n)| hud::text_width(n) + GAP + ICON).sum::<f32>() + BETWEEN * 3.0;
    let k = (area.w * 0.9 / total.max(1.0)).min(area.h * 0.6 / ICON).min(PREVIEW_SCALE);
    let mut x = area.centre().x - total * k / 2.0;
    let top = area.centre().y - ICON * k / 2.0;
    let size = HUD_TEXT * k;
    for (item, wear) in ARMOR_PIECES {
        let width = hud::text_width(wear) * k;
        let cell = Rect::new(x, top + (ICON * k - size * 1.5) / 2.0 + size * GLYPH_DROP, width + 4.0 * k, size * 1.5);
        d.text_in(Rect::new(cell.x + 0.6 * k, cell.y + 0.6 * k, cell.w, cell.h), wear, size, [0.0, 0.0, 0.0, 0.55], Align::Start, GAME_FONT);
        d.text_in(cell, wear, size, [0.33, 0.9, 0.33, 1.0], Align::Start, GAME_FONT);
        x += width + GAP * k;
        put_item(d, Rect::new(x, top, ICON * k, ICON * k), item);
        x += (ICON + BETWEEN) * k;
    }
}

/// What the inventory card holds: slot (row-major, nine to a row), the item and how many.
const INVENTORY_ITEMS: [(usize, &str, u32); 9] = [
    (0, "minecraft:diamond", 32),
    (1, "minecraft:enchanted_book", 1),
    (8, "minecraft:totem_of_undying", 1),
    (9, "minecraft:lapis_lazuli", 64),
    (17, "minecraft:elytra", 1),
    (18, "minecraft:golden_apple", 8),
    (26, "minecraft:netherite_pickaxe", 1),
    (4, "minecraft:ender_pearl", 16),
    (13, "minecraft:bow", 1),
];

/// Draws the Inventory element: its box of three rows of nine slots with the game's own item icons and stack counts.
fn paint_inventory(d: &mut Painter, area: Rect) {
    let (cols, rows, cell, pad) = hud::INVENTORY_GRID;
    let (background, rounded, shadow) = hud::sample_box(Element::Inventory);
    let (w, h) = (cols as f32 * cell + pad * 2.0, rows as f32 * cell + pad * 2.0);
    // The inventory is a wide block of small icons, so it is drawn larger than the other HUDs, centred in the card.
    let k = (area.w * 0.96 / w).min(area.h * 0.9 / h).min(INVENTORY_SCALE);
    let r = Rect::new(area.centre().x - w * k / 2.0, area.centre().y - h * k / 2.0, w * k, h * k);
    d.rect(r, background, Radii::all(if rounded { 3.0 * k } else { 0.0 }));
    let size = HUD_TEXT * k;
    for (slot, item, count) in INVENTORY_ITEMS {
        let (col, row) = ((slot % cols) as f32, (slot / cols) as f32);
        let x = r.x + (pad + col * cell + 1.0) * k;
        let y = r.y + (pad + row * cell + 1.0) * k;
        put_item(d, Rect::new(x, y, 16.0 * k, 16.0 * k), item);
        if count > 1 {
            let text = count.to_string();
            let cell = Rect::new(x + 16.0 * k - hud::text_width(&text) * k, y + 16.0 * k - size * 1.05, hud::text_width(&text) * k + 4.0 * k, size * 1.5);
            if shadow {
                d.text_in(Rect::new(cell.x + 0.6 * k, cell.y + 0.6 * k, cell.w, cell.h), &text, size, [0.0, 0.0, 0.0, 0.55], Align::Start, GAME_FONT);
            }
            d.text_in(cell, &text, size, [1.0, 1.0, 1.0, 1.0], Align::Start, GAME_FONT);
        }
    }
}

/// Draws Potion status: a box with the Speed effect's icon from the pack, its name and its time.
fn paint_potion(d: &mut Painter, area: Rect) {
    const ICON: f32 = 18.0;
    const NEAR: f32 = 4.0;
    let (background, rounded, shadow) = hud::sample_box(Element::Potion);
    let (name, time) = ("Speed", "02:22");
    let text_w = hud::text_width(name).max(hud::text_width(time));
    let (w, h) = (PAD_X * 2.0 + ICON + NEAR + text_w, 26.0);
    let k = (area.w * 0.8 / w).min(area.h * 0.8 / h).min(PREVIEW_SCALE);
    let r = Rect::new(area.centre().x - w * k / 2.0, area.centre().y - h * k / 2.0, w * k, h * k);
    d.rect(r, background, Radii::all(if rounded { 3.0 * k } else { 0.0 }));
    let icon = Rect::new(r.x + PAD_X * k, r.y + (r.h - ICON * k) / 2.0, ICON * k, ICON * k);
    let rect = d.at(icon);
    d.scene.push(Primitive::Texture { rect, path: "textures/ui/speed_effect".to_owned() });
    let size = HUD_TEXT * k;
    let x = icon.right() + NEAR * k;
    for (i, text) in [name, time].into_iter().enumerate() {
        let cell = Rect::new(x, r.y + (r.h - 2.0 * ROW * k) / 2.0 + i as f32 * ROW * k + (ROW * k - size * 1.5) / 2.0 + size * GLYPH_DROP, text_w * k + 4.0 * k, size * 1.5);
        if shadow {
            d.text_in(Rect::new(cell.x + 0.6 * k, cell.y + 0.6 * k, cell.w, cell.h), text, size, [0.0, 0.0, 0.0, 0.55], Align::Start, GAME_FONT);
        }
        d.text_in(cell, text, size, [1.0, 1.0, 1.0, 1.0], Align::Start, GAME_FONT);
    }
}

/// The waypoint's colour in the card: the light blue of a default waypoint.
const WAYPOINT_BLUE: Rgba = [0.36, 0.63, 1.0, 1.0];

/// Draws a waypoint's label as the game shows it over a marked spot: a dark box outlined in the waypoint's colour,
/// holding its name and distance in the pack's font.
fn paint_waypoint(d: &mut Painter, area: Rect) {
    const PAD: f32 = 4.0;
    const HEIGHT: f32 = 14.0;
    let text = "Waypoint 1 [20m]";
    let w = hud::text_width(text) + PAD * 2.0;
    let k = (area.w * 0.8 / w).min(area.h * 0.5 / HEIGHT).min(PREVIEW_SCALE * 1.2);
    let r = Rect::new(area.centre().x - w * k / 2.0, area.centre().y - HEIGHT * k / 2.0, w * k, HEIGHT * k);
    d.rect(r, BOX, Radii::default());
    d.ring(r, Radii::default(), (1.0 * k).max(0.8), WAYPOINT_BLUE);
    let size = HUD_TEXT * k;
    d.text_in(Rect::new(r.x, r.y + (r.h - size * 1.5) / 2.0 + size * GLYPH_DROP, r.w, size * 1.5), text, size, [1.0, 1.0, 1.0, 1.0], Align::Centre, GAME_FONT);
}

/// Draws the Keystrokes element at its defaults: its keys at their real places.
fn paint_keys(d: &mut Painter, area: Rect) {
    let (keys, idle) = hud::sample_keys();
    let right = keys.iter().map(|k| k.rect[0] + k.rect[2]).max().unwrap_or(1) as f32;
    let bottom = keys.iter().map(|k| k.rect[1] + k.rect[3]).max().unwrap_or(1) as f32;
    // The keys are drawn larger than the other cards' HUDs: they are a small block that would otherwise look lost.
    let k = (area.w * 0.7 / right).min(area.h * 0.95 / bottom).min(KEYS_SCALE);
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
        if element == Element::Armor {
            paint_armor(d, area);
            return true;
        }
        if element == Element::Inventory {
            paint_inventory(d, area);
            return true;
        }
        if element == Element::Potion {
            paint_potion(d, area);
            return true;
        }
        if let Some(sample) = sample(element) {
            paint_sample(d, sample, area);
            return true;
        }
    }
    if id == "gp_wp_on" {
        paint_waypoint(d, area);
        return true;
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
            // As the game draws a potion effect: a dark rounded box, the effect's icon at its left and the name over the
            // time in the game's font.
            let h = px(34.0);
            let w = px(88.0);
            let r = Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h);
            d.rect(r, [0.1, 0.12, 0.1, 0.85], Radii::all(px(5.0)));
            let icon = Rect::new(r.x + px(5.0), r.y + (h - px(18.0)) / 2.0, px(18.0), px(18.0));
            d.rect(icon, [0.78, 0.2, 0.2, 0.95], Radii::all(px(4.0)));
            let size = px(11.0);
            let text_x = icon.right() + px(5.0);
            d.text_in(Rect::new(text_x, r.y + px(3.0), r.right() - text_x, size * 1.5), "Speed", size, [1.0, 1.0, 1.0, 1.0], Align::Start, GAME_FONT);
            d.text_in(Rect::new(text_x, r.y + px(16.0), r.right() - text_x, size * 1.5), "02:22", size, [1.0, 1.0, 1.0, 1.0], Align::Start, GAME_FONT);
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
