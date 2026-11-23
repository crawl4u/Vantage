//! The HUD: every element is its own panel on the mod's one HUD layer, placed, sized and
//! coloured from its own settings, and only the elements that are switched on are in the
//! document. The document changes only with the settings; the values are bindings, and a frame
//! only rewrites the bindings of elements whose values changed.
//!
//! Documents are written already escaped to sit inside a JSON string (escaping them afterwards
//! would cost more wasm fuel than building them).

use crate::config::{ARMOR_MAX_SIZE, ARMOR_MIN_SIZE, Corner, Element, MAX_SCALE, MAX_SPEED_DECIMALS, MAX_STRETCH, MIN_SCALE, MIN_STRETCH, Settings, Style, rgba};
use crate::crosshair::{self, Bar, Look, Ring};
use crate::crosshair_config::{CrosshairSettings, Frame};
use crate::json::{num, push_i, push_u, q, rgba as write_rgba};
use crate::potion;
use crate::saturation::{self, Parts};
use std::cell::{Cell, RefCell};

/// What the guardian finder knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guardians {
    Searching,
    Found { active: u8, blocked: u8, checked: bool },
    /// Only the Overworld has monuments; also when flying too high to read the base.
    Unavailable,
}

/// The seven keys the Keystrokes element shows, named for the keys they start with: the four
/// movement keys, then the attack button ("MouseLeft"), the use button ("MouseRight") and jump
/// ("Space"). Which physical keys they follow is the player's own binding (`binds.rs`).
pub const KEY_NAMES: [&str; 7] = ["KeyW", "KeyA", "KeyS", "KeyD", "MouseLeft", "MouseRight", "Space"];

/// The actor the Target HUD shows: its name and health points (the maximum when the server gave one).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TargetView {
    pub name: String,
    pub health: Option<f32>,
    pub max_health: Option<f32>,
}

/// Everything the HUD shows in one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Views {
    /// Whether each of `KEY_NAMES` is held, in the same order.
    pub held: [bool; 7],
    /// Clicks per second: left, right.
    pub cps: [u32; 2],
    /// Frames per second, rounded; none when not measured yet.
    pub fps: Option<u32>,
    pub feet: [i32; 3],
    /// The position to a hundredth of a block, for Coordinates' decimals (zero while they are off).
    pub feet_exact: [f64; 3],
    /// Facing as 0 south, 1 west, 2 north, 3 east.
    pub facing: Option<u8>,
    /// The compass's heading in half degrees (0 to 719, Bedrock yaw: 0 south, 180 west, 360 north, 540 east);
    /// only read while the Direction element draws the compass.
    pub yaw: Option<u16>,
    pub overworld: bool,
    pub slime: Option<bool>,
    /// None while the finder is not running.
    pub guardians: Option<Guardians>,
    /// The worn armour: helmet, chestplate, leggings, boots.
    pub armor: [Worn; ARMOR_SLOTS],
    /// How many of the Item counter's item are carried.
    pub item_count: u32,
    /// How many healing potions and mushroom stews are carried (Potion counter).
    pub potion_count: u32,
    /// How many times your totem has popped this session (Totem counter).
    pub totems: u32,
    /// This session's fights (PvP Info): hits, accuracy in percent, kills, deaths.
    pub pvp: [u32; 4],
    /// The server's sidebar for the Scoreboard (cleaned); none when the server has none.
    pub sidebar: Option<Sidebar>,
    /// The Scoreboard has nothing to show and is left out of sight (not while the elements are moved).
    pub sidebar_hidden: bool,
    /// The actor you aim at or just hit, for the Target HUD; none when there is none.
    pub target: Option<TargetView>,
    /// The Target HUD has nobody to show and is left out of sight (not while the elements are moved).
    pub target_hidden: bool,
    /// The block you aim at, for WAILA; none when the crosshair is on nothing.
    pub waila: Option<crate::waila::WailaView>,
    /// WAILA has no block to show and is left out of sight (not while the elements are moved).
    pub waila_hidden: bool,
    /// The blocks left up to the world's height limit (Height limit); none before the position is known.
    pub height_limit: Option<i32>,
    /// The sounds just heard, newest first (Audio subtitles), and whether there are none to show (the element is left out of sight).
    pub subtitles: Vec<crate::subtitles::Row>,
    pub subtitles_hidden: bool,
    /// CPS statistics: the average in tenths, the highest CPS and the clicks made this session.
    pub cps_stats: [u32; 3],
    /// The nearest marked spots, nearest first (Waypoints).
    pub waypoints: Vec<crate::waypoints::Row>,
    /// The local time of day in seconds (whole minutes unless the Clock shows seconds); none when the host has no clock.
    pub clock: Option<u32>,
    /// The numbers View angles shows (yaw -180 to 180, 0 south; pitch -90 up to 90 down; speed; height), each times ten to the
    /// number of decimals; none until measured. Rounded that way before the view is compared, so only a visible change redraws.
    pub va: Option<VaView>,
    /// View angles is left out of sight (first or third person only, or while not flying an elytra; not while the elements are moved).
    pub va_hidden: bool,
    /// The main inventory for the Inventory HUD: 27 slots, top row first; empty while it is off.
    pub inventory: Vec<Worn>,
    /// Round-trip time in milliseconds; none when not measured (or in a world with no server).
    pub ping: Option<u32>,
    /// The speed in the unit the player picked (blocks per second or km/h), times ten to the
    /// number of decimals they show it with (56 with one decimal is 5.6); none until measured.
    /// The mod rounds it that way before the view is compared, so only a visible change redraws.
    pub speed: Option<u32>,
    /// Whole seconds on the stopwatch.
    pub stopwatch: u32,
    /// Whole seconds of playtime this session.
    pub playtime: u32,
    /// The distance of your last hit, in hundredths of a block; none before the first.
    pub reach: Option<u32>,
    /// Reach display is left out of sight: its Hide 0 Blocks is on and the reach reads 0 (and the
    /// elements are not being moved, which needs it in view).
    pub reach_hidden: bool,
    /// Hits in a row.
    pub combo: u32,
    /// Hunger points (rounded) and saturation in tenths; none until the server has said.
    pub food: Option<[u32; 2]>,
    /// What the food in your hand would give (hunger points, saturation in tenths), for the
    /// Saturation overlay's previews; none when you hold no food or the previews are off.
    pub eating: Option<(u32, u32)>,
    /// How bright the previews flash now (0 to `saturation::PULSE_MAX`); 0 while nothing flashes.
    pub pulse: u8,
    /// The world's day count (24000 ticks to a day, the first day is 0); none until the server sets a time.
    pub day: Option<u32>,
    /// The active potion effects for Potion status, in the game's order; empty while it is off.
    pub potions: Vec<potion::Row>,
    /// Which half of a blink the short effects are in (it flips a few times a second).
    pub blink: bool,
    /// What the Crosshair shows now; none while it is switched off.
    pub crosshair: Option<Frame>,
    /// The time in 30ths of a second while any element has chroma on (its colours are cut from it); 0 otherwise.
    pub chroma: u32,
}

impl Views {
    /// Plausible numbers for the Mods page's cards, which show what each element looks like.
    #[must_use]
    pub fn preview() -> Self {
        Self {
            cps: [12, 8],
            cps_stats: [87, 14, 320],
            fps: Some(144),
            feet: [38, -57, 4],
            feet_exact: [38.5, -56.5, 4.5],
            facing: Some(3),
            yaw: Some(360),
            overworld: true,
            slime: Some(true),
            item_count: 64,
            potion_count: 12,
            totems: 3,
            pvp: [4, 62, 2, 1],
            sidebar: Some(Sidebar { title: "Scoreboard".to_owned(), rows: vec![("You".to_owned(), 12), ("Them".to_owned(), 9)] }),
            height_limit: Some(256),
            clock: Some(12 * 3600 + 30 * 60),
            ping: Some(24),
            speed: Some(43),
            stopwatch: 12,
            playtime: 5040,
            reach: Some(320),
            combo: 5,
            food: Some([18, 50]),
            day: Some(12),
            ..Self::default()
        }
    }
}

/// An element as it looks at its default settings with example numbers: the box it fills, its background
/// and its lines of text.
#[derive(Clone, Debug)]
pub struct Sample {
    /// The box in unscaled GUI pixels.
    pub size: [f32; 2],
    /// The background colour with its opacity as alpha.
    pub background: [f32; 4],
    pub rounded: bool,
    pub shadow: bool,
    /// The element's size setting, which scales its text and its rows.
    pub scale: f32,
    pub lines: Vec<Line>,
    /// CPS's separator bar between the two counts as `[x, y, width, height]` in the box's pixels.
    pub bar: Option<[f32; 4]>,
    /// Direction's compass: each mark's text and how opaque it is, from the left of the strip to the right.
    pub compass: Option<Vec<(String, f32)>>,
    /// The box's vertical padding in GUI pixels at size 1.
    pub pad_y: f32,
}

/// `element` at its default settings, or none when it has no text to show (keys, items, effects and elements that
/// need live data such as a target).
#[must_use]
pub fn sample(element: Element) -> Option<Sample> {
    // Direction is drawn as the compass strip, the way the game shows it by default.
    let settings = Settings { facing_text: false, ..Settings::default() };
    let lines = lines(element, &Views::preview(), &settings);
    let compass = compass_on(&settings, element).then(|| {
        (-4..=4)
            .map(|n: i32| (compass_mark(240 + n * COMPASS_DEG as i32), 1.0 - 0.65 * (n.abs() as f32 / 4.0).powi(2)))
            .collect::<Vec<_>>()
    });
    if compass.is_none() && lines.iter().all(|l| l.text.is_empty() && l.head.is_none() && l.tail.is_none()) {
        return None;
    }
    let style = settings.style(element);
    // The element's own size is fitted to the lines it shows live; here they are the example ones, so the box is
    // fitted to them: as wide as the longest line and a row for each.
    let [base_w, base_h] = size(&settings, element);
    let k = settings.style(element).scale as f32;
    let widest = if compass.is_some() { 0.0 } else { lines.iter().map(|l| text_width(&format!("{} {} {}", l.head.as_ref().map_or("", |h| h.0.as_str()), l.text, l.tail.as_ref().map_or("", |t| t.0.as_str())).trim().to_owned())).fold(0.0_f32, f32::max) };
    let fitted = [(widest + 2.0 * PAD_X) * k, (pad_y(style) * 2.0 + ROW_H * lines.len() as f32) * k];
    Some(Sample {
        size: [base_w.max(fitted[0]), base_h.max(fitted[1])],
        background: rgba(style.bg_color, if style.background { style.opacity / 100.0 } else { 0.0 }),
        rounded: style.rounded,
        shadow: style.shadow,
        scale: k,
        compass,
        pad_y: pad_y(style),
        bar: cps_bar(&settings, element).then(|| {
            let at = cps_bar_offset(&lines[0].text, &settings);
            [at[0], at[1], k, CPS_BAR_HEIGHT * k]
        }),
        lines,
    })
}

/// The Keystrokes element at its default settings: its keys and their idle colour.
#[must_use]
pub fn sample_keys() -> (Vec<Key>, [f32; 4]) {
    // The space bar is shown too, so the card shows the whole element.
    let settings = Settings { show_space: true, show_mouse: true, ..Settings::default() };
    let style = settings.style(Element::Keystrokes);
    (keys(&settings), rgba(style.bg_color, if style.background { style.opacity / 100.0 } else { 0.0 }))
}

/// The server's sidebar as the Scoreboard shows it: the title and the rows (label, score), top to bottom.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sidebar {
    pub title: String,
    pub rows: Vec<(String, i32)>,
}

/// A row's score that means "no score to show" (the Horizontal option writes the scores into the text itself).
pub const NO_SCORE: i32 = i32::MIN;

/// How many lines the Scoreboard has for `board` (its title, if any, and a line per row; never fewer than one).
pub fn sidebar_line_count(board: Option<&Sidebar>) -> usize {
    board.map_or(1, |b| usize::from(!b.title.is_empty()) + b.rows.len()).max(1)
}

thread_local! {
    /// How many lines the Audio subtitles element has now (its box grows and shrinks with the list).
    static SUBTITLE_LINES: Cell<usize> = const { Cell::new(1) };
}

/// Tells the layout how many lines Audio subtitles has; true when that changed (the layer is written again).
pub fn set_subtitle_lines(lines: usize) -> bool {
    SUBTITLE_LINES.with(|cell| cell.replace(lines.max(1))) != lines.max(1)
}

thread_local! {
    /// How many lines the Waypoints element has now (its box grows and shrinks with the list).
    static WAYPOINT_LINES: Cell<usize> = const { Cell::new(1) };
}

/// Tells the layout how many lines the Waypoints element has; true when that changed (the layer is written again).
pub fn set_waypoint_lines(lines: usize) -> bool {
    WAYPOINT_LINES.with(|cell| cell.replace(lines.max(1))) != lines.max(1)
}

thread_local! {
    /// How many lines the Scoreboard has now (its box grows and shrinks with the server's sidebar).
    static SIDEBAR_LINES: Cell<usize> = const { Cell::new(1) };
}

/// Tells the layout how many lines the Scoreboard has; true when that changed (the layer is written again).
pub fn set_sidebar_lines(lines: usize) -> bool {
    SIDEBAR_LINES.with(|cell| cell.replace(lines.max(1))) != lines.max(1)
}

thread_local! {
    /// How many lines WAILA has now (its box grows with what the aimed block has to tell).
    static WAILA_LINES: Cell<usize> = const { Cell::new(1) };
    /// Whether the Show details key is held now (WAILA's extra lines in the Hold key mode).
    static WAILA_HELD: Cell<bool> = const { Cell::new(false) };
}

/// Tells WAILA whether its details key is held; true when that changed.
pub fn set_waila_held(held: bool) -> bool {
    WAILA_HELD.with(|cell| cell.replace(held)) != held
}

/// Whether WAILA's extra lines show now.
fn waila_details_shown(settings: &Settings) -> bool {
    match settings.waila_details {
        0 => true,
        1 => WAILA_HELD.with(Cell::get),
        _ => false,
    }
}

/// Tells the layout how many lines WAILA has; true when that changed (the layer is written again).
pub fn set_waila_lines(lines: usize) -> bool {
    WAILA_LINES.with(|cell| cell.replace(lines.max(1))) != lines.max(1)
}

/// What a WAILA line is, which decides its colour (Jade's palette: white title, blue mod, gray info, green and red).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WailaTone {
    Title,
    Mod,
    Info,
    Good,
}

/// The text of WAILA's lines for `block`: the name, then the mod, a crop's growth, a redstone signal, a mob's health,
/// the distance, the identifier and the position, each only while its option is on and the block has it.
pub fn waila_rows(block: &crate::waila::WailaView, settings: &Settings) -> Vec<(String, WailaTone)> {
    let mut rows = vec![(block.name.clone(), WailaTone::Title)];
    let details = waila_details_shown(settings);
    if settings.waila_mod && details {
        rows.push((block.mod_name.clone(), WailaTone::Mod));
    }
    if settings.waila_growth {
        if let Some(percent) = block.growth {
            rows.push(if percent >= 100 {
                ("Growth: Mature".to_owned(), WailaTone::Good)
            } else {
                (format!("Growth: {percent}%"), WailaTone::Info)
            });
        }
    }
    if settings.waila_state && details {
        if let Some(power) = block.power {
            rows.push((format!("Power: {power}"), WailaTone::Info));
        }
    }
    if let Some(health) = block.health {
        rows.push((crate::waila::health_text(health), WailaTone::Info));
    }
    if settings.waila_dist && details {
        if let Some(tenths) = block.distance {
            rows.push((crate::waila::distance_text(tenths), WailaTone::Info));
        }
    }
    if settings.waila_id && details {
        rows.push((block.id.clone(), WailaTone::Info));
    }
    if settings.waila_pos && details && block.health.is_none() {
        rows.push((crate::waila::position_text(block.pos), WailaTone::Info));
    }
    rows
}

/// How many lines WAILA has for the block aimed at (a sample block stands in while there is none).
pub fn waila_line_total(block: Option<&crate::waila::WailaView>, settings: &Settings) -> usize {
    match block {
        Some(block) => waila_rows(block, settings).len(),
        None => waila_rows(&crate::waila::sample(), settings).len(),
    }
}

/// The colour of a WAILA tone: the player's pick for each (the title is the element's text colour).
fn waila_color(tone: WailaTone, title: [f32; 4], settings: &Settings) -> [f32; 4] {
    match tone {
        WailaTone::Title => title,
        WailaTone::Mod => rgba(settings.waila_mod_color, 1.0),
        WailaTone::Info => rgba(settings.waila_info_color, 1.0),
        WailaTone::Good => rgba(settings.waila_good_color, 1.0),
    }
}

/// What WAILA's tool mark says: a check or a cross for a block that needs the right tool, none otherwise
/// (and none while the tool option is off).
fn waila_mark(block: &crate::waila::WailaView, settings: &Settings) -> Option<(&'static str, bool)> {
    let mining = block.mining.filter(|_| settings.waila_tool)?;
    mining.required.then(|| if block.harvest == Some(true) { ("\u{2713}", true) } else { ("\u{2717}", false) })
}

/// The width WAILA's first line needs beyond its text: the tool's icon and mark, at the right of the name.
fn waila_tool_width(block: &crate::waila::WailaView, settings: &Settings) -> f32 {
    if !settings.waila_tool || block.mining.is_none() {
        return 0.0;
    }
    // The mark's room is kept whether or not this block has one, so the icon stays where it is from block to block.
    WAILA_GAP + WAILA_TOOL + WAILA_GAP * 0.5 + WAILA_MARK_W
}

/// One worn armour piece, as the Armor status element shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Worn {
    /// The item's identifier ("minecraft:diamond_helmet"); empty when the slot is empty.
    pub item: String,
    /// The item's metadata value for its icon.
    pub metadata: u16,
    /// Durability left and the most it can have; none for an item that does not wear out.
    pub durability: Option<(u32, u32)>,
    /// How many are in the stack.
    pub count: u16,
}

impl Views {
    /// Whether `element` has something new to show compared with `other`.
    pub fn changed(&self, other: &Views, element: Element) -> bool {
        match element {
            // The mouse cells also show the click counts.
            Element::Keystrokes => self.held != other.held || self.cps != other.cps,
            Element::Cps => self.cps != other.cps || self.cps_stats != other.cps_stats,
            Element::Fps => self.fps != other.fps,
            Element::Coords => self.feet != other.feet || self.feet_exact != other.feet_exact || self.facing != other.facing,
            Element::Chunk => self.feet != other.feet,
            Element::Facing => self.facing != other.facing || self.yaw != other.yaw || self.waypoints != other.waypoints,
            Element::Slime => self.overworld != other.overworld || self.slime != other.slime,
            Element::Monument => self.guardians != other.guardians,
            Element::Armor => self.armor != other.armor,
            Element::Arrows => false,
            Element::Ping => self.ping != other.ping,
            Element::Speed => self.speed != other.speed,
            Element::Stopwatch => self.stopwatch != other.stopwatch,
            Element::Playtime => self.playtime != other.playtime,
            Element::Reach => self.reach != other.reach || self.reach_hidden != other.reach_hidden,
            Element::Combo => self.combo != other.combo,
            Element::Food => self.food != other.food,
            Element::Day => self.day != other.day,
            Element::Potion => self.potions != other.potions || self.blink != other.blink,
            // The Array list's lines follow the settings, which rewrite the whole layer when they change.
            Element::ArrayList => false,
            Element::ItemCounter => self.item_count != other.item_count,
            Element::Inventory => self.inventory != other.inventory,
            Element::Clock => self.clock != other.clock,
            Element::ViewAngles => self.va != other.va || self.va_hidden != other.va_hidden,
            Element::PotionCounter => self.potion_count != other.potion_count,
            Element::TotemCounter => self.totems != other.totems,
            Element::PvpInfo => self.pvp != other.pvp,
            Element::Scoreboard => self.sidebar != other.sidebar || self.sidebar_hidden != other.sidebar_hidden,
            Element::Waypoints => self.waypoints != other.waypoints,
            Element::TargetHud => self.target != other.target || self.target_hidden != other.target_hidden,
            Element::Waila => self.waila != other.waila || self.waila_hidden != other.waila_hidden,
            Element::HeightLimit => self.height_limit != other.height_limit,
            Element::AudioSubtitles => self.subtitles != other.subtitles || self.subtitles_hidden != other.subtitles_hidden,
        }
    }

    fn holds(&self, name: &str) -> bool {
        KEY_NAMES.iter().position(|known| *known == name).is_some_and(|index| self.held[index])
    }
}

pub const SCREEN: &str = "technic.hud";
/// Gap between an element and the screen edge when pinned to a corner, in GUI pixels.
const MARGIN: f32 = 6.0;
/// Vertical padding of a text box, and the row height of its lines.
const PAD: f32 = 4.0;
const ROW_H: f32 = 10.0;
/// Horizontal padding of a text box (between the text and the box edge, each side).
const PAD_X: f32 = 5.0;
/// Square boxes have no rounded corners to soften them, so they hug the text a little closer.
const SQUARE_PAD: f32 = 3.0;
const SQUARE_PAD_X: f32 = 4.0;

fn pad_y(style: &Style) -> f32 {
    if style.rounded { PAD } else { SQUARE_PAD }
}

fn pad_x(style: &Style) -> f32 {
    if style.rounded { PAD_X } else { SQUARE_PAD_X }
}
/// WAILA's block icon (a square, GUI pixels) and the gap between it and the text.
const WAILA_ICON: f32 = 16.0;
const WAILA_GAP: f32 = 4.0;
/// The binding that says which item WAILA's icon shows.
const WAILA_ICON_BINDING: &str = "#wl_item";
/// The tool icon beside the name (Jade draws it at three quarters of an item), and its mark's bindings.
const WAILA_TOOL: f32 = 12.0;
const WAILA_TOOL_BINDING: &str = "#wl_tool";
/// Width of the check / cross glyph (U+2713, U+2717, drawn in the mod font by tools/hud_glyphs.py).
const WAILA_MARK_W: f32 = 8.0;
const WAILA_MARK_BINDING: &str = "#wl_mark";
const WAILA_MARK_COLOR_BINDING: &str = "#wl_mark_c";
/// A label's text is drawn at the game's own font size times the element's size.
const FONT: f32 = 1.0;
/// The click counts inside the mouse keys are a little smaller than the key's name.
const CPS_FONT: f32 = 0.75;
/// Opaque enough to read a held key at any idle opacity.
const PRESSED_ALPHA: f64 = 0.9;

// The Armor status element: item icons with their durability, no box behind them.
const ARMOR_ICON: f32 = 16.0;
/// Space between an icon and its text.
const ARMOR_NEAR: f32 = 3.0;
/// Height of a line of text.
const ARMOR_LINE: f32 = 9.0;
/// How many pieces it can show: the four armour slots and the held item.
pub const ARMOR_SLOTS: usize = 5;

/// The Inventory HUD shows the main inventory: three rows of nine slots (the hotbar is the game's own).
pub const INV_SLOTS: usize = 27;
const INV_COLS: usize = 9;
/// One slot's cell: a 16 pixel icon with a pixel around it (GUI pixels, before scaling).
const INV_CELL: f32 = 18.0;
/// The space between the cells and the edge of the box.
const INV_PAD: f32 = 3.0;
const INV_WIDTH: f32 = INV_COLS as f32 * INV_CELL + INV_PAD * 2.0;
const INV_HEIGHT: f32 = (INV_SLOTS / INV_COLS) as f32 * INV_CELL + INV_PAD * 2.0;

/// One piece of the Armor status element: its slot (helmet 0 .. held item 4), where its icon and
/// text sit, and the corner and size of the cell they fill, in GUI pixels before scaling.
struct ArmorCell {
    slot: usize,
    icon: [f32; 2],
    text_at: [f32; 2],
    text_size: [f32; 2],
    align: &'static str,
    origin: [f32; 2],
    extent: [f32; 2],
    /// The icon's size, and the text's size relative to the element's (a piece moved apart can be
    /// resized on its own).
    icon_size: [f32; 2],
    font: f32,
    /// How far the cell's drawn box sits left of `origin`: a row fits each piece to its number,
    /// but the document's boxes are made for the longest one, so a shorter number has its
    /// right-aligned text box pulled back by the difference.
    shift: f32,
}

/// Where everything of the Armor status element sits, in GUI pixels before scaling.
struct ArmorGeometry {
    size: [f32; 2],
    cells: Vec<ArmorCell>,
}

/// What a piece is showing right now, for a layout that follows the numbers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArmorLive {
    /// Whether there is an item in the slot (an empty slot takes no room).
    pub present: bool,
    /// How many characters its number has (0 for none).
    pub chars: u8,
}

thread_local! {
    /// What the pieces last showed (None: unknown, every piece gets the room of its longest number).
    static ARMOR_LIVE: Cell<Option<[ArmorLive; ARMOR_SLOTS]>> = const { Cell::new(None) };
}

/// Tells the layout what the pieces show, so rows and columns fit their numbers and skip empty slots.
pub fn set_armor_live(live: Option<[ArmorLive; ARMOR_SLOTS]>) {
    ARMOR_LIVE.with(|cell| cell.set(live));
}

/// What each piece shows in `view`.
pub fn armor_live(view: &Views, settings: &Settings) -> [ArmorLive; ARMOR_SLOTS] {
    let style = settings.style(Element::Armor);
    let mut out = [ArmorLive::default(); ARMOR_SLOTS];
    for (slot, line) in armor_slots(style).into_iter().zip(lines(Element::Armor, view, settings)) {
        out[slot] = ArmorLive { present: !view.armor[slot].item.is_empty(), chars: line.text.len().min(255) as u8 };
    }
    out
}

/// Width of one character of a number, in GUI pixels.
const ARMOR_DIGIT: f32 = 6.0;

/// How wide the number beside a piece is, in GUI pixels (a digit is 6 wide). The boxes are only
/// as wide as the longest number the piece can show, so rows stay tight: armour never wears past
/// three digits, the held item may (a netherite tool has four).
fn armor_text_width(style: &Style, slot: usize) -> f32 {
    let held = slot >= 4;
    match style.armor_text {
        0 => if held { 24.0 } else { 18.0 },
        1 => 24.0,
        2 => if held { 54.0 } else { 42.0 },
        _ => 0.0,
    }
}

/// The slots that show, in order: the worn pieces that are switched on, then the held item.
pub fn armor_slots(style: &Style) -> Vec<usize> {
    (0..ARMOR_SLOTS).filter(|slot| if *slot < 4 { style.armor_pieces[*slot] } else { style.armor_held }).collect()
}

/// One piece laid out alone at the origin: its cell is `extent` big.
///
/// The numbers go on the side chosen (`armor_side`): Left (the default, like Lunar Client) and
/// Right share the icon's line, Above and Below stack on it and are centred over the icon. Left
/// numbers are right-aligned so they end at the icons. It works the same in a row or a column.
fn armor_cell(style: &Style, slot: usize, tw: f32) -> ArmorCell {
    let side = style.armor_side;
    if tw > 0.0 && side >= 2 {
        let cw = ARMOR_ICON.max(tw);
        let text_h = ARMOR_LINE + 1.0;
        let (icon_y, text_y) = if side == 2 { (text_h, 0.0) } else { (0.0, ARMOR_ICON + 1.0) };
        return ArmorCell {
            slot,
            icon: [(cw - ARMOR_ICON) * 0.5, icon_y],
            text_at: [(cw - tw) * 0.5, text_y],
            text_size: [tw, ARMOR_LINE],
            align: "center",
            origin: [0.0, 0.0],
            extent: [cw, ARMOR_ICON + text_h],
            icon_size: [ARMOR_ICON; 2],
            font: 1.0,
            shift: 0.0,
        };
    }
    let near = if tw > 0.0 { ARMOR_NEAR } else { 0.0 };
    let text_y = (ARMOR_ICON - 8.0) * 0.5;
    let extent = [ARMOR_ICON + near + tw, ARMOR_ICON];
    if side == 0 {
        ArmorCell { slot, icon: [tw + near, 0.0], text_at: [0.0, text_y], text_size: [tw, ARMOR_LINE], align: "right", origin: [0.0, 0.0], extent, icon_size: [ARMOR_ICON; 2], font: 1.0, shift: 0.0 }
    } else {
        ArmorCell { slot, icon: [0.0, 0.0], text_at: [ARMOR_ICON + near, text_y], text_size: [tw, ARMOR_LINE], align: "left", origin: [0.0, 0.0], extent, icon_size: [ARMOR_ICON; 2], font: 1.0, shift: 0.0 }
    }
}

/// Stretches a piece laid out alone at the origin by `by` on each axis (its own size, when the
/// pieces are moved apart). The text follows the height.
fn armor_size(cell: &mut ArmorCell, by: [f32; 2]) {
    for point in [&mut cell.icon, &mut cell.text_at, &mut cell.text_size, &mut cell.extent, &mut cell.icon_size] {
        point[0] *= by[0];
        point[1] *= by[1];
    }
    cell.font = by[1];
}

fn armor_shift(cell: &mut ArmorCell, by: [f32; 2]) {
    for point in [&mut cell.icon, &mut cell.text_at, &mut cell.origin] {
        point[0] += by[0];
        point[1] += by[1];
    }
}

/// The layout the pieces have now: a row fits each piece to its number and a row or column skips
/// empty slots, like Lunar Client.
fn armor_geometry(style: &Style) -> ArmorGeometry {
    armor_geometry_in(style, ARMOR_LIVE.with(Cell::get))
}

/// The layout the document is written for: every piece has room for its longest number, so the
/// document stays the same while the numbers change (only the places of the pieces move).
fn armor_geometry_static(style: &Style) -> ArmorGeometry {
    armor_geometry_in(style, None)
}

fn armor_geometry_in(style: &Style, live: Option<[ArmorLive; ARMOR_SLOTS]>) -> ArmorGeometry {
    let gap = style.armor_gap as f32;
    // Pieces moved apart sit where they were put, whatever they show.
    let live = live.filter(|_| !style.armor_split);
    // Only pieces with their number on their own line can sit closer for a shorter number; in a
    // column the icons must stay in line.
    let tight = style.armor_horizontal && style.armor_side < 2;
    // In a column every piece gets the room of the longest number of any, so the icons line up.
    let column = if style.armor_horizontal { 0.0 } else { armor_slots(style).into_iter().map(|slot| armor_text_width(style, slot)).fold(0.0, f32::max) };
    let mut cells: Vec<ArmorCell> = Vec::new();
    for slot in armor_slots(style) {
        let longest = if style.armor_horizontal { armor_text_width(style, slot) } else { column };
        let mut tw = longest;
        if let Some(live) = &live {
            if !live[slot].present {
                continue;
            }
            if tight {
                tw = (f32::from(live[slot].chars) * ARMOR_DIGIT).min(longest);
            }
        }
        let mut cell = armor_cell(style, slot, tw);
        if style.armor_split {
            armor_size(&mut cell, style.armor_sizes[slot]);
        }
        if tight && live.is_some() && style.armor_side == 0 {
            cell.shift = longest - tw;
        }
        cells.push(cell);
    }
    if cells.is_empty() {
        return ArmorGeometry { size: [ARMOR_ICON, ARMOR_ICON], cells };
    }
    if style.armor_split {
        // Every piece sits where it was put; the element is as big as they need together.
        let mut size = [0.0f32, 0.0];
        for cell in &mut cells {
            armor_shift(cell, style.armor_offsets[cell.slot]);
            size[0] = size[0].max(cell.origin[0] + cell.extent[0]);
            size[1] = size[1].max(cell.origin[1] + cell.extent[1]);
        }
        return ArmorGeometry { size, cells };
    }
    // A row (or a column) of the pieces that show.
    let mut at = 0.0f32;
    for cell in &mut cells {
        let by = if style.armor_horizontal { [at, 0.0] } else { [0.0, at] };
        at += if style.armor_horizontal { cell.extent[0] } else { cell.extent[1] } + gap;
        armor_shift(cell, by);
    }
    let last = &cells[cells.len() - 1];
    let size = [last.origin[0] + last.extent[0], last.origin[1] + last.extent[1]];
    ArmorGeometry { size, cells }
}

/// Where each piece that shows sits on screen: its slot and rectangle (x, y, width, height).
pub fn armor_pieces(settings: &Settings, viewport: [f32; 2]) -> Vec<(usize, [f32; 4])> {
    let style = settings.style(Element::Armor);
    let k = style.scale as f32;
    let [x, y] = top_left(settings, Element::Armor, viewport);
    armor_geometry(style)
        .cells
        .iter()
        .map(|c| (c.slot, [x + c.origin[0] * k, y + c.origin[1] * k, c.extent[0] * k, c.extent[1] * k]))
        .collect()
}

/// The slot of the piece under a point, when pieces are placed one by one (the last drawn wins).
pub fn armor_piece_at(settings: &Settings, viewport: [f32; 2], point: [f32; 2]) -> Option<usize> {
    armor_pieces(settings, viewport)
        .into_iter()
        .rev()
        .find(|(_, [x, y, w, h])| point[0] >= *x && point[0] <= x + w && point[1] >= *y && point[1] <= y + h)
        .map(|(slot, _)| slot)
}

/// The guides of the screen itself: the common gap from each edge and the middle lines, while Snap to guides
/// is on; only the middle lines when just the centre lines are on.
fn screen_guides(settings: &Settings, viewport: [f32; 2]) -> (Vec<f32>, Vec<f32>) {
    if settings.align {
        (vec![MARGIN, viewport[0] * 0.5, viewport[0] - MARGIN], vec![MARGIN, viewport[1] * 0.5, viewport[1] - MARGIN])
    } else {
        (vec![viewport[0] * 0.5], vec![viewport[1] * 0.5])
    }
}

/// Where a dragged piece's top-left corner may go: kept on screen, and with `snap` also moved
/// onto nearby guides (the screen's edges and centre lines, the other pieces and the other
/// elements that are on), each axis on its own.
#[allow(dead_code)] // the drag uses `ArmorDrag`; kept for the tests
pub fn armor_target(settings: &Settings, viewport: [f32; 2], slot: usize, target: [f32; 2], snap: bool) -> [f32; 2] {
    let pieces = armor_pieces(settings, viewport);
    let Some((_, [_, _, w, h])) = pieces.iter().find(|(s, _)| *s == slot).copied() else { return target };
    let mut at = target;
    if snap {
        let mut guides_x = vec![MARGIN, viewport[0] * 0.5, viewport[0] - MARGIN];
        let mut guides_y = vec![MARGIN, viewport[1] * 0.5, viewport[1] - MARGIN];
        for (s, [x, y, pw, ph]) in &pieces {
            if *s != slot {
                guides_x.extend([*x, x + pw * 0.5, x + pw]);
                guides_y.extend([*y, y + ph * 0.5, y + ph]);
            }
        }
        for other in Element::ALL {
            if other != Element::Armor && settings.is_on(other) {
                let [x, y, ow, oh] = visible_rect(settings, other, viewport);
                guides_x.extend([x, x + ow * 0.5, x + ow]);
                guides_y.extend([y, y + oh * 0.5, y + oh]);
            }
        }
        at = [snap_axis(at[0], w, &guides_x, viewport[0] * 0.5), snap_axis(at[1], h, &guides_y, viewport[1] * 0.5)];
    }
    // Never off screen: the element's corner would otherwise run away from the other pieces.
    [at[0].clamp(0.0, (viewport[0] - w).max(0.0)).round(), at[1].clamp(0.0, (viewport[1] - h).max(0.0)).round()]
}

/// `start` moved towards the closest guide that one of its edges or its centre has within
/// `SNAP`. The pull is smooth, not a jump: far from the guide it lets go of the element, closer
/// it eases the element in, and within `SNAP_LOCK` it sits exactly on the guide, so the element
/// follows the pointer without ever leaping.
fn snap_axis(start: f32, length: f32, guides: &[f32], centre: f32) -> f32 {
    // (distance to the guide, how far it pulls, how close it locks): the centre line of the screen
    // pulls from further and holds the element over a wider stretch than the other guides.
    let mut best: Option<(f32, f32, f32)> = None;
    for edge in [start, start + length * 0.5, start + length] {
        for guide in guides {
            let delta = guide - edge;
            let (reach, lock) = if (guide - centre).abs() < 0.01 { (CENTER_SNAP, CENTER_LOCK) } else { (SNAP, SNAP_LOCK) };
            if delta.abs() <= reach && best.is_none_or(|b| delta.abs() < b.0.abs()) {
                best = Some((delta, reach, lock));
            }
        }
    }
    let Some((delta, reach, lock)) = best else { return start };
    let distance = delta.abs();
    // How far from the guide the element is left: nothing inside the lock, the whole reach at the
    // edge of the pull, and a curve between them that meets both without a step.
    let left = if distance <= lock { 0.0 } else { reach * ((distance - lock) / (reach - lock)).powi(2) };
    start + delta - delta.signum() * left
}

/// Puts `slot`'s top-left corner at `target` on screen (GUI pixels) while the other pieces stay
/// where they are: the offsets are stored from the new top-left of all of them.
#[allow(dead_code)] // the drag uses `ArmorDrag`; kept for the tests
pub fn armor_move_piece(settings: &mut Settings, viewport: [f32; 2], slot: usize, target: [f32; 2]) {
    let target = armor_target(settings, viewport, slot, target, false);
    let mut now = armor_pieces(settings, viewport);
    let k = settings.style(Element::Armor).scale as f32;
    for (s, rect) in &mut now {
        if *s == slot {
            rect[0] = target[0];
            rect[1] = target[1];
        }
    }
    let min = [
        now.iter().map(|(_, r)| r[0]).fold(f32::MAX, f32::min),
        now.iter().map(|(_, r)| r[1]).fold(f32::MAX, f32::min),
    ];
    let style = settings.style_mut(Element::Armor);
    for (s, rect) in &now {
        style.armor_offsets[*s] = [((rect[0] - min[0]) / k).round(), ((rect[1] - min[1]) / k).round()];
    }
    let at = place_at(settings, Element::Armor, viewport, min);
    settings.style_mut(Element::Armor).position = Some(at);
}

