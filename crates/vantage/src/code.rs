//! Share codes: a whole setup as a short string like `VNT1-4F7QK-M2X9A-…`.
//!
//! The bytes are a bit-packed difference from the defaults: every option and every field of every
//! HUD element has one "changed" bit, and only a changed one is followed by its value (numbers cut
//! to a hundredth, colours to 3 bits and so on). An untouched setup is a few bytes; a heavily
//! customised one is about a hundred. Letters and digits are base 32 without the look-alikes
//! (no I, L, O or U), in groups of five, and a one-byte check at the end catches a mistyped code.
//!
//! Every struct is written with all its fields named (`let Style { .. } = style`), so adding a
//! field to the settings without adding it here does not compile.
//!
//! Writing and reading go in steps (the options, then each element) so the caller can spread them
//! over frames: a frame has little fuel.

use crate::config::{
    ARMOR_MAX_SIZE, ARMOR_MIN_SIZE, CpsMode, Corner, Element, Grid, KEY_SLOTS, MAX_ARMOR_OFFSET, MAX_SPEED_DECIMALS, PALETTE, Settings, Style,
    custom_color, custom_rgb, key_slot_id, valid_key,
};
use crate::crosshair_config::CrosshairSettings;
use crate::zoom_config::{KeyMode, ZoomKeys, ZoomSettings, transition_at, transition_index};

pub const PREFIX: &str = "VNT1";
/// Version 2 writes a colour with a flag for a custom (hex) one; version 1 codes still read.
/// Version 3 names a key slot in six bits instead of five (there are more than 32 of them now);
/// version 1 and 2 codes still read, with five. Version 4 adds PvP Info (the last late element) and the Auto Text
/// Hotkey's switch and four commands; version 3 codes read without them. Version 9 lets a custom colour carry transparency. Version 8 adds the Hive requeue, Perspective's
/// switches and the Scoreboard's header and one-line options; older codes read without them. Version 10 adds Glass; version 11 gives coloured glass its own style.
/// Version 12 can name the mods a code leaves out (`share.rs`), after the options. The version has four bits: 15 is the last.
const VERSION: u32 = 12;
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Bits for a key slot (and for how many keys are bound) in a code of version 3 or later.
const SLOT_BITS: u32 = 6;
/// Only the first slots travel in a share code: the count of bound keys must fit `SLOT_BITS` too
/// (so at most 63). Slots after them travel in the last section (`put_last`) with a seven-bit
/// offset, so `KEY_SLOTS` may grow to 190.
const CODED_SLOTS: usize = (1 << SLOT_BITS) - 1;

/// Bits for a palette index.
const COLOR_BITS: u32 = 3;
const _: () = assert!(PALETTE.len() <= 1 << COLOR_BITS);

/// Writes bits, most significant first.
pub struct Writer {
    bytes: Vec<u8>,
    acc: u32,
    used: u32,
    /// The mods this code leaves out (their numbers in `share::MODS`); `put_options` writes them. The caller
    /// has already put those mods' settings at the defaults.
    pub skip: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        let mut w = Self { bytes: Vec::with_capacity(160), acc: 0, used: 0, skip: Vec::new() };
        w.put(VERSION, 4);
        w
    }

    fn put(&mut self, value: u32, bits: u32) {
        for i in (0..bits).rev() {
            self.acc = (self.acc << 1) | ((value >> i) & 1);
            self.used += 1;
            if self.used == 8 {
                self.bytes.push(self.acc as u8);
                self.acc = 0;
                self.used = 0;
            }
        }
    }

    /// The code: the bytes with their check, as groups of five characters after the prefix.
    pub fn finish(mut self) -> String {
        while self.used != 0 {
            self.put(0, 1);
        }
        let check = checksum(&self.bytes);
        self.bytes.push(check);
        let mut text = String::with_capacity(PREFIX.len() + self.bytes.len() * 2 + 8);
        text.push_str(PREFIX);
        let (mut acc, mut n, mut chars) = (0u32, 0u32, 0usize);
        let mut emit = |text: &mut String, v: u32| {
            if chars % 5 == 0 {
                text.push('-');
            }
            text.push(ALPHABET[v as usize] as char);
            chars += 1;
        };
        for &byte in &self.bytes {
            acc = (acc << 8) | u32::from(byte);
            n += 8;
            while n >= 5 {
                emit(&mut text, (acc >> (n - 5)) & 31);
                n -= 5;
            }
            acc &= (1 << n) - 1;
        }
        if n > 0 {
            emit(&mut text, (acc << (5 - n)) & 31);
        }
        text
    }
}

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0x5Au8, |a, b| a.rotate_left(3) ^ b)
}

/// Reads bits back.
pub struct Reader {
    bytes: Vec<u8>,
    at: usize,
    end: usize,
    version: u32,
    /// The mods the code leaves out, once `get_options` has read them.
    skipped: Vec<u8>,
}

impl Reader {
    /// The code's version (what it carries depends on it).
    pub fn version(&self) -> u32 {
        self.version
    }

    /// The mods the code leaves out (their numbers in `share::MODS`): a load keeps the loader's own settings
    /// for them. Empty before `get_options` and for a code that carries everything.
    pub fn skipped(&self) -> &[u8] {
        &self.skipped
    }

    /// A reader over a code's text (dashes, spaces and case do not matter). None when the text
    /// is not a code of this version or its check fails.
    pub fn new(text: &str) -> Option<Self> {
        let body = text.trim();
        let body = body.get(..PREFIX.len()).filter(|p| p.eq_ignore_ascii_case(PREFIX)).map(|_| &body[PREFIX.len()..])?;
        let (mut acc, mut n) = (0u32, 0u32);
        let mut bytes = Vec::with_capacity(body.len() * 5 / 8 + 1);
        for c in body.chars() {
            if matches!(c, '-' | ' ' | '_') {
                continue;
            }
            let c = match c.to_ascii_uppercase() {
                'O' => '0',
                'I' | 'L' => '1',
                other => other,
            };
            let v = ALPHABET.iter().position(|a| *a as char == c)? as u32;
            acc = (acc << 5) | v;
            n += 5;
            if n >= 8 {
                bytes.push((acc >> (n - 8)) as u8);
                n -= 8;
                acc &= (1 << n) - 1;
            }
        }
        let (&check, data) = bytes.split_last()?;
        if data.is_empty() || checksum(data) != check {
            return None;
        }
        let end = data.len() * 8;
        let mut reader = Self { bytes, at: 0, end, version: VERSION, skipped: Vec::new() };
        reader.version = reader.get(4)?;
        (1..=VERSION).contains(&reader.version).then_some(reader)
    }

    fn get(&mut self, bits: u32) -> Option<u32> {
        if self.at + bits as usize > self.end {
            return None;
        }
        let mut v = 0;
        for _ in 0..bits {
            let bit = (self.bytes[self.at / 8] >> (7 - self.at % 8)) & 1;
            v = (v << 1) | u32::from(bit);
            self.at += 1;
        }
        Some(v)
    }
}

// ---- values --------------------------------------------------------------------------------

/// A number to `scale` steps per unit, cut to `bits`.
fn quant(v: f64, scale: f64, bits: u32) -> u32 {
    let q = if v.is_finite() { (v * scale).round().max(0.0) } else { 0.0 };
    (q as u64).min((1u64 << bits) - 1) as u32
}

fn put_bool(w: &mut Writer, a: bool, d: bool) {
    w.put(u32::from(a != d), 1);
}

fn get_bool(r: &mut Reader, d: bool) -> Option<bool> {
    Some(if r.get(1)? == 1 { !d } else { d })
}

fn put_uint(w: &mut Writer, a: usize, d: usize, bits: u32) {
    let max = (1usize << bits) - 1;
    let a = a.min(max);
    let changed = a != d;
    w.put(u32::from(changed), 1);
    if changed {
        w.put(a as u32, bits);
    }
}

fn get_uint(r: &mut Reader, d: usize, bits: u32) -> Option<usize> {
    Some(if r.get(1)? == 1 { r.get(bits)? as usize } else { d })
}

/// A colour: unchanged, a palette index, or (from version 2) a custom one's 24 bits (from version 9 with its transparency).
fn put_color(w: &mut Writer, a: usize, d: usize) {
    w.put(u32::from(a != d), 1);
    if a != d {
        if let Some(rgb) = custom_rgb(a) {
            w.put(1, 1);
            w.put(rgb, 24);
            // From version 9 a custom colour may carry transparency (0..=100 percent).
            let transparency = crate::config::custom_transparency(a);
            w.put(u32::from(transparency != 0), 1);
            if transparency != 0 {
                w.put(transparency, 7);
            }
        } else {
            w.put(0, 1);
            w.put(a as u32, COLOR_BITS);
        }
    }
}

fn get_color(r: &mut Reader, d: usize) -> Option<usize> {
    if r.get(1)? == 0 {
        return Some(d);
    }
    if r.version >= 2 && r.get(1)? == 1 {
        let rgb = r.get(24)?;
        if r.version >= 9 && r.get(1)? == 1 {
            return Some(crate::config::custom_color_alpha(rgb, r.get(7)?));
        }
        return Some(custom_color(rgb));
    }
    Some(r.get(COLOR_BITS)? as usize)
}

fn put_num(w: &mut Writer, a: f64, d: f64, scale: f64, bits: u32) {
    let (qa, qd) = (quant(a, scale, bits), quant(d, scale, bits));
    w.put(u32::from(qa != qd), 1);
    if qa != qd {
        w.put(qa, bits);
    }
}

fn get_num(r: &mut Reader, d: f64, scale: f64, bits: u32) -> Option<f64> {
    Some(if r.get(1)? == 1 { f64::from(r.get(bits)?) / scale } else { d })
}

fn put_f32(w: &mut Writer, a: f32, d: f32, scale: f64, bits: u32) {
    put_num(w, f64::from(a), f64::from(d), scale, bits);
}

fn get_f32(r: &mut Reader, d: f32, scale: f64, bits: u32) -> Option<f32> {
    Some(get_num(r, f64::from(d), scale, bits)? as f32)
}

/// A pair that is written together when either differs.
fn put_pair(w: &mut Writer, a: [f32; 2], d: [f32; 2], scale: f64, bits: u32) {
    let same = (0..2).all(|i| quant(f64::from(a[i]), scale, bits) == quant(f64::from(d[i]), scale, bits));
    w.put(u32::from(!same), 1);
    if !same {
        for v in a {
            w.put(quant(f64::from(v), scale, bits), bits);
        }
    }
}

fn get_pair(r: &mut Reader, d: [f32; 2], scale: f64, bits: u32) -> Option<[f32; 2]> {
    if r.get(1)? == 0 {
        return Some(d);
    }
    Some([r.get(bits)? as f32 / scale as f32, r.get(bits)? as f32 / scale as f32])
}

fn corner_index(c: Corner) -> usize {
    match c {
        Corner::TopLeft => 0,
        Corner::TopRight => 1,
        Corner::BottomLeft => 2,
        Corner::BottomRight => 3,
    }
}

fn corner_at(i: usize) -> Corner {
    match i {
        1 => Corner::TopRight,
        2 => Corner::BottomLeft,
        3 => Corner::BottomRight,
        _ => Corner::TopLeft,
    }
}

/// A key name: nothing, a letter, a digit, an F key, or any other plain name spelled out.
fn put_key(w: &mut Writer, key: &str) {
    let bytes = key.as_bytes();
    let letter = key.strip_prefix("Key").filter(|k| k.len() == 1 && k.as_bytes()[0].is_ascii_uppercase());
    let digit = key.strip_prefix("Digit").filter(|k| k.len() == 1 && k.as_bytes()[0].is_ascii_digit());
    let function = key.strip_prefix('F').and_then(|k| k.parse::<u32>().ok()).filter(|n| (1..=24).contains(n) && key.len() <= 3);
    if key.is_empty() {
        w.put(0, 3);
    } else if let Some(k) = letter {
        w.put(1, 3);
        w.put(u32::from(k.as_bytes()[0] - b'A'), 5);
    } else if let Some(k) = digit {
        w.put(2, 3);
        w.put(u32::from(k.as_bytes()[0] - b'0'), 4);
    } else if let Some(n) = function {
        w.put(3, 3);
        w.put(n - 1, 5);
    } else {
        w.put(4, 3);
        let len = bytes.len().min(31);
        w.put(len as u32, 5);
        for &b in &bytes[..len] {
            w.put(char_code(b), 6);
        }
    }
}

fn char_code(b: u8) -> u32 {
    match b {
        b'0'..=b'9' => u32::from(b - b'0'),
        b'A'..=b'Z' => u32::from(b - b'A') + 10,
        b'a'..=b'z' => u32::from(b - b'a') + 36,
        _ => 62,
    }
}

fn code_char(v: u32) -> char {
    match v {
        0..=9 => (b'0' + v as u8) as char,
        10..=35 => (b'A' + (v - 10) as u8) as char,
        36..=61 => (b'a' + (v - 36) as u8) as char,
        _ => 'x',
    }
}

fn get_key(r: &mut Reader) -> Option<String> {
    Some(match r.get(3)? {
        0 => String::new(),
        1 => format!("Key{}", (b'A' + r.get(5)?.min(25) as u8) as char),
        2 => format!("Digit{}", r.get(4)?.min(9)),
        3 => format!("F{}", r.get(5)? + 1),
        4 => {
            let len = r.get(5)?;
            let mut key = String::new();
            for _ in 0..len {
                key.push(code_char(r.get(6)?));
            }
            if valid_key(&key) { key } else { String::new() }
        }
        _ => return None,
    })
}

// ---- one element ---------------------------------------------------------------------------

/// A style with the options `put_extras` carries (the border and the CPS bar's colour) put back
/// to `d`'s: what the element's own part of a code is written from.
fn without_box_extras(style: &Style, d: &Style) -> Style {
    Style { border: d.border, border_width: d.border_width, border_color: d.border_color, border_opacity: d.border_opacity, bar_color: d.bar_color, ..style.clone() }
}

