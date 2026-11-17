//! Potion Status: Lunar's Potion Effects list drawn by Vantage (the game's own effect list is hidden).
//!
//! Everything here is plain Rust: the names, the colours, how a time is written, which effects
//! blink and how the cells are laid out in a grid. `hud` writes the document, `guest` reads the
//! effects from the host.

/// Most effects drawn at once (the game has fewer kinds than this).
pub const MAX_ROWS: usize = 24;
/// The icon's side, in GUI pixels at size 1 (the game draws its effect icons at 18).
pub const ICON: f32 = 18.0;
/// Space between the icon and its text.
pub const NEAR: f32 = 4.0;
/// Height of a line of text.
pub const LINE: f32 = 9.0;
/// Distance between the tops of two rows in a list.
pub const ROW: f32 = 22.0;
/// Space between two cells in the same box (Normal, side by side) or between two tiles (Minimal).
pub const GAP: f32 = 4.0;
/// An effect with this many seconds left or fewer blinks, when Blink is on.
pub const BLINK_BELOW: u32 = 10;
/// The most tiles a line can hold in Horizontal mode (and the slider's top).
pub const MAX_PER_LINE: usize = 8;

/// How the effects are drawn (index into `MODES`).
pub const MODES: [&str; 2] = ["Normal", "Minimal"];

/// One active effect.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    /// The game's effect id (1 Speed, 2 Slowness ...).
    pub id: i32,
    /// The level shown: the wire amplifier plus one.
    pub level: u32,
    /// Seconds left, rounded up; none for an effect that does not run out.
    pub seconds: Option<u32>,
    /// The effect's name in English.
    pub name: String,
    /// The game's texture for its icon, as the host names it; empty when it has none.
    pub texture: String,
}

/// What the player asked to be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Text {
    pub name: bool,
    pub level: bool,
    pub duration: bool,
    pub formatted: bool,
    pub upper: bool,
}

/// An effect's English name: the game's own list, ids as the game numbers them.
pub fn effect_name(id: i32) -> Option<&'static str> {
    Some(match id {
        1 => "Speed",
        2 => "Slowness",
        3 => "Haste",
        4 => "Mining Fatigue",
        5 => "Strength",
        6 => "Instant Health",
        7 => "Instant Damage",
        8 => "Jump Boost",
        9 => "Nausea",
        10 => "Regeneration",
        11 => "Resistance",
        12 => "Fire Resistance",
        13 => "Water Breathing",
        14 => "Invisibility",
        15 => "Blindness",
        16 => "Night Vision",
        17 => "Hunger",
        18 => "Weakness",
        19 => "Poison",
        20 => "Wither",
        21 => "Health Boost",
        22 => "Absorption",
        23 => "Saturation",
        24 => "Levitation",
        25 => "Poison",
        26 => "Conduit Power",
        27 => "Slow Falling",
        28 => "Bad Omen",
        29 => "Hero of the Village",
        30 => "Darkness",
        31 => "Trial Omen",
        32 => "Wind Charged",
        33 => "Weaving",
        34 => "Oozing",
        35 => "Infested",
        36 => "Raid Omen",
        _ => return None,
    })
}

/// A name from the host's localisation key when the id is not in the table: the last part of the
/// key ("effect.moveSpeed" gives "Move Speed").
pub fn name_from_key(key: &str) -> String {
    let last = key.rsplit('.').next().unwrap_or(key);
    let mut out = String::with_capacity(last.len() + 4);
    for (i, c) in last.chars().enumerate() {
        if c == '_' {
            out.push(' ');
        } else if c.is_uppercase() && i > 0 {
            out.push(' ');
            out.push(c);
        } else if i == 0 {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
    }
    if out.is_empty() { "Effect".to_owned() } else { out }
}

/// An effect's colour (the game's potion colour), for "Color based on effect".
pub fn effect_color(id: i32) -> [f32; 3] {
    let rgb = match id {
        1 => 0x7C_AF_C6,
        2 => 0x5A_6C_81,
        3 => 0xD9_C0_43,
        4 => 0x4A_42_17,
        5 => 0x93_24_23,
        6 => 0xF8_24_23,
        7 => 0x43_0A_09,
        8 => 0x22_FF_4C,
        9 => 0x55_1D_4A,
        10 => 0xCD_5C_AB,
        11 => 0x99_45_3A,
        12 => 0xE4_9A_3A,
        13 => 0x2E_52_99,
        14 => 0x7F_83_92,
        15 => 0x1F_1F_23,
        16 => 0x1F_1F_A1,
        17 => 0x58_76_53,
        18 => 0x48_4D_48,
        19 | 25 => 0x4E_93_31,
        20 => 0x35_2A_27,
        21 => 0xF8_7D_23,
        22 => 0x25_52_AF,
        23 => 0xF8_24_23,
        24 => 0xCE_FF_FF,
        26 => 0x1D_C2_D1,
        27 => 0xF3_CF_BA,
        28 => 0x0B_61_38,
        29 => 0x44_FF_44,
        30 => 0x29_29_29,
        _ => 0xFF_FF_FF,
    };
    [(rgb >> 16 & 0xff) as f32 / 255.0, (rgb >> 8 & 0xff) as f32 / 255.0, (rgb & 0xff) as f32 / 255.0]
}

/// A level as the game writes it: Roman up to ten, a number after that.
pub fn roman(level: u32) -> String {
    const NUMERALS: [&str; 10] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    match level {
        1..=10 => NUMERALS[level as usize - 1].to_owned(),
        _ => level.to_string(),
    }
}

/// The first line of a row: "Regeneration II". A first level has no numeral, as in Lunar.
pub fn title(row: &Row, text: Text) -> String {
    let mut out = String::new();
    if text.name {
        out.push_str(&row.name);
    }
    if text.level && row.level > 1 {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&roman(row.level));
    }
    if text.upper { out.to_uppercase() } else { out }
}