/// What a drag of Armor status keeps from the moment it was picked up: where the pieces were,
/// the guides it can snap to and the last place asked for. The other elements do not move while
/// one is dragged, so none of this is worked out again on each step: a step is a few sums and
/// a small patch.
pub struct ArmorDrag {
    /// The piece being dragged, or none when the whole element is.
    slot: Option<usize>,
    rects: Vec<(usize, [f32; 4])>,
    guides: [Vec<f32>; 2],
    start: [f32; 2],
    size: [f32; 2],
    last: Option<[f32; 2]>,
}

impl ArmorDrag {
    pub fn new(settings: &Settings, viewport: [f32; 2], slot: Option<usize>) -> Self {
        let rects = armor_pieces(settings, viewport);
        let (mut gx, mut gy) = screen_guides(settings, viewport);
        if let (Some(dragged), true) = (slot, settings.align) {
            for (s, [x, y, w, h]) in &rects {
                if *s != dragged {
                    gx.extend([*x, x + w * 0.5, x + w]);
                    gy.extend([*y, y + h * 0.5, y + h]);
                }
            }
        }
        for other in Element::ALL {
            if other != Element::Armor && settings.is_on(other) && settings.align {
                let [x, y, w, h] = visible_rect(settings, other, viewport);
                gx.extend([x, x + w * 0.5, x + w]);
                gy.extend([y, y + h * 0.5, y + h]);
            }
        }
        Self {
            slot,
            rects,
            guides: [gx, gy],
            start: top_left(settings, Element::Armor, viewport),
            size: size(settings, Element::Armor),
            last: None,
        }
    }

    /// Moves the piece (or the element) so its top-left corner follows `wanted`, and returns the
    /// patch that places the pieces; none when nothing moved since the last step.
    pub fn step(&mut self, settings: &mut Settings, viewport: [f32; 2], wanted: [f32; 2], snap: bool) -> Option<String> {
        if self.last == Some(wanted) {
            return None;
        }
        self.last = Some(wanted);
        let k = settings.style(Element::Armor).scale as f32;
        let mut now: Vec<(usize, [f32; 2])> = Vec::with_capacity(self.rects.len());
        let at;
        if let Some(dragged) = self.slot {
            let [_, _, w, h] = self.rects.iter().find(|(s, _)| *s == dragged)?.1;
            let mut to = wanted;
            if snap {
                to = [snap_axis(to[0], w, &self.guides[0], viewport[0] * 0.5), snap_axis(to[1], h, &self.guides[1], viewport[1] * 0.5)];
            }
            // Never off screen: the other pieces would otherwise be pushed away from it.
            to = [to[0].clamp(0.0, (viewport[0] - w).max(0.0)).round(), to[1].clamp(0.0, (viewport[1] - h).max(0.0)).round()];
            let mut min = [f32::MAX, f32::MAX];
            let (mut right, mut bottom) = (0.0f32, 0.0f32);
            for (s, r) in &self.rects {
                let p = if *s == dragged { to } else { [r[0], r[1]] };
                min = [min[0].min(p[0]), min[1].min(p[1])];
                right = right.max(p[0] + r[2]);
                bottom = bottom.max(p[1] + r[3]);
                now.push((*s, p));
            }
            let style = settings.style_mut(Element::Armor);
            for (s, p) in &now {
                style.armor_offsets[*s] = [((p[0] - min[0]) / k).round(), ((p[1] - min[1]) / k).round()];
            }
            at = min;
            self.size = [right - min[0], bottom - min[1]];
        } else {
            let [w, h] = self.size;
            let mut to = wanted;
            if snap {
                to = [snap_axis(to[0], w, &self.guides[0], viewport[0] * 0.5), snap_axis(to[1], h, &self.guides[1], viewport[1] * 0.5)];
            }
            // Never off screen: the pieces would otherwise follow the pointer past the edge while the
            // saved place (always kept on screen) stayed behind, and jump back when the element was let go.
            to = [to[0].clamp(0.0, (viewport[0] - w).max(0.0)).round(), to[1].clamp(0.0, (viewport[1] - h).max(0.0)).round()];
            at = to;
            for (s, r) in &self.rects {
                now.push((*s, [r[0] - self.start[0] + to[0], r[1] - self.start[1] + to[1]]));
            }
        }
        let free = [(viewport[0] - self.size[0]).max(1.0), (viewport[1] - self.size[1]).max(1.0)];
        settings.style_mut(Element::Armor).position = Some([(at[0] / free[0]).clamp(0.0, 1.0), (at[1] / free[1]).clamp(0.0, 1.0)]);
        let mut o = String::with_capacity(160);
        o.push_str("{\"binding_changes\":{");
        armor_position_members(&mut o, &now, &mut true);
        o.push_str("}}");
        Some(o)
    }
}

/// Where a piece (moved apart) is on screen: its rectangle, the cell its icon and number fill.
pub fn armor_piece_rect(settings: &Settings, viewport: [f32; 2], slot: usize) -> Option<[f32; 4]> {
    armor_pieces(settings, viewport).into_iter().find(|(s, _)| *s == slot).map(|(_, r)| r)
}

/// Saves where the pieces are on screen (`rects`, in GUI pixels): each piece's offset from the
/// top left of them all, and where that corner is as a share of the free room, like a drag does.
fn armor_place(settings: &mut Settings, viewport: [f32; 2], rects: &[(usize, [f32; 4])]) {
    let k = settings.style(Element::Armor).scale as f32;
    let mut min = [f32::MAX, f32::MAX];
    let (mut right, mut bottom) = (0.0f32, 0.0f32);
    for (_, r) in rects {
        min = [min[0].min(r[0]), min[1].min(r[1])];
        right = right.max(r[0] + r[2]);
        bottom = bottom.max(r[1] + r[3]);
    }
    let style = settings.style_mut(Element::Armor);
    for (s, r) in rects {
        style.armor_offsets[*s] = [(((r[0] - min[0]) / k) * 10.0).round() / 10.0, (((r[1] - min[1]) / k) * 10.0).round() / 10.0];
    }
    let free = [(viewport[0] - (right - min[0])).max(1.0), (viewport[1] - (bottom - min[1])).max(1.0)];
    style.position = Some([(min[0] / free[0]).clamp(0.0, 1.0), (min[1] / free[1]).clamp(0.0, 1.0)]);
}

/// A drag of one of the handles of a piece that is moved apart. A corner resizes the piece, the
/// middle of a side stretches it on that axis; the opposite corner or side stays put.
pub struct PieceResize {
    slot: usize,
    handle: usize,
    anchor: [f32; 2],
    grab: [f32; 2],
    /// The piece's rectangle and own size when the drag began, and where every piece was.
    rect: [f32; 4],
    size: [f32; 2],
    rects: Vec<(usize, [f32; 4])>,
}

impl PieceResize {
    pub fn new(settings: &Settings, viewport: [f32; 2], slot: usize, handle: usize, pointer: [f32; 2]) -> Option<Self> {
        let rect = armor_piece_rect(settings, viewport, slot)?;
        let (held, anchor) = if handle < 4 {
            (corners(rect)[handle], corners(rect)[3 - handle])
        } else {
            (sides(rect)[handle - 4], sides(rect)[(handle - 4) ^ 1])
        };
        Some(Self {
            slot,
            handle,
            anchor,
            grab: [pointer[0] - held[0], pointer[1] - held[1]],
            rect,
            size: settings.style(Element::Armor).armor_sizes[slot],
            rects: armor_pieces(settings, viewport),
        })
    }

    /// Follows the pointer: sets the piece's size (to a hundredth) and puts it where the opposite
    /// corner or side stays. Returns whether anything changed.
    pub fn step(&self, settings: &mut Settings, viewport: [f32; 2], pointer: [f32; 2]) -> bool {
        let [x0, y0, w0, h0] = self.rect;
        let (w0, h0) = (w0.max(1.0), h0.max(1.0));
        let mut ratio = [1.0f32, 1.0];
        if self.handle < 4 {
            let sx = if self.handle % 2 == 0 { -1.0 } else { 1.0 };
            let sy = if self.handle < 2 { -1.0 } else { 1.0 };
            let dx = (pointer[0] - self.grab[0] - self.anchor[0]) * sx;
            let dy = (pointer[1] - self.grab[1] - self.anchor[1]) * sy;
            let r = (dx * w0 + dy * h0) / (w0 * w0 + h0 * h0);
            let low = (ARMOR_MIN_SIZE / self.size[0]).max(ARMOR_MIN_SIZE / self.size[1]);
            let high = (ARMOR_MAX_SIZE / self.size[0]).min(ARMOR_MAX_SIZE / self.size[1]).min((viewport[0] / w0).min(viewport[1] / h0)).max(low);
            ratio = [r.clamp(low, high); 2];
        } else {
            let side = self.handle - 4;
            let axis = usize::from(side >= 2);
            let sign = if side % 2 == 0 { -1.0 } else { 1.0 };
            let length = (pointer[axis] - self.grab[axis] - self.anchor[axis]) * sign;
            let along = if axis == 0 { w0 } else { h0 };
            let low = ARMOR_MIN_SIZE / self.size[axis];
            let high = (ARMOR_MAX_SIZE / self.size[axis]).min(viewport[axis] / along).max(low);
            ratio[axis] = (length / along).clamp(low, high);
        }
        let wanted = [0, 1].map(|a| ((self.size[a] * ratio[a]).clamp(ARMOR_MIN_SIZE, ARMOR_MAX_SIZE) * 100.0).round() / 100.0);
        if settings.style(Element::Armor).armor_sizes[self.slot] == wanted {
            return false;
        }
        settings.style_mut(Element::Armor).armor_sizes[self.slot] = wanted;
        let w = w0 * wanted[0] / self.size[0];
        let h = h0 * wanted[1] / self.size[1];
        // The opposite corner (or side) stays; the piece grows away from it.
        let (mut x, mut y) = (x0, y0);
        if self.handle < 4 {
            x = if self.handle % 2 == 0 { self.anchor[0] - w } else { self.anchor[0] };
            y = if self.handle < 2 { self.anchor[1] - h } else { self.anchor[1] };
        } else {
            let side = self.handle - 4;
            if side < 2 {
                x = if side == 0 { self.anchor[0] - w } else { self.anchor[0] };
            } else {
                y = if side == 2 { self.anchor[1] - h } else { self.anchor[1] };
            }
        }
        let x = x.clamp(0.0, (viewport[0] - w).max(0.0));
        let y = y.clamp(0.0, (viewport[1] - h).max(0.0));
        let rects: Vec<(usize, [f32; 4])> = self.rects.iter().map(|(s, r)| if *s == self.slot { (*s, [x, y, w, h]) } else { (*s, *r) }).collect();
        armor_place(settings, viewport, &rects);
        true
    }
}

/// Starts pieces off where the row (or column) has them, so switching to separate pieces moves
/// nothing.
pub fn armor_split_from_layout(style: &mut Style) {
    let was = style.armor_split;
    style.armor_split = false;
    style.armor_sizes = [[1.0; 2]; 5];
    // Pieces that are switched off keep the spot they would have in a full row.
    let mut full = style.clone();
    full.armor_pieces = [true; 4];
    full.armor_held = true;
    for cell in armor_geometry_static(&full).cells {
        style.armor_offsets[cell.slot] = cell.origin;
    }
    for cell in armor_geometry_static(style).cells {
        style.armor_offsets[cell.slot] = cell.origin;
    }
    style.armor_split = was;
}

/// Lays the pieces out again after a setting that changes how big a piece is or which way they
/// run (direction, text side, what the text says, spacing). While the pieces are moved apart
/// their saved spots belong to the layout they were put in, so without this a piece that grows
/// or turns lands on top of its neighbours. Does nothing while the pieces are in a row.
pub fn armor_reflow(style: &mut Style) {
    if style.armor_split {
        armor_split_from_layout(style);
    }
}

const GRAY: [f32; 4] = [0.7, 0.7, 0.7, 1.0];
const GREEN: [f32; 4] = [0.35, 1.0, 0.35, 1.0];
const RED: [f32; 4] = [1.0, 0.35, 0.35, 1.0];
const YELLOW: [f32; 4] = [1.0, 0.85, 0.25, 1.0];
const FACING: [&str; 4] = ["South (+Z)", "West (-X)", "North (-Z)", "East (+X)"];

// ---- Keystrokes layout -----------------------------------------------------------------

/// One key on screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    /// The key's slot name, one of `KEY_NAMES`.
    pub name: &'static str,
    /// What the cell says: the short name of the player's key for it.
    pub label: String,
    /// Cell rectangle in unscaled GUI pixels: x, y, width, height.
    pub rect: [u32; 4],
}

const SIDE: u32 = 22;
const GAP: u32 = 2;
const ROW: u32 = SIDE + GAP;
/// The keys card's width: three keys and their gaps.
pub const CARD_WIDTH: u32 = SIDE * 3 + GAP * 2;
const WIDE: u32 = (CARD_WIDTH - GAP) / 2;
const SPACE_HEIGHT: u32 = 10;

thread_local! {
    /// Where the game's own hunger row is (its right-most icon's corner), when the host said.
    static HUNGER_AT: Cell<Option<[f32; 2]>> = const { Cell::new(None) };
}

/// Tells the Saturation overlay where the game drew its hunger row (None: use the layout setting).
/// Returns whether it changed, so the caller can redraw.
pub fn set_hunger_at(at: Option<[f32; 2]>) -> bool {
    HUNGER_AT.with(|cell| {
        let changed = cell.get() != at;
        cell.set(at);
        changed
    })
}

thread_local! {
    /// The names on the key cells (forward, left, back, right, attack, use), from the player's bindings.
    static KEY_LABELS: RefCell<[String; 6]> = RefCell::new(default_key_labels());
}

fn default_key_labels() -> [String; 6] {
    ["W", "A", "S", "D", "LMB", "RMB"].map(String::from)
}

/// Sets the names on the key cells. They are part of the layer's document, so a change (true) needs it written again.
pub fn set_key_labels(labels: [String; 6]) -> bool {
    KEY_LABELS.with(|cell| {
        let changed = *cell.borrow() != labels;
        if changed {
            *cell.borrow_mut() = labels;
        }
        changed
    })
}

fn key_labels() -> [String; 6] {
    KEY_LABELS.with(|cell| cell.borrow().clone())
}

/// The keys the settings ask for, top to bottom, with the rows below a hidden one moved up.
pub fn keys(settings: &Settings) -> Vec<Key> {
    let [w, a, s, d, attack, use_] = key_labels();
    let mut keys = vec![
        Key { name: "KeyW", label: w, rect: [ROW, 0, SIDE, SIDE] },
        Key { name: "KeyA", label: a, rect: [0, ROW, SIDE, SIDE] },
        Key { name: "KeyS", label: s, rect: [ROW, ROW, SIDE, SIDE] },
        Key { name: "KeyD", label: d, rect: [ROW * 2, ROW, SIDE, SIDE] },
    ];
    let mut y = ROW * 2;
    if settings.show_mouse {
        keys.push(Key { name: "MouseLeft", label: attack, rect: [0, y, WIDE, SIDE] });
        keys.push(Key { name: "MouseRight", label: use_, rect: [WIDE + GAP, y, WIDE, SIDE] });
        y += ROW;
    }
    if settings.show_space {
        keys.push(Key { name: "Space", label: String::new(), rect: [0, y, CARD_WIDTH, SPACE_HEIGHT] });
    }
    keys
}

/// The index of a key in `KEY_NAMES`, which names its bindings.
fn slot(name: &str) -> usize {
    KEY_NAMES.iter().position(|known| *known == name).unwrap_or(0)
}

// ---- Sizes and places ------------------------------------------------------------------

/// A text element's width before it has been measured, and its number of lines. Boxes are
/// fitted to their text (see `measure`); this is only the starting width.
fn text_dims(element: Element) -> (f32, usize) {
    match element {
        Element::Cps => (56.0, 1),
        Element::Fps => (50.0, 1),
        Element::Coords => (104.0, 1),
        Element::Facing => (82.0, 1),
        Element::Chunk => (116.0, 3),
        Element::Slime => (78.0, 1),
        Element::Monument => (110.0, 1),
        Element::Keystrokes => (CARD_WIDTH as f32, 0),
        Element::Armor => (ARMOR_ICON, 0),
        Element::Arrows => (62.0, 1),
        Element::Ping => (50.0, 1),
        Element::Speed => (66.0, 1),
        Element::Stopwatch => (46.0, 1),
        Element::Playtime => (46.0, 1),
        Element::Reach => (66.0, 1),
        Element::Combo => (62.0, 1),
        Element::Food => (92.0, 1),
        Element::Day => (52.0, 1),
        Element::Potion => (60.0, 1),
        Element::ArrayList => (80.0, 1),
        Element::ItemCounter => (90.0, 1),
        Element::Inventory => (INV_WIDTH, 0),
        Element::Clock => (64.0, 1),
        Element::ViewAngles => (90.0, 1),
        Element::PotionCounter => (84.0, 1),
        Element::TotemCounter => (84.0, 1),
        Element::PvpInfo => (84.0, 4),
        Element::Scoreboard => (96.0, 1),
        Element::TargetHud => (96.0, 2),
        Element::Waypoints => (110.0, 1),
        Element::Waila => (110.0, 1),
        Element::HeightLimit => (84.0, 1),
        Element::AudioSubtitles => (110.0, 1),
    }
}

/// How many lines a text element shows. Chunk info has up to three, each switchable (never
/// fewer than one, so the box cannot vanish).
fn line_count(settings: &Settings, element: Element) -> usize {
    if element == Element::Chunk {
        let s = settings.style(element);
        return [s.show_chunk, s.show_inchunk, s.show_sub].iter().filter(|on| **on).count().max(1);
    }
    if element == Element::ArrayList {
        return array_names(settings).len().max(1);
    }
    if element == Element::Waypoints {
        return WAYPOINT_LINES.with(Cell::get).max(1);
    }
    if element == Element::Scoreboard {
        return SIDEBAR_LINES.with(Cell::get).max(1);
    }
    if element == Element::Waila {
        return WAILA_LINES.with(Cell::get).max(1);
    }
    if element == Element::AudioSubtitles {
        return SUBTITLE_LINES.with(Cell::get).max(1);
    }
    if element == Element::ViewAngles {
        return va_line_count(settings);
    }
    if element == Element::Coords {
        return coords_line_count(settings);
    }
    text_dims(element).1
}

/// An element's size in GUI pixels at its scale, before any stretch.
fn base_size(settings: &Settings, element: Element) -> [f32; 2] {
    let k = settings.style(element).scale as f32;
    if element == Element::Keystrokes {
        let bottom = keys(settings).iter().map(|key| key.rect[1] + key.rect[3]).max().unwrap_or(0);
        return [CARD_WIDTH as f32 * k, bottom as f32 * k];
    }
    if element == Element::Armor {
        let g = armor_geometry(settings.style(element)).size;
        return [g[0] * k, g[1] * k];
    }
    if element == Element::Potion {
        let size = POTION.with(|cell| cell.borrow().size);
        return [size[0] * k, size[1] * k];
    }
    if element == Element::Inventory {
        return [INV_WIDTH * k, INV_HEIGHT * k];
    }
    if compass_on(settings, element) {
        return [COMPASS_W * k, (pad_y(settings.style(element)) * 2.0 + COMPASS_H) * k];
    }
    let fallback = text_dims(element).0;
    let lines = line_count(settings, element);
    let measured = f32::from(MEASURED.with(Cell::get)[element.index()]);
    let width = if measured > 0.0 { measured } else { fallback };
    let rows = if element == Element::Waila { (ROW_H * lines as f32).max(WAILA_ICON) } else { ROW_H * lines as f32 };
    [width * k, (pad_y(settings.style(element)) * 2.0 + rows) * k]
}

/// An element's size in GUI pixels at its scale, with its box stretched.
pub fn size(settings: &Settings, element: Element) -> [f32; 2] {
    let [w, h] = base_size(settings, element);
    let [sx, sy] = settings.stretch(element);
    [w * sx, h * sy]
}

/// What a stretch adds to an element's box beyond what its text needs, shared between the two
/// sides of the text (GUI pixels).
fn stretch_extra(settings: &Settings, element: Element) -> [f32; 2] {
    let [w, h] = base_size(settings, element);
    let [sx, sy] = settings.stretch(element);
    [w * (sx - 1.0) * 0.5, h * (sy - 1.0) * 0.5]
}

thread_local! {
    /// The unscaled width each text element's box was last fitted to (0: not measured yet).
    static MEASURED: Cell<[u16; Element::COUNT]> = const { Cell::new([0; Element::COUNT]) };
}

/// Asks the host how wide a line of text is (None if it cannot say). Set by the guest.
pub type Measurer = fn(&str) -> Option<f32>;

thread_local! {
    static MEASURER: Cell<Option<Measurer>> = const { Cell::new(None) };
}

/// Installs the host's text measurement for `text_width`.
pub fn set_measurer(measurer: Option<Measurer>) {
    MEASURER.with(|cell| cell.set(measurer));
}

/// Tells the layout which widths the boxes have now (from `measure`, possibly held wider for a
/// moment so they do not jitter).
pub fn set_widths(widths: [u16; Element::COUNT]) {
    MEASURED.with(|cell| cell.set(widths));
}

/// How wide `text` is in the game's font, in unscaled GUI pixels (each letter's advance, which
/// already includes the one pixel between letters, minus the trailing one). Slightly generous
/// for the few letters it is unsure of: too wide only adds padding, too narrow would cut text.
pub fn text_width(text: &str) -> f32 {
    // The host knows the real width of its own font: use it when it answers (one extra pixel
    // for rounding), and the table below only when it cannot.
    if let Some(width) = MEASURER.with(Cell::get).and_then(|measure| measure(text)) {
        return width.ceil() + 1.0;
    }
    let mut total = 0u32;
    for c in text.chars() {
        total += match c {
            ' ' => 4,
            '|' | ',' | '.' | ':' | ';' | '!' | '\'' | 'i' => 2,
            'l' => 3,
            't' | 'I' | '[' | ']' => 4,
            '(' | ')' | 'f' | 'k' | '<' | '>' | '"' | '*' | '{' | '}' => 5,
            '@' | '~' => 7,
            _ => 6,
        };
    }
    // The table runs narrow against the game's wider font, so add slack (about 8%) rather than cut text.
    (total.saturating_sub(1) as f32 * 1.08).ceil() + 1.0
}

/// The width each text element's box needs for what it shows now: its widest line plus padding,
/// rounded up to a whole pixel. Elements that are off, and Keystrokes, are 0.
#[inline(never)]
pub fn measure(view: &Views, settings: &Settings) -> [u16; Element::COUNT] {
    let mut widths = [0u16; Element::COUNT];
    for element in Element::ALL {
        if matches!(element, Element::Keystrokes | Element::Armor | Element::Potion | Element::Inventory) || !settings.is_on(element) || compass_on(settings, element) {
            continue;
        }
        // A fixed box is sized for the longest number it will show (two digits of CPS on each
        // side, three of FPS), so it does not change size as the count grows.
        let reference;
        let shown = if !settings.auto_width(element) && matches!(element, Element::Cps | Element::Fps) {
            reference = Views { cps: [99, 99], fps: Some(999), ..view.clone() };
            &reference
        } else {
            view
        };
        let mut widest = lines(element, shown, settings).iter().map(|l| text_width(&l.whole())).fold(0.0f32, f32::max);
        if element == Element::Coords {
            // The text at the left, a gap, and the direction at the right (the longest of each, so the box keeps its shape).
            let all = lines(element, shown, settings);
            let left = if settings.co_horizontal {
                text_width(&all.iter().map(Line::left).collect::<Vec<_>>().join("  "))
            } else {
                all.iter().map(|l| text_width(&l.left())).fold(0.0f32, f32::max)
            };
            let right = all.iter().filter_map(|l| l.tail.as_ref()).map(|(t, _)| text_width(t)).fold(0.0f32, f32::max);
            widest = left + if right > 0.0 { COORDS_GAP + right } else { 0.0 };
        }
        if element == Element::Waila {
            // The first line also holds the tool's icon and mark.
            let sample = crate::waila::sample();
            let block = shown.waila.as_ref().unwrap_or(&sample);
            let name = lines(element, shown, settings).first().map_or(0.0, |l| text_width(&l.text));
            widest = widest.max(name + waila_tool_width(block, settings));
        }
        let icon = if element == Element::Waila { WAILA_ICON + WAILA_GAP } else { 0.0 };
        widths[element.index()] = (widest + icon + pad_x(settings.style(element)) * 2.0).ceil().clamp(1.0, 400.0) as u16;
    }
    widths
}

/// How far an element's box reaches past what can be seen of it. With the background off only
/// the text shows, so the padding around it is empty: it must not keep the text from the screen's
/// edge. Zero while the box or its border is drawn (and for Keystrokes and Armor status, which have no padding).
pub fn bleed(settings: &Settings, element: Element) -> [f32; 2] {
    let style = settings.style(element);
    // A border shows the box's edge, so the box counts as drawn.
    if style.background || (style.border && Settings::has_border(element)) || matches!(element, Element::Keystrokes | Element::Armor | Element::Inventory) || (element == Element::Potion && settings.potion_mode == 1) {
        return [0.0, 0.0];
    }
    let k = style.scale as f32;
    let extra = stretch_extra(settings, element);
    [pad_x(style) * k + extra[0], pad_y(style) * k + extra[1]]
}

/// Where an element's top-left corner goes for a placed (not pinned) element. The visible part
/// (the box, or just the text without one) can reach every edge of the screen.
fn placed_offset(size: [f32; 2], viewport: [f32; 2], at: [f32; 2], bleed: [f32; 2]) -> [f32; 2] {
    let axis = |i: usize| (viewport[i] - size[i] + bleed[i] * 2.0).max(0.0) * at[i] - bleed[i];
    [axis(0), axis(1)]
}

/// An element's top-left corner on a screen of `viewport` GUI pixels.
pub fn top_left(settings: &Settings, element: Element, viewport: [f32; 2]) -> [f32; 2] {
    let style = settings.style(element);
    let [w, h] = size(settings, element);
    if let Some(at) = style.position {
        return placed_offset([w, h], viewport, at, bleed(settings, element));
    }
    let anchor = style.corner.anchor();
    [
        if anchor.ends_with("right") { viewport[0] - w - MARGIN } else { MARGIN },
        if anchor.starts_with("bottom") { viewport[1] - h - MARGIN } else { MARGIN },
    ]
}

/// The placement that puts an element's top-left corner at `corner`, kept on screen.
pub fn place_at(settings: &Settings, element: Element, viewport: [f32; 2], corner: [f32; 2]) -> [f32; 2] {
    let [w, h] = size(settings, element);
    let b = bleed(settings, element);
    let free = [(viewport[0] - w + b[0] * 2.0).max(1.0), (viewport[1] - h + b[1] * 2.0).max(1.0)];
    [((corner[0] + b[0]) / free[0]).clamp(0.0, 1.0), ((corner[1] + b[1]) / free[1]).clamp(0.0, 1.0)]
}

/// What can be seen of an element: its rectangle without the empty padding a box that is off
/// leaves around the text (x, y, width, height).
pub fn visible_rect(settings: &Settings, element: Element, viewport: [f32; 2]) -> [f32; 4] {
    let [x, y, w, h] = rect(settings, element, viewport);
    let [bx, by] = bleed(settings, element);
    [x + bx, y + by, (w - bx * 2.0).max(1.0), (h - by * 2.0).max(1.0)]
}

/// How close (GUI pixels) an edge or centre must come to a guide to jump onto it.
pub const SNAP: f32 = 5.0;
/// Within this many pixels of a guide an element sits exactly on it.
pub const SNAP_LOCK: f32 = 1.0;
/// The same for the lines through the middle of the screen, which hold an element more firmly.
pub const CENTER_SNAP: f32 = 8.0;
pub const CENTER_LOCK: f32 = 3.0;

/// Moves a dragged element's top-left `target` onto nearby guides: the screen's edges (and the
/// gap kept at the corners) and centre lines, and the edges and centres of the other elements
/// that are on. Each axis snaps on its own, to the closest guide within `SNAP`.
pub fn snap(settings: &Settings, element: Element, viewport: [f32; 2], target: [f32; 2]) -> [f32; 2] {
    let [w, h] = size(settings, element);
    // No guides flush with the screen edge: they competed with the margin and left elements at
    // different distances from the edges. Every edge snaps to the same `MARGIN` gap.
    // With Snap to guides off, only the centre lines pull (they are on their own switch).
    let (mut guides_x, mut guides_y) = screen_guides(settings, viewport);
    for other in Element::ALL {
        if other == element || !settings.is_on(other) || !settings.align {
            continue;
        }
        let [x, y, ow, oh] = visible_rect(settings, other, viewport);
        guides_x.extend([x, x + ow * 0.5, x + ow]);
        guides_y.extend([y, y + oh * 0.5, y + oh]);
    }
    // Only what shows is lined up: the empty padding of a box that is off is not.
    let [bx, by] = bleed(settings, element);
    [snap_axis(target[0] + bx, w - bx * 2.0, &guides_x, viewport[0] * 0.5) - bx, snap_axis(target[1] + by, h - by * 2.0, &guides_y, viewport[1] * 0.5) - by]
}

/// An element's rectangle on screen: x, y, width, height.
pub fn rect(settings: &Settings, element: Element, viewport: [f32; 2]) -> [f32; 4] {
    let [x, y] = top_left(settings, element, viewport);
    let [w, h] = size(settings, element);
    [x, y, w, h]
}

/// Whether any element that is on has been placed with the mouse or the X/Y sliders, so the
/// layer follows the screen size.
pub fn any_placed(settings: &Settings) -> bool {
    // Armor status always needs the screen size: its pieces are placed with bindings.
    Element::ALL.iter().any(|e| settings.is_on(*e) && (settings.style(*e).position.is_some() || *e == Element::Armor))
}

/// Whether any element is on.
pub fn any_on(settings: &Settings) -> bool {
    settings.crosshair.enabled || Element::ALL.iter().any(|e| settings.is_on(*e))
}

// ---- Resize handles --------------------------------------------------------------------

/// The side of a resize handle's square, in GUI pixels.
const HANDLE: f32 = 6.0;
/// How far the editor's frame sits outside what it frames, so it does not touch the text.
const FRAME_GAP: f32 = 3.0;
/// The frame and handles' colour: the menu's violet.
const FRAME_COLOR: [f32; 4] = [0.62, 0.42, 1.0, 1.0];
/// How far from a corner a press still takes that corner's handle.
const HANDLE_REACH: f32 = 7.0;
/// The handles' bindings, one for each corner: top left, top right, bottom left, bottom right.
const HANDLE_BINDINGS: [&str; 8] = ["#hp0", "#hp1", "#hp2", "#hp3", "#hp4", "#hp5", "#hp6", "#hp7"];
/// The binding that places the frame's top left corner.
const FRAME_BINDING: &str = "#hf";
/// The editor's two buttons on the frame (Lunar's): the settings gear and the red X that removes the element.
const GEAR_BINDING: &str = "#hg";
const REMOVE_BINDING: &str = "#hx";
/// The side of a button's round badge, in GUI pixels.
const ICON: f32 = 12.0;

/// Which button of the frame a press took.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameButton {
    Settings,
    Remove,
}

/// Where the gear and the X sit for a frame: inside its top corners, or (for a frame too small to hold them)
/// in a row just above it.
pub fn button_places(frame: [f32; 4]) -> [[f32; 2]; 2] {
    let [x, y, w, h] = frame;
    let icon = ICON;
    if w >= icon * 2.0 + 4.0 && h >= icon + 4.0 {
        [[(x + 1.0).round(), (y + 1.0).round()], [(x + w - 1.0 - icon).round(), (y + 1.0).round()]]
    } else {
        let top = (y - icon - 1.0).max(0.0).round();
        [[x.round(), top], [(x + icon + 2.0).round(), top]]
    }
}

/// The frame's button under `point`, if any.
pub fn button_at(frame: [f32; 4], point: [f32; 2]) -> Option<FrameButton> {
    let places = button_places(frame);
    let icon = ICON;
    [FrameButton::Settings, FrameButton::Remove].into_iter().zip(places).find_map(|(button, at)| {
        let inside = point[0] >= at[0] + 1.0 && point[0] <= at[0] + icon - 1.0 && point[1] >= at[1] + 1.0 && point[1] <= at[1] + icon - 1.0;
        inside.then_some(button)
    })
}

thread_local! {
    /// The visible rectangle of the element the editor shows resize handles on (none: no handles).
    static HANDLES: Cell<Option<[f32; 4]>> = const { Cell::new(None) };
    /// Whether the handles at the middles of the sides show too (they stretch the box).
    static EDGE_HANDLES: Cell<bool> = const { Cell::new(false) };
    /// Whether the lines through the middle of the screen show (while elements are moved).
    static GUIDES: Cell<bool> = const { Cell::new(false) };
}

/// Shows or hides the centre lines. They are part of the layer's document, so a change needs it written again.
pub fn set_guides(on: bool) {
    GUIDES.with(|cell| cell.set(on));
}