#[inline(never)]
pub fn put_element(w: &mut Writer, element: Element, style: &Style) {
    let d = element.default_style();
    // An element that differs only in its border is written as unchanged here: the border goes in
    // `put_extras`, so codes made before it still read the same.
    let style = &without_box_extras(style, &d);
    let changed = *style != d;
    w.put(u32::from(changed), 1);
    if !changed {
        return;
    }
    let Style {
        enabled,
        corner,
        position,
        scale,
        stretch,
        bg_color,
        text_color,
        pressed_color,
        pressed_text_color,
        opacity,
        labels,
        smart,
        rounded,
        background,
        shadow,
        brackets,
        reverse,
        auto_width,
        active_color,
        show_chunk,
        show_inchunk,
        show_sub,
        sub_range,
        armor_horizontal,
        armor_side,
        armor_text,
        armor_held,
        armor_pieces,
        armor_split,
        armor_offsets,
        armor_sizes,
        armor_gap,
        armor_good,
        armor_mid,
        armor_low,
        // Written by `put_extras`.
        border: _,
        border_width: _,
        border_color: _,
        border_opacity: _,
        bar_color: _,
    } = style;
    put_bool(w, *enabled, d.enabled);
    put_uint(w, corner_index(*corner), corner_index(d.corner), 2);
    // A placed element: a flag, then its place as twelve-bit fractions.
    let (a, b) = (*position, d.position);
    let quantised = |p: Option<[f32; 2]>| p.map(|p| [quant(f64::from(p[0]), 4095.0, 12), quant(f64::from(p[1]), 4095.0, 12)]);
    let same = quantised(a) == quantised(b);
    w.put(u32::from(!same), 1);
    if !same {
        match a {
            Some(p) => {
                w.put(1, 1);
                w.put(quant(f64::from(p[0]), 4095.0, 12), 12);
                w.put(quant(f64::from(p[1]), 4095.0, 12), 12);
            }
            None => w.put(0, 1),
        }
    }
    put_num(w, *scale, d.scale, 100.0, 9);
    put_num(w, stretch[0], d.stretch[0], 100.0, 9);
    put_num(w, stretch[1], d.stretch[1], 100.0, 9);
    for (a, b) in [(bg_color, d.bg_color), (text_color, d.text_color), (pressed_color, d.pressed_color), (pressed_text_color, d.pressed_text_color)] {
        put_color(w, *a, b);
    }
    put_num(w, *opacity, d.opacity, 100.0, 14);
    for (a, b) in [
        (labels, d.labels),
        (smart, d.smart),
        (rounded, d.rounded),
        (background, d.background),
        (shadow, d.shadow),
        (brackets, d.brackets),
        (reverse, d.reverse),
    ] {
        put_bool(w, *a, b);
    }
    let width = |v: Option<bool>| match v {
        None => 0,
        Some(false) => 1,
        Some(true) => 2,
    };
    let changed = width(*auto_width) != width(d.auto_width);
    w.put(u32::from(changed), 1);
    if changed {
        w.put(width(*auto_width), 2);
    }
    let changed = *active_color != d.active_color;
    w.put(u32::from(changed), 1);
    if changed {
        w.put(u32::from(active_color.is_some()), 1);
        put_color(w, active_color.unwrap_or(0), usize::MAX);
    }
    for (a, b) in [(show_chunk, d.show_chunk), (show_inchunk, d.show_inchunk), (show_sub, d.show_sub), (sub_range, d.sub_range), (armor_horizontal, d.armor_horizontal)] {
        put_bool(w, *a, b);
    }
    put_uint(w, *armor_side, d.armor_side, 2);
    put_uint(w, *armor_text, d.armor_text, 2);
    put_bool(w, *armor_held, d.armor_held);
    for i in 0..4 {
        put_bool(w, armor_pieces[i], d.armor_pieces[i]);
    }
    put_bool(w, *armor_split, d.armor_split);
    for i in 0..5 {
        put_pair(w, armor_offsets[i], d.armor_offsets[i], 10.0, 16);
    }
    for i in 0..5 {
        put_pair(w, armor_sizes[i], d.armor_sizes[i], 100.0, 9);
    }
    put_num(w, *armor_gap, d.armor_gap, 100.0, 10);
    for (a, b) in [(armor_good, d.armor_good), (armor_mid, d.armor_mid), (armor_low, d.armor_low)] {
        put_color(w, *a, b);
    }
}

#[inline(never)]
pub fn get_element(r: &mut Reader, element: Element) -> Option<Style> {
    let d = element.default_style();
    if r.get(1)? == 0 {
        return Some(d);
    }
    let enabled = get_bool(r, d.enabled)?;
    let corner = corner_at(get_uint(r, corner_index(d.corner), 2)?);
    let position = if r.get(1)? == 1 {
        if r.get(1)? == 1 {
            Some([r.get(12)? as f32 / 4095.0, r.get(12)? as f32 / 4095.0])
        } else {
            None
        }
    } else {
        d.position
    };
    let scale = get_num(r, d.scale, 100.0, 9)?;
    let stretch = [get_num(r, d.stretch[0], 100.0, 9)?, get_num(r, d.stretch[1], 100.0, 9)?];
    let bg_color = get_color(r, d.bg_color)?;
    let text_color = get_color(r, d.text_color)?;
    let pressed_color = get_color(r, d.pressed_color)?;
    let pressed_text_color = get_color(r, d.pressed_text_color)?;
    let opacity = get_num(r, d.opacity, 100.0, 14)?;
    let labels = get_bool(r, d.labels)?;
    let smart = get_bool(r, d.smart)?;
    let rounded = get_bool(r, d.rounded)?;
    let background = get_bool(r, d.background)?;
    let shadow = get_bool(r, d.shadow)?;
    let brackets = get_bool(r, d.brackets)?;
    let reverse = get_bool(r, d.reverse)?;
    let auto_width = if r.get(1)? == 1 {
        match r.get(2)? {
            1 => Some(false),
            2 => Some(true),
            _ => None,
        }
    } else {
        d.auto_width
    };
    let active_color = if r.get(1)? == 1 {
        let present = r.get(1)? == 1;
        let color = get_color(r, 0)?;
        present.then_some(color)
    } else {
        d.active_color
    };
    let show_chunk = get_bool(r, d.show_chunk)?;
    let show_inchunk = get_bool(r, d.show_inchunk)?;
    let show_sub = get_bool(r, d.show_sub)?;
    let sub_range = get_bool(r, d.sub_range)?;
    let armor_horizontal = get_bool(r, d.armor_horizontal)?;
    let armor_side = get_uint(r, d.armor_side, 2)?;
    let armor_text = get_uint(r, d.armor_text, 2)?;
    let armor_held = get_bool(r, d.armor_held)?;
    let mut armor_pieces = d.armor_pieces;
    for i in 0..4 {
        armor_pieces[i] = get_bool(r, d.armor_pieces[i])?;
    }
    let armor_split = get_bool(r, d.armor_split)?;
    let mut armor_offsets = d.armor_offsets;
    for i in 0..5 {
        armor_offsets[i] = get_pair(r, d.armor_offsets[i], 10.0, 16)?;
    }
    let mut armor_sizes = d.armor_sizes;
    for i in 0..5 {
        armor_sizes[i] = get_pair(r, d.armor_sizes[i], 100.0, 9)?;
    }
    let armor_gap = get_num(r, d.armor_gap, 100.0, 10)?;
    let armor_good = get_color(r, d.armor_good)?;
    let armor_mid = get_color(r, d.armor_mid)?;
    let armor_low = get_color(r, d.armor_low)?;
    Some(Style {
        enabled,
        corner,
        position,
        scale,
        stretch,
        bg_color,
        text_color,
        pressed_color,
        pressed_text_color,
        opacity,
        labels,
        smart,
        rounded,
        background,
        shadow,
        brackets,
        reverse,
        auto_width,
        active_color,
        show_chunk,
        show_inchunk,
        show_sub,
        sub_range,
        armor_horizontal,
        armor_side,
        armor_text,
        armor_held,
        armor_pieces,
        armor_split,
        armor_offsets,
        armor_sizes,
        armor_gap,
        armor_good,
        armor_mid,
        armor_low,
        // Read by `get_extras`.
        border: d.border,
        border_width: d.border_width,
        border_color: d.border_color,
        border_opacity: d.border_opacity,
        bar_color: d.bar_color,
    })
}

// ---- the options ---------------------------------------------------------------------------

/// Everything except the elements (and the menu key and the saved setups, which a code never
/// carries).
#[inline(never)]
pub fn put_options(w: &mut Writer, s: &Settings) {
    let d = Settings::default();
    let Settings {
        borders,
        grid,
        subchunks,
        slime_chunks,
        guardians,
        lights,
        blaze,
        hitboxes,
        hitbox_color,
        hitbox_player_color,
        hit_mode,
        hit_color,
        hit_opacity,
        fullbright,
        // The options after Clock's, the gameplay mods and the late elements go in the last
        // section (`put_last`), which names every one of their fields.
        item_delay_fix: _,
        gameplay: _,
        hide_scoreboard,
        sb_scores: _,
        waila_id: _,
        waila_pos: _,
        waila_mod: _,
        waila_growth: _,
        waila_tool: _,
        waila_dist: _,
        waila_state: _,
        waila_entity: _,
        waila_mod_color: _,
        waila_info_color: _,
        waila_good_color: _,
        waila_bad_color: _,
        va_yaw: _,
        va_pitch: _,
        va_speed: _,
        va_altitude: _,
        va_first: _,
        va_third: _,
        va_elytra: _,
        va_reverse: _,
        va_target: _,
        va_decimals: _,
        va_align: _,
        va_label_color: _,
        va_value_color: _,
        co_x: _,
        co_y: _,
        co_z: _,
        co_dir: _,
        co_decimal: _,
        co_horizontal: _,
        co_x_label: _,
        co_y_label: _,
        co_z_label: _,
        co_dir_cardinal: _,
        co_dir_mod: _,
        co_x_lc: _,
        co_x_tc: _,
        co_y_lc: _,
        co_y_tc: _,
        co_z_lc: _,
        co_z_tc: _,
        co_dir_tc: _,
        co_dir_mx: _,
        co_dir_mz: _,
        pvp_hits_lc: _,
        pvp_acc_lc: _,
        pvp_kills_lc: _,
        pvp_deaths_lc: _,
        pvp_num_tc: _,
        pvp_pct_tc: _,
        inv_empty: _,
        inv_show_bg: _,
        inv_mc: _,
        inv_grid: _,
        inv_grid_color: _,
        inv_grid_opacity: _,
        facing_label_chroma: _,
        waila_details: _,
        line_width,
        menu_key: _,
        favorites: _,
        hidden_mods: _,
        share_skip: _,
        show_mouse,
        show_space,
        align,
        show_guides,
        new_menu: _,
        cps_mode,
        // These came after the first codes were shared: `put_extras` writes them after the elements.
        speed_unit: _,
        speed_decimals: _,
        speed_vertical: _,
        facing_text: _,
        // Clock's options go with the elements after Inventory HUD (`put_extras`).
        clock_12h: _,
        clock_seconds: _,
        stopwatch_keep: _,
        playtime_format: _,
        playtime_world_only: _,
        // The Item counter's item goes with the elements after Potion status (`put_extras`).
        counter_item: _,
        // After those, in the last section `put_extras` writes (`put_reach`).
        reach_hide_zero: _,
        reach_highlight: _,
        reach_highlight_color: _,
        // And after those, the very last section (`sat_changed`).
        sat_overlay: _,
        sat_hunger: _,
        sat_potential: _,
        sat_inverse: _,
        sat_color: _,
        sat_layout: _,
        // And Potion status's, in the section after that (`potion_changed`).
        potion_mode: _,
        potion_horizontal: _,
        potion_per_line: _,
        potion_name: _,
        potion_level: _,
        potion_duration: _,
        potion_formatted: _,
        potion_upper: _,
        potion_blink: _,
        potion_color_name: _,
        potion_color_info: _,
        potion_icons: _,
        potion_hide_vanilla: _,
        potion_split: _,
        potion_offsets: _,
        potion_scales: _,
        // Also after the first codes: `put_extras` writes it at the very end.
        crosshair: _,
        elements: _,
        keys,
        zoom,
        profiles: _,
        names: _,
    } = s;
    for (a, b) in [
        (borders, d.borders),
        (subchunks, d.subchunks),
        (slime_chunks, d.slime_chunks),
        (guardians, d.guardians),
        (lights, d.lights),
        (blaze, d.blaze),
        (hitboxes, d.hitboxes),
        (fullbright, d.fullbright),
        (hide_scoreboard, d.hide_scoreboard),
        (show_mouse, d.show_mouse),
        (show_space, d.show_space),
        (align, d.align),
        (show_guides, d.show_guides),
    ] {
        put_bool(w, *a, b);
    }
    put_uint(w, grid.index(), d.grid.index(), 2);
    put_uint(w, cps_mode.index(), d.cps_mode.index(), 2);
    put_color(w, *hitbox_color, d.hitbox_color);
    put_color(w, *hitbox_player_color, d.hitbox_player_color);
    put_uint(w, *hit_mode, d.hit_mode, 2);
    put_color(w, *hit_color, d.hit_color);
    put_num(w, *hit_opacity, d.hit_opacity, 100.0, 14);
    put_num(w, *line_width, d.line_width, 100.0, 9);
    // The keys bound to options and actions: how many, then each one's slot and key.
    let bound: Vec<(usize, &String)> =
        (0..KEY_SLOTS.min(CODED_SLOTS)).filter_map(|i| key_slot_id(i).and_then(|id| keys.get(id)).filter(|k| !k.is_empty()).map(|k| (i, k))).collect();
    w.put(bound.len() as u32, SLOT_BITS);
    for (i, key) in bound {
        w.put(i as u32, SLOT_BITS);
        put_key(w, key);
    }
    put_zoom(w, zoom, &d.zoom);
    // Version 12: the mods left out, as one bit each up to the last one named. Nothing left out is one bit.
    let skip = crate::share::clean(&w.skip);
    w.put(u32::from(!skip.is_empty()), 1);
    if let Some(last) = skip.last().copied() {
        w.put(u32::from(last) + 1, SKIP_BITS);
        for n in 0..=last {
            w.put(u32::from(skip.contains(&n)), 1);
        }
    }
}

/// Bits for how many mods the left-out list covers (so up to 127 mods).
const SKIP_BITS: u32 = 7;
const _: () = assert!(crate::share::MODS.len() < 1 << SKIP_BITS);

