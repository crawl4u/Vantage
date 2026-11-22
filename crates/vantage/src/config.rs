//! What the player can change: the world tools, and for every HUD element whether it is on,
//! where it sits, how big it is and how it looks. Saved to the mod's settings file.

use crate::crosshair_config::CrosshairSettings;
use crate::gameplay::GameplaySettings;
use crate::zoom_config::ZoomSettings;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The colours offered in the menu; a choice control holds at most eight options.
pub const PALETTE: [(&str, [f32; 3]); 8] = [
    ("White", [1.0, 1.0, 1.0]),
    ("Gray", [0.5, 0.5, 0.5]),
    ("Black", [0.0, 0.0, 0.0]),
    ("Red", [1.0, 0.33, 0.33]),
    ("Green", [0.33, 1.0, 0.33]),
    ("Blue", [0.33, 0.55, 1.0]),
    ("Yellow", [1.0, 0.9, 0.25]),
    ("Purple", [0.75, 0.4, 1.0]),
];
/// The names alone, for the menu's choice controls (taken from `PALETTE`, so they cannot drift).
pub const PALETTE_NAMES: [&str; 8] = {
    let mut names = [""; 8];
    let mut i = 0;
    while i < 8 {
        names[i] = PALETTE[i].0;
        i += 1;
    }
    names
};
/// The dropdown's last row: pick it to type a hex colour. A colour setting holds a palette index,
/// or `CUSTOM_BASE + (transparency << 24 | 0xRRGGBB)` for a custom one. The transparency is 0..=100 percent;
/// 0 (every colour saved before 2.2.0) is fully opaque.
pub const CUSTOM_ROW: usize = PALETTE.len();
/// The first value of a custom colour (the row itself, `CUSTOM_ROW`, only opens the hex editor).
pub const CUSTOM_BASE: usize = CUSTOM_ROW + 1;
/// The palette's names and the custom row, for the colour dropdowns.
pub const COLOR_CHOICES: [&str; 9] = {
    let mut names = [""; 9];
    let mut i = 0;
    while i < 8 {
        names[i] = PALETTE[i].0;
        i += 1;
    }
    names[8] = "Custom";
    names
};
pub const WHITE: usize = 0;
pub const BLACK: usize = 2;
pub const GREEN: usize = 4;
/// The saturation overlay's colour out of the box: Lunar's #ffc600 (a custom colour).
pub const SAT_GOLD: usize = CUSTOM_BASE + 0xFF_C600;

/// Whether a stored colour is a palette index or a custom colour.
pub fn valid_color(color: usize) -> bool {
    color <= CUSTOM_BASE + ((MAX_TRANSPARENCY as usize) << 24 | 0xFF_FFFF)
}

/// The most transparent a colour can be, in percent.
pub const MAX_TRANSPARENCY: u32 = 100;

/// The 24 bits of a custom colour (None for a palette one).
pub fn custom_rgb(color: usize) -> Option<u32> {
    color.checked_sub(CUSTOM_BASE).filter(|_| valid_color(color)).map(|v| v as u32 & 0xFF_FFFF)
}

/// How transparent a colour is, in percent (0 for a palette colour or one with no transparency).
pub fn custom_transparency(color: usize) -> u32 {
    color.checked_sub(CUSTOM_BASE).filter(|_| valid_color(color)).map_or(0, |v| (v as u32) >> 24)
}

/// The colour setting for `rgb` at `transparency` percent.
pub fn custom_color_alpha(rgb: u32, transparency: u32) -> usize {
    CUSTOM_BASE + ((transparency.min(MAX_TRANSPARENCY) as usize) << 24 | (rgb & 0xFF_FFFF) as usize)
}

/// The same colour with its transparency removed (a palette colour is returned as it is).
pub fn opaque(color: usize) -> usize {
    custom_rgb(color).map_or(color, custom_color)
}

/// The colour setting for the hex colour `rgb`.
pub fn custom_color(rgb: u32) -> usize {
    CUSTOM_BASE + (rgb & 0xFF_FFFF) as usize
}

/// What a colour dropdown shows for a stored colour: a custom one is the "Custom" row.
pub fn shown(color: usize) -> usize {
    color.min(CUSTOM_ROW)
}

/// A dropdown row (or a typed colour, already a stored value) as the stored colour.
pub fn pick(index: usize) -> usize {
    if index >= CUSTOM_BASE { index } else { index.min(CUSTOM_ROW - 1) }
}

/// The 24 bits a colour setting shows: a custom colour's own, or the palette colour's.
pub fn color_rgb(color: usize) -> u32 {
    custom_rgb(color).unwrap_or_else(|| {
        let [r, g, b] = PALETTE.get(color).map_or([1.0; 3], |(_, rgb)| *rgb);
        (((r * 255.0).round() as u32) << 16) | (((g * 255.0).round() as u32) << 8) | (b * 255.0).round() as u32
    })
}

/// A palette colour as host RGBA, at `alpha`.
pub fn rgba(index: usize, alpha: f64) -> [f32; 4] {
    let [r, g, b] = match custom_rgb(index) {
        Some(v) => [(v >> 16 & 255) as f32 / 255.0, (v >> 8 & 255) as f32 / 255.0, (v & 255) as f32 / 255.0],
        None => PALETTE.get(index).map_or([1.0; 3], |(_, rgb)| *rgb),
    };
    let keep = 1.0 - f64::from(custom_transparency(index)) / 100.0;
    [r, g, b, (alpha * keep).clamp(0.0, 1.0) as f32]
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Corner {
    /// Where the corner is, as fractions of the free space.
    pub fn fractions(self) -> [f32; 2] {
        match self {
            Self::TopLeft => [0.0, 0.0],
            Self::TopRight => [1.0, 0.0],
            Self::BottomLeft => [0.0, 1.0],
            Self::BottomRight => [1.0, 1.0],
        }
    }

    pub fn anchor(self) -> &'static str {
        match self {
            Self::TopLeft => "top_left",
            Self::TopRight => "top_right",
            Self::BottomLeft => "bottom_left",
            Self::BottomRight => "bottom_right",
        }
    }
}

/// The hit flash choices (`Settings::hit_mode`).
pub const HIT_MODES: [&str; 3] = ["Native", "Java", "Custom"];

/// Minecraft Java's hurt flash: pure red mixed in at 77/255 (about 30%).
pub const JAVA_HIT: [f32; 4] = [1.0, 0.0, 0.0, 77.0 / 255.0];

/// How many chunks around you get their borders drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grid {
    Current,
    #[default]
    Three,
    Five,
}

impl Grid {
    pub const LABELS: [&'static str; 3] = ["Current Chunk", "3x3 Chunks", "5x5 Chunks"];

    pub fn index(self) -> usize {
        match self {
            Self::Current => 0,
            Self::Three => 1,
            Self::Five => 2,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Current,
            1 => Self::Three,
            _ => Self::Five,
        }
    }

    /// Chunks drawn on each side of the current one.
    pub fn reach(self) -> i32 {
        self.index() as i32
    }
}

/// What the CPS element shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpsMode {
    #[default]
    Both,
    Left,
    Right,
    Total,
}

impl CpsMode {
    pub const LABELS: [&'static str; 4] = ["Left + Right", "Left only", "Right only", "Total"];

    pub fn index(self) -> usize {
        match self {
            Self::Both => 0,
            Self::Left => 1,
            Self::Right => 2,
            Self::Total => 3,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Left,
            2 => Self::Right,
            3 => Self::Total,
            _ => Self::Both,
        }
    }
}

/// What the Speed element measures in (`Settings::speed_unit`).
pub const SPEED_UNITS: [&str; 2] = ["Blocks/sec", "km/h"];
/// The decimals Speed can show: the choice's index is the number of decimals.
pub const SPEED_DECIMALS: [&str; 4] = ["0", "1", "2", "3"];
pub const MAX_SPEED_DECIMALS: usize = 3;
/// How Playtime writes the time (`Settings::playtime_format`): minutes can pass 59 in the first,
/// the second always has hours.
pub const TIME_FORMATS: [&str; 2] = ["m:ss", "h:mm:ss"];
/// The items Item counter can count (`Settings::counter_item` indexes this): the item's identifier and
/// its name, in the singular and the plural. A dropdown holds at most eight options.
pub const COUNTER_ITEMS: [(&str, &str, &str); 8] = [
    ("minecraft:arrow", "Arrow", "Arrows"),
    ("minecraft:golden_apple", "Golden Apple", "Golden Apples"),
    ("minecraft:ender_pearl", "Ender Pearl", "Ender Pearls"),
    ("minecraft:snowball", "Snowball", "Snowballs"),
    ("minecraft:egg", "Egg", "Eggs"),
    ("minecraft:splash_potion", "Splash Potion", "Splash Potions"),
    ("minecraft:tnt", "TNT", "TNT"),
    ("minecraft:totem_of_undying", "Totem", "Totems"),
];
/// The names of `COUNTER_ITEMS`, for the menu's dropdown.
pub const COUNTER_ITEM_NAMES: [&str; 8] = ["Arrow", "Golden Apple", "Ender Pearl", "Snowball", "Egg", "Splash Potion", "TNT", "Totem"];

/// Keys that start something instead of switching an option on or off: the Stopwatch's start /
/// pause and reset. They are bound like the option keys (same `Settings::keys`, same capture, same
/// share codes) and sit after `TOGGLES` in the key slots, so codes made before them still read.
/// View angles: the most decimals, and the alignments of its lines.
pub const VA_MAX_DECIMALS: usize = 4;
pub const VA_ALIGN: [&str; 3] = ["Left", "Center", "Right"];
/// WAILA: when the extra lines show.
pub const WAILA_DETAILS: [&str; 3] = ["Always", "Hold key", "Never"];
pub const ACTION_KEYS: [(&str, &str); 2] = [("sw_pause", "Stopwatch start / pause"), ("sw_reset", "Stopwatch reset")];
/// The switches of the elements added after the first share codes (Reach display and on). Their
/// keys sit after the actions in the key slots, so the slots of every older code keep their meaning.
pub const LATER_TOGGLES: [(&str, &str); 12] = [
    ("hud_reach", "Reach display"),
    ("hud_combo", "Combo counter"),
    ("hud_food", "Saturation"),
    ("hud_day", "Day counter"),
    ("hud_potion", "Potion status"),
    ("hud_arraylist", "Array list"),
    ("hud_itemcounter", "Item counter"),
    ("hud_inventory", "Inventory HUD"),
    ("hud_clock", "Clock"),
    ("hud_viewangles", "View angles"),
    ("hud_potioncount", "Potion counter"),
    ("hud_totems", "Totem counter"),
];
/// The switches of the mods that live on their own pages (crosshair, zoom, the gameplay mods) and the item delay fix: each can have
/// a key like the others. After the later options in the key slots, so every older code keeps its meaning.
pub const MORE_TOGGLES: [(&str, &str); 51] = [
    ("item_delay_fix", "Item delay fix"),
    ("ch_on", "Crosshair"),
    ("zoom_on", "Zoom"),
    ("gp_toggle_sprint", "Toggle sprint"),
    ("gp_toggle_sneak", "Toggle sneak"),
    ("gp_always_sprint", "Always sprint"),
    ("gp_time_on", "Time changer"),
    ("gp_weather_on", "Weather changer"),
    ("gp_no_hurt_cam", "Hurt camera"),
    ("gp_cursor_center", "Center cursor"),
    ("gp_nametags", "Nametags"),
    ("hud_scoreboard", "Scoreboard"),
    ("gp_hit_on", "Hit color"),
    ("gp_blur_on", "Menu blur"),
    ("gp_dmg_on", "Damage indicator"),
    ("hud_target", "Target HUD"),
    ("hud_waypoints", "Waypoints"),
    // The Waypoints keys: not switches (`toggle_mut` knows nothing of them), actions like the Stopwatch's.
    ("wp_add", "Add waypoint"),
    ("wp_remove", "Remove nearest waypoint"),
    ("wp_undo", "Undo waypoint removal"),
    ("gp_wp_hidden", "Hide waypoints"),
    ("wp_quick", "Quick-create waypoint"),
    ("wp_menu", "Waypoint menu"),
    // The handler mods (Part 2).
    ("gp_nocam_on", "No cam reset"),
    ("gp_noswing_on", "No swing sounds"),
    ("gp_cpslim_on", "CPS limiter"),
    ("gp_bo_on", "Block outline"),
    ("hud_waila", "WAILA"),
    ("hud_height", "Height limit"),
    ("hud_subtitles", "Audio subtitles"),
    ("gp_mk_on", "Markers"),
    // The Markers keys: actions, not switches.
    ("mk_place", "Place marker"),
    ("mk_clear", "Clear markers"),
    ("mk_danger", "Place danger marker"),
    ("waila_details", "Show WAILA details"),
    ("gp_tnt_on", "TNT countdown"),
    ("hud_pvp", "PvP Info"),
    ("gp_th_on", "Auto Text Hotkey"),
    // The Auto Text Hotkey keys: actions, one per command.
    ("th_key0", "Text hotkey 1"),
    ("th_key1", "Text hotkey 2"),
    ("th_key2", "Text hotkey 3"),
    ("th_key3", "Text hotkey 4"),
    ("gp_hive_on", "Hive auto requeue"),
    // The Hive key: an action (queue the next game now).
    ("hive_rq", "Requeue now"),
    ("gp_persp_on", "Perspective"),
    ("persp_cycle", "Cycle perspective"),
    ("persp_behind", "Look behind"),
    ("persp_free", "Freelook"),
    ("persp_snap", "Snaplook"),
    ("persp_snap_fwd", "Snaplook forward view"),
    ("gp_glass_on", "Glass"),
];
/// Every key a player can bind to something: the options, then the actions, then the later options.
/// A share code stores a slot in five bits (codes of version 2 and before, so up to 32 slots) or in
/// six bits (version 3 on). Slots past the first 63 travel in the share code's last section (`code::CODED_SLOTS`),
/// so there is no upper limit on this count.
pub const KEY_SLOTS: usize = TOGGLES.len() + ACTION_KEYS.len() + LATER_TOGGLES.len() + MORE_TOGGLES.len();

/// The id of key slot `i` (an option's, then an action's, then a later option's).
pub fn key_slot_id(i: usize) -> Option<&'static str> {
    if i < TOGGLES.len() {
        return Some(TOGGLES[i].0);
    }
    let i = i - TOGGLES.len();
    if i < ACTION_KEYS.len() {
        return Some(ACTION_KEYS[i].0);
    }
    let i = i - ACTION_KEYS.len();
    if i < LATER_TOGGLES.len() {
        return Some(LATER_TOGGLES[i].0);
    }
    MORE_TOGGLES.get(i - LATER_TOGGLES.len()).map(|(id, _)| *id)
}

/// The key slot of an option or action id.
pub fn key_slot_position(id: &str) -> Option<usize> {
    (0..KEY_SLOTS).find(|i| key_slot_id(*i) == Some(id))
}

/// A group of elements in the picker.
pub struct Group {
    pub name: &'static str,
    pub elements: &'static [Element],
    pub labels: &'static [&'static str],
}

/// The HUD elements. Each one is drawn, placed, sized, coloured and switched on and off on its
/// own. The order is the order of the menu's "Editing" choice (at most eight options).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    Keystrokes,
    Cps,
    Fps,
    Coords,
    Facing,
    Chunk,
    Slime,
    Monument,
    /// The worn armour with its durability (item icons drawn by the host).
    Armor,
    /// Retired: the Arrow counter, folded into Item counter (which counts arrows by default). The slot stays
    /// so the settings, share codes and key slots of earlier builds keep their places; it is never shown
    /// or drawn, and `Settings::sanitize` moves a setup that had it on to Item counter.
    Arrows,
    /// Round-trip time to the server, in milliseconds.
    Ping,
    /// How fast you move, in blocks per second.
    Speed,
    /// A stopwatch that runs while the element is on.
    Stopwatch,
    /// How long Vantage has been running this session.
    Playtime,
    /// The distance of your last hit.
    Reach,
    /// Hits in a row.
    Combo,
    /// Hunger and saturation.
    Food,
    /// The world's day count.
    Day,
    /// Active potion effects, with the game's icons (Lunar's Potion Effects).
    Potion,
    /// The Vantage elements that are switched on, one per line.
    ArrayList,
    /// How many of one chosen item you carry (`Settings::counter_item`).
    ItemCounter,
    /// The main inventory drawn on screen, with the stack counts.
    Inventory,
    /// The local time of day.
    Clock,
    /// Where you look: yaw and pitch in degrees.
    ViewAngles,
    /// How many healing potions and mushroom stews you carry.
    PotionCounter,
    /// How many times your totem has popped.
    TotemCounter,
    /// The server's sidebar scoreboard, restyled (the game's own is hidden while this is on).
    Scoreboard,
    /// Name and health of the player or mob you aim at or just hit.
    TargetHud,
    /// The nearest marked spots with distance and compass direction.
    Waypoints,
    /// The name of the block you aim at (WAILA: what am I looking at), with its identifier and position.
    Waila,
    /// How many blocks are left up to the world's height limit.
    HeightLimit,
    /// The sounds you just heard, with the side they came from.
    AudioSubtitles,
    /// This session's fights: hits, accuracy, kills, deaths.
    PvpInfo,
}

