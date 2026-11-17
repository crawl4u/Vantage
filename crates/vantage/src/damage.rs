//! Damage Indicator: a number that rises and fades above a player or mob when its health drops.
//!
//! The host gives each nearby actor's replicated health. This module keeps the last value seen per
//! actor, turns a drop into a floating number, and lays the digits out as camera-facing strokes
//! (`glyph`): the host has no text in the world, so every digit is a small polyline.

use std::collections::HashMap;

/// Numbers alive at once; the oldest goes first.
pub const MAX_FLOATS: usize = 12;
/// Characters drawn in one frame (the host takes 1024 instances in all).
pub const MAX_GLYPHS: usize = 64;
/// The smallest health drop that counts (health arrives in tenths of a point at least).
const MIN_DROP: f32 = 0.05;

/// The glyphs, by index: the digits 0 to 9, then the point and the minus sign.
pub const GLYPHS: usize = 100;
/// The parts of a waypoint's label plate, each 15 pixels tall: a filled cell 6 wide, a strip 6 wide and 1 tall (the
/// top and bottom of the outline) and a column 1 wide (its sides). The characters of the game's font (`FONT`) follow.
pub const BLOCK: usize = 13;
pub const BAR: usize = 14;
pub const SIDE: usize = 15;
const FONT_FIRST: usize = 16;
/// The label plate's padded parts (padding 0 to 8 pixels): filled cells 6 wide and columns 1 wide, each `7 + 2 * padding`
/// tall. They follow the characters of the font.
pub const MAX_PADDING: usize = 8;
const PADDED_BLOCK: usize = FONT_FIRST + 66;
const PADDED_SIDE: usize = PADDED_BLOCK + MAX_PADDING + 1;
const POINT: usize = 10;
const MINUS: usize = 11;
const HEART: usize = 12;

/// How big the numbers are: (name, factor).
pub const SIZES: [(&str, f32); 6] = [("50%", 0.5), ("75%", 0.75), ("100%", 1.0), ("150%", 1.5), ("200%", 2.0), ("300%", 3.0)];
pub const SIZE_NAMES: [&str; 6] = ["50%", "75%", "100%", "150%", "200%", "300%"];
/// How long a number stays: (name, seconds).
pub const TIMES: [(&str, f64); 5] = [("0.5 s", 0.5), ("1 s", 1.0), ("1.5 s", 1.5), ("2 s", 2.0), ("3 s", 3.0)];
pub const TIME_NAMES: [&str; 5] = ["0.5 s", "1 s", "1.5 s", "2 s", "3 s"];
/// The colour of the numbers: (name, red, green, blue).
pub const COLORS: [(&str, [f32; 3]); 7] = [
    ("Red", [1.0, 0.25, 0.25]),
    ("Orange", [1.0, 0.6, 0.15]),
    ("Yellow", [1.0, 0.92, 0.25]),
    ("White", [1.0, 1.0, 1.0]),
    ("Green", [0.35, 1.0, 0.4]),
    ("Cyan", [0.3, 0.95, 1.0]),
    ("Pink", [1.0, 0.5, 0.85]),
];
pub const COLOR_NAMES: [&str; 7] = ["Red", "Orange", "Yellow", "White", "Green", "Cyan", "Pink"];

/// One number on its way up.
#[derive(Clone, Debug, PartialEq)]
pub struct Float {
    /// Above the actor's head when it was hurt.
    pub origin: [f64; 3],
    /// Health points lost.
    pub amount: f32,
    pub born: f64,
}

/// The last health seen of each nearby actor, and the numbers alive.
#[derive(Default)]
pub struct Tracker {
    health: HashMap<u64, f32>,
    pub floats: Vec<Float>,
}

impl Tracker {
    pub fn clear(&mut self) {
        self.health.clear();
        self.floats.clear();
    }