/// The options added after the first codes were shared (Speed's unit, decimals and vertical, the
/// Stopwatch's keep running, Playtime's format and world only). They come after the elements: a
/// code made before them ends with padding bits that read as "unchanged", so it still loads with
/// these at their defaults, and a code with them still loads in a version that does not know them.
#[inline(never)]
pub fn put_extras(w: &mut Writer, s: &Settings) {
    let d = Settings::default();
    let now = (s.speed_unit, s.speed_decimals, s.speed_vertical, s.stopwatch_keep, s.playtime_format, s.playtime_world_only, s.facing_text);
    let start = (d.speed_unit, d.speed_decimals, d.speed_vertical, d.stopwatch_keep, d.playtime_format, d.playtime_world_only, d.facing_text);
    w.put(u32::from(now != start), 1);
    if now != start {
        put_uint(w, s.speed_unit, d.speed_unit, 1);
        put_uint(w, s.speed_decimals, d.speed_decimals, 2);
        put_bool(w, s.speed_vertical, d.speed_vertical);
        put_bool(w, s.stopwatch_keep, d.stopwatch_keep);
        put_uint(w, s.playtime_format, d.playtime_format, 1);
        put_bool(w, s.playtime_world_only, d.playtime_world_only);
        put_bool(w, s.facing_text, d.facing_text);
    }
    // The Crosshair comes last and has a flag only when it is not at its defaults, so a setup
    // without one is written exactly as before, and a code made before it ends in padding bits
    // that read as "no crosshair section".
    let boxes = box_extras_changed(s);
    let later = later_changed(s);
    // The elements after Potion status (Array list, Item counter, Inventory HUD) come after it, so a
    // code with them has Potion status's flag too.
    // The last section (the gameplay mods, WAILA, the Scoreboard and the other late elements) comes
    // after Clock's, so a code with it has Clock's flag (and every flag before it) too.
    let last = last_changed(s);
    let round_two = round_two_changed(s) || last;
    let round_one = round_one_changed(s) || round_two;
    let potion = potion_changed(s) || round_one;
    let sat = sat_changed(s) || potion;
    // Reach display's section comes before Saturation's: a code with Saturation's has Reach's flag too.
    let reach = reach_changed(s) || sat;
    if s.crosshair != d.crosshair {
        w.put(1, 1);
        put_crosshair(w, &s.crosshair, &d.crosshair);
    } else if boxes || later || reach {
        // A section follows: say "no crosshair" in so many bits.
        w.put(0, 1);
    }
    // The borders and the CPS bar's colour come after that, the same way: a flag only when one of
    // them is not at its default, so codes without them end as before. (Only the first elements:
    // the later ones keep theirs in their own section, so a code made before them reads the same.)
    if boxes {
        w.put(1, 1);
        for element in &Element::ALL[..Element::FIRST] {
            put_box_extras(w, s.style(*element), &element.default_style());
        }
    } else if later || reach {
        w.put(0, 1);
    }
    // The elements added after the first codes (Reach display and on): a flag, then each one's
    // look and its border.
    if later {
        w.put(1, 1);
        for element in &Element::ALL[Element::FIRST..Element::CODED] {
            let d = element.default_style();
            put_element(w, *element, s.style(*element));
            put_box_extras(w, s.style(*element), &d);
        }
    } else if reach {
        w.put(0, 1);
    }
    // Reach display's own options, again with a flag only when one is not at its default (and
    // always when Saturation's follow, which come last).
    if reach {
        w.put(1, 1);
        put_bool(w, s.reach_hide_zero, d.reach_hide_zero);
        put_bool(w, s.reach_highlight, d.reach_highlight);
        put_color(w, s.reach_highlight_color, d.reach_highlight_color);
    }
    // Saturation's options come last of all, with a flag only when one is not at its default.
    if sat {
        w.put(1, 1);
        put_bool(w, s.sat_overlay, d.sat_overlay);
        put_bool(w, s.sat_hunger, d.sat_hunger);
        put_bool(w, s.sat_potential, d.sat_potential);
        put_bool(w, s.sat_inverse, d.sat_inverse);
        put_color(w, s.sat_color, d.sat_color);
        put_uint(w, s.sat_layout, d.sat_layout, 1);
    }
    // Potion status is the very last: its look and its options, with a flag only when one is not at
    // its default (a code made before it ends in padding bits that read as "none").
    if potion {
        w.put(1, 1);
        let e = Element::Potion;
        put_element(w, e, s.style(e));
        put_box_extras(w, s.style(e), &e.default_style());
        put_uint(w, s.potion_mode, d.potion_mode, 1);
        put_bool(w, s.potion_horizontal, d.potion_horizontal);
        put_uint(w, s.potion_per_line, d.potion_per_line, 4);
        put_bool(w, s.potion_name, d.potion_name);
        put_bool(w, s.potion_level, d.potion_level);
        put_bool(w, s.potion_duration, d.potion_duration);
        put_bool(w, s.potion_formatted, d.potion_formatted);
        put_bool(w, s.potion_upper, d.potion_upper);
        put_bool(w, s.potion_blink, d.potion_blink);
        put_bool(w, s.potion_color_name, d.potion_color_name);
        put_bool(w, s.potion_color_info, d.potion_color_info);
        put_bool(w, s.potion_icons, d.potion_icons);
        put_bool(w, s.potion_hide_vanilla, d.potion_hide_vanilla);
        put_bool(w, s.potion_split, d.potion_split);
        // Only the effects that were moved: how many, then each one's id and offset (shifted so it is
        // never negative).
        let moved: Vec<usize> = (0..crate::config::POTION_SLOTS).filter(|i| potion_moved(s, *i)).collect();
        w.put(moved.len() as u32, 6);
        for i in moved {
            w.put(i as u32, 6);
            for v in s.potion_offsets[i] {
                w.put(quant(f64::from(v.round()) + 2048.0, 1.0, 12), 12);
            }
            w.put(quant(f64::from(s.potion_scales[i]), 100.0, 9), 9);
        }
    }
    // The elements after Potion status are the very last, with a flag only when one of them (or the
    // Item counter's item) is not at its default.
    if round_one {
        w.put(1, 1);
        for element in &Element::ALL[Element::AFTER_POTION..Element::AFTER_ROUND_ONE] {
            let d = element.default_style();
            put_element(w, *element, s.style(*element));
            put_box_extras(w, s.style(*element), &d);
        }
        put_uint(w, s.counter_item, Settings::default().counter_item, 3);
        // Clock, View angles and Potion counter come last, with a flag of their own.
        if round_two {
            w.put(1, 1);
            for element in &Element::ALL[Element::AFTER_ROUND_ONE..Element::CODED_END] {
                let d = element.default_style();
                put_element(w, *element, s.style(*element));
                put_box_extras(w, s.style(*element), &d);
            }
            put_bool(w, s.clock_12h, Settings::default().clock_12h);
            put_bool(w, s.clock_seconds, Settings::default().clock_seconds);
            // The last section: a code made before it ends in padding bits that read as "none".
            if last {
                w.put(1, 1);
                put_last(w, s);
            }
        } else {
            w.put(0, 1);
        }
    }
}

/// Whether Array list, Item counter or Inventory HUD (or the Item counter's item) is not at its default.
fn round_one_changed(s: &Settings) -> bool {
    Element::ALL[Element::AFTER_POTION..Element::AFTER_ROUND_ONE].iter().any(|e| *s.style(*e) != e.default_style()) || s.counter_item != Settings::default().counter_item
}

/// Whether Clock, View angles or Potion counter (or the Clock's options) is not at its default.
fn round_two_changed(s: &Settings) -> bool {
    let d = Settings::default();
    Element::ALL[Element::AFTER_ROUND_ONE..Element::CODED_END].iter().any(|e| *s.style(*e) != e.default_style()) || s.clock_12h != d.clock_12h || s.clock_seconds != d.clock_seconds
}

/// Whether effect `i` of Potion status was moved or resized.
fn potion_moved(s: &Settings, i: usize) -> bool {
    s.potion_offsets.get(i).is_some_and(|o| *o != [0.0; 2]) || s.potion_scales.get(i).is_some_and(|v| (*v - 1.0).abs() > 0.004)
}

/// Whether Potion status's look or any of its options is not at its default.
fn potion_changed(s: &Settings) -> bool {
    let d = Settings::default();
    let e = Element::Potion;
    *s.style(e) != e.default_style()
        || (
            s.potion_mode,
            s.potion_horizontal,
            s.potion_per_line,
            (s.potion_name, s.potion_level, s.potion_duration, s.potion_formatted, s.potion_upper),
            (s.potion_blink, s.potion_color_name, s.potion_color_info, s.potion_icons, s.potion_hide_vanilla),
            s.potion_split,
            (0..crate::config::POTION_SLOTS).any(|i| potion_moved(s, i)),
        ) != (
            d.potion_mode,
            d.potion_horizontal,
            d.potion_per_line,
            (d.potion_name, d.potion_level, d.potion_duration, d.potion_formatted, d.potion_upper),
            (d.potion_blink, d.potion_color_name, d.potion_color_info, d.potion_icons, d.potion_hide_vanilla),
            d.potion_split,
            false,
        )
}

/// Whether any of Saturation's options is not at its default.
fn sat_changed(s: &Settings) -> bool {
    let d = Settings::default();
    (s.sat_overlay, s.sat_hunger, s.sat_potential, s.sat_inverse, s.sat_color, s.sat_layout) != (d.sat_overlay, d.sat_hunger, d.sat_potential, d.sat_inverse, d.sat_color, d.sat_layout)
}

/// Whether any of Reach display's own options (hide at 0, highlight, its colour) is not at its default.
fn reach_changed(s: &Settings) -> bool {
    let d = Settings::default();
    (s.reach_hide_zero, s.reach_highlight, s.reach_highlight_color) != (d.reach_hide_zero, d.reach_highlight, d.reach_highlight_color)
}

/// Whether any of the later elements is not at its default.
fn later_changed(s: &Settings) -> bool {
    Element::ALL[Element::FIRST..Element::CODED].iter().any(|e| {
        let a = s.style(*e);
        *a != e.default_style()
    })
}

/// Whether any element's border or the CPS bar's colour is not at its default.
fn box_extras_changed(s: &Settings) -> bool {
    Element::ALL[..Element::FIRST].iter().any(|e| {
        let (a, d) = (s.style(*e), e.default_style());
        without_box_extras(a, &d) != *a
    })
}

/// One element's border and bar colour: a flag, then each one.
fn put_box_extras(w: &mut Writer, a: &Style, d: &Style) {
    let changed = without_box_extras(a, d) != *a;
    w.put(u32::from(changed), 1);
    if !changed {
        return;
    }
    put_bool(w, a.border, d.border);
    put_num(w, a.border_width, d.border_width, 100.0, 9);
    put_color(w, a.border_color, d.border_color);
    put_num(w, a.border_opacity, d.border_opacity, 100.0, 14);
    let bar = a.bar_color != d.bar_color;
    w.put(u32::from(bar), 1);
    if bar {
        w.put(u32::from(a.bar_color.is_some()), 1);
        put_color(w, a.bar_color.unwrap_or(0), usize::MAX);
    }
}

fn get_box_extras(r: &mut Reader, a: &mut Style, d: &Style) -> Option<()> {
    if r.get(1)? == 0 {
        return Some(());
    }
    a.border = get_bool(r, d.border)?;
    a.border_width = get_num(r, d.border_width, 100.0, 9)?;
    a.border_color = get_color(r, d.border_color)?;
    a.border_opacity = get_num(r, d.border_opacity, 100.0, 14)?;
    if r.get(1)? == 1 {
        let present = r.get(1)? == 1;
        let color = get_color(r, 0)?;
        a.bar_color = present.then_some(color);
    }
    Some(())
}

/// Reads what `put_extras` wrote into `s`. A code that has none (made before them) leaves the
/// defaults; None only when a code that has them is cut short.
#[inline(never)]
pub fn get_extras(r: &mut Reader, s: &mut Settings) -> Option<()> {
    let d = Settings::default();
    // Nothing left at all, or a padding bit: an older code.
    if r.get(1).unwrap_or(0) == 1 {
        s.speed_unit = get_uint(r, d.speed_unit, 1)?;
        s.speed_decimals = get_uint(r, d.speed_decimals, 2)?.min(MAX_SPEED_DECIMALS);
        s.speed_vertical = get_bool(r, d.speed_vertical)?;
        s.stopwatch_keep = get_bool(r, d.stopwatch_keep)?;
        s.playtime_format = get_uint(r, d.playtime_format, 1)?;
        s.playtime_world_only = get_bool(r, d.playtime_world_only)?;
        s.facing_text = get_bool(r, d.facing_text)?;
    }
    // The Crosshair section, if there is one: padding reads as none.
    if r.get(1).unwrap_or(0) == 1 {
        s.crosshair = get_crosshair(r, &d.crosshair)?;
    }
    // Every element has a style, the later ones at their defaults unless the code says otherwise.
    for element in Element::ALL {
        if element.index() >= s.elements.len() {
            s.elements.push(element.default_style());
        }
    }
    // The borders and the CPS bar's colour, if there are any: padding reads as none.
    if r.get(1).unwrap_or(0) == 1 {
        for element in Element::ALL[..Element::FIRST].iter().copied() {
            let d = element.default_style();
            get_box_extras(r, &mut s.elements[element.index()], &d)?;
        }
    }
    // The later elements, if there are any: padding reads as none.
    if r.get(1).unwrap_or(0) == 1 {
        for element in Element::ALL[Element::FIRST..Element::CODED].iter().copied() {
            let d = element.default_style();
            s.elements[element.index()] = get_element(r, element)?;
            get_box_extras(r, &mut s.elements[element.index()], &d)?;
        }
    }
    // Reach display's own options, if there are any: padding reads as none.
    if r.get(1).unwrap_or(0) == 1 {
        s.reach_hide_zero = get_bool(r, d.reach_hide_zero)?;
        s.reach_highlight = get_bool(r, d.reach_highlight)?;
        s.reach_highlight_color = get_color(r, d.reach_highlight_color)?;
    }
    // Saturation's options, if there are any: padding reads as none.
    if r.get(1).unwrap_or(0) == 1 {
        s.sat_overlay = get_bool(r, d.sat_overlay)?;
        s.sat_hunger = get_bool(r, d.sat_hunger)?;
        s.sat_potential = get_bool(r, d.sat_potential)?;
        s.sat_inverse = get_bool(r, d.sat_inverse)?;
        s.sat_color = get_color(r, d.sat_color)?;
        s.sat_layout = get_uint(r, d.sat_layout, 1)?;
    }
    // Potion status's look and options, if there are any: padding reads as none.
    if r.get(1).unwrap_or(0) == 1 {
        let e = Element::Potion;
        let style_default = e.default_style();
        s.elements[e.index()] = get_element(r, e)?;
        get_box_extras(r, &mut s.elements[e.index()], &style_default)?;
        s.potion_mode = get_uint(r, d.potion_mode, 1)?;
        s.potion_horizontal = get_bool(r, d.potion_horizontal)?;
        s.potion_per_line = get_uint(r, d.potion_per_line, 4)?;
        s.potion_name = get_bool(r, d.potion_name)?;
        s.potion_level = get_bool(r, d.potion_level)?;
        s.potion_duration = get_bool(r, d.potion_duration)?;
        s.potion_formatted = get_bool(r, d.potion_formatted)?;
        s.potion_upper = get_bool(r, d.potion_upper)?;
        s.potion_blink = get_bool(r, d.potion_blink)?;
        s.potion_color_name = get_bool(r, d.potion_color_name)?;
        s.potion_color_info = get_bool(r, d.potion_color_info)?;
        s.potion_icons = get_bool(r, d.potion_icons)?;
        s.potion_hide_vanilla = get_bool(r, d.potion_hide_vanilla)?;
        s.potion_split = get_bool(r, d.potion_split)?;
        for _ in 0..r.get(6)? {
            let i = r.get(6)? as usize;
            let x = r.get(12)? as f32 - 2048.0;
            let y = r.get(12)? as f32 - 2048.0;
            let scale = r.get(9)? as f32 / 100.0;
            if i < s.potion_offsets.len() {
                s.potion_offsets[i] = [x, y];
                s.potion_scales[i] = scale;
            }
        }
    }
    // The elements after Potion status, if there are any: padding reads as none.
    if r.get(1).unwrap_or(0) == 1 {
        for element in Element::ALL[Element::AFTER_POTION..Element::AFTER_ROUND_ONE].iter().copied() {
            let style_default = element.default_style();
            s.elements[element.index()] = get_element(r, element)?;
            get_box_extras(r, &mut s.elements[element.index()], &style_default)?;
        }
        s.counter_item = get_uint(r, d.counter_item, 3)?.min(crate::config::COUNTER_ITEMS.len() - 1);
        // Clock, View angles and Potion counter, if there are any: padding reads as none.
        if r.get(1).unwrap_or(0) == 1 {
            for element in Element::ALL[Element::AFTER_ROUND_ONE..Element::CODED_END].iter().copied() {
                let style_default = element.default_style();
                s.elements[element.index()] = get_element(r, element)?;
                get_box_extras(r, &mut s.elements[element.index()], &style_default)?;
            }
            s.clock_12h = get_bool(r, d.clock_12h)?;
            s.clock_seconds = get_bool(r, d.clock_seconds)?;
            if r.get(1).unwrap_or(0) == 1 {
                get_last(r, s)?;
            }
        }
    }
    Some(())
}