impl Element {
    pub const COUNT: usize = 33;
    /// The elements share codes know: Scoreboard, Target HUD, Waypoints and WAILA come in the code's last section (`code::put_last`).
    pub const CODED_END: usize = 26;
    /// The elements the share codes keep in their own looks section (the first ones and Reach, Combo,
    /// Saturation and Day). Potion status, after them, has a section of its own (`code::put_potion`).
    pub const CODED: usize = 18;
    /// The elements after Potion status (Array list, Item counter, Inventory HUD): the share codes
    /// write them in a section of their own at the very end (`code::put_round_one`).
    pub const AFTER_POTION: usize = 19;
    /// The elements after Inventory HUD (Clock, View angles, Potion counter): the share codes write them in a
    /// section of their own after the Round one section (`code::put_extras`).
    pub const AFTER_ROUND_ONE: usize = 22;
    /// The elements the first share codes and setups know, in their order. The ones after them are
    /// written in a section of their own at the end of a code (see `code.rs`).
    pub const FIRST: usize = 14;
    pub const ALL: [Element; 33] = [
        Element::Keystrokes,
        Element::Cps,
        Element::Fps,
        Element::Coords,
        Element::Facing,
        Element::Chunk,
        Element::Slime,
        Element::Monument,
        Element::Armor,
        Element::Arrows,
        Element::Ping,
        Element::Speed,
        Element::Stopwatch,
        Element::Playtime,
        Element::Reach,
        Element::Combo,
        Element::Food,
        Element::Day,
        Element::Potion,
        Element::ArrayList,
        Element::ItemCounter,
        Element::Inventory,
        Element::Clock,
        Element::ViewAngles,
        Element::PotionCounter,
        Element::TotemCounter,
        Element::Scoreboard,
        Element::TargetHud,
        Element::Waypoints,
        Element::Waila,
        Element::HeightLimit,
        Element::AudioSubtitles,
        Element::PvpInfo,
    ];
    /// The names shown in the "Editing" choice.
    pub const LABELS: [&'static str; 33] = [
        "Keystrokes",
        "CPS",
        "FPS",
        "Coordinates",
        "Direction",
        "Chunk info",
        "Slime chunk",
        "Guardians",
        "Armor status",
        "Arrow counter",
        "Ping",
        "Momentum",
        "Stopwatch",
        "Playtime",
        "Reach display",
        "Combo counter",
        "Saturation",
        "Day counter",
        "Potion status",
        "Array list",
        "Item counter",
        "Inventory HUD",
        "Clock",
        "View angles",
        "Potion counter",
        "Totem counter",
        "Scoreboard",
        "Target HUD",
        "Waypoints",
        "WAILA",
        "Height limit",
        "Audio subtitles",
        "PvP Info",
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// The element picker's two dropdowns: a group, then an element of it. A dropdown holds at
    /// most eight options, so elements are grouped; a new element goes into a group (or a new
    /// group) here, and in `tools/gen_menu.py`'s `GROUPS`.
    pub const GROUPS: [Group; 4] = [
        Group {
            name: "Player",
            elements: &[Element::Keystrokes, Element::Cps, Element::Fps, Element::Armor, Element::Ping, Element::Speed, Element::Stopwatch, Element::ViewAngles],
            labels: &["Keystrokes", "CPS", "FPS", "Armor status", "Ping", "Momentum", "Stopwatch", "View angles"],
        },
        Group {
            name: "World",
            elements: &[Element::Coords, Element::Facing, Element::Chunk, Element::Slime, Element::Monument, Element::Playtime, Element::Day, Element::Clock],
            labels: &["Coordinates", "Direction", "Chunk info", "Slime chunk", "Guardians", "Playtime", "Day counter", "Clock"],
        },
        Group {
            name: "Combat",
            elements: &[Element::Reach, Element::Combo, Element::PotionCounter, Element::TotemCounter, Element::TargetHud, Element::AudioSubtitles, Element::PvpInfo],
            labels: &["Reach display", "Combo counter", "Potion counter", "Totem counter", "Target HUD", "Audio subtitles", "PvP Info"],
        },
        Group {
            name: "More",
            elements: &[Element::Food, Element::Potion, Element::ArrayList, Element::ItemCounter, Element::Inventory, Element::Scoreboard, Element::Waila, Element::HeightLimit],
            labels: &["Saturation", "Potion status", "Array list", "Item counter", "Inventory HUD", "Scoreboard", "WAILA", "Height limit"],
        },
    ];
    pub const GROUP_NAMES: [&'static str; 4] = [Self::GROUPS[0].name, Self::GROUPS[1].name, Self::GROUPS[2].name, Self::GROUPS[3].name];

    /// The group this element is in and its place there.
    pub fn group_position(self) -> (usize, usize) {
        for (g, group) in Self::GROUPS.iter().enumerate() {
            if let Some(i) = group.elements.iter().position(|e| *e == self) {
                return (g, i);
            }
        }
        (0, 0)
    }

    pub fn from_index(index: usize) -> Element {
        Self::ALL[index.min(Self::COUNT - 1)]
    }

    pub fn label(self) -> &'static str {
        Self::LABELS[self.index()]
    }

    /// The menu's id for this element's on/off switch.
    pub fn toggle_id(self) -> &'static str {
        match self {
            Self::Keystrokes => "hud_keystrokes",
            Self::Cps => "hud_cps",
            Self::Fps => "hud_fps",
            Self::Coords => "hud_coords",
            Self::Facing => "hud_facing",
            Self::Chunk => "hud_chunk",
            Self::Slime => "hud_slime",
            Self::Monument => "hud_monument",
            Self::Armor => "hud_armor",
            Self::Arrows => "hud_arrows",
            Self::Ping => "hud_ping",
            Self::Speed => "hud_speed",
            Self::Stopwatch => "hud_stopwatch",
            Self::Playtime => "hud_playtime",
            Self::Reach => "hud_reach",
            Self::Combo => "hud_combo",
            Self::Food => "hud_food",
            Self::Day => "hud_day",
            Self::Potion => "hud_potion",
            Self::ArrayList => "hud_arraylist",
            Self::ItemCounter => "hud_itemcounter",
            Self::Inventory => "hud_inventory",
            Self::Clock => "hud_clock",
            Self::ViewAngles => "hud_viewangles",
            Self::PotionCounter => "hud_potioncount",
            Self::TotemCounter => "hud_totems",
            Self::Scoreboard => "hud_scoreboard",
            Self::TargetHud => "hud_target",
            Self::Waypoints => "hud_waypoints",
            Self::Waila => "hud_waila",
            Self::HeightLimit => "hud_height",
            Self::AudioSubtitles => "hud_subtitles",
            Self::PvpInfo => "hud_pvp",
        }
    }

    pub fn from_toggle_id(id: &str) -> Option<Element> {
        Self::ALL.iter().copied().find(|e| e.toggle_id() == id)
    }

    /// Where the element starts: kept apart so nothing overlaps on a fresh install.
    pub fn default_style(self) -> Style {
        let mut style = Style::default();
        style.enabled = matches!(self, Self::Keystrokes | Self::Cps);
        // FPS starts in plain text; its green/yellow/red colouring is the "Status colors" switch.
        // Armor status colours its durability numbers by how worn each piece is.
        style.smart = matches!(self, Self::Slime | Self::Monument | Self::Armor);
        if self == Self::Waila {
            // Jade's dark theme: a near-black box at 70% with a faint lighter edge.
            style.bg_color = custom_color(0x131313);
            style.opacity = 70.0;
            style.border = true;
            style.border_color = custom_color(0x2E2E2E);
            style.border_width = 1.0;
            style.border_opacity = 90.0;
            // The tuned look: square corners.
            style.rounded = false;
        }
        let (corner, position) = match self {
            Self::Keystrokes => (Corner::TopLeft, None),
            // Under the keys, to the left.
            Self::Cps => (Corner::TopLeft, Some([0.0, 0.3])),
            Self::Fps => (Corner::TopRight, None),
            Self::Coords => (Corner::TopRight, Some([1.0, 0.1])),
            // The compass strip: top centre, like Lunar's.
            Self::Facing => (Corner::TopLeft, Some([0.5, 0.0])),
            Self::Chunk => (Corner::TopRight, Some([1.0, 0.3])),
            Self::Slime => (Corner::TopRight, Some([1.0, 0.45])),
            Self::Monument => (Corner::TopRight, Some([1.0, 0.55])),
            Self::Armor => (Corner::TopLeft, Some([0.0, 0.5])),
            Self::Arrows => (Corner::TopLeft, Some([0.0, 0.4])),
            Self::Ping => (Corner::TopRight, Some([1.0, 0.05])),
            Self::Speed => (Corner::TopLeft, Some([0.0, 0.6])),
            Self::Stopwatch => (Corner::TopLeft, Some([0.0, 0.7])),
            Self::Playtime => (Corner::TopLeft, Some([0.0, 0.8])),
            Self::Reach => (Corner::TopRight, Some([1.0, 0.65])),
            Self::Combo => (Corner::TopRight, Some([1.0, 0.75])),
            Self::Food => (Corner::TopRight, Some([1.0, 0.85])),
            Self::Day => (Corner::TopRight, Some([1.0, 0.95])),
            Self::Potion => (Corner::TopLeft, Some([0.0, 0.67])),
            Self::ArrayList => (Corner::TopRight, Some([1.0, 0.5])),
            Self::ItemCounter => (Corner::TopLeft, Some([0.0, 0.35])),
            Self::Inventory => (Corner::TopLeft, Some([0.5, 0.78])),
            Self::Clock => (Corner::TopRight, Some([1.0, 0.0])),
            Self::ViewAngles => (Corner::TopLeft, Some([0.0, 0.9])),
            Self::PotionCounter => (Corner::TopLeft, Some([0.0, 0.95])),
            Self::TotemCounter => (Corner::TopLeft, Some([0.0, 0.85])),
            Self::Scoreboard => (Corner::TopRight, Some([1.0, 0.4])),
            Self::TargetHud => (Corner::TopLeft, Some([0.5, 0.62])),
            Self::Waypoints => (Corner::TopLeft, Some([0.0, 0.45])),
            // Under the compass strip, in the middle.
            Self::Waila => (Corner::TopLeft, Some([0.5, 0.02])),
            Self::HeightLimit => (Corner::TopRight, Some([1.0, 0.2])),
            Self::AudioSubtitles => (Corner::TopLeft, Some([1.0, 0.9])),
            Self::PvpInfo => (Corner::TopLeft, Some([0.0, 0.2])),
        };
        style.corner = corner;
        style.position = position;
        if self == Self::Facing {
            // Lunar's compass has square corners.
            style.rounded = false;
        }
        if self == Self::Inventory {
            // Lunar's look: clear cells with the slot lines; the Border card is an optional outline round the box.
            style.background = false;
            style.rounded = false;
            style.border = false;
            style.border_color = 1;
            style.border_opacity = 100.0;
        }
        style
    }
}

/// Everything about one HUD element.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct Style {
    pub enabled: bool,
    /// The corner it is pinned to, unless it has been dragged somewhere else.
    pub corner: Corner,
    /// Where the editor left it, as fractions of the free space on each axis.
    pub position: Option<[f32; 2]>,
    pub scale: f64,
    /// How much wider and taller than its size the element's box is stretched (1 is not at all).
    /// The text keeps its size and stays in the middle of the box.
    pub stretch: [f64; 2],
    /// Indices into `PALETTE`.
    pub bg_color: usize,
    pub text_color: usize,
    /// Keystrokes only.
    pub pressed_color: usize,
    pub pressed_text_color: usize,
    /// Opacity of the background, in percent.
    pub opacity: f64,
    /// Text elements: say what the number is ("FPS 144") or show it bare ("144").
    pub labels: bool,
    /// Colour by value (green/yellow/red FPS; slime chunk and guardians use their active colour
    /// when active and the text colour otherwise).
    pub smart: bool,
    /// Rounded corners on the background (Lunar's look); off gives square boxes.
    pub rounded: bool,
    /// Draw the box behind the element; off leaves just the text (or the keys' text and pressed
    /// colour).
    pub background: bool,
    /// Drop shadow behind the element's text. The same whether the background is on, off or at 0%.
    pub shadow: bool,
    /// FPS and CPS: wrap the text in brackets, like Lunar's "[144 FPS]".
    pub brackets: bool,
    /// FPS and CPS: put the label first ("FPS 144") instead of after the number ("144 FPS").
    pub reverse: bool,
    /// Whether the box follows the text as the number grows. None: the element's own default
    /// (CPS keeps a fixed width, the rest follow their text).
    pub auto_width: Option<bool>,
    /// Slime chunk and Guardians: the colour of the "active" state (YES in a slime chunk,
    /// guardians found) while Status colors is on. None: the element's own default.
    pub active_color: Option<usize>,
    /// Chunk info: which lines to show, and whether the subchunk line includes its Y range.
    pub show_chunk: bool,
    pub show_inchunk: bool,
    pub show_sub: bool,
    pub sub_range: bool,
    /// Armor status: pieces side by side instead of stacked.
    pub armor_horizontal: bool,
    /// Armor status: which side of each icon the numbers sit on (see `ARMOR_SIDE`).
    pub armor_side: usize,
    /// Armor status: what to write next to each piece (see `ARMOR_TEXT`).
    pub armor_text: usize,
    /// Armor status: also show the item in your hand, with its durability or stack size.
    pub armor_held: bool,
    /// Armor status: which of helmet, chestplate, leggings and boots show (the held item has
    /// `armor_held`).
    pub armor_pieces: [bool; 4],
    /// Armor status: every piece (the held item too) is placed on its own instead of in a row.
    pub armor_split: bool,
    /// Armor status, when split: where each piece's top-left corner sits inside the element, in
    /// GUI pixels before scaling (helmet, chestplate, leggings, boots, held item).
    pub armor_offsets: [[f32; 2]; 5],
    /// Armor status, when split: how much wider and taller each piece (helmet .. held item) is drawn
    /// than the element's size makes it. A corner handle changes both, a side's middle one of them.
    pub armor_sizes: [[f32; 2]; 5],
    /// Armor status: pixels between pieces.
    pub armor_gap: f64,
    /// Armor status: the colours of a healthy, a worn and a nearly broken piece (palette indices),
    /// used while Status colors is on.
    pub armor_good: usize,
    pub armor_mid: usize,
    pub armor_low: usize,
    /// A line around the inside of the box (Lunar's Border): on or off, how thick it is in GUI
    /// pixels at size 1 (`MIN_BORDER` to `MAX_BORDER`) and its colour. Every element with a box (each
    /// key of Keystrokes has one).
    pub border: bool,
    pub border_width: f64,
    pub border_color: usize,
    /// How solid the border is, in percent.
    pub border_opacity: f64,
    /// CPS: the colour of the bar between the left and right counts (Lunar's Line Color).
    /// None: the text colour, which is what the bar had before it had its own.
    pub bar_color: Option<usize>,
}

#[cfg(test)]
impl Style {
    /// Every field written, as the derived `Serialize` wrote them before only changed fields were.
    pub fn whole_json(&self) -> serde_json::Value {
        let Style { enabled, corner, position, scale, stretch, bg_color, text_color, pressed_color, pressed_text_color, opacity, labels, smart, rounded, background, shadow, brackets, reverse, auto_width, active_color, show_chunk, show_inchunk, show_sub, sub_range, armor_horizontal, armor_side, armor_text, armor_held, armor_pieces, armor_split, armor_offsets, armor_sizes, armor_gap, armor_good, armor_mid, armor_low, border, border_width, border_color, border_opacity, bar_color } = self;
        let mut map = serde_json::Map::new();
        map.insert("enabled".into(), serde_json::to_value(enabled).unwrap());
        map.insert("corner".into(), serde_json::to_value(corner).unwrap());
        map.insert("position".into(), serde_json::to_value(position).unwrap());
        map.insert("scale".into(), serde_json::to_value(scale).unwrap());
        map.insert("stretch".into(), serde_json::to_value(stretch).unwrap());
        map.insert("bg_color".into(), serde_json::to_value(bg_color).unwrap());
        map.insert("text_color".into(), serde_json::to_value(text_color).unwrap());
        map.insert("pressed_color".into(), serde_json::to_value(pressed_color).unwrap());
        map.insert("pressed_text_color".into(), serde_json::to_value(pressed_text_color).unwrap());
        map.insert("opacity".into(), serde_json::to_value(opacity).unwrap());
        map.insert("labels".into(), serde_json::to_value(labels).unwrap());
        map.insert("smart".into(), serde_json::to_value(smart).unwrap());
        map.insert("rounded".into(), serde_json::to_value(rounded).unwrap());
        map.insert("background".into(), serde_json::to_value(background).unwrap());
        map.insert("shadow".into(), serde_json::to_value(shadow).unwrap());
        map.insert("brackets".into(), serde_json::to_value(brackets).unwrap());
        map.insert("reverse".into(), serde_json::to_value(reverse).unwrap());
        map.insert("auto_width".into(), serde_json::to_value(auto_width).unwrap());
        map.insert("active_color".into(), serde_json::to_value(active_color).unwrap());
        map.insert("show_chunk".into(), serde_json::to_value(show_chunk).unwrap());
        map.insert("show_inchunk".into(), serde_json::to_value(show_inchunk).unwrap());
        map.insert("show_sub".into(), serde_json::to_value(show_sub).unwrap());
        map.insert("sub_range".into(), serde_json::to_value(sub_range).unwrap());
        map.insert("armor_horizontal".into(), serde_json::to_value(armor_horizontal).unwrap());
        map.insert("armor_side".into(), serde_json::to_value(armor_side).unwrap());
        map.insert("armor_text".into(), serde_json::to_value(armor_text).unwrap());
        map.insert("armor_held".into(), serde_json::to_value(armor_held).unwrap());
        map.insert("armor_pieces".into(), serde_json::to_value(armor_pieces).unwrap());
        map.insert("armor_split".into(), serde_json::to_value(armor_split).unwrap());
        map.insert("armor_offsets".into(), serde_json::to_value(armor_offsets).unwrap());
        map.insert("armor_sizes".into(), serde_json::to_value(armor_sizes).unwrap());
        map.insert("armor_gap".into(), serde_json::to_value(armor_gap).unwrap());
        map.insert("armor_good".into(), serde_json::to_value(armor_good).unwrap());
        map.insert("armor_mid".into(), serde_json::to_value(armor_mid).unwrap());
        map.insert("armor_low".into(), serde_json::to_value(armor_low).unwrap());
        map.insert("border".into(), serde_json::to_value(border).unwrap());
        map.insert("border_width".into(), serde_json::to_value(border_width).unwrap());
        map.insert("border_color".into(), serde_json::to_value(border_color).unwrap());
        map.insert("border_opacity".into(), serde_json::to_value(border_opacity).unwrap());
        map.insert("bar_color".into(), serde_json::to_value(bar_color).unwrap());
        serde_json::Value::Object(map)
    }
}