/// The centre lines: one down the middle of the screen and one across it, thin, white and see-through, under the
/// elements. Each is a panel the width of the screen's middle, so they follow any screen size.
#[inline(never)]
fn guides_overlay(o: &mut String) {
    let line = [1.0, 1.0, 1.0, 0.35];
    for (name, vertical) in [("gv", true), ("gh", false)] {
        o.push_str(r##"{\""##);
        o.push_str(name);
        o.push_str(r##"\":{\"type\":\"panel\",\"size\":["##);
        o.push_str(if vertical { r##"1,\"100%\""## } else { r##"\"100%\",1"## });
        o.push_str(r##"],\"anchor_from\":\""##);
        o.push_str(if vertical { "top_middle" } else { "left_middle" });
        o.push_str(r##"\",\"anchor_to\":\""##);
        o.push_str(if vertical { "top_middle" } else { "left_middle" });
        o.push_str(r##"\",\"offset\":[0,0],\"controls\":[{\"l\":{\"type\":\"custom\",\"renderer\":\"cinnabar_rounded_rectangle\",\"radius\":0,\"color\":"##);
        write_rgba(o, line);
        o.push_str(r##",\"size\":[\"100%\",\"100%\"]}}]}}"##);
        if vertical {
            o.push(',');
        }
    }
}

/// The editor's frame around a visible rectangle: a little bigger, so it clears the text.
pub fn frame_rect(visible: [f32; 4]) -> [f32; 4] {
    [(visible[0] - FRAME_GAP).round(), (visible[1] - FRAME_GAP).round(), (visible[2] + FRAME_GAP * 2.0).round(), (visible[3] + FRAME_GAP * 2.0).round()]
}

/// Sets where the resize handles are drawn: at the corners of `rect`, or nowhere.
pub fn set_handles(rect: Option<[f32; 4]>, edges: bool) {
    HANDLES.with(|cell| cell.set(rect));
    EDGE_HANDLES.with(|cell| cell.set(edges));
}

/// How many handles show: the four corners, and with `edges` the four sides' middles.
fn handle_count(edges: bool) -> usize {
    if edges { 8 } else { 4 }
}

/// Each corner of `rect`: top left, top right, bottom left, bottom right.
fn corners(rect: [f32; 4]) -> [[f32; 2]; 4] {
    let [x, y, w, h] = rect;
    [[x, y], [x + w, y], [x, y + h], [x + w, y + h]]
}

/// The middles of a rectangle's sides: left, right, top, bottom.
fn sides(rect: [f32; 4]) -> [[f32; 2]; 4] {
    let [x, y, w, h] = rect;
    [[x, y + h * 0.5], [x + w, y + h * 0.5], [x + w * 0.5, y], [x + w * 0.5, y + h]]
}

/// Where each handle is: the corners (0 to 3, as in `corners`), then the sides' middles (4 left,
/// 5 right, 6 top, 7 bottom).
fn handle_points(rect: [f32; 4]) -> [[f32; 2]; 8] {
    let c = corners(rect);
    let m = sides(rect);
    [c[0], c[1], c[2], c[3], m[0], m[1], m[2], m[3]]
}

/// The handle of `rect` that a press at `point` takes, if any (0 to 7, see `handle_points`). A
/// corner wins over a side's middle when both are in reach.
pub fn handle_at(rect: [f32; 4], edges: bool, point: [f32; 2]) -> Option<usize> {
    handle_points(rect)
        .iter()
        .take(handle_count(edges))
        .enumerate()
        .map(|(i, c)| (i, (c[0] - point[0]).abs().max((c[1] - point[1]).abs())))
        .filter(|(_, d)| *d <= HANDLE_REACH)
        .min_by(|a, b| (a.1 + if a.0 < 4 { 0.0 } else { 1.5 }).total_cmp(&(b.1 + if b.0 < 4 { 0.0 } else { 1.5 })))
        .map(|(i, _)| i)
}

/// Where each handle's square sits so it is centred on its point.
fn handle_offsets(rect: [f32; 4]) -> [[f32; 2]; 8] {
    handle_points(rect).map(|c| [(c[0] - HANDLE * 0.5).round(), (c[1] - HANDLE * 0.5).round()])
}

fn handle_members(o: &mut String, rect: [f32; 4], edges: bool) {
    o.push('"');
    o.push_str(FRAME_BINDING);
    o.push_str("\":[");
    signed(o, rect[0]);
    o.push(',');
    signed(o, rect[1]);
    o.push(']');
    for (i, at) in handle_offsets(rect).iter().take(handle_count(edges)).enumerate() {
        o.push(',');
        o.push('"');
        o.push_str(HANDLE_BINDINGS[i]);
        o.push_str("\":[");
        signed(o, at[0]);
        o.push(',');
        signed(o, at[1]);
        o.push(']');
    }
    for (binding, at) in [GEAR_BINDING, REMOVE_BINDING].into_iter().zip(button_places(rect)) {
        o.push_str(",\"");
        o.push_str(binding);
        o.push_str("\":[");
        signed(o, at[0]);
        o.push(',');
        signed(o, at[1]);
        o.push(']');
    }
}

/// A patch that moves the handles to `rect`.
pub fn handles_patch(rect: [f32; 4], edges: bool) -> String {
    let mut o = String::with_capacity(160);
    o.push_str("{\"binding_changes\":{");
    handle_members(&mut o, rect, edges);
    o.push_str("}}");
    o
}

/// A thin filled rectangle of the frame or a handle, in the escaped document.
fn solid(o: &mut String, name: &str, color: [f32; 4], size: [f32; 2], offset: [f32; 2], radius: f32) {
    o.push_str(r##"{\""##);
    o.push_str(name);
    o.push_str(r##"\":{\"type\":\"custom\",\"renderer\":\"cinnabar_rounded_rectangle\",\"radius\":"##);
    num(o, f64::from(radius));
    o.push_str(r##",\"color\":"##);
    write_rgba(o, color);
    o.push_str(r##",\"size\":["##);
    num(o, f64::from(size[0]));
    o.push(',');
    num(o, f64::from(size[1]));
    o.push_str(r##"],\"offset\":["##);
    num(o, f64::from(offset[0]));
    o.push(',');
    num(o, f64::from(offset[1]));
    o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\"}}"##);
}

/// The handles' part of the layer: a thin frame around the element and a small square on each
/// of its corners, each placed by its binding. The frame's size is part of the document.
#[inline(never)]
fn handles_overlay(o: &mut String, rect: [f32; 4], edges: bool) {
    let (w, h) = (rect[2].max(2.0), rect[3].max(2.0));
    o.push_str(r##"{\"hd\":{\"type\":\"panel\",\"size\":[\"100%\",\"100%\"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"controls\":["##);
    // The frame: four one-pixel lines in a panel placed by `#hf`.
    o.push_str(r##"{\"fr\":{\"type\":\"panel\",\"size\":["##);
    num(o, f64::from(w));
    o.push(',');
    num(o, f64::from(h));
    o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":[0,0],\"bindings\":[{\"binding_name\":\""##);
    o.push_str(FRAME_BINDING);
    o.push_str(r##"\",\"binding_name_override\":\"#offset\"}],\"controls\":["##);
    let line = [FRAME_COLOR[0], FRAME_COLOR[1], FRAME_COLOR[2], 0.8];
    solid(o, "t", line, [w, 1.0], [0.0, 0.0], 0.0);
    o.push(',');
    solid(o, "b", line, [w, 1.0], [0.0, h - 1.0], 0.0);
    o.push(',');
    solid(o, "l", line, [1.0, h], [0.0, 0.0], 0.0);
    o.push(',');
    solid(o, "r", line, [1.0, h], [w - 1.0, 0.0], 0.0);
    o.push_str("]}}");
    for (i, binding) in HANDLE_BINDINGS.iter().take(handle_count(edges)).enumerate() {
        o.push(',');
        o.push_str(r##"{\"h"##);
        push_u(o, i as u64);
        o.push_str(r##"\":{\"type\":\"panel\",\"size\":["##);
        num(o, f64::from(HANDLE));
        o.push(',');
        num(o, f64::from(HANDLE));
        o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":[0,0],\"bindings\":[{\"binding_name\":\""##);
        o.push_str(binding);
        o.push_str(r##"\",\"binding_name_override\":\"#offset\"}],\"controls\":["##);
        solid(o, "rim", FRAME_COLOR, [HANDLE, HANDLE], [0.0, 0.0], 1.5);
        o.push(',');
        solid(o, "dot", [1.0, 1.0, 1.0, 1.0], [HANDLE - 2.0, HANDLE - 2.0], [1.0, 1.0], 1.0);
        o.push_str("]}}");
    }
    // The buttons: the Lucide cog and X from the mod's own font (`mod_panel`, see `tools/icons.py`: "settings" and "x"),
    // as plain icons with the game's own text drop shadow and nothing behind them. A glyph is smooth at any GUI scale.
    // The glyph sits a little inside its square, so each is nudged towards its corner (x, y in GUI pixels).
    let icon = ICON;
    for (name, binding, glyph, color, nudge, size) in [
        ("bg", GEAR_BINDING, "\u{E806}", [0.88, 0.89, 0.93, 1.0], [-2.0_f32, -2.0_f32], 0.64_f32),
        ("bx", REMOVE_BINDING, "\u{E809}", [0.93, 0.16, 0.2, 1.0], [-9.4, -3.5], 0.83),
    ] {
        o.push_str(r##",{\""##);
        o.push_str(name);
        o.push_str(r##"\":{\"type\":\"panel\",\"size\":["##);
        num(o, f64::from(icon));
        o.push(',');
        num(o, f64::from(icon));
        o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":[0,0],\"bindings\":[{\"binding_name\":\""##);
        o.push_str(binding);
        o.push_str(r##"\",\"binding_name_override\":\"#offset\"}],\"controls\":["##);
        // The font has one weight and the game's text shadow sits a full pixel away, so both are made here: the glyph
        // is laid twice, a few tenths of a pixel apart (a little bolder), and a black copy of that sits just under it.
        // The copies fan out along the bottom and right edges, not just down-right: a diagonal stroke (the X's
        // backslash) would hide a shadow that runs along it.
        // Lunar's buttons are the same size on every element: about 5.6 GUI pixels (the cog) and 4.7 (the X), measured on
        // its big, medium and small boxes. `size` is the font scale that gives that; the nudges and the shadow follow it.
        let k = size / 0.95;
        let dark = [0.0, 0.0, 0.0, 0.55];
        let layers = [
            (dark, 0.0_f32, 1.1_f32),
            (dark, 0.55, 1.1),
            (dark, 1.1, 1.1),
            (dark, 1.1, 0.55),
            (dark, 1.1, 0.0),
            (color, 0.0, 0.0),
            (color, 0.4, 0.0),
        ];
        for (n, (tint, dx, dy)) in layers.into_iter().enumerate() {
            if n > 0 {
                o.push(',');
            }
            o.push_str(r##"{\"i"##);
            push_u(o, n as u64);
            o.push_str(r##"\":{\"type\":\"label\",\"text\":\""##);
            o.push_str(glyph);
            o.push_str(r##"\",\"localize\":false,\"font_type\":\"mod_panel\",\"shadow\":false,\"color\":"##);
            write_rgba(o, tint);
            // One line of the font at scale 0.95 is 11.4 pixels tall: centred on the square.
            o.push_str(r##",\"text_alignment\":\"center\",\"font_scale_factor\":"##);
            num(o, f64::from(size));
            o.push_str(r##",\"size\":["##);
            num(o, f64::from(icon));
            o.push_str(r##","##);
            num(o, f64::from(11.4 * k));
            o.push_str(r##"],\"offset\":["##);
            num(o, f64::from((dx + nudge[0]) * k));
            o.push(',');
            num(o, f64::from((icon - 11.4 * k) / 2.0 + (dy + nudge[1]) * k));
            o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\"}}"##);
        }
        o.push_str("]}");
        o.push('}');
    }
    o.push_str("]}}");
}

/// A drag of one of the handles. A corner resizes the element: the opposite corner stays where
/// it is and the size follows the pointer. The middle of a side stretches the box on that axis,
/// with the opposite side staying put.
pub struct Resize {
    element: Element,
    /// Which handle is held (see `handle_points`).
    handle: usize,
    /// The opposite corner (or side), which stays put.
    anchor: [f32; 2],
    /// Where the pointer held the handle, from the handle.
    grab: [f32; 2],
    scale: f64,
    stretch: [f64; 2],
    /// The visible size and top left corner at the start of the drag.
    size: [f32; 2],
    origin: [f32; 2],
}

impl Resize {
    pub fn new(settings: &Settings, element: Element, viewport: [f32; 2], handle: usize, pointer: [f32; 2]) -> Self {
        let rect = visible_rect(settings, element, viewport);
        let (held, anchor) = if handle < 4 {
            (corners(rect)[handle], corners(rect)[3 - handle])
        } else {
            (sides(rect)[handle - 4], sides(rect)[(handle - 4) ^ 1])
        };
        Self {
            element,
            handle,
            anchor,
            grab: [pointer[0] - held[0], pointer[1] - held[1]],
            scale: settings.style(element).scale,
            stretch: settings.style(element).stretch,
            size: [rect[2], rect[3]],
            origin: [rect[0], rect[1]],
        }
    }

    /// Follows the pointer: sets the element's scale or stretch (to a hundredth, like the
    /// sliders) and moves it so the opposite corner or side stays. Returns whether it changed.
    pub fn step(&self, settings: &mut Settings, viewport: [f32; 2], pointer: [f32; 2]) -> bool {
        if self.handle >= 4 {
            return self.stretch_step(settings, viewport, pointer);
        }
        // Which way each axis grows from the anchor: left corners (0 and 2) grow leftwards.
        let sx = if self.handle % 2 == 0 { -1.0 } else { 1.0 };
        let sy = if self.handle < 2 { -1.0 } else { 1.0 };
        let dx = (pointer[0] - self.grab[0] - self.anchor[0]) * sx;
        let dy = (pointer[1] - self.grab[1] - self.anchor[1]) * sy;
        let [w0, h0] = self.size;
        // The pointer's place along the element's diagonal, as a share of the starting size.
        let ratio = f64::from((dx * w0 + dy * h0) / (w0 * w0 + h0 * h0).max(1.0));
        let largest = f64::from((viewport[0] / w0).min(viewport[1] / h0));
        let wanted = (self.scale * ratio).clamp(MIN_SCALE, (self.scale * largest).clamp(MIN_SCALE, MAX_SCALE));
        // Hundredths, like the Size slider.
        let wanted = (wanted * 100.0).round() / 100.0;
        let wanted = wanted.clamp(MIN_SCALE, MAX_SCALE);
        if (settings.style(self.element).scale - wanted).abs() < 1e-9 {
            return false;
        }
        settings.style_mut(self.element).scale = wanted;
        let k = (wanted / self.scale) as f32;
        let (w, h) = (w0 * k, h0 * k);
        let visible = [if sx > 0.0 { self.anchor[0] } else { self.anchor[0] - w }, if sy > 0.0 { self.anchor[1] } else { self.anchor[1] - h }];
        let b = bleed(settings, self.element);
        let at = place_at(settings, self.element, viewport, [visible[0] - b[0], visible[1] - b[1]]);
        settings.style_mut(self.element).position = Some(at);
        true
    }

    /// A side's middle: only that axis changes, the other keeps its place and length.
    fn stretch_step(&self, settings: &mut Settings, viewport: [f32; 2], pointer: [f32; 2]) -> bool {
        let side = self.handle - 4;
        let axis = usize::from(side >= 2);
        // Left and top sides grow away from the anchor towards smaller coordinates.
        let sign = if side % 2 == 0 { -1.0 } else { 1.0 };
        let length = (pointer[axis] - self.grab[axis] - self.anchor[axis]) * sign;
        let along = self.size[axis];
        let ratio = f64::from(length / along.max(1.0));
        let most = (self.stretch[axis] * f64::from(viewport[axis] / along.max(1.0))).clamp(MIN_STRETCH, MAX_STRETCH);
        let wanted = ((self.stretch[axis] * ratio).clamp(MIN_STRETCH, most) * 100.0).round() / 100.0;
        let wanted = wanted.clamp(MIN_STRETCH, MAX_STRETCH);
        if (settings.style(self.element).stretch[axis] - wanted).abs() < 1e-9 {
            return false;
        }
        settings.style_mut(self.element).stretch[axis] = wanted;
        let new_length = along * (wanted / self.stretch[axis]) as f32;
        let mut visible = self.origin;
        visible[axis] = if sign > 0.0 { self.anchor[axis] } else { self.anchor[axis] - new_length };
        let b = bleed(settings, self.element);
        let at = place_at(settings, self.element, viewport, [visible[0] - b[0], visible[1] - b[1]]);
        settings.style_mut(self.element).position = Some(at);
        true
    }
}

// ---- Text ------------------------------------------------------------------------------

/// One line of a text element. View angles' lines have two colours: the number (`text`) and its label (`tail`).
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    pub color: [f32; 4],
    pub tail: Option<(String, [f32; 4])>,
    /// Coordinates' lines have a third piece, the label before the number ("X:") in a colour of its own.
    pub head: Option<(String, [f32; 4])>,
}

fn line(text: String, color: [f32; 4]) -> Line {
    Line { text, color, tail: None, head: None }
}

impl Line {
    /// The left part of a Coordinates line as one piece of text ("X: 500").
    fn left(&self) -> String {
        match &self.head {
            Some((head, _)) => format!("{head} {}", self.text),
            None => self.text.clone(),
        }
    }

    /// The whole line as one piece of text ("120.00° Yaw").
    fn whole(&self) -> String {
        match &self.tail {
            Some((tail, _)) => format!("{} {tail}", self.text),
            None => self.text.clone(),
        }
    }
}

/// The numbers View angles shows, each times ten to the number of decimals (see `Views::va`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VaView {
    pub yaw: i32,
    pub pitch: i32,
    pub speed: i32,
    pub altitude: i32,
}

/// `value` (times ten to the `decimals`) as text: 12000 with 2 decimals is "120.00", -4000 is "-40.00".
fn scaled_text(value: i32, decimals: usize) -> String {
    let scale = 10i64.pow(decimals as u32);
    let v = i64::from(value);
    let (sign, v) = if v < 0 { ("-", -v) } else { ("", v) };
    if decimals == 0 {
        format!("{sign}{v}")
    } else {
        format!("{sign}{}.{:0width$}", v / scale, v % scale, width = decimals)
    }
}

/// The pitch range (degrees, up is negative) in which View angles turns the pitch green: the climbing angle of a firework flight.
const VA_TARGET: (f64, f64) = (-45.0, -35.0);
const VA_GREEN: [f32; 4] = [0.333, 1.0, 0.333, 1.0];

/// The lines View angles has: (number, label, whether it is the pitch inside the target), in the order shown.
fn va_rows(view: &Views, settings: &Settings) -> Vec<(String, &'static str, bool)> {
    let d = settings.va_decimals.min(crate::config::VA_MAX_DECIMALS);
    let v = view.va.unwrap_or_default();
    let scale = 10f64.powi(d as i32);
    let mut rows: Vec<(String, &'static str, bool)> = Vec::new();
    if settings.va_yaw {
        rows.push((format!("{}\u{b0}", scaled_text(v.yaw, d)), "Yaw", false));
    }
    if settings.va_pitch {
        let pitch = f64::from(v.pitch) / scale;
        rows.push((format!("{}\u{b0}", scaled_text(v.pitch, d)), "Pitch", settings.va_target && pitch >= VA_TARGET.0 && pitch <= VA_TARGET.1));
    }
    if settings.va_speed {
        rows.push((format!("{} m/s", scaled_text(v.speed, d)), "Speed", false));
    }
    if settings.va_altitude {
        rows.push((scaled_text(v.altitude, d), "Altitude", false));
    }
    if rows.is_empty() {
        rows.push((format!("{}\u{b0}", scaled_text(v.yaw, d)), "Yaw", false));
    }
    if settings.va_reverse {
        rows.reverse();
    }
    rows
}

/// The room between Coordinates' text and its direction (GUI pixels).
const COORDS_GAP: f32 = 12.0;

/// The number of one coordinate: whole, or to two decimals.
fn coord_text(whole: i32, exact: f64, decimal: bool) -> String {
    if decimal { format!("{exact:.2}") } else { whole.to_string() }
}

/// The direction Coordinates shows at its right: the letter, and the axis it faces along with its sign (S "+", N "-" on Z; W "-", E "+" on X).
fn coords_direction(view: &Views) -> Option<(&'static str, &'static str)> {
    view.facing.map(|f| match f & 3 {
        0 => ("S", "+"),
        1 => ("W", "-"),
        2 => ("N", "-"),
        _ => ("E", "+"),
    })
}

/// How many coordinates Coordinates shows (never none).
fn coords_cells(settings: &Settings) -> usize {
    [settings.co_x, settings.co_y, settings.co_z].iter().filter(|on| **on).count().max(1)
}

/// How many lines Coordinates has: one across in horizontal mode, else a line for each coordinate.
fn coords_line_count(settings: &Settings) -> usize {
    if settings.co_horizontal { 1 } else { coords_cells(settings) }
}

/// Lunar's Coordinates: "X: 500", "Y: 62", "Z: 250" (each label and number in colours of their own), a line each or all on one line,
/// and the direction at the right (its letter beside the second line, its sign beside the third). Every piece is a `Line`: `head` the
/// label, `text` the number, `tail` what sits at the right edge.
fn coords_lines(view: &Views, settings: &Settings) -> Vec<Line> {
    let decimal = settings.co_decimal;
    let [x, y, z] = view.feet;
    let [fx, fy, fz] = view.feet_exact;
    let any = settings.co_x || settings.co_y || settings.co_z;
    let mut cells: Vec<Line> = Vec::new();
    for (on, name, label, whole, exact, lc, tc) in [
        (settings.co_x || !any, "X:", settings.co_x_label, x, fx, settings.co_x_lc, settings.co_x_tc),
        (settings.co_y, "Y:", settings.co_y_label, y, fy, settings.co_y_lc, settings.co_y_tc),
        (settings.co_z, "Z:", settings.co_z_label, z, fz, settings.co_z_lc, settings.co_z_tc),
    ] {
        if on {
            let color = rgba(tc, 1.0);
            cells.push(Line { text: coord_text(whole, exact, decimal), color, tail: Some((String::new(), color)), head: label.then(|| (name.to_owned(), rgba(lc, 1.0))) });
        }
    }
    let direction = if settings.co_dir { coords_direction(view) } else { None };
    let letter_color = rgba(settings.co_dir_tc, 1.0);
    if let Some((letter, sign)) = direction {
        let letter = if settings.co_dir_cardinal { letter } else { "" };
        let sign = if settings.co_dir_mod { sign } else { "" };
        if settings.co_horizontal {
            let both = if letter.is_empty() || sign.is_empty() { format!("{letter}{sign}") } else { format!("{letter} {sign}") };
            if let Some(last) = cells.last_mut() {
                last.tail = Some((both, letter_color));
            }
        } else {
            // The sign wears the colour of the axis it runs along: X for east and west, Z for north and south.
            let sign_color = rgba(if view.facing.is_some_and(|f| f & 1 == 1) { settings.co_dir_mx } else { settings.co_dir_mz }, 1.0);
            let letter_row = 1.min(cells.len() - 1);
            if let Some(cell) = cells.get_mut(letter_row) {
                cell.tail = Some((letter.to_owned(), letter_color));
            }
            if let Some(cell) = cells.get_mut(letter_row + 1) {
                cell.tail = Some((sign.to_owned(), sign_color));
            }
        }
    }
    cells
}

/// Where Coordinates' piece `n` puts its label, its number and its tail (flush with the right edge), in screen pixels.
fn coords_place(settings: &Settings, cells: &[Line], n: usize) -> [[f32; 2]; 3] {
    let style = settings.style(Element::Coords);
    let k = style.scale as f32;
    let s = k * FONT;
    let [w, _] = size(settings, Element::Coords);
    let extra = stretch_extra(settings, Element::Coords);
    let raw = |t: &str| (text_width(t) - 1.0).max(0.0) * s;
    let x0 = pad_x(style) * k + extra[0];
    let y0 = (pad_y(style) + 1.0) * k + extra[1];
    let row = &cells[n.min(cells.len() - 1)];
    let (x, y) = if settings.co_horizontal {
        let before: Vec<String> = cells[..n].iter().map(Line::left).collect();
        (x0 + if n == 0 { 0.0 } else { raw(&format!("{}  ", before.join("  "))) }, y0)
    } else {
        (x0, y0 + ROW_H * n as f32 * k)
    };
    let value_x = x + row.head.as_ref().map_or(0.0, |(head, _)| raw(&format!("{head} ")));
    let tail = row.tail.as_ref().map_or(0.0, |(t, _)| raw(t));
    [[x, y], [value_x, y], [(w - extra[0] - pad_x(style) * k - tail).max(value_x), y0 + if settings.co_horizontal { 0.0 } else { ROW_H * n as f32 * k }]]
}

/// How many lines View angles has (never fewer than one).
fn va_line_count(settings: &Settings) -> usize {
    [settings.va_yaw, settings.va_pitch, settings.va_speed, settings.va_altitude].iter().filter(|on| **on).count().max(1)
}

fn va_lines(view: &Views, settings: &Settings) -> Vec<Line> {
    let value = rgba(settings.va_value_color, 1.0);
    let label = rgba(settings.va_label_color, 1.0);
    va_rows(view, settings)
        .into_iter()
        .map(|(text, name, target)| Line { text, color: if target { VA_GREEN } else { value }, tail: Some((name.to_owned(), label)), head: None })
        .collect()
}

/// Where View angles' line `n` puts its number and its label (offsets in screen pixels): the pair sits left, centred or right in the box.
fn pair_place(settings: &Settings, element: Element, row: &Line, n: usize) -> [[f32; 2]; 2] {
    let style = settings.style(element);
    let k = style.scale as f32;
    let s = k * FONT;
    let [w, _] = size(settings, element);
    let extra = stretch_extra(settings, element);
    let raw = |t: &str| (text_width(t) - 1.0).max(0.0) * s;
    let whole = raw(&row.whole());
    let room = w - extra[0] * 2.0 - pad_x(style) * 2.0 * k;
    // PvP Info is always left-aligned.
    let align = if element == Element::ViewAngles { settings.va_align } else { 0 };
    let start = match align {
        0 => 0.0,
        1 => (room - whole) / 2.0,
        _ => room - whole,
    }
    .max(0.0);
    let x = pad_x(style) * k + extra[0] + start;
    let y = (pad_y(style) + 1.0 + ROW_H * n as f32) * k + extra[1];
    [[x, y], [x + raw(&format!("{} ", row.text)), y]]
}

/// A "label value" line, or just the value when labels are off.
fn labelled(labels: bool, label: &str, value: &str) -> String {
    if labels {
        let mut text = String::with_capacity(label.len() + 1 + value.len());
        text.push_str(label);
        text.push(' ');
        text.push_str(value);
        text
    } else {
        value.to_owned()
    }
}

fn pair(a: i32, b: i32) -> String {
    let mut text = String::new();
    push_i(&mut text, i64::from(a));
    text.push_str(", ");
    push_i(&mut text, i64::from(b));
    text
}

/// A bare value, in brackets when the element asks for them.
fn bracketed(style: &Style, value: String) -> String {
    if style.brackets { format!("[{value}]") } else { value }
}

/// How a time is written: "m:ss" from the first minute, "h:mm:ss" from the first hour, or by the
/// player's choice always the one or the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockStyle {
    /// "m:ss", and "h:mm:ss" from an hour on (the Stopwatch).
    Auto,
    /// "m:ss" whatever the time: minutes pass 59 ("75:20").
    Minutes,
    /// "h:mm:ss" whatever the time ("0:01:05").
    Hours,
}

/// Seconds as a clock.
pub fn clock_text(secs: u32, style: ClockStyle) -> String {
    let hours = match style {
        ClockStyle::Auto => secs >= 3600,
        ClockStyle::Minutes => false,
        ClockStyle::Hours => true,
    };
    let (h, m, s) = if hours { (secs / 3600, secs / 60 % 60, secs % 60) } else { (0, secs / 60, secs % 60) };
    let mut t = String::new();
    if hours {
        t.push_str(&count(h));
        t.push(':');
        if m < 10 {
            t.push('0');
        }
    }
    t.push_str(&count(m));
    t.push(':');
    if s < 10 {
        t.push('0');
    }
    t.push_str(&count(s));
    t
}

/// `value` with `decimals` digits after the point, where `value` is the number times ten to the
/// `decimals`: 56 with one decimal is "5.6", 5 with two is "0.05", 56 with none is "56".
pub fn fixed(value: u32, decimals: usize) -> String {
    let decimals = decimals.min(MAX_SPEED_DECIMALS);
    let scale = 10u32.pow(decimals as u32);
    let mut text = count(value / scale);
    if decimals > 0 {
        text.push('.');
        let fraction = count(value % scale);
        for _ in fraction.len()..decimals {
            text.push('0');
        }
        text.push_str(&fraction);
    }
    text
}

/// What the Speed element calls its unit.
pub fn speed_unit_label(settings: &Settings) -> &'static str {
    if settings.speed_unit == 1 { "km/h" } else { "BPS" }
}

/// The Clock's text: "14:05" or "2:05 PM" (with seconds when the Clock shows them).
fn clock_time(seconds: Option<u32>, settings: &Settings) -> String {
    let Some(total) = seconds else { return "--:--".to_owned() };
    let (h, m, s) = (total / 3600 % 24, total / 60 % 60, total % 60);
    let (shown, suffix) = if settings.clock_12h { (if h % 12 == 0 { 12 } else { h % 12 }, if h < 12 { " AM" } else { " PM" }) } else { (h, "") };
    if settings.clock_seconds {
        format!("{shown}:{m:02}:{s:02}{suffix}")
    } else {
        format!("{shown}:{m:02}{suffix}")
    }
}

fn count(n: u32) -> String {
    let mut text = String::new();
    push_u(&mut text, u64::from(n));
    text
}

/// A value with its label the way the element's display options ask: "144 FPS", "FPS 144"
/// (label first), and either of them in brackets. Without labels only the value.
fn decorate(style: &Style, label: &str, value: &str) -> String {
    let mut text = String::with_capacity(label.len() + value.len() + 3);
    if style.brackets {
        text.push('[');
    }
    if style.labels && style.reverse {
        text.push_str(label);
        text.push(' ');
        text.push_str(value);
    } else {
        text.push_str(value);
        if style.labels {
            text.push(' ');
            text.push_str(label);
        }
    }
    if style.brackets {
        text.push(']');
    }
    text
}

/// Lunar's Reach Display: the distance of your last hit, "3.12 blocks" and "0 blocks" when there is
/// none. Reverse Order (the Label first switch) writes it "Blocks: 3.12".
fn reach_text(style: &Style, hundredths: u32) -> String {
    let value = if hundredths == 0 { "0".to_owned() } else { fixed(hundredths, 2) };
    let text = if !style.labels {
        value
    } else if style.reverse {
        format!("Blocks: {value}")
    } else {
        format!("{value} blocks")
    };
    bracketed(style, text)
}

/// The binding that shows or hides Reach display's box (see `Views::reach_hidden`).
const REACH_VISIBLE: &str = "#reach_vis";
/// The same for the Scoreboard (hidden while the server has no sidebar).
const SIDEBAR_VISIBLE: &str = "#sb_vis";
/// The binding that shows or hides the Target HUD's box.
const TARGET_VISIBLE: &str = "#tg_vis";
/// The same for WAILA (hidden while the crosshair is on no block).
const WAILA_VISIBLE: &str = "#wl_vis";
const SUBTITLES_VISIBLE: &str = "#as_vis";
/// The same for View angles (hidden in the views and while the elytra is not out that its options ask for).
const VA_VISIBLE: &str = "#va_vis";

/// Whether a text element's colour is a binding (it changes with the value).
fn colour_bound(element: Element, style: &Style) -> bool {
    element == Element::Waila || element == Element::ViewAngles || element == Element::PvpInfo || element == Element::Coords || (style.smart && matches!(element, Element::Fps | Element::Ping | Element::Slime | Element::Monument | Element::Armor))
}

/// `colour_bound`, or the text is Chroma's (the colour then follows the clock).
fn text_bound(element: Element, style: &Style, settings: &Settings) -> bool {
    colour_bound(element, style) || settings.gameplay.chroma(element.index()).on
}

/// The text colour of line (or key) `step` of `element` at `view`'s time, when its chroma is on.
fn chroma_rgba(element: Element, view: &Views, settings: &Settings, step: usize, alpha: f32) -> Option<[f32; 4]> {
    let chroma = settings.gameplay.chroma(element.index());
    chroma.on.then(|| {
        let [r, g, b] = crate::outline::chroma_color(view.chroma, chroma.speed, chroma.wave, step);
        [r, g, b, alpha]
    })
}

/// The CPS statistics after the count, " (avg 6.2, max 12, 340 clicks)" with the ones switched on; the gap
/// before it is one space, so the bar between the two counts stays where `cps_bar_offset` puts it.
fn cps_stats_text(stats: [u32; 3], settings: &Settings) -> String {
    let g = &settings.gameplay;
    let mut parts: Vec<String> = Vec::new();
    if g.cps_avg {
        parts.push(format!("avg {}.{}", stats[0] / 10, stats[0] % 10));
    }
    if g.cps_max {
        parts.push(format!("max {}", stats[1]));
    }
    if g.cps_total {
        parts.push(format!("{} clicks", stats[2]));
    }
    if parts.is_empty() { String::new() } else { format!(" ({})", parts.join(", ")) }
}

/// A health value the way the Target HUD writes it: whole numbers plain, otherwise one decimal.
fn health_text(value: f32) -> String {
    let tenths = (value.max(0.0) * 10.0).round() as u32;
    if tenths % 10 == 0 { count(tenths / 10) } else { format!("{}.{}", tenths / 10, tenths % 10) }
}

/// What the Target HUD calls an actor: its name, or for a mob without one the identifier made readable
/// ("minecraft:zombie_villager" becomes "Zombie villager").
pub fn target_name(name: &str, identifier: &str) -> String {
    let name: String = name.chars().filter(|c| !c.is_control()).collect();
    let name = name.trim();
    if !name.is_empty() {
        return name.to_owned();
    }
    let short = identifier.rsplit(':').next().unwrap_or("").replace('_', " ");
    let mut chars = short.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The Target HUD's two lines: the name, then the health ("14 / 20 HP", or "14 HP" when the server gave no maximum).
/// With nobody targeted (only seen while the elements are moved) it shows a sample.
fn target_lines(view: &Views, style: &Style, plain: [f32; 4]) -> Vec<Line> {
    let sample = TargetView { name: "Steve".to_owned(), health: Some(20.0), max_health: Some(20.0) };
    let target = view.target.as_ref().unwrap_or(&sample);
    let mut name: String = target.name.chars().filter(|c| !c.is_control()).take(24).collect();
    if name.is_empty() {
        name.push('?');
    }
    let value = match (target.health, target.max_health) {
        (Some(health), Some(max)) if max > 0.0 => format!("{} / {}", health_text(health), health_text(max)),
        (Some(health), _) => health_text(health),
        _ => "--".to_owned(),
    };
    vec![line(bracketed(style, name), plain), line(decorate(style, "HP", &value), plain)]
}

/// WAILA's lines (see `waila_rows`), each in its tone's colour. While the elements are being placed, or nothing is
/// aimed at, a sample block stands in so the box has its real size.
fn waila_lines(view: &Views, settings: &Settings, style: &Style, color: [f32; 4]) -> Vec<Line> {
    let sample = crate::waila::sample();
    let block = view.waila.as_ref().unwrap_or(&sample);
    waila_rows(block, settings)
        .into_iter()
        .enumerate()
        .map(|(n, (row, tone))| line(if n == 0 { bracketed(style, row) } else { row }, waila_color(tone, color, settings)))
        .collect()
}

/// The Scoreboard's lines: the sidebar's title, then a line per row ("label  score", or just the label while the scores are off).
fn scoreboard_lines(view: &Views, scores: bool, color: [f32; 4]) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    if let Some(board) = &view.sidebar {
        if !board.title.is_empty() {
            out.push(line(board.title.clone(), color));
        }
        for (label, score) in &board.rows {
            out.push(line(if scores && *score != NO_SCORE { format!("{label}  {score}") } else { label.clone() }, color));
        }
    }
    if out.is_empty() {
        out.push(line(String::new(), color));
    }
    out
}

/// The lines of a text element, top to bottom.
pub fn lines(element: Element, view: &Views, settings: &Settings) -> Vec<Line> {
    let style = settings.style(element);
    let labels = style.labels;
    let plain = rgba(style.text_color, 1.0);
    let [x, y, z] = view.feet;
    match element {
        Element::Keystrokes | Element::Potion | Element::Arrows => Vec::new(),
        Element::ArrayList => {
            let names = array_names(settings);
            if names.is_empty() {
                return vec![line(String::new(), plain)];
            }
            names.into_iter().map(|name| line(bracketed(style, name.to_owned()), plain)).collect()
        }
        Element::ItemCounter => {
            // "12 Golden Apples" (label first: "Golden Apples 12"); the item is picked on the Style page.
            let (_, one, many) = crate::config::COUNTER_ITEMS[settings.counter_item.min(crate::config::COUNTER_ITEMS.len() - 1)];
            let label = if view.item_count == 1 { one } else { many };
            vec![line(decorate(style, label, &count(view.item_count)), plain)]
        }
        // One line per slot, holding the stack's count (nothing for a single item or an empty slot).
        Element::Inventory => (0..INV_SLOTS)
            .map(|n| {
                let stack = view.inventory.get(n).map_or(0, |piece| u32::from(piece.count));
                line(if stack > 1 { count(stack) } else { String::new() }, plain)
            })
            .collect(),
        Element::Armor => armor_slots(style)
            .into_iter()
            .map(|slot| &view.armor[slot])
            .map(|piece| {
                if piece.item.is_empty() || style.armor_text == 3 {
                    return line(String::new(), plain);
                }
                let Some((left, max)) = piece.durability else {
                    // Something that does not wear out: a stack shows how many there are.
                    let text = if piece.count > 1 { count(u32::from(piece.count)) } else { String::new() };
                    return line(text, plain);
                };
                let color = if !style.smart {
                    plain
                } else if left * 5 <= max {
                    // Red at 20% or below, like Cinnabar Client.
                    rgba(style.armor_low, 1.0)
                } else if left * 2 <= max {
                    rgba(style.armor_mid, 1.0)
                } else {
                    rgba(style.armor_good, 1.0)
                };
                let text = match style.armor_text {
                    1 => {
                        let mut t = count(if max == 0 { 0 } else { (u64::from(left) * 100 / u64::from(max)) as u32 });
                        t.push('%');
                        t
                    }
                    2 => {
                        let mut t = count(left);
                        t.push('/');
                        t.push_str(&count(max));
                        t
                    }
                    _ => count(left),
                };
                line(text, color)
            })
            .collect(),
        Element::Ping => {
            // Lunar's Ping: "42 ms"; with status colours green under 80 ms, yellow under 150, red above.
            let (value, color) = match view.ping {
                Some(ms) => {
                    let color = if !style.smart {
                        plain
                    } else if ms < 80 {
                        GREEN
                    } else if ms < 150 {
                        YELLOW
                    } else {
                        RED
                    };
                    (count(ms), color)
                }
                None => ("--".to_owned(), if style.smart { GRAY } else { plain }),
            };
            vec![line(decorate(style, "ms", &value), color)]
        }
        Element::Speed => {
            // Blocks per second ("5.6 BPS") or kilometres per hour ("20.2 km/h"), with the decimals
            // the player asked for. `view.speed` is already in that unit, times ten to the decimals.
            let value = match view.speed {
                Some(t) => fixed(t, settings.speed_decimals),
                None => "--".to_owned(),
            };
            vec![line(decorate(style, speed_unit_label(settings), &value), plain)]
        }
        Element::Stopwatch => vec![line(bracketed(style, clock_text(view.stopwatch, ClockStyle::Auto)), plain)],
        Element::Playtime => {
            let clock = if settings.playtime_format == 1 { ClockStyle::Hours } else { ClockStyle::Minutes };
            vec![line(bracketed(style, clock_text(view.playtime, clock)), plain)]
        }
        Element::Reach => {
            vec![line(reach_text(style, view.reach.unwrap_or(0)), plain)]
        }
        Element::Combo => {
            // "3 Combo", and "No Combo" while there is none (just "0" without labels).
            let text = if view.combo > 0 {
                decorate(style, "Combo", &count(view.combo))
            } else if style.labels {
                bracketed(style, "No Combo".to_owned())
            } else {
                bracketed(style, "0".to_owned())
            };
            vec![line(text, plain)]
        }
        Element::Food => {
            // Saturation, like Lunar's Saturation HUD: "4.5" ("Saturation: 4.5" with labels). A whole
            // number has no decimal ("0", not "0.0"), as there.
            let value = match view.food {
                Some([_, saturation]) => {
                    let text = fixed(saturation, 1);
                    text.strip_suffix(".0").map_or(text.clone(), str::to_owned)
                }
                None => "--".to_owned(),
            };
            vec![line(labelled(labels, "Saturation", &value), plain)]
        }
        Element::Day => {
            let value = view.day.map_or_else(|| "--".to_owned(), count);
            vec![line(labelled(labels, "Day", &value), plain)]
        }
        Element::Clock => vec![line(bracketed(style, clock_time(view.clock, settings)), plain)],
        Element::ViewAngles => va_lines(view, settings),
        Element::Scoreboard => scoreboard_lines(view, settings.sb_scores, plain),
        Element::TargetHud => target_lines(view, style, plain),
        Element::Waila => waila_lines(view, settings, style, plain),
        Element::HeightLimit => {
            let value = view.height_limit.map_or_else(|| "--".to_owned(), |blocks| blocks.to_string());
            vec![line(bracketed(style, labelled(labels, "Height limit", &value)), plain)]
        }
        Element::AudioSubtitles => {
            if view.subtitles.is_empty() {
                return vec![line(bracketed(style, "No sounds".to_owned()), plain)];
            }
            view.subtitles.iter().map(|row| line(bracketed(style, row.line()), plain)).collect()
        }
        Element::Waypoints => {
            if view.waypoints.is_empty() {
                return vec![line(bracketed(style, "No waypoints".to_owned()), plain)];
            }
            view.waypoints
                .iter()
                .take(crate::waypoints::SHOWN)
                .map(|row| line(bracketed(style, format!("{} {}m {}", row.name, row.distance, row.direction)), plain))
                .collect()
        }
        Element::PvpInfo => {
            let [hits, accuracy, kills, deaths] = view.pvp;
            let (num, pct) = (rgba(settings.pvp_num_tc, 1.0), rgba(settings.pvp_pct_tc, 1.0));
            let row = |value: String, color: [f32; 4], name: &str, label: usize| Line { text: value, color, tail: Some((name.to_owned(), rgba(label, 1.0))), head: None };
            vec![
                row(count(hits), num, "Hits", settings.pvp_hits_lc),
                row(format!("{accuracy}%"), pct, "Accuracy", settings.pvp_acc_lc),
                row(count(kills), num, "Kills", settings.pvp_kills_lc),
                row(count(deaths), num, "Deaths", settings.pvp_deaths_lc),
            ]
        }
        Element::TotemCounter => {
            let label = if view.totems == 1 { "Totem pop" } else { "Totem pops" };
            vec![line(decorate(style, label, &count(view.totems)), plain)]
        }
        Element::PotionCounter => {
            let label = if view.potion_count == 1 { "Potion" } else { "Potions" };
            vec![line(decorate(style, label, &count(view.potion_count)), plain)]
        }
        Element::Cps => {
            let [left, right] = view.cps;
            let text = match settings.cps_mode {
                crate::config::CpsMode::Both => {
                    let mut t = count(left);
                    // A plain gap, not a `|`: some texture packs have no such glyph and others do, and the
                    // bar between the counts is drawn by the mod (`cps_bar_offset`), so no font can double it.
                    t.push_str(CPS_GAP);
                    t.push_str(&count(right));
                    t
                }
                crate::config::CpsMode::Left => count(left),
                crate::config::CpsMode::Right => count(right),
                crate::config::CpsMode::Total => count(left + right),
            };
            // Like Lunar's CPS: "7 CPS", or "7 | 3 CPS" with the right button (the bar is drawn).
            let mut shown = decorate(style, "CPS", &text);
            shown.push_str(&cps_stats_text(view.cps_stats, settings));
            vec![line(shown, plain)]
        }
        Element::Fps => {
            let (value, color) = match view.fps {
                Some(fps) => {
                    let color = if !style.smart {
                        plain
                    } else if fps >= 60 {
                        GREEN
                    } else if fps >= 30 {
                        YELLOW
                    } else {
                        RED
                    };
                    (count(fps), color)
                }
                None => ("--".to_owned(), if style.smart { GRAY } else { plain }),
            };
            // Like Lunar's FPS: "144 FPS", "[144 FPS]" with brackets, "FPS 144" label first.
            vec![line(decorate(style, "FPS", &value), color)]
        }
        Element::Coords => coords_lines(view, settings),
        Element::Facing => {
            let value = view.facing.map_or("--", |f| FACING[usize::from(f & 3)]);
            vec![line(labelled(labels, "Facing", value), plain)]
        }
        Element::Chunk => {
            let section = y.div_euclid(16);
            let mut sub = String::new();
            push_i(&mut sub, i64::from(section));
            if style.sub_range {
                sub.push_str(" (Y ");
                push_i(&mut sub, i64::from(section * 16));
                sub.push_str(" to ");
                push_i(&mut sub, i64::from(section * 16 + 15));
                sub.push(')');
            }
            let mut all = Vec::new();
            if style.show_chunk {
                all.push(line(labelled(labels, "Chunk", &pair(x.div_euclid(16), z.div_euclid(16))), plain));
            }
            if style.show_inchunk {
                all.push(line(labelled(labels, "In chunk", &pair(x.rem_euclid(16), z.rem_euclid(16))), plain));
            }
            if style.show_sub {
                all.push(line(labelled(labels, "Subchunk", &sub), plain));
            }
            if all.is_empty() {
                all.push(line(labelled(labels, "Chunk", &pair(x.div_euclid(16), z.div_euclid(16))), plain));
            }
            all
        }
        Element::Slime => {
            // Only the active state (YES) takes the active colour; the rest use the text colour.
            let active = if style.smart { rgba(settings.active_color(element), 1.0) } else { plain };
            let (value, color) = match (view.overworld, view.slime) {
                (false, _) => ("n/a", plain),
                (true, Some(true)) => ("YES", active),
                (true, Some(false)) => ("No", plain),
                (true, None) => ("...", plain),
            };
            vec![line(labelled(labels, "Slime chunk:", value), color)]
        }
        Element::Monument => {
            let on_color = if style.smart { rgba(settings.active_color(element), 1.0) } else { plain };
            let (value, color) = match view.guardians {
                None | Some(Guardians::Unavailable) => ("none here".to_owned(), plain),
                Some(Guardians::Searching) => ("searching...".to_owned(), plain),
                Some(Guardians::Found { active, blocked, checked }) => {
                    let mut text = count(u32::from(active));
                    text.push_str(" on");
                    if checked {
                        text.push_str(" / ");
                        text.push_str(&count(u32::from(blocked)));
                        text.push_str(" off");
                    }
                    (text, on_color)
                }
            };
            vec![line(labelled(labels, "Guardians:", &value), color)]
        }
    }
}

// ---- The document ----------------------------------------------------------------------

fn signed(o: &mut String, v: f32) {
    if v < 0.0 {
        o.push('-');
    }
    num(o, f64::from(v.abs()));
}

/// `#e<element>_<kind><n>`: the binding of a text element's line (`t` text, `c` colour).
fn binding(element: Element, kind: char, n: usize) -> String {
    let mut name = String::from("#e");
    push_u(&mut name, element.index() as u64);
    name.push('_');
    name.push(kind);
    push_u(&mut name, n as u64);
    name
}

/// `#k<slot>_<suffix>`: a key's binding.
fn key_binding(slot: usize, suffix: &str) -> String {
    let mut name = String::from("#k");
    push_u(&mut name, slot as u64);
    name.push_str(suffix);
    name
}

/// Opens an element's panel: `{"e<i>":{"type":"panel",...,"controls":[`.
fn open_panel(o: &mut String, element: Element, settings: &Settings, viewport: Option<[f32; 2]>) {
    let style = settings.style(element);
    let [w, h] = size(settings, element);
    // Potion status with its effects moved apart: the panel is made bigger on every side so an effect
    // moved out of the list's own area is not cut off by it; the effects are shifted by the same room.
    let room = potion_room(settings, element);
    // With the screen's size known the panel is the whole screen and the effects are placed on it, so
    // nothing the editor moves is ever outside it (the host cuts off what lies beyond a panel).
    let full = if room > 0.0 { viewport } else { None };
    o.push_str(r##"{\"e"##);
    push_u(o, element.index() as u64);
    o.push_str(r##"\":{\"type\":\"panel\",\"size\":["##);
    let [pw, ph] = full.unwrap_or([w + room * 2.0, h + room * 2.0]);
    num(o, f64::from(pw));
    o.push(',');
    num(o, f64::from(ph));
    o.push(']');
    let (anchor, mut offset) = match (style.position, viewport) {
        _ if full.is_some() => ("top_left", [0.0, 0.0]),
        (Some(at), Some(view)) => ("top_left", placed_offset([w, h], view, at, bleed(settings, element))),
        _ => {
            let corner = if style.position.is_some() { Corner::TopLeft } else { style.corner };
            let a = corner.anchor();
            (a, [if a.ends_with("right") { -MARGIN } else { MARGIN }, if a.starts_with("bottom") { -MARGIN } else { MARGIN }])
        }
    };
    if room > 0.0 && full.is_none() {
        offset[0] += if anchor.ends_with("right") { room } else { -room };
        offset[1] += if anchor.starts_with("bottom") { room } else { -room };
    }
    o.push_str(r##",\"anchor_from\":\""##);
    o.push_str(anchor);
    o.push_str(r##"\",\"anchor_to\":\""##);
    o.push_str(anchor);
    o.push_str(r##"\",\"offset\":["##);
    signed(o, offset[0]);
    o.push(',');
    signed(o, offset[1]);
    o.push(']');
    // Reach display can be hidden while it reads 0: its panel follows a binding for that.
    if matches!(element, Element::Reach | Element::Scoreboard | Element::TargetHud | Element::Waila | Element::AudioSubtitles | Element::ViewAngles) {
        o.push_str(r##",\"bindings\":[{\"binding_name\":\""##);
        o.push_str(match element {
            Element::Reach => REACH_VISIBLE,
            Element::Scoreboard => SIDEBAR_VISIBLE,
            Element::Waila => WAILA_VISIBLE,
            Element::AudioSubtitles => SUBTITLES_VISIBLE,
            Element::ViewAngles => VA_VISIBLE,
            _ => TARGET_VISIBLE,
        });
        o.push_str(r##"\",\"binding_name_override\":\"#visible\"}]"##);
    }
    o.push_str(r##",\"controls\":["##);
}

/// How much bigger Potion status's panel is on each side while its effects are moved apart (device
/// pixels): enough for the furthest an effect can go.
fn potion_room(settings: &Settings, element: Element) -> f32 {
    if element == Element::Potion && settings.potion_split {
        (crate::config::MAX_POTION_OFFSET + 200.0) * settings.style(element).scale as f32
    } else {
        0.0
    }
}

fn close_panel(o: &mut String) {
    o.push_str("]}}");
}

/// One label in the escaped document. `text` is the text, or the binding that carries it when
/// `text_bound`; `color_binding` names a binding that carries the colour.
#[allow(clippy::too_many_arguments)]
fn label(
    o: &mut String,
    name: &str,
    text: &str,
    text_bound: bool,
    color: [f32; 4],
    color_binding: Option<&str>,
    scale: f32,
    size: [f32; 2],
    offset: [f32; 2],
    align: &str,
    shadow: bool,
) {
    o.push_str(r##",{\""##);
    o.push_str(name);
    o.push_str(r##"\":{\"type\":\"label\",\"text\":\""##);
    o.push_str(text);
    o.push_str(r##"\",\"localize\":false,\"shadow\":"##);
    o.push_str(if shadow { "true" } else { "false" });
    // The host takes a label's RGB from its colour and its opacity from the control's own `alpha` (shadow included), so the
    // colour's opacity goes there.
    if color[3] < 0.999 {
        o.push_str(r##",\"alpha\":"##);
        num(o, f64::from(color[3]));
    }
    o.push_str(r##",\"color\":"##);
    write_rgba(o, color);
    o.push_str(r##",\"text_alignment\":\""##);
    o.push_str(align);
    o.push_str(r##"\",\"font_scale_factor\":"##);
    num(o, f64::from(scale));
    o.push_str(r##",\"size\":["##);
    num(o, f64::from(size[0]));
    o.push(',');
    num(o, f64::from(size[1]));
    o.push_str(r##"],\"offset\":["##);
    num(o, f64::from(offset[0]));
    o.push(',');
    num(o, f64::from(offset[1]));
    o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\""##);
    if text_bound || color_binding.is_some() {
        o.push_str(r##",\"bindings\":["##);
        if text_bound {
            o.push_str(r##"{\"binding_name\":\""##);
            o.push_str(text);
            o.push_str(r##"\"}"##);
        }
        if let Some(binding) = color_binding {
            if text_bound {
                o.push(',');
            }
            o.push_str(r##"{\"binding_name\":\""##);
            o.push_str(binding);
            o.push_str(r##"\",\"binding_name_override\":\"#color\"}"##);
        }
        o.push(']');
    }
    o.push_str("}}");
}

// ---- Potion status ---------------------------------------------------------------------

/// What the Potion status list was laid out for: its cells, how they sit in the grid, and the
/// sizes that follow, in GUI pixels at size 1.
#[derive(Clone, Debug, Default, PartialEq)]
struct PotionLayout {
    count: usize,
    columns: usize,
    rows: usize,
    minimal: bool,
    icons: bool,
    show_title: bool,
    show_time: bool,
    /// The content of one cell, and the room around it in a box (Minimal: each tile's box).
    cell: [f32; 2],
    pad: [f32; 2],
    gap: f32,
    /// How wide the text column is (Normal) or the time (Minimal).
    text_w: f32,
    /// The whole element.
    size: [f32; 2],
    textures: Vec<String>,
    /// Every effect in a box of its own, placed by `offsets` (the game's effect ids in `ids`).
    split: bool,
    ids: Vec<i32>,
    offsets: Vec<[f32; 2]>,
    scales: Vec<f32>,
}

impl PotionLayout {
    /// Whether each effect has a box (tile) of its own.
    fn tiles(&self) -> bool {
        self.minimal || self.split
    }

    /// One tile's size: its content and the room around it.
    fn tile(&self) -> [f32; 2] {
        [self.cell[0] + self.pad[0] * 2.0, self.cell[1] + self.pad[1] * 2.0]
    }

    /// How much tile `i` is drawn bigger or smaller than the list has it.
    fn scale(&self, i: usize) -> f32 {
        if self.split { self.scales.get(i).copied().unwrap_or(1.0) } else { 1.0 }
    }

    /// Where tile `i` starts, in GUI pixels at size 1 from the element's corner (moved, when split).
    fn slot(&self, i: usize) -> [f32; 2] {
        let (c, r) = potion::cell_at(i, self.columns);
        let tile = self.tile();
        let at = [c as f32 * (tile[0] + self.gap), r as f32 * (tile[1] + self.gap)];
        let by = if self.split { self.offsets.get(i).copied().unwrap_or([0.0; 2]) } else { [0.0; 2] };
        [at[0] + by[0], at[1] + by[1]]
    }
}

thread_local! {
    static POTION: RefCell<PotionLayout> = RefCell::new(PotionLayout { size: [1.0, 1.0], ..PotionLayout::default() });
}

/// What Potion status writes, from the settings.
fn potion_text(settings: &Settings) -> potion::Text {
    potion::Text {
        name: settings.potion_name,
        level: settings.potion_level,
        duration: settings.potion_duration,
        formatted: settings.potion_formatted,
        upper: settings.potion_upper,
    }
}

/// A time as wide as the widest one the row can show, so the box does not change as it runs down.
fn potion_time_reference(row: &potion::Row, formatted: bool) -> &'static str {
    match (row.seconds, formatted) {
        (None, _) => "**:**",
        (Some(s), false) if s >= 3600 => "0:00:00",
        (Some(_), false) => "00:00",
        (Some(s), true) if s >= 3600 => "00h 00m",
        (Some(_), true) => "00m 00s",
    }
}

fn potion_layout(rows: &[potion::Row], settings: &Settings) -> PotionLayout {
    let style = settings.style(Element::Potion);
    let text = potion_text(settings);
    let minimal = settings.potion_mode == 1;
    let count = rows.len().min(potion::MAX_ROWS);
    let rows = &rows[..count];
    let show_title = (text.name || text.level) && !minimal;
    let show_time = text.duration;
    let icons = settings.potion_icons && !ICONS_OFF.with(Cell::get);
    let title_w = if show_title { rows.iter().map(|r| text_width(&potion::title(r, text))).fold(0.0f32, f32::max) } else { 0.0 };
    let time_w = if show_time { rows.iter().map(|r| text_width(potion_time_reference(r, text.formatted))).fold(0.0f32, f32::max) } else { 0.0 };
    let text_w = title_w.max(time_w);
    let lines = usize::from(show_title) + usize::from(show_time);
    let text_h = if lines == 0 { 0.0 } else { lines as f32 * potion::LINE + (lines - 1) as f32 };
    let icon = if icons { potion::ICON } else { 0.0 };
    let cell = if minimal {
        let gap_under_icon = if icons { 1.0 } else { 0.0 };
        [icon.max(time_w), icon + if show_time { gap_under_icon + potion::LINE } else { 0.0 }]
    } else {
        [if icons { potion::ICON + if lines > 0 { potion::NEAR } else { 0.0 } } else { 0.0 } + text_w, icon.max(text_h)]
    };
    let cell = [cell[0].max(1.0), cell[1].max(1.0)];
    let pad = [pad_x(style), pad_y(style)];
    let split = settings.potion_split;
    let gap = if minimal || split { potion::GAP } else { 3.0 };
    let (columns, grid_rows) = potion::grid(count, settings.potion_horizontal, settings.potion_per_line);
    let size = if count == 0 {
        [1.0, 1.0]
    } else if minimal || split {
        potion::extent(columns, grid_rows, [cell[0] + pad[0] * 2.0, cell[1] + pad[1] * 2.0], gap)
    } else {
        let inner = potion::extent(columns, grid_rows, cell, gap);
        [inner[0] + pad[0] * 2.0, inner[1] + pad[1] * 2.0]
    };
    PotionLayout {
        count,
        columns,
        rows: grid_rows,
        minimal,
        icons,
        show_title,
        show_time,
        cell,
        pad,
        gap,
        text_w,
        size,
        textures: rows.iter().map(|r| r.texture.clone()).collect(),
        split,
        ids: rows.iter().map(|r| r.id).collect(),
        offsets: rows
            .iter()
            .map(|r| usize::try_from(r.id).ok().and_then(|i| settings.potion_offsets.get(i).copied()).unwrap_or([0.0; 2]))
            .collect(),
        scales: rows
            .iter()
            .map(|r| usize::try_from(r.id).ok().and_then(|i| settings.potion_scales.get(i).copied()).unwrap_or(1.0))
            .collect(),
    }
}

thread_local! {
    /// Set when the host refused a document with the effect icons in it: the list is text only then.
    static ICONS_OFF: Cell<bool> = const { Cell::new(false) };
}

/// Takes the effect icons out of Potion status for good (this run). Returns whether that is new.
pub fn disable_potion_icons() -> bool {
    ICONS_OFF.with(|cell| !cell.replace(true))
}

/// Lays Potion status out for `rows`. Returns whether the layout changed (the document must then
/// be written again).
pub fn set_potion(rows: &[potion::Row], settings: &Settings) -> bool {
    let layout = potion_layout(rows, settings);
    POTION.with(|cell| {
        let mut now = cell.borrow_mut();
        let changed = *now != layout;
        if changed {
            *now = layout;
        }
        changed
    })
}

/// What Potion status's document is written from: its layout and what it shows besides the style.
fn potion_signature(_settings: &Settings) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    POTION.with(|cell| {
        let layout = cell.borrow();
        (layout.count, layout.columns, layout.rows, layout.minimal, layout.icons, layout.show_title, layout.show_time).hash(&mut hasher);
        for v in layout.cell.iter().chain(layout.pad.iter()).chain(layout.size.iter()).chain([layout.gap, layout.text_w].iter()) {
            v.to_bits().hash(&mut hasher);
        }
        layout.textures.hash(&mut hasher);
        layout.split.hash(&mut hasher);
        layout.ids.hash(&mut hasher);
        for v in layout.scales.iter() {
            v.to_bits().hash(&mut hasher);
        }
        for v in layout.offsets.iter().flatten() {
            v.to_bits().hash(&mut hasher);
        }
    });
    hasher.finish()
}

/// `#p<kind><row>`: Potion status's per-row bindings (`t` title, `d` time, `c` title colour, `e` time
/// colour, `i` icon opacity).
fn potion_binding(kind: char, row: usize) -> String {
    let mut name = String::from("#p");
    name.push(kind);
    push_u(&mut name, row as u64);
    name
}

/// A texture path that is safe to write into the document (a plain game path).
fn potion_texture_ok(texture: &str) -> bool {
    !texture.is_empty() && texture.len() <= 96 && texture.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'/' | b'-' | b'.'))
}

/// One box of Potion status: the element's rounded fill and border, at `size` from `at`.
fn potion_box(o: &mut String, name: &str, settings: &Settings, k: f32, at: [f32; 2], size: [f32; 2]) {
    let style = settings.style(Element::Potion);
    o.push_str(r##",{\""##);
    o.push_str(name);
    o.push_str(r##"\":{\"type\":\"custom\",\"renderer\":\"cinnabar_rounded_rectangle\",\"radius\":"##);
    num(o, if style.rounded { f64::from(3.0 * k) } else { 0.0 });
    o.push_str(r##",\"color\":"##);
    write_rgba(o, rgba(style.bg_color, if style.background { style.opacity / 100.0 } else { 0.0 }));
    if style.border && Settings::has_border(Element::Potion) {
        o.push_str(r##",\"border_width\":"##);
        num(o, f64::from(style.border_width as f32 * k));
        o.push_str(r##",\"border_color\":"##);
        write_rgba(o, rgba(style.border_color, style.border_opacity / 100.0));
    }
    o.push_str(r##",\"size\":["##);
    num(o, f64::from(size[0] * k));
    o.push(',');
    num(o, f64::from(size[1] * k));
    o.push_str(r##"],\"offset\":["##);
    num(o, f64::from(at[0] * k));
    o.push(',');
    num(o, f64::from(at[1] * k));
    o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\"}}"##);
}

/// Potion status: the game's effect icons with their names and times, in a box (Normal) or in a
/// tile each (Minimal), in a column or in lines (Horizontal).
#[inline(never)]
fn potion_element(o: &mut String, settings: &Settings, viewport: Option<[f32; 2]>) {
    let style = settings.style(Element::Potion);
    let k = style.scale as f32;
    let layout = POTION.with(|cell| cell.borrow().clone());
    open_panel(o, Element::Potion, settings, viewport);
    let room = potion_room(settings, Element::Potion);
    // Where the effects' own area starts inside the panel: the screen's corner while the panel is the
    // whole screen, else the room around the list.
    let shift = match (room > 0.0, viewport) {
        (true, Some(view)) => top_left(settings, Element::Potion, view),
        (true, None) => [room, room],
        _ => [0.0, 0.0],
    };
    o.push_str(r##"{\"_\":{\"type\":\"panel\",\"size\":[0,0]}}"##);
    if layout.count > 0 && !layout.tiles() {
        potion_box(o, "bg", settings, k, [0.0, 0.0], layout.size);
    }
    let [cw, ch] = layout.cell;
    let [px, py] = layout.pad;
    let icon = if layout.icons { potion::ICON } else { 0.0 };
    for (i, texture) in layout.textures.iter().enumerate() {
        let (c, r) = potion::cell_at(i, layout.columns);
        // Where the cell's content starts: Minimal (and moved-apart effects) gives every cell a box of its own.
        let s = layout.scale(i);
        let ki = k * s;
        let (base, origin) = if layout.tiles() {
            let tile = layout.tile();
            let at = layout.slot(i);
            let mut name = String::from("bg");
            push_u(&mut name, i as u64);
            potion_box(o, &name, settings, k, [at[0] + shift[0] / k, at[1] + shift[1] / k], [tile[0] * s, tile[1] * s]);
            (at, [px, py])
        } else {
            ([0.0, 0.0], [px + c as f32 * (cw + layout.gap), py + r as f32 * (ch + layout.gap)])
        };
        let time = potion_binding('d', i);
        let time_color = potion_binding('e', i);
        let title = potion_binding('t', i);
        let title_color = potion_binding('c', i);
        let pos = |x: f32, y: f32| [base[0] * k + x * ki + shift[0], base[1] * k + y * ki + shift[1]];
        let push_icon = |o: &mut String, x: f32, y: f32| {
            if layout.icons && potion_texture_ok(texture) {
                o.push_str(r##",{\"pi"##);
                push_u(o, i as u64);
                o.push_str(r##"\":{\"type\":\"image\",\"texture\":\""##);
                o.push_str(texture);
                o.push_str(r##"\",\"size\":["##);
                num(o, f64::from(potion::ICON * ki));
                o.push(',');
                num(o, f64::from(potion::ICON * ki));
                o.push_str(r##"],\"offset\":["##);
                let at = pos(x, y);
                num(o, f64::from(at[0]));
                o.push(',');
                num(o, f64::from(at[1]));
                o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"bindings\":[{\"binding_name\":\""##);
                o.push_str(&potion_binding('i', i));
                o.push_str(r##"\",\"binding_name_override\":\"#alpha\"}]}}"##);
            }
        };
        let text = rgba(style.text_color, 1.0);
        if layout.minimal {
            push_icon(o, origin[0] + (cw - icon) / 2.0, origin[1]);
            if layout.show_time {
                let name = format!("pd{i}");
                label(o, &name, &time, true, text, Some(time_color.as_str()), ki * FONT, [cw * ki, potion::LINE * ki], pos(origin[0], origin[1] + if layout.icons { icon + 1.0 } else { 0.0 }), "center", style.shadow);
            }
        } else {
            push_icon(o, origin[0], origin[1] + (ch - icon) / 2.0);
            let lines = usize::from(layout.show_title) + usize::from(layout.show_time);
            let block = if lines == 0 { 0.0 } else { lines as f32 * potion::LINE + (lines - 1) as f32 };
            let x = origin[0] + if layout.icons { potion::ICON + potion::NEAR } else { 0.0 };
            let mut y = origin[1] + (ch - block) / 2.0;
            if layout.show_title {
                let name = format!("pt{i}");
                label(o, &name, &title, true, text, Some(title_color.as_str()), ki * FONT, [layout.text_w * ki, potion::LINE * ki], pos(x, y), "left", style.shadow);
                y += potion::LINE + 1.0;
            }
            if layout.show_time {
                let name = format!("pd{i}");
                label(o, &name, &time, true, text, Some(time_color.as_str()), ki * FONT, [layout.text_w * ki, potion::LINE * ki], pos(x, y), "left", style.shadow);
            }
        }
    }
    close_panel(o);
}

/// Potion status's bindings: each row's title, time, colours and icon opacity.
fn potion_values(o: &mut String, member: &mut impl FnMut(&mut String, &str), view: &Views, settings: &Settings) {
    let style = settings.style(Element::Potion);
    let text = potion_text(settings);
    let base = rgba(style.text_color, 1.0);
    for (i, row) in view.potions.iter().take(potion::MAX_ROWS).enumerate() {
        let alpha = potion::blink_alpha(row.seconds, settings.potion_blink, view.blink);
        let chroma = chroma_rgba(Element::Potion, view, settings, i, 1.0);
        let tint = |by_effect: bool| {
            let mut c = if let Some(rainbow) = chroma {
                rainbow
            } else if by_effect {
                let [r, g, b] = potion::effect_color(row.id);
                [r, g, b, 1.0]
            } else {
                base
            };
            c[3] *= alpha;
            c
        };
        member(o, &potion_binding('t', i));
        q(o, &potion::title(row, text));
        member(o, &potion_binding('d', i));
        q(o, &potion::duration(row.seconds, text.formatted));
        member(o, &potion_binding('c', i));
        write_rgba(o, tint(settings.potion_color_name));
        member(o, &potion_binding('e', i));
        write_rgba(o, tint(settings.potion_color_info));
        member(o, &potion_binding('i', i));
        num(o, f64::from(alpha));
    }
}

/// Where each effect of Potion status that has a box of its own is on screen: its effect id and
/// rectangle (x, y, width, height). Empty unless the effects are moved apart.
pub fn potion_pieces(settings: &Settings, viewport: [f32; 2]) -> Vec<(i32, [f32; 4])> {
    let layout = POTION.with(|cell| cell.borrow().clone());
    if !layout.split {
        return Vec::new();
    }
    let k = settings.style(Element::Potion).scale as f32;
    let [x, y] = top_left(settings, Element::Potion, viewport);
    let tile = layout.tile();
    layout.ids.iter().enumerate().map(|(i, id)| {
        let at = layout.slot(i);
        let sc = layout.scale(i);
        (*id, [x + at[0] * k, y + at[1] * k, tile[0] * k * sc, tile[1] * k * sc])
    }).collect()
}

/// The effect under a point when they are moved apart (the last drawn wins).
pub fn potion_piece_at(settings: &Settings, viewport: [f32; 2], point: [f32; 2]) -> Option<(i32, [f32; 4])> {
    potion_pieces(settings, viewport)
        .into_iter()
        .rev()
        .find(|(_, [x, y, w, h])| point[0] >= *x && point[0] <= x + w && point[1] >= *y && point[1] <= y + h)
}

/// A drag of one effect of Potion status: where the list starts, where the effect's own place in
/// the list is, and the guides it can snap to.
pub struct PotionDrag {
    id: i32,
    origin: [f32; 2],
    slot: [f32; 2],
    size: [f32; 2],
    k: f32,
    guides: [Vec<f32>; 2],
}

impl PotionDrag {
    pub fn new(settings: &Settings, viewport: [f32; 2], id: i32) -> Option<Self> {
        let layout = POTION.with(|cell| cell.borrow().clone());
        let i = layout.ids.iter().position(|x| *x == id)?;
        let k = settings.style(Element::Potion).scale as f32;
        let (mut gx, mut gy) = screen_guides(settings, viewport);
        if settings.align {
            for (other, [x, y, w, h]) in potion_pieces(settings, viewport) {
                if other != id {
                    gx.extend([x, x + w * 0.5, x + w]);
                    gy.extend([y, y + h * 0.5, y + h]);
                }
            }
            for other in Element::ALL {
                if other != Element::Potion && settings.is_on(other) {
                    let [x, y, w, h] = visible_rect(settings, other, viewport);
                    gx.extend([x, x + w * 0.5, x + w]);
                    gy.extend([y, y + h * 0.5, y + h]);
                }
            }
        }
        let tile = layout.tile();
        let mut slot = layout.slot(i);
        let by = layout.offsets.get(i).copied().unwrap_or([0.0; 2]);
        slot = [slot[0] - by[0], slot[1] - by[1]];
        let sc = layout.scale(i);
        Some(Self { id, origin: top_left(settings, Element::Potion, viewport), slot, size: [tile[0] * k * sc, tile[1] * k * sc], k, guides: [gx, gy] })
    }

    /// Moves the effect so its top-left corner follows `wanted`. Returns whether anything moved.
    pub fn step(&self, settings: &mut Settings, viewport: [f32; 2], wanted: [f32; 2], snap: bool) -> bool {
        let [w, h] = self.size;
        let mut to = wanted;
        if snap {
            to = [snap_axis(to[0], w, &self.guides[0], viewport[0] * 0.5), snap_axis(to[1], h, &self.guides[1], viewport[1] * 0.5)];
        }
        to = [to[0].clamp(0.0, (viewport[0] - w).max(0.0)), to[1].clamp(0.0, (viewport[1] - h).max(0.0))];
        let by = [
            ((to[0] - self.origin[0]) / self.k - self.slot[0]).round(),
            ((to[1] - self.origin[1]) / self.k - self.slot[1]).round(),
        ];
        let by = [by[0].clamp(-crate::config::MAX_POTION_OFFSET, crate::config::MAX_POTION_OFFSET), by[1].clamp(-crate::config::MAX_POTION_OFFSET, crate::config::MAX_POTION_OFFSET)];
        let Some(slot) = usize::try_from(self.id).ok().filter(|i| *i < settings.potion_offsets.len()) else { return false };
        if settings.potion_offsets[slot] == by {
            return false;
        }
        settings.potion_offsets[slot] = by;
        true
    }
}

/// A drag of a corner of one effect of Potion status: the effect is drawn bigger or smaller (the
/// same on both axes) and the opposite corner stays where it is.
pub struct PotionResize {
    id: i32,
    corner: usize,
    anchor: [f32; 2],
    grab: [f32; 2],
    rect: [f32; 4],
    scale: f32,
    origin: [f32; 2],
    slot: [f32; 2],
    k: f32,
}

impl PotionResize {
    pub fn new(settings: &Settings, viewport: [f32; 2], id: i32, corner: usize, pointer: [f32; 2]) -> Option<Self> {
        if corner > 3 {
            return None;
        }
        let layout = POTION.with(|cell| cell.borrow().clone());
        let i = layout.ids.iter().position(|x| *x == id)?;
        let rect = potion_pieces(settings, viewport).into_iter().find(|(x, _)| *x == id)?.1;
        let held = corners(rect)[corner];
        let by = layout.offsets.get(i).copied().unwrap_or([0.0; 2]);
        let at = layout.slot(i);
        Some(Self {
            id,
            corner,
            anchor: corners(rect)[3 - corner],
            grab: [pointer[0] - held[0], pointer[1] - held[1]],
            rect,
            scale: layout.scale(i),
            origin: top_left(settings, Element::Potion, viewport),
            slot: [at[0] - by[0], at[1] - by[1]],
            k: settings.style(Element::Potion).scale as f32,
        })
    }

    /// Follows the pointer. Returns whether anything changed.
    pub fn step(&self, settings: &mut Settings, viewport: [f32; 2], pointer: [f32; 2]) -> bool {
        use crate::config::{MAX_POTION_OFFSET, POTION_MAX_SCALE, POTION_MIN_SCALE};
        let [_, _, w0, h0] = self.rect;
        let (w0, h0) = (w0.max(1.0), h0.max(1.0));
        let sx = if self.corner % 2 == 0 { -1.0 } else { 1.0 };
        let sy = if self.corner < 2 { -1.0 } else { 1.0 };
        let dx = (pointer[0] - self.grab[0] - self.anchor[0]) * sx;
        let dy = (pointer[1] - self.grab[1] - self.anchor[1]) * sy;
        let r = (dx * w0 + dy * h0) / (w0 * w0 + h0 * h0);
        let wanted = ((self.scale * r).clamp(POTION_MIN_SCALE, POTION_MAX_SCALE).min(self.scale * (viewport[0] / w0).min(viewport[1] / h0)).max(POTION_MIN_SCALE) * 100.0).round() / 100.0;
        let Some(slot) = usize::try_from(self.id).ok().filter(|i| *i < settings.potion_offsets.len()) else { return false };
        let ratio = wanted / self.scale;
        let (w, h) = (w0 * ratio, h0 * ratio);
        let x = if self.corner % 2 == 0 { self.anchor[0] - w } else { self.anchor[0] };
        let y = if self.corner < 2 { self.anchor[1] - h } else { self.anchor[1] };
        let by = [((x - self.origin[0]) / self.k - self.slot[0]).round(), ((y - self.origin[1]) / self.k - self.slot[1]).round()]
            .map(|v| v.clamp(-MAX_POTION_OFFSET, MAX_POTION_OFFSET));
        if settings.potion_scales[slot] == wanted && settings.potion_offsets[slot] == by {
            return false;
        }
        settings.potion_scales[slot] = wanted;
        settings.potion_offsets[slot] = by;
        true
    }
}

/// The Keystrokes element: key cells laid out like a keyboard, each coloured by its own bindings.
#[inline(never)]
fn keystrokes(o: &mut String, settings: &Settings, viewport: Option<[f32; 2]>) {
    let style = settings.style(Element::Keystrokes);
    let k = style.scale as f32;
    open_panel(o, Element::Keystrokes, settings, viewport);
    let shadow = style.shadow;
    let [sx, sy] = settings.stretch(Element::Keystrokes);
    for (index, key) in keys(settings).iter().enumerate() {
        if index > 0 {
            o.push(',');
        }
        let [x, y, cw, ch] = key.rect.map(|v| v as f32 * k);
        // A stretched card has bigger keys: the names stay in the middle of theirs.
        let lift = (ch * (sy - 1.0)) * 0.5;
        let [x, y, cw, ch] = [x * sx, y * sy, cw * sx, ch * sy];
        let i = slot(key.name);
        o.push_str(r##"{\"k"##);
        push_u(o, i as u64);
        o.push_str(r##"\":{\"type\":\"panel\",\"size\":["##);
        num(o, f64::from(cw));
        o.push(',');
        num(o, f64::from(ch));
        o.push_str(r##"],\"offset\":["##);
        num(o, f64::from(x));
        o.push(',');
        num(o, f64::from(y));
        o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"controls\":["##);
        // Background.
        o.push_str(r##"{\"bg\":{\"type\":\"custom\",\"renderer\":\"cinnabar_rounded_rectangle\",\"radius\":"##);
        num(o, if style.rounded { f64::from(1.5 * k) } else { 0.0 });
        o.push_str(r##",\"color\":[0,0,0,0.5]"##);
        if style.border {
            // Each key's box has the border, whatever its colour while held.
            o.push_str(r##",\"border_width\":"##);
            num(o, f64::from(style.border_width as f32 * k));
            o.push_str(r##",\"border_color\":"##);
            write_rgba(o, rgba(style.border_color, style.border_opacity / 100.0));
        }
        o.push_str(r##",\"size\":[\"100%\",\"100%\"],\"bindings\":[{\"binding_name\":\"#k"##);
        push_u(o, i as u64);
        o.push_str(r##"_bg\",\"binding_name_override\":\"#color\"}]}}"##);
        let fg = key_binding(i, "_fg");
        let mouse = key.name.starts_with("Mouse");
        if mouse && style.labels {
            // Like Lunar: the button's name, with its clicks per second underneath.
            let text_h = 9.0 * k;
            label(
                o,
                "l",
                &key.label,
                false,
                rgba(style.text_color, 1.0),
                Some(fg.as_str()),
                k * FONT,
                [cw, text_h],
                [0.5 * k, 2.5 * k + lift],
                "center",
                shadow,
            );
            let text = key_binding(i, "_t");
            label(
                o,
                "c",
                &text,
                true,
                rgba(style.text_color, 1.0),
                Some(fg.as_str()),
                k * CPS_FONT,
                [cw, 8.0 * k],
                [0.5 * k, 12.0 * k + lift],
                "center",
                shadow,
            );
        } else if !key.label.is_empty() {
            let text_h = 9.0 * k;
            label(
                o,
                "l",
                &key.label,
                false,
                rgba(style.text_color, 1.0),
                Some(fg.as_str()),
                k * FONT,
                [cw, text_h],
                [0.5 * k, (ch - text_h) * 0.5],
                "center",
                shadow,
            );
        } else if key.name == "Space" {
            // Like Lunar: a short bar across the middle of the space key, coloured like its text.
            let bar_w = (cw * 0.5).round();
            let bar_h = k.max(1.0);
            let bar = |o: &mut String, name: &str, at: [f32; 2], color: [f32; 4], bound: bool| {
                o.push_str(r##",{\""##);
                o.push_str(name);
                o.push_str(r##"\":{\"type\":\"custom\",\"renderer\":\"cinnabar_rounded_rectangle\",\"radius\":0.5,\"color\":"##);
                write_rgba(o, color);
                o.push_str(r##",\"size\":["##);
                num(o, f64::from(bar_w));
                o.push(',');
                num(o, f64::from(bar_h));
                o.push_str(r##"],\"offset\":["##);
                num(o, f64::from(at[0]));
                o.push(',');
                num(o, f64::from(at[1]));
                o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\""##);
                if bound {
                    o.push_str(r##",\"bindings\":[{\"binding_name\":\""##);
                    o.push_str(&fg);
                    o.push_str(r##"\",\"binding_name_override\":\"#color\"}]"##);
                }
                o.push_str("}}");
            };
            let at = [(cw - bar_w) * 0.5, (ch - bar_h) * 0.5];
            let text = rgba(style.text_color, 1.0);
            if shadow {
                // The text's shadow: a quarter as bright, one pixel down and right, fading with the colour's opacity.
                bar(o, "bar_s", [at[0] + k, at[1] + k], [text[0] * 0.25, text[1] * 0.25, text[2] * 0.25, text[3]], false);
            }
            bar(o, "bar", at, text, true);
        }
        o.push_str("]}}");
    }
    close_panel(o);
}

/// A text element: a rounded background and one label per line.
#[inline(never)]
fn text_element(o: &mut String, element: Element, settings: &Settings, viewport: Option<[f32; 2]>) {
    let style = settings.style(element);
    let k = style.scale as f32;
    let [w, _] = size(settings, element);
    // A stretched box has room around the text: the text stays in the middle of it.
    let extra = stretch_extra(settings, element);
    let line_count = line_count(settings, element);
    open_panel(o, element, settings, viewport);
    o.push_str(r##"{\"bg\":{\"type\":\"custom\",\"renderer\":\"cinnabar_rounded_rectangle\",\"radius\":"##);
    num(o, if style.rounded { f64::from(3.0 * k) } else { 0.0 });
    o.push_str(r##",\"color\":"##);
    // Without a background the box stays (fully clear), so the layout and dragging do not change.
    write_rgba(o, rgba(style.bg_color, if style.background { style.opacity / 100.0 } else { 0.0 }));
    if style.border && Settings::has_border(element) {
        // The renderer's own inside border: a solid ring along the edge that the (translucent) fill
        // does not blend with, drawn at any size. (A whole-box vector path is not an option: the host
        // refuses paths wider than 256 half-pixel cells, which a CPS box passes at GUI scale 3.)
        o.push_str(r##",\"border_width\":"##);
        num(o, f64::from(style.border_width as f32 * k));
        o.push_str(r##",\"border_color\":"##);
        write_rgba(o, rgba(style.border_color, style.border_opacity / 100.0));
    }
    o.push_str(r##",\"size\":[\"100%\",\"100%\"]}}"##);
    let bound = text_bound(element, style, settings);
    // The shadow is the player's choice, and does not depend on the background (off and 0% match).
    let shadow = style.shadow;
    // One line sits centred in its box, like Lunar's boxes; several lines read from the left.
    let waila = element == Element::Waila;
    let single = line_count == 1 && !waila && !matches!(element, Element::ViewAngles | Element::PvpInfo | Element::Coords);
    // WAILA: the block's icon at the left, the text beside it, centred on the icon's height when it is shorter.
    let (text_left, text_top) = if waila {
        let [_, box_h] = base_size(settings, element);
        let inner = box_h / k - pad_y(style) * 2.0;
        o.push_str(r##",{\"icon\":{\"type\":\"custom\",\"renderer\":\"inventory_item_renderer\",\"size\":["##);
        num(o, f64::from(WAILA_ICON * k));
        o.push(',');
        num(o, f64::from(WAILA_ICON * k));
        o.push_str(r##"],\"offset\":["##);
        num(o, f64::from(pad_x(style) * k + extra[0]));
        o.push(',');
        num(o, f64::from((pad_y(style) + (inner - WAILA_ICON) * 0.5) * k + extra[1]));
        o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"bindings\":[{\"binding_name\":\""##);
        o.push_str(WAILA_ICON_BINDING);
        o.push_str(r##"\",\"binding_name_override\":\"#item_renderer_data\"}]}}"##);
        (WAILA_ICON + WAILA_GAP, ((inner - ROW_H * line_count as f32) * 0.5).max(0.0))
    } else {
        (0.0, 0.0)
    };
    let items = if element == Element::Coords { coords_cells(settings) } else { line_count };
    for n in 0..items {
        if matches!(element, Element::ViewAngles | Element::PvpInfo | Element::Coords) {
            // Two labels a line, the number and its name, each in a panel that a binding moves (where they go depends on the text's width).
            let pieces: &[(char, char, char)] = if element == Element::Coords { &[('q', 'v', 'e'), ('o', 't', 'c'), ('p', 'u', 'd')] } else { &[('o', 't', 'c'), ('p', 'u', 'd')] };
            for &(kind, text_kind, color_kind) in pieces {
                o.push_str(r##",{\"v"##);
                o.push(kind);
                push_u(o, n as u64);
                o.push_str(r##"\":{\"type\":\"panel\",\"size\":["##);
                num(o, f64::from(w));
                o.push(',');
                num(o, f64::from(9.0 * k));
                o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":[0,0],\"bindings\":[{\"binding_name\":\""##);
                o.push_str(&binding(element, kind, n));
                o.push_str(r##"\",\"binding_name_override\":\"#offset\"}],\"controls\":[{\"_\":{\"type\":\"panel\",\"size\":[0,0]}}"##);
                let text = binding(element, text_kind, n);
                let color = binding(element, color_kind, n);
                label(o, "t", &text, true, rgba(style.text_color, 1.0), Some(color.as_str()), k * FONT, [w, 9.0 * k], [0.0, 0.0], "left", shadow);
                o.push_str("]}}");
            }
            continue;
        }
        let text = binding(element, 't', n);
        let color = binding(element, 'c', n);
        let name = {
            let mut name = String::from("t");
            push_u(&mut name, n as u64);
            name
        };
        // The font centres a line including the blank pixel after its last letter, which leaves
        // the text a half pixel left of centre: nudge it back.
        let (width, x, align) = if single {
            (w, 0.5 * k, "center")
        } else {
            (w - extra[0] * 2.0 - pad_x(style) * 2.0 * k - text_left * k, (pad_x(style) + text_left) * k + extra[0], "left")
        };
        label(
            o,
            &name,
            &text,
            true,
            rgba(style.text_color, 1.0),
            if bound { Some(color.as_str()) } else { None },
            k * FONT,
            [width, 9.0 * k],
            [x, (pad_y(style) + text_top + 1.0 + ROW_H * n as f32) * k + extra[1]],
            align,
            shadow,
        );
    }
    if waila {
        // The tool that mines the block, at the right of the first line: its icon, and "OK" or "X" for whether the
        // held tool gets the drops (only for a block that needs the right tool).
        let right = w - (pad_x(style) * k + extra[0]);
        let mark_w = WAILA_GAP * 0.5 + WAILA_MARK_W;
        let row = (pad_y(style) + text_top) * k + extra[1];
        o.push_str(r##",{\"tool\":{\"type\":\"custom\",\"renderer\":\"inventory_item_renderer\",\"size\":["##);
        num(o, f64::from(WAILA_TOOL * k));
        o.push(',');
        num(o, f64::from(WAILA_TOOL * k));
        o.push_str(r##"],\"offset\":["##);
        num(o, f64::from(right - (WAILA_TOOL + mark_w) * k));
        o.push(',');
        num(o, f64::from((row - k).max(0.0)));
        o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"bindings\":[{\"binding_name\":\""##);
        o.push_str(WAILA_TOOL_BINDING);
        o.push_str(r##"\",\"binding_name_override\":\"#item_renderer_data\"}]}}"##);
        label(
            o,
            "mark",
            WAILA_MARK_BINDING,
            true,
            rgba(settings.waila_good_color, 1.0),
            Some(WAILA_MARK_COLOR_BINDING),
            k * FONT,
            [mark_w * k, 9.0 * k],
            [right - mark_w * k + 0.5 * k, row + 2.0 * k],
            "left",
            shadow,
        );
    }
    if cps_bar(settings, element) {
        // Not every texture pack's font has a `|` (and the ones that do would double the bar), so
        // the separator is a thin bar of its own in the gap the text leaves; the mod places it
        // from the text's width (`cps_bar_offset`), through a binding that follows the numbers.
        // The bar and its shadow (the text's, a quarter as bright and one pixel down and right,
        // when the text has one) sit in a small panel, which is what the binding moves.
        let bar = |o: &mut String, name: &str, at: f32, color: [f32; 4]| {
            o.push_str(r##",{\""##);
            o.push_str(name);
            o.push_str(r##"\":{\"type\":\"custom\",\"renderer\":\"cinnabar_rounded_rectangle\",\"radius\":0,\"color\":"##);
            write_rgba(o, color);
            o.push_str(r##",\"size\":["##);
            num(o, f64::from(k));
            o.push(',');
            num(o, f64::from(CPS_BAR_HEIGHT * k));
            o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":["##);
            num(o, f64::from(at));
            o.push(',');
            num(o, f64::from(at));
            o.push_str("]}}");
        };
        o.push_str(r##",{\"barbox\":{\"type\":\"panel\",\"size\":["##);
        num(o, f64::from(2.0 * k));
        o.push(',');
        num(o, f64::from((CPS_BAR_HEIGHT + 1.0) * k));
        o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":[0,0],\"bindings\":[{\"binding_name\":\""##);
        o.push_str(&binding(element, 'b', 0));
        o.push_str(r##"\",\"binding_name_override\":\"#offset\"}],\"controls\":[{\"_\":{\"type\":\"panel\",\"size\":[0,0]}}"##);
        // The bar has a colour of its own (Lunar's Line Color), the text's until one is picked.
        let colour = rgba(settings.bar_color(), 1.0);
        if style.shadow {
            // The shadow the numbers cast (the text colour at a quarter), one pixel down and right
            // like theirs: a shadow from the bar's own, brighter or darker, colour stood out from the
            // text's and looked detached.
            let text = rgba(style.text_color, 1.0);
            bar(o, "bar_s", k, [text[0] * 0.25, text[1] * 0.25, text[2] * 0.25, colour[3]]);
        }
        bar(o, "bar", 0.0, colour);
        o.push_str("]}}");
    }
    close_panel(o);
}

/// The names the Array list shows: every other element that is switched on, the longest name first
/// (the order of the elements for names as long).
fn array_names(settings: &Settings) -> Vec<&'static str> {
    let mut names: Vec<(usize, &'static str)> = Element::ALL
        .iter()
        .copied()
        .filter(|e| *e != Element::ArrayList && settings.is_on(*e))
        .map(|e| (e.index(), e.label()))
        .collect();
    names.sort_by(|a, b| b.1.chars().count().cmp(&a.1.chars().count()).then(a.0.cmp(&b.0)));
    names.into_iter().map(|(_, name)| name).collect()
}

/// `#iv<n>`: the binding that says which item slot `n` of the Inventory HUD shows.
/// One solid rectangle of the Inventory HUD's slot lines, at `at` with `size` (device pixels) inside its panel.
fn inv_line(o: &mut String, name: &str, at: [f32; 2], size: [f32; 2], color: [f32; 4]) {
    o.push_str(r##",{\""##);
    o.push_str(name);
    o.push_str(r##"\":{\"type\":\"custom\",\"renderer\":\"cinnabar_rounded_rectangle\",\"radius\":0,\"color\":"##);
    write_rgba(o, color);
    o.push_str(r##",\"size\":["##);
    num(o, f64::from(size[0]));
    o.push(',');
    num(o, f64::from(size[1]));
    o.push_str(r##"],\"offset\":["##);
    num(o, f64::from(at[0]));
    o.push(',');
    num(o, f64::from(at[1]));
    o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\"}}"##);
}

fn inv_cell_binding(n: usize) -> String {
    let mut name = String::from("#cv");
    push_u(&mut name, n as u64);
    name
}

/// What the Inventory HUD's document depends on besides its style: the switches of its looks.
fn inv_flags(settings: &Settings) -> usize {
    usize::from(settings.inv_empty) | usize::from(settings.inv_show_bg) << 1 | usize::from(settings.inv_mc) << 2 | usize::from(settings.inv_grid) << 3 | (settings.inv_grid_color.wrapping_mul(0x9E37_79B1) & !0xF)
}

fn inv_binding(n: usize) -> String {
    let mut name = String::from("#iv");
    push_u(&mut name, n as u64);
    name
}

// ---- The compass (Direction, Lunar style) ----------------------------------------------

/// Degrees between two marks, and how wide that is, in GUI pixels; the window shows nine marks.
const COMPASS_DEG: f32 = 15.0;
const COMPASS_STEP: f32 = 22.0;
const COMPASS_W: f32 = COMPASS_STEP * 9.0;
/// A row for the waypoint dots (6 pixels), the ticks and a line of text.
const COMPASS_H: f32 = 20.0;

/// Whether `element` is the Direction element drawn as the compass strip (the default look).
fn compass_on(settings: &Settings, element: Element) -> bool {
    element == Element::Facing && !settings.facing_text
}

/// Whether the compass's numbers and letters are Chroma's (the ticks follow Chroma whenever it is on).
fn compass_label_chroma(settings: &Settings) -> bool {
    settings.facing_label_chroma && settings.gameplay.chroma(Element::Facing.index()).on
}

/// What a mark says: a letter at each 45 degrees (Bedrock yaw: 0 south, 90 west, 180 north, 270
/// east), the degrees between.
fn compass_mark(angle: i32) -> String {
    let a = angle.rem_euclid(360);
    match a {
        0 => "S".to_owned(),
        45 => "SW".to_owned(),
        90 => "W".to_owned(),
        135 => "NW".to_owned(),
        180 => "N".to_owned(),
        225 => "NE".to_owned(),
        270 => "E".to_owned(),
        315 => "SE".to_owned(),
        _ => a.to_string(),
    }
}

/// Lunar's compass: a strip of marks every 15 degrees, a tick over each, sliding under the middle as
/// you turn. The marks run past a full turn each way, so the strip wraps without a seam; the heading
/// moves the strip through one binding (`values`).
fn compass_element(o: &mut String, settings: &Settings, viewport: Option<[f32; 2]>) {
    let element = Element::Facing;
    let style = settings.style(element);
    let k = style.scale as f32;
    let extra = stretch_extra(settings, element);
    open_panel(o, element, settings, viewport);
    o.push_str(r##"{\"bg\":{\"type\":\"custom\",\"renderer\":\"cinnabar_rounded_rectangle\",\"radius\":"##);
    num(o, if style.rounded { f64::from(3.0 * k) } else { 0.0 });
    o.push_str(r##",\"color\":"##);
    write_rgba(o, rgba(style.bg_color, if style.background { style.opacity / 100.0 } else { 0.0 }));
    if style.border && Settings::has_border(element) {
        o.push_str(r##",\"border_width\":"##);
        num(o, f64::from(style.border_width as f32 * k));
        o.push_str(r##",\"border_color\":"##);
        write_rgba(o, rgba(style.border_color, style.border_opacity / 100.0));
    }
    o.push_str(r##",\"size\":[\"100%\",\"100%\"]}}"##);
    // The window the marks slide through: everything past it is cut off.
    o.push_str(r##",{\"win\":{\"type\":\"panel\",\"clip_children\":true,\"size\":["##);
    num(o, f64::from(COMPASS_W * k));
    o.push(',');
    num(o, f64::from(COMPASS_H * k));
    o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":["##);
    num(o, f64::from(extra[0]));
    o.push(',');
    num(o, f64::from(pad_y(style) * k + extra[1]));
    o.push_str(r##"],\"controls\":[{\"strip\":{\"type\":\"panel\",\"size\":["##);
    num(o, f64::from(COMPASS_STEP * 36.0 * k));
    o.push(',');
    num(o, f64::from(COMPASS_H * k));
    o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":[0,0],\"bindings\":[{\"binding_name\":\""##);
    o.push_str(&binding(element, 'b', 0));
    o.push_str(r##"\",\"binding_name_override\":\"#offset\"}],\"controls\":[{\"_\":{\"type\":\"panel\",\"size\":[0,0]}}"##);
    // Marks from -90 to 450 degrees: the window reaches 67.5 degrees either side of the heading.
    for n in 0..=36i32 {
        let angle = n * 15 - 90;
        let x = n as f32 * COMPASS_STEP * k;
        // Lunar's strip: a tick and a label at every 15 degrees, all alike (the fade does the rest).
        let height = 6.0;
        let mut name = String::from("k");
        push_u(&mut name, n as u64);
        o.push_str(r##",{\""##);
        o.push_str(&name);
        o.push_str(r##"\":{\"type\":\"custom\",\"renderer\":\"cinnabar_rounded_rectangle\",\"radius\":0,\"color\":"##);
        write_rgba(o, rgba(style.text_color, 1.0));
        o.push_str(r##",\"size\":["##);
        num(o, f64::from(k));
        o.push(',');
        num(o, f64::from(height * k));
        o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":["##);
        num(o, f64::from(x - k * 0.5));
        o.push(',');
        num(o, f64::from(2.5 * k));
        // The mark fades towards the ends of the window through its colour (`values` writes it as the strip turns).
        o.push_str(r##"],\"bindings\":[{\"binding_name\":\""##);
        o.push_str(&binding(element, 'f', n as usize));
        o.push_str(r##"\",\"binding_name_override\":\"#color\"}]}}"##);
        name.insert(0, 'l');
        {
            label(
                o,
                &name,
                &compass_mark(angle),
                false,
                rgba(style.text_color, 1.0),
                None,
                k * FONT * 0.85,
                [COMPASS_STEP * k, 9.0 * k],
                [x - COMPASS_STEP * k * 0.5 + 0.5 * k, 10.0 * k],
                "center",
                false,
            );
            // A label's colour does not carry opacity: it fades through `#alpha` (written with the heading).
            o.truncate(o.len() - 2);
            o.push_str(r##",\"bindings\":[{\"binding_name\":\""##);
            o.push_str(&binding(element, 'g', n as usize));
            o.push_str(r##"\",\"binding_name_override\":\"#alpha\"}"##);
            // Chroma on the numbers and letters too (the colour is a binding, written with the heading).
            if compass_label_chroma(settings) {
                o.push_str(r##",{\"binding_name\":\""##);
                o.push_str(&binding(element, 'h', n as usize));
                o.push_str(r##"\",\"binding_name_override\":\"#color\"}"##);
            }
            o.push_str("]}}");
        }
    }
    // Waypoint markers: they ride inside the strip, so they turn with the marks and a bearing cannot drift. Each
    // slot is a panel slid to its place in the strip by a binding (a drawn node does not take one) with a dot and
    // a thin line under it, coloured by a second binding; two slots per waypoint (a bearing near north shows
    // twice in the strip, once each side of the wrap). Unused slots wait off the window.
    for n in 0..MARKERS {
        o.push_str(r##",{\"wm"##);
        push_u(o, n as u64);
        o.push_str(r##"\":{\"type\":\"panel\",\"size\":["##);
        num(o, f64::from(6.0 * k));
        o.push(',');
        num(o, f64::from(6.0 * k));
        o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":[-99,0],\"bindings\":[{\"binding_name\":\""##);
        o.push_str(&binding(element, 'm', n));
        o.push_str(r##"\",\"binding_name_override\":\"#offset\"}],\"controls\":[{\"d\":{\"type\":\"custom\",\"renderer\":\"cinnabar_rounded_rectangle\",\"radius\":"##);
        num(o, f64::from(3.0 * k));
        o.push_str(r##",\"color\":[1,1,1,1],\"size\":["##);
        num(o, f64::from(6.0 * k));
        o.push(',');
        num(o, f64::from(6.0 * k));
        o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":[0,0],\"bindings\":[{\"binding_name\":\""##);
        o.push_str(&binding(element, 'w', n));
        o.push_str(r##"\",\"binding_name_override\":\"#color\"},{\"binding_name\":\""##);
        o.push_str(&binding(element, 'a', n));
        o.push_str(r##"\",\"binding_name_override\":\"#alpha\"}]}}]}}"##);
    }
    o.push_str("]}}");
    o.push_str("]}}");
    close_panel(o);
}

fn rgba_f(r: f32, g: f32, b: f32) -> [f32; 4] {
    [r, g, b, 1.0]
}

/// Waypoint dots on the compass: two for each of the eight colours.
const MARKERS: usize = 16;

/// Where each marker goes in the strip (x in GUI pixels at scale 1, -99 when unused) and its colour. The strip's marks
/// run from -90 to 450 degrees, so a waypoint's bearing `h` (0 to 360) sits at `h`, and again at `h - 360` (from 270 up)
/// or `h + 360` (up to 90): two slots for each of the eight nearest.
fn marker_slots(rows: &[crate::waypoints::Row]) -> [(f32, [f32; 4]); MARKERS] {
    let mut slots = [(-99.0, [1.0, 1.0, 1.0, 1.0]); MARKERS];
    for (i, row) in rows.iter().take(MARKERS / 2).enumerate() {
        let degrees = f32::from(row.heading) * 0.5;
        let [r, g, b] = crate::waypoints::rgb(row.color);
        let at = |angle: f32| (angle + 90.0) / COMPASS_DEG * COMPASS_STEP - 3.0;
        slots[i * 2] = (at(degrees), rgba_f(r, g, b));
        if degrees >= 270.0 {
            slots[i * 2 + 1] = (at(degrees - 360.0), rgba_f(r, g, b));
        } else if degrees <= 90.0 {
            slots[i * 2 + 1] = (at(degrees + 360.0), rgba_f(r, g, b));
        }
    }
    slots
}

/// The Inventory HUD: a box (like a text element's) with the 27 slots' item icons in it and each
/// stack's count at the corner of its cell.
fn inventory_element(o: &mut String, settings: &Settings, viewport: Option<[f32; 2]>) {
    let element = Element::Inventory;
    let style = settings.style(element);
    let k = style.scale as f32;
    // A stretched box has room around the slots: they stay in the middle of it.
    let extra = stretch_extra(settings, element);
    open_panel(o, element, settings, viewport);
    o.push_str(r##"{\"bg\":{\"type\":\"custom\",\"renderer\":\"cinnabar_rounded_rectangle\",\"radius\":"##);
    num(o, if style.rounded { f64::from(3.0 * k) } else { 0.0 });
    o.push_str(r##",\"color\":"##);
    write_rgba(o, rgba(style.bg_color, if style.background && settings.inv_show_bg { style.opacity / 100.0 } else { 0.0 }));
    // The Border card is the box's outline, like every other element's (the slot lines have their own colours).
    if style.border && Settings::has_border(element) {
        o.push_str(r##",\"border_width\":"##);
        num(o, f64::from(style.border_width as f32 * k));
        o.push_str(r##",\"border_color\":"##);
        write_rgba(o, rgba(style.border_color, style.border_opacity / 100.0));
    }
    o.push_str(r##",\"size\":[\"100%\",\"100%\"]}}"##);
    // Lunar's looks. The Minecraft background: one-pixel lines between the cells and round the outside (the Border card's
    // colour and opacity), cells clear. Without it, the Grid: thin lines in its own colour. Show Empty off draws a ring only
    // round each slot that holds something; Show Background off draws no lines at all.
    let lines_on = settings.inv_show_bg && (settings.inv_mc || settings.inv_grid);
    // The Minecraft background is Lunar's slot bevel: each slot has a dark line along its top and left and a lighter one
    // along its bottom and right (a shade of the Border card's colour: 43% and 67% of it, so its Gray gives #373737 and
    // #555555 over the border opacity). The Grid is one thin line in its own colour.
    let bevel = settings.inv_mc;
    // Lunar's two greys: #373737 at 90% and #555555 at 85%.
    let (dark, light) = ([0.216, 0.216, 0.216, 0.9], [0.333, 0.333, 0.333, 0.85]);
    let thin = rgba(settings.inv_grid_color, settings.inv_grid_opacity / 100.0);
    let th = if bevel { k } else { k * 0.5 };
    let (cols, rows) = (INV_COLS, INV_SLOTS / INV_COLS);
    if lines_on && !settings.inv_empty {
        // Show Empty off: each slot that holds something is drawn on its own (a panel that a binding shows).
        for n in 0..INV_SLOTS {
            let (col, row) = (n % INV_COLS, n / INV_COLS);
            let at = [(INV_PAD + col as f32 * INV_CELL) * k + extra[0], (INV_PAD + row as f32 * INV_CELL) * k + extra[1]];
            o.push_str(r##",{\"c"##);
            push_u(o, n as u64);
            o.push_str(r##"\":{\"type\":\"panel\",\"size\":["##);
            num(o, f64::from(INV_CELL * k));
            o.push(',');
            num(o, f64::from(INV_CELL * k));
            o.push_str(r##"],\"offset\":["##);
            num(o, f64::from(at[0]));
            o.push(',');
            num(o, f64::from(at[1]));
            o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"bindings\":[{\"binding_name\":\""##);
            o.push_str(&inv_cell_binding(n));
            o.push_str(r##"\",\"binding_name_override\":\"#visible\"}],\"controls\":[{\"_\":{\"type\":\"panel\",\"size\":[0,0]}}"##);
            let cell = INV_CELL * k;
            let edges: [(&str, [f32; 2], [f32; 2], [f32; 4]); 4] = if bevel {
                [("a", [0.0, 0.0], [cell, th], dark), ("b", [0.0, 0.0], [th, cell], dark), ("c", [0.0, cell - th], [cell, th], light), ("d", [cell - th, 0.0], [th, cell], light)]
            } else {
                [("a", [0.0, 0.0], [cell + th, th], thin), ("b", [0.0, 0.0], [th, cell + th], thin), ("c", [0.0, cell], [cell + th, th], thin), ("d", [cell, 0.0], [th, cell + th], thin)]
            };
            for (name, off, size, color) in edges {
                inv_line(o, name, off, size, color);
            }
            o.push_str("]}}");
        }
    }
    if lines_on && settings.inv_empty {
        let origin = [INV_PAD * k + extra[0], INV_PAD * k + extra[1]];
        let (full_w, full_h) = (cols as f32 * INV_CELL * k, rows as f32 * INV_CELL * k);
        let mut name = String::new();
        if bevel {
            for c in 0..cols {
                // Left edge of a column of slots: dark. Right edge: light.
                name.clear();
                name.push_str("dv");
                push_u(&mut name, c as u64);
                inv_line(o, &name, [origin[0] + c as f32 * INV_CELL * k, origin[1]], [th, full_h], dark);
                name.clear();
                name.push_str("lv");
                push_u(&mut name, c as u64);
                inv_line(o, &name, [origin[0] + ((c + 1) as f32 * INV_CELL - 1.0) * k, origin[1]], [th, full_h], light);
            }
            for r in 0..rows {
                name.clear();
                name.push_str("dh");
                push_u(&mut name, r as u64);
                inv_line(o, &name, [origin[0], origin[1] + r as f32 * INV_CELL * k], [full_w, th], dark);
                name.clear();
                name.push_str("lh");
                push_u(&mut name, r as u64);
                inv_line(o, &name, [origin[0], origin[1] + ((r + 1) as f32 * INV_CELL - 1.0) * k], [full_w, th], light);
            }
        } else {
            // Open: only the lines between the slots, none round the outside.
            for c in 1..cols {
                name.clear();
                name.push_str("gv");
                push_u(&mut name, c as u64);
                inv_line(o, &name, [origin[0] + c as f32 * INV_CELL * k - th * 0.5, origin[1]], [th, full_h], thin);
            }
            for r in 1..rows {
                name.clear();
                name.push_str("gh");
                push_u(&mut name, r as u64);
                inv_line(o, &name, [origin[0], origin[1] + r as f32 * INV_CELL * k - th * 0.5], [full_w, th], thin);
            }
        }
    }
    let bound = text_bound(element, style, settings);
    for n in 0..INV_SLOTS {
        let (col, row) = (n % INV_COLS, n / INV_COLS);
        let cell = [(INV_PAD + col as f32 * INV_CELL) * k + extra[0], (INV_PAD + row as f32 * INV_CELL) * k + extra[1]];
        // The icon: the host resolves the item bound to `#item_renderer_data`.
        o.push_str(r##",{\"i"##);
        push_u(o, n as u64);
        o.push_str(r##"\":{\"type\":\"custom\",\"renderer\":\"inventory_item_renderer\",\"size\":["##);
        num(o, f64::from(ARMOR_ICON * k));
        o.push(',');
        num(o, f64::from(ARMOR_ICON * k));
        o.push_str(r##"],\"offset\":["##);
        num(o, f64::from(cell[0] + k));
        o.push(',');
        num(o, f64::from(cell[1] + k));
        o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"bindings\":[{\"binding_name\":\""##);
        o.push_str(&inv_binding(n));
        o.push_str(r##"\",\"binding_name_override\":\"#item_renderer_data\"}]}}"##);
        // The count, at the bottom right of the cell and above the icon.
        let text = binding(element, 't', n);
        let color = binding(element, 'c', n);
        let mut name = String::from("t");
        push_u(&mut name, n as u64);
        label(
            o,
            &name,
            &text,
            true,
            rgba(style.text_color, 1.0),
            if bound { Some(color.as_str()) } else { None },
            k * FONT,
            [(INV_CELL - 1.0) * k, ARMOR_LINE * k],
            [cell[0], cell[1] + 9.0 * k],
            "right",
            style.shadow,
        );
    }
    close_panel(o);
}

/// Whether the CPS element draws the separator bar between its two counts.
fn cps_bar(settings: &Settings, element: Element) -> bool {
    element == Element::Cps && settings.cps_mode == crate::config::CpsMode::Both
}

/// What separates the two counts in the text: two spaces, which every font has, with the bar drawn between.
const CPS_GAP: &str = "  ";

/// The bar's height in GUI pixels at size 1: the digits are 7 tall, and the bar reaches one past them
/// at the top and the bottom, like a `|` does.
const CPS_BAR_HEIGHT: f32 = 9.0;

/// Where the separator bar sits in the CPS box (offset of its top-left corner, in GUI pixels):
/// in the gap between the counts in `text`, which is centred in the box like the label.
fn cps_bar_offset(text: &str, settings: &Settings) -> [f32; 2] {
    let style = settings.style(Element::Cps);
    let k = style.scale as f32;
    let s = k * FONT;
    let raw = |t: &str| (text_width(t) - 1.0).max(0.0);
    let [w, _] = size(settings, Element::Cps);
    let extra = stretch_extra(settings, Element::Cps);
    let total = raw(text);
    // The bar goes in the middle of the gap between the counts.
    let (before, gap) = match text.find(CPS_GAP) {
        Some(at) => (raw(&text[..at]), raw(CPS_GAP)),
        None => (total / 2.0, 0.0),
    };
    let left = (w - total * s) / 2.0 + 0.5 * k;
    let centre = left + (before + gap / 2.0) * s;
    [centre - 0.5 * k, (pad_y(style) + 1.0) * k + extra[1]]
}

/// `#a<slot>_item`: the binding of an armour piece's item icon.
fn armor_binding(slot: usize) -> String {
    let mut name = String::from("#a");
    push_u(&mut name, slot as u64);
    name.push_str("_item");
    name
}

/// `#a<slot>_pos`: the binding that places an armour piece on screen.
fn armor_pos_binding(slot: usize) -> String {
    let mut name = String::from("#a");
    push_u(&mut name, slot as u64);
    name.push_str("_pos");
    name
}

/// Where the host puts each piece's box on screen (a screen of `viewport` GUI pixels): every piece
/// the document has, the empty ones beside their neighbour (they draw nothing).
fn armor_positions(settings: &Settings, viewport: Option<[f32; 2]>) -> Vec<(usize, [f32; 2])> {
    let view = viewport.unwrap_or([640.0, 360.0]);
    let style = settings.style(Element::Armor);
    let k = style.scale as f32;
    let [x, y] = top_left(settings, Element::Armor, view);
    let geometry = armor_geometry(style);
    let mut last = [x, y];
    let mut out = Vec::new();
    for slot in armor_slots(style) {
        if let Some(cell) = geometry.cells.iter().find(|c| c.slot == slot) {
            last = [x + (cell.origin[0] - cell.shift) * k, y + cell.origin[1] * k];
        }
        out.push((slot, last));
    }
    out
}

fn armor_position_members(o: &mut String, positions: &[(usize, [f32; 2])], first: &mut bool) {
    for (slot, at) in positions {
        if !*first {
            o.push(',');
        }
        *first = false;
        o.push('"');
        o.push_str(&armor_pos_binding(*slot));
        o.push_str("\":[");
        signed(o, at[0]);
        o.push(',');
        signed(o, at[1]);
        o.push(']');
    }
}

/// A patch that only moves the armour pieces: what dragging sends, a few numbers instead of the
/// whole layer.
#[allow(dead_code)] // the drag uses `ArmorDrag`; kept for the tests
pub fn armor_position_patch(settings: &Settings, viewport: [f32; 2]) -> String {
    let mut o = String::with_capacity(160);
    o.push_str("{\"binding_changes\":{");
    armor_position_members(&mut o, &armor_positions(settings, Some(viewport)), &mut true);
    o.push_str("}}");
    o
}

/// The Armor status element: an item icon the host draws from `#a<slot>_item` for each piece,
/// and the durability beside it. There is no box behind it. The element fills the screen and
/// each piece is a small panel the host places from `#a<slot>_pos`, so moving a piece sends a
/// few numbers and never rewrites this document.
#[inline(never)]
fn armor_element(o: &mut String, settings: &Settings, _viewport: Option<[f32; 2]>) {
    let element = Element::Armor;
    let style = settings.style(element);
    let k = style.scale as f32;
    o.push_str(r##"{\"e"##);
    push_u(o, element.index() as u64);
    o.push_str(r##"\":{\"type\":\"panel\",\"size\":[\"100%\",\"100%\"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":[0,0],\"controls\":["##);
    let bound = text_bound(element, style, settings);
    let geometry = armor_geometry_static(style);
    for (n, cell) in geometry.cells.iter().enumerate() {
        let (slot, align) = (cell.slot, cell.align);
        let icon = [cell.icon[0] - cell.origin[0], cell.icon[1] - cell.origin[1]];
        let text_at = [cell.text_at[0] - cell.origin[0], cell.text_at[1] - cell.origin[1]];
        let text_size = cell.text_size;
        if n > 0 {
            o.push(',');
        }
        // The piece: a panel of the cell's size, placed by its binding.
        o.push_str(r##"{\"p"##);
        push_u(o, slot as u64);
        o.push_str(r##"\":{\"type\":\"panel\",\"size\":["##);
        num(o, f64::from(cell.extent[0] * k));
        o.push(',');
        num(o, f64::from(cell.extent[1] * k));
        o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":[0,0],\"bindings\":[{\"binding_name\":\""##);
        o.push_str(&armor_pos_binding(slot));
        o.push_str(r##"\",\"binding_name_override\":\"#offset\"}],\"controls\":["##);
        // The icon: the host resolves the item bound to `#item_renderer_data`; an empty slot
        // is bound to air, which draws nothing.
        o.push_str(r##"{\"i"##);
        push_u(o, slot as u64);
        o.push_str(r##"\":{\"type\":\"custom\",\"renderer\":\"inventory_item_renderer\",\"size\":["##);
        num(o, f64::from(cell.icon_size[0] * k));
        o.push(',');
        num(o, f64::from(cell.icon_size[1] * k));
        o.push_str(r##"],\"offset\":["##);
        num(o, f64::from(icon[0] * k));
        o.push(',');
        num(o, f64::from(icon[1] * k));
        o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"bindings\":[{\"binding_name\":\""##);
        o.push_str(&armor_binding(slot));
        o.push_str(r##"\",\"binding_name_override\":\"#item_renderer_data\"}]}}"##);
        if text_size[0] > 0.0 {
            let text = binding(element, 't', n);
            let color = binding(element, 'c', n);
            let mut name = String::from("d");
            push_u(&mut name, slot as u64);
            label(
                o,
                &name,
                &text,
                true,
                rgba(style.text_color, 1.0),
                if bound { Some(color.as_str()) } else { None },
                k * FONT * cell.font,
                [text_size[0] * k, text_size[1] * k],
                [text_at[0] * k, text_at[1] * k],
                align,
                style.shadow,
            );
        }
        o.push_str("]}}");
    }
    close_panel(o);
}

/// What an element's piece of the document is built from. The same inputs write the same text,
/// so a piece is kept until one of them changes (customising one element then rebuilds only
/// that element: Keystrokes alone costs most of a frame's fuel to write).
#[derive(Clone, PartialEq)]
pub struct DocKey {
    style: Style,
    /// The measured box width (text elements; 0 for Keystrokes).
    width: u16,
    /// The screen size, for an element that was placed with the mouse or sliders.
    viewport: Option<[f32; 2]>,
    /// Keystrokes only: which rows of keys there are.
    mouse: bool,
    space: bool,
    /// CPS only: whether the separator bar is drawn (the Left | Right mode).
    bar: bool,
    /// Keystrokes only: the names on the key cells.
    labels: [String; 6],
    /// Potion status only: what its layout and options were written from (see `potion_signature`).
    potion: u64,
    /// Array list only: how many lines it has (they follow which elements are on, not its own style).
    lines: usize,
    /// Direction only: the compass strip (not the text line).
    compass: bool,
    /// Inventory HUD only: the opacity of the grid lines.
    grid_opacity: u64,
    /// Whether the element's text is Chroma's (the document then binds its colours).
    chroma: bool,
    /// Direction only: the compass's numbers and letters are Chroma's too.
    label_chroma: bool,
}

/// Each element's piece of the document, with the inputs it was written from.
pub struct DocCache {
    elements: [Option<(DocKey, String)>; Element::COUNT],
    /// The Crosshair's piece, with what it was written from: its drawings and whether it has an outline.
    crosshair: Option<(CrosshairKey, String)>,
    /// The Saturation overlay's piece: the layers it has and where the bar is.
    saturation: Option<(SaturationKey, String)>,
}

// Arrays of more than 32 elements have no `Default`, so it is written out.
impl Default for DocCache {
    fn default() -> Self {
        Self { elements: std::array::from_fn(|_| None), crosshair: None, saturation: None }
    }
}

/// What the Saturation overlay's document depends on: which layers it has, and the bar's corner.
type SaturationKey = ([bool; saturation::LAYERS], [i32; 2], i32);

/// The overlay's parts the settings ask for (none unless Saturation is on).
pub fn saturation_parts(settings: &Settings) -> Parts {
    if !settings.is_on(Element::Food) {
        return Parts::default();
    }
    Parts { saturation: settings.sat_overlay, hunger: settings.sat_hunger, potential: settings.sat_potential }
}

fn doc_key(settings: &Settings, element: Element, viewport: Option<[f32; 2]>) -> DocKey {
    let style = settings.style(element);
    DocKey {
        style: style.clone(),
        width: MEASURED.with(Cell::get)[element.index()],
        viewport: if style.position.is_some() { viewport } else { None },
        mouse: element == Element::Keystrokes && settings.show_mouse,
        space: element == Element::Keystrokes && settings.show_space,
        bar: cps_bar(settings, element),
        labels: if element == Element::Keystrokes { key_labels() } else { Default::default() },
        potion: if element == Element::Potion { potion_signature(settings) } else { 0 },
        lines: if element == Element::Coords {
            coords_cells(settings) * 2 + usize::from(settings.co_horizontal)
        } else if element == Element::Inventory {
            inv_flags(settings)
        } else if matches!(element, Element::ArrayList | Element::Scoreboard | Element::Waypoints | Element::Waila | Element::AudioSubtitles | Element::ViewAngles) {
            line_count(settings, element)
        } else {
            0
        },
        compass: compass_on(settings, element),
        grid_opacity: if element == Element::Inventory { settings.inv_grid_opacity.to_bits() } else { 0 },
        chroma: settings.gameplay.chroma(element.index()).on,
        label_chroma: element == Element::Facing && compass_label_chroma(settings),
    }
}

/// What the crosshair's document depends on: the drawings, whether they are outlined and with round
/// corners, and the middle pixel of the screen.
type CrosshairKey = (Vec<Look>, bool, bool, Option<[i32; 2]>, i32, i32);

/// The HUD document for the elements that are on, already escaped to sit inside a JSON string.
#[allow(dead_code)] // the game uses the cached versions; the tests use this one
pub fn document(settings: &Settings, viewport: Option<[f32; 2]>) -> String {
    document_cached(settings, viewport, &mut DocCache::default())
}

/// `document`, rewriting only the elements whose inputs changed since `cache` was filled.
#[inline(never)]
pub fn document_cached(settings: &Settings, viewport: Option<[f32; 2]>, cache: &mut DocCache) -> String {
    let mut o = String::with_capacity(4096);
    o.push_str(r##"{\"namespace\":\"technic\",\"hud\":{\"type\":\"screen\",\"size\":[\"100%\",\"100%\"],\"controls\":["##);
    let mut first = true;
    // The centre lines go first, so every element sits on top of them.
    if GUIDES.with(Cell::get) {
        guides_overlay(&mut o);
        first = false;
    }
    for element in Element::ALL {
        let slot = element.index();
        if !settings.is_on(element) {
            cache.elements[slot] = None;
            continue;
        }
        if !first {
            o.push(',');
        }
        first = false;
        let key = doc_key(settings, element, viewport);
        let kept = matches!(&cache.elements[slot], Some((known, _)) if *known == key);
        if kept {
            if let Some((_, text)) = &cache.elements[slot] {
                o.push_str(text);
            }
            continue;
        }
        let mut part = String::with_capacity(1024);
        match element {
            Element::Keystrokes => keystrokes(&mut part, settings, viewport),
            Element::Armor => armor_element(&mut part, settings, viewport),
            Element::Potion => potion_element(&mut part, settings, viewport),
            Element::Inventory => inventory_element(&mut part, settings, viewport),
            Element::Facing if !settings.facing_text => compass_element(&mut part, settings, viewport),
            _ => text_element(&mut part, element, settings, viewport),
        }
        o.push_str(&part);
        cache.elements[slot] = Some((key, part));
    }
    // The Crosshair sits in the middle of the screen, above the elements and below the handles.
    if settings.crosshair.enabled {
        if !first {
            o.push(',');
        }
        first = false;
        let (looks, _) = settings.crosshair.drawings();
        // The middle pixel of the screen, counted in device pixels from the top-left.
        let px = crosshair::pixel();
        let origin = viewport.map(|[w, h]| [(w * px / 2.0).floor() as i32, (h * px / 2.0).floor() as i32]);
        let key: CrosshairKey = (
            looks,
            settings.crosshair.outlined(),
            settings.crosshair.outline_round,
            origin,
            (settings.crosshair.outline_width * 1000.0).round() as i32,
            (px * 1000.0).round() as i32,
        );
        let kept = matches!(&cache.crosshair, Some((known, _)) if *known == key);
        if !kept {
            let mut part = String::with_capacity(1024);
            crosshair_element(&mut part, &key.0, key.1, key.2, key.3, key.4 as f32 / 1000.0);
            cache.crosshair = Some((key, part));
        }
        if let Some((_, text)) = &cache.crosshair {
            o.push_str(text);
        }
    } else {
        cache.crosshair = None;
    }
    // The Saturation overlay sits over the game's hunger bar, so it needs the screen size.
    let parts = saturation_parts(settings);
    match viewport.filter(|_| parts.any()) {
        Some(screen) => {
            if !first {
                o.push(',');
            }
            first = false;
            // The game's own hunger row, when the host reports it, beats the layout guess.
            let at = HUNGER_AT.with(Cell::get).unwrap_or_else(|| saturation::origin(saturation::Layout::from_index(settings.sat_layout), screen));
            let key: SaturationKey = (parts.layers(), [(at[0] * 100.0).round() as i32, (at[1] * 100.0).round() as i32], (crosshair::pixel() * 1000.0).round() as i32);
            let kept = matches!(&cache.saturation, Some((known, _)) if *known == key);
            if !kept {
                let mut part = String::with_capacity(8192);
                saturation_element(&mut part, [key.1[0] as f32 / 100.0, key.1[1] as f32 / 100.0], key.0);
                cache.saturation = Some((key, part));
            }
            if let Some((_, text)) = &cache.saturation {
                o.push_str(text);
            }
        }
        None => cache.saturation = None,
    }
    // The editor's resize handles go last, so they sit on top of every element.
    if let Some(rect) = HANDLES.with(Cell::get) {
        if !first {
            o.push(',');
        }
        handles_overlay(&mut o, rect, EDGE_HANDLES.with(Cell::get));
    }
    o.push_str("]}}");
    o
}

// ---- Crosshair -------------------------------------------------------------------------

/// `#xh<n>`: whether drawing `n` of the crosshair is the one shown.
fn crosshair_visible(n: usize) -> String {
    let mut name = String::from("#xh");
    push_u(&mut name, n as u64);
    name
}

/// A fraction of a device pixel: sizes are written this much under and the origin this much over,
/// so the host's rounding (sizes up, positions down) lands on the intended pixel.
const PIXEL_SLACK: f32 = 0.2;

/// The most bars the crosshair's document holds, outline included.
const CROSSHAIR_NODES: usize = 120;

/// The fill colour of every bar, and the outline's.
const CROSSHAIR_FILL: &str = "#xhc";
const CROSSHAIR_EDGE: &str = "#xho";

/// One bar of the crosshair in the escaped document: a rectangle whose top-left corner is `bar.x`,
/// `bar.y` from the middle pixel (the panel's corner), coloured by `colour_binding`.
///
/// With `border`, the bar is an outline: the rectangle has no fill and its inside border, `border` wide, is the bound colour,
/// so what lies under the crosshair's own bar shows through it instead of a translucent fill blending with the outline.
fn crosshair_bar(o: &mut String, name: &str, n: usize, bar: &Bar, colour_binding: &str, border: Option<f32>) {
    o.push_str(r##"{\""##);
    o.push_str(name);
    push_u(o, n as u64);
    o.push_str(r##"\":{\"type\":\"custom\",\"renderer\":\"cinnabar_rounded_rectangle\",\"radius\":"##);
    num(o, f64::from(bar.radius));
    if let Some(width) = border {
        o.push_str(r##",\"color\":[1,1,1,0],\"border_color\":[1,1,1,1],\"border_width\":"##);
        num(o, f64::from(width));
        o.push_str(r##",\"size\":["##);
    } else {
        o.push_str(r##",\"color\":[1,1,1,1],\"size\":["##);
    }
    // The host rounds a size *up* to whole device pixels and a position *down*, so a size of exactly
    // n pixels (or n + a rounding error) could come out as n + 1: ask for a little under.
    num(o, f64::from(bar.w - PIXEL_SLACK / crosshair::pixel()));
    o.push(',');
    num(o, f64::from(bar.h - PIXEL_SLACK / crosshair::pixel()));
    o.push_str(r##"],\"offset\":["##);
    signed(o, bar.x);
    o.push(',');
    signed(o, bar.y);
    o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"bindings\":[{\"binding_name\":\""##);
    o.push_str(colour_binding);
    o.push_str(if border.is_some() { r##"\",\"binding_name_override\":\"#border_color\"}]}}"## } else { r##"\",\"binding_name_override\":\"#color\"}]}}"## });
}

/// The ring of the Circle shape in the escaped document: a closed vector path stroked `ring.width`
/// wide, in a square node whose middle is the middle of the screen. The view box is in GUI pixels,
/// so the points and the stroke are too, and the renderer anti-aliases the edge.
fn crosshair_ring(o: &mut String, name: &str, ring: &Ring, colour_binding: &str) {
    // Room for the stroke and a pixel of anti-aliasing on every side. The node is an odd number of
    // pixels wide with its corner on a whole pixel (the host may snap a fractional position, which
    // would shift the ring by half a pixel); the ring's middle is the middle of its centre pixel,
    // which is the middle pixel of the screen.
    let half = (ring.radius + ring.width / 2.0 + 1.0).ceil();
    let side = half * 2.0 + 1.0;
    o.push_str(r##"{\""##);
    o.push_str(name);
    o.push_str(r##"\":{\"type\":\"custom\",\"renderer\":\"cinnabar_vector_path\",\"size\":["##);
    num(o, f64::from(side));
    o.push(',');
    num(o, f64::from(side));
    o.push_str(r##"],\"offset\":["##);
    signed(o, -half);
    o.push(',');
    signed(o, -half);
    o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"view_box\":["##);
    signed(o, -half - 0.5 * crosshair::step());
    o.push(',');
    signed(o, -half - 0.5 * crosshair::step());
    o.push(',');
    num(o, f64::from(side));
    o.push(',');
    num(o, f64::from(side));
    o.push_str(r##"],\"stroke_width\":"##);
    num(o, f64::from(ring.width));
    o.push_str(r##",\"closed\":true,\"color\":[1,1,1,1],\"paths\":[["##);
    for (i, [x, y]) in crosshair::ring_points(ring).iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        o.push('[');
        signed(o, round2(*x));
        o.push(',');
        signed(o, round2(*y));
        o.push(']');
    }
    o.push_str(r##"]],\"bindings\":[{\"binding_name\":\""##);
    o.push_str(colour_binding);
    o.push_str(r##"\",\"binding_name_override\":\"#color\"}]}}"##);
}

/// A coordinate to a hundredth of a pixel: finer than the renderer can show, and shorter to write.
fn round2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

/// The crosshair: one panel in the middle of the screen holding each drawing (see
/// `CrosshairSettings::drawings`) in a panel of its own, of which the bindings show one. Every
/// bar takes its colour from a binding, so a change of target is a patch of a few numbers.
#[inline(never)]
fn crosshair_element(o: &mut String, looks: &[Look], outlined: bool, round: bool, origin: Option<[i32; 2]>, outline_width: f32) {
    let px = crosshair::pixel();
    // A frame has little fuel and every bar is a node to write, so a crosshair with many bars (a big
    // ring in several looks) goes without its outline rather than risk the frame. A gapped crosshair
    // has 4 or 5 bars and a ring is one node, so this is far from reached.
    // (A ring's outline is two rings, an outer band and an inner one.)
    let total: usize = looks.iter().map(|look| crosshair::bars(look).len() * 2 + if crosshair::ring_of(look).is_some() { 3 } else { 0 }).sum();
    let outlined = outlined && total <= CROSSHAIR_NODES;
    // The panel's corner is the middle pixel: on a whole pixel from the top-left of the screen, so
    // every bar lands on the pixel grid. Without a screen size it is centred.
    match origin {
        Some([x, y]) => {
            o.push_str(r##"{\"xh\":{\"type\":\"panel\",\"size\":[1,1],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":["##);
            num(o, f64::from((x.max(0) as f32 + PIXEL_SLACK) / px));
            o.push(',');
            num(o, f64::from((y.max(0) as f32 + PIXEL_SLACK) / px));
            o.push_str(r##"],\"controls\":["##);
        }
        None => o.push_str(r##"{\"xh\":{\"type\":\"panel\",\"size\":[1,1],\"anchor_from\":\"center\",\"anchor_to\":\"center\",\"offset\":[0,0],\"controls\":["##),
    }
    for (n, look) in looks.iter().enumerate() {
        if n > 0 {
            o.push(',');
        }
        o.push_str(r##"{\"d"##);
        push_u(o, n as u64);
        o.push_str(r##"\":{\"type\":\"panel\",\"size\":[1,1],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":[0,0],\"bindings\":[{\"binding_name\":\""##);
        o.push_str(&crosshair_visible(n));
        o.push_str(r##"\",\"binding_name_override\":\"#visible\"}],\"controls\":["##);
        let bars = crosshair::bars(look);
        let ring = crosshair::ring_of(look);
        let mut first = true;
        if outlined {
            let by = crosshair::snap(outline_width * look.scale.max(0.1), 1);
            // The outline lies beside the crosshair, never under it, so a see-through colour shows what is behind it and not a
            // dark outline: a ring has an outer band and (if it has room) an inner one, a bar a border round its edge.
            if let Some(ring) = &ring {
                first = false;
                crosshair_ring(o, "or", &Ring { radius: ring.radius + ring.width / 2.0 + by / 2.0, width: by }, CROSSHAIR_EDGE);
                let inside = ring.radius - ring.width / 2.0;
                if inside > 0.25 {
                    let band = by.min(inside);
                    o.push(',');
                    crosshair_ring(o, "oi", &Ring { radius: inside - band / 2.0, width: band }, CROSSHAIR_EDGE);
                }
            }
            for (i, bar) in crosshair::outline(&bars, by, round).iter().enumerate() {
                if !first {
                    o.push(',');
                }
                first = false;
                crosshair_bar(o, "o", i, bar, CROSSHAIR_EDGE, Some(by));
            }
        }
        if let Some(ring) = &ring {
            if !first {
                o.push(',');
            }
            first = false;
            crosshair_ring(o, "fr", ring, CROSSHAIR_FILL);
        }
        for (i, bar) in bars.iter().enumerate() {
            if !first {
                o.push(',');
            }
            first = false;
            crosshair_bar(o, "f", i, bar, CROSSHAIR_FILL, None);
        }
        o.push_str("]}}");
    }
    o.push_str("]}}");
}

/// The crosshair's bindings: which drawing shows and the two colours. With `previous`, only what
/// changed since it.
fn crosshair_values(
    o: &mut String,
    member: &mut impl FnMut(&mut String, &str),
    now: &Option<Frame>,
    previous: Option<&Option<Frame>>,
    crosshair: &CrosshairSettings,
) {
    let Some(frame) = now else {
        // The crosshair is on but not shown (third person): hide every drawing, once.
        if crosshair.enabled && previous.is_none_or(|p| p.is_some()) {
            for n in 0..crosshair.drawings().0.len() {
                member(o, &crosshair_visible(n));
                o.push_str("false");
            }
        }
        return;
    };
    let before = previous.and_then(|p| p.as_ref());
    if before.is_none_or(|b| b.drawing != frame.drawing) {
        for n in 0..crosshair.drawings().0.len() {
            member(o, &crosshair_visible(n));
            o.push_str(if n == frame.drawing { "true" } else { "false" });
        }
    }
    if before.is_none_or(|b| b.fill != frame.fill) {
        member(o, CROSSHAIR_FILL);
        write_rgba(o, frame.fill);
    }
    if before.is_none_or(|b| b.outline != frame.outline) {
        member(o, CROSSHAIR_EDGE);
        write_rgba(o, frame.outline);
    }
}

// ---- Saturation overlay ----------------------------------------------------------------

/// `#sy<layer><icon>`: the colour (and so the opacity) of one outline.
fn saturation_binding(layer: usize, icon: usize) -> String {
    let mut name = String::from("#sy");
    push_u(&mut name, layer as u64);
    push_u(&mut name, icon as u64);
    name
}

/// The overlay in the escaped document: one panel whose corner is the right-most hunger icon's,
/// holding, for every icon of every wanted layer, the rectangles that cover the hunger sprite's
/// pixels (see `saturation::SPRITE`). All the rectangles of one icon share one colour binding
/// (alpha 0 hides them), so a change of food or saturation is a patch of a few colours.
/// The overlay's rectangles for one GUI pixel size, written once and kept: for each icon, the text of
/// all its rectangles with layer 0 in their names and bindings, and where those layer digits are.
/// Every layer's text is that same text with its own digit (`saturation_element`).
struct SatBlocks {
    slack_bits: u32,
    blocks: Vec<(String, Vec<usize>)>,
}

thread_local! {
    static SAT_BLOCKS: RefCell<Option<SatBlocks>> = const { RefCell::new(None) };
}

// A layer's number is one digit in the names and bindings, so a block of layer 0 becomes any
// other layer's by changing that digit.
const _: () = assert!(saturation::LAYERS <= 10);

/// The text of every rectangle of every icon for layer 0, for this GUI pixel size (`slack`).
fn sat_blocks(slack: f32) -> Vec<(String, Vec<usize>)> {
    let rects = saturation::rects();
    // What depends only on the rectangle, written once for all the icons: its number, size and
    // top ("_<n>\":{...\"offset\":[" and ",<top>]...\"binding_name\":\"").
    let parts: Vec<(String, String)> = rects
        .iter()
        .enumerate()
        .map(|(n, [_, y, w, h])| {
            let mut head = String::with_capacity(160);
            head.push('_');
            push_u(&mut head, n as u64);
            head.push_str(r##"\":{\"type\":\"custom\",\"renderer\":\"cinnabar_rounded_rectangle\",\"radius\":0,\"color\":[1,1,1,0],\"size\":["##);
            num(&mut head, f64::from(w - slack));
            head.push(',');
            num(&mut head, f64::from(h - slack));
            head.push_str(r##"],\"offset\":["##);
            let mut tail = String::with_capacity(120);
            tail.push(',');
            signed(&mut tail, y + slack);
            tail.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"bindings\":[{\"binding_name\":\""##);
            (head, tail)
        })
        .collect();
    let mut blocks = Vec::with_capacity(saturation::SHANKS);
    for icon in 0..saturation::SHANKS {
        let mut o = String::with_capacity(rects.len() * 300);
        let mut digits = Vec::with_capacity(rects.len() * 2);
        let binding = saturation_binding(0, icon);
        for (n, ([x, ..], (head, tail))) in rects.iter().zip(&parts).enumerate() {
            if n > 0 {
                o.push(',');
            }
            o.push_str(r##"{\"s"##);
            digits.push(o.len());
            push_u(&mut o, 0);
            push_u(&mut o, icon as u64);
            o.push_str(head);
            signed(&mut o, x - icon as f32 * saturation::STEP + slack);
            o.push_str(tail);
            // `#sy<layer><icon>`: the layer digit follows the three characters "#sy".
            digits.push(o.len() + 3);
            o.push_str(&binding);
            o.push_str(r##"\",\"binding_name_override\":\"#color\"}]}}"##);
        }
        blocks.push((o, digits));
    }
    blocks
}

/// The overlay in the escaped document: one panel whose corner is the right-most hunger icon's,
/// holding, for every icon of every wanted layer, the rectangles that cover the hunger sprite's
/// pixels (see `saturation::SPRITE`). All the rectangles of one icon share one colour binding
/// (alpha 0 hides them), so a change of food or saturation is a patch of a few colours.
///
/// The overlay is hundreds of rectangles, most of a first layer's cost, and every layer repeats the
/// same icons: their text is written once for the GUI pixel size (`SAT_BLOCKS`) and copied here,
/// with each layer's digit put in. The document is the same as writing every rectangle afresh
/// (`saturation_element_reference`, checked by the tests).
#[inline(never)]
fn saturation_element(o: &mut String, origin: [f32; 2], layers: [bool; saturation::LAYERS]) {
    o.push_str(r##"{\"sat\":{\"type\":\"panel\",\"size\":[1,1],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":["##);
    signed(o, origin[0]);
    o.push(',');
    signed(o, origin[1]);
    o.push_str(r##"],\"controls\":["##);
    // The host rounds a size up to whole device pixels and a position down (see `crosshair_bar`).
    let slack = PIXEL_SLACK / crosshair::pixel();
    SAT_BLOCKS.with_borrow_mut(|cache| {
        if cache.as_ref().is_none_or(|c| c.slack_bits != slack.to_bits()) {
            *cache = Some(SatBlocks { slack_bits: slack.to_bits(), blocks: sat_blocks(slack) });
        }
        let Some(cache) = cache.as_ref() else { return };
        let wanted = layers.iter().filter(|on| **on).count();
        o.reserve(wanted * cache.blocks.iter().map(|(text, _)| text.len() + 1).sum::<usize>());
        let mut first = true;
        for (layer, wanted) in layers.iter().enumerate() {
            if !wanted {
                continue;
            }
            let digit = b'0' + layer as u8;
            for (text, digits) in &cache.blocks {
                if !first {
                    o.push(',');
                }
                first = false;
                let at = o.len();
                o.push_str(text);
                if layer != 0 {
                    // SAFETY: each position holds the ASCII digit '0' written by `sat_blocks`, and
                    // replacing one ASCII byte with another keeps the string valid UTF-8.
                    let bytes = unsafe { o.as_mut_vec() };
                    for &d in digits {
                        bytes[at + d] = digit;
                    }
                }
            }
        }
    });
    o.push_str("]}}");
}

/// `saturation_element` as first written, one rectangle at a time: kept to check that the faster
/// one writes exactly the same document.
#[cfg(test)]
fn saturation_element_reference(o: &mut String, origin: [f32; 2], layers: [bool; saturation::LAYERS]) {
    o.push_str(r##"{\"sat\":{\"type\":\"panel\",\"size\":[1,1],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"offset\":["##);
    signed(o, origin[0]);
    o.push(',');
    signed(o, origin[1]);
    o.push_str(r##"],\"controls\":["##);
    // The host rounds a size up to whole device pixels and a position down (see `crosshair_bar`).
    let slack = PIXEL_SLACK / crosshair::pixel();
    let rects = saturation::rects();
    let mut first = true;
    for (layer, wanted) in layers.iter().enumerate() {
        if !wanted {
            continue;
        }
        for icon in 0..saturation::SHANKS {
            let binding = saturation_binding(layer, icon);
            for (n, [x, y, w, h]) in rects.iter().enumerate() {
                if !first {
                    o.push(',');
                }
                first = false;
                o.push_str(r##"{\"s"##);
                push_u(o, layer as u64);
                push_u(o, icon as u64);
                o.push('_');
                push_u(o, n as u64);
                o.push_str(r##"\":{\"type\":\"custom\",\"renderer\":\"cinnabar_rounded_rectangle\",\"radius\":0,\"color\":[1,1,1,0],\"size\":["##);
                num(o, f64::from(w - slack));
                o.push(',');
                num(o, f64::from(h - slack));
                o.push_str(r##"],\"offset\":["##);
                signed(o, x - icon as f32 * saturation::STEP + slack);
                o.push(',');
                signed(o, y + slack);
                o.push_str(r##"],\"anchor_from\":\"top_left\",\"anchor_to\":\"top_left\",\"bindings\":[{\"binding_name\":\""##);
                o.push_str(&binding);
                o.push_str(r##"\",\"binding_name_override\":\"#color\"}]}}"##);
            }
        }
    }
    o.push_str("]}}");
}

/// The outlines' colours: written with the whole layer, and again when the food, the saturation,
/// the food in your hand or the flash changed.
fn saturation_values(o: &mut String, member: &mut impl FnMut(&mut String, &str), view: &Views, previous: Option<&Views>, settings: &Settings, viewport: Option<[f32; 2]>) {
    let parts = saturation_parts(settings);
    if !parts.any() || viewport.is_none() {
        return;
    }
    if previous.is_some_and(|p| p.food == view.food && p.eating == view.eating && p.pulse == view.pulse) {
        return;
    }
    let alphas = saturation::alphas(view.food, view.eating, view.pulse, parts);
    let [r, g, b, _] = rgba(settings.sat_color, 1.0);
    for (layer, wanted) in parts.layers().iter().enumerate() {
        if !wanted {
            continue;
        }
        let [r, g, b] = saturation::layer_rgb(layer, [r, g, b], settings.sat_inverse);
        for icon in 0..saturation::SHANKS {
            member(o, &saturation_binding(layer, icon));
            write_rgba(o, [r, g, b, alphas[layer][icon]]);
        }
    }
}

// ---- Values ----------------------------------------------------------------------------

/// The bindings of every element that is on (and, with `previous`, whose values changed), as JSON
/// object members.
fn values(o: &mut String, view: &Views, previous: Option<&Views>, settings: &Settings, viewport: Option<[f32; 2]>) {
    let mut first = true;
    let mut member = |o: &mut String, name: &str| {
        if !first {
            o.push(',');
        }
        first = false;
        o.push('"');
        o.push_str(name);
        o.push_str("\":");
    };
    // A whole layer starts with the handles where they are; a patch moves them separately.
    if previous.is_none() {
        if let Some(rect) = HANDLES.with(Cell::get) {
            let corners = handle_offsets(rect);
            let count = handle_count(EDGE_HANDLES.with(Cell::get));
            for (binding, at) in std::iter::once((FRAME_BINDING, [rect[0], rect[1]])).chain(HANDLE_BINDINGS.iter().copied().zip(corners).take(count)) {
                member(o, binding);
                o.push('[');
                signed(o, at[0]);
                o.push(',');
                signed(o, at[1]);
                o.push(']');
            }
            for (binding, at) in [GEAR_BINDING, REMOVE_BINDING].into_iter().zip(button_places(rect)) {
                member(o, binding);
                o.push('[');
                signed(o, at[0]);
                o.push(',');
                signed(o, at[1]);
                o.push(']');
            }
        }
    }
    for element in Element::ALL {
        let chroma_moved = previous.is_some_and(|prev| prev.chroma != view.chroma) && settings.gameplay.chroma(element.index()).on;
        if !settings.is_on(element) || previous.is_some_and(|prev| !view.changed(prev, element)) && !chroma_moved {
            continue;
        }
        let style = settings.style(element);
        if element == Element::Keystrokes {
            let idle = if style.background { style.opacity / 100.0 } else { 0.0 };
            for key in keys(settings) {
                let i = slot(key.name);
                let held = view.holds(key.name);
                // On a patch, only what changed for this key: its colours when it was pressed or
                // released, and its count when that side's clicks changed.
                let held_changed = previous.is_none_or(|prev| prev.holds(key.name) != held);
                if held_changed {
                    member(o, &key_binding(i, "_bg"));
                    write_rgba(o, if held { rgba(style.pressed_color, PRESSED_ALPHA) } else { rgba(style.bg_color, idle) });
                }
                if held_changed || chroma_moved {
                    member(o, &key_binding(i, "_fg"));
                    let plain = rgba(if held { style.pressed_text_color } else { style.text_color }, 1.0);
                    write_rgba(o, chroma_rgba(element, view, settings, i, 1.0).unwrap_or(plain));
                }
                if style.labels && key.name.starts_with("Mouse") {
                    let side = usize::from(key.name == "MouseRight");
                    if previous.is_none_or(|prev| prev.cps[side] != view.cps[side]) {
                        let mut text = count(view.cps[side]);
                        text.push_str(" CPS");
                        member(o, &key_binding(i, "_t"));
                        q(o, &text);
                    }
                }
            }
            continue;
        }
        if element == Element::Armor {
            // Where the pieces are: written with the whole layer, by `armor_position_patch` when
            // one is dragged, and again when what they show changes how they sit (a number gaining
            // a digit, a piece put on or taken off).
            let moved = match previous {
                None => {
                    set_armor_live(Some(armor_live(view, settings)));
                    true
                }
                Some(before) if before.armor != view.armor => {
                    let was = armor_live(before, settings);
                    let now = armor_live(view, settings);
                    set_armor_live(Some(now));
                    was != now
                }
                Some(_) => false,
            };
            if moved {
                for (slot, at) in armor_positions(settings, viewport) {
                    member(o, &armor_pos_binding(slot));
                    o.push('[');
                    signed(o, at[0]);
                    o.push(',');
                    signed(o, at[1]);
                    o.push(']');
                }
            }
            for slot in armor_slots(style) {
                let piece = &view.armor[slot];
                member(o, &armor_binding(slot));
                o.push_str(r#"{"item":{"identifier":"#);
                q(o, if piece.item.is_empty() { "minecraft:air" } else { piece.item.as_str() });
                o.push_str(r#","metadata":"#);
                push_u(o, u64::from(piece.metadata));
                o.push_str("}}");
            }
        }
        if element == Element::Inventory {
            // The icons: the item in each slot (an empty slot is bound to air, which draws nothing).
            // The counts are the element's lines, written with the other text below.
            for n in 0..INV_SLOTS {
                let piece = view.inventory.get(n);
                let item = piece.map_or("", |piece| piece.item.as_str());
                if !settings.inv_empty {
                    member(o, &inv_cell_binding(n));
                    o.push_str(if item.is_empty() { "false" } else { "true" });
                }
                member(o, &inv_binding(n));
                o.push_str(r#"{"item":{"identifier":"#);
                q(o, if item.is_empty() { "minecraft:air" } else { item });
                o.push_str(r#","metadata":"#);
                push_u(o, u64::from(piece.map_or(0, |piece| piece.metadata)));
                o.push_str("}}");
            }
        }
        if element == Element::Potion {
            potion_values(o, &mut member, view, settings);
            continue;
        }
        if compass_on(settings, element) {
            // The strip slides by the heading: one binding, written when the heading moves.
            if chroma_moved || previous.is_none_or(|prev| prev.yaw != view.yaw || prev.waypoints != view.waypoints) {
                let k = style.scale as f32;
                let at = COMPASS_W * 0.5 - (f32::from(view.yaw.unwrap_or(0)) * 0.5 + 90.0) / COMPASS_DEG * COMPASS_STEP;
                member(o, &binding(element, 'b', 0));
                o.push('[');
                signed(o, at * k);
                o.push_str(",0]");
                // Marks fade out towards the ends of the window: full within 25 degrees of the heading, gone at
                // 67 (only those near the window are written; the rest are cut off by it anyway).
                let heading = f32::from(view.yaw.unwrap_or(0)) * 0.5;
                for n in 0..=36i32 {
                    let away = ((n * 15 - 90) as f32 - heading).abs();
                    if away > 80.0 {
                        continue;
                    }
                    let alpha = 1.0 - ((away - 25.0) / 42.0).clamp(0.0, 1.0);
                    member(o, &binding(element, 'f', n as usize));
                    write_rgba(o, chroma_rgba(element, view, settings, n as usize, alpha).unwrap_or_else(|| rgba(style.text_color, f64::from(alpha))));
                    member(o, &binding(element, 'g', n as usize));
                    num(o, f64::from(alpha));
                    if compass_label_chroma(settings) {
                        member(o, &binding(element, 'h', n as usize));
                        write_rgba(o, chroma_rgba(element, view, settings, n as usize, 1.0).unwrap_or_else(|| rgba(style.text_color, 1.0)));
                    }
                }
                // The waypoint dots fade the same way (their place in the strip gives their bearing).
                let shown = if settings.gameplay.wp_compass && settings.gameplay.wp_on { &view.waypoints[..] } else { &[] };
                for (n, (x, _)) in marker_slots(shown).iter().enumerate() {
                    let angle = (x + 3.0) / COMPASS_STEP * COMPASS_DEG - 90.0;
                    let alpha = if *x < 0.0 { 0.0 } else { 1.0 - (((angle - heading).abs() - 25.0) / 42.0).clamp(0.0, 1.0) };
                    member(o, &binding(element, 'a', n));
                    num(o, f64::from(alpha));
                }
            }
            // The markers sit in the strip, so only a change of the waypoints moves them.
            if previous.is_none_or(|prev| prev.waypoints != view.waypoints) {
                let k = style.scale as f32;
                for (n, (x, color)) in marker_slots(if settings.gameplay.wp_compass && settings.gameplay.wp_on { &view.waypoints } else { &[] }).iter().enumerate() {
                    member(o, &binding(element, 'm', n));
                    o.push('[');
                    signed(o, x * k);
                    o.push_str(",0]");
                    member(o, &binding(element, 'w', n));
                    write_rgba(o, *color);
                }
            }
            continue;
        }
        let bound = text_bound(element, style, settings);
        if element == Element::Reach {
            member(o, REACH_VISIBLE);
            o.push_str(if view.reach_hidden { "false" } else { "true" });
        }
        if element == Element::Scoreboard {
            member(o, SIDEBAR_VISIBLE);
            o.push_str(if view.sidebar_hidden { "false" } else { "true" });
        }
        if element == Element::AudioSubtitles {
            member(o, SUBTITLES_VISIBLE);
            o.push_str(if view.subtitles_hidden { "false" } else { "true" });
        }
        if element == Element::ViewAngles {
            member(o, VA_VISIBLE);
            o.push_str(if view.va_hidden { "false" } else { "true" });
        }
        if element == Element::TargetHud {
            member(o, TARGET_VISIBLE);
            o.push_str(if view.target_hidden { "false" } else { "true" });
        }
        if element == Element::Waila {
            // The icon: the aimed block's item (the identifier is the item's for almost every block); air draws nothing.
            member(o, WAILA_ICON_BINDING);
            o.push_str(r#"{"item":{"identifier":"#);
            q(o, view.waila.as_ref().map_or("minecraft:air", |block| block.icon.as_str()));
            o.push_str(r#","metadata":0}}"#);
            // The tool's icon (air when the block has none or the option is off) and its mark.
            let block = view.waila.as_ref();
            member(o, WAILA_TOOL_BINDING);
            o.push_str(r#"{"item":{"identifier":"#);
            let tool_icon = block.and_then(|b| b.mining.filter(|_| settings.waila_tool)).map_or("minecraft:air".to_owned(), |m| m.icon());
            q(o, &tool_icon);
            o.push_str(r#","metadata":0}}"#);
            let mark = block.and_then(|b| waila_mark(b, settings));
            member(o, WAILA_MARK_BINDING);
            q(o, mark.map_or("", |(text, _)| text));
            member(o, WAILA_MARK_COLOR_BINDING);
            write_rgba(o, rgba(if mark.is_some_and(|(_, ok)| ok) { settings.waila_good_color } else { settings.waila_bad_color }, 1.0));
            member(o, WAILA_VISIBLE);
            o.push_str(if view.waila_hidden { "false" } else { "true" });
        }
        let rows = lines(element, view, settings);
        for (n, text_line) in rows.iter().enumerate() {
            member(o, &binding(element, 't', n));
            q(o, &text_line.text);
            if bound {
                member(o, &binding(element, 'c', n));
                write_rgba(o, chroma_rgba(element, view, settings, n, 1.0).unwrap_or(text_line.color));
            }
            if let Some((tail, tail_color)) = &text_line.tail {
                member(o, &binding(element, 'u', n));
                q(o, tail);
                member(o, &binding(element, 'd', n));
                write_rgba(o, chroma_rgba(element, view, settings, n, 1.0).unwrap_or(*tail_color));
                let places: Vec<(char, [f32; 2])> = if element == Element::Coords {
                    let [head_at, value_at, tail_at] = coords_place(settings, &rows, n);
                    let (head, head_color) = text_line.head.clone().unwrap_or((String::new(), text_line.color));
                    member(o, &binding(element, 'v', n));
                    q(o, &head);
                    member(o, &binding(element, 'e', n));
                    write_rgba(o, chroma_rgba(element, view, settings, n, 1.0).unwrap_or(head_color));
                    vec![('q', head_at), ('o', value_at), ('p', tail_at)]
                } else {
                    let [value_at, tail_at] = pair_place(settings, element, text_line, n);
                    vec![('o', value_at), ('p', tail_at)]
                };
                for (kind, at) in places {
                    member(o, &binding(element, kind, n));
                    o.push('[');
                    signed(o, at[0]);
                    o.push(',');
                    signed(o, at[1]);
                    o.push(']');
                }
            }
            if n == 0 && cps_bar(settings, element) {
                let at = cps_bar_offset(&text_line.text, settings);
                member(o, &binding(element, 'b', 0));
                o.push('[');
                signed(o, at[0]);
                o.push(',');
                signed(o, at[1]);
                o.push(']');
            }
        }
    }
    crosshair_values(o, &mut member, &view.crosshair, previous.map(|p| &p.crosshair), &settings.crosshair);
    saturation_values(o, &mut member, view, previous, settings, viewport);
}

/// The whole layer for `hud.set-surface`.
#[allow(dead_code)] // the game uses the cached versions; the tests use this one
pub fn surface(view: &Views, settings: &Settings, viewport: Option<[f32; 2]>) -> String {
    surface_cached(view, settings, viewport, &mut DocCache::default())
}

/// `surface`, reusing the pieces of the document that did not change.
#[inline(never)]
pub fn surface_cached(view: &Views, settings: &Settings, viewport: Option<[f32; 2]>, cache: &mut DocCache) -> String {
    let doc = document_cached(settings, viewport, cache);
    let mut o = String::with_capacity(doc.len() + 1024);
    o.push_str("{\"screen\":\"");
    o.push_str(SCREEN);
    o.push_str("\",\"document\":\"");
    o.push_str(&doc);
    o.push_str("\",\"bindings\":{");
    values(&mut o, view, None, settings, viewport);
    o.push('}');
    transitions(&mut o, settings);
    o.push('}');
    o
}

/// Set to false to send the HUD without any easing (every value then changes at once, as before).
pub const HUD_EASE: bool = true;

/// How long a held key takes to change colour, in seconds. Short enough to feel instant, long
/// enough that a tap is not a one-frame flash.
#[cfg_attr(not(test), allow(dead_code))]
const KEY_EASE_SECONDS: f64 = 0.09;
/// `KEY_EASE_SECONDS` as the document writes it (what `format!("{}")` gives for it; a test checks
/// they agree). Writing a float with `format!` costs thousands of fuel, and a layer with Keystrokes
/// on writes it twice per key.
const KEY_EASE_TEXT: &str = "0.09";

/// The HUD's declared easing: `,"transitions":{name: {duration, easing}}` for the bindings that
/// should glide to their new value (the host samples them on its own clock, so it costs the mod
/// no fuel and no extra patches). This is the groundwork for smooth HUD changes: to ease another
/// number or colour, add its binding name here. The binding has to be numeric or an array of numbers
/// (colours and offsets are), and the host allows 512 of them on the HUD layer.
fn transitions(o: &mut String, settings: &Settings) {
    if !HUD_EASE {
        return;
    }
    let mut first = true;
    let mut add = |o: &mut String, name: &str, seconds: &str| {
        o.push_str(if first { ",\"transitions\":{" } else { "," });
        first = false;
        o.push('"');
        o.push_str(name);
        o.push_str("\":{\"duration\":");
        o.push_str(seconds);
        o.push_str(",\"easing\":[0.2,0.8,0.2,1]}");
    };
    if settings.is_on(Element::Keystrokes) {
        for key in keys(settings) {
            let i = slot(key.name);
            add(o, &key_binding(i, "_bg"), KEY_EASE_TEXT);
            add(o, &key_binding(i, "_fg"), KEY_EASE_TEXT);
        }
    }
    if !first {
        o.push('}');
    }
}

/// A patch with only the values that changed since `previous` (the guest uses `patch_at`; this
/// is the form the tests read).
#[cfg(test)]
pub fn patch(view: &Views, previous: &Views, settings: &Settings) -> String {
    patch_at(view, previous, settings, None)
}

/// `patch` for a screen of `viewport` GUI pixels (the pieces of Armor status move with the
/// numbers, and where they go depends on the screen).
#[inline(never)]
pub fn patch_at(view: &Views, previous: &Views, settings: &Settings, viewport: Option<[f32; 2]>) -> String {
    let mut o = String::with_capacity(512);
    o.push_str("{\"binding_changes\":{");
    values(&mut o, view, Some(previous), settings, viewport);
    o.push_str("}}");
    o
}

#[cfg(test)]
mod tests {

    #[test]
    fn compass_marks_and_mode() {
        assert_eq!([compass_mark(0), compass_mark(180), compass_mark(-90), compass_mark(360 + 15)], ["S", "N", "E", "15"]);
        let mut settings = Settings::default();
        assert!(compass_on(&settings, Element::Facing) && !compass_on(&settings, Element::Fps));
        assert_eq!(size(&settings, Element::Facing)[0], COMPASS_W * settings.style(Element::Facing).scale as f32);
        settings.facing_text = true;
        assert!(!compass_on(&settings, Element::Facing));
    }

    #[test]
    fn the_key_easing_time_is_written_as_format_writes_it() {
        assert_eq!(KEY_EASE_TEXT, format!("{KEY_EASE_SECONDS}"));
    }

    #[test]
    fn the_saturation_overlay_is_written_as_before() {
        let before = crosshair::pixel();
        for scale in [1.0_f32, 1.5, 2.0, 2.25, 3.0, 4.0, 0.75] {
            crosshair::set_pixel(scale);
            for origin in [[0.0_f32, 0.0], [427.0, 431.5], [-12.25, 3.75], [0.1, -0.1], [1919.999, 1079.5]] {
                for mask in 0..(1 << saturation::LAYERS) {
                    let layers: [bool; saturation::LAYERS] = std::array::from_fn(|l| mask & (1 << l) != 0);
                    let (mut fast, mut slow) = (String::from("x"), String::from("x"));
                    saturation_element(&mut fast, origin, layers);
                    saturation_element_reference(&mut slow, origin, layers);
                    assert_eq!(fast, slow, "scale {scale} origin {origin:?} layers {layers:?}");
                }
            }
        }
        crosshair::set_pixel(before);
    }
    use super::*;
    use serde_json::Value;

    fn parse(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    fn all_on() -> Settings {
        let mut settings = Settings::default();
        for element in Element::ALL {
            settings.style_mut(element).enabled = true;
        }
        settings
    }

    fn view() -> Views {
        Views {
            held: [true, false, false, false, false, false, false],
            cps: [7, 3],
            fps: Some(144),
            feet: [-5, 70, 33],
            facing: Some(2),
            overworld: true,
            slime: Some(true),
            guardians: Some(Guardians::Found { active: 22, blocked: 3, checked: true }),
            armor: Default::default(),
            item_count: 5,
            inventory: Vec::new(),
            ping: Some(42),
            speed: Some(56),
            stopwatch: 3725,
            playtime: 65,
            reach: Some(296),
            reach_hidden: false,
            combo: 3,
            food: Some([18, 45]),
            eating: None,
            pulse: 0,
            day: Some(11),
            potions: Vec::new(),
            blink: false,
            crosshair: None,
            sidebar: None,
            sidebar_hidden: false,
            target: None,
            target_hidden: false,
            waila: None,
            waila_hidden: false,
            height_limit: None,
            pvp: [0; 4],
            subtitles: Vec::new(),
            subtitles_hidden: false,
            cps_stats: [0; 3],
            waypoints: Vec::new(),
            ..Default::default()
        }
    }

    #[test]
    fn waila_lines_follow_the_options() {
        let mut settings = Settings::default();
        let block = crate::waila::describe("minecraft:wheat", Some("{\"growth\":7}"), &[], [1, 2, 3]);
        let rows = waila_rows(&block, &settings);
        assert_eq!(rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(), ["Wheat", "Minecraft", "Growth: Mature"]);
        assert_eq!(rows[2].1, WailaTone::Good);
        settings.waila_id = true;
        settings.waila_pos = true;
        settings.waila_growth = false;
        let rows = waila_rows(&block, &settings);
        assert_eq!(rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(), ["Wheat", "Minecraft", "minecraft:wheat", "X 1  Y 2  Z 3"]);
        assert_eq!(waila_line_total(Some(&block), &settings), 4);
        let cow = crate::waila::entity("Cow", "minecraft:cow", Some(10.0), Some(10.0));
        assert_eq!(waila_rows(&cow, &settings).last().map(|r| r.0.as_str()), Some("minecraft:cow"));
    }

    #[test]
    fn waila_marks_follow_the_held_tool() {
        let settings = Settings::default();
        let mut ore = crate::waila::describe("minecraft:iron_ore", None, &[], [0, 0, 0]);
        ore.harvest = Some(true);
        assert_eq!(waila_mark(&ore, &settings), Some(("\u{2713}", true)));
        ore.harvest = Some(false);
        assert_eq!(waila_mark(&ore, &settings), Some(("\u{2717}", false)));
        let dirt = crate::waila::describe("minecraft:dirt", None, &[], [0, 0, 0]);
        assert_eq!(waila_mark(&dirt, &settings), None);
        assert!(waila_tool_width(&dirt, &settings) > 0.0);
    }

    #[test]
    fn the_target_hud_names_and_counts_health() {
        assert_eq!(target_name("", "minecraft:zombie_villager"), "Zombie villager");
        assert_eq!(target_name(" Steve ", "minecraft:player"), "Steve");
        assert_eq!(health_text(20.0), "20");
        assert_eq!(health_text(13.5), "13.5");
        let settings = Settings::default();
        let style = settings.style(Element::TargetHud);
        let target = TargetView { name: "Steve".into(), health: Some(13.5), max_health: Some(20.0) };
        let view = Views { target: Some(target), ..Views::default() };
        let texts: Vec<String> = target_lines(&view, style, [1.0; 4]).into_iter().map(|l| l.text).collect();
        assert_eq!(texts[0], "Steve");
        assert!(texts[1].contains("13.5 / 20"), "{texts:?}");
    }

    #[test]
    fn the_array_list_names_the_other_elements_that_are_on_longest_first() {
        let mut settings = Settings::default();
        for element in Element::ALL {
            settings.style_mut(element).enabled = false;
        }
        settings.style_mut(Element::ArrayList).enabled = true;
        assert_eq!(line_count(&settings, Element::ArrayList), 1, "never fewer than one line");
        assert_eq!(lines(Element::ArrayList, &view(), &settings)[0].text, "");
        for element in [Element::Fps, Element::Cps, Element::Armor] {
            settings.style_mut(element).enabled = true;
        }
        let text: Vec<String> = lines(Element::ArrayList, &view(), &settings).into_iter().map(|l| l.text).collect();
        assert_eq!(text, ["Armor status", "CPS", "FPS"]);
        assert_eq!(line_count(&settings, Element::ArrayList), 3);
        settings.style_mut(Element::ArrayList).brackets = true;
        assert_eq!(lines(Element::ArrayList, &view(), &settings)[1].text, "[CPS]");
    }

    #[test]
    fn the_item_counter_names_its_item() {
        let mut settings = all_on();
        let text = |s: &Settings, n| lines(Element::ItemCounter, &Views { item_count: n, ..view() }, s)[0].text.clone();
        assert_eq!(text(&settings, 5), "5 Arrows");
        assert_eq!(text(&settings, 1), "1 Arrow");
        settings.counter_item = 1;
        assert_eq!(text(&settings, 3), "3 Golden Apples");
        settings.style_mut(Element::ItemCounter).reverse = true;
        assert_eq!(text(&settings, 3), "Golden Apples 3");
    }

    #[test]
    fn the_inventory_hud_has_a_cell_for_each_slot_and_counts_stacks() {
        let mut settings = Settings::default();
        settings.style_mut(Element::Inventory).enabled = true;
        let mut inventory = vec![Worn::default(); INV_SLOTS];
        inventory[0] = Worn { item: "minecraft:arrow".into(), metadata: 0, durability: None, count: 32 };
        inventory[1] = Worn { item: "minecraft:ender_pearl".into(), metadata: 0, durability: None, count: 1 };
        let shown = Views { inventory, ..view() };
        let counts: Vec<String> = lines(Element::Inventory, &shown, &settings).into_iter().map(|l| l.text).collect();
        assert_eq!(counts.len(), INV_SLOTS);
        assert_eq!((counts[0].as_str(), counts[1].as_str(), counts[2].as_str()), ("32", "", ""));
        assert_eq!(size(&settings, Element::Inventory), [INV_WIDTH, INV_HEIGHT]);
        let text = surface(&shown, &settings, None);
        assert!(text.contains("minecraft:arrow") && text.contains("#iv26"), "every slot has its own item binding");
        // Nothing to compare while the inventory is the same; a changed stack redraws it.
        assert!(!shown.changed(&shown.clone(), Element::Inventory));
        let mut other = shown.clone();
        other.inventory[2].count = 3;
        assert!(shown.changed(&other, Element::Inventory));
    }

    #[test]
    fn combo_says_no_combo_at_zero() {
        let mut settings = all_on();
        let text = |s: &Settings, combo| lines(Element::Combo, &Views { combo, ..view() }, s)[0].text.clone();
        assert_eq!(text(&settings, 3), "3 Combo");
        assert_eq!(text(&settings, 0), "No Combo");
        settings.style_mut(Element::Combo).brackets = true;
        assert_eq!(text(&settings, 0), "[No Combo]");
        settings.style_mut(Element::Combo).labels = false;
        assert_eq!(text(&settings, 0), "[0]");
    }

    #[test]
    fn reach_reads_like_lunars_display_and_hides_with_its_flag() {
        let mut settings = all_on();
        let text = |s: &Settings, hundredths| reach_text(s.style(Element::Reach), hundredths);
        assert_eq!(text(&settings, 296), "2.96 blocks");
        assert_eq!(text(&settings, 0), "0 blocks", "nothing hit yet reads 0, as in Lunar");
        settings.style_mut(Element::Reach).reverse = true;
        assert_eq!(text(&settings, 296), "Blocks: 2.96");
        assert_eq!(text(&settings, 0), "Blocks: 0");
        settings.style_mut(Element::Reach).brackets = true;
        assert_eq!(text(&settings, 310), "[Blocks: 3.10]");
        settings.style_mut(Element::Reach).labels = false;
        assert_eq!(text(&settings, 310), "[3.10]");
        // Hiding at 0 is a change the element has to be redrawn for.
        let shown = view();
        let hidden = Views { reach_hidden: true, ..view() };
        assert!(hidden.changed(&shown, Element::Reach) && !hidden.changed(&shown, Element::Combo));
        // The element's panel follows the visibility binding, and the value says which way.
        let mut doc = String::new();
        open_panel(&mut doc, Element::Reach, &settings, None);
        assert!(doc.contains(REACH_VISIBLE) && doc.contains("#visible"));
        let mut other = String::new();
        open_panel(&mut other, Element::Combo, &settings, None);
        assert!(!other.contains("#visible"), "only Reach display can be hidden");
        let (mut now, mut then) = (String::new(), String::new());
        values(&mut now, &hidden, None, &settings, None);
        values(&mut then, &shown, None, &settings, None);
        assert!(now.contains(r##""#reach_vis":false"##) && then.contains(r##""#reach_vis":true"##), "{now}");
    }

    #[test]
    fn text_lines_read_the_position_right() {
        let settings = all_on();
        let view = view();
        let text = |element| lines(element, &view, &settings).into_iter().map(|l| l.text).collect::<Vec<_>>();
        assert_eq!(text(Element::Fps), ["144 FPS"]);
        assert_eq!(lines(Element::Coords, &view, &settings).iter().map(Line::left).collect::<Vec<_>>(), ["X: -5", "Y: 70", "Z: 33"]);
        assert_eq!(text(Element::Facing), ["Facing North (-Z)"]);
        assert_eq!(text(Element::Chunk), ["Chunk -1, 2", "In chunk 11, 1", "Subchunk 4 (Y 64 to 79)"]);
        assert_eq!(text(Element::Slime), ["Slime chunk: YES"]);
        assert_eq!(text(Element::Monument), ["Guardians: 22 on / 3 off"]);
        assert_eq!(text(Element::Cps), ["7  3 CPS"]);
    }

    #[test]
    fn labels_and_cps_modes_change_the_text() {
        let mut settings = all_on();
        settings.style_mut(Element::Fps).labels = false;
        settings.style_mut(Element::Cps).labels = false;
        let view = view();
        assert_eq!(lines(Element::Fps, &view, &settings)[0].text, "144");
        assert_eq!(lines(Element::Cps, &view, &settings)[0].text, "7  3");
        settings.cps_mode = crate::config::CpsMode::Total;
        assert_eq!(lines(Element::Cps, &view, &settings)[0].text, "10");
        settings.style_mut(Element::Cps).labels = true;
        settings.cps_mode = crate::config::CpsMode::Right;
        assert_eq!(lines(Element::Cps, &view, &settings)[0].text, "3 CPS");
    }

    #[test]
    fn speed_and_playtime_are_written_the_way_they_are_asked_to() {
        // Numbers with their decimals: the value arrives times ten to the decimals.
        assert_eq!(fixed(56, 1), "5.6");
        assert_eq!(fixed(56, 0), "56");
        assert_eq!(fixed(5, 2), "0.05");
        assert_eq!(fixed(7, 3), "0.007");
        assert_eq!(fixed(0, 1), "0.0");
        assert_eq!(fixed(1234, 3), "1.234");
        assert_eq!(fixed(1234, 9), "1.234", "more decimals than the menu offers are cut to three");
        let mut settings = all_on();
        let mut v = view();
        // As before the options: blocks per second, one decimal.
        assert_eq!(lines(Element::Speed, &v, &settings)[0].text, "5.6 BPS");
        settings.speed_unit = 1;
        v.speed = Some(202);
        assert_eq!(lines(Element::Speed, &v, &settings)[0].text, "20.2 km/h");
        settings.speed_decimals = 3;
        v.speed = Some(20_160);
        assert_eq!(lines(Element::Speed, &v, &settings)[0].text, "20.160 km/h");
        settings.style_mut(Element::Speed).brackets = true;
        settings.style_mut(Element::Speed).reverse = true;
        assert_eq!(lines(Element::Speed, &v, &settings)[0].text, "[km/h 20.160]");
        v.speed = None;
        assert_eq!(lines(Element::Speed, &v, &settings)[0].text, "[km/h --]");
        // Clocks.
        assert_eq!(clock_text(0, ClockStyle::Auto), "0:00");
        assert_eq!(clock_text(65, ClockStyle::Auto), "1:05");
        assert_eq!(clock_text(3725, ClockStyle::Auto), "1:02:05");
        assert_eq!(clock_text(3725, ClockStyle::Minutes), "62:05");
        assert_eq!(clock_text(65, ClockStyle::Hours), "0:01:05");
        assert_eq!(clock_text(3725, ClockStyle::Hours), "1:02:05");
        assert_eq!(clock_text(36_000 + 600 + 9, ClockStyle::Hours), "10:10:09");
        // Playtime: m:ss by default, h:mm:ss when asked, and the stopwatch writes either as it needs.
        let mut v = view();
        v.playtime = 3725;
        let mut settings = all_on();
        assert_eq!(lines(Element::Playtime, &v, &settings)[0].text, "62:05");
        settings.playtime_format = 1;
        assert_eq!(lines(Element::Playtime, &v, &settings)[0].text, "1:02:05");
        v.playtime = 65;
        assert_eq!(lines(Element::Playtime, &v, &settings)[0].text, "0:01:05");
        settings.style_mut(Element::Playtime).brackets = true;
        assert_eq!(lines(Element::Playtime, &v, &settings)[0].text, "[0:01:05]");
        assert_eq!(lines(Element::Stopwatch, &view(), &settings)[0].text, "1:02:05");
    }

    #[test]
    fn only_the_elements_that_are_on_are_in_the_document() {
        let mut settings = Settings::default();
        for element in Element::ALL {
            settings.style_mut(element).enabled = false;
        }
        assert!(!any_on(&settings));
        settings.style_mut(Element::Fps).enabled = true;
        settings.style_mut(Element::Chunk).enabled = true;
        let json = parse(&surface(&view(), &settings, Some([427.0, 240.0])));
        assert_eq!(json["screen"], SCREEN);
        let doc = parse(json["document"].as_str().unwrap());
        let controls = doc["hud"]["controls"].as_array().unwrap();
        assert_eq!(controls.len(), 2);
        assert!(controls[0].get("e2").is_some() && controls[1].get("e5").is_some());
        assert_eq!(json["bindings"]["#e2_t0"], "144 FPS");
        assert_eq!(json["bindings"]["#e5_t1"], "In chunk 11, 1");
        assert!(json["bindings"].get("#e0_t0").is_none(), "no values for elements that are off");
    }

    #[test]
    fn the_cps_separator_is_a_bar_that_follows_the_numbers() {
        let mut settings = Settings::default();
        for element in Element::ALL {
            settings.style_mut(element).enabled = false;
        }
        settings.style_mut(Element::Cps).enabled = true;
        let json = parse(&surface(&view(), &settings, Some([427.0, 240.0])));
        let doc = json["document"].as_str().unwrap();
        assert!(doc.contains("\"bar\""), "the bar's node is in the document");
        let at = json["bindings"]["#e1_b0"].as_array().unwrap();
        assert!(at.len() == 2 && at[0].as_f64().unwrap() > 0.0 && at[1].as_f64().unwrap() > 0.0, "{at:?}");
        // Wider numbers on the left push the bar to the right.
        let mut wide = view();
        wide.cps = [77, 3];
        let wide_at = parse(&surface(&wide, &settings, Some([427.0, 240.0])))["bindings"]["#e1_b0"][0].as_f64().unwrap();
        assert_ne!(wide_at, at[0].as_f64().unwrap());
        // Only the Left | Right mode has one.
        settings.cps_mode = crate::config::CpsMode::Left;
        let json = parse(&surface(&view(), &settings, Some([427.0, 240.0])));
        assert!(!json["document"].as_str().unwrap().contains("\"bar\""));
        assert!(json["bindings"].get("#e1_b0").is_none());
    }

    /// Every node named `name` anywhere in a document.
    fn nodes_named<'a>(value: &'a Value, name: &str, found: &mut Vec<&'a Value>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    if key == name {
                        found.push(child);
                    }
                    nodes_named(child, name, found);
                }
            }
            Value::Array(items) => items.iter().for_each(|child| nodes_named(child, name, found)),
            _ => {}
        }
    }

    /// The HUD document as JSON (the surface carries it as a string).
    fn doc_of(settings: &Settings) -> Value {
        parse(parse(&surface(&view(), settings, Some([427.0, 240.0])))["document"].as_str().unwrap())
    }

    fn cps_only() -> Settings {
        let mut settings = Settings::default();
        for element in Element::ALL {
            settings.style_mut(element).enabled = false;
        }
        settings.style_mut(Element::Cps).enabled = true;
        settings
    }

    #[test]
    fn the_cps_bar_has_its_own_colour_and_its_shadow_is_the_texts() {
        let mut settings = cps_only();
        settings.style_mut(Element::Cps).text_color = crate::config::custom_color(0x20_4080);
        let colour = |node: &Value| node["color"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect::<Vec<_>>();
        let close = |a: &[f64], b: &[f64]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3);
        let text = rgba(settings.style(Element::Cps).text_color, 1.0).map(f64::from);
        let doc = doc_of(&settings);
        let mut bars = Vec::new();
        nodes_named(&doc, "bar", &mut bars);
        assert_eq!(bars.len(), 1);
        assert!(close(&colour(bars[0]), &text), "an unpicked bar is the text's colour");
        settings.style_mut(Element::Cps).bar_color = Some(3);
        let doc = doc_of(&settings);
        let (mut bars, mut shadows) = (Vec::new(), Vec::new());
        nodes_named(&doc, "bar", &mut bars);
        nodes_named(&doc, "bar_s", &mut shadows);
        assert!(close(&colour(bars[0]), &rgba(3, 1.0).map(f64::from)), "the picked colour, not the text's");
        let quarter: Vec<f64> = text[..3].iter().map(|v| v * 0.25).collect();
        assert!(close(&colour(shadows[0])[..3], &quarter), "the shadow the numbers cast, whatever the bar's colour");
        // One pixel down and right, like the text's shadow.
        assert_eq!(shadows[0]["offset"], serde_json::json!([1, 1]));
    }

    #[test]
    fn a_border_is_the_boxs_own_inside_border_for_text_elements_only() {
        let mut settings = cps_only();
        let bg = |settings: &Settings| {
            let doc = doc_of(settings);
            let mut found = Vec::new();
            nodes_named(&doc, "bg", &mut found);
            found.first().map(|n| (*n).clone()).expect("the box")
        };
        let node = bg(&settings);
        assert!(node.get("border_width").is_none() && node.get("border_color").is_none(), "off by default");
        let style = settings.style_mut(Element::Cps);
        style.border = true;
        style.border_width = 1.5;
        style.border_color = 3;
        let node = bg(&settings);
        assert_eq!(node["renderer"], "cinnabar_rounded_rectangle");
        assert_eq!(node["border_width"], 1.5);
        let red: Vec<f64> = rgba(3, 1.0).iter().map(|v| f64::from(*v)).collect();
        let got: Vec<f64> = node["border_color"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        assert!(red.iter().zip(&got).all(|(a, b)| (a - b).abs() < 1e-3), "{got:?}");
        assert_eq!(node["border_color"][3], 1, "solid by default, whatever the box's opacity");
        settings.style_mut(Element::Cps).border_opacity = 40.0;
        let alpha = bg(&settings)["border_color"][3].as_f64().unwrap();
        assert!((alpha - 0.4).abs() < 1e-3, "{alpha}");
        settings.style_mut(Element::Cps).border_opacity = 100.0;
        // It grows with the element's size.
        settings.style_mut(Element::Cps).scale = 2.0;
        assert_eq!(bg(&settings)["border_width"], 3);
        // With its background off a bordered box still keeps its padding (the border shows it), and
        // the clear box still has the border.
        settings.style_mut(Element::Cps).background = false;
        assert_eq!(bleed(&settings, Element::Cps), [0.0, 0.0]);
        assert!(bg(&settings).get("border_width").is_some());
        // Armor status has none, whatever its style says.
        let mut armor = Settings::default();
        armor.style_mut(Element::Armor).border = true;
        armor.style_mut(Element::Armor).enabled = true;
        armor.style_mut(Element::Keystrokes).enabled = false;
        armor.style_mut(Element::Cps).enabled = false;
        assert!(!doc_of(&armor).to_string().contains("border_width"));
    }

    #[test]
    fn every_key_of_keystrokes_has_the_border() {
        let mut settings = Settings::default();
        settings.style_mut(Element::Cps).enabled = false;
        let keys = |settings: &Settings| {
            let doc = doc_of(settings);
            let mut found = Vec::new();
            nodes_named(&doc, "bg", &mut found);
            found.into_iter().cloned().collect::<Vec<Value>>()
        };
        let plain = keys(&settings);
        assert!(plain.len() > 4 && plain.iter().all(|n| n.get("border_width").is_none()));
        let style = settings.style_mut(Element::Keystrokes);
        style.border = true;
        style.border_width = 1.0;
        style.border_color = 3;
        style.border_opacity = 50.0;
        style.scale = 2.0;
        let bordered = keys(&settings);
        assert_eq!(bordered.len(), plain.len());
        for node in &bordered {
            assert_eq!(node["border_width"], 2, "grows with the size");
            let alpha = node["border_color"][3].as_f64().unwrap();
            assert!((alpha - 0.5).abs() < 1e-3, "{alpha}");
            // Still bound to the key's own colour (held or not), so the border does not change with it.
            assert!(node["bindings"][0]["binding_name"].as_str().unwrap().ends_with("_bg"));
        }
    }

    #[test]
    fn saturation_reads_like_lunars_number() {
        let mut settings = all_on();
        let text = |view: &Views, s: &Settings| lines(Element::Food, view, s)[0].text.clone();
        settings.style_mut(Element::Food).labels = false;
        assert_eq!(text(&view(), &settings), "4.5");
        assert_eq!(text(&Views { food: Some([20, 0]), ..view() }, &settings), "0", "a whole number has no decimal");
        settings.style_mut(Element::Food).labels = true;
        assert_eq!(text(&view(), &settings), "Saturation 4.5");
        assert_eq!(text(&Views { food: None, ..view() }, &settings), "Saturation --");
    }

    fn food_only() -> Settings {
        let mut settings = Settings::default();
        for element in Element::ALL {
            settings.style_mut(element).enabled = false;
        }
        settings.style_mut(Element::Food).enabled = true;
        settings
    }

    #[test]
    fn the_saturation_overlay_sits_on_the_hunger_bar() {
        let settings = food_only();
        let screen = Some([480.0, 270.0]);
        let eating = Views { eating: Some((8, 128)), pulse: 7, ..view() };
        let json = parse(&surface(&eating, &settings, screen));
        let doc = parse(json["document"].as_str().unwrap());
        let mut found = Vec::new();
        nodes_named(&doc, "sat", &mut found);
        assert_eq!(found.len(), 1);
        let panel = found[0];
        let at: Vec<f64> = panel["offset"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        assert_eq!(at, [322.0, 231.0], "right-most icon: 82 right of the middle, 39 above the bottom");
        let controls = panel["controls"].as_array().unwrap();
        let per_icon = saturation::rects().len();
        assert_eq!(controls.len(), saturation::LAYERS * saturation::SHANKS * per_icon);
        // The last rectangle of the tenth icon of the first layer: nine steps to the left, bound to its colour.
        let last = controls[10 * per_icon - 1].as_object().unwrap().values().next().unwrap();
        assert_eq!(last["renderer"], "cinnabar_rounded_rectangle");
        assert!(last["offset"][0].as_f64().unwrap() > -72.0 && last["offset"][0].as_f64().unwrap() < -60.0, "{}", last["offset"][0]);
        assert_eq!(last["bindings"][0]["binding_name"], "#sy09");
        // Every outline has a colour in the first layer, and the previews have theirs.
        let values = json["bindings"].as_object().unwrap();
        for layer in 0..saturation::LAYERS {
            for icon in 0..saturation::SHANKS {
                assert!(values.contains_key(&format!("#sy{layer}{icon}")), "{layer}{icon}");
            }
        }
        // Saturation 4.5 covers the first icon whole; the steak (hunger 18 to 20) flashes the last
        // icon, and its saturation (to 17.3) the fourth.
        let alpha = |layer: usize, icon: usize| values[&format!("#sy{layer}{icon}")].as_array().unwrap()[3].as_f64().unwrap();
        assert!((alpha(0, 0) - 1.0).abs() < 1e-3 && alpha(1, 0) == 0.0 && alpha(2, 0) == 0.0);
        assert!(alpha(1, 9) > 0.7 && alpha(2, 3) > 0.7 && alpha(2, 9) == 0.0);
    }

    #[test]
    fn the_saturation_overlay_needs_the_screen_and_the_options() {
        let screen = Some([480.0, 270.0]);
        let has = |settings: &Settings, screen: Option<[f32; 2]>| surface(&view(), settings, screen).contains("#sy0");
        let mut settings = food_only();
        assert!(has(&settings, screen));
        assert!(!has(&settings, None), "the bar's place depends on the screen");
        // One layer is ten icons of rectangles, and only the layers wanted are written.
        settings.sat_hunger = false;
        settings.sat_potential = false;
        let doc = surface(&view(), &settings, screen);
        let one = parse(&parse(&doc)["document"].as_str().unwrap().to_owned());
        let mut found = Vec::new();
        nodes_named(&one, "sat", &mut found);
        assert_eq!(found[0]["controls"].as_array().unwrap().len(), saturation::SHANKS * saturation::rects().len());
        assert!(!doc.contains("#sy10") && !doc.contains("#sy20"));
        // The layout moves the bar a pixel.
        settings.sat_layout = 1;
        let moved = parse(&parse(&surface(&view(), &settings, screen))["document"].as_str().unwrap());
        let mut found = Vec::new();
        nodes_named(&moved, "sat", &mut found);
        assert_eq!(found[0]["offset"][1].as_f64(), Some(232.0));
        settings.sat_overlay = false;
        assert!(!has(&settings, screen), "all three off");
        settings.sat_overlay = true;
        settings.style_mut(Element::Food).enabled = false;
        let mut other = all_on();
        other.style_mut(Element::Food).enabled = false;
        assert!(!has(&other, screen), "only while Saturation is on");
    }

    #[test]
    fn the_saturation_overlay_patches_only_when_something_changed() {
        let settings = food_only();
        let screen = Some([480.0, 270.0]);
        let before = Views { eating: Some((8, 128)), pulse: 2, ..view() };
        assert!(!patch_at(&before, &before, &settings, screen).contains("#sy"));
        let flashed = Views { pulse: 3, ..before.clone() };
        assert!(patch_at(&flashed, &before, &settings, screen).contains("#sy"));
        let ate = Views { food: Some([20, 90]), ..before.clone() };
        assert!(patch_at(&ate, &before, &settings, screen).contains("#sy"));
    }

    #[test]
    fn the_crosshair_is_a_panel_with_one_visible_drawing() {
        use crate::crosshair::Target;
        let mut settings = Settings::default();
        for element in Element::ALL {
            settings.style_mut(element).enabled = false;
        }
        assert!(!any_on(&settings));
        settings.crosshair.enabled = true;
        assert!(any_on(&settings), "the crosshair alone is a HUD");
        settings.crosshair.outline = true;
        settings.crosshair.target_scale = 1.4;
        let mut v = view();
        v.crosshair = Some(settings.crosshair.frame(Target::None, false));
        let json = parse(&surface(&v, &settings, None));
        let doc = parse(json["document"].as_str().unwrap());
        let controls = doc["hud"]["controls"].as_array().unwrap();
        assert_eq!(controls.len(), 1);
        let xh = &controls[0]["xh"];
        assert_eq!((xh["type"].as_str(), xh["anchor_from"].as_str()), (Some("panel"), Some("center")));
        // Two drawings (at rest and on target), each with an outline bar and a fill bar for each of the
        // four arms, and every bar takes its colour from a binding.
        let drawings = xh["controls"].as_array().unwrap();
        assert_eq!(drawings.len(), 2);
        let bars = drawings[0]["d0"]["controls"].as_array().unwrap();
        assert_eq!(bars.len(), 8);
        assert_eq!(bars[0]["o0"]["bindings"][0]["binding_name"], CROSSHAIR_EDGE);
        assert_eq!(bars[0]["o0"]["radius"], 0, "the outline is square unless it is switched to round");
        assert_eq!(bars[0]["o0"]["color"], serde_json::json!([1, 1, 1, 0]), "the outline is a border: nothing is drawn under the crosshair's own bar");
        assert!(bars[0]["o0"]["border_width"].as_f64().is_some_and(|w| w > 0.0));
        assert_eq!(bars[0]["o0"]["bindings"][0]["binding_name_override"], "#border_color");
        assert_eq!(bars[4]["f0"]["bindings"][0]["binding_name"], CROSSHAIR_FILL);
        assert_eq!(bars[4]["f0"]["bindings"][0]["binding_name_override"], "#color");
        // The first fill bar is the top arm: 1 wide, 5 long, from 7 above the middle pixel to 2 above it.
        assert_eq!(bars[4]["f0"]["size"], serde_json::json!([1, 5]));
        assert_eq!(bars[4]["f0"]["offset"], serde_json::json!([0, -7]));
        assert_eq!(bars[4]["f0"]["anchor_from"], "top_left");
        assert_eq!(json["bindings"]["#xh0"], true);
        assert_eq!(json["bindings"]["#xh1"], false);
        assert_eq!(json["bindings"]["#xhc"], serde_json::json!([1, 1, 1, 0.9]));
        // On a player: the other drawing and the other colour, and nothing else is sent.
        let mut on = v.clone();
        on.crosshair = Some(settings.crosshair.frame(Target::Player, false));
        let changes = parse(&patch(&on, &v, &settings));
        let changes = changes["binding_changes"].as_object().unwrap();
        assert_eq!(changes["#xh0"], false);
        assert_eq!(changes["#xh1"], true);
        assert_eq!(changes["#xhc"], serde_json::json!([1, 0.33, 0.33, 1]));
        // Nothing changed, nothing sent; and a switched-off crosshair is not in the document.
        assert_eq!(parse(&patch(&v, &v, &settings))["binding_changes"].as_object().unwrap().len(), 0);
        settings.crosshair.enabled = false;
        settings.style_mut(Element::Fps).enabled = true;
        let json = parse(&surface(&view(), &settings, None));
        assert!(!json["document"].as_str().unwrap().contains("xh"));
        assert!(json["bindings"].get("#xhc").is_none());
        // A big ring in two looks would be too many nodes with an outline: it is drawn without one.
        let mut big = Settings::default();
        big.crosshair.enabled = true;
        big.crosshair.outline = true;
        big.crosshair.shape = 3;
        big.crosshair.size = 12.0;
        big.crosshair.target_scale = 1.2;
        let doc = parse(parse(&surface(&view(), &big, None))["document"].as_str().unwrap());
        let bars = doc["hud"]["controls"][0]["xh"]["controls"][0]["d0"]["controls"].as_array().unwrap();
        assert!(bars.iter().all(|b| b.get("o0").is_none()) && bars.len() > 8);
    }

    #[test]
    fn every_element_is_valid_json_when_all_are_on() {
        let settings = all_on();
        let json = parse(&surface(&view(), &settings, Some([427.0, 240.0])));
        let doc = parse(json["document"].as_str().unwrap());
        // One panel per element, and the Saturation overlay over the hunger bar (the screen size is known).
        assert_eq!(doc["hud"]["controls"].as_array().unwrap().len(), Element::COUNT + 1);
        assert_eq!(json["bindings"]["#k0_bg"], serde_json::json!([1, 1, 1, 0.9]), "held W uses the pressed colour");
        assert_eq!(json["bindings"]["#k1_bg"], serde_json::json!([0, 0, 0, 0.5]));
        if std::env::var_os("TECHNIC_DUMP").is_some() {
            std::fs::write("/home/claude/technic-hud.json", surface(&view(), &settings, Some([427.0, 240.0]))).unwrap();
        }
    }

    #[test]
    fn an_element_without_its_box_reaches_every_edge() {
        let mut settings = Settings::default();
        let view = [427.0, 240.0];
        for element in Element::ALL {
            settings.style_mut(element).enabled = false;
        }
        settings.style_mut(Element::Fps).enabled = true;
        settings.style_mut(Element::Fps).background = false;
        for (target, want) in [([-50.0, -50.0], [0.0, 0.0]), ([9999.0, 9999.0], [1.0, 1.0])] {
            let at = place_at(&settings, Element::Fps, view, target);
            assert_eq!(at, want);
            settings.style_mut(Element::Fps).position = Some(at);
        }
        let [x, y, w, h] = visible_rect(&settings, Element::Fps, view);
        assert_eq!([x + w, y + h], view, "the text itself touches the bottom right corner");
        settings.style_mut(Element::Fps).position = Some([0.0, 0.0]);
        let [x, y, ..] = visible_rect(&settings, Element::Fps, view);
        assert_eq!([x, y], [0.0, 0.0], "and the top left");
    }

    #[test]
    fn a_resize_handle_keeps_the_opposite_corner_where_it_is() {
        let mut settings = Settings::default();
        let view = [427.0, 240.0];
        for element in Element::ALL {
            settings.style_mut(element).enabled = false;
        }
        settings.style_mut(Element::Fps).enabled = true;
        settings.style_mut(Element::Fps).position = Some(place_at(&settings, Element::Fps, view, [100.0, 100.0]));
        let r = visible_rect(&settings, Element::Fps, view);
        let at = [[r[0], r[1]], [r[0] + r[2], r[1]], [r[0], r[1] + r[3]], [r[0] + r[2], r[1] + r[3]]];
        assert_eq!(handle_at(r, false, [at[3][0] + 2.0, at[3][1] - 2.0]), Some(3));
        assert_eq!(handle_at(r, true, [r[0] + r[2] * 0.5, r[1] + r[3] * 0.5]), None, "the middle is the element itself");
        assert_eq!(handle_at(r, true, [r[0], r[1] + r[3] * 0.5 + 1.0]), Some(4), "the left side's middle");
        assert_eq!(handle_at(r, false, [r[0], r[1] + r[3] * 0.5]), None, "no side handles unless they show");
        for corner in 0..4 {
            let mut now = settings.clone();
            let drag = Resize::new(&now, Element::Fps, view, corner, at[corner]);
            let (dx, dy) = (if corner % 2 == 0 { -r[2] } else { r[2] }, if corner < 2 { -r[3] } else { r[3] });
            assert!(drag.step(&mut now, view, [at[corner][0] + dx, at[corner][1] + dy]));
            assert_eq!(now.style(Element::Fps).scale, 2.0, "pulled out by its own size: twice as big");
            let after = visible_rect(&now, Element::Fps, view);
            let corners = |r: [f32; 4]| [[r[0], r[1]], [r[0] + r[2], r[1]], [r[0], r[1] + r[3]], [r[0] + r[2], r[1] + r[3]]];
            for axis in 0..2 {
                assert!((corners(after)[3 - corner][axis] - at[3 - corner][axis]).abs() < 0.01, "corner {corner}: the opposite one did not move");
            }
        }
    }

    #[test]
    fn the_handles_are_in_the_layer_only_while_they_show() {
        let mut settings = Settings::default();
        for element in Element::ALL {
            settings.style_mut(element).enabled = false;
        }
        settings.style_mut(Element::Fps).enabled = true;
        let count = |settings: &Settings| {
            let json = parse(&surface(&view(), settings, Some([427.0, 240.0])));
            let doc = parse(json["document"].as_str().unwrap());
            (doc["hud"]["controls"].as_array().unwrap().len(), json["bindings"].get("#hp3").cloned())
        };
        set_handles(None, false);
        assert_eq!(count(&settings), (1, None));
        set_handles(Some([10.0, 20.0, 40.0, 10.0]), false);
        let (controls, corner) = count(&settings);
        set_handles(None, false);
        assert_eq!(controls, 2);
        assert_eq!(corner, Some(serde_json::json!([47, 27])));
    }

    #[test]
    fn stretching_changes_one_axis_and_keeps_the_text_in_the_middle() {
        let mut settings = Settings::default();
        let view = [427.0, 240.0];
        for element in Element::ALL {
            settings.style_mut(element).enabled = false;
        }
        settings.style_mut(Element::Fps).enabled = true;
        settings.style_mut(Element::Fps).position = Some(place_at(&settings, Element::Fps, view, [100.0, 100.0]));
        let [w, h] = size(&settings, Element::Fps);
        settings.style_mut(Element::Fps).stretch = [2.0, 1.5];
        assert_eq!(size(&settings, Element::Fps), [w * 2.0, h * 1.5]);
        settings.style_mut(Element::Fps).stretch = [1.0, 1.0];
        let r = visible_rect(&settings, Element::Fps, view);
        // Pull the right side out by the box's own width: twice as wide, the left side stays.
        let drag = Resize::new(&settings, Element::Fps, view, 5, [r[0] + r[2], r[1] + r[3] * 0.5]);
        assert!(drag.step(&mut settings, view, [r[0] + r[2] * 2.0, r[1] + r[3] * 0.5]));
        assert_eq!(settings.style(Element::Fps).stretch, [2.0, 1.0]);
        let after = visible_rect(&settings, Element::Fps, view);
        assert!((after[0] - r[0]).abs() < 0.01 && (after[2] - r[2] * 2.0).abs() < 0.01 && after[3] == r[3]);
        // Pull the top side up: only the height grows, and the bottom stays.
        let drag = Resize::new(&settings, Element::Fps, view, 6, [after[0] + after[2] * 0.5, after[1]]);
        assert!(drag.step(&mut settings, view, [after[0] + after[2] * 0.5, after[1] - after[3]]));
        assert_eq!(settings.style(Element::Fps).stretch, [2.0, 2.0]);
        let tall = visible_rect(&settings, Element::Fps, view);
        assert!((tall[1] + tall[3] - (after[1] + after[3])).abs() < 0.01);
        // Never narrower than the text needs.
        let drag = Resize::new(&settings, Element::Fps, view, 5, [tall[0] + tall[2], tall[1]]);
        drag.step(&mut settings, view, [0.0, tall[1]]);
        assert_eq!(settings.style(Element::Fps).stretch[0], 1.0);
    }

    #[test]
    fn a_patch_has_only_what_changed() {
        let mut settings = all_on();
        settings.style_mut(Element::Fps).smart = true;
        let before = view();
        let mut after = view();
        after.fps = Some(30);
        after.cps = [8, 3];
        let json = parse(&patch(&after, &before, &settings));
        let changes = json["binding_changes"].as_object().unwrap();
        assert_eq!(changes.len(), 5, "FPS text and colour, the CPS text and its separator bar, and the left mouse key's count: {changes:?}");
        assert!(changes.contains_key("#e1_b0"));
        assert_eq!(changes["#k4_t"], "8 CPS");
        assert_eq!(changes["#e2_t0"], "30 FPS");
        assert_eq!(changes["#e1_t0"], "8  3 CPS");
        assert_eq!(parse(&patch(&before, &before, &settings))["binding_changes"].as_object().unwrap().len(), 0);
    }

    #[test]
    fn elements_are_placed_and_sized_on_their_own() {
        let mut settings = Settings::default();
        settings.style_mut(Element::Fps).scale = 2.0;
        let view = [427.0, 240.0];
        assert_eq!(size(&settings, Element::Fps), [100.0, 36.0], "before it is measured");
        assert_eq!(top_left(&settings, Element::Keystrokes, view), [6.0, 6.0]);
        settings.style_mut(Element::Fps).snap_to(Corner::BottomRight);
        assert_eq!(top_left(&settings, Element::Fps, view), [427.0 - 100.0 - 6.0, 240.0 - 36.0 - 6.0]);
        let at = place_at(&settings, Element::Fps, view, [100.0, 50.0]);
        settings.style_mut(Element::Fps).position = Some(at);
        let corner = top_left(&settings, Element::Fps, view);
        assert!((corner[0] - 100.0).abs() <= 0.5 && (corner[1] - 50.0).abs() <= 0.5, "{corner:?}");
        assert_eq!(place_at(&settings, Element::Fps, view, [-50.0, 999.0]), [0.0, 1.0], "kept on screen");
        // The others did not move.
        assert_eq!(top_left(&settings, Element::Keystrokes, view), [6.0, 6.0]);
    }

    #[test]
    fn frame_buttons_sit_in_the_top_corners_and_take_presses() {
        let frame = [100.0, 50.0, 120.0, 60.0];
        let [gear, cross] = button_places(frame);
        assert_eq!(gear, [101.0, 51.0]);
        assert_eq!(cross, [207.0, 51.0]);
        assert_eq!(button_at(frame, [gear[0] + 4.0, gear[1] + 4.0]), Some(FrameButton::Settings));
        assert_eq!(button_at(frame, [cross[0] + 4.0, cross[1] + 4.0]), Some(FrameButton::Remove));
        assert_eq!(button_at(frame, [160.0, 80.0]), None);
        // The top corner's resize handle is not stolen by a button.
        assert_eq!(button_at(frame, [100.0, 50.0]), None);
        // A frame too small for both gets them in the row above it, not overlapping.
        let tiny = [10.0, 40.0, 14.0, 12.0];
        let [gear, cross] = button_places(tiny);
        assert_eq!(gear[1], cross[1]);
        assert!(cross[0] >= gear[0] + ICON);
    }

    #[test]
    fn dragging_snaps_to_edges_centres_and_other_elements() {
        let mut settings = Settings::default();
        let view = [427.0, 240.0];
        for element in Element::ALL {
            settings.style_mut(element).enabled = false;
        }
        settings.style_mut(Element::Fps).enabled = true;
        settings.style_mut(Element::Coords).enabled = true;
        settings.style_mut(Element::Coords).snap_to(Corner::BottomRight);
        let [fw, _] = size(&settings, Element::Fps);
        // The screen edge itself is no guide: near it, an element still lands on the common gap.
        assert_eq!(snap(&settings, Element::Fps, view, [5.5, 100.0])[0], MARGIN);
        assert_eq!(snap(&settings, Element::Fps, view, [view[0] - fw - 6.5, 100.0])[0], view[0] - fw - MARGIN);
        // Near the screen's left gap: onto it.
        assert_eq!(snap(&settings, Element::Fps, view, [6.8, 100.0])[0], MARGIN);
        // Centred on the screen.
        let centred = (view[0] - fw) * 0.5;
        assert_eq!(snap(&settings, Element::Fps, view, [centred + 0.9, 100.0])[0], centred);
        // Far from every guide: left alone.
        assert_eq!(snap(&settings, Element::Fps, view, [60.0, 60.5]), [60.0, 60.5]);
        // Lined up with the other element's top edge (it sits in the bottom-right corner).
        let top = rect(&settings, Element::Coords, view)[1];
        assert_eq!(snap(&settings, Element::Fps, view, [60.0, top - 0.5])[1], top);
        // Between the lock and the edge of the pull the element is eased in: it never jumps, and
        // moves the same way as the pointer.
        let mut last = f32::MIN;
        for step in 0..120 {
            let x = 60.0 + step as f32 * 0.1;
            let at = snap(&settings, Element::Fps, view, [x, 60.0])[0];
            assert!(at >= last && at - last < 0.5, "{x}: {at} after {last}");
            last = at;
        }
    }

    #[test]
    fn display_options_and_background_switch() {
        let mut settings = all_on();
        let v = view();
        let text = |settings: &Settings, element| lines(element, &v, settings)[0].text.clone();
        settings.style_mut(Element::Fps).brackets = true;
        assert_eq!(text(&settings, Element::Fps), "[144 FPS]");
        settings.style_mut(Element::Fps).reverse = true;
        assert_eq!(text(&settings, Element::Fps), "[FPS 144]");
        settings.style_mut(Element::Cps).reverse = true;
        assert_eq!(text(&settings, Element::Cps), "CPS 7  3");
        settings.style_mut(Element::Cps).labels = false;
        assert_eq!(text(&settings, Element::Cps), "7  3");
        settings.style_mut(Element::Fps).background = false;
        let json = surface(&v, &settings, Some([427.0, 240.0]));
        assert!(json.contains("[0,0,0,0]"), "a clear box stays in the layout");
    }

    #[test]
    fn background_off_and_zero_opacity_look_the_same_and_the_shadow_is_its_own_choice() {
        let v = view();
        let mut off = all_on();
        off.style_mut(Element::Fps).background = false;
        let mut zero = all_on();
        zero.style_mut(Element::Fps).opacity = 0.0;
        assert_eq!(surface(&v, &off, Some([427.0, 240.0])), surface(&v, &zero, Some([427.0, 240.0])));
        // The shadow follows its own switch, whatever the background does.
        for background in [true, false] {
            for shadow in [true, false] {
                let mut s = all_on();
                s.style_mut(Element::Fps).background = background;
                s.style_mut(Element::Fps).shadow = shadow;
                let json = surface(&v, &s, Some([427.0, 240.0]));
                // Every label carries the flag, however many times the JSON is escaped.
                let want = if shadow { "true" } else { "false" };
                let flags: Vec<&str> = json
                    .match_indices("shadow")
                    .map(|(at, _)| json[at + 6..].trim_start_matches(['\\', '"', ':']))
                    .collect();
                assert!(!flags.is_empty() && flags.iter().any(|f| f.starts_with(want)), "{background} {shadow}");
            }
        }
    }

    #[test]
    fn a_fixed_box_keeps_its_width_as_the_count_grows() {
        let settings = all_on();
        assert!(!settings.auto_width(Element::Cps) && settings.auto_width(Element::Fps));
        let mut v = view();
        let single = measure(&v, &settings)[Element::Cps.index()];
        v.cps = [12, 31];
        assert_eq!(measure(&v, &settings)[Element::Cps.index()], single);
        let mut auto = all_on();
        auto.style_mut(Element::Cps).auto_width = Some(true);
        v.cps = [1, 2];
        let small = measure(&v, &auto)[Element::Cps.index()];
        v.cps = [12, 31];
        assert!(measure(&v, &auto)[Element::Cps.index()] > small);
        assert!(single > small, "the fixed box is already wide enough for two digits");
    }

    #[test]
    fn square_corners_set_the_radius_to_zero() {
        let mut settings = all_on();
        for element in Element::ALL {
            settings.style_mut(element).rounded = false;
        }
        let json = surface(&view(), &settings, Some([427.0, 240.0]));
        assert!(!json.contains("radius\\\":1.5") && !json.contains("radius\\\":3,"));
        assert!(json.contains("radius\\\":0,"));
    }

    #[test]
    fn boxes_are_fitted_to_their_text() {
        // Without the host (as here) the table answers, with a little slack against the wider font.
        assert_eq!(text_width("FPS"), 20.0);
        let settings = all_on();
        let mut v = view();
        let widths = measure(&v, &settings);
        assert_eq!(widths[Element::Keystrokes.index()], 0);
        // "144 FPS": 3 digits, a space and F P S, plus 5 each side.
        assert_eq!(widths[Element::Fps.index()], (text_width("144 FPS") + 10.0) as u16);
        v.fps = Some(60);
        assert!(measure(&v, &settings)[Element::Fps.index()] < widths[Element::Fps.index()]);
        set_widths(widths);
        assert_eq!(size(&settings, Element::Fps)[0], f32::from(widths[Element::Fps.index()]));
        set_widths([0; Element::COUNT]);
    }

    #[test]
    fn the_mouse_keys_show_their_clicks_and_the_space_bar_a_bar() {
        let settings = all_on();
        let json = parse(&surface(&view(), &settings, Some([427.0, 240.0])));
        let doc = json["document"].as_str().unwrap();
        assert!(parse(doc)["hud"]["controls"][0]["e0"].is_object());
        assert_eq!(json["bindings"]["#k4_t"], "7 CPS");
        assert_eq!(json["bindings"]["#k5_t"], "3 CPS");
        assert!(doc.contains("\"bar\""));
        let mut off = all_on();
        off.style_mut(Element::Keystrokes).labels = false;
        let json = parse(&surface(&view(), &off, Some([427.0, 240.0])));
        assert!(json["bindings"].get("#k4_t").is_none());
    }

    #[test]
    fn hiding_the_mouse_row_moves_the_space_bar_up() {
        let mut settings = Settings::default();
        assert_eq!(keys(&settings).len(), 7);
        settings.show_mouse = false;
        let shown = keys(&settings);
        assert_eq!(shown.len(), 5);
        assert_eq!(shown[4].name, "Space");
        assert_eq!(shown[4].rect[1], ROW * 2);
        settings.show_space = false;
        assert_eq!(keys(&settings).len(), 4);
    }

    #[test]
    fn active_colours_are_the_players_and_the_other_states_use_the_text_colour() {
        let mut settings = all_on();
        let mut v = view();
        let slime = |s: &Settings, v: &Views| lines(Element::Slime, v, s)[0].color;
        let guards = |s: &Settings, v: &Views| lines(Element::Monument, v, s)[0].color;
        // Defaults: YES green, guardians found blue (the palette has no cyan).
        assert_eq!(slime(&settings, &v), rgba(4, 1.0));
        assert_eq!(guards(&settings, &v), rgba(5, 1.0));
        // The player's pick wins, for each element on its own.
        settings.style_mut(Element::Slime).active_color = Some(6);
        settings.style_mut(Element::Monument).active_color = Some(3);
        assert_eq!(slime(&settings, &v), rgba(6, 1.0));
        assert_eq!(guards(&settings, &v), rgba(3, 1.0));
        // Everything else follows the text colour.
        settings.style_mut(Element::Slime).text_color = 7;
        settings.style_mut(Element::Monument).text_color = 7;
        v.slime = Some(false);
        v.guardians = None;
        assert_eq!(slime(&settings, &v), rgba(7, 1.0));
        assert_eq!(guards(&settings, &v), rgba(7, 1.0));
        v.overworld = false;
        assert_eq!(slime(&settings, &v), rgba(7, 1.0));
        // Status colors off: the text colour even when active.
        v = view();
        settings.style_mut(Element::Slime).smart = false;
        assert_eq!(slime(&settings, &v), rgba(7, 1.0));
    }

    #[test]
    fn chunk_info_lines_can_be_switched_and_the_box_follows() {
        let mut settings = all_on();
        let v = view();
        let n = |s: &Settings| lines(Element::Chunk, &v, s).len();
        let height = |s: &Settings| size(s, Element::Chunk)[1];
        let full = height(&settings);
        assert_eq!(n(&settings), 3);
        assert!(lines(Element::Chunk, &v, &settings)[2].text.contains("(Y 64 to 79)"));
        settings.style_mut(Element::Chunk).sub_range = false;
        assert_eq!(lines(Element::Chunk, &v, &settings)[2].text, "Subchunk 4");
        settings.style_mut(Element::Chunk).show_inchunk = false;
        assert_eq!(n(&settings), 2);
        assert!(height(&settings) < full);
        settings.style_mut(Element::Chunk).show_chunk = false;
        settings.style_mut(Element::Chunk).show_sub = false;
        assert_eq!(n(&settings), 1, "never empty");
        let doc = document(&settings, Some([640.0, 360.0]));
        assert!(doc.contains("e5") && !doc.contains("\\\"t1\\\""));
    }

    #[test]
    fn armor_pieces_switch_and_move_apart() {
        set_armor_live(None);
        let mut settings = all_on();
        let mut v = view();
        v.armor[0] = Worn { item: "minecraft:diamond_helmet".into(), metadata: 0, durability: Some((300, 363)), count: 1 };
        v.armor[2] = Worn { item: "minecraft:iron_leggings".into(), metadata: 0, durability: Some((10, 225)), count: 1 };
        settings.style_mut(Element::Armor).armor_pieces[1] = false;
        assert_eq!(armor_slots(settings.style(Element::Armor)), vec![0, 2, 3]);
        let text = lines(Element::Armor, &v, &settings).into_iter().map(|l| l.text).collect::<Vec<_>>();
        assert_eq!(text, ["300", "10", ""], "a piece that is off is left out");
        let row = size(&settings, Element::Armor);

        // Moving apart starts where the row had the pieces.
        let style = settings.style_mut(Element::Armor);
        armor_split_from_layout(style);
        style.armor_split = true;
        assert_eq!(size(&settings, Element::Armor), row, "nothing jumps when the pieces are let loose");
        let viewport = [854.0, 480.0];
        let before = armor_pieces(&settings, viewport);
        assert_eq!(before.iter().map(|(slot, _)| *slot).collect::<Vec<_>>(), [0, 2, 3]);

        // Dragging one piece leaves the others where they are.
        let target = [before[0].1[0] + 100.0, before[0].1[1] + 60.0];
        armor_move_piece(&mut settings, viewport, 0, target);
        let after = armor_pieces(&settings, viewport);
        let near = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(a, b)| (a - b).abs() <= 1.5);
        assert!(near(after[0].1, [target[0], target[1], before[0].1[2], before[0].1[3]]), "{:?}", after[0]);
        assert!(near(after[1].1, before[1].1), "{:?} {:?}", after[1], before[1]);
        assert!(near(after[2].1, before[2].1), "{:?} {:?}", after[2], before[2]);
        assert_eq!(armor_piece_at(&settings, viewport, [target[0] + 2.0, target[1] + 2.0]), Some(0));
        // Pushing a piece past the screen's edge keeps it on screen and leaves the others alone
        // (they used to run away from it).
        for _ in 0..5 {
            armor_move_piece(&mut settings, viewport, 0, [-80.0, -80.0]);
        }
        let edge = armor_pieces(&settings, viewport);
        assert!(edge[0].1[0] >= 0.0 && edge[0].1[1] >= 0.0, "{:?}", edge[0]);
        assert!(near(edge[1].1, before[1].1), "{:?} {:?}", edge[1], before[1]);
        assert!(near(edge[2].1, before[2].1), "{:?} {:?}", edge[2], before[2]);
        let doc = {
            let mut part = String::new();
            armor_element(&mut part, &settings, Some(viewport));
            part
        };
        assert!(doc.contains("#a0_item") && doc.contains("#a2_item") && !doc.contains("#a1_item"), "{doc}");
        assert!(doc.contains("#a0_pos") && !doc.contains("#a1_pos"), "pieces are placed by bindings: {doc}");
        let patch = armor_position_patch(&settings, viewport);
        assert!(patch.contains("#a0_pos") && patch.contains("#a2_pos") && !patch.contains("#a1_pos"), "{patch}");
        serde_json::from_str::<Value>(&patch).expect("the position patch is JSON");
    }

    #[test]
    fn armor_layout_changes_reflow_pieces_that_are_moved_apart() {
        set_armor_live(None);
        let mut settings = all_on();
        settings.style_mut(Element::Armor).armor_held = true;
        let style = settings.style_mut(Element::Armor);
        armor_split_from_layout(style);
        style.armor_split = true;
        // Every way of changing the layout must leave no two pieces on top of each other.
        let apart = |settings: &Settings| {
            let pieces = armor_pieces(settings, [854.0, 480.0]);
            pieces.iter().enumerate().all(|(i, (_, a))| {
                pieces.iter().skip(i + 1).all(|(_, b)| {
                    a[0] + a[2] <= b[0] + 0.01 || b[0] + b[2] <= a[0] + 0.01 || a[1] + a[3] <= b[1] + 0.01 || b[1] + b[3] <= a[1] + 0.01
                })
            })
        };
        assert!(apart(&settings));
        for (horizontal, side, text) in [(true, 0, 0), (true, 1, 0), (true, 2, 0), (true, 3, 2), (false, 0, 0), (false, 1, 2), (false, 2, 1), (false, 3, 0), (false, 0, 3)] {
            let style = settings.style_mut(Element::Armor);
            style.armor_horizontal = horizontal;
            style.armor_side = side;
            style.armor_text = text;
            armor_reflow(style);
            assert!(apart(&settings), "horizontal={horizontal} side={side} text={text}: {:?}", armor_pieces(&settings, [854.0, 480.0]));
        }
        // Spacing counts while the pieces are moved apart too.
        let tight = size(&settings, Element::Armor);
        let style = settings.style_mut(Element::Armor);
        style.armor_gap = 10.0;
        armor_reflow(style);
        assert!(size(&settings, Element::Armor)[0] > tight[0] || size(&settings, Element::Armor)[1] > tight[1]);
        // In a plain row there is nothing saved to redo.
        let style = settings.style_mut(Element::Armor);
        style.armor_split = false;
        let saved = style.armor_offsets;
        style.armor_horizontal = !style.armor_horizontal;
        armor_reflow(style);
        assert_eq!(style.armor_offsets, saved);
    }

    #[test]
    fn armor_text_sides_work_in_both_orientations() {
        set_armor_live(None);
        let mut settings = all_on();
        for horizontal in [true, false] {
            for side in 0..4 {
                let style = settings.style_mut(Element::Armor);
                style.armor_horizontal = horizontal;
                style.armor_side = side;
                let g = armor_geometry(settings.style(Element::Armor));
                assert!(g.cells.len() >= 2);
                for cell in &g.cells {
                    let (icon, text) = (cell.icon, cell.text_at);
                    match side {
                        0 => assert!(text[0] < icon[0] && text[1] + 1.0 > icon[1] && cell.align == "right", "left"),
                        1 => assert!(text[0] > icon[0] && cell.align == "left", "right"),
                        2 => assert!(text[1] < icon[1] && cell.align == "center", "above"),
                        _ => assert!(text[1] > icon[1] && cell.align == "center", "below"),
                    }
                    // The icon and the number are inside the cell.
                    assert!(icon[0] >= cell.origin[0] && icon[0] + ARMOR_ICON <= cell.origin[0] + cell.extent[0] + 0.01);
                    assert!(icon[1] + ARMOR_ICON <= cell.origin[1] + cell.extent[1] + 0.01);
                    assert!(text[0] + cell.text_size[0] <= cell.origin[0] + cell.extent[0] + 0.01);
                    assert!(text[1] + cell.text_size[1] <= cell.origin[1] + cell.extent[1] + 0.01);
                }
                let along = |c: &ArmorCell| if horizontal { (c.origin[0], c.origin[1]) } else { (c.origin[1], c.origin[0]) };
                assert!(g.cells.windows(2).all(|w| along(&w[0]).0 < along(&w[1]).0 && along(&w[0]).1 == along(&w[1]).1), "one line of pieces");
            }
        }
        // Left and Right make the same size; Above and Below make a taller piece.
        let size_of = |side: usize, settings: &mut Settings| {
            settings.style_mut(Element::Armor).armor_side = side;
            armor_geometry(settings.style(Element::Armor)).cells[0].extent
        };
        assert_eq!(size_of(0, &mut settings), size_of(1, &mut settings));
        assert!(size_of(2, &mut settings)[1] > size_of(0, &mut settings)[1]);
        // Without numbers the icons sit flush, whatever the side.
        for side in 0..4 {
            let style = settings.style_mut(Element::Armor);
            style.armor_side = side;
            style.armor_text = 3;
            let g = armor_geometry(settings.style(Element::Armor));
            assert!(g.cells.iter().all(|c| c.extent == [ARMOR_ICON, ARMOR_ICON] && c.icon == c.origin));
        }
    }

    /// A view with the armor pieces wearing the given (durability left, max) pairs; None: empty slot.
    fn worn_view(pieces: [Option<u16>; ARMOR_SLOTS]) -> Views {
        let mut v = view();
        for (slot, left) in pieces.into_iter().enumerate() {
            if let Some(left) = left {
                v.armor[slot] = Worn { item: "minecraft:golden_helmet".into(), metadata: 0, durability: Some((u32::from(left), 100)), count: 1 };
            }
        }
        v
    }

    #[test]
    fn armor_row_follows_the_numbers_like_lunar() {
        let mut settings = all_on();
        let style = settings.style_mut(Element::Armor);
        style.armor_held = true;
        style.armor_horizontal = true;
        style.armor_side = 0;
        style.armor_text = 0;
        let gap = style.armor_gap as f32;
        let longest = armor_text_width(settings.style(Element::Armor), 0);
        let live = [
            ArmorLive { present: true, chars: 2 },
            ArmorLive { present: true, chars: 3 },
            ArmorLive { present: false, chars: 0 },
            ArmorLive { present: true, chars: 2 },
            ArmorLive { present: true, chars: 3 },
        ];
        set_armor_live(Some(live));
        let g = armor_geometry(settings.style(Element::Armor));
        assert_eq!(g.cells.len(), 4, "an empty slot takes no room");
        for pair in g.cells.windows(2) {
            let end = pair[0].origin[0] + pair[0].extent[0];
            assert!((pair[1].origin[0] - end - gap).abs() < 0.01, "the same space between every piece");
        }
        assert_eq!(g.cells[0].extent[0], 2.0 * ARMOR_DIGIT + ARMOR_NEAR + ARMOR_ICON);
        assert_eq!(g.cells[1].extent[0], 3.0 * ARMOR_DIGIT + ARMOR_NEAR + ARMOR_ICON);
        // The document's boxes stay the same size; each box is pulled back to meet its number.
        let fixed = armor_geometry_static(settings.style(Element::Armor));
        assert_eq!(fixed.cells.len(), 5);
        assert!(fixed.cells.iter().all(|c| c.extent[0] == ARMOR_ICON + ARMOR_NEAR + if c.slot == 4 { 24.0 } else { 18.0 }));
        let at = armor_positions(&settings, Some([640.0, 360.0]));
        assert_eq!(at.len(), 5, "every piece of the document has a place");
        let pos = |slot: usize| at.iter().find(|(s, _)| *s == slot).unwrap().1;
        for cell in &g.cells {
            let box_x = pos(cell.slot)[0] - top_left(&settings, Element::Armor, [640.0, 360.0])[0];
            let live_width = f32::from(live[cell.slot].chars) * ARMOR_DIGIT;
            let longest = armor_text_width(settings.style(Element::Armor), cell.slot);
            // The number ends where it did, just before the icon; the icon is where the row has it.
            assert!((box_x + longest + ARMOR_NEAR - (cell.origin[0] + live_width + ARMOR_NEAR)).abs() < 0.01);
        }
        // Digits do not move the first piece off the row's start, and a number gaining a digit
        // only moves what follows.
        assert!((g.cells[0].origin[0] - 0.0).abs() < 0.01);
        let mut grown = live;
        grown[0].chars = 3;
        set_armor_live(Some(grown));
        let g2 = armor_geometry(settings.style(Element::Armor));
        assert_eq!(g2.cells[1].origin[0] - g.cells[1].origin[0], ARMOR_DIGIT);
        // Columns keep their icons in line whatever the numbers; they only skip empty slots.
        let style = settings.style_mut(Element::Armor);
        style.armor_horizontal = false;
        set_armor_live(Some(live));
        let column = armor_geometry(settings.style(Element::Armor));
        assert_eq!(column.cells.len(), 4);
        assert!(column.cells.iter().all(|c| c.icon[0] == column.cells[0].icon[0]));
        // Moved-apart pieces ignore what they show.
        let style = settings.style_mut(Element::Armor);
        style.armor_horizontal = true;
        armor_split_from_layout(style);
        style.armor_split = true;
        assert_eq!(armor_geometry(settings.style(Element::Armor)).cells.len(), 5);
        let _ = longest;
        set_armor_live(None);
    }

    #[test]
    fn armor_patch_moves_pieces_when_a_number_changes_width() {
        let mut settings = all_on();
        let style = settings.style_mut(Element::Armor);
        style.armor_horizontal = true;
        style.armor_side = 0;
        style.armor_text = 0;
        let before = worn_view([Some(100), Some(91), Some(50), Some(40), None]);
        let doc = surface(&before, &settings, Some([640.0, 360.0]));
        assert!(doc.contains("_pos"), "positions are written with the layer");
        // The same pieces with the same widths: nothing to move.
        let same = worn_view([Some(100), Some(91), Some(50), Some(40), None]);
        let quiet = patch_at(&same, &before, &settings, Some([640.0, 360.0]));
        assert!(!quiet.contains("_pos"), "{quiet}");
        // A number losing a digit moves the pieces after it.
        let after = worn_view([Some(100), Some(9), Some(50), Some(40), None]);
        let moved = patch_at(&after, &before, &settings, Some([640.0, 360.0]));
        assert!(moved.contains("_pos"), "{moved}");
        set_armor_live(None);
    }

    #[test]
    fn armor_drag_steps_are_small_and_stable() {
        set_armor_live(None);
        let mut settings = all_on();
        let viewport = [854.0, 480.0];
        let style = settings.style_mut(Element::Armor);
        armor_split_from_layout(style);
        style.armor_split = true;
        let before = armor_pieces(&settings, viewport);
        let mut drag = ArmorDrag::new(&settings, viewport, Some(0));
        let patch = drag.step(&mut settings, viewport, [-90.0, -90.0], true).expect("moved");
        assert!(patch.len() < 200, "{patch}");
        assert!(drag.step(&mut settings, viewport, [-90.0, -90.0], true).is_none(), "a resting pointer sends nothing");
        let after = armor_pieces(&settings, viewport);
        assert!(after[0].1[0] >= 0.0 && after[0].1[1] >= 0.0, "kept on screen: {:?}", after[0]);
        for n in 1..before.len() {
            assert!(after[n].1.iter().zip(before[n].1).all(|(a, b)| (a - b).abs() <= 1.5), "{:?} {:?}", after[n], before[n]);
        }
        // The whole element follows too.
        let mut whole = ArmorDrag::new(&settings, viewport, None);
        let moved = whole.step(&mut settings, viewport, [200.0, 100.0], false).expect("moved");
        serde_json::from_str::<Value>(&moved).expect("JSON");
        assert_eq!(top_left(&settings, Element::Armor, viewport), [200.0, 100.0]);
        // Dragged far past the screen's corner, the pieces stop where the saved place stops.
        let [w, h] = whole.size;
        let far = whole.step(&mut settings, viewport, [9000.0, 9000.0], false).expect("moved");
        let corner = whole.step(&mut settings, viewport, [viewport[0] - w, viewport[1] - h], false);
        assert!(corner.is_none() || corner.as_deref() == Some(far.as_str()), "{far} {corner:?}");
        let [x, y] = top_left(&settings, Element::Armor, viewport);
        assert!(x + w <= viewport[0] + 1.0 && y + h <= viewport[1] + 1.0, "{x} {y}");
    }

    #[test]
    fn a_separate_piece_resizes_and_stretches_on_its_own() {
        set_armor_live(None);
        let mut settings = all_on();
        let viewport = [854.0, 480.0];
        let style = settings.style_mut(Element::Armor);
        armor_split_from_layout(style);
        style.armor_split = true;
        let before = armor_pieces(&settings, viewport);
        let rect = armor_piece_rect(&settings, viewport, 1).expect("the chestplate shows");
        // A corner resizes the piece, the opposite corner stays.
        let grow = PieceResize::new(&settings, viewport, 1, 3, [rect[0] + rect[2], rect[1] + rect[3]]).expect("piece");
        assert!(grow.step(&mut settings, viewport, [rect[0] + rect[2] + 30.0, rect[1] + rect[3] + 30.0]));
        let after = armor_piece_rect(&settings, viewport, 1).unwrap();
        assert!(after[2] > rect[2] && after[3] > rect[3], "{after:?}");
        assert!((after[0] - rect[0]).abs() < 1.0 && (after[1] - rect[1]).abs() < 1.0, "{after:?} {rect:?}");
        // The other pieces keep their size and place.
        for (slot, r) in &before {
            if *slot != 1 {
                let now = armor_piece_rect(&settings, viewport, *slot).unwrap();
                assert!(now.iter().zip(r).all(|(a, b)| (a - b).abs() <= 1.0), "{slot}: {now:?} {r:?}");
            }
        }
        // A side's middle stretches one axis only.
        let wide = armor_piece_rect(&settings, viewport, 1).unwrap();
        let pull = PieceResize::new(&settings, viewport, 1, 5, [wide[0] + wide[2], wide[1] + wide[3] * 0.5]).expect("piece");
        assert!(pull.step(&mut settings, viewport, [wide[0] + wide[2] + 20.0, wide[1] + wide[3] * 0.5]));
        let stretched = armor_piece_rect(&settings, viewport, 1).unwrap();
        assert!(stretched[2] > wide[2] && (stretched[3] - wide[3]).abs() < 0.01, "{stretched:?} {wide:?}");
        // The sizes are what the document is written from.
        let doc = document(&settings, Some(viewport));
        assert!(doc.contains("inventory_item_renderer"));
        settings.sanitize();
        let [w, h] = settings.style(Element::Armor).armor_sizes[1];
        assert!((ARMOR_MIN_SIZE..=ARMOR_MAX_SIZE).contains(&w) && (ARMOR_MIN_SIZE..=ARMOR_MAX_SIZE).contains(&h));
    }

    #[test]
    fn armor_status_options() {
        set_armor_live(None);
        let mut settings = all_on();
        let mut v = view();
        v.armor[0] = Worn { item: "minecraft:diamond_helmet".into(), metadata: 0, durability: Some((300, 363)), count: 1 };
        v.armor[1] = Worn { item: "minecraft:iron_chestplate".into(), metadata: 0, durability: Some((100, 240)), count: 1 };
        v.armor[2] = Worn { item: "minecraft:iron_leggings".into(), metadata: 0, durability: Some((10, 225)), count: 1 };
        v.armor[4] = Worn { item: "minecraft:arrow".into(), metadata: 0, durability: None, count: 32 };
        let text = |s: &Settings| lines(Element::Armor, &v, s).into_iter().map(|l| l.text).collect::<Vec<_>>();
        assert_eq!(text(&settings), ["300", "100", "10", ""], "held item is off by default");
        settings.style_mut(Element::Armor).armor_text = 1;
        assert_eq!(text(&settings), ["82%", "41%", "4%", ""]);
        settings.style_mut(Element::Armor).armor_text = 2;
        assert_eq!(text(&settings)[0], "300/363");
        settings.style_mut(Element::Armor).armor_held = true;
        assert_eq!(text(&settings).len(), 5);
        assert_eq!(text(&settings)[4], "32", "a stack shows its size");
        settings.style_mut(Element::Armor).armor_text = 3;
        assert!(text(&settings).iter().all(|t| t.is_empty()));
        // Colours follow the status colours chosen.
        settings.style_mut(Element::Armor).armor_text = 0;
        settings.style_mut(Element::Armor).armor_low = 7;
        let colours: Vec<_> = lines(Element::Armor, &v, &settings).iter().map(|l| l.color).collect();
        assert_eq!(colours[0], rgba(4, 1.0));
        assert_eq!(colours[1], rgba(6, 1.0));
        assert_eq!(colours[2], rgba(7, 1.0));
        // The box follows the layout, and nothing is drawn behind it.
        let vertical = size(&settings, Element::Armor);
        settings.style_mut(Element::Armor).armor_horizontal = true;
        let horizontal = size(&settings, Element::Armor);
        assert!(vertical[1] > horizontal[1] && horizontal[0] > vertical[0], "{vertical:?} {horizontal:?}");
        let doc = surface(&v, &settings, Some([427.0, 240.0]));
        let mut part = String::new();
        armor_element(&mut part, &settings, None);
        assert!(!part.contains("cinnabar_rounded_rectangle"), "no box behind the armor");
        assert!(doc.contains("#a4_item"));
        settings.style_mut(Element::Armor).armor_held = false;
        assert!(!surface(&v, &settings, Some([427.0, 240.0])).contains("#a4_item"));
    }

    #[test]
    fn view_angles_write_their_numbers_with_the_decimals_and_a_green_target_pitch() {
        assert_eq!(scaled_text(12000, 2), "120.00");
        assert_eq!(scaled_text(-4000, 2), "-40.00");
        assert_eq!(scaled_text(-5, 2), "-0.05");
        assert_eq!(scaled_text(7, 0), "7");
        let mut settings = Settings::default();
        settings.va_pitch = true;
        settings.va_target = true;
        settings.va_decimals = 2;
        let view = Views { va: Some(VaView { yaw: 12000, pitch: -4000, speed: 0, altitude: 6400 }), ..Views::default() };
        let rows = va_rows(&view, &settings);
        assert_eq!(rows[0].1, "Yaw");
        assert_eq!(rows[1].0, "-40.00\u{b0}");
        assert!(rows[1].2, "-40 is inside the target");
        settings.va_reverse = true;
        assert_eq!(va_rows(&view, &settings)[0].1, "Pitch");
        assert_eq!(va_line_count(&settings), 2);
    }
}