    /// Notes one actor's health this read; a drop starts a number at `above` (over its head).
    pub fn note(&mut self, id: u64, health: Option<f32>, above: [f64; 3], now: f64) {
        let Some(health) = health.filter(|h| h.is_finite()) else {
            self.health.remove(&id);
            return;
        };
        if let Some(&before) = self.health.get(&id) {
            let lost = before - health;
            if lost >= MIN_DROP {
                if self.floats.len() >= MAX_FLOATS {
                    self.floats.remove(0);
                }
                self.floats.push(Float { origin: above, amount: lost, born: now });
            }
        }
        self.health.insert(id, health);
    }

    /// Forgets the actors that are no longer near (their next sighting starts fresh).
    pub fn retain_seen(&mut self, seen: &[u64]) {
        self.health.retain(|id, _| seen.contains(id));
    }

    /// Drops the numbers that have run their time.
    pub fn expire(&mut self, now: f64, duration: f64) {
        self.floats.retain(|f| now - f.born < duration);
    }
}

/// The text of a loss: health points, or hearts (two points each) with `hearts`.
pub fn amount_text(amount: f32, hearts: bool) -> String {
    let value = if hearts { amount / 2.0 } else { amount };
    let tenths = ((value * 10.0).round() as i64).max(1);
    if tenths % 10 == 0 {
        format!("-{}", tenths / 10)
    } else {
        format!("-{}.{}", tenths / 10, tenths % 10)
    }
}

/// The glyph of a character of `amount_text` (and the heart that follows it in hearts mode).
fn glyph_of(c: char) -> Option<usize> {
    match c {
        '0'..='9' => Some(c as usize - '0' as usize),
        '.' => Some(POINT),
        '-' => Some(MINUS),
        '\u{2665}' => Some(HEART),
        _ => None,
    }
}