/// A style is written with only the fields that differ from `Style::default()`: a missing field
/// reads back as that same default (`#[serde(default)]`), so nothing changes when the file is
/// loaded, by this version or an earlier one. Every element's style used to be written whole
/// (19 elements, about 900 bytes each), which made the settings larger than the 16 KiB the host
/// accepts, so it refused to save them, and made loading them most of the start-up's fuel.
impl Serialize for Style {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let defaults = Style::default();
        // Every field, by name: a field added to `Style` and not listed here does not compile.
        let Style { enabled, corner, position, scale, stretch, bg_color, text_color, pressed_color, pressed_text_color, opacity, labels, smart, rounded, background, shadow, brackets, reverse, auto_width, active_color, show_chunk, show_inchunk, show_sub, sub_range, armor_horizontal, armor_side, armor_text, armor_held, armor_pieces, armor_split, armor_offsets, armor_sizes, armor_gap, armor_good, armor_mid, armor_low, border, border_width, border_color, border_opacity, bar_color } = self;
        // serde_json writes a struct announced with no fields as `{}` at once, so the count is exact.
        let count = 0 
            + usize::from(*enabled != defaults.enabled)
            + usize::from(*corner != defaults.corner)
            + usize::from(*position != defaults.position)
            + usize::from(*scale != defaults.scale)
            + usize::from(*stretch != defaults.stretch)
            + usize::from(*bg_color != defaults.bg_color)
            + usize::from(*text_color != defaults.text_color)
            + usize::from(*pressed_color != defaults.pressed_color)
            + usize::from(*pressed_text_color != defaults.pressed_text_color)
            + usize::from(*opacity != defaults.opacity)
            + usize::from(*labels != defaults.labels)
            + usize::from(*smart != defaults.smart)
            + usize::from(*rounded != defaults.rounded)
            + usize::from(*background != defaults.background)
            + usize::from(*shadow != defaults.shadow)
            + usize::from(*brackets != defaults.brackets)
            + usize::from(*reverse != defaults.reverse)
            + usize::from(*auto_width != defaults.auto_width)
            + usize::from(*active_color != defaults.active_color)
            + usize::from(*show_chunk != defaults.show_chunk)
            + usize::from(*show_inchunk != defaults.show_inchunk)
            + usize::from(*show_sub != defaults.show_sub)
            + usize::from(*sub_range != defaults.sub_range)
            + usize::from(*armor_horizontal != defaults.armor_horizontal)
            + usize::from(*armor_side != defaults.armor_side)
            + usize::from(*armor_text != defaults.armor_text)
            + usize::from(*armor_held != defaults.armor_held)
            + usize::from(*armor_pieces != defaults.armor_pieces)
            + usize::from(*armor_split != defaults.armor_split)
            + usize::from(*armor_offsets != defaults.armor_offsets)
            + usize::from(*armor_sizes != defaults.armor_sizes)
            + usize::from(*armor_gap != defaults.armor_gap)
            + usize::from(*armor_good != defaults.armor_good)
            + usize::from(*armor_mid != defaults.armor_mid)
            + usize::from(*armor_low != defaults.armor_low)
            + usize::from(*border != defaults.border)
            + usize::from(*border_width != defaults.border_width)
            + usize::from(*border_color != defaults.border_color)
            + usize::from(*border_opacity != defaults.border_opacity)
            + usize::from(*bar_color != defaults.bar_color);
        let mut out = serializer.serialize_struct("Style", count)?;
        if *enabled != defaults.enabled {
            out.serialize_field("enabled", enabled)?;
        }
        if *corner != defaults.corner {
            out.serialize_field("corner", corner)?;
        }
        if *position != defaults.position {
            out.serialize_field("position", position)?;
        }
        if *scale != defaults.scale {
            out.serialize_field("scale", scale)?;
        }
        if *stretch != defaults.stretch {
            out.serialize_field("stretch", stretch)?;
        }
        if *bg_color != defaults.bg_color {
            out.serialize_field("bg_color", bg_color)?;
        }
        if *text_color != defaults.text_color {
            out.serialize_field("text_color", text_color)?;
        }
        if *pressed_color != defaults.pressed_color {
            out.serialize_field("pressed_color", pressed_color)?;
        }
        if *pressed_text_color != defaults.pressed_text_color {
            out.serialize_field("pressed_text_color", pressed_text_color)?;
        }
        if *opacity != defaults.opacity {
            out.serialize_field("opacity", opacity)?;
        }
        if *labels != defaults.labels {
            out.serialize_field("labels", labels)?;
        }
        if *smart != defaults.smart {
            out.serialize_field("smart", smart)?;
        }
        if *rounded != defaults.rounded {
            out.serialize_field("rounded", rounded)?;
        }
        if *background != defaults.background {
            out.serialize_field("background", background)?;
        }
        if *shadow != defaults.shadow {
            out.serialize_field("shadow", shadow)?;
        }
        if *brackets != defaults.brackets {
            out.serialize_field("brackets", brackets)?;
        }
        if *reverse != defaults.reverse {
            out.serialize_field("reverse", reverse)?;
        }
        if *auto_width != defaults.auto_width {
            out.serialize_field("auto_width", auto_width)?;
        }
        if *active_color != defaults.active_color {
            out.serialize_field("active_color", active_color)?;
        }
        if *show_chunk != defaults.show_chunk {
            out.serialize_field("show_chunk", show_chunk)?;
        }
        if *show_inchunk != defaults.show_inchunk {
            out.serialize_field("show_inchunk", show_inchunk)?;
        }
        if *show_sub != defaults.show_sub {
            out.serialize_field("show_sub", show_sub)?;
        }
        if *sub_range != defaults.sub_range {
            out.serialize_field("sub_range", sub_range)?;
        }
        if *armor_horizontal != defaults.armor_horizontal {
            out.serialize_field("armor_horizontal", armor_horizontal)?;
        }
        if *armor_side != defaults.armor_side {
            out.serialize_field("armor_side", armor_side)?;
        }
        if *armor_text != defaults.armor_text {
            out.serialize_field("armor_text", armor_text)?;
        }
        if *armor_held != defaults.armor_held {
            out.serialize_field("armor_held", armor_held)?;
        }
        if *armor_pieces != defaults.armor_pieces {
            out.serialize_field("armor_pieces", armor_pieces)?;
        }
        if *armor_split != defaults.armor_split {
            out.serialize_field("armor_split", armor_split)?;
        }
        if *armor_offsets != defaults.armor_offsets {
            out.serialize_field("armor_offsets", armor_offsets)?;
        }
        if *armor_sizes != defaults.armor_sizes {
            out.serialize_field("armor_sizes", armor_sizes)?;
        }
        if *armor_gap != defaults.armor_gap {
            out.serialize_field("armor_gap", armor_gap)?;
        }
        if *armor_good != defaults.armor_good {
            out.serialize_field("armor_good", armor_good)?;
        }
        if *armor_mid != defaults.armor_mid {
            out.serialize_field("armor_mid", armor_mid)?;
        }
        if *armor_low != defaults.armor_low {
            out.serialize_field("armor_low", armor_low)?;
        }
        if *border != defaults.border {
            out.serialize_field("border", border)?;
        }
        if *border_width != defaults.border_width {
            out.serialize_field("border_width", border_width)?;
        }
        if *border_color != defaults.border_color {
            out.serialize_field("border_color", border_color)?;
        }
        if *border_opacity != defaults.border_opacity {
            out.serialize_field("border_opacity", border_opacity)?;
        }
        if *bar_color != defaults.bar_color {
            out.serialize_field("bar_color", bar_color)?;
        }
        out.end()
    }
}

/// What Armor status writes beside a piece.
pub const ARMOR_TEXT: [&str; 4] = ["Durability", "Percent", "Left / max", "Off"];
/// Where the numbers sit next to each icon: Left and Right share its line, Above and Below stack.
pub const ARMOR_SIDE: [&str; 4] = ["Left", "Right", "Above", "Below"];
pub const MAX_ARMOR_GAP: f64 = 8.0;
/// Effect ids that can be moved on their own in Potion status (index = id).
pub const POTION_SLOTS: usize = 40;
/// The range of a moved-apart effect's own size.
pub const POTION_MIN_SCALE: f32 = 0.4;
pub const POTION_MAX_SCALE: f32 = 4.0;
/// How far an effect can be moved from its place in the list, in GUI pixels.
pub const MAX_POTION_OFFSET: f32 = 1500.0;
/// The farthest (GUI pixels) a split piece can sit from the element's corner.
pub const MAX_ARMOR_OFFSET: f32 = 4000.0;
/// The range of a box's border thickness (GUI pixels at size 1).
pub const MIN_BORDER: f64 = 0.5;
pub const MAX_BORDER: f64 = 4.0;

impl Default for Style {
    fn default() -> Self {
        Self {
            enabled: false,
            corner: Corner::TopLeft,
            position: None,
            scale: 1.0,
            stretch: [1.0, 1.0],
            bg_color: BLACK,
            text_color: WHITE,
            pressed_color: WHITE,
            pressed_text_color: BLACK,
            opacity: 50.0,
            labels: true,
            smart: false,
            rounded: true,
            background: true,
            shadow: true,
            brackets: false,
            reverse: false,
            auto_width: None,
            active_color: None,
            show_chunk: true,
            show_inchunk: true,
            show_sub: true,
            sub_range: true,
            armor_horizontal: false,
            armor_side: 0,
            armor_text: 0,
            armor_held: false,
            armor_pieces: [true; 4],
            armor_split: false,
            armor_offsets: [[0.0; 2]; 5],
            armor_sizes: [[1.0; 2]; 5],
            armor_gap: 2.0,
            armor_good: 4,
            armor_mid: 6,
            armor_low: 3,
            border: false,
            border_width: 1.0,
            border_color: BLACK,
            border_opacity: 100.0,
            bar_color: None,
        }
    }
}

impl Style {
    pub fn snap_to(&mut self, corner: Corner) {
        self.corner = corner;
        self.position = None;
    }

    fn sanitize(&mut self) {
        let defaults = Style::default();
        self.scale = if self.scale.is_finite() { self.scale.clamp(MIN_SCALE, MAX_SCALE) } else { defaults.scale };
        for (value, default) in self.stretch.iter_mut().zip(defaults.stretch) {
            *value = if value.is_finite() { value.clamp(MIN_STRETCH, MAX_STRETCH) } else { default };
        }
        self.opacity = if self.opacity.is_finite() { self.opacity.clamp(0.0, 100.0) } else { defaults.opacity };
        for color in [&mut self.bg_color, &mut self.text_color, &mut self.pressed_color, &mut self.pressed_text_color] {
            if !valid_color(*color) {
                *color = WHITE;
            }
        }
        for color in [&mut self.armor_good, &mut self.armor_mid, &mut self.armor_low, &mut self.border_color] {
            if !valid_color(*color) {
                *color = WHITE;
            }
        }
        self.armor_text = self.armor_text.min(ARMOR_TEXT.len() - 1);
        self.armor_side = self.armor_side.min(ARMOR_SIDE.len() - 1);
        for offset in &mut self.armor_offsets {
            for v in offset.iter_mut() {
                *v = if v.is_finite() { v.clamp(0.0, MAX_ARMOR_OFFSET) } else { 0.0 };
            }
        }
        for size in &mut self.armor_sizes {
            for v in size.iter_mut() {
                *v = if v.is_finite() { v.clamp(ARMOR_MIN_SIZE, ARMOR_MAX_SIZE) } else { 1.0 };
            }
        }
        self.armor_gap = if self.armor_gap.is_finite() { self.armor_gap.clamp(0.0, MAX_ARMOR_GAP) } else { defaults.armor_gap };
        if self.active_color.is_some_and(|c| !valid_color(c)) {
            self.active_color = None;
        }
        if self.bar_color.is_some_and(|c| !valid_color(c)) {
            self.bar_color = None;
        }
        self.border_opacity = if self.border_opacity.is_finite() { self.border_opacity.clamp(0.0, 100.0) } else { defaults.border_opacity };
        self.border_width =
            if self.border_width.is_finite() { self.border_width.clamp(MIN_BORDER, MAX_BORDER) } else { defaults.border_width };
        if let Some(position) = &mut self.position {
            if position.iter().all(|v| v.is_finite()) {
                *position = position.map(|v| v.clamp(0.0, 1.0));
            } else {
                self.position = None;
            }
        }
    }
}