/// The late elements (Scoreboard, Target HUD, Waypoints, WAILA).
fn last_elements() -> Vec<Element> {
    Element::ALL[Element::CODED_END..].to_vec()
}

/// Whether `a`'s gameplay mods differ from the defaults (what a code does not carry stays out of
/// the comparison: the colour picker's lists and the marked spots are the person's own).
fn gameplay_changed(a: &crate::gameplay::GameplaySettings) -> bool {
    let d = crate::gameplay::GameplaySettings::default();
    let mut a = a.clone();
    a.recent_colors = Vec::new();
    a.favorite_colors = Vec::new();
    a.waypoints = Vec::new();
    // Chroma lists are compared by what they mean, so a trailing default does not count.
    let chroma_same = (0..Element::COUNT).all(|i| a.chroma(i) == d.chroma(i));
    a.chroma_on = Vec::new();
    a.chroma_speed = Vec::new();
    a.chroma_type = Vec::new();
    !chroma_same || a != d
}

/// Whether any key slot past the ones the first sections carry is bound.
fn late_keys_bound(s: &Settings) -> bool {
    (CODED_SLOTS..KEY_SLOTS).any(|i| key_slot_id(i).is_some_and(|id| !s.key(id).is_empty()))
}

fn last_settings_changed(s: &Settings) -> bool {
    let d = Settings::default();
    (s.item_delay_fix, s.sb_scores, s.waila_id, s.waila_pos, s.waila_mod, s.waila_growth, s.waila_tool, s.waila_dist, s.waila_state, s.waila_entity)
        != (d.item_delay_fix, d.sb_scores, d.waila_id, d.waila_pos, d.waila_mod, d.waila_growth, d.waila_tool, d.waila_dist, d.waila_state, d.waila_entity)
        || (s.waila_mod_color, s.waila_info_color, s.waila_good_color, s.waila_bad_color) != (d.waila_mod_color, d.waila_info_color, d.waila_good_color, d.waila_bad_color)
        || (s.va_yaw, s.va_pitch, s.va_speed, s.va_altitude, s.va_first, s.va_third, s.va_elytra, s.va_reverse, s.va_target)
            != (d.va_yaw, d.va_pitch, d.va_speed, d.va_altitude, d.va_first, d.va_third, d.va_elytra, d.va_reverse, d.va_target)
        || (s.co_x, s.co_y, s.co_z, s.co_dir, s.co_decimal, s.co_horizontal) != (d.co_x, d.co_y, d.co_z, d.co_dir, d.co_decimal, d.co_horizontal)
        || (s.co_x_label, s.co_y_label, s.co_z_label, s.co_dir_cardinal, s.co_dir_mod) != (d.co_x_label, d.co_y_label, d.co_z_label, d.co_dir_cardinal, d.co_dir_mod)
        || (s.co_x_lc, s.co_x_tc, s.co_y_lc, s.co_y_tc, s.co_z_lc, s.co_z_tc) != (d.co_x_lc, d.co_x_tc, d.co_y_lc, d.co_y_tc, d.co_z_lc, d.co_z_tc)
        || (s.co_dir_tc, s.co_dir_mx, s.co_dir_mz) != (d.co_dir_tc, d.co_dir_mx, d.co_dir_mz)
        || s.waila_details != d.waila_details
        || (s.inv_empty, s.inv_show_bg, s.inv_mc, s.inv_grid, s.inv_grid_color) != (d.inv_empty, d.inv_show_bg, d.inv_mc, d.inv_grid, d.inv_grid_color)
        || s.inv_grid_opacity != d.inv_grid_opacity
        || s.facing_label_chroma != d.facing_label_chroma
        || (s.pvp_hits_lc, s.pvp_acc_lc, s.pvp_kills_lc, s.pvp_deaths_lc, s.pvp_num_tc, s.pvp_pct_tc) != (d.pvp_hits_lc, d.pvp_acc_lc, d.pvp_kills_lc, d.pvp_deaths_lc, d.pvp_num_tc, d.pvp_pct_tc)
        || (s.va_decimals, s.va_align, s.va_label_color, s.va_value_color) != (d.va_decimals, d.va_align, d.va_label_color, d.va_value_color)
}

/// Whether anything the last section carries is not at its default.
fn last_changed(s: &Settings) -> bool {
    last_elements().iter().any(|e| *s.style(*e) != e.default_style()) || last_settings_changed(s) || gameplay_changed(&s.gameplay) || late_keys_bound(s)
}

/// The last section: the late elements, the options after Clock's, the gameplay mods (all of them
/// but the colour picker's lists and the marked spots), the chroma of each element and the keys
/// past the first slots. Each part has a flag of its own.
#[inline(never)]
fn put_last(w: &mut Writer, s: &Settings) {
    let d = Settings::default();
    // The late elements.
    let elements = last_elements().iter().any(|e| *s.style(*e) != e.default_style());
    w.put(u32::from(elements), 1);
    if elements {
        for element in last_elements() {
            let ed = element.default_style();
            put_element(w, element, s.style(element));
            put_box_extras(w, s.style(element), &ed);
        }
    }
    // The options after Clock's: Item delay fix, the Scoreboard's scores and WAILA's.
    let settings = last_settings_changed(s);
    w.put(u32::from(settings), 1);
    if settings {
        put_bool(w, s.item_delay_fix, d.item_delay_fix);
        put_bool(w, s.sb_scores, d.sb_scores);
        put_bool(w, s.waila_id, d.waila_id);
        put_bool(w, s.waila_pos, d.waila_pos);
        put_bool(w, s.waila_mod, d.waila_mod);
        put_bool(w, s.waila_growth, d.waila_growth);
        put_bool(w, s.waila_tool, d.waila_tool);
        put_bool(w, s.waila_dist, d.waila_dist);
        put_bool(w, s.waila_state, d.waila_state);
        put_bool(w, s.waila_entity, d.waila_entity);
        put_color(w, s.waila_mod_color, d.waila_mod_color);
        put_color(w, s.waila_info_color, d.waila_info_color);
        put_color(w, s.waila_good_color, d.waila_good_color);
        put_color(w, s.waila_bad_color, d.waila_bad_color);
        put_bool(w, s.va_yaw, d.va_yaw);
        put_bool(w, s.va_pitch, d.va_pitch);
        put_bool(w, s.va_speed, d.va_speed);
        put_bool(w, s.va_altitude, d.va_altitude);
        put_bool(w, s.va_first, d.va_first);
        put_bool(w, s.va_third, d.va_third);
        put_bool(w, s.va_elytra, d.va_elytra);
        put_bool(w, s.va_reverse, d.va_reverse);
        put_bool(w, s.va_target, d.va_target);
        put_uint(w, s.va_decimals, d.va_decimals, 3);
        put_uint(w, s.va_align, d.va_align, 2);
        put_color(w, s.va_label_color, d.va_label_color);
        put_color(w, s.va_value_color, d.va_value_color);
        put_bool(w, s.co_x, d.co_x);
        put_bool(w, s.co_y, d.co_y);
        put_bool(w, s.co_z, d.co_z);
        put_bool(w, s.co_dir, d.co_dir);
        put_bool(w, s.co_decimal, d.co_decimal);
        put_bool(w, s.co_horizontal, d.co_horizontal);
        put_bool(w, s.co_x_label, d.co_x_label);
        put_bool(w, s.co_y_label, d.co_y_label);
        put_bool(w, s.co_z_label, d.co_z_label);
        put_bool(w, s.co_dir_cardinal, d.co_dir_cardinal);
        put_bool(w, s.co_dir_mod, d.co_dir_mod);
        put_color(w, s.co_x_lc, d.co_x_lc);
        put_color(w, s.co_x_tc, d.co_x_tc);
        put_color(w, s.co_y_lc, d.co_y_lc);
        put_color(w, s.co_y_tc, d.co_y_tc);
        put_color(w, s.co_z_lc, d.co_z_lc);
        put_color(w, s.co_z_tc, d.co_z_tc);
        put_color(w, s.co_dir_tc, d.co_dir_tc);
        put_color(w, s.co_dir_mx, d.co_dir_mx);
        put_color(w, s.co_dir_mz, d.co_dir_mz);
        put_uint(w, s.waila_details, d.waila_details, 2);
        put_color(w, s.pvp_hits_lc, d.pvp_hits_lc);
        put_color(w, s.pvp_acc_lc, d.pvp_acc_lc);
        put_color(w, s.pvp_kills_lc, d.pvp_kills_lc);
        put_color(w, s.pvp_deaths_lc, d.pvp_deaths_lc);
        put_color(w, s.pvp_num_tc, d.pvp_num_tc);
        put_color(w, s.pvp_pct_tc, d.pvp_pct_tc);
        put_bool(w, s.inv_empty, d.inv_empty);
        put_bool(w, s.inv_show_bg, d.inv_show_bg);
        put_bool(w, s.inv_mc, d.inv_mc);
        put_bool(w, s.inv_grid, d.inv_grid);
        put_color(w, s.inv_grid_color, d.inv_grid_color);
        put_num(w, s.inv_grid_opacity, d.inv_grid_opacity, 100.0, 14);
        put_bool(w, s.facing_label_chroma, d.facing_label_chroma);
    }
    // The gameplay mods.
    let gameplay = gameplay_changed(&s.gameplay);
    w.put(u32::from(gameplay), 1);
    if gameplay {
        put_gameplay(w, &s.gameplay);
    }
    // The keys of the slots past the first 63: how many, then each one's offset and key.
    let late: Vec<(usize, &str)> = (CODED_SLOTS..KEY_SLOTS).filter_map(|i| key_slot_id(i).map(|id| (i, s.key(id)))).filter(|(_, k)| !k.is_empty()).collect();
    w.put(u32::from(!late.is_empty()), 1);
    if !late.is_empty() {
        w.put(late.len() as u32, LATE_SLOT_BITS);
        for (i, key) in late {
            w.put((i - CODED_SLOTS) as u32, LATE_SLOT_BITS);
            put_key(w, key);
        }
    }
}

/// Bits for a slot's offset past `CODED_SLOTS` (and for how many there are).
const LATE_SLOT_BITS: u32 = 7;
const _: () = assert!(KEY_SLOTS <= CODED_SLOTS || KEY_SLOTS - CODED_SLOTS < 1 << LATE_SLOT_BITS);

/// A waypoint's quick name: its length, then each character in sixteen bits (what does not fit is a
/// question mark).
fn put_name(w: &mut Writer, a: &str, d: &str) {
    w.put(u32::from(a != d), 1);
    if a != d {
        let chars: Vec<char> = a.chars().filter(|c| !c.is_control()).take(24).collect();
        w.put(chars.len() as u32, 5);
        for c in chars {
            w.put(if (c as u32) < 0x10000 { c as u32 } else { u32::from(b'?') }, 16);
        }
    }
}

fn get_name(r: &mut Reader, d: &str) -> Option<String> {
    if r.get(1)? == 0 {
        return Some(d.to_owned());
    }
    let len = r.get(5)?;
    let mut name = String::new();
    for _ in 0..len {
        name.push(char::from_u32(r.get(16)?).filter(|c| !c.is_control()).unwrap_or('?'));
    }
    Some(name)
}