/// Seconds as the game writes them: "03:07", or "1:02:03" past an hour; formatted, "3m 7s".
/// An effect that never runs out is "**:**".
pub fn duration(seconds: Option<u32>, formatted: bool) -> String {
    let Some(s) = seconds else { return "**:**".to_owned() };
    let (h, m, sec) = (s / 3600, s / 60 % 60, s % 60);
    if formatted {
        if h > 0 {
            format!("{h}h {m}m")
        } else if m > 0 {
            format!("{m}m {sec}s")
        } else {
            format!("{sec}s")
        }
    } else if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m:02}:{sec:02}")
    }
}

/// Opacity of a row now: it blinks while its time is short and Blink is on. `phase` flips a few
/// times a second (the caller's clock).
pub fn blink_alpha(seconds: Option<u32>, blink: bool, phase: bool) -> f32 {
    match seconds {
        Some(s) if blink && s <= BLINK_BELOW && phase => 0.3,
        _ => 1.0,
    }
}

/// How many columns and rows `count` cells fill: one column unless Horizontal, which fills lines
/// of `per_line` before it starts the next.
pub fn grid(count: usize, horizontal: bool, per_line: usize) -> (usize, usize) {
    if count == 0 {
        return (0, 0);
    }
    let columns = if horizontal { per_line.clamp(1, MAX_PER_LINE).min(count) } else { 1 };
    (columns, count.div_ceil(columns))
}

/// Where cell `index` goes in the grid, as (column, row).
pub fn cell_at(index: usize, columns: usize) -> (usize, usize) {
    (index % columns.max(1), index / columns.max(1))
}

/// A list's size in GUI pixels at size 1, before padding: `cell` is one cell's size, `gap` the
/// space between cells.
pub fn extent(columns: usize, rows: usize, cell: [f32; 2], gap: f32) -> [f32; 2] {
    if columns == 0 || rows == 0 {
        return [0.0, 0.0];
    }
    [columns as f32 * cell[0] + (columns - 1) as f32 * gap, rows as f32 * cell[1] + (rows - 1) as f32 * gap]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i32, level: u32, seconds: Option<u32>) -> Row {
        Row { id, level, seconds, name: effect_name(id).unwrap_or("?").to_owned(), texture: String::new() }
    }

    const ALL: Text = Text { name: true, level: true, duration: true, formatted: false, upper: false };

    #[test]
    fn titles_show_the_level_from_the_second_on() {
        assert_eq!(title(&row(10, 1, Some(5)), ALL), "Regeneration");
        assert_eq!(title(&row(10, 2, Some(5)), ALL), "Regeneration II");
        assert_eq!(title(&row(1, 12, None), ALL), "Speed 12");
        assert_eq!(title(&row(10, 2, None), Text { level: false, ..ALL }), "Regeneration");
        assert_eq!(title(&row(10, 2, None), Text { name: false, ..ALL }), "II");
        assert_eq!(title(&row(10, 2, None), Text { upper: true, ..ALL }), "REGENERATION II");
    }

    #[test]
    fn times_read_like_the_game() {
        assert_eq!(duration(Some(187), false), "03:07");
        assert_eq!(duration(Some(9), false), "00:09");
        assert_eq!(duration(Some(3723), false), "1:02:03");
        assert_eq!(duration(None, false), "**:**");
        assert_eq!(duration(Some(187), true), "3m 7s");
        assert_eq!(duration(Some(42), true), "42s");
        assert_eq!(duration(Some(7260), true), "2h 1m");
    }

    #[test]
    fn only_short_effects_blink_and_only_in_one_phase() {
        assert_eq!(blink_alpha(Some(5), true, true), 0.3);
        assert_eq!(blink_alpha(Some(5), true, false), 1.0);
        assert_eq!(blink_alpha(Some(5), false, true), 1.0);
        assert_eq!(blink_alpha(Some(60), true, true), 1.0);
        assert_eq!(blink_alpha(None, true, true), 1.0);
    }

    #[test]
    fn the_grid_fills_lines_in_order() {
        assert_eq!(grid(0, true, 4), (0, 0));
        assert_eq!(grid(3, false, 4), (1, 3));
        assert_eq!(grid(3, true, 4), (3, 1));
        assert_eq!(grid(6, true, 4), (4, 2));
        assert_eq!(grid(2, true, 99), (2, 1));
        assert_eq!((cell_at(0, 4), cell_at(3, 4), cell_at(4, 4), cell_at(5, 1)), ((0, 0), (3, 0), (0, 1), (0, 5)));
        assert_eq!(extent(2, 2, [10.0, 20.0], 4.0), [24.0, 44.0]);
        assert_eq!(extent(0, 0, [10.0, 20.0], 4.0), [0.0, 0.0]);
    }

    #[test]
    fn names_and_colours_cover_the_game_s_effects() {
        for id in 1..=36 {
            assert!(effect_name(id).is_some(), "effect {id} has no name");
        }
        assert_eq!(effect_name(99), None);
        assert_eq!(name_from_key("effect.moveSpeed"), "Move Speed");
        assert_eq!(name_from_key("potion.night_vision"), "Night vision");
        assert_eq!(effect_color(6), [248.0 / 255.0, 36.0 / 255.0, 35.0 / 255.0]);
        assert_eq!(effect_color(500), [1.0, 1.0, 1.0]);
    }
}