/// Every on/off option with its menu label: the world tools, then the HUD elements. Each can
/// also have its own key.
pub const TOGGLES: [(&str, &str); 23] = [
    ("borders", "Chunk borders"),
    ("subchunks", "Subchunks"),
    ("slime_chunks", "Slime chunks"),
    ("guardians", "Guardian spawns"),
    ("lights", "Light spots"),
    ("blaze", "Blaze spawners"),
    ("hitboxes", "Hitboxes"),
    ("hud_keystrokes", "Keystrokes"),
    ("hud_cps", "CPS"),
    ("hud_fps", "FPS"),
    ("hud_coords", "Coordinates"),
    ("hud_facing", "Direction"),
    ("hud_chunk", "Chunk info"),
    ("hud_slime", "Slime chunk"),
    ("hud_monument", "Guardians"),
    ("hud_armor", "Armor status"),
    ("hud_arrows", "Arrow counter"),
    ("hud_ping", "Ping"),
    ("hud_speed", "Momentum"),
    ("hud_stopwatch", "Stopwatch"),
    ("hud_playtime", "Playtime"),
    ("fullbright", "Fullbright"),
    ("hide_scoreboard", "Hide scoreboard"),
];

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub borders: bool,
    pub grid: Grid,
    pub subchunks: bool,
    pub slime_chunks: bool,
    pub guardians: bool,
    /// Where to put lights on flat ground: a diagonal grid, 5 and 5 blocks apart.
    pub lights: bool,
    /// Spawners in the Nether (blaze spawners in fortresses), with their spawn and
    /// activation areas.
    pub blaze: bool,
    /// Boxes around nearby mobs and players, with where they look.
    pub hitboxes: bool,
    /// Hitbox colours (indices into `PALETTE`): mobs and other entities, and players (you too).
    pub hitbox_color: usize,
    pub hitbox_player_color: usize,
    /// Custom hurt flash on hit actors (Lunar's Hit Color): its colour (index into `PALETTE`) and
    /// opacity in percent.
    /// Which flash: 0 native, 1 Java's, 2 the colour and opacity below.
    pub hit_mode: usize,
    pub hit_color: usize,
    pub hit_opacity: f64,
    /// Full light everywhere (a local lighting override; needs the fullbright permission).
    pub fullbright: bool,
    /// Item delay fix: a right-click skips the game's wait between item uses (needs the item use permission).
    pub item_delay_fix: bool,
    /// Hide the game's scoreboard completely (the Scoreboard options' "Hide scoreboard"): nothing is drawn in its place either.
    pub hide_scoreboard: bool,
    /// Scoreboard: show each row's score (the numbers the server keeps), like Cinnabar Client's Show scores.
    pub sb_scores: bool,
    /// WAILA: show the block's identifier ("minecraft:oak_log") under its name.
    pub waila_id: bool,
    /// WAILA: show the block's position under its name.
    pub waila_pos: bool,
    /// WAILA: show the mod the block belongs to ("Minecraft") under its name.
    pub waila_mod: bool,
    /// WAILA: show a crop's growth, like Jade.
    pub waila_growth: bool,
    /// WAILA: show the tool that mines the block fastest.
    pub waila_tool: bool,
    /// WAILA: show how far the block is.
    pub waila_dist: bool,
    /// WAILA: show a redstone block's signal strength.
    pub waila_state: bool,
    /// WAILA: show the mob the crosshair is on, with its health.
    pub waila_entity: bool,
    /// WAILA: the colour of the mod's name (Jade's blue).
    pub waila_mod_color: usize,
    /// WAILA: the colour of the lines under the name (growth, power, distance, health).
    pub waila_info_color: usize,
    /// WAILA: the colour of a good sign (a mature crop, a tool that can mine the block).
    pub waila_good_color: usize,
    /// WAILA: the colour of a bad sign (a tool that cannot mine the block).
    pub waila_bad_color: usize,
    /// View angles: show the yaw.
    pub va_yaw: bool,
    /// View angles: show the pitch.
    pub va_pitch: bool,
    /// View angles: show the speed (blocks per second).
    pub va_speed: bool,
    /// View angles: show the height (Y).
    pub va_altitude: bool,
    /// View angles: show in first person.
    pub va_first: bool,
    /// View angles: show in third person.
    pub va_third: bool,
    /// View angles: show only while flying with an elytra.
    pub va_elytra: bool,
    /// View angles: the lines in reverse order.
    pub va_reverse: bool,
    /// View angles: the pitch turns green inside the flight target range.
    pub va_target: bool,
    /// View angles: how many decimals the numbers have (0 to 4).
    pub va_decimals: usize,
    /// View angles: where the lines sit in the box (0 left, 1 centre, 2 right).
    pub va_align: usize,
    /// View angles: the colour of the labels (Lunar's orange).
    pub va_label_color: usize,
    /// View angles: the colour of the numbers (Lunar's grey).
    pub va_value_color: usize,
    /// Coordinates: show X.
    pub co_x: bool,
    /// Coordinates: show Y.
    pub co_y: bool,
    /// Coordinates: show Z.
    pub co_z: bool,
    /// Coordinates: show the direction you face and its axis sign, at the right.
    pub co_dir: bool,
    /// Coordinates: show two decimals.
    pub co_decimal: bool,
    /// Coordinates: the list runs across in one line.
    pub co_horizontal: bool,
    /// Coordinates: X has its label.
    pub co_x_label: bool,
    /// Coordinates: Y has its label.
    pub co_y_label: bool,
    /// Coordinates: Z has its label.
    pub co_z_label: bool,
    /// Coordinates: the direction shows its letter.
    pub co_dir_cardinal: bool,
    /// Coordinates: the direction shows its axis sign.
    pub co_dir_mod: bool,
    /// Coordinates: the colour of the X label.
    pub co_x_lc: usize,
    /// Coordinates: the colour of the X number.
    pub co_x_tc: usize,
    /// Coordinates: the colour of the Y label.
    pub co_y_lc: usize,
    /// Coordinates: the colour of the Y number.
    pub co_y_tc: usize,
    /// Coordinates: the colour of the Z label.
    pub co_z_lc: usize,
    /// Coordinates: the colour of the Z number.
    pub co_z_tc: usize,
    /// Coordinates: the colour of the direction letter.
    pub co_dir_tc: usize,
    /// Coordinates: the colour of the direction sign along X.
    pub co_dir_mx: usize,
    /// Coordinates: the colour of the direction sign along Z.
    pub co_dir_mz: usize,
    /// PvP Info: the colour of the Hits label.
    pub pvp_hits_lc: usize,
    /// PvP Info: the colour of the Accuracy label.
    pub pvp_acc_lc: usize,
    /// PvP Info: the colour of the Kills label.
    pub pvp_kills_lc: usize,
    /// PvP Info: the colour of the Deaths label.
    pub pvp_deaths_lc: usize,
    /// PvP Info: the colour of the numbers value.
    pub pvp_num_tc: usize,
    /// PvP Info: the colour of the percentage value.
    pub pvp_pct_tc: usize,
    /// Inventory HUD: draw the empty slots too (off: only the slots that hold an item).
    pub inv_empty: bool,
    /// Inventory HUD: draw the background at all (off: the items alone).
    pub inv_show_bg: bool,
    /// Inventory HUD: Minecraft background, Lunar's bevelled slot lines (off: the thin Grid below).
    pub inv_mc: bool,
    /// Inventory HUD: thin grid lines (while the Minecraft background is off).
    pub inv_grid: bool,
    /// Inventory HUD: the colour of the thin grid lines.
    pub inv_grid_color: usize,
    /// Inventory HUD: the opacity of the grid lines, in percent.
    pub inv_grid_opacity: f64,
    /// Direction compass: the numbers and letters follow the element's Chroma too (off: only the ticks do).
    pub facing_label_chroma: bool,
    /// WAILA: when the extra lines (mod name, power, distance, identifier, position) show: always, only while the Show details key is held, or never.
    pub waila_details: usize,
    pub line_width: f64,
    pub menu_key: String,
    /// The Mods grid's starred cards: the ids of their toggles (`MODS` in menu_controls.rs).
    pub favorites: Vec<String>,
    /// The Mods grid's hidden cards: the ids of their toggles.
    pub hidden_mods: Vec<String>,
    /// The mods left out of the share codes made here (the Sharing page): their numbers in `share::MODS`.
    pub share_skip: Vec<u8>,
    /// The Keystrokes element's rows.
    pub show_mouse: bool,
    pub show_space: bool,
    /// Dragging snaps to nearby guides (screen edges and centre, other elements and pieces).
    pub align: bool,
    /// Lines through the middle of the screen while the elements are being moved.
    pub show_guides: bool,
    /// The redesigned menu is used where it exists (the Mods page).
    pub new_menu: bool,
    pub cps_mode: CpsMode,
    /// Speed: the unit (index into `SPEED_UNITS`), how many decimals it shows (0 to
    /// `MAX_SPEED_DECIMALS`) and whether climbing and falling count (off: only along the ground).
    pub speed_unit: usize,
    pub speed_decimals: usize,
    pub speed_vertical: bool,
    /// Clock: 12-hour time with AM / PM (off: 24-hour), and whether the seconds show.
    pub clock_12h: bool,
    pub clock_seconds: bool,
    /// Direction: the Lunar-style compass strip (off, the default) or the plain text line (on).
    pub facing_text: bool,
    /// Stopwatch: keep counting while the element is switched off (otherwise switching it off
    /// throws the time away, and switching it on starts again from zero).
    pub stopwatch_keep: bool,
    /// Playtime: how the time is written (index into `TIME_FORMATS`), and whether it counts only
    /// while a world is loaded (otherwise from when Vantage started).
    pub playtime_format: usize,
    pub playtime_world_only: bool,
    /// Item counter: which item it counts (index into `COUNTER_ITEMS`).
    pub counter_item: usize,
    /// Reach display: show nothing while the reach reads 0 (Lunar's Hide 0 Blocks), and outline the
    /// players within hitting distance in a colour (Lunar's Highlight Players in Range).
    pub reach_hide_zero: bool,
    pub reach_highlight: bool,
    pub reach_highlight_color: usize,
    /// Saturation (the Food element, Lunar's Saturation HUD with AppleSkin): the golden overlay on
    /// the hunger bar, the flashing preview of the hunger and the saturation the food in your hand
    /// would give, the preview's inverse colour, and the overlay's colour.
    pub sat_overlay: bool,
    pub sat_hunger: bool,
    pub sat_potential: bool,
    pub sat_inverse: bool,
    pub sat_color: usize,
    /// The hotbar layout the overlay is placed for (index into `saturation::Layout::NAMES`).
    pub sat_layout: usize,
    /// Potion status (Lunar's Potion Effects): how the effects are drawn (index into
    /// `potion::MODES`), side by side with `potion_per_line` to a line, what is written, and what the
    /// list does with the game's own effect icons.
    pub potion_mode: usize,
    pub potion_horizontal: bool,
    pub potion_per_line: usize,
    pub potion_name: bool,
    pub potion_level: bool,
    pub potion_duration: bool,
    pub potion_formatted: bool,
    pub potion_upper: bool,
    pub potion_blink: bool,
    pub potion_color_name: bool,
    pub potion_color_info: bool,
    pub potion_icons: bool,
    pub potion_hide_vanilla: bool,
    /// Potion status: every effect has its own box and can be moved on its own.
    pub potion_split: bool,
    /// Potion status, when split: how far each effect (by the game's effect id) is moved from the place
    /// the list gives it, in GUI pixels. `POTION_SLOTS` long.
    pub potion_offsets: Vec<[f32; 2]>,
    /// Potion status, when split: how much bigger or smaller each effect is drawn (1 is as the list has it).
    pub potion_scales: Vec<f32>,
    /// One entry per `Element`, in order.
    pub elements: Vec<Style>,
    /// The key of each option in `TOGGLES` and each action in `ACTION_KEYS`, by id; absent when
    /// unbound.
    pub keys: BTreeMap<String, String>,
    /// The Zoomify-based zoom (off until switched on in the Zoom page).
    pub zoom: ZoomSettings,
    /// The gameplay mods with no HUD element (sprint and sneak toggles, time, weather, hurt cam, nametags, cursor).
    pub gameplay: GameplaySettings,
    /// The custom crosshair (off until switched on in the Crosshair page).
    pub crosshair: CrosshairSettings,
    /// The saved setups (see `setups.rs`): one entry per slot, empty when the slot is.
    pub profiles: Vec<String>,
    /// The names given to the slots (empty: unnamed).
    pub names: Vec<String>,
}

#[cfg(test)]
impl Settings {
    /// Every field written, as the derived `Serialize` wrote them before only changed fields were.
    pub fn whole_json(&self) -> serde_json::Value {
        let Settings { borders, grid, subchunks, slime_chunks, guardians, lights, blaze, hitboxes, hitbox_color, hitbox_player_color, hit_mode, hit_color, hit_opacity, fullbright, item_delay_fix, hide_scoreboard, sb_scores, waila_id, waila_pos, waila_mod, waila_growth, waila_tool, waila_dist, waila_state, waila_entity, waila_mod_color, waila_info_color, waila_good_color, waila_bad_color, va_yaw, va_pitch, va_speed, va_altitude, va_first, va_third, va_elytra, va_reverse, va_target, va_decimals, va_align, va_label_color, va_value_color, co_x, co_y, co_z, co_dir, co_decimal, co_horizontal, co_x_label, co_y_label, co_z_label, co_dir_cardinal, co_dir_mod, co_x_lc, co_x_tc, co_y_lc, co_y_tc, co_z_lc, co_z_tc, co_dir_tc, co_dir_mx, co_dir_mz, pvp_hits_lc, pvp_acc_lc, pvp_kills_lc, pvp_deaths_lc, pvp_num_tc, pvp_pct_tc, inv_empty, inv_show_bg, inv_mc, inv_grid, inv_grid_color, inv_grid_opacity, facing_label_chroma, waila_details, line_width, menu_key, favorites, hidden_mods, share_skip, show_mouse, show_space, align, show_guides, new_menu, cps_mode, speed_unit, speed_decimals, speed_vertical, facing_text, clock_12h, clock_seconds, stopwatch_keep, playtime_format, playtime_world_only, counter_item, reach_hide_zero, reach_highlight, reach_highlight_color, sat_overlay, sat_hunger, sat_potential, sat_inverse, sat_color, sat_layout, potion_mode, potion_horizontal, potion_per_line, potion_name, potion_level, potion_duration, potion_formatted, potion_upper, potion_blink, potion_color_name, potion_color_info, potion_icons, potion_hide_vanilla, potion_split, potion_offsets, potion_scales, elements, keys, zoom, gameplay, crosshair, profiles, names } = self;
        let mut map = serde_json::Map::new();
        map.insert("borders".into(), serde_json::to_value(borders).unwrap());
        map.insert("grid".into(), serde_json::to_value(grid).unwrap());
        map.insert("subchunks".into(), serde_json::to_value(subchunks).unwrap());
        map.insert("slime_chunks".into(), serde_json::to_value(slime_chunks).unwrap());
        map.insert("guardians".into(), serde_json::to_value(guardians).unwrap());
        map.insert("lights".into(), serde_json::to_value(lights).unwrap());
        map.insert("blaze".into(), serde_json::to_value(blaze).unwrap());
        map.insert("hitboxes".into(), serde_json::to_value(hitboxes).unwrap());
        map.insert("hitbox_color".into(), serde_json::to_value(hitbox_color).unwrap());
        map.insert("hitbox_player_color".into(), serde_json::to_value(hitbox_player_color).unwrap());
        map.insert("hit_mode".into(), serde_json::to_value(hit_mode).unwrap());
        map.insert("hit_color".into(), serde_json::to_value(hit_color).unwrap());
        map.insert("hit_opacity".into(), serde_json::to_value(hit_opacity).unwrap());
        map.insert("fullbright".into(), serde_json::to_value(fullbright).unwrap());
        map.insert("item_delay_fix".into(), serde_json::to_value(item_delay_fix).unwrap());
        map.insert("hide_scoreboard".into(), serde_json::to_value(hide_scoreboard).unwrap());
        map.insert("sb_scores".into(), serde_json::to_value(sb_scores).unwrap());
        map.insert("waila_id".into(), serde_json::to_value(waila_id).unwrap());
        map.insert("waila_pos".into(), serde_json::to_value(waila_pos).unwrap());
        map.insert("waila_mod".into(), serde_json::to_value(waila_mod).unwrap());
        map.insert("waila_growth".into(), serde_json::to_value(waila_growth).unwrap());
        map.insert("waila_tool".into(), serde_json::to_value(waila_tool).unwrap());
        map.insert("waila_dist".into(), serde_json::to_value(waila_dist).unwrap());
        map.insert("waila_state".into(), serde_json::to_value(waila_state).unwrap());
        map.insert("waila_entity".into(), serde_json::to_value(waila_entity).unwrap());
        map.insert("waila_mod_color".into(), serde_json::to_value(waila_mod_color).unwrap());
        map.insert("waila_info_color".into(), serde_json::to_value(waila_info_color).unwrap());
        map.insert("waila_good_color".into(), serde_json::to_value(waila_good_color).unwrap());
        map.insert("waila_bad_color".into(), serde_json::to_value(waila_bad_color).unwrap());
        map.insert("va_yaw".into(), serde_json::to_value(va_yaw).unwrap());
        map.insert("va_pitch".into(), serde_json::to_value(va_pitch).unwrap());
        map.insert("va_speed".into(), serde_json::to_value(va_speed).unwrap());
        map.insert("va_altitude".into(), serde_json::to_value(va_altitude).unwrap());
        map.insert("va_first".into(), serde_json::to_value(va_first).unwrap());
        map.insert("va_third".into(), serde_json::to_value(va_third).unwrap());
        map.insert("va_elytra".into(), serde_json::to_value(va_elytra).unwrap());
        map.insert("va_reverse".into(), serde_json::to_value(va_reverse).unwrap());
        map.insert("va_target".into(), serde_json::to_value(va_target).unwrap());
        map.insert("va_decimals".into(), serde_json::to_value(va_decimals).unwrap());
        map.insert("va_align".into(), serde_json::to_value(va_align).unwrap());
        map.insert("va_label_color".into(), serde_json::to_value(va_label_color).unwrap());
        map.insert("va_value_color".into(), serde_json::to_value(va_value_color).unwrap());
        map.insert("co_x".into(), serde_json::to_value(co_x).unwrap());
        map.insert("co_y".into(), serde_json::to_value(co_y).unwrap());
        map.insert("co_z".into(), serde_json::to_value(co_z).unwrap());
        map.insert("co_dir".into(), serde_json::to_value(co_dir).unwrap());
        map.insert("co_decimal".into(), serde_json::to_value(co_decimal).unwrap());
        map.insert("co_horizontal".into(), serde_json::to_value(co_horizontal).unwrap());
        map.insert("co_x_label".into(), serde_json::to_value(co_x_label).unwrap());
        map.insert("co_y_label".into(), serde_json::to_value(co_y_label).unwrap());
        map.insert("co_z_label".into(), serde_json::to_value(co_z_label).unwrap());
        map.insert("co_dir_cardinal".into(), serde_json::to_value(co_dir_cardinal).unwrap());
        map.insert("co_dir_mod".into(), serde_json::to_value(co_dir_mod).unwrap());
        map.insert("co_x_lc".into(), serde_json::to_value(co_x_lc).unwrap());
        map.insert("co_x_tc".into(), serde_json::to_value(co_x_tc).unwrap());
        map.insert("co_y_lc".into(), serde_json::to_value(co_y_lc).unwrap());
        map.insert("co_y_tc".into(), serde_json::to_value(co_y_tc).unwrap());
        map.insert("co_z_lc".into(), serde_json::to_value(co_z_lc).unwrap());
        map.insert("co_z_tc".into(), serde_json::to_value(co_z_tc).unwrap());
        map.insert("co_dir_tc".into(), serde_json::to_value(co_dir_tc).unwrap());
        map.insert("co_dir_mx".into(), serde_json::to_value(co_dir_mx).unwrap());
        map.insert("co_dir_mz".into(), serde_json::to_value(co_dir_mz).unwrap());
        map.insert("pvp_hits_lc".into(), serde_json::to_value(pvp_hits_lc).unwrap());
        map.insert("pvp_acc_lc".into(), serde_json::to_value(pvp_acc_lc).unwrap());
        map.insert("pvp_kills_lc".into(), serde_json::to_value(pvp_kills_lc).unwrap());
        map.insert("pvp_deaths_lc".into(), serde_json::to_value(pvp_deaths_lc).unwrap());
        map.insert("pvp_num_tc".into(), serde_json::to_value(pvp_num_tc).unwrap());
        map.insert("pvp_pct_tc".into(), serde_json::to_value(pvp_pct_tc).unwrap());
        map.insert("inv_empty".into(), serde_json::to_value(inv_empty).unwrap());
        map.insert("inv_show_bg".into(), serde_json::to_value(inv_show_bg).unwrap());
        map.insert("inv_mc".into(), serde_json::to_value(inv_mc).unwrap());
        map.insert("inv_grid".into(), serde_json::to_value(inv_grid).unwrap());
        map.insert("inv_grid_color".into(), serde_json::to_value(inv_grid_color).unwrap());
        map.insert("inv_grid_opacity".into(), serde_json::to_value(inv_grid_opacity).unwrap());
        map.insert("facing_label_chroma".into(), serde_json::to_value(facing_label_chroma).unwrap());
        map.insert("waila_details".into(), serde_json::to_value(waila_details).unwrap());
        map.insert("line_width".into(), serde_json::to_value(line_width).unwrap());
        map.insert("menu_key".into(), serde_json::to_value(menu_key).unwrap());
        map.insert("favorites".into(), serde_json::to_value(favorites).unwrap());
        map.insert("hidden_mods".into(), serde_json::to_value(hidden_mods).unwrap());
        map.insert("share_skip".into(), serde_json::to_value(share_skip).unwrap());
        map.insert("show_mouse".into(), serde_json::to_value(show_mouse).unwrap());
        map.insert("show_space".into(), serde_json::to_value(show_space).unwrap());
        map.insert("align".into(), serde_json::to_value(align).unwrap());
        map.insert("show_guides".into(), serde_json::to_value(show_guides).unwrap());
        map.insert("new_menu".into(), serde_json::to_value(new_menu).unwrap());
        map.insert("cps_mode".into(), serde_json::to_value(cps_mode).unwrap());
        map.insert("speed_unit".into(), serde_json::to_value(speed_unit).unwrap());
        map.insert("speed_decimals".into(), serde_json::to_value(speed_decimals).unwrap());
        map.insert("speed_vertical".into(), serde_json::to_value(speed_vertical).unwrap());
        map.insert("facing_text".into(), serde_json::to_value(facing_text).unwrap());
        map.insert("clock_12h".into(), serde_json::to_value(clock_12h).unwrap());
        map.insert("clock_seconds".into(), serde_json::to_value(clock_seconds).unwrap());
        map.insert("stopwatch_keep".into(), serde_json::to_value(stopwatch_keep).unwrap());
        map.insert("playtime_format".into(), serde_json::to_value(playtime_format).unwrap());
        map.insert("playtime_world_only".into(), serde_json::to_value(playtime_world_only).unwrap());
        map.insert("counter_item".into(), serde_json::to_value(counter_item).unwrap());
        map.insert("reach_hide_zero".into(), serde_json::to_value(reach_hide_zero).unwrap());
        map.insert("reach_highlight".into(), serde_json::to_value(reach_highlight).unwrap());
        map.insert("reach_highlight_color".into(), serde_json::to_value(reach_highlight_color).unwrap());
        map.insert("sat_overlay".into(), serde_json::to_value(sat_overlay).unwrap());
        map.insert("sat_hunger".into(), serde_json::to_value(sat_hunger).unwrap());
        map.insert("sat_potential".into(), serde_json::to_value(sat_potential).unwrap());
        map.insert("sat_inverse".into(), serde_json::to_value(sat_inverse).unwrap());
        map.insert("sat_color".into(), serde_json::to_value(sat_color).unwrap());
        map.insert("sat_layout".into(), serde_json::to_value(sat_layout).unwrap());
        map.insert("potion_mode".into(), serde_json::to_value(potion_mode).unwrap());
        map.insert("potion_horizontal".into(), serde_json::to_value(potion_horizontal).unwrap());
        map.insert("potion_per_line".into(), serde_json::to_value(potion_per_line).unwrap());
        map.insert("potion_name".into(), serde_json::to_value(potion_name).unwrap());
        map.insert("potion_level".into(), serde_json::to_value(potion_level).unwrap());
        map.insert("potion_duration".into(), serde_json::to_value(potion_duration).unwrap());
        map.insert("potion_formatted".into(), serde_json::to_value(potion_formatted).unwrap());
        map.insert("potion_upper".into(), serde_json::to_value(potion_upper).unwrap());
        map.insert("potion_blink".into(), serde_json::to_value(potion_blink).unwrap());
        map.insert("potion_color_name".into(), serde_json::to_value(potion_color_name).unwrap());
        map.insert("potion_color_info".into(), serde_json::to_value(potion_color_info).unwrap());
        map.insert("potion_icons".into(), serde_json::to_value(potion_icons).unwrap());
        map.insert("potion_hide_vanilla".into(), serde_json::to_value(potion_hide_vanilla).unwrap());
        map.insert("potion_split".into(), serde_json::to_value(potion_split).unwrap());
        map.insert("potion_offsets".into(), serde_json::to_value(potion_offsets).unwrap());
        map.insert("potion_scales".into(), serde_json::to_value(potion_scales).unwrap());
        map.insert("elements".into(), serde_json::Value::Array(elements.iter().map(Style::whole_json).collect()));
        map.insert("keys".into(), serde_json::to_value(keys).unwrap());
        map.insert("zoom".into(), zoom.whole_json());
        map.insert("gameplay".into(), serde_json::to_value(gameplay).unwrap());
        map.insert("crosshair".into(), crosshair.whole_json());
        map.insert("profiles".into(), serde_json::to_value(profiles).unwrap());
        map.insert("names".into(), serde_json::to_value(names).unwrap());
        serde_json::Value::Object(map)
    }
}