#[inline(never)]
fn put_gameplay(w: &mut Writer, g: &crate::gameplay::GameplaySettings) {
    let d = crate::gameplay::GameplaySettings::default();
    let crate::gameplay::GameplaySettings {
        toggle_sprint,
        toggle_sneak,
        always_sprint,
        time_on,
        time_preset,
        weather_on,
        weather,
        no_hurt_cam,
        hurt_off,
        hurt_strength,
        hurt_fixed,
        // The colour picker's colours and the marked spots are the person's own: not in a code.
        recent_colors: _,
        favorite_colors: _,
        hide_game_coords,
        cps_avg,
        cps_max,
        cps_total,
        waypoints: _,
        wp_compass,
        wp_world,
        wp_hidden,
        bo_on,
        mk_on,
        mk_color,
        mk_time,
        mk_middle,
        tnt_on,
        th_on,
        th_cmd,
        mk_danger_color,
        bo_line,
        bo_mode,
        bo_color,
        bo_opacity,
        bo_fill,
        bo_fix,
        nocam_on,
        noswing_on,
        cpslim_on,
        cpslim_max,
        chroma_on: _,
        chroma_speed: _,
        chroma_type: _,
        wp_on,
        wp_color,
        wp_confirm,
        wp_death,
        wp_quick_name,
        wp_beams,
        wp_borders,
        wp_shadow,
        wp_padding,
        wp_label_scale,
        wp_highlight,
        wp_line,
        wp_distance,
        wp_near,
        blur_on,
        blur_amount,
        blur_dim,
        blur_sat,
        hit_on,
        hit_armor,
        dmg_on,
        dmg_size,
        dmg_time,
        dmg_color,
        dmg_hearts,
        nametags,
        name_no_background,
        name_scale,
        name_shadow,
        name_self,
        cursor_center,
        // The Hive requeue, Perspective's switches and the Scoreboard's two options travel from version 8 on.
        hive_on,
        hive_end,
        hive_elim,
        hive_solo,
        hive_check,
        hive_hub,
        hive_log,
        hive_say,
        persp_on,
        persp_snap_toggle,
        sb_title,
        sb_horizontal,
        // Glass travels from version 10 on.
        glass_on,
        glass_mode,
        // The coloured glass style travels from version 11 on.
        glass_coloured,
    } = g;
    for (a, b) in [
        (*toggle_sprint, d.toggle_sprint),
        (*toggle_sneak, d.toggle_sneak),
        (*always_sprint, d.always_sprint),
        (*time_on, d.time_on),
        (*weather_on, d.weather_on),
        (*no_hurt_cam, d.no_hurt_cam),
        (*hurt_off, d.hurt_off),
        (*hurt_fixed, d.hurt_fixed),
        (*hide_game_coords, d.hide_game_coords),
        (*cps_avg, d.cps_avg),
        (*cps_max, d.cps_max),
        (*cps_total, d.cps_total),
        (*wp_compass, d.wp_compass),
        (*wp_world, d.wp_world),
        (*wp_hidden, d.wp_hidden),
        (*bo_on, d.bo_on),
        (*mk_on, d.mk_on),
        (*mk_middle, d.mk_middle),
        (*tnt_on, d.tnt_on),
        (*nocam_on, d.nocam_on),
        (*noswing_on, d.noswing_on),
        (*cpslim_on, d.cpslim_on),
        (*wp_on, d.wp_on),
        (*wp_confirm, d.wp_confirm),
        (*wp_death, d.wp_death),
        (*wp_beams, d.wp_beams),
        (*wp_borders, d.wp_borders),
        (*wp_shadow, d.wp_shadow),
        (*wp_highlight, d.wp_highlight),
        (*wp_distance, d.wp_distance),
        (*wp_near, d.wp_near),
        (*blur_on, d.blur_on),
        (*hit_on, d.hit_on),
        (*hit_armor, d.hit_armor),
        (*dmg_on, d.dmg_on),
        (*dmg_hearts, d.dmg_hearts),
        (*nametags, d.nametags),
        (*name_no_background, d.name_no_background),
        (*name_shadow, d.name_shadow),
        (*name_self, d.name_self),
        (*cursor_center, d.cursor_center),
    ] {
        put_bool(w, a, b);
    }
    for (a, b) in [
        (*time_preset, d.time_preset),
        (*weather, d.weather),
        (*hurt_strength, d.hurt_strength),
        (*bo_mode, d.bo_mode),
        (*bo_opacity, d.bo_opacity),
        (*bo_fill, d.bo_fill),
        (*bo_fix, d.bo_fix),
        (*mk_color, d.mk_color),
        (*mk_time, d.mk_time),
        (*mk_danger_color, d.mk_danger_color),
        (*cpslim_max, d.cpslim_max),
        (*wp_color, d.wp_color),
        (*dmg_size, d.dmg_size),
        (*dmg_time, d.dmg_time),
        (*dmg_color, d.dmg_color),
        (*name_scale, d.name_scale),
    ] {
        put_uint(w, a, b, 4);
    }
    put_color(w, *bo_color, d.bo_color);
    for (a, b) in [
        (*bo_line, d.bo_line),
        (*wp_padding, d.wp_padding),
        (*wp_label_scale, d.wp_label_scale),
        (*wp_line, d.wp_line),
        (*blur_amount, d.blur_amount),
        (*blur_dim, d.blur_dim),
        (*blur_sat, d.blur_sat),
    ] {
        put_num(w, a, b, 100.0, 15);
    }
    put_name(w, wp_quick_name, &d.wp_quick_name);
    put_bool(w, *th_on, d.th_on);
    for (a, b) in th_cmd.iter().zip(&d.th_cmd) {
        put_name(w, a, b);
    }
    // Chroma: only the elements whose chroma is not the default, each with its index.
    let dc = d.chroma(0);
    let changed: Vec<usize> = (0..Element::COUNT).filter(|i| g.chroma(*i) != dc).collect();
    w.put(changed.len() as u32, 6);
    for i in changed {
        let c = g.chroma(i);
        w.put(i as u32, 6);
        w.put(u32::from(c.on), 1);
        w.put(c.speed.round() as u32, 7);
        w.put(u32::from(c.wave), 1);
    }
    // Version 8: every option of the Hive requeue, Perspective and the Scoreboard card.
    for (a, b) in [
        (*hive_on, d.hive_on),
        (*hive_end, d.hive_end),
        (*hive_elim, d.hive_elim),
        (*hive_solo, d.hive_solo),
        (*hive_check, d.hive_check),
        (*hive_hub, d.hive_hub),
        (*hive_log, d.hive_log),
        (*hive_say, d.hive_say),
        (*persp_on, d.persp_on),
        (*persp_snap_toggle, d.persp_snap_toggle),
        (*sb_title, d.sb_title),
        (*sb_horizontal, d.sb_horizontal),
    ] {
        put_bool(w, a, b);
    }
    // Version 10: Glass.
    put_bool(w, *glass_on, d.glass_on);
    put_uint(w, *glass_mode, d.glass_mode, 2);
    // Version 11: the coloured glass section.
    put_uint(w, *glass_coloured, d.glass_coloured, 2);
}

#[inline(never)]
fn get_gameplay(r: &mut Reader, g: &mut crate::gameplay::GameplaySettings) -> Option<()> {
    let d = crate::gameplay::GameplaySettings::default();
    macro_rules! flags {
        ($($f:ident),* $(,)?) => { $( g.$f = get_bool(r, d.$f)?; )* };
    }
    flags!(
        toggle_sprint, toggle_sneak, always_sprint, time_on, weather_on, no_hurt_cam, hurt_off, hurt_fixed, hide_game_coords, cps_avg, cps_max, cps_total, wp_compass, wp_world,
        wp_hidden, bo_on, mk_on, mk_middle, tnt_on, nocam_on, noswing_on, cpslim_on, wp_on, wp_confirm, wp_death, wp_beams, wp_borders, wp_shadow, wp_highlight, wp_distance, wp_near, blur_on, hit_on, hit_armor,
        dmg_on, dmg_hearts, nametags, name_no_background, name_shadow, name_self, cursor_center,
    );
    g.time_preset = get_uint(r, d.time_preset, 4)?.min(crate::gameplay::TIME_PRESETS.len() - 1);
    g.weather = get_uint(r, d.weather, 4)?.min(crate::gameplay::WEATHER_NAMES.len() - 1);
    g.hurt_strength = get_uint(r, d.hurt_strength, 4)?.min(crate::gameplay::HURT_STRENGTHS.len() - 1);
    g.bo_mode = get_uint(r, d.bo_mode, 4)?.min(crate::outline::MODE_NAMES.len() - 1);
    g.bo_opacity = get_uint(r, d.bo_opacity, 4)?.min(crate::outline::OPACITIES.len() - 1);
    g.bo_fill = get_uint(r, d.bo_fill, 4)?.min(crate::outline::FILL_PERCENTS.len() - 1);
    g.bo_fix = get_uint(r, d.bo_fix, 4)?.min(crate::outline::FIX_PERCENTS.len() - 1);
    g.mk_color = get_uint(r, d.mk_color, 4)?.min(crate::waypoints::COLORS.len() - 1);
    g.mk_time = get_uint(r, d.mk_time, 4)?.min(crate::gameplay::MARKER_TIMES.len() - 1);
    g.mk_danger_color = get_uint(r, d.mk_danger_color, 4)?.min(crate::waypoints::COLORS.len() - 1);
    g.cpslim_max = get_uint(r, d.cpslim_max, 4)?.min(crate::handlers::CPS_LIMITS.len() - 1);
    g.wp_color = get_uint(r, d.wp_color, 4)?.min(crate::waypoints::COLORS.len() - 1);
    g.dmg_size = get_uint(r, d.dmg_size, 4)?.min(crate::damage::SIZES.len() - 1);
    g.dmg_time = get_uint(r, d.dmg_time, 4)?.min(crate::damage::TIMES.len() - 1);
    g.dmg_color = get_uint(r, d.dmg_color, 4)?.min(crate::damage::COLORS.len() - 1);
    g.name_scale = get_uint(r, d.name_scale, 4)?.min(crate::gameplay::NAME_SCALES.len() - 1);
    g.bo_color = get_color(r, d.bo_color)?;
    g.bo_line = get_num(r, d.bo_line, 100.0, 15)?.clamp(1.0, 8.0);
    g.wp_padding = get_num(r, d.wp_padding, 100.0, 15)?.clamp(2.0, 8.0);
    g.wp_label_scale = get_num(r, d.wp_label_scale, 100.0, 15)?.clamp(0.5, 3.0);
    g.wp_line = get_num(r, d.wp_line, 100.0, 15)?.clamp(1.0, 8.0);
    g.blur_amount = get_num(r, d.blur_amount, 100.0, 15)?.clamp(0.0, crate::gameplay::MAX_BLUR);
    g.blur_dim = get_num(r, d.blur_dim, 100.0, 15)?.clamp(0.0, 100.0);
    g.blur_sat = get_num(r, d.blur_sat, 100.0, 15)?.clamp(0.0, 200.0);
    g.wp_quick_name = get_name(r, &d.wp_quick_name)?;
    if r.version >= 4 {
        g.th_on = get_bool(r, d.th_on)?;
        for i in 0..4 {
            g.th_cmd[i] = crate::gameplay::clean_command(&get_name(r, &d.th_cmd[i])?);
        }
    }
    for _ in 0..r.get(6)? {
        let i = r.get(6)? as usize;
        let on = r.get(1)? == 1;
        let speed = f64::from(r.get(7)?);
        let wave = r.get(1)? == 1;
        if i < Element::COUNT {
            g.set_chroma_on(i, on);
            g.set_chroma_speed(i, speed);
            g.set_chroma_type(i, usize::from(!wave));
        }
    }
    // Codes before version 8 have none of these: the caller keeps the person's own (see `setups.rs`).
    if r.version >= 8 {
        flags!(hive_on, hive_end, hive_elim, hive_solo, hive_check, hive_hub, hive_log, hive_say, persp_on, persp_snap_toggle, sb_title, sb_horizontal);
    }
    // Codes before version 10 have no Glass: the caller keeps the person's own (see `setups.rs`).
    if r.version >= 10 {
        g.glass_on = get_bool(r, d.glass_on)?;
        g.glass_mode = crate::glass::clamp(get_uint(r, d.glass_mode, 2)?);
    }
    // Before version 11 one style covered both kinds of glass.
    g.glass_coloured = if r.version >= 11 { crate::glass::clamp(get_uint(r, d.glass_coloured, 2)?) } else { g.glass_mode };
    Some(())
}