/// The pixels of each glyph, 7 rows from the top, in the shapes of the game's own font (`#` is lit).
const BITMAPS: [[&str; 7]; 13] = [
    [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
    ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."],
    [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"],
    [".###.", "#...#", "....#", "..##.", "....#", "#...#", ".###."],
    ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."],
    ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."],
    ["..##.", ".#...", "#....", "####.", "#...#", "#...#", ".###."],
    ["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."],
    [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."],
    [".###.", "#...#", "#...#", ".####", "....#", "...#.", ".##.."],
    [".", ".", ".", ".", ".", ".", "#"],
    [".....", ".....", ".....", "#####", ".....", ".....", "....."],
    [".#.#.", "#####", "#####", "#####", ".###.", "..#..", "....."],
];

/// The characters a waypoint's label can show, in the game's own font (Minecraft Seven, read pixel by pixel from the
/// font file): the character, its rows from seven above the baseline to one below it, and how far the pen moves.
const FONT: [(char, [&str; 8], f32); 66] = [
    ('0', [".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###.", "....."], 6.0),
    ('1', ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", "#####", "....."], 6.0),
    ('2', [".###.", "#...#", "....#", "..##.", ".#...", "#...#", "#####", "....."], 6.0),
    ('3', [".###.", "#...#", "....#", "..##.", "....#", "#...#", ".###.", "....."], 6.0),
    ('4', ["...##", "..#.#", ".#..#", "#...#", "#####", "....#", "....#", "....."], 6.0),
    ('5', ["#####", "#....", "####.", "....#", "....#", "#...#", ".###.", "....."], 6.0),
    ('6', ["..##.", ".#...", "#....", "####.", "#...#", "#...#", ".###.", "....."], 6.0),
    ('7', ["#####", "#...#", "....#", "...#.", "..#..", "..#..", "..#..", "....."], 6.0),
    ('8', [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###.", "....."], 6.0),
    ('9', [".###.", "#...#", "#...#", ".####", "....#", "...#.", ".##..", "....."], 6.0),
    ('.', [".", ".", ".", ".", ".", "#", "#", "."], 2.0),
    ('-', [".....", ".....", ".....", "#####", ".....", ".....", ".....", "....."], 6.0),
    ('[', ["###", "#..", "#..", "#..", "#..", "#..", "###", "..."], 4.0),
    (']', ["###", "..#", "..#", "..#", "..#", "..#", "###", "..."], 4.0),
    ('A', [".###.", "#...#", "#####", "#...#", "#...#", "#...#", "#...#", "....."], 6.0),
    ('B', ["####.", "#...#", "####.", "#...#", "#...#", "#...#", "####.", "....."], 6.0),
    ('C', [".###.", "#...#", "#....", "#....", "#....", "#...#", ".###.", "....."], 6.0),
    ('D', ["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####.", "....."], 6.0),
    ('E', ["#####", "#....", "###..", "#....", "#....", "#....", "#####", "....."], 6.0),
    ('F', ["#####", "#....", "###..", "#....", "#....", "#....", "#....", "....."], 6.0),
    ('G', [".####", "#....", "#..##", "#...#", "#...#", "#...#", ".###.", "....."], 6.0),
    ('H', ["#...#", "#...#", "#####", "#...#", "#...#", "#...#", "#...#", "....."], 6.0),
    ('I', ["###", ".#.", ".#.", ".#.", ".#.", ".#.", "###", "..."], 4.0),
    ('J', ["....#", "....#", "....#", "....#", "....#", "#...#", ".###.", "....."], 6.0),
    ('K', ["#...#", "#..#.", "###..", "#..#.", "#...#", "#...#", "#...#", "....."], 6.0),
    ('L', ["#....", "#....", "#....", "#....", "#....", "#....", "#####", "....."], 6.0),
    ('M', ["#...#", "##.##", "#.#.#", "#...#", "#...#", "#...#", "#...#", "....."], 6.0),
    ('N', ["#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#", "....."], 6.0),
    ('O', [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.", "....."], 6.0),
    ('P', ["####.", "#...#", "####.", "#....", "#....", "#....", "#....", "....."], 6.0),
    ('Q', [".###.", "#...#", "#...#", "#...#", "#...#", "#..#.", ".##.#", "....."], 6.0),
    ('R', ["####.", "#...#", "####.", "#...#", "#...#", "#...#", "#...#", "....."], 6.0),
    ('S', [".####", "#....", ".###.", "....#", "....#", "#...#", ".###.", "....."], 6.0),
    ('T', ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#..", "....."], 6.0),
    ('U', ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.", "....."], 6.0),
    ('V', ["#...#", "#...#", "#...#", "#...#", ".#.#.", ".#.#.", "..#..", "....."], 6.0),
    ('W', ["#...#", "#...#", "#...#", "#...#", "#.#.#", "##.##", "#...#", "....."], 6.0),
    ('X', ["#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#", "#...#", "....."], 6.0),
    ('Y', ["#...#", ".#.#.", "..#..", "..#..", "..#..", "..#..", "..#..", "....."], 6.0),
    ('Z', ["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####", "....."], 6.0),
    ('a', [".....", ".....", ".###.", "....#", ".####", "#...#", ".####", "....."], 6.0),
    ('b', ["#....", "#....", "#.##.", "##..#", "#...#", "#...#", "####.", "....."], 6.0),
    ('c', [".....", ".....", ".###.", "#...#", "#....", "#...#", ".###.", "....."], 6.0),
    ('d', ["....#", "....#", ".##.#", "#..##", "#...#", "#...#", ".####", "....."], 6.0),
    ('e', [".....", ".....", ".###.", "#...#", "#####", "#....", ".####", "....."], 6.0),
    ('f', ["..##", ".#..", "####", ".#..", ".#..", ".#..", ".#..", "...."], 5.0),
    ('g', [".....", ".....", ".####", "#...#", "#...#", ".####", "....#", "####."], 6.0),
    ('h', ["#....", "#....", "#.##.", "##..#", "#...#", "#...#", "#...#", "....."], 6.0),
    ('i', ["#", ".", "#", "#", "#", "#", "#", "."], 2.0),
    ('j', [".....", "....#", ".....", "....#", "....#", "#...#", "#...#", ".###."], 6.0),
    ('k', ["#...", "#...", "#..#", "#.#.", "##..", "#.#.", "#..#", "...."], 5.0),
    ('l', ["#.", "#.", "#.", "#.", "#.", "#.", ".#", ".."], 3.0),
    ('m', [".....", ".....", "##.#.", "#.#.#", "#.#.#", "#...#", "#...#", "....."], 6.0),
    ('n', [".....", ".....", "####.", "#...#", "#...#", "#...#", "#...#", "....."], 6.0),
    ('o', [".....", ".....", ".###.", "#...#", "#...#", "#...#", ".###.", "....."], 6.0),
    ('p', [".....", ".....", "#.##.", "##..#", "#...#", "####.", "#....", "#...."], 6.0),
    ('q', [".....", ".....", ".##.#", "#..##", "#...#", ".####", "....#", "....#"], 6.0),
    ('r', [".....", ".....", "#.##.", "##..#", "#....", "#....", "#....", "....."], 6.0),
    ('s', [".....", ".....", ".####", "#....", ".###.", "....#", "####.", "....."], 6.0),
    ('t', [".#.", ".#.", "###", ".#.", ".#.", ".#.", "..#", "..."], 4.0),
    ('u', [".....", ".....", "#...#", "#...#", "#...#", "#...#", ".####", "....."], 6.0),
    ('v', [".....", ".....", "#...#", "#...#", "#...#", ".#.#.", "..#..", "....."], 6.0),
    ('w', [".....", ".....", "#...#", "#...#", "#.#.#", "#.#.#", ".####", "....."], 6.0),
    ('x', [".....", ".....", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "....."], 6.0),
    ('y', [".....", ".....", "#...#", "#...#", "#...#", ".####", "....#", "####."], 6.0),
    ('z', [".....", ".....", "#####", "...#.", "..#..", ".#...", "#####", "....."], 6.0),
];
const BLOCK_ROWS: [&str; 15] = ["######"; 15];
const BAR_ROWS: [&str; 1] = ["######"];
const SIDE_ROWS: [&str; 15] = ["#"; 15];
const PADDED_BLOCK_ROWS: [&str; 7 + 2 * MAX_PADDING] = ["######"; 7 + 2 * MAX_PADDING];
const PADDED_SIDE_ROWS: [&str; 7 + 2 * MAX_PADDING] = ["#"; 7 + 2 * MAX_PADDING];

/// Which entry of `FONT` a character is.
fn font_index(c: char) -> Option<usize> {
    FONT.iter().position(|f| f.0 == c)
}

/// The rows of a glyph, top first.
fn rows_of(glyph: usize) -> &'static [&'static str] {
    match glyph {
        BLOCK => &BLOCK_ROWS,
        BAR => &BAR_ROWS,
        SIDE => &SIDE_ROWS,
        g if g >= PADDED_SIDE => &PADDED_SIDE_ROWS[..7 + 2 * (g - PADDED_SIDE)],
        g if g >= PADDED_BLOCK => &PADDED_BLOCK_ROWS[..7 + 2 * (g - PADDED_BLOCK)],
        g if g >= FONT_FIRST => &FONT[g - FONT_FIRST].1,
        g => &BITMAPS[g],
    }
}

/// Pixels a glyph is wide; the pen moves one more after it.
fn width_of(glyph: usize) -> f32 {
    rows_of(glyph)[0].len() as f32
}

/// The pixel rows of every glyph in a text are 7 high: a pixel is this share of the height.
pub const ROWS: f32 = 7.0;

/// The filled shape of a glyph: triangles for every lit pixel, in pixel units (x right from 0, y up
/// from 0, the top row at y = 7), each pixel facing both ways so no culling can hide it.
pub fn glyph_mesh(index: usize) -> (Vec<[f32; 3]>, Vec<u32>) {
    let (mut vertices, mut indices) = (Vec::new(), Vec::new());
    let rows = rows_of(index);
    let top = rows.len() as f32;
    for (row, line) in rows.iter().enumerate() {
        for (column, c) in line.chars().enumerate() {
            if c != '#' {
                continue;
            }
            // The game's font sits on a baseline one row above the bottom; the other glyphs on the bottom.
            let y = if (FONT_FIRST..PADDED_BLOCK).contains(&index) { 6.0 - row as f32 } else { top - 1.0 - row as f32 };
            let x = column as f32;
            let first = vertices.len() as u32;
            vertices.extend([[x, y, 0.0], [x + 1.0, y, 0.0], [x + 1.0, y + 1.0, 0.0], [x, y + 1.0, 0.0]]);
            indices.extend([0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2].map(|i| first + i));
        }
    }
    (vertices, indices)
}

/// One character to draw: which glyph, the anchor, the shift to its corner along the camera's right,
/// the size of one pixel in blocks, and the opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub glyph: usize,
    pub origin: [f64; 3],
    /// The glyph's corner relative to the anchor, in world axes.
    pub shift: [f32; 3],
    /// Blocks per pixel of the font.
    pub scale: f32,
    pub alpha: f32,
}

/// The characters of `float` at time `now`, for a camera at `camera` looking `yaw` degrees to the left of -Z.
/// `size` and `duration` are the player's choices. Nothing once the number has run its time.
pub fn place(float: &Float, now: f64, duration: f64, size: f32, hearts: bool, camera: [f64; 3], yaw: f64, out: &mut Vec<Placed>) {
    let age = now - float.born;
    if !(0.0..duration).contains(&age) || duration <= 0.0 {
        return;
    }
    let p = (age / duration) as f32;
    let rise = 1.0 * (1.0 - (1.0 - p).powi(3));
    // Each number leans a little to one side (the same one for its whole life), so quick hits do not stack.
    let drift = (float.born * 997.0).sin() as f32 * 0.25;
    let alpha = if p < 0.6 { 1.0 } else { ((1.0 - p) / 0.4).clamp(0.0, 1.0) };
    let (dx, dy, dz) = (float.origin[0] - camera[0], float.origin[1] - camera[1], float.origin[2] - camera[2]);
    let distance = ((dx * dx + dy * dy + dz * dz).sqrt() as f32).clamp(1.0, 24.0);
    // Farther numbers grow so they stay readable; a new one pops.
    let pop = if p < 0.15 { 1.0 + 0.4 * (0.15 - p) / 0.15 } else { 1.0 };
    let height = size * 0.32 * (0.7 + 0.1 * distance) * pop;
    let pixel = height / ROWS;
    let mut text = amount_text(float.amount, hearts);
    if hearts {
        text.push('\u{2665}');
    }
    let glyphs: Vec<usize> = text.chars().filter_map(glyph_of).collect();
    // The pen starts so the whole number is centred (the pixel after the last glyph is not counted).
    let width: f32 = glyphs.iter().map(|g| width_of(*g) + 1.0).sum::<f32>() - 1.0;
    let (sin, cos) = yaw.to_radians().sin_cos();
    let right = [cos as f32, -sin as f32];
    let mut pen = -width / 2.0 + drift / pixel;
    for g in glyphs {
        if out.len() >= MAX_GLYPHS {
            return;
        }
        let along = pen * pixel;
        out.push(Placed {
            glyph: g,
            origin: [float.origin[0], float.origin[1] + f64::from(rise), float.origin[2]],
            shift: [right[0] * along, 0.0, right[1] * along],
            scale: pixel,
            alpha,
        });
        pen += width_of(g) + 1.0;
    }
}

/// A plain label (digits, point, minus) centred over `origin`, `height` blocks tall, facing a camera looking `yaw` degrees
/// to the left of -Z; used by the Waypoints element for distances.
pub fn place_text(text: &str, origin: [f64; 3], height: f32, yaw: f64, out: &mut Vec<Placed>) {
    let pixel = height / ROWS;
    // Each character: a glyph, or None for a space (three pixels wide).
    let chars: Vec<Option<usize>> = text.chars().filter_map(|c| if c == ' ' { Some(None) } else { glyph_of(c).map(Some) }).collect();
    if chars.iter().all(Option::is_none) {
        return;
    }
    let advance = |g: &Option<usize>| g.map_or(3.0, |g| width_of(g)) + 1.0;
    let width: f32 = chars.iter().map(advance).sum::<f32>() - 1.0;
    let (sin, cos) = yaw.to_radians().sin_cos();
    let right = [cos as f32, -sin as f32];
    let mut pen = -width / 2.0;
    for c in &chars {
        if let Some(g) = *c {
            if out.len() >= MAX_GLYPHS {
                return;
            }
            let along = pen * pixel;
            out.push(Placed { glyph: g, origin, shift: [right[0] * along, 0.0, right[1] * along], scale: pixel, alpha: 1.0 });
        }
        pen += advance(c);
    }
}

/// A waypoint's label, Lunar's: `text` in the game's font on a dark plate with a one-pixel outline, centred over
/// `origin`, the text `height` blocks tall, facing a camera looking `yaw` degrees to the left of -Z. The letters are
/// spaced as the game spaces them (a space is three pixels); the plate is built of six-pixel cells, three pixels of air
/// beyond the text at least, and four above and below. Pushes the plate's cells to `fill`, its outline to `outline`
/// and the characters to `out`.
/// The camera's right and up directions in the world for the angles `yaw` and `pitch` (degrees, as `hitbox::look_rotation`).
pub fn camera_axes(yaw: f64, pitch: f64) -> ([f32; 3], [f32; 3]) {
    let (x, y, z, w) = crate::hitbox::look_rotation(yaw, pitch);
    (
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w)],
        [2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w)],
    )
}

pub fn place_label(text: &str, origin: [f64; 3], height: f32, axes: ([f32; 3], [f32; 3]), padding: usize, fill: &mut Vec<Placed>, outline: &mut Vec<Placed>, out: &mut Vec<Placed>) {
    let padding = padding.min(MAX_PADDING);
    let pad = padding as f32;
    let pixel = height / ROWS;
    // (glyph or None for a space, advance) of each character.
    let chars: Vec<(Option<usize>, f32)> = text
        .chars()
        .filter_map(|c| match c {
            ' ' => Some((None, 3.0)),
            c => font_index(c).map(|i| (Some(FONT_FIRST + i), FONT[i].2)),
        })
        .collect();
    if chars.is_empty() {
        return;
    }
    // The pieces are laid out along the camera's own right and up, so they stay in one plane (the glyphs are turned by
    // the camera's rotation about their corners; with only the heading a pitched view tore the outline apart).
    let (right, up) = axes;
    let at = |glyph: usize, x: f32, y: f32| Placed {
        glyph,
        origin,
        shift: [(right[0] * x + up[0] * y) * pixel, (right[1] * x + up[1] * y) * pixel, (right[2] * x + up[2] * y) * pixel],
        scale: pixel,
        alpha: 1.0,
    };
    let text_width = chars.iter().map(|c| c.1).sum::<f32>() - 1.0;
    // The plate fits the text exactly: whole 6-pixel cells, then 1-pixel columns for what is left over; the outline's
    // last strip is moved back so it ends on the edge.
    let width = (text_width + 2.0 * pad).round().max(1.0) as usize;
    let (whole, rest) = (width / 6, width % 6);
    let left = -(width as f32) / 2.0;
    if fill.len() + outline.len() + 3 * (whole + 2 + rest) > 4 * MAX_GLYPHS {
        return;
    }
    for i in 0..whole {
        fill.push(at(PADDED_BLOCK + padding, left + i as f32 * 6.0, -pad));
    }
    for i in 0..rest {
        fill.push(at(PADDED_SIDE + padding, left + (whole * 6 + i) as f32, -pad));
    }
    for i in 0..whole + usize::from(rest > 0) {
        let x = (left + i as f32 * 6.0).min(left + width as f32 - 6.0);
        outline.push(at(BAR, x, -pad));
        outline.push(at(BAR, x, pad + 6.0));
    }
    outline.push(at(PADDED_SIDE + padding, left, -pad));
    outline.push(at(PADDED_SIDE + padding, left + width as f32 - 1.0, -pad));
    let mut pen = left + pad;
    for (glyph, advance) in chars {
        if out.len() >= MAX_GLYPHS {
            return;
        }
        if let Some(g) = glyph {
            out.push(at(g, pen, 0.0));
        }
        pen += advance;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_health_drop_starts_a_number_and_a_gain_does_not() {
        let mut t = Tracker::default();
        t.note(7, Some(20.0), [0.0, 2.0, 0.0], 1.0);
        assert!(t.floats.is_empty(), "the first sighting only records the health");
        t.note(7, Some(16.5), [0.0, 2.0, 0.0], 2.0);
        assert_eq!(t.floats.len(), 1);
        assert!((t.floats[0].amount - 3.5).abs() < 1e-4);
        t.note(7, Some(18.0), [0.0, 2.0, 0.0], 3.0);
        assert_eq!(t.floats.len(), 1, "healing shows nothing");
        t.note(7, Some(17.99), [0.0, 2.0, 0.0], 4.0);
        assert_eq!(t.floats.len(), 1, "a hundredth of a point is not a hit");
    }

    #[test]
    fn an_actor_that_left_starts_fresh_and_the_oldest_number_goes_first() {
        let mut t = Tracker::default();
        t.note(1, Some(10.0), [0.0; 3], 0.0);
        t.retain_seen(&[]);
        t.note(1, Some(4.0), [0.0; 3], 1.0);
        assert!(t.floats.is_empty(), "after leaving, the first sighting records only");
        for i in 0..20u64 {
            t.note(100 + i, Some(10.0), [0.0; 3], 0.0);
            t.note(100 + i, Some(9.0), [0.0; 3], i as f64);
        }
        assert_eq!(t.floats.len(), MAX_FLOATS);
        assert_eq!(t.floats[0].born, 8.0);
    }

    #[test]
    fn a_label_is_centred_and_capped() {
        let mut out = Vec::new();
        place_text("12", [0.0; 3], 0.7, 0.0, &mut out);
        assert_eq!(out.len(), 2);
        assert!(out[0].shift[0] < 0.0 && out[1].shift[0] > out[0].shift[0], "left to right, around the anchor");
        let mut many = Vec::new();
        place_text(&"8".repeat(200), [0.0; 3], 0.7, 0.0, &mut many);
        assert_eq!(many.len(), MAX_GLYPHS);
    }

    #[test]
    fn numbers_expire() {
        let mut t = Tracker::default();
        t.note(1, Some(10.0), [0.0; 3], 0.0);
        t.note(1, Some(9.0), [0.0; 3], 1.0);
        t.expire(1.9, 1.0);
        assert_eq!(t.floats.len(), 1);
        t.expire(2.0, 1.0);
        assert!(t.floats.is_empty());
    }

    #[test]
    fn amounts_read_in_points_or_hearts() {
        assert_eq!(amount_text(3.5, false), "-3.5");
        assert_eq!(amount_text(4.0, false), "-4");
        assert_eq!(amount_text(7.0, true), "-3.5");
        assert_eq!(amount_text(1.0, true), "-0.5");
        assert_eq!(amount_text(0.06, false), "-0.1");
        assert_eq!(amount_text(12.25, false), "-12.3");
    }

    #[test]
    fn every_glyph_is_pixels_inside_its_box_and_the_digits_differ() {
        for g in 0..GLYPHS {
            let (vertices, indices) = glyph_mesh(g);
            assert!(!vertices.is_empty() && indices.len() == vertices.len() / 4 * 12, "glyph {g}");
            assert!(indices.iter().all(|i| (*i as usize) < vertices.len()));
            assert!(vertices.iter().all(|v| (0.0..=width_of(g)).contains(&v[0]) && (-1.0..=23.0).contains(&v[1]) && v[2] == 0.0), "glyph {g}");
            {
                for row in rows_of(g) {
                    assert_eq!(row.len() as f32, width_of(g), "glyph {g} has rows of one width");
                }
            }
        }
        for a in 0..10 {
            for b in a + 1..10 {
                assert_ne!(BITMAPS[a], BITMAPS[b], "{a} and {b}");
            }
        }
        assert_eq!(glyph_of('7'), Some(7));
        assert_eq!(glyph_of('.'), Some(POINT));
        assert_eq!(glyph_of('-'), Some(MINUS));
        assert_eq!(glyph_of('\u{2665}'), Some(HEART));
        assert_eq!(glyph_of('?'), None);
        assert_eq!(font_index('0'), Some(0));
        assert!(['a', 'Z', '[', ']', 'm'].iter().all(|c| font_index(*c).is_some()));
    }

    #[test]
    fn a_number_is_centred_rises_fades_and_faces_the_camera() {
        let float = Float { origin: [10.0, 65.0, 10.0], amount: 3.0, born: 0.0 };
        let mut out = Vec::new();
        // Facing -Z (yaw 0), the right is +X: the glyphs run along +X, the first left of the middle.
        place(&float, 0.0, 1.0, 1.0, false, [10.0, 65.0, 16.0], 0.0, &mut out);
        assert_eq!(out.len(), 2, "minus and 3");
        assert!(out[0].shift[0] < 0.0 && out[1].shift[0] > out[0].shift[0] && out[0].shift[2] == 0.0);
        assert_eq!(out[0].alpha, 1.0);
        let early = out[0].origin[1];
        out.clear();
        place(&float, 0.9, 1.0, 1.0, false, [10.0, 65.0, 16.0], 0.0, &mut out);
        assert!(out[0].origin[1] > early, "it rises");
        assert!(out[0].alpha < 0.5, "and fades");
        out.clear();
        place(&float, 1.0, 1.0, 1.0, false, [10.0, 65.0, 16.0], 0.0, &mut out);
        assert!(out.is_empty(), "nothing after its time");
        // Turned a quarter to the left (yaw 90), the right is -Z.
        place(&float, 0.3, 1.0, 1.0, false, [10.0, 65.0, 16.0], 90.0, &mut out);
        assert!(out[1].shift[0].abs() < 1e-5 && out[1].shift[2] < out[0].shift[2]);
    }

    #[test]
    fn hearts_end_with_a_heart_and_points_do_not() {
        let float = Float { origin: [0.0; 3], amount: 3.0, born: 0.0 };
        let (mut points, mut hearts) = (Vec::new(), Vec::new());
        place(&float, 0.5, 1.0, 1.0, false, [0.0, 0.0, 4.0], 0.0, &mut points);
        place(&float, 0.5, 1.0, 1.0, true, [0.0, 0.0, 4.0], 0.0, &mut hearts);
        assert_eq!(points.len(), 2, "-3");
        assert_eq!(hearts.len(), 5, "-1.5 and a heart");
        assert_eq!(hearts[4].glyph, HEART);
    }

    #[test]
    fn farther_numbers_are_bigger_and_size_scales_them() {
        let float = Float { origin: [0.0, 0.0, 0.0], amount: 1.0, born: 0.0 };
        let (mut near, mut far, mut big) = (Vec::new(), Vec::new(), Vec::new());
        place(&float, 0.5, 1.0, 1.0, false, [0.0, 0.0, 2.0], 0.0, &mut near);
        place(&float, 0.5, 1.0, 1.0, false, [0.0, 0.0, 20.0], 0.0, &mut far);
        place(&float, 0.5, 1.0, 2.0, false, [0.0, 0.0, 2.0], 0.0, &mut big);
        assert!(far[0].scale > near[0].scale);
        assert!((big[0].scale / near[0].scale - 2.0).abs() < 1e-4);
    }
}