/// Written with only the fields that differ from `Settings::default()`: a missing field reads back as
/// that same default (`#[serde(default)]`), so the settings load exactly as when every field was
/// written, by this version or an earlier one, from a much smaller file (see `Style`'s `Serialize`).
impl serde::Serialize for Settings {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let defaults = Settings::default();
        // Every field, by name: a field added to `Settings` and not listed here does not compile.
        let Settings { borders, grid, subchunks, slime_chunks, guardians, lights, blaze, hitboxes, hitbox_color, hitbox_player_color, hit_mode, hit_color, hit_opacity, fullbright, item_delay_fix, hide_scoreboard, sb_scores, waila_id, waila_pos, waila_mod, waila_growth, waila_tool, waila_dist, waila_state, waila_entity, waila_mod_color, waila_info_color, waila_good_color, waila_bad_color, va_yaw, va_pitch, va_speed, va_altitude, va_first, va_third, va_elytra, va_reverse, va_target, va_decimals, va_align, va_label_color, va_value_color, co_x, co_y, co_z, co_dir, co_decimal, co_horizontal, co_x_label, co_y_label, co_z_label, co_dir_cardinal, co_dir_mod, co_x_lc, co_x_tc, co_y_lc, co_y_tc, co_z_lc, co_z_tc, co_dir_tc, co_dir_mx, co_dir_mz, pvp_hits_lc, pvp_acc_lc, pvp_kills_lc, pvp_deaths_lc, pvp_num_tc, pvp_pct_tc, inv_empty, inv_show_bg, inv_mc, inv_grid, inv_grid_color, inv_grid_opacity, facing_label_chroma, waila_details, line_width, menu_key, favorites, hidden_mods, share_skip, show_mouse, show_space, align, show_guides, new_menu, cps_mode, speed_unit, speed_decimals, speed_vertical, facing_text, clock_12h, clock_seconds, stopwatch_keep, playtime_format, playtime_world_only, counter_item, reach_hide_zero, reach_highlight, reach_highlight_color, sat_overlay, sat_hunger, sat_potential, sat_inverse, sat_color, sat_layout, potion_mode, potion_horizontal, potion_per_line, potion_name, potion_level, potion_duration, potion_formatted, potion_upper, potion_blink, potion_color_name, potion_color_info, potion_icons, potion_hide_vanilla, potion_split, potion_offsets, potion_scales, elements, keys, zoom, gameplay, crosshair, profiles, names } = self;
        // serde_json writes a struct announced with no fields as `{}` at once, so the count is exact.
        let count = 0
            + usize::from(*borders != defaults.borders)
            + usize::from(*grid != defaults.grid)
            + usize::from(*subchunks != defaults.subchunks)
            + usize::from(*slime_chunks != defaults.slime_chunks)
            + usize::from(*guardians != defaults.guardians)
            + usize::from(*lights != defaults.lights)
            + usize::from(*blaze != defaults.blaze)
            + usize::from(*hitboxes != defaults.hitboxes)
            + usize::from(*hitbox_color != defaults.hitbox_color)
            + usize::from(*hitbox_player_color != defaults.hitbox_player_color)
            + usize::from(*hit_mode != defaults.hit_mode)
            + usize::from(*hit_color != defaults.hit_color)
            + usize::from(*hit_opacity != defaults.hit_opacity)
            + usize::from(*fullbright != defaults.fullbright)
            + usize::from(*item_delay_fix != defaults.item_delay_fix)
            + usize::from(*hide_scoreboard != defaults.hide_scoreboard)
            + usize::from(*sb_scores != defaults.sb_scores)
            + usize::from(*waila_id != defaults.waila_id)
            + usize::from(*waila_pos != defaults.waila_pos)
            + usize::from(*waila_mod != defaults.waila_mod)
            + usize::from(*waila_growth != defaults.waila_growth)
            + usize::from(*waila_tool != defaults.waila_tool)
            + usize::from(*waila_dist != defaults.waila_dist)
            + usize::from(*waila_state != defaults.waila_state)
            + usize::from(*waila_entity != defaults.waila_entity)
            + usize::from(*waila_mod_color != defaults.waila_mod_color)
            + usize::from(*waila_info_color != defaults.waila_info_color)
            + usize::from(*waila_good_color != defaults.waila_good_color)
            + usize::from(*waila_bad_color != defaults.waila_bad_color)
            + usize::from(*va_yaw != defaults.va_yaw)
            + usize::from(*va_pitch != defaults.va_pitch)
            + usize::from(*va_speed != defaults.va_speed)
            + usize::from(*va_altitude != defaults.va_altitude)
            + usize::from(*va_first != defaults.va_first)
            + usize::from(*va_third != defaults.va_third)
            + usize::from(*va_elytra != defaults.va_elytra)
            + usize::from(*va_reverse != defaults.va_reverse)
            + usize::from(*va_target != defaults.va_target)
            + usize::from(*va_decimals != defaults.va_decimals)
            + usize::from(*va_align != defaults.va_align)
            + usize::from(*va_label_color != defaults.va_label_color)
            + usize::from(*va_value_color != defaults.va_value_color)
            + usize::from(*co_x != defaults.co_x)
            + usize::from(*co_y != defaults.co_y)
            + usize::from(*co_z != defaults.co_z)
            + usize::from(*co_dir != defaults.co_dir)
            + usize::from(*co_decimal != defaults.co_decimal)
            + usize::from(*co_horizontal != defaults.co_horizontal)
            + usize::from(*co_x_label != defaults.co_x_label)
            + usize::from(*co_y_label != defaults.co_y_label)
            + usize::from(*co_z_label != defaults.co_z_label)
            + usize::from(*co_dir_cardinal != defaults.co_dir_cardinal)
            + usize::from(*co_dir_mod != defaults.co_dir_mod)
            + usize::from(*co_x_lc != defaults.co_x_lc)
            + usize::from(*co_x_tc != defaults.co_x_tc)
            + usize::from(*co_y_lc != defaults.co_y_lc)
            + usize::from(*co_y_tc != defaults.co_y_tc)
            + usize::from(*co_z_lc != defaults.co_z_lc)
            + usize::from(*co_z_tc != defaults.co_z_tc)
            + usize::from(*co_dir_tc != defaults.co_dir_tc)
            + usize::from(*co_dir_mx != defaults.co_dir_mx)
            + usize::from(*co_dir_mz != defaults.co_dir_mz)
            + usize::from(*pvp_hits_lc != defaults.pvp_hits_lc)
            + usize::from(*pvp_acc_lc != defaults.pvp_acc_lc)
            + usize::from(*pvp_kills_lc != defaults.pvp_kills_lc)
            + usize::from(*pvp_deaths_lc != defaults.pvp_deaths_lc)
            + usize::from(*pvp_num_tc != defaults.pvp_num_tc)
            + usize::from(*pvp_pct_tc != defaults.pvp_pct_tc)
            + usize::from(*inv_empty != defaults.inv_empty)
            + usize::from(*inv_show_bg != defaults.inv_show_bg)
            + usize::from(*inv_mc != defaults.inv_mc)
            + usize::from(*inv_grid != defaults.inv_grid)
            + usize::from(*inv_grid_color != defaults.inv_grid_color)
            + usize::from(*inv_grid_opacity != defaults.inv_grid_opacity)
            + usize::from(*facing_label_chroma != defaults.facing_label_chroma)
            + usize::from(*waila_details != defaults.waila_details)
            + usize::from(*line_width != defaults.line_width)
            + usize::from(*menu_key != defaults.menu_key)
            + usize::from(*favorites != defaults.favorites)
            + usize::from(*hidden_mods != defaults.hidden_mods)
            + usize::from(*share_skip != defaults.share_skip)
            + usize::from(*show_mouse != defaults.show_mouse)
            + usize::from(*show_space != defaults.show_space)
            + usize::from(*align != defaults.align)
            + usize::from(*show_guides != defaults.show_guides)
            + usize::from(*new_menu != defaults.new_menu)
            + usize::from(*cps_mode != defaults.cps_mode)
            + usize::from(*speed_unit != defaults.speed_unit)
            + usize::from(*speed_decimals != defaults.speed_decimals)
            + usize::from(*speed_vertical != defaults.speed_vertical)
            + usize::from(*facing_text != defaults.facing_text)
            + usize::from(*clock_12h != defaults.clock_12h)
            + usize::from(*clock_seconds != defaults.clock_seconds)
            + usize::from(*stopwatch_keep != defaults.stopwatch_keep)
            + usize::from(*playtime_format != defaults.playtime_format)
            + usize::from(*playtime_world_only != defaults.playtime_world_only)
            + usize::from(*counter_item != defaults.counter_item)
            + usize::from(*reach_hide_zero != defaults.reach_hide_zero)
            + usize::from(*reach_highlight != defaults.reach_highlight)
            + usize::from(*reach_highlight_color != defaults.reach_highlight_color)
            + usize::from(*sat_overlay != defaults.sat_overlay)
            + usize::from(*sat_hunger != defaults.sat_hunger)
            + usize::from(*sat_potential != defaults.sat_potential)
            + usize::from(*sat_inverse != defaults.sat_inverse)
            + usize::from(*sat_color != defaults.sat_color)
            + usize::from(*sat_layout != defaults.sat_layout)
            + usize::from(*potion_mode != defaults.potion_mode)
            + usize::from(*potion_horizontal != defaults.potion_horizontal)
            + usize::from(*potion_per_line != defaults.potion_per_line)
            + usize::from(*potion_name != defaults.potion_name)
            + usize::from(*potion_level != defaults.potion_level)
            + usize::from(*potion_duration != defaults.potion_duration)
            + usize::from(*potion_formatted != defaults.potion_formatted)
            + usize::from(*potion_upper != defaults.potion_upper)
            + usize::from(*potion_blink != defaults.potion_blink)
            + usize::from(*potion_color_name != defaults.potion_color_name)
            + usize::from(*potion_color_info != defaults.potion_color_info)
            + usize::from(*potion_icons != defaults.potion_icons)
            + usize::from(*potion_hide_vanilla != defaults.potion_hide_vanilla)
            + usize::from(*potion_split != defaults.potion_split)
            + usize::from(*potion_offsets != defaults.potion_offsets)
            + usize::from(*potion_scales != defaults.potion_scales)
            + usize::from(*elements != defaults.elements)
            + usize::from(*keys != defaults.keys)
            + usize::from(*zoom != defaults.zoom)
            + usize::from(*gameplay != defaults.gameplay)
            + usize::from(*crosshair != defaults.crosshair)
            + usize::from(*profiles != defaults.profiles)
            + usize::from(*names != defaults.names);
        let mut out = serializer.serialize_struct("Settings", count)?;
        if *borders != defaults.borders {
            out.serialize_field("borders", borders)?;
        }
        if *grid != defaults.grid {
            out.serialize_field("grid", grid)?;
        }
        if *subchunks != defaults.subchunks {
            out.serialize_field("subchunks", subchunks)?;
        }
        if *slime_chunks != defaults.slime_chunks {
            out.serialize_field("slime_chunks", slime_chunks)?;
        }
        if *guardians != defaults.guardians {
            out.serialize_field("guardians", guardians)?;
        }
        if *lights != defaults.lights {
            out.serialize_field("lights", lights)?;
        }
        if *blaze != defaults.blaze {
            out.serialize_field("blaze", blaze)?;
        }
        if *hitboxes != defaults.hitboxes {
            out.serialize_field("hitboxes", hitboxes)?;
        }
        if *hitbox_color != defaults.hitbox_color {
            out.serialize_field("hitbox_color", hitbox_color)?;
        }
        if *hitbox_player_color != defaults.hitbox_player_color {
            out.serialize_field("hitbox_player_color", hitbox_player_color)?;
        }
        if *hit_mode != defaults.hit_mode {
            out.serialize_field("hit_mode", hit_mode)?;
        }
        if *hit_color != defaults.hit_color {
            out.serialize_field("hit_color", hit_color)?;
        }
        if *hit_opacity != defaults.hit_opacity {
            out.serialize_field("hit_opacity", hit_opacity)?;
        }
        if *fullbright != defaults.fullbright {
            out.serialize_field("fullbright", fullbright)?;
        }
        if *item_delay_fix != defaults.item_delay_fix {
            out.serialize_field("item_delay_fix", item_delay_fix)?;
        }
        if *hide_scoreboard != defaults.hide_scoreboard {
            out.serialize_field("hide_scoreboard", hide_scoreboard)?;
        }
        if *sb_scores != defaults.sb_scores {
            out.serialize_field("sb_scores", sb_scores)?;
        }
        if *waila_id != defaults.waila_id {
            out.serialize_field("waila_id", waila_id)?;
        }
        if *waila_pos != defaults.waila_pos {
            out.serialize_field("waila_pos", waila_pos)?;
        }
        if *waila_mod != defaults.waila_mod {
            out.serialize_field("waila_mod", waila_mod)?;
        }
        if *waila_growth != defaults.waila_growth {
            out.serialize_field("waila_growth", waila_growth)?;
        }
        if *waila_tool != defaults.waila_tool {
            out.serialize_field("waila_tool", waila_tool)?;
        }
        if *waila_dist != defaults.waila_dist {
            out.serialize_field("waila_dist", waila_dist)?;
        }
        if *waila_state != defaults.waila_state {
            out.serialize_field("waila_state", waila_state)?;
        }
        if *waila_entity != defaults.waila_entity {
            out.serialize_field("waila_entity", waila_entity)?;
        }
        if *waila_mod_color != defaults.waila_mod_color {
            out.serialize_field("waila_mod_color", waila_mod_color)?;
        }
        if *waila_info_color != defaults.waila_info_color {
            out.serialize_field("waila_info_color", waila_info_color)?;
        }
        if *waila_good_color != defaults.waila_good_color {
            out.serialize_field("waila_good_color", waila_good_color)?;
        }
        if *waila_bad_color != defaults.waila_bad_color {
            out.serialize_field("waila_bad_color", waila_bad_color)?;
        }
        if *va_yaw != defaults.va_yaw {
            out.serialize_field("va_yaw", va_yaw)?;
        }
        if *va_pitch != defaults.va_pitch {
            out.serialize_field("va_pitch", va_pitch)?;
        }
        if *va_speed != defaults.va_speed {
            out.serialize_field("va_speed", va_speed)?;
        }
        if *va_altitude != defaults.va_altitude {
            out.serialize_field("va_altitude", va_altitude)?;
        }
        if *va_first != defaults.va_first {
            out.serialize_field("va_first", va_first)?;
        }
        if *va_third != defaults.va_third {
            out.serialize_field("va_third", va_third)?;
        }
        if *va_elytra != defaults.va_elytra {
            out.serialize_field("va_elytra", va_elytra)?;
        }
        if *va_reverse != defaults.va_reverse {
            out.serialize_field("va_reverse", va_reverse)?;
        }
        if *va_target != defaults.va_target {
            out.serialize_field("va_target", va_target)?;
        }
        if *va_decimals != defaults.va_decimals {
            out.serialize_field("va_decimals", va_decimals)?;
        }
        if *va_align != defaults.va_align {
            out.serialize_field("va_align", va_align)?;
        }
        if *va_label_color != defaults.va_label_color {
            out.serialize_field("va_label_color", va_label_color)?;
        }
        if *va_value_color != defaults.va_value_color {
            out.serialize_field("va_value_color", va_value_color)?;
        }
        if *co_x != defaults.co_x {
            out.serialize_field("co_x", co_x)?;
        }
        if *co_y != defaults.co_y {
            out.serialize_field("co_y", co_y)?;
        }
        if *co_z != defaults.co_z {
            out.serialize_field("co_z", co_z)?;
        }
        if *co_dir != defaults.co_dir {
            out.serialize_field("co_dir", co_dir)?;
        }
        if *co_decimal != defaults.co_decimal {
            out.serialize_field("co_decimal", co_decimal)?;
        }
        if *co_horizontal != defaults.co_horizontal {
            out.serialize_field("co_horizontal", co_horizontal)?;
        }
        if *co_x_label != defaults.co_x_label {
            out.serialize_field("co_x_label", co_x_label)?;
        }
        if *co_y_label != defaults.co_y_label {
            out.serialize_field("co_y_label", co_y_label)?;
        }
        if *co_z_label != defaults.co_z_label {
            out.serialize_field("co_z_label", co_z_label)?;
        }
        if *co_dir_cardinal != defaults.co_dir_cardinal {
            out.serialize_field("co_dir_cardinal", co_dir_cardinal)?;
        }
        if *co_dir_mod != defaults.co_dir_mod {
            out.serialize_field("co_dir_mod", co_dir_mod)?;
        }
        if *co_x_lc != defaults.co_x_lc {
            out.serialize_field("co_x_lc", co_x_lc)?;
        }
        if *co_x_tc != defaults.co_x_tc {
            out.serialize_field("co_x_tc", co_x_tc)?;
        }
        if *co_y_lc != defaults.co_y_lc {
            out.serialize_field("co_y_lc", co_y_lc)?;
        }
        if *co_y_tc != defaults.co_y_tc {
            out.serialize_field("co_y_tc", co_y_tc)?;
        }
        if *co_z_lc != defaults.co_z_lc {
            out.serialize_field("co_z_lc", co_z_lc)?;
        }
        if *co_z_tc != defaults.co_z_tc {
            out.serialize_field("co_z_tc", co_z_tc)?;
        }
        if *co_dir_tc != defaults.co_dir_tc {
            out.serialize_field("co_dir_tc", co_dir_tc)?;
        }
        if *co_dir_mx != defaults.co_dir_mx {
            out.serialize_field("co_dir_mx", co_dir_mx)?;
        }
        if *co_dir_mz != defaults.co_dir_mz {
            out.serialize_field("co_dir_mz", co_dir_mz)?;
        }
        if *pvp_hits_lc != defaults.pvp_hits_lc {
            out.serialize_field("pvp_hits_lc", pvp_hits_lc)?;
        }
        if *pvp_acc_lc != defaults.pvp_acc_lc {
            out.serialize_field("pvp_acc_lc", pvp_acc_lc)?;
        }
        if *pvp_kills_lc != defaults.pvp_kills_lc {
            out.serialize_field("pvp_kills_lc", pvp_kills_lc)?;
        }
        if *pvp_deaths_lc != defaults.pvp_deaths_lc {
            out.serialize_field("pvp_deaths_lc", pvp_deaths_lc)?;
        }
        if *pvp_num_tc != defaults.pvp_num_tc {
            out.serialize_field("pvp_num_tc", pvp_num_tc)?;
        }
        if *pvp_pct_tc != defaults.pvp_pct_tc {
            out.serialize_field("pvp_pct_tc", pvp_pct_tc)?;
        }
        if *inv_empty != defaults.inv_empty {
            out.serialize_field("inv_empty", inv_empty)?;
        }
        if *inv_show_bg != defaults.inv_show_bg {
            out.serialize_field("inv_show_bg", inv_show_bg)?;
        }
        if *inv_mc != defaults.inv_mc {
            out.serialize_field("inv_mc", inv_mc)?;
        }
        if *inv_grid != defaults.inv_grid {
            out.serialize_field("inv_grid", inv_grid)?;
        }
        if *inv_grid_color != defaults.inv_grid_color {
            out.serialize_field("inv_grid_color", inv_grid_color)?;
        }
        if *inv_grid_opacity != defaults.inv_grid_opacity {
            out.serialize_field("inv_grid_opacity", inv_grid_opacity)?;
        }
        if *facing_label_chroma != defaults.facing_label_chroma {
            out.serialize_field("facing_label_chroma", facing_label_chroma)?;
        }
        if *waila_details != defaults.waila_details {
            out.serialize_field("waila_details", waila_details)?;
        }
        if *line_width != defaults.line_width {
            out.serialize_field("line_width", line_width)?;
        }
        if *menu_key != defaults.menu_key {
            out.serialize_field("menu_key", menu_key)?;
        }
        if *favorites != defaults.favorites {
            out.serialize_field("favorites", favorites)?;
        }
        if *hidden_mods != defaults.hidden_mods {
            out.serialize_field("hidden_mods", hidden_mods)?;
        }
        if *share_skip != defaults.share_skip {
            out.serialize_field("share_skip", share_skip)?;
        }
        if *show_mouse != defaults.show_mouse {
            out.serialize_field("show_mouse", show_mouse)?;
        }
        if *show_space != defaults.show_space {
            out.serialize_field("show_space", show_space)?;
        }
        if *align != defaults.align {
            out.serialize_field("align", align)?;
        }
        if *show_guides != defaults.show_guides {
            out.serialize_field("show_guides", show_guides)?;
        }
        if *new_menu != defaults.new_menu {
            out.serialize_field("new_menu", new_menu)?;
        }
        if *cps_mode != defaults.cps_mode {
            out.serialize_field("cps_mode", cps_mode)?;
        }
        if *speed_unit != defaults.speed_unit {
            out.serialize_field("speed_unit", speed_unit)?;
        }
        if *speed_decimals != defaults.speed_decimals {
            out.serialize_field("speed_decimals", speed_decimals)?;
        }
        if *speed_vertical != defaults.speed_vertical {
            out.serialize_field("speed_vertical", speed_vertical)?;
        }
        if *facing_text != defaults.facing_text {
            out.serialize_field("facing_text", facing_text)?;
        }
        if *clock_12h != defaults.clock_12h {
            out.serialize_field("clock_12h", clock_12h)?;
        }
        if *clock_seconds != defaults.clock_seconds {
            out.serialize_field("clock_seconds", clock_seconds)?;
        }
        if *stopwatch_keep != defaults.stopwatch_keep {
            out.serialize_field("stopwatch_keep", stopwatch_keep)?;
        }
        if *playtime_format != defaults.playtime_format {
            out.serialize_field("playtime_format", playtime_format)?;
        }
        if *playtime_world_only != defaults.playtime_world_only {
            out.serialize_field("playtime_world_only", playtime_world_only)?;
        }
        if *counter_item != defaults.counter_item {
            out.serialize_field("counter_item", counter_item)?;
        }
        if *reach_hide_zero != defaults.reach_hide_zero {
            out.serialize_field("reach_hide_zero", reach_hide_zero)?;
        }
        if *reach_highlight != defaults.reach_highlight {
            out.serialize_field("reach_highlight", reach_highlight)?;
        }
        if *reach_highlight_color != defaults.reach_highlight_color {
            out.serialize_field("reach_highlight_color", reach_highlight_color)?;
        }
        if *sat_overlay != defaults.sat_overlay {
            out.serialize_field("sat_overlay", sat_overlay)?;
        }
        if *sat_hunger != defaults.sat_hunger {
            out.serialize_field("sat_hunger", sat_hunger)?;
        }
        if *sat_potential != defaults.sat_potential {
            out.serialize_field("sat_potential", sat_potential)?;
        }
        if *sat_inverse != defaults.sat_inverse {
            out.serialize_field("sat_inverse", sat_inverse)?;
        }
        if *sat_color != defaults.sat_color {
            out.serialize_field("sat_color", sat_color)?;
        }
        if *sat_layout != defaults.sat_layout {
            out.serialize_field("sat_layout", sat_layout)?;
        }
        if *potion_mode != defaults.potion_mode {
            out.serialize_field("potion_mode", potion_mode)?;
        }
        if *potion_horizontal != defaults.potion_horizontal {
            out.serialize_field("potion_horizontal", potion_horizontal)?;
        }
        if *potion_per_line != defaults.potion_per_line {
            out.serialize_field("potion_per_line", potion_per_line)?;
        }
        if *potion_name != defaults.potion_name {
            out.serialize_field("potion_name", potion_name)?;
        }
        if *potion_level != defaults.potion_level {
            out.serialize_field("potion_level", potion_level)?;
        }
        if *potion_duration != defaults.potion_duration {
            out.serialize_field("potion_duration", potion_duration)?;
        }
        if *potion_formatted != defaults.potion_formatted {
            out.serialize_field("potion_formatted", potion_formatted)?;
        }
        if *potion_upper != defaults.potion_upper {
            out.serialize_field("potion_upper", potion_upper)?;
        }
        if *potion_blink != defaults.potion_blink {
            out.serialize_field("potion_blink", potion_blink)?;
        }
        if *potion_color_name != defaults.potion_color_name {
            out.serialize_field("potion_color_name", potion_color_name)?;
        }
        if *potion_color_info != defaults.potion_color_info {
            out.serialize_field("potion_color_info", potion_color_info)?;
        }
        if *potion_icons != defaults.potion_icons {
            out.serialize_field("potion_icons", potion_icons)?;
        }
        if *potion_hide_vanilla != defaults.potion_hide_vanilla {
            out.serialize_field("potion_hide_vanilla", potion_hide_vanilla)?;
        }
        if *potion_split != defaults.potion_split {
            out.serialize_field("potion_split", potion_split)?;
        }
        if *potion_offsets != defaults.potion_offsets {
            out.serialize_field("potion_offsets", potion_offsets)?;
        }
        if *potion_scales != defaults.potion_scales {
            out.serialize_field("potion_scales", potion_scales)?;
        }
        if *elements != defaults.elements {
            out.serialize_field("elements", elements)?;
        }
        if *keys != defaults.keys {
            out.serialize_field("keys", keys)?;
        }
        if *zoom != defaults.zoom {
            out.serialize_field("zoom", zoom)?;
        }
        if *gameplay != defaults.gameplay {
            out.serialize_field("gameplay", gameplay)?;
        }
        if *crosshair != defaults.crosshair {
            out.serialize_field("crosshair", crosshair)?;
        }
        if *profiles != defaults.profiles {
            out.serialize_field("profiles", profiles)?;
        }
        if *names != defaults.names {
            out.serialize_field("names", names)?;
        }
        out.end()
    }
}