#[inline(never)]
fn get_last(r: &mut Reader, s: &mut Settings) -> Option<()> {
    let d = Settings::default();
    if r.get(1)? == 1 {
        let has_pvp = r.version >= 4;
        for element in last_elements().iter().copied().filter(|e| has_pvp || *e != Element::PvpInfo) {
            let ed = element.default_style();
            s.elements[element.index()] = get_element(r, element)?;
            get_box_extras(r, &mut s.elements[element.index()], &ed)?;
        }
    }
    if r.get(1)? == 1 {
        s.item_delay_fix = get_bool(r, d.item_delay_fix)?;
        s.sb_scores = get_bool(r, d.sb_scores)?;
        s.waila_id = get_bool(r, d.waila_id)?;
        s.waila_pos = get_bool(r, d.waila_pos)?;
        s.waila_mod = get_bool(r, d.waila_mod)?;
        s.waila_growth = get_bool(r, d.waila_growth)?;
        s.waila_tool = get_bool(r, d.waila_tool)?;
        s.waila_dist = get_bool(r, d.waila_dist)?;
        s.waila_state = get_bool(r, d.waila_state)?;
        s.waila_entity = get_bool(r, d.waila_entity)?;
        s.waila_mod_color = get_color(r, d.waila_mod_color)?;
        s.waila_info_color = get_color(r, d.waila_info_color)?;
        s.waila_good_color = get_color(r, d.waila_good_color)?;
        s.waila_bad_color = get_color(r, d.waila_bad_color)?;
        s.va_yaw = get_bool(r, d.va_yaw)?;
        s.va_pitch = get_bool(r, d.va_pitch)?;
        s.va_speed = get_bool(r, d.va_speed)?;
        s.va_altitude = get_bool(r, d.va_altitude)?;
        s.va_first = get_bool(r, d.va_first)?;
        s.va_third = get_bool(r, d.va_third)?;
        s.va_elytra = get_bool(r, d.va_elytra)?;
        s.va_reverse = get_bool(r, d.va_reverse)?;
        s.va_target = get_bool(r, d.va_target)?;
        s.va_decimals = get_uint(r, d.va_decimals, 3)?.min(crate::config::VA_MAX_DECIMALS);
        s.va_align = get_uint(r, d.va_align, 2)?.min(crate::config::VA_ALIGN.len() - 1);
        s.va_label_color = get_color(r, d.va_label_color)?;
        s.va_value_color = get_color(r, d.va_value_color)?;
        s.co_x = get_bool(r, d.co_x)?;
        s.co_y = get_bool(r, d.co_y)?;
        s.co_z = get_bool(r, d.co_z)?;
        s.co_dir = get_bool(r, d.co_dir)?;
        s.co_decimal = get_bool(r, d.co_decimal)?;
        s.co_horizontal = get_bool(r, d.co_horizontal)?;
        s.co_x_label = get_bool(r, d.co_x_label)?;
        s.co_y_label = get_bool(r, d.co_y_label)?;
        s.co_z_label = get_bool(r, d.co_z_label)?;
        s.co_dir_cardinal = get_bool(r, d.co_dir_cardinal)?;
        s.co_dir_mod = get_bool(r, d.co_dir_mod)?;
        s.co_x_lc = get_color(r, d.co_x_lc)?;
        s.co_x_tc = get_color(r, d.co_x_tc)?;
        s.co_y_lc = get_color(r, d.co_y_lc)?;
        s.co_y_tc = get_color(r, d.co_y_tc)?;
        s.co_z_lc = get_color(r, d.co_z_lc)?;
        s.co_z_tc = get_color(r, d.co_z_tc)?;
        s.co_dir_tc = get_color(r, d.co_dir_tc)?;
        s.co_dir_mx = get_color(r, d.co_dir_mx)?;
        s.co_dir_mz = get_color(r, d.co_dir_mz)?;
        s.waila_details = get_uint(r, d.waila_details, 2)?.min(crate::config::WAILA_DETAILS.len() - 1);
        if r.version >= 4 {
            s.pvp_hits_lc = get_color(r, d.pvp_hits_lc)?;
            s.pvp_acc_lc = get_color(r, d.pvp_acc_lc)?;
            s.pvp_kills_lc = get_color(r, d.pvp_kills_lc)?;
            s.pvp_deaths_lc = get_color(r, d.pvp_deaths_lc)?;
            s.pvp_num_tc = get_color(r, d.pvp_num_tc)?;
            s.pvp_pct_tc = get_color(r, d.pvp_pct_tc)?;
        }
        if r.version >= 5 {
            s.inv_empty = get_bool(r, d.inv_empty)?;
            s.inv_show_bg = get_bool(r, d.inv_show_bg)?;
            s.inv_mc = get_bool(r, d.inv_mc)?;
            s.inv_grid = get_bool(r, d.inv_grid)?;
            s.inv_grid_color = get_color(r, d.inv_grid_color)?;
        }
        if r.version >= 6 {
            s.inv_grid_opacity = get_num(r, d.inv_grid_opacity, 100.0, 14)?;
        }
        if r.version >= 7 {
            s.facing_label_chroma = get_bool(r, d.facing_label_chroma)?;
        }
    }
    if r.get(1)? == 1 {
        get_gameplay(r, &mut s.gameplay)?;
    }
    if r.get(1)? == 1 {
        for _ in 0..r.get(LATE_SLOT_BITS)? {
            let i = r.get(LATE_SLOT_BITS)? as usize + CODED_SLOTS;
            let key = get_key(r)?;
            if let Some(id) = key_slot_id(i) {
                s.set_key(id, &key);
            }
        }
    }
    Some(())
}

fn put_crosshair(w: &mut Writer, c: &CrosshairSettings, d: &CrosshairSettings) {
    let CrosshairSettings {
        enabled,
        third_person,
        shape,
        size,
        gap,
        thickness,
        dot,
        dot_round,
        color,
        opacity,
        outline,
        outline_color,
        outline_round,
        outline_width,
        round_lines,
        on_target,
        players,
        mobs,
        player_color,
        mob_color,
        target_scale,
        target_opacity,
        weapon,
        weapon_shape,
        weapon_size,
        weapon_gap,
        weapon_thickness,
    } = c;
    put_bool(w, *enabled, d.enabled);
    put_bool(w, *third_person, d.third_person);
    put_uint(w, *shape, d.shape, 2);
    put_num(w, *size, d.size, 100.0, 11);
    put_num(w, *gap, d.gap, 100.0, 11);
    put_num(w, *thickness, d.thickness, 100.0, 11);
    put_bool(w, *dot, d.dot);
    put_bool(w, *dot_round, d.dot_round);
    put_color(w, *color, d.color);
    put_num(w, *opacity, d.opacity, 100.0, 14);
    put_bool(w, *outline, d.outline);
    put_color(w, *outline_color, d.outline_color);
    put_bool(w, *outline_round, d.outline_round);
    put_num(w, *outline_width, d.outline_width, 100.0, 10);
    put_bool(w, *round_lines, d.round_lines);
    put_bool(w, *on_target, d.on_target);
    put_bool(w, *players, d.players);
    put_bool(w, *mobs, d.mobs);
    put_color(w, *player_color, d.player_color);
    put_color(w, *mob_color, d.mob_color);
    put_num(w, *target_scale, d.target_scale, 100.0, 8);
    put_num(w, *target_opacity, d.target_opacity, 100.0, 14);
    put_bool(w, *weapon, d.weapon);
    put_uint(w, *weapon_shape, d.weapon_shape, 2);
    put_num(w, *weapon_size, d.weapon_size, 100.0, 11);
    put_num(w, *weapon_gap, d.weapon_gap, 100.0, 11);
    put_num(w, *weapon_thickness, d.weapon_thickness, 100.0, 11);
}

fn get_crosshair(r: &mut Reader, d: &CrosshairSettings) -> Option<CrosshairSettings> {
    Some(CrosshairSettings {
        enabled: get_bool(r, d.enabled)?,
        third_person: get_bool(r, d.third_person)?,
        shape: get_uint(r, d.shape, 2)?,
        size: get_num(r, d.size, 100.0, 11)?,
        gap: get_num(r, d.gap, 100.0, 11)?,
        thickness: get_num(r, d.thickness, 100.0, 11)?,
        dot: get_bool(r, d.dot)?,
        dot_round: get_bool(r, d.dot_round)?,
        color: get_color(r, d.color)?,
        opacity: get_num(r, d.opacity, 100.0, 14)?,
        outline: get_bool(r, d.outline)?,
        outline_color: get_color(r, d.outline_color)?,
        outline_round: get_bool(r, d.outline_round)?,
        outline_width: get_num(r, d.outline_width, 100.0, 10)?,
        round_lines: get_bool(r, d.round_lines)?,
        on_target: get_bool(r, d.on_target)?,
        players: get_bool(r, d.players)?,
        mobs: get_bool(r, d.mobs)?,
        player_color: get_color(r, d.player_color)?,
        mob_color: get_color(r, d.mob_color)?,
        target_scale: get_num(r, d.target_scale, 100.0, 8)?,
        target_opacity: get_num(r, d.target_opacity, 100.0, 14)?,
        weapon: get_bool(r, d.weapon)?,
        weapon_shape: get_uint(r, d.weapon_shape, 2)?,
        weapon_size: get_num(r, d.weapon_size, 100.0, 11)?,
        weapon_gap: get_num(r, d.weapon_gap, 100.0, 11)?,
        weapon_thickness: get_num(r, d.weapon_thickness, 100.0, 11)?,
    })
}

fn put_zoom(w: &mut Writer, z: &ZoomSettings, d: &ZoomSettings) {
    let ZoomSettings {
        enabled,
        initial_zoom,
        zoom_in_time,
        zoom_out_time,
        zoom_in_transition,
        zoom_out_transition,
        step_keys,
        scroll_step_count,
        zoom_per_step,
        scroll_zoom_smoothness,
        retain_zoom_steps,
        relative_sensitivity,
        key_mode,
        keys,
    } = z;
    let changed = z != d;
    w.put(u32::from(changed), 1);
    if !changed {
        return;
    }
    put_bool(w, *enabled, d.enabled);
    put_num(w, *initial_zoom, d.initial_zoom, 100.0, 11);
    put_num(w, *zoom_in_time, d.zoom_in_time, 100.0, 9);
    put_num(w, *zoom_out_time, d.zoom_out_time, 100.0, 9);
    put_uint(w, transition_index(*zoom_in_transition), transition_index(d.zoom_in_transition), 4);
    put_uint(w, transition_index(*zoom_out_transition), transition_index(d.zoom_out_transition), 4);
    put_bool(w, *step_keys, d.step_keys);
    put_uint(w, *scroll_step_count as usize, d.scroll_step_count as usize, 5);
    put_num(w, *zoom_per_step, d.zoom_per_step, 100.0, 15);
    put_num(w, *scroll_zoom_smoothness, d.scroll_zoom_smoothness, 100.0, 14);
    put_bool(w, *retain_zoom_steps, d.retain_zoom_steps);
    put_num(w, *relative_sensitivity, d.relative_sensitivity, 100.0, 16);
    put_uint(w, key_mode.index(), d.key_mode.index(), 1);
    for (a, b) in [(&keys.zoom, &d.keys.zoom), (&keys.step_in, &d.keys.step_in), (&keys.step_out, &d.keys.step_out)] {
        let changed = a != b;
        w.put(u32::from(changed), 1);
        if changed {
            put_key(w, a);
        }
    }
}

/// The settings a code's options describe, with every element at its own default (the elements
/// are read next, one at a time).
#[inline(never)]
pub fn get_options(r: &mut Reader) -> Option<Settings> {
    let d = Settings::default();
    let mut s = Settings::default();
    s.borders = get_bool(r, d.borders)?;
    s.subchunks = get_bool(r, d.subchunks)?;
    s.slime_chunks = get_bool(r, d.slime_chunks)?;
    s.guardians = get_bool(r, d.guardians)?;
    s.lights = get_bool(r, d.lights)?;
    s.blaze = get_bool(r, d.blaze)?;
    s.hitboxes = get_bool(r, d.hitboxes)?;
    s.fullbright = get_bool(r, d.fullbright)?;
    s.hide_scoreboard = get_bool(r, d.hide_scoreboard)?;
    s.show_mouse = get_bool(r, d.show_mouse)?;
    s.show_space = get_bool(r, d.show_space)?;
    s.align = get_bool(r, d.align)?;
    s.show_guides = get_bool(r, d.show_guides)?;
    s.grid = Grid::from_index(get_uint(r, d.grid.index(), 2)?);
    s.cps_mode = CpsMode::from_index(get_uint(r, d.cps_mode.index(), 2)?);
    s.hitbox_color = get_color(r, d.hitbox_color)?;
    s.hitbox_player_color = get_color(r, d.hitbox_player_color)?;
    s.hit_mode = get_uint(r, d.hit_mode, 2)?;
    s.hit_color = get_color(r, d.hit_color)?;
    s.hit_opacity = get_num(r, d.hit_opacity, 100.0, 14)?;
    s.line_width = get_num(r, d.line_width, 100.0, 9)?;
    // Version 3 on names a slot (and counts the bound ones) in six bits, older codes in five.
    let slot_bits = if r.version >= 3 { SLOT_BITS } else { 5 };
    let count = r.get(slot_bits)?;
    for _ in 0..count {
        let i = r.get(slot_bits)? as usize;
        let key = get_key(r)?;
        if let Some(id) = key_slot_id(i) {
            s.set_key(id, &key);
        }
    }
    s.zoom = get_zoom(r, &d.zoom)?;
    if r.version >= 12 && r.get(1)? == 1 {
        let count = r.get(SKIP_BITS)?;
        let mut skipped = Vec::new();
        for n in 0..count {
            if r.get(1)? == 1 {
                skipped.push(n as u8);
            }
        }
        // A number a later version added names a mod this one does not have: nothing to keep for it.
        r.skipped = crate::share::clean(&skipped);
    }
    Some(s)
}

fn get_zoom(r: &mut Reader, d: &ZoomSettings) -> Option<ZoomSettings> {
    if r.get(1)? == 0 {
        return Some(d.clone());
    }
    let enabled = get_bool(r, d.enabled)?;
    let initial_zoom = get_num(r, d.initial_zoom, 100.0, 11)?;
    let zoom_in_time = get_num(r, d.zoom_in_time, 100.0, 9)?;
    let zoom_out_time = get_num(r, d.zoom_out_time, 100.0, 9)?;
    let zoom_in_transition = transition_at(get_uint(r, transition_index(d.zoom_in_transition), 4)?);
    let zoom_out_transition = transition_at(get_uint(r, transition_index(d.zoom_out_transition), 4)?);
    let step_keys = get_bool(r, d.step_keys)?;
    let scroll_step_count = get_uint(r, d.scroll_step_count as usize, 5)? as u32;
    let zoom_per_step = get_num(r, d.zoom_per_step, 100.0, 15)?;
    let scroll_zoom_smoothness = get_num(r, d.scroll_zoom_smoothness, 100.0, 14)?;
    let retain_zoom_steps = get_bool(r, d.retain_zoom_steps)?;
    let relative_sensitivity = get_num(r, d.relative_sensitivity, 100.0, 16)?;
    let key_mode = KeyMode::from_index(get_uint(r, d.key_mode.index(), 1)?);
    let mut keys = ZoomKeys { zoom: d.keys.zoom.clone(), step_in: d.keys.step_in.clone(), step_out: d.keys.step_out.clone() };
    for (slot, other) in [(&mut keys.zoom, &d.keys.zoom), (&mut keys.step_in, &d.keys.step_in), (&mut keys.step_out, &d.keys.step_out)] {
        *slot = if r.get(1)? == 1 { get_key(r)? } else { other.clone() };
    }
    Some(ZoomSettings {
        enabled,
        initial_zoom,
        zoom_in_time,
        zoom_out_time,
        zoom_in_transition,
        zoom_out_transition,
        step_keys,
        scroll_step_count,
        zoom_per_step,
        scroll_zoom_smoothness,
        retain_zoom_steps,
        relative_sensitivity,
        key_mode,
        keys,
    })
}

/// A whole settings value as a code in one go (the steps above are for the game, which spreads
/// them over frames).
pub fn encode(s: &Settings) -> String {
    let mut w = Writer::new();
    put_options(&mut w, s);
    for element in &Element::ALL[..Element::FIRST] {
        put_element(&mut w, *element, s.style(*element));
    }
    put_extras(&mut w, s);
    w.finish()
}

/// A code that leaves the mods in `skip` out: their settings are written as the defaults and the code names
/// them (the game does the same in steps, see `setups.rs`).
#[cfg(test)]
pub fn encode_part(s: &Settings, skip: &[u8]) -> String {
    let mut part = s.clone();
    crate::share::copy_all(&mut part, &Settings::default(), skip);
    let mut w = Writer::new();
    w.skip = skip.to_vec();
    put_options(&mut w, &part);
    for element in &Element::ALL[..Element::FIRST] {
        put_element(&mut w, *element, part.style(*element));
    }
    put_extras(&mut w, &part);
    w.finish()
}