impl Default for Settings {
    #[inline(never)]
    fn default() -> Self {
        Self {
            borders: false,
            grid: Grid::Three,
            subchunks: true,
            slime_chunks: false,
            guardians: false,
            lights: false,
            blaze: false,
            hitboxes: false,
            hitbox_color: WHITE,
            hitbox_player_color: 5,
            hit_mode: 0,
            hit_color: 3,
            hit_opacity: 60.0,
            fullbright: false,
            item_delay_fix: false,
            hide_scoreboard: false,
            sb_scores: true,
            waila_id: false,
            waila_pos: false,
            waila_mod: true,
            waila_growth: true,
            waila_tool: true,
            waila_dist: false,
            waila_state: true,
            waila_entity: true,
            waila_mod_color: custom_color(0x5555FF),
            waila_info_color: custom_color(0xA0A0A0),
            waila_good_color: custom_color(0x55FF55),
            waila_bad_color: custom_color(0xFF5555),
            va_yaw: true,
            va_pitch: true,
            va_speed: true,
            va_altitude: false,
            va_first: true,
            va_third: true,
            va_elytra: false,
            va_reverse: false,
            va_target: true,
            va_decimals: 2,
            va_align: 0,
            va_label_color: custom_color(0xFFAA00),
            va_value_color: custom_color(0xAAAAAA),
            co_x: true,
            co_y: true,
            co_z: true,
            co_dir: true,
            co_decimal: false,
            co_horizontal: false,
            co_x_label: true,
            co_y_label: true,
            co_z_label: true,
            co_dir_cardinal: true,
            co_dir_mod: true,
            co_x_lc: custom_color(0xFFFFFF),
            co_x_tc: custom_color(0xFFFFFF),
            co_y_lc: custom_color(0xFFFFFF),
            co_y_tc: custom_color(0xFFFFFF),
            co_z_lc: custom_color(0xFFFFFF),
            co_z_tc: custom_color(0xFFFFFF),
            co_dir_tc: custom_color(0xFFFFFF),
            co_dir_mx: custom_color(0xFFFFFF),
            co_dir_mz: custom_color(0xFFFFFF),
            pvp_hits_lc: GREEN,
            pvp_acc_lc: 6,
            pvp_kills_lc: 5,
            pvp_deaths_lc: 3,
            pvp_num_tc: WHITE,
            pvp_pct_tc: WHITE,
            inv_empty: true,
            inv_show_bg: true,
            inv_mc: true,
            inv_grid: false,
            inv_grid_color: BLACK,
            inv_grid_opacity: 50.0,
            facing_label_chroma: false,
            waila_details: 0,
            line_width: 2.0,
            // Right Shift opens the menu (F10 in Mods Core still does); the General page rebinds it.
            menu_key: "ShiftRight".to_owned(),
            favorites: Vec::new(),
            hidden_mods: Vec::new(),
            share_skip: Vec::new(),
            show_mouse: true,
            show_space: true,
            align: true,
            show_guides: true,
            new_menu: true,
            cps_mode: CpsMode::Both,
            speed_unit: 0,
            // One decimal and the ground plane only: what Speed showed before it had options.
            speed_decimals: 1,
            speed_vertical: false,
            facing_text: false,
            clock_12h: false,
            clock_seconds: false,
            stopwatch_keep: false,
            playtime_format: 0,
            playtime_world_only: false,
            counter_item: 0,
            reach_hide_zero: false,
            reach_highlight: false,
            reach_highlight_color: GREEN,
            sat_overlay: true,
            sat_hunger: true,
            sat_potential: true,
            sat_inverse: false,
            sat_color: SAT_GOLD,
            sat_layout: 0,
            potion_mode: 0,
            potion_horizontal: false,
            potion_per_line: 4,
            potion_name: true,
            potion_level: true,
            potion_duration: true,
            potion_formatted: false,
            potion_upper: false,
            potion_blink: true,
            potion_color_name: false,
            potion_color_info: false,
            potion_icons: true,
            potion_hide_vanilla: true,
            potion_split: false,
            potion_offsets: vec![[0.0; 2]; POTION_SLOTS],
            potion_scales: vec![1.0; POTION_SLOTS],
            elements: Element::ALL.iter().map(|e| e.default_style()).collect(),
            keys: BTreeMap::new(),
            zoom: ZoomSettings::default(),
            gameplay: GameplaySettings::default(),
            crosshair: CrosshairSettings::default(),
            profiles: Vec::new(),
            names: Vec::new(),
        }
    }
}

/// The waypoints of a settings file that does not parse as a whole: each one that reads on its own.
fn salvage_waypoints(text: &str) -> Vec<crate::waypoints::Waypoint> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else { return Vec::new() };
    let Some(list) = value.pointer("/gameplay/waypoints").and_then(|v| v.as_array()) else { return Vec::new() };
    list.iter().filter_map(|w| serde_json::from_value(w.clone()).ok()).collect()
}

pub const MIN_SCALE: f64 = 0.25;
pub const MAX_SCALE: f64 = 5.0;
/// The range of a split armor piece's own size (a share of the element's size).
pub const ARMOR_MIN_SIZE: f32 = 0.4;
pub const ARMOR_MAX_SIZE: f32 = 4.0;
pub const MIN_STRETCH: f64 = 1.0;
pub const MAX_STRETCH: f64 = 5.0;
pub const MIN_WIDTH: f64 = 1.0;
pub const MAX_WIDTH: f64 = 5.0;

/// The settings one mod's own page holds (exact names, at the top and inside `gameplay`), for `Settings::reset_fields` (generated by `tools/gen_reset.py`: run it after changing a list).
/// What a mod's on/off switch is stays out: a reset puts the options back, not the mod off.
pub fn page_keys(page: &str) -> &'static [&'static str] {
    match page {
        "Chunk borders" => &["grid", "line_width"],
        "Hitboxes" => &["hitbox_color", "hitbox_player_color"],
        "Hit color" => &["hit_mode", "hit_color", "hit_opacity", "hit_armor"],
        "Time changer" => &["time_preset"],
        "Weather changer" => &["weather"],
        "Nametags" => &["name_no_background", "name_scale", "name_shadow", "name_self"],
        "Hurt camera" => &["hurt_off", "hurt_strength", "hurt_fixed"],
        "Menu blur" => &["blur_amount", "blur_dim", "blur_sat"],
        "Waypoint settings" => &[
            "wp_confirm", "wp_death", "wp_quick_name", "wp_color", "wp_world", "wp_compass", "wp_beams", "wp_borders", "wp_shadow", "wp_padding", "wp_label_scale", "wp_highlight", "wp_line", "wp_distance",
            "wp_near", "wp_hidden",
        ],
        "Damage indicator" => &["dmg_size", "dmg_time", "dmg_color", "dmg_hearts"],
        "CPS limiter" => &["cpslim_max"],
        "Block outline" => &["bo_line", "bo_mode", "bo_color", "bo_opacity", "bo_fill", "bo_fix"],
        "Markers" => &["mk_middle", "mk_time", "mk_color", "mk_danger_color"],
        "Text hotkey" => &["th_cmd"],
        "Hive" => &["hive_end", "hive_elim", "hive_solo", "hive_check", "hive_hub", "hive_log", "hive_say"],
        "Perspective" => &["persp_snap_toggle"],
        "Glass" => &["glass_mode", "glass_coloured"],
        _ => &[],
    }
}

/// The settings (besides its style) one HUD element's Style and Layout pages hold, for `Settings::reset_fields` (generated by `tools/gen_reset.py`: run it after changing a list).
pub fn element_keys(element: Element) -> &'static [&'static str] {
    match element {
        Element::Keystrokes => &["show_mouse", "show_space"],
        Element::Cps => &["cps_mode", "cps_avg", "cps_max", "cps_total"],
        Element::Coords => &[
            "co_x", "co_y", "co_z", "co_dir", "co_decimal", "co_horizontal", "co_x_label", "co_y_label", "co_z_label", "co_dir_cardinal", "co_dir_mod", "co_x_lc", "co_x_tc", "co_y_lc", "co_y_tc", "co_z_lc",
            "co_z_tc", "co_dir_tc", "co_dir_mx", "co_dir_mz", "hide_game_coords",
        ],
        Element::Facing => &["facing_text", "facing_label_chroma"],
        Element::Speed => &["speed_unit", "speed_decimals", "speed_vertical"],
        Element::Stopwatch => &["stopwatch_keep"],
        Element::Playtime => &["playtime_format", "playtime_world_only"],
        Element::Reach => &["reach_hide_zero", "reach_highlight", "reach_highlight_color"],
        Element::Food => &["sat_overlay", "sat_hunger", "sat_potential", "sat_inverse", "sat_color", "sat_layout"],
        Element::Potion => &[
            "potion_mode", "potion_horizontal", "potion_per_line", "potion_name", "potion_level", "potion_duration", "potion_formatted", "potion_upper", "potion_blink", "potion_color_name",
            "potion_color_info", "potion_icons", "potion_hide_vanilla", "potion_split", "potion_offsets", "potion_scales",
        ],
        Element::ArrayList => &["align"],
        Element::ItemCounter => &["counter_item"],
        Element::Inventory => &["inv_empty", "inv_show_bg", "inv_mc", "inv_grid", "inv_grid_color", "inv_grid_opacity"],
        Element::Clock => &["clock_12h", "clock_seconds"],
        Element::ViewAngles => &[
            "va_yaw", "va_pitch", "va_speed", "va_altitude", "va_first", "va_third", "va_elytra", "va_reverse", "va_target", "va_decimals", "va_align", "va_label_color", "va_value_color",
        ],
        Element::Scoreboard => &["sb_scores", "hide_scoreboard", "sb_title", "sb_horizontal"],
        Element::Waila => &[
            "waila_id", "waila_pos", "waila_mod", "waila_growth", "waila_tool", "waila_dist", "waila_state", "waila_entity", "waila_mod_color", "waila_info_color", "waila_good_color", "waila_bad_color",
            "waila_details",
        ],
        Element::PvpInfo => &["pvp_hits_lc", "pvp_acc_lc", "pvp_kills_lc", "pvp_deaths_lc", "pvp_num_tc", "pvp_pct_tc"],
        _ => &[],
    }
}

impl Settings {
    #[inline(never)]
    pub fn from_json(text: &str) -> Self {
        let mut settings = serde_json::from_str::<Settings>(text).unwrap_or_else(|_| {
            // A file the new version cannot read (a changed setting, a hand edit): start fresh, but keep the
            // player's waypoints, which they cannot get back by changing a switch.
            Settings { gameplay: crate::gameplay::GameplaySettings { waypoints: salvage_waypoints(text), ..Default::default() }, ..Default::default() }
        });
        settings.gameplay.waypoints.truncate(crate::waypoints::MAX_WAYPOINTS);
        settings.sanitize();
        settings
    }

    #[inline(never)]
    pub fn to_json(&self) -> String {
        // The same text as `serde_json::to_string`, for much less fuel (`json::to_json`).
        crate::json::to_json(self).unwrap_or_else(|_| "{}".into())
    }

    /// The file to write: the settings, without the saved setups if the host would refuse it for
    /// being too big (losing the slots is better than losing the session's changes).
    #[inline(never)]
    pub fn to_file(&self) -> String {
        let json = self.to_json();
        if json.len() <= crate::setups::MAX_FILE + 400 || self.profiles.is_empty() {
            return json;
        }
        let mut lean = self.clone();
        lean.profiles.clear();
        lean.to_json()
    }

    #[inline(never)]
    pub fn sanitize(&mut self) {
        let defaults = Settings::default();
        self.profiles.truncate(crate::setups::SLOTS);
        for profile in &mut self.profiles {
            if profile.chars().count() > crate::setups::MAX_PAYLOAD {
                profile.clear();
            }
        }
        self.names.truncate(crate::setups::SLOTS);
        for name in &mut self.names {
            *name = crate::setups::clean_name(name);
        }
        // Only ids of the grid's cards, each once, so a stale or edited file cannot grow the list.
        let mut seen: Vec<String> = Vec::new();
        for id in std::mem::take(&mut self.favorites) {
            if crate::menu_controls::MODS.iter().any(|m| m.0 == id) && !seen.contains(&id) {
                seen.push(id);
            }
        }
        self.favorites = seen;
        let mut hidden: Vec<String> = Vec::new();
        for id in std::mem::take(&mut self.hidden_mods) {
            if crate::menu_controls::MODS.iter().any(|m| m.0 == id) && !hidden.contains(&id) {
                hidden.push(id);
            }
        }
        self.hidden_mods = hidden;
        self.share_skip = crate::share::clean(&self.share_skip);
        self.hit_mode = self.hit_mode.min(HIT_MODES.len() - 1);
        self.speed_unit = self.speed_unit.min(SPEED_UNITS.len() - 1);
        self.speed_decimals = self.speed_decimals.min(MAX_SPEED_DECIMALS);
        self.playtime_format = self.playtime_format.min(TIME_FORMATS.len() - 1);
        self.counter_item = self.counter_item.min(COUNTER_ITEMS.len() - 1);
        if !valid_color(self.reach_highlight_color) {
            self.reach_highlight_color = GREEN;
        }
        self.sat_layout = self.sat_layout.min(crate::saturation::Layout::NAMES.len() - 1);
        self.potion_mode = self.potion_mode.min(crate::potion::MODES.len() - 1);
        self.potion_per_line = self.potion_per_line.clamp(1, crate::potion::MAX_PER_LINE);
        self.potion_offsets.resize(POTION_SLOTS, [0.0; 2]);
        self.potion_scales.resize(POTION_SLOTS, 1.0);
        for v in &mut self.potion_scales {
            *v = if v.is_finite() { v.clamp(POTION_MIN_SCALE, POTION_MAX_SCALE) } else { 1.0 };
        }
        for offset in &mut self.potion_offsets {
            for v in offset.iter_mut() {
                *v = if v.is_finite() { v.clamp(-MAX_POTION_OFFSET, MAX_POTION_OFFSET) } else { 0.0 };
            }
        }
        if !valid_color(self.sat_color) {
            self.sat_color = SAT_GOLD;
        }
        self.hit_opacity = if self.hit_opacity.is_finite() { self.hit_opacity.clamp(0.0, 100.0) } else { defaults.hit_opacity };
        for color in [&mut self.hitbox_color, &mut self.hitbox_player_color, &mut self.hit_color] {
            if !valid_color(*color) {
                *color = WHITE;
            }
        }
        self.waila_details = self.waila_details.min(WAILA_DETAILS.len() - 1);
        self.inv_grid_opacity = if self.inv_grid_opacity.is_finite() { self.inv_grid_opacity.clamp(0.0, 100.0) } else { defaults.inv_grid_opacity };
        if !valid_color(self.inv_grid_color) {
            self.inv_grid_color = BLACK;
        }
        for color in [&mut self.co_x_lc, &mut self.co_x_tc, &mut self.co_y_lc, &mut self.co_y_tc, &mut self.co_z_lc, &mut self.co_z_tc, &mut self.co_dir_tc, &mut self.co_dir_mx, &mut self.co_dir_mz, &mut self.pvp_hits_lc, &mut self.pvp_acc_lc, &mut self.pvp_kills_lc, &mut self.pvp_deaths_lc, &mut self.pvp_num_tc, &mut self.pvp_pct_tc] {
            if !valid_color(*color) {
                *color = custom_color(0xFFFFFF);
            }
        }
        for (color, fallback) in [(&mut self.va_label_color, 0xFFAA00), (&mut self.va_value_color, 0xAAAAAA)] {
            if !valid_color(*color) {
                *color = custom_color(fallback);
            }
        }
        self.va_decimals = self.va_decimals.min(VA_MAX_DECIMALS);
        self.va_align = self.va_align.min(VA_ALIGN.len() - 1);
        for (color, fallback) in [(&mut self.waila_mod_color, 0x5555FF), (&mut self.waila_info_color, 0xA0A0A0), (&mut self.waila_good_color, 0x55FF55), (&mut self.waila_bad_color, 0xFF5555)] {
            if !valid_color(*color) {
                *color = custom_color(fallback);
            }
        }
        self.line_width =
            if self.line_width.is_finite() { self.line_width.clamp(MIN_WIDTH, MAX_WIDTH) } else { defaults.line_width };
        // Exactly one style per element, whatever the file held.
        self.elements.truncate(Element::COUNT);
        while self.elements.len() < Element::COUNT {
            self.elements.push(Element::from_index(self.elements.len()).default_style());
        }
        for style in &mut self.elements {
            style.sanitize();
        }
        // The Arrow counter is gone (Item counter counts arrows by default): a setup that had it on gets
        // an Item counter in the same place, and its key.
        let (old, new) = (Element::Arrows.index(), Element::ItemCounter.index());
        if self.elements[old].enabled {
            if !self.elements[new].enabled && self.elements[new] == Element::ItemCounter.default_style() {
                let mut moved = self.elements[old].clone();
                moved.enabled = true;
                self.elements[new] = moved;
                self.counter_item = 0;
            }
            self.elements[old].enabled = false;
        }
        // The Direction compass that was never moved from its old place goes to the top centre.
        let facing = &mut self.elements[Element::Facing.index()];
        if facing.position == Some([1.0, 0.2]) && facing.corner == Corner::TopRight {
            facing.corner = Corner::TopLeft;
            facing.position = Some([0.5, 0.0]);
        }
        // Waypoints is not a HUD element (they show on the Direction compass and in the world): a slot kept.
        self.elements[Element::Waypoints.index()].enabled = false;
        if let Some(key) = self.keys.remove("hud_arrows") {
            self.keys.entry("hud_itemcounter".to_owned()).or_insert(key);
        }
        if !valid_key(&self.menu_key) {
            self.menu_key = defaults.menu_key;
        }
        let menu = self.menu_key.clone();
        self.keys.retain(|id, key| {
            key_slot_position(id.as_str()).is_some() && valid_key(key.as_str()) && key.as_str() != menu && key.as_str() != "F10"
        });
        // One key, one job: the zoom gives up any key the menu or an option already uses.
        self.crosshair.sanitize();
        self.zoom.sanitize();
        self.zoom.release(&menu);
        self.zoom.release("F10");
        for key in self.keys.values() {
            self.zoom.release(key);
        }
    }

    /// One element's style.
    pub fn style(&self, element: Element) -> &Style {
        &self.elements[element.index().min(self.elements.len() - 1)]
    }

    pub fn style_mut(&mut self, element: Element) -> &mut Style {
        let last = self.elements.len() - 1;
        &mut self.elements[element.index().min(last)]
    }

    /// How much an element's box is stretched sideways and up and down. Armor status has no box
    /// to stretch.
    pub fn stretch(&self, element: Element) -> [f32; 2] {
        if matches!(element, Element::Armor | Element::Potion) {
            return [1.0, 1.0];
        }
        let [x, y] = self.style(element).stretch;
        [x.clamp(MIN_STRETCH, MAX_STRETCH) as f32, y.clamp(MIN_STRETCH, MAX_STRETCH) as f32]
    }

    /// Whether an element's box follows its text (see `Style::auto_width`).
    pub fn auto_width(&self, element: Element) -> bool {
        self.style(element).auto_width.unwrap_or(element != Element::Cps)
    }

    /// The CPS bar's colour: the player's pick, else the text colour.
    pub fn bar_color(&self) -> usize {
        let style = self.style(Element::Cps);
        style.bar_color.unwrap_or(style.text_color)
    }

    /// Whether an element can have a border: every one with a box (each key of Keystrokes is one);
    /// Armor status has none.
    pub fn has_border(element: Element) -> bool {
        element != Element::Armor
    }

    /// The palette index of an element's "active" colour (Slime YES, Guardians found): the
    /// player's pick, else Green for slime chunks and Blue for guardians.
    pub fn active_color(&self, element: Element) -> usize {
        self.style(element).active_color.unwrap_or(if element == Element::Monument { 5 } else { 4 })
    }

    pub fn is_on(&self, element: Element) -> bool {
        self.style(element).enabled
    }