/// The settings a code describes, repaired like any loaded settings (the saved setups and the menu
/// key are the default's: the caller keeps its own).
pub fn decode(text: &str) -> Option<Settings> {
    decode_part(text).map(|(settings, _)| settings)
}

/// `decode`, with the mods the code leaves out (their settings in the result are the defaults).
pub fn decode_part(text: &str) -> Option<(Settings, Vec<u8>)> {
    let mut r = Reader::new(text)?;
    let mut s = get_options(&mut r)?;
    s.elements = Vec::with_capacity(Element::COUNT);
    for element in Element::ALL[..Element::FIRST].iter().copied() {
        // Codes made before Speed, Stopwatch and Playtime existed end after Ping: those three keep
        // their defaults. The elements added after them are not here at all: they have a section
        // of their own at the end (`put_extras`).
        let got = get_element(&mut r, element);
        s.elements.push(if element.index() >= 11 { got.unwrap_or_else(|| element.default_style()) } else { got? });
    }
    for element in Element::ALL[Element::FIRST..Element::CODED].iter().copied() {
        s.elements.push(element.default_style());
    }
    get_extras(&mut r, &mut s)?;
    s.sanitize();
    Some((s, r.skipped))
}

/// The range an armor piece's size can take, so the quantised value is always inside it.
const _: () = assert!(ARMOR_MAX_SIZE <= 5.0 && ARMOR_MIN_SIZE >= 0.0);
const _: () = assert!(MAX_ARMOR_OFFSET <= 6553.0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_a_very_short_code() {
        let code = encode(&Settings::default());
        assert!(code.len() < 24, "{code}");
        let back = decode(&code).unwrap();
        assert_eq!(back, Settings::default());
    }

    fn busy() -> Settings {
        let mut s = Settings::default();
        s.borders = true;
        s.grid = Grid::Five;
        s.line_width = 3.5;
        s.hit_mode = 2;
        s.hit_opacity = 42.5;
        s.set_key("borders", "KeyB");
        s.set_key("hud_fps", "F9");
        s.set_key("hitboxes", "Numpad7");
        s.set_key("sw_pause", "KeyP");
        s.set_key("sw_reset", "KeyO");
        s.speed_unit = 1;
        s.speed_decimals = 3;
        s.speed_vertical = true;
        s.stopwatch_keep = true;
        s.playtime_format = 1;
        s.playtime_world_only = true;
        s.reach_hide_zero = true;
        s.reach_highlight = true;
        s.reach_highlight_color = 6;
        s.zoom.enabled = true;
        s.zoom.initial_zoom = 6.5;
        s.zoom.keys.zoom = "KeyX".into();
        s.zoom.keys.step_out = String::new();
        for element in Element::ALL {
            let style = s.style_mut(element);
            style.enabled = true;
            style.scale = 1.25;
            style.stretch = [1.5, 1.25];
            style.position = Some([0.25, 0.75]);
            style.bg_color = 5;
            style.text_color = 6;
            style.opacity = 33.0;
            style.brackets = true;
            style.shadow = false;
            style.auto_width = Some(true);
            style.active_color = Some(3);
            style.armor_split = true;
            style.armor_gap = 4.5;
            style.armor_offsets = [[1.5, 2.5], [10.0, 20.0], [0.0, 30.0], [40.0, 0.5], [100.0, 120.0]];
            style.armor_sizes = [[1.5, 1.0], [0.5, 2.0], [1.0, 1.0], [3.0, 3.0], [1.25, 1.75]];
            style.armor_pieces = [true, false, true, false];
        }
        s
    }

    #[test]
    fn a_heavily_changed_setup_round_trips() {
        let s = busy();
        let code = encode(&s);
        let back = decode(&code).unwrap();
        let mut want = s.clone();
        want.sanitize();
        // Positions are twelve-bit fractions: compare to that grid.
        for element in Element::ALL {
            let (a, b) = (want.style(element).position.unwrap(), back.style(element).position.unwrap());
            assert!((a[0] - b[0]).abs() < 0.001 && (a[1] - b[1]).abs() < 0.001);
        }
        let mut got = back.clone();
        for element in Element::ALL {
            got.style_mut(element).position = want.style(element).position;
        }
        assert_eq!(got, want);
        println!("a setup with every element changed: {} characters", code.len());
    }

    #[test]
    fn the_gameplay_mods_waila_and_the_late_elements_round_trip() {
        let mut s = Settings::default();
        s.item_delay_fix = true;
        s.sb_scores = true;
        s.waila_id = true;
        s.waila_mod = false;
        s.waila_bad_color = custom_color(0x123456);
        s.elements[Element::Waila.index()].enabled = true;
        s.elements[Element::Scoreboard.index()].scale = 1.5;
        s.elements[Element::Waypoints.index()].border = !s.elements[Element::Waypoints.index()].border;
        s.elements[Element::AudioSubtitles.index()].enabled = true;
        s.elements[Element::HeightLimit.index()].scale = 1.25;
        s.set_key("mk_place", "KeyM");
        let g = &mut s.gameplay;
        g.toggle_sprint = true;
        g.time_on = true;
        g.time_preset = 4;
        g.hurt_off = false;
        g.hurt_strength = 1;
        g.bo_on = true;
        g.bo_line = 3.5;
        g.bo_color = 5;
        g.bo_fill = 3;
        g.cpslim_on = true;
        g.cpslim_max = 5;
        g.wp_quick_name = "Base \u{e9}".into();
        g.th_on = true;
        g.th_cmd[1] = "/home base".into();
        g.wp_padding = 6.0;
        g.wp_label_scale = 1.5;
        g.blur_amount = 12.0;
        g.blur_sat = 150.0;
        g.dmg_on = true;
        g.dmg_color = 4;
        g.nametags = true;
        g.name_scale = 3;
        g.mk_on = true;
        g.mk_color = 3;
        g.mk_time = 2;
        g.set_chroma_on(Element::Fps.index(), true);
        g.set_chroma_speed(Element::Fps.index(), 70.0);
        g.set_chroma_type(Element::Fps.index(), 1);
        // The person's own lists stay out of a code.
        g.recent_colors = vec![1, 2];
        g.favorite_colors = vec![3];
        s.set_key("hud_waila", "KeyJ");
        s.set_key("gp_toggle_sprint", "KeyV");
        let back = decode(&encode(&s)).unwrap();
        let mut want = s.clone();
        want.sanitize();
        want.gameplay.recent_colors = Vec::new();
        want.gameplay.favorite_colors = Vec::new();
        for element in Element::ALL {
            let (a, b) = (want.style(element).position, back.style(element).position);
            if let (Some(a), Some(b)) = (a, b) {
                assert!((a[0] - b[0]).abs() < 0.001 && (a[1] - b[1]).abs() < 0.001);
            }
        }
        let mut got = back.clone();
        for element in Element::ALL {
            got.style_mut(element).position = want.style(element).position;
        }
        assert_eq!(got, want);
        assert_eq!(back.key("hud_waila"), "KeyJ");
        assert_eq!(back.key("mk_place"), "KeyM");
    }

    #[test]
    fn one_late_option_keeps_the_code_short_and_leaves_the_rest_alone() {
        let mut s = Settings::default();
        s.gameplay.toggle_sprint = true;
        let code = encode(&s);
        assert!(code.len() < 60, "{code}");
        let back = decode(&code).unwrap();
        assert_eq!(back, s);
        // A code made before the last section (padding where it would be) still loads.
        let mut old = Settings::default();
        old.sat_overlay = !old.sat_overlay;
        assert!(decode(&encode(&old)).unwrap().gameplay == Settings::default().gameplay);
    }

    #[test]
    fn the_hive_perspective_and_scoreboard_options_travel_in_a_code() {
        let mut s = Settings::default();
        let g = &mut s.gameplay;
        for field in [
            &mut g.hive_on, &mut g.hive_end, &mut g.hive_elim, &mut g.hive_solo, &mut g.hive_check, &mut g.hive_hub, &mut g.hive_log, &mut g.hive_say,
            &mut g.persp_on, &mut g.persp_snap_toggle, &mut g.sb_title, &mut g.sb_horizontal,
        ] {
            *field = !*field;
        }
        let back = decode(&encode(&s)).expect("a code of this version reads");
        let (a, b) = (&back.gameplay, &s.gameplay);
        assert_eq!(
            (a.hive_on, a.hive_end, a.hive_elim, a.hive_solo, a.hive_check, a.hive_hub, a.hive_log, a.hive_say),
            (b.hive_on, b.hive_end, b.hive_elim, b.hive_solo, b.hive_check, b.hive_hub, b.hive_log, b.hive_say)
        );
        assert_eq!((a.persp_on, a.persp_snap_toggle, a.sb_title, a.sb_horizontal), (b.persp_on, b.persp_snap_toggle, b.sb_title, b.sb_horizontal));
        // An untouched setup still has nothing to carry.
        assert!(!gameplay_changed(&Settings::default().gameplay));
    }

    #[test]
    fn every_setting_a_code_carries_belongs_to_a_mod() {
        let mut theirs = busy();
        theirs.gameplay.glass_on = true;
        theirs.gameplay.mk_time = 3;
        theirs.gameplay.hive_on = true;
        theirs.gameplay.wp_beams = !theirs.gameplay.wp_beams;
        theirs.gameplay.set_chroma_on(Element::Fps.index(), true);
        theirs.crosshair.enabled = true;
        theirs.sb_scores = !theirs.sb_scores;
        theirs.item_delay_fix = true;
        theirs.set_key("persp_snap", "KeyV");
        let theirs = decode(&encode(&theirs)).unwrap();
        // Every mod's settings copied onto the defaults is the whole setup again...
        let all: Vec<u8> = (0..crate::share::MODS.len() as u8).collect();
        let mut rebuilt = Settings::default();
        crate::share::copy_all(&mut rebuilt, &theirs, &all);
        // ...but for what no mod owns: the editor's guides and the retired Arrow counter's place.
        rebuilt.show_guides = theirs.show_guides;
        *rebuilt.style_mut(Element::Arrows) = theirs.style(Element::Arrows).clone();
        assert_eq!(encode(&rebuilt), encode(&theirs), "a setting that travels in a code belongs to no mod in share.rs");
    }

    #[test]
    fn a_code_can_leave_mods_out_and_a_load_keeps_your_own_for_them() {
        let theirs = decode(&encode(&busy())).unwrap();
        let zoom = crate::share::position("zoom_on").unwrap() as u8;
        let fps = crate::share::position("hud_fps").unwrap() as u8;
        let code = encode_part(&theirs, &[zoom, fps]);
        let (mut got, skipped) = decode_part(&code).unwrap();
        assert_eq!(skipped, crate::share::clean(&[zoom, fps]));
        // The left-out mods read as the defaults, everything else as it was shared.
        assert_eq!(got.zoom, Settings::default().zoom);
        assert_eq!(got.key("hud_fps"), "");
        assert_eq!(*got.style(Element::Fps), Element::Fps.default_style());
        assert_eq!(got.style(Element::Coords), theirs.style(Element::Coords));
        assert_eq!(got.key("borders"), "KeyB");
        // The loader's own settings fill the gaps.
        let mut mine = Settings::default();
        mine.zoom.initial_zoom = 3.0;
        mine.set_key("hud_fps", "KeyK");
        crate::share::copy_all(&mut got, &mine, &skipped);
        assert_eq!(got.zoom.initial_zoom, 3.0);
        assert_eq!(got.key("hud_fps"), "KeyK");
        assert_eq!(got.borders, theirs.borders);
        // A code that leaves nothing out names nothing.
        assert!(decode_part(&encode(&theirs)).unwrap().1.is_empty());
        // Leaving everything out is still a code that loads, and it changes nothing.
        let all: Vec<u8> = (0..crate::share::MODS.len() as u8).collect();
        let (mut nothing, skipped) = decode_part(&encode_part(&theirs, &all)).unwrap();
        assert_eq!(skipped, all);
        crate::share::copy_all(&mut nothing, &mine, &skipped);
        nothing.show_guides = mine.show_guides;
        assert_eq!(encode(&nothing), encode(&mine));
    }

    #[test]
    fn the_glass_style_travels_in_a_code() {
        let mut s = Settings::default();
        s.gameplay.glass_on = true;
        s.gameplay.glass_mode = crate::glass::FRAMELESS;
        let back = decode(&encode(&s)).expect("a code of this version reads");
        assert_eq!((back.gameplay.glass_on, back.gameplay.glass_mode), (true, crate::glass::FRAMELESS));
        s.gameplay.glass_mode = crate::glass::NORMAL;
        let back = decode(&encode(&s)).expect("a code of this version reads");
        assert_eq!(back.gameplay.glass_mode, crate::glass::NORMAL);
        s.gameplay.glass_mode = crate::glass::CLEAR;
        let back = decode(&encode(&s)).expect("a code of this version reads");
        assert_eq!(back.gameplay.glass_mode, crate::glass::CLEAR);
        s.gameplay.glass_coloured = crate::glass::FRAMELESS;
        let back = decode(&encode(&s)).expect("a code of this version reads");
        assert_eq!((back.gameplay.glass_mode, back.gameplay.glass_coloured), (crate::glass::CLEAR, crate::glass::FRAMELESS));
        assert!(gameplay_changed(&s.gameplay));
    }

    #[test]
    fn a_colours_transparency_travels_in_a_code() {
        let mut s = Settings::default();
        s.crosshair.color = crate::config::custom_color_alpha(0x12AB9F, 40);
        s.crosshair.outline_color = crate::config::custom_color(0x445566);
        let back = decode(&encode(&s)).expect("a code of this version reads");
        assert_eq!(back.crosshair.color, s.crosshair.color);
        assert_eq!(crate::config::custom_transparency(back.crosshair.color), 40);
        assert_eq!(back.crosshair.outline_color, crate::config::custom_color(0x445566));
        assert_eq!(crate::config::custom_rgb(back.crosshair.color), Some(0x12AB9F));
    }

    #[test]
    fn the_extra_options_and_the_stopwatch_keys_round_trip() {
        let mut s = Settings::default();
        s.speed_decimals = 0;
        s.speed_vertical = true;
        s.playtime_format = 1;
        s.set_key("sw_reset", "KeyR");
        let back = decode(&encode(&s)).unwrap();
        assert_eq!((back.speed_unit, back.speed_decimals, back.speed_vertical), (0, 0, true));
        assert_eq!((back.stopwatch_keep, back.playtime_format, back.playtime_world_only), (false, 1, false));
        assert_eq!((back.key("sw_pause"), back.key("sw_reset")), ("", "KeyR"));
        // Changing nothing costs one bit, and an old option's key keeps its slot.
        let mut plain = Settings::default();
        plain.set_key("hide_scoreboard", "KeyH");
        assert_eq!(decode(&encode(&plain)).unwrap().key("hide_scoreboard"), "KeyH");
    }

    #[test]
    fn reach_options_round_trip_alone_and_leave_other_codes_unchanged() {
        // Alone: no crosshair, borders or later elements ahead of them in the code.
        let mut s = Settings::default();
        s.reach_highlight = true;
        s.reach_highlight_color = custom_color(0x12_34_56);
        let back = decode(&encode(&s)).unwrap();
        assert!(back.reach_highlight && !back.reach_hide_zero);
        assert_eq!(back.reach_highlight_color, custom_color(0x12_34_56));
        // With the later elements and a crosshair changed too.
        s.reach_hide_zero = true;
        s.crosshair.enabled = true;
        s.style_mut(Element::Reach).enabled = true;
        let back = decode(&encode(&s)).unwrap();
        assert!(back.reach_hide_zero && back.reach_highlight && back.style(Element::Reach).enabled && back.crosshair.enabled);
        // At their defaults they cost nothing: the code is what it was before them.
        let mut plain = Settings::default();
        plain.speed_decimals = 0;
        let mut same = plain.clone();
        same.reach_highlight_color = crate::config::GREEN;
        assert_eq!(encode(&plain), encode(&same));
        assert!(!decode(&encode(&plain)).unwrap().reach_highlight);
        // A cut-off code that has them is refused.
        let code = encode(&busy());
        assert!(decode(&code[..code.len() - 6]).is_none());
    }

    #[test]
    fn moved_potion_effects_round_trip() {
        let mut s = Settings::default();
        s.potion_split = true;
        s.potion_offsets[1] = [-30.0, 12.0];
        s.potion_offsets[36] = [100.0, -1200.0];
        s.potion_scales[36] = 1.75;
        let back = decode(&encode(&s)).unwrap();
        assert!(back.potion_split);
        assert_eq!(back.potion_offsets[1], [-30.0, 12.0]);
        assert_eq!(back.potion_offsets[36], [100.0, -1200.0]);
        assert_eq!(back.potion_offsets[2], [0.0, 0.0]);
        assert_eq!(back.potion_scales[36], 1.75);
        assert_eq!(back.potion_scales[1], 1.0);
        assert_eq!(back.potion_offsets.len(), crate::config::POTION_SLOTS);
    }

    #[test]
    fn saturation_options_round_trip_and_cost_nothing_at_their_defaults() {
        let mut s = Settings::default();
        s.sat_inverse = true;
        s.sat_overlay = false;
        s.sat_color = custom_color(0x12_34_56);
        s.sat_layout = 1;
        let back = decode(&encode(&s)).unwrap();
        assert!(back.sat_inverse && !back.sat_overlay && back.sat_hunger && back.sat_potential && back.sat_layout == 1);
        assert_eq!(back.sat_color, custom_color(0x12_34_56));
        // The two previews switch off and back on independently of the overlay.
        let mut previews = Settings::default();
        previews.sat_hunger = false;
        let back = decode(&encode(&previews)).unwrap();
        assert!(back.sat_overlay && !back.sat_hunger && back.sat_potential && !back.sat_inverse);
        previews.sat_hunger = true;
        previews.sat_potential = false;
        let back = decode(&encode(&previews)).unwrap();
        assert!(back.sat_hunger && !back.sat_potential);
        // With Reach's options and a crosshair as well: both sections read back.
        s.reach_highlight = true;
        s.crosshair.enabled = true;
        let back = decode(&encode(&s)).unwrap();
        assert!(back.reach_highlight && back.crosshair.enabled && back.sat_inverse && !back.sat_overlay);
        // Reach's options alone still load, with Saturation's at their defaults.
        let mut only_reach = Settings::default();
        only_reach.reach_hide_zero = true;
        let back = decode(&encode(&only_reach)).unwrap();
        assert!(back.reach_hide_zero && back.sat_overlay && !back.sat_inverse);
        // At their defaults they add nothing.
        let mut same = Settings::default();
        same.sat_color = crate::config::SAT_GOLD;
        assert_eq!(encode(&Settings::default()), encode(&same));
        // A cut-off code that has them is refused.
        let mut full = busy();
        full.sat_inverse = true;
        let code = encode(&full);
        assert!(decode(&code[..code.len() - 6]).is_none());
    }

    #[test]
    fn the_crosshair_round_trips_and_costs_nothing_while_it_is_at_its_defaults() {
        let plain = Settings::default();
        let with_default_crosshair = encode(&plain);
        // The same code as before the crosshair existed: it is only written when it differs.
        let mut w = Writer::new();
        put_options(&mut w, &plain);
        for element in Element::ALL[..Element::FIRST].iter().copied() {
            put_element(&mut w, element, plain.style(element));
        }
        w.put(0, 1);
        assert_eq!(with_default_crosshair, w.finish());
        let mut s = Settings::default();
        s.speed_decimals = 3;
        s.crosshair.enabled = true;
        s.crosshair.shape = 3;
        s.crosshair.size = 7.5;
        s.crosshair.gap = 0.0;
        s.crosshair.thickness = 2.5;
        s.crosshair.color = 4;
        s.crosshair.opacity = 62.5;
        s.crosshair.outline = true;
        s.crosshair.outline_round = true;
        s.crosshair.outline_width = 2.5;
        s.crosshair.round_lines = true;
        s.crosshair.third_person = true;
        s.crosshair.dot_round = true;
        s.crosshair.mobs = false;
        s.crosshair.target_scale = 1.4;
        s.crosshair.weapon = true;
        s.crosshair.weapon_shape = 2;
        s.crosshair.weapon_thickness = 3.0;
        for with_extras in [true, false] {
            if !with_extras {
                s.speed_decimals = Settings::default().speed_decimals;
            }
            let back = decode(&encode(&s)).expect("a code with a crosshair decodes");
            assert_eq!(back.crosshair, s.crosshair);
            assert_eq!(back.speed_decimals, s.speed_decimals);
        }
        // A cut-off crosshair section is refused, not half read.
        let code = encode(&s);
        assert!(decode(&code[..code.len() - 6]).is_none());
    }

    #[test]
    fn borders_and_the_bar_colour_round_trip_and_cost_nothing_at_their_defaults() {
        // At their defaults nothing is written: the code is the one made before they existed.
        let mut w = Writer::new();
        let plain = Settings::default();
        put_options(&mut w, &plain);
        for element in Element::ALL[..Element::FIRST].iter().copied() {
            put_element(&mut w, element, plain.style(element));
        }
        w.put(0, 1);
        assert_eq!(encode(&plain), w.finish());
        // Without the crosshair, and with it (the border section comes after it either way).
        for crosshair in [false, true] {
            let mut s = Settings::default();
            s.crosshair.enabled = crosshair;
            let cps = s.style_mut(Element::Cps);
            cps.border = true;
            cps.border_width = 2.25;
            cps.border_color = crate::config::custom_color(0x9F_1020);
            cps.border_opacity = 35.5;
            cps.bar_color = Some(3);
            let fps = s.style_mut(Element::Fps);
            fps.border = true;
            fps.border_color = 5;
            let back = decode(&encode(&s)).expect("decodes");
            assert_eq!(back.style(Element::Cps), s.style(Element::Cps));
            assert_eq!(back.style(Element::Fps), s.style(Element::Fps));
            assert_eq!(back.crosshair.enabled, crosshair);
            assert_eq!(back, decode(&encode(&back)).unwrap());
            // A cut-off border section is refused.
            let code = encode(&s);
            assert!(decode(&code[..code.len() - 6]).is_none());
        }
        // An element that differs only in its border keeps its own part of the code unchanged.
        let mut s = Settings::default();
        s.style_mut(Element::Coords).border = true;
        let mut w = Writer::new();
        put_element(&mut w, Element::Coords, s.style(Element::Coords));
        let mut d = Writer::new();
        put_element(&mut d, Element::Coords, &Element::Coords.default_style());
        assert_eq!(w.finish(), d.finish());
        assert!(decode(&encode(&s)).unwrap().style(Element::Coords).border);
    }

    #[test]
    fn a_code_made_before_the_extra_options_still_loads() {
        let mut s = busy();
        let mut w = Writer::new();
        put_options(&mut w, &s);
        for element in Element::ALL[..Element::FIRST].iter().copied() {
            put_element(&mut w, element, s.style(element));
        }
        // No `put_extras`: how the codes shared so far end.
        let old = decode(&w.finish()).expect("an older code still decodes");
        let d = Settings::default();
        assert_eq!((old.speed_unit, old.speed_decimals, old.speed_vertical), (d.speed_unit, d.speed_decimals, d.speed_vertical));
        assert_eq!((old.stopwatch_keep, old.playtime_format, old.playtime_world_only), (false, 0, false));
        assert_eq!(old.key("sw_pause"), "KeyP", "the stopwatch keys are in the key list, not the extras");
        // A cut-off new code is still refused.
        s.speed_decimals = 2;
        let code = encode(&s);
        assert!(decode(&code[..code.len() - 6]).is_none());
    }

    #[test]
    fn the_later_elements_round_trip_and_leave_older_codes_alone() {
        // While they are at their defaults nothing is written for them: the code is the old one.
        let plain = Settings::default();
        let mut w = Writer::new();
        put_options(&mut w, &plain);
        for element in Element::ALL[..Element::FIRST].iter().copied() {
            put_element(&mut w, element, plain.style(element));
        }
        w.put(0, 1);
        assert_eq!(encode(&plain), w.finish());
        // Changed, with and without a crosshair and borders ahead of them in the code.
        for (crosshair, border) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut s = Settings::default();
            s.crosshair.enabled = crosshair;
            if border {
                let cps = s.style_mut(Element::Cps);
                cps.border = true;
                cps.border_color = 5;
            }
            let reach = s.style_mut(Element::Reach);
            reach.enabled = true;
            reach.scale = 1.5;
            reach.text_color = 6;
            reach.border = true;
            reach.border_width = 2.0;
            s.style_mut(Element::Day).brackets = true;
            s.set_key("hud_combo", "KeyG");
            let back = decode(&encode(&s)).expect("decodes");
            for element in [Element::Reach, Element::Day, Element::Combo, Element::Cps] {
                assert_eq!(back.style(element), s.style(element), "{element:?}");
            }
            assert_eq!(back.key("hud_combo"), "KeyG");
            assert_eq!(back.crosshair.enabled, crosshair);
        }
    }

    #[test]
    fn the_elements_after_potion_status_round_trip_and_cost_nothing_at_their_defaults() {
        let defaults = encode(&Settings::default());
        assert_eq!(decode(&defaults).unwrap(), Settings::default());
        let mut s = Settings::default();
        s.counter_item = 5;
        s.style_mut(Element::ArrayList).enabled = true;
        s.style_mut(Element::ArrayList).brackets = true;
        s.style_mut(Element::Inventory).scale = 1.5;
        // The keys of the new elements sit past slot 31, which needs the six-bit slots of version 3.
        s.set_key("hud_inventory", "KeyI");
        s.set_key("hud_arraylist", "KeyJ");
        let back = decode(&encode(&s)).expect("the code reads back");
        assert_eq!(back.counter_item, 5);
        assert!(back.style(Element::ArrayList).enabled && back.style(Element::ArrayList).brackets);
        assert_eq!(back.style(Element::Inventory).scale, 1.5);
        assert_eq!(back.style(Element::ItemCounter), &Element::ItemCounter.default_style());
        assert_eq!(back.key("hud_inventory"), "KeyI");
        assert_eq!(back.key("hud_arraylist"), "KeyJ");
        // Only the Item counter's item changed: the section is still there.
        let mut only = Settings::default();
        only.counter_item = 2;
        assert_eq!(decode(&encode(&only)).unwrap().counter_item, 2);
    }

    #[test]
    fn a_normal_look_fits_one_field() {
        let mut s = Settings::default();
        for element in [Element::Keystrokes, Element::Cps, Element::Fps, Element::Coords] {
            let style = s.style_mut(element);
            style.enabled = true;
            style.scale = 1.2;
            style.text_color = 4;
            style.position = Some([0.1, 0.2]);
            style.brackets = true;
        }
        s.borders = true;
        s.set_key("borders", "KeyB");
        let code = encode(&s);
        assert!(code.len() <= 256, "{}", code.len());
        println!("a normal look: {} characters: {code}", code.len());
    }

    #[test]
    fn a_wrong_or_damaged_code_is_refused() {
        let code = encode(&busy());
        assert!(decode("hello").is_none());
        assert!(decode("VNT1").is_none());
        let mut bad = code.clone().into_bytes();
        let at = bad.len() / 2;
        bad[at] = if bad[at] == b'A' { b'B' } else { b'A' };
        assert!(decode(&String::from_utf8(bad).unwrap()).is_none(), "the check catches a changed character");
        // Case, dashes and look-alikes do not matter.
        let loose = code.to_lowercase().replace('-', " ");
        assert_eq!(decode(&loose), decode(&code));
        assert!(decode(&code[..code.len() - 6]).is_none());
    }

    #[test]
    fn keys_use_few_bits() {
        for key in ["", "KeyQ", "Digit4", "F12", "ShiftRight", "Numpad7"] {
            let mut w = Writer::new();
            put_key(&mut w, key);
            let code = w.finish();
            let mut r = Reader::new(&code).unwrap();
            assert_eq!(get_key(&mut r).unwrap(), key);
        }
    }

    #[test]
    fn custom_colours_round_trip_and_old_codes_still_read() {
        let mut settings = Settings::default();
        settings.crosshair.color = crate::config::custom_color(0x12AB9F);
        settings.crosshair.outline_color = crate::config::custom_color(0);
        settings.hit_color = 5;
        let code = encode(&settings);
        let back = decode(&code).expect("the code reads back");
        assert_eq!(back.crosshair.color, settings.crosshair.color);
        assert_eq!(back.crosshair.outline_color, settings.crosshair.outline_color);
        assert_eq!(back.hit_color, 5);
    }
}