    /// An option of `TOGGLES` by id.
    pub fn toggle_mut(&mut self, id: &str) -> Option<&mut bool> {
        if let Some(element) = Element::from_toggle_id(id) {
            return Some(&mut self.style_mut(element).enabled);
        }
        Some(match id {
            "borders" => &mut self.borders,
            "subchunks" => &mut self.subchunks,
            "slime_chunks" => &mut self.slime_chunks,
            "guardians" => &mut self.guardians,
            "lights" => &mut self.lights,
            "blaze" => &mut self.blaze,
            "hitboxes" => &mut self.hitboxes,
            "fullbright" => &mut self.fullbright,
            "item_delay_fix" => &mut self.item_delay_fix,
            "hide_scoreboard" => &mut self.hide_scoreboard,
            "ch_on" => &mut self.crosshair.enabled,
            "zoom_on" => &mut self.zoom.enabled,
            _ if id.starts_with("gp_") => return self.gameplay.toggle_mut(id),
            _ => return None,
        })
    }

    pub fn toggle(&self, id: &str) -> bool {
        if let Some(element) = Element::from_toggle_id(id) {
            return self.is_on(element);
        }
        match id {
            "borders" => self.borders,
            "subchunks" => self.subchunks,
            "slime_chunks" => self.slime_chunks,
            "guardians" => self.guardians,
            "lights" => self.lights,
            "blaze" => self.blaze,
            "hitboxes" => self.hitboxes,
            "fullbright" => self.fullbright,
            "item_delay_fix" => self.item_delay_fix,
            "hide_scoreboard" => self.hide_scoreboard,
            "ch_on" => self.crosshair.enabled,
            "zoom_on" => self.zoom.enabled,
            _ if id.starts_with("gp_") => self.gameplay.toggle(id).unwrap_or(false),
            _ => false,
        }
    }

    /// The key of an option of `TOGGLES` or an action of `ACTION_KEYS` by id; empty when unbound.
    pub fn key(&self, id: &str) -> &str {
        self.keys.get(id).map_or("", String::as_str)
    }

    /// Binds (or, with an empty key, unbinds) an option's or action's key.
    pub fn set_key(&mut self, id: &str, key: &str) {
        if key.is_empty() {
            self.keys.remove(id);
        } else if key_slot_position(id).is_some() {
            self.keys.insert(id.to_owned(), key.to_owned());
        }
    }

    /// Every bound option key, for reserving and reading them.
    pub fn bound_keys(&self) -> Vec<String> {
        // The Freelook and Cycle perspective keys belong to the game's own actions (written to its Controls), so
        // the mod must not reserve them.
        let mut keys: Vec<String> = self.keys.iter().filter(|(id, k)| !k.is_empty() && !matches!(id.as_str(), "persp_free" | "persp_cycle")).map(|(_, k)| k.clone()).collect();
        keys.sort();
        keys.dedup();
        keys
    }

    /// Every key the game must not act on, and the mod reads: the option keys and the zoom's.
    pub fn reserved_keys(&self) -> Vec<String> {
        let mut keys = self.bound_keys();
        keys.extend(self.zoom.reserved_keys());
        keys.sort();
        keys.dedup();
        // The host reserves at most 64 keys; one is left for the menu key.
        keys.truncate(63);
        keys
    }

    /// The finders behind the HUD readouts run when the HUD element or the world tool is on.
    pub fn wants_monument(&self) -> bool {
        self.guardians || self.is_on(Element::Monument)
    }
}

pub fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 32 && key.bytes().all(|b| b.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_repairs() {
        let s = Settings::default();
        assert_eq!(s.menu_key, "ShiftRight");
        assert!(!s.borders && !s.slime_chunks && !s.guardians);
        assert_eq!(s.elements.len(), Element::COUNT);
        assert!(s.is_on(Element::Keystrokes) && s.is_on(Element::Cps) && !s.is_on(Element::Fps));
        let s = Settings::from_json(
            r#"{"line_width": 0, "menu_key": "F6", "grid": "five", "elements": [{"scale": 9, "opacity": -5, "bg_color": 99}]}"#,
        );
        assert_eq!(s.line_width, MIN_WIDTH);
        assert_eq!(s.grid, Grid::Five);
        assert_eq!(s.menu_key, "F6");
        assert_eq!(s.elements.len(), Element::COUNT, "missing elements are filled in");
        assert_eq!(s.style(Element::Keystrokes).scale, MAX_SCALE);
        assert_eq!(s.style(Element::Keystrokes).opacity, 0.0);
        assert_eq!(s.style(Element::Keystrokes).bg_color, WHITE);
        assert!(!s.is_on(Element::Fps), "the filled-in elements get their own defaults");
        assert_eq!(Settings::from_json(&s.to_json()), s);
        assert_eq!(Grid::from_index(Grid::Five.index()), Grid::Five);
        assert_eq!(CpsMode::from_index(CpsMode::Total.index()), CpsMode::Total);
    }

    #[test]
    fn every_element_has_a_toggle_and_a_style() {
        let mut s = Settings::default();
        for element in Element::ALL {
            assert!(TOGGLES.iter().chain(LATER_TOGGLES.iter()).chain(MORE_TOGGLES.iter()).any(|(id, _)| *id == element.toggle_id()));
            assert_eq!(Element::from_toggle_id(element.toggle_id()), Some(element));
            assert_eq!(Element::from_index(element.index()), element);
            *s.toggle_mut(element.toggle_id()).unwrap() = true;
            assert!(s.is_on(element) && s.toggle(element.toggle_id()));
        }
        assert_eq!(PALETTE_NAMES.len(), PALETTE.len());
        for (i, (name, _)) in PALETTE.iter().enumerate() {
            assert_eq!(PALETTE_NAMES[i], *name);
        }
    }

    #[test]
    fn the_zoom_gives_way_to_keys_in_use() {
        let s = Settings::from_json(r#"{"menu_key": "KeyU", "keys": {"borders": "KeyC"}, "zoom": {"keys": {"step_in": "KeyU"}}}"#);
        assert_eq!(s.key("borders"), "KeyC", "an existing option key is kept");
        assert_eq!((s.zoom.keys.zoom.as_str(), s.zoom.keys.step_in.as_str(), s.zoom.keys.step_out.as_str()), ("", "", "Minus"));
        let mut s = Settings::default();
        s.zoom.enabled = true;
        s.set_key("hud_fps", "KeyF");
        assert_eq!(s.reserved_keys(), ["Equal", "KeyC", "KeyF", "Minus"]);
        assert_eq!(s.bound_keys(), ["KeyF"]);
        assert!(!Settings::default().zoom.enabled, "the zoom starts off, like the other tools");
    }

    #[test]
    fn option_keys_are_checked() {
        let mut s = Settings::from_json(r#"{"keys": {"borders": "KeyB", "hitboxes": "F10", "lights": "Bad Key", "nope": "KeyZ"}}"#);
        assert_eq!(s.key("borders"), "KeyB");
        assert_eq!(s.key("hitboxes"), "", "F10 opens the menus, so it cannot also toggle an option");
        assert_eq!(s.key("lights"), "");
        assert_eq!(s.key("nope"), "");
        assert_eq!(s.bound_keys(), vec!["KeyB".to_string()]);
        s.set_key("hud_fps", "KeyF");
        assert_eq!(s.key("hud_fps"), "KeyF");
        s.set_key("hud_fps", "");
        assert_eq!(s.key("hud_fps"), "");
    }

    #[test]
    fn the_stopwatch_keys_are_bound_like_option_keys() {
        // The slots after the options, so a share code made before them still reads.
        assert_eq!(key_slot_id(TOGGLES.len() - 1), Some("hide_scoreboard"));
        assert_eq!(key_slot_id(TOGGLES.len()), Some("sw_pause"));
        assert_eq!(key_slot_id(TOGGLES.len() + 1), Some("sw_reset"));
        // The elements added later come after the actions, so older codes keep their slots.
        assert_eq!(key_slot_id(TOGGLES.len() + ACTION_KEYS.len()), Some("hud_reach"));
        assert_eq!(key_slot_id(KEY_SLOTS - 1), Some("gp_glass_on"));
        assert_eq!(key_slot_id(KEY_SLOTS), None);
        assert_eq!(key_slot_position("sw_reset"), Some(TOGGLES.len() + 1));
        assert_eq!(key_slot_position("nope"), None);
        let mut s = Settings::from_json(r#"{"keys": {"sw_pause": "KeyP", "sw_reset": "F10", "sw_other": "KeyQ"}}"#);
        assert_eq!(s.key("sw_pause"), "KeyP");
        assert_eq!(s.key("sw_reset"), "", "F10 opens the menus");
        assert_eq!(s.key("sw_other"), "");
        s.set_key("sw_reset", "KeyR");
        assert_eq!(s.bound_keys(), ["KeyP", "KeyR"]);
        assert_eq!(s.reserved_keys(), ["KeyP", "KeyR"], "the game leaves the stopwatch keys alone");
        s.set_key("sw_reset", "");
        assert_eq!(s.key("sw_reset"), "");
    }

    #[test]
    fn speed_and_playtime_options_are_repaired() {
        let d = Settings::default();
        assert_eq!((d.speed_unit, d.speed_decimals, d.speed_vertical), (0, 1, false), "what Speed showed before it had options");
        assert!(!d.stopwatch_keep && d.playtime_format == 0 && !d.playtime_world_only);
        let s = Settings::from_json(r#"{"speed_unit": 9, "speed_decimals": 40, "playtime_format": 7}"#);
        assert_eq!((s.speed_unit, s.speed_decimals, s.playtime_format), (SPEED_UNITS.len() - 1, MAX_SPEED_DECIMALS, TIME_FORMATS.len() - 1));
        // A settings file from before these options still loads, with their defaults.
        let old = Settings::from_json(r#"{"borders": true}"#);
        assert!(old.borders && old.speed_decimals == 1 && !old.speed_vertical);
    }

    #[test]
    fn reach_options_default_off_and_a_bad_colour_is_repaired() {
        let d = Settings::default();
        assert!(!d.reach_hide_zero && !d.reach_highlight);
        assert_eq!(PALETTE[d.reach_highlight_color].0, "Green");
        let s = Settings::from_json(r#"{"reach_highlight": true, "reach_highlight_color": 99999999999}"#);
        assert!(s.reach_highlight && s.reach_highlight_color == GREEN);
    }

    #[test]
    fn border_and_bar_colour_are_repaired_and_the_bar_follows_the_text_until_picked() {
        let d = Settings::default();
        let cps = d.style(Element::Cps);
        assert!(!cps.border && cps.border_width == 1.0 && cps.border_color == BLACK && cps.bar_color.is_none());
        assert_eq!(cps.border_opacity, 100.0);
        let s = Settings::from_json(r#"{"elements": [{}, {"border_opacity": 250}, {"border_opacity": -3}]}"#);
        assert_eq!((s.style(Element::Cps).border_opacity, s.style(Element::Fps).border_opacity), (100.0, 0.0));
        assert_eq!(d.bar_color(), cps.text_color);
        let s = Settings::from_json(r#"{"elements": [{}, {"text_color": 3, "border_width": 99, "border_color": 99999999, "bar_color": 99999999}]}"#);
        let cps = s.style(Element::Cps);
        assert_eq!((cps.border_width, cps.border_color, cps.bar_color), (MAX_BORDER, WHITE, None));
        assert_eq!(s.bar_color(), 3, "an unpicked bar takes the text colour");
        let s = Settings::from_json(r#"{"elements": [{}, {"text_color": 3, "bar_color": 5, "border_width": 0}]}"#);
        assert_eq!((s.bar_color(), s.style(Element::Cps).border_width), (5, MIN_BORDER));
        assert!(Settings::has_border(Element::Cps) && Settings::has_border(Element::Keystrokes) && !Settings::has_border(Element::Armor));
    }
}

#[cfg(test)]
mod compact_style_tests {
    use super::*;

    /// The settings written whole, every field of every struct, as the derived `Serialize` wrote them.
    fn whole(settings: &Settings) -> String {
        settings.whole_json().to_string()
    }

    /// The file now written reads back exactly as the whole form did (and as `settings`, for the
    /// values a slider can hold).
    fn round_trip(settings: &Settings) {
        let text = settings.to_json();
        let now: Settings = serde_json::from_str(&text).expect("the written settings read back");
        let before: Settings = serde_json::from_str(&whole(settings)).unwrap();
        for element in Element::ALL {
            assert_eq!(now.style(element), before.style(element), "{element:?}");
        }
        assert_eq!(now, before, "{text}");
        assert_eq!(Settings::from_json(&text), Settings::from_json(&whole(settings)));
    }

    /// Changes `style` in a way that depends on `seed`, touching every kind of field.
    fn vary(style: &mut Style, seed: u32) {
        let bit = |n: u32| (seed >> (n % 31)) & 1 == 1;
        style.enabled = bit(0);
        style.corner = [Corner::TopLeft, Corner::TopRight, Corner::BottomLeft, Corner::BottomRight][(seed % 4) as usize];
        style.position = bit(1).then(|| [(seed % 97) as f32 / 97.0, (seed % 89) as f32 / 89.0]);
        style.scale = f64::from(25 + seed % 400) / 100.0;
        style.stretch = [1.0 + f64::from(seed % 7) / 3.0, 1.0 + f64::from(seed % 5) / 9.0];
        style.bg_color = (seed % 9) as usize;
        style.text_color = (seed % 11) as usize;
        style.pressed_color = (seed % 13) as usize;
        style.pressed_text_color = (seed % 3) as usize;
        style.opacity = f64::from(seed % 101);
        style.labels = bit(2);
        style.smart = bit(3);
        style.rounded = bit(4);
        style.background = bit(5);
        style.shadow = bit(6);
        style.brackets = bit(7);
        style.reverse = bit(8);
        style.auto_width = if bit(9) { Some(bit(10)) } else { None };
        style.active_color = bit(11).then_some((seed % 8) as usize);
        style.show_chunk = bit(12);
        style.show_inchunk = bit(13);
        style.show_sub = bit(14);
        style.sub_range = bit(15);
        style.armor_horizontal = bit(16);
        style.armor_side = (seed % 4) as usize;
        style.armor_text = (seed % 4) as usize;
        style.armor_held = bit(17);
        style.armor_pieces = [bit(18), bit(19), bit(20), bit(21)];
        style.armor_split = bit(22);
        style.armor_offsets[(seed % 5) as usize] = [(seed % 50) as f32 - 25.5, (seed % 30) as f32 * 0.25];
        style.armor_sizes[(seed % 5) as usize] = [1.0 + (seed % 3) as f32 * 0.5, 0.4 + (seed % 9) as f32 * 0.1];
        style.armor_gap = f64::from(seed % 8);
        style.armor_good = (seed % 7) as usize;
        style.armor_mid = (seed % 6) as usize;
        style.armor_low = (seed % 5) as usize;
        style.border = bit(23);
        style.border_width = 0.25 + f64::from(seed % 12) / 4.0;
        style.border_color = (seed % 8) as usize;
        style.border_opacity = f64::from(seed % 100);
        style.bar_color = bit(24).then_some((seed % 8) as usize);
    }

    #[test]
    fn settings_read_back_exactly_as_written() {
        round_trip(&Settings::default());
        let mut seed = 0x2545_F491_u32;
        for _ in 0..400 {
            let mut settings = Settings::default();
            for element in Element::ALL {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                if seed % 3 != 0 {
                    vary(settings.style_mut(element), seed);
                }
            }
            round_trip(&settings);
        }
        // Every field away from its default at once.
        let mut settings = Settings::default();
        for element in Element::ALL {
            vary(settings.style_mut(element), 0x7FFF_FFFF);
        }
        round_trip(&settings);
    }

    #[test]
    fn a_style_at_its_defaults_is_written_empty_and_others_only_with_what_differs() {
        assert_eq!(crate::json::to_json(&Style::default()).unwrap(), "{}");
        let mut style = Style::default();
        style.scale = 2.5;
        style.armor_pieces[2] = false;
        assert_eq!(crate::json::to_json(&style).unwrap(), r#"{"scale":2.5,"armor_pieces":[true,true,false,true]}"#);
    }

    #[test]
    fn files_written_whole_still_read_the_same() {
        // The old form wrote every field of every struct; it must read back to the same settings.
        let mut settings = Settings::default();
        vary(settings.style_mut(Element::Armor), 12345);
        vary(settings.style_mut(Element::Fps), 777);
        settings.crosshair.enabled = true;
        settings.zoom.enabled = true;
        let whole_text = whole(&settings);
        assert!(whole_text.len() > 4 * settings.to_json().len());
        assert_eq!(Settings::from_json(&whole_text), Settings::from_json(&settings.to_json()));
    }

    #[test]
    fn the_settings_fit_the_hosts_file_limit() {
        let mut all_on = Settings::default();
        for element in Element::ALL {
            all_on.style_mut(element).enabled = true;
        }
        for settings in [Settings::default(), all_on] {
            let text = settings.to_file();
            assert!(text.len() < 16 * 1024 - 1024, "{} bytes", text.len());
        }
    }

    #[test]
    fn waypoints_survive_a_settings_file_that_no_longer_parses() {
        // `elements` is a list in the current version; an old or edited file has text there.
        let text = r#"{"elements":"oops","gameplay":{"waypoints":[{"name":"Home","world":"w","dimension":0,"pos":[1,64,2],"color":2},{"name":5}]}}"#;
        let settings = Settings::from_json(text);
        assert_eq!(settings.gameplay.waypoints.len(), 1);
        assert_eq!(settings.gameplay.waypoints[0].name, "Home");
    }
}
