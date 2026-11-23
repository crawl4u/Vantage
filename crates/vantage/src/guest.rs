//! Host glue: reads the player and the terrain, runs the menu, and draws everything.
//!
//! A frame has 100k fuel, and lowering a terrain region into the mod costs most of it, so the
//! terrain is read one small slab at a time and every heavy step is spread over frames.

use crate::blocks::{self, CLEAR, EMPTY_SLOT, Info as BlockInfo, KNOWN, LIQUID};
use crate::config::{Element, LATER_TOGGLES, MORE_TOGGLES, Settings, TOGGLES};
use crate::cps::{Counter, Pairer};
use crate::hitbox;
use crate::hud::{self, Guardians, Views};
use crate::monument::{self, Bounds};
use crate::panel::{Effect, PanelState};
use crate::shapes;
use crate::potion;
use crate::outline;
use crate::saturation;
use crate::slime;
use crate::timers;
use crate::zoom::{self, Zoomer};
use crate::host::{action_filter, camera, clock, environment, facts, gameplay, hud as host_hud, input, item_use, panel, player_state, presentation, render, screen, settings, sound_handling, teleport_handling, terrain, touch};
use crate::host::{drawing as draw, masks};
use mod_api::handled_player_mod::Guest;
use std::cell::RefCell;
use std::collections::HashMap;

/// Terrain queries kept in flight; each one arrives as one region to lower and decode.
const IN_FLIGHT: usize = 2;
/// Fuel given to the light flood and the spot search in a frame with nothing else heavy.
/// Slime chunks are computed a few per frame (about 5500 fuel each) and kept.
const SLIME_PER_FRAME: usize = 3;
const SLIME_REACH: i32 = 3;
const SLIME_SIDE: usize = (SLIME_REACH * 2 + 1) as usize;
/// New block types classified per frame (string table lookups, a few thousand fuel each).
const CLASSIFY_PER_FRAME: usize = 6;
/// How long text must stay shorter before its box shrinks, in seconds: long enough that a
/// count flickering between 9 and 10 does not rebuild the layer every frame, short enough to
/// feel immediate.
const WIDTH_SHRINK_DELAY: f64 = 0.2;
/// Recheck the monument's columns this often, in seconds.
const COLUMN_RECHECK: f64 = 3.0;
/// Settings are written this long after the last change, or when the menu closes: writing them
/// (every value of every element, as text) on each step of a slider drag cost most of a frame.
const SAVE_DELAY: f64 = 0.4;
/// Seconds the move bar waits after an element is let go before it fades back in, so it does not
/// flicker between two quick drags.
const BAR_RETURN_DELAY: f32 = 0.35;
/// Where the move bar's Reset and Done buttons sit, from the middle of the top edge (GUI pixels:
/// left, right, top, bottom), a little wider than the buttons themselves so a press that just
/// misses one still counts as a press on it.
const DONE_ZONE: [f32; 4] = [-31.0, 111.0, 5.0, 25.0];
/// Seconds between layer rewrites while Armor status is resized (it is the heaviest element).
const ARMOR_RESIZE_STEP: f64 = 0.05;
/// Seconds between two looks at whether the player changed their key bindings (the host answers
/// "unchanged" cheaply, but it still counts against the callback's controls reads).
const BINDS_RECHECK: f64 = 0.5;

/// Adds the keys that are not in `list` yet.
fn push_new(list: &mut Vec<String>, keys: &[String]) {
    for key in keys {
        if !list.contains(key) {
            list.push(key.clone());
        }
    }
}

pub struct Mod;

/// A hasher for runtime IDs: the default SipHash costs hundreds of fuel per lookup.
/// Seconds after a use or a swing within which the game would hold back the next use (200 ms, with margin).
const ITEM_DELAY_WINDOW: f64 = 0.25;

#[derive(Clone, Copy, Default)]
struct IdHash;

#[derive(Default)]
struct IdHasher(u64);

impl std::hash::BuildHasher for IdHash {
    type Hasher = IdHasher;
    fn build_hasher(&self) -> IdHasher {
        IdHasher(0)
    }
}

impl std::hash::Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    fn write_u32(&mut self, value: u32) {
        self.0 = u64::from(value).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Job {
    Base { tile: [i32; 2] },
    Column { index: usize, corner: [i32; 2] },
}

impl Job {
    fn query(self) -> terrain::RegionQuery {
        let (min, size) = match self {
            Job::Base { tile } => (
                [tile[0], monument::BASE_LOW, tile[1]],
                [monument::TILE, monument::BASE_HIGH - monument::BASE_LOW, monument::TILE],
            ),
            Job::Column { index, corner } => {
                let [x, z] = monument::columns(corner)[index];
                ([x, 59, z], [1, 3, 1])
            }
        };
        terrain::RegionQuery {
            min: terrain::Position { x: min[0], y: min[1], z: min[2] },
            max: terrain::Position { x: min[0] + size[0], y: min[1] + size[1], z: min[2] + size[2] },
            geometry: terrain::Geometry::COLLISION,
        }
    }
}

/// A received region still being classified, over as many frames as it takes.
struct Pending {
    job: Job,
    region: terrain::Region,
}

/// Handles of the shapes uploaded once.
struct Shapes {
    wall: draw::Handle,
    post: draw::Handle,
    ring: draw::Handle,
    footprint: draw::Handle,
    column: draw::Handle,
    cube: draw::Handle,
    ray: draw::Handle,
    /// A spawner's spawn area (9 × 3 × 9) and its activation circle (16 blocks).
    spawn_area: draw::Handle,
    activation: draw::Handle,
}

#[derive(Default)]
struct MonumentState {
    /// Where the search is centred.
    center: Option<[i32; 2]>,
    tiles: Vec<[i32; 2]>,
    next_tile: usize,
    bounds: Bounds,
    attempts: u8,
    corner: Option<[i32; 2]>,
    blocked: [Option<bool>; 25],
    next_column: usize,
    checked_at: f64,
    /// Searched everything without finding one.
    none: bool,
}

#[derive(Default)]
struct State {
    settings: Settings,
    panel: PanelState,
    /// The Mods page on the Vantage GUI toolkit.
    gui: crate::gui::GuiMenu,
    zoomer: Zoomer,
    /// The key the mouse wheel is claimed for (the zoom key), and when it was last sent.
    scroll_claim: Option<String>,
    scroll_claim_at: f64,
    /// Wheel movement not yet worth a whole step.
    scroll_acc: f32,
    /// The keys last written to the game's Freelook and Cycle perspective bindings, and whether the host refused one.
    bind_sent: [String; 2],
    bind_refused: bool,
    /// The Glass style last handed to the host (None before the first call, which sends Normal's empty list too so no rules from an earlier run stay), and the
    /// style the host refused, so a refusal is told once and tried again only when the style changes.
    glass_applied: Option<(usize, usize)>,
    glass_refused: Option<(usize, usize)>,
    /// Snaplook in Toggle Press Mode: 0 off, 1 third person from behind, 2 from the front.
    snap_toggled: u8,
    /// Reading the game's own bindings: the last complete revision, the page being collected, and when each happened.
    bind_after: Option<u64>,
    bind_collecting: Option<(u64, u32, Vec<(String, String)>)>,
    bind_polled_at: f64,
    bind_written_at: f64,
    clock: f64,
    /// The option keys last reserved.
    reserved: Option<Vec<String>>,
    /// The on-screen buttons for touch controls are set.
    touch_set: bool,
    /// Hitbox drawings: wanted now, and what each retained handle shows.
    /// Hitboxes, one per nearby actor, each drawn anchored to its actor so the host moves it
    /// every frame with no delay.
    hits: Vec<Hit>,
    /// Box sizes read from the actor list, by runtime id: offset from the feet and size.
    hit_sizes: HashMap<u64, ([f32; 3], [f32; 3], bool), IdHash>,
    /// The hitbox colours the drawings were made with; a change redraws them.
    hit_colors: (usize, usize),
    hits_recolor: bool,
    /// The screen size a placed HUD was laid out for.
    viewport: Option<[f32; 2]>,
    dimension: i32,
    feet: [i32; 3],
    have_world: bool,

    shapes: Option<Shapes>,
    shapes_failed: bool,
    /// Instances of each drawing group, and whether they need sending again.
    borders: Pool,
    slime: Pool,
    lights: Pool,
    spawners: Pool,
    /// Spawner positions drawn, and whether the finder is on in the Nether.
    spawners_shown: Option<(bool, Vec<[i32; 3]>, u32)>,
    guardians: Pool,
    borders_key: Option<(i32, [i32; 3], u8, bool, u32)>,
    slime_key: Option<(i32, [i32; 2], i32, u32)>,
    lights_key: Option<(bool, [i32; 2], i32, u32)>,
    /// Your own hitbox and look line, drawn at your feet in third person.
    own_box: Option<draw::Handle>,
    /// Block outline: one uploaded line per axis and one quad per face (drawn stretched), the block they are drawn for,
    /// its selection boxes (a hair larger than the block) and the merged outline's segments, and the clocks of the
    /// shape read and of the upload retry.
    bo_edges: Vec<draw::Handle>,
    bo_faces: Vec<draw::Handle>,
    bo_block: Option<[i32; 3]>,
    bo_boxes: Vec<[f64; 6]>,
    bo_segments: Vec<outline::Segment>,
    bo_shape_known: bool,
    /// The stamp of the last shape read (the host keeps a terrain read only while it is repeated every frame).
    bo_stamp: Option<terrain::Stamp>,
    bo_upload_at: f64,
    bo_drawn: bool,
    own_ray: Option<draw::Handle>,
    /// Exact feet position, for your own hitbox.
    feet_exact: [f64; 3],
    guardian_key: Option<([i32; 2], [Option<bool>; 25], u32)>,
    frames: u32,
    fps: Option<u32>,
    fps_checked: f64,
    /// Round-trip time to the server, read with the FPS.
    ping: Option<u32>,
    ping_checked: f64,
    /// Reach and combo from the host's combat evidence: the tracker, the last event read, and
    /// whether the history from before the element was switched on has been read past.
    combat: crate::combat::Tracker,
    combat_after: u64,
    combat_primed: bool,
    /// Highlight Players in Range: the host mask, the actor references it holds (runtime ID and
    /// handle), the colour it was styled with, and whether the host refused it (said once in chat).
    hl_mask: Option<draw::Handle>,
    hl_handles: Vec<(u64, draw::Handle)>,
    hl_colour: Option<usize>,
    hl_refused: bool,
    /// Damage Indicator: the health seen and the numbers alive, the uploaded digit strokes, whether the
    /// host refused them (nothing more is tried) and whether a group of numbers is on screen.
    dmg: crate::damage::Tracker,
    dmg_glyphs: Vec<draw::Handle>,
    dmg_failed: bool,
    dmg_drawn: bool,
    /// TNT countdown: each lit TNT's runtime id with the clock when it was first seen, what the last read found (position by id), and the drawing's state.
    tnt_seen: Vec<(u64, f64)>,
    tnt_actors: Vec<(u64, [f64; 3])>,
    tnt_drawn: bool,
    tnt_failed: bool,
    /// Target HUD: who is shown, when it was last aimed at (`clock`), and the shown name's runtime ID.
    target: Option<hud::TargetView>,
    target_id: u64,
    target_seen: f64,
    /// WAILA: the block aimed at, its shown name, and when its state was last asked for (`clock`).
    waila: Option<crate::waila::WailaView>,
    waila_block: Option<[i32; 3]>,
    waila_seen: f64,
    /// WAILA: the mob aimed at (and when it was last seen), and where the eye is (for the distance).
    waila_mob: Option<crate::waila::WailaView>,
    waila_mob_seen: f64,
    waila_eye: Option<[f32; 3]>,
    waila_stamp: Option<terrain::Stamp>,
    /// Hunger points and saturation tenths, read four times a second while Food is on.
    food: Option<[u32; 2]>,
    food_checked: f64,
    /// The world's day count, read once a second while Day counter is on.
    day: Option<u32>,
    day_checked: f64,
    /// The server's sidebar, as the Scoreboard shows it, and when it was last read.
    sidebar: Option<hud::Sidebar>,
    /// Waypoints: the host's id of the world, and the lines the element shows now.
    world_id: String,
    wp_rows: Vec<crate::waypoints::Row>,
    /// Audio subtitles: the sounds heard lately and the lines they make.
    subs: crate::subtitles::Subtitles,
    /// Markers: the marks placed with the key, each with the `clock` it ends at (infinity: never). Drawn like waypoints.
    markers: Vec<(crate::waypoints::Waypoint, f64)>,
    sub_rows: Vec<crate::subtitles::Row>,
    /// Whether the waypoints' world drawing is on screen or was refused.
    wp_drawn: bool,
    wp_failed: bool,
    /// Death waypoints: the player was seen dead (so one death marks one waypoint).
    wp_dead: bool,
    /// Seconds (the mod's clock) before the waypoints try drawing again after a refusal.
    wp_wait: f64,
    /// The waypoint beam's mesh, uploaded on first use.
    wp_beam: Option<draw::Handle>,
    /// The marker's diamond icon: its rim and its inside (flat, unit size, facing the camera when placed).
    wp_diamond: Option<[draw::Handle; 2]>,
    sidebar_checked: f64,
    /// Speed in blocks per second, from the position now and then (see `timers.rs`).
    speed: timers::SpeedMeter,
    /// The Stopwatch (its time and whether it counts) and Playtime (all of it, and the part in a
    /// world), and the clock at the last step of both.
    stopwatch: timers::Stopwatch,
    playtime: timers::Playtime,
    timer_clock: f64,
    facing: Option<u8>,
    yaw: Option<u16>,

    /// Slime chunk answers around `slime_center`, row by row in X.
    slime_center: [i32; 2],
    slime_known: Vec<Option<bool>>,

    in_flight: Vec<Job>,
    /// A region whose block types are still being classified, a few per frame.
    pending: Option<Pending>,
    palette: HashMap<u32, BlockInfo, IdHash>,
    registry: u64,
    monument: MonumentState,

    /// The last drawing error, shown on the card so a missing permission is visible.
    draw_error: Option<String>,
    shown_view: Option<Views>,
    /// The settings revision and screen size the HUD layer was built for.
    hud_shape: Option<(u64, Option<[f32; 2]>, [u16; Element::COUNT])>,
    /// The widths the HUD's text boxes are fitted to: they grow at once and shrink after a
    /// moment, so a count flickering between 9 and 10 does not rebuild the layer every frame.
    hud_widths: Option<[u16; Element::COUNT]>,
    hud_widths_revision: u64,
    hud_shrink_since: Option<f64>,
    hud_error: Option<String>,
    /// The drawing error already reported in chat.
    draw_reported: Option<String>,
    settings_revision: u64,
    /// Keystrokes and CPS: held keys, the click counters and their last counts.
    held: [bool; 7],
    left: Counter,
    right: Counter,
    /// Pairs a click's key press with the game's resolved action (they come a frame apart), per side.
    click_pairs: [Pairer; 2],
    cps: [u32; 2],
    /// CPS statistics over the session (see `cps::Stats`).
    cps_stats: crate::cps::Stats,
    /// The slime chunk answer for the chunk you stand in, for the HUD readout.
    hud_slime: Option<([i32; 2], bool)>,
    /// While dragging: the element held and where the pointer holds it, from its top-left corner.
    grab: Option<(Element, [f32; 2])>,
    /// Armor status with separate pieces: the slot being dragged.
    grab_piece: Option<usize>,
    /// When the Armor status layer was last rewritten by a drag (it is the heaviest element to write).
    drag_applied: f64,
    /// What an Armor status drag keeps from when it was picked up.
    armor_drag: Option<hud::ArmorDrag>,
    /// Potion status with its effects moved apart: the effect being dragged.
    potion_drag: Option<hud::PotionDrag>,
    /// A corner of a moved-apart effect being dragged, and the effect the frame is on.
    potion_resize: Option<hud::PotionResize>,
    potion_sel: i32,
    /// A resize handle being dragged.
    resize: Option<hud::Resize>,
    /// A resize handle of a separate armor piece being dragged, and the piece the handles are on.
    piece_resize: Option<hud::PieceResize>,
    /// Whether the layer was last written with the centre lines.
    guides_sent: bool,
    armor_sel: usize,
    /// Where the editor's resize handles were last drawn (none: not drawn).
    handles_sent: Option<([f32; 4], bool)>,
    /// Seconds left before the move bar fades back in after an element was let go.
    bar_hold: f32,
    /// The worn armour for the Armor status element, when it was last read, and the reader.
    armor: [hud::Worn; hud::ARMOR_SLOTS],
    armor_checked: f64,
    /// The active potion effects for Potion status and when they were last read, what the host said
    /// about each effect (name and icon texture), and what the layout was last made from.
    potions: Vec<potion::Row>,
    potions_checked: f64,
    potion_facts: HashMap<i32, (String, String)>,
    potion_key: u64,
    /// How many of the Item counter's item were in the inventory at the last read.
    item_count: u32,
    potion_count: u32,
    totems: u32,
    clock_time: Option<u32>,
    clock_checked: f64,
    /// View angles: the last yaw and pitch read (degrees), and its own speed meter (height counts).
    va_angles: Option<[f64; 2]>,
    va_meter: timers::SpeedMeter,
    /// The main inventory at the last read, for the Inventory HUD.
    inventory: Vec<crate::hud::Worn>,
    armor_reader: crate::armor::Reader,
    /// The whole menu was sent this frame: leave the rest of the frame's fuel alone.
    skip_rest: bool,
    /// A tab changed: its page is sent at the start of the next frame (`send_tab`).
    tab_build_due: bool,
    /// The Mods grid's values go out at the start of the next frame (`send_patch`): a click there (a card's toggle) already
    /// spent its part of this one, and the grid's patch is the biggest single piece of work the menu asks for.
    patch_due: bool,
    /// Until when a menu that closes by itself after a page change is opened again (the host drops the panel on some page swaps).
    tab_grace: Option<f64>,
    /// Each HUD element's piece of the layer document, kept while its inputs do not change.
    doc_cache: hud::DocCache,
    /// The keys the mod asks the game to report, until a setting that could change them.
    bound_cache: Option<Vec<String>>,
    /// The player's own keys for movement, attack, use and jump (what Keystrokes and CPS watch).
    binds: crate::binds::Binds,
    binds_collector: crate::binds::Collector,
    /// When the bindings were last asked for.
    binds_checked: f64,
    /// The menu was open at the last frame: the left button is then read for its scroll bar.
    panel_open_seen: bool,
    /// When the settings are due to be written (they changed and have not been saved yet).
    save_due: Option<f64>,
    /// The menu was sent values this frame, so a full HUD rebuild waits a frame.
    menu_busy: bool,
    /// True while the game has the keyboard (focused, no menu or panel): the host only accepts control modes then.
    in_gameplay: bool,
    /// Frames the HUD rebuild has been held back for the menu in a row.
    hud_defers: u8,
    /// Whether the host has been told fullbright is on, and whether it refused (no permission).
    fullbright_sent: bool,
    /// When fullbright was last sent (`clock` seconds).
    fullbright_at: f64,
    /// The hit colour last sent to the host (none: native), when, and whether it was refused.
    hurt_sent: Option<[f32; 4]>,
    hurt_at: f64,
    /// What the gameplay mods last told the host, when, and whether the host refused.
    gameplay_sent: GameplayWanted,
    gameplay_at: f64,
    gameplay_refused: bool,
    /// The mod's own toggle sprint / toggle sneak latches (they survive an inventory, unlike the host's).
    sprint_latch: bool,
    sneak_latch: bool,
    /// Whether the admitted controls (touch buttons included) held sprint / sneak last frame, for edge detection.
    admitted_sprint: bool,
    admitted_sneak: bool,
    intents_sent: bool,
    intents_refused: bool,
    cursor_screen: Option<String>,
    cursor_until: f64,
    hurt_refused: bool,
    fullbright_refused: bool,
    /// The item delay fix was refused for want of the permission (said once).
    item_fix_refused: bool,
    /// When the player last pressed use or attack, for the item delay fix.
    last_use_or_swing: Option<f64>,
    /// Whether the host has been told to hide the scoreboard and the game's crosshair, and whether it refused.
    hidden_sent: (bool, bool, bool, bool),
    /// The host has been told the list at least once since this copy of the mod started: the host
    /// keeps the list, so a crosshair hidden by an earlier copy must be given back even when this
    /// copy has nothing to hide.
    hidden_synced: bool,
    /// The last menu update the host refused (said once).
    patch_error: Option<String>,
    hidden_refused: bool,
    /// Whether the game's position line is hidden (what the host holds), and whether it refused once.
    position_hidden: bool,
    /// What the Crosshair is on (read a few times a second while it is on), and whether a bow or
    /// crossbow is in the hand (from the inventory read).
    xh_target: crate::crosshair::Target,
    holding_ranged: bool,
}

thread_local! {
    /// The menu's life: set and opened by the menu key, dropped after it closes.
    static CORE: RefCell<crate::menu_link::Menu> = RefCell::new(crate::menu_link::Menu::new());

    static STATE: RefCell<State> = RefCell::new(State::default());

    /// What the event handlers do, copied from the settings every frame (the handlers cannot reach `STATE`'s host calls
    /// and run outside the frame), and the CPS limiter's memory.
    static HANDLERS: std::cell::Cell<crate::handlers::Config> = const { std::cell::Cell::new(crate::handlers::Config { no_cam_reset: false, no_swing_sounds: false, cps_limit: None, subtitles: false }) };
    /// The sounds the host announced since the frame last looked (Audio subtitles): the name and where it plays.
    static HEARD: RefCell<Vec<(String, Option<[f64; 3]>)>> = const { RefCell::new(Vec::new()) };
    /// Attacks the limiter let through since the CPS element last looked (it shows these while the limiter is on).
    static LIMITED_CLICKS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// Attack and use presses the game resolved since the CPS element last looked, from any device (touch included).
    static ACTION_CLICKS: std::cell::Cell<[u32; 2]> = const { std::cell::Cell::new([0, 0]) };
    static LIMITER: RefCell<crate::handlers::CpsLimiter> = RefCell::new(crate::handlers::CpsLimiter::default());
}

/// What the gameplay mods want the host to have (compared with what it was last told).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct GameplayWanted {
    modes: Option<[bool; 3]>,
    time: Option<u32>,
    weather: Option<usize>,
    /// Hurt camera: remove the shake, strength in percent, fixed direction.
    hurt: Option<(bool, u32, bool)>,
    /// Nametags: no background, scale in percent, shadow, own name.
    names: Option<(bool, u32, bool, bool)>,
}

/// Whether Vantage's own block outline is switched on, so the client can drop its default one.
pub fn block_outline_on() -> bool {
    STATE.with_borrow(|state| state.settings.gameplay.bo_on)
}

/// What the event handlers do right now, so the client can skip building contexts nobody wants.
pub fn handler_config() -> crate::handlers::Config {
    HANDLERS.get()
}

impl Guest for Mod {
    fn init() {
        STATE.with_borrow_mut(|state| {
            state.settings = match settings::load() {
                Ok(text) => Settings::from_json(&text),
                Err(_) => Settings::default(),
            };
            state.apply(Effect { republish: true, rebind: true, ..Effect::default() });
        });
    }

    fn frame() {
        STATE.with_borrow_mut(State::frame);
    }

    // The ordinary screen and key events are not used: Vantage has its own menu.
    fn screen_changed(_layout: Option<screen::ScreenLayout>) {}
    fn action(_id: String, _collection_index: Option<u32>) {}
    fn secondary_action(_id: String, _collection_index: Option<u32>) {}
    fn scrolled(_delta: f64, _x: f64, _y: f64, _modifiers: screen::Modifiers) {}
    fn text_changed(_control: String, _text: String) {}
    fn key(_id: String, _hovered: Option<mod_api::handled_player_mod::cinnabar::session::items::Stack>, _row: Option<(String, u32)>) {}
    fn data_changed(_sources: Vec<mod_api::handled_player_mod::DataSource>) {}
    fn view_closed() {}

    /// The CPS limiter: one verdict per transition (`handlers::CpsLimiter`); everything passes while it is off.
    fn filter_actions(frame: action_filter::ActionFrame) -> action_filter::ActionVerdict {
        let limit = HANDLERS.get().cps_limit;
        let transitions: Vec<crate::handlers::Transition> = frame
            .transitions
            .iter()
            .map(|t| crate::handlers::Transition {
                attack: matches!(t.action, action_filter::FilteredAction::Attack),
                edge: match t.kind {
                    action_filter::TransitionKind::Press => crate::handlers::Edge::Press,
                    action_filter::TransitionKind::Release => crate::handlers::Edge::Release,
                    action_filter::TransitionKind::Repeat => crate::handlers::Edge::Repeat,
                },
            })
            .collect();
        let accepted = LIMITER.with_borrow_mut(|limiter| limiter.verdicts(frame.epoch, frame.time_us, limit, &transitions));
        // Touch buttons are not keys, so the CPS element also counts the presses the game itself resolved.
        let mut clicks = ACTION_CLICKS.get();
        for t in &frame.transitions {
            if matches!(t.kind, action_filter::TransitionKind::Press) {
                let side = usize::from(!matches!(t.action, action_filter::FilteredAction::Attack));
                clicks[side] = clicks[side].saturating_add(1);
            }
        }
        ACTION_CLICKS.set(clicks);
        if limit.is_some() {
            LIMITED_CLICKS.set(LIMITED_CLICKS.get().saturating_add(crate::handlers::accepted_attacks(&transitions, &accepted)));
        }
        action_filter::ActionVerdict { accepted, release: action_filter::Held { attack: false, item_use: false } }
    }

    /// No swing sounds: cancels the sound of an attack that hit nothing.
    fn handle_sound(mut ctx: sound_handling::SoundContext) -> sound_handling::SoundContext {
        if HANDLERS.get().no_swing_sounds && crate::handlers::is_swing_sound(&ctx.name) {
            ctx.cancelled = true;
        }
        if HANDLERS.get().subtitles && !ctx.cancelled {
            let at = ctx.position.as_ref().map(|p| [f64::from(p.x), f64::from(p.y), f64::from(p.z)]);
            HEARD.with(|heard| {
                let mut heard = heard.borrow_mut();
                if heard.len() < 48 {
                    heard.push((ctx.name.clone(), at));
                }
            });
        }
        ctx
    }

    /// No cam reset: the view stays where it is when the server turns or moves you.
    fn handle_teleport(mut ctx: teleport_handling::TeleportContext) -> teleport_handling::TeleportContext {
        if HANDLERS.get().no_cam_reset {
            ctx.rotation = ctx.presented;
        }
        ctx
    }
}

/// The selection boxes of the one block a 1x1x1 region holds (layer 0), block-local; None while the block is not
/// loaded. A block without shape data is a full cube unless it has no layer at all (air).
fn selection_boxes(region: &terrain::Region) -> Option<Vec<[f64; 6]>> {
    if region.loaded.first().is_none_or(|word| word & 1 == 0) {
        return None;
    }
    let index = *region.states.first()?;
    if index == 65535 {
        return Some(Vec::new());
    }
    let state = region.palette.get(usize::from(index))?;
    let known = |shape: &terrain::ResolvedShape| match shape {
        terrain::ResolvedShape::Known(index) => Some(*index),
        terrain::ResolvedShape::Unavailable => None,
    };
    let shape = match &state.selection {
        terrain::ShapeBinding::Fixed(shape) => known(shape),
        terrain::ShapeBinding::PerCell => region.overrides.iter().find(|o| o.slot == 0).and_then(|o| o.selection.as_ref().and_then(known)),
        terrain::ShapeBinding::NotRequested => None,
    };
    let Some(shape) = shape else {
        let air = state.identifier.as_deref().is_none_or(|id| id.is_empty() || id.ends_with("air") || id.contains("water") || id.contains("lava"));
        return Some(if air { Vec::new() } else { vec![[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]] });
    };
    let shape = shape as usize;
    let (start, end) = (*region.shape_offsets.get(shape)? as usize, *region.shape_offsets.get(shape + 1)? as usize);
    Some(
        (start..end)
            .filter_map(|b| {
                let v = region.shape_boxes.get(b * 6..b * 6 + 6)?;
                Some([v[0], v[1], v[2], v[3], v[4], v[5]])
            })
            .collect(),
    )
}

fn vertex(position: shapes::Point) -> draw::Vertex {
    draw::Vertex {
        position: (position[0], position[1], position[2]),
        color: (1.0, 1.0, 1.0, 1.0),
        uv: (0.0, 0.0),
        data: (0.0, 0.0, 0.0, 0.0),
    }
}

fn line(points: Vec<shapes::Point>, closed: bool) -> Result<draw::Handle, draw::DrawError> {
    draw::create_geometry(&draw::GeometrySpec::Line(draw::Polyline {
        vertices: points.into_iter().map(vertex).collect(),
        closed,
    }))
}

/// The Target HUD's view of one observed actor.
fn target_view(observed: &player_state::ObservedActor) -> hud::TargetView {
    let actor = &observed.actor;
    hud::TargetView {
        name: hud::target_name(&actor.name, &actor.identifier),
        health: actor.health.filter(|h| h.is_finite()),
        max_health: observed.max_health.filter(|h| h.is_finite()),
    }
}

/// The waypoint beam: an upright strip a block wide and 60 tall, facing the camera (the instance turns it), see-through
/// and fading to nothing at the top. Colours are premultiplied, as the drawings are blended.
fn beam_mesh() -> Result<draw::Handle, draw::DrawError> {
    let corner = |x: f32, y: f32, alpha: f32| draw::Vertex {
        position: (x, y, 0.0),
        color: (alpha, alpha, alpha, alpha),
        uv: (0.0, 0.0),
        data: (0.0, 0.0, 0.0, 0.0),
    };
    let vertices = vec![corner(-0.3, 0.0, 1.0), corner(0.3, 0.0, 1.0), corner(0.3, 60.0, 1.0), corner(-0.3, 60.0, 1.0)];
    // Both ways round, so no culling can hide it.
    draw::create_geometry(&draw::GeometrySpec::Triangles(draw::Mesh { vertices, indices: vec![0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2] }))
}

/// A flat diamond of the given radius (a square on its corner), white, for tinting: the marker's icon is a rim and a darker inside.
fn diamond_mesh(radius: f32) -> Result<draw::Handle, draw::DrawError> {
    let corner = |x: f32, y: f32| draw::Vertex { position: (x, y, 0.0), color: (1.0, 1.0, 1.0, 1.0), uv: (0.0, 0.0), data: (0.0, 0.0, 0.0, 0.0) };
    let vertices = vec![corner(0.0, radius), corner(-radius, 0.0), corner(0.0, -radius), corner(radius, 0.0)];
    draw::create_geometry(&draw::GeometrySpec::Triangles(draw::Mesh { vertices, indices: vec![0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2] }))
}

/// Filled triangles (points and indices) as a retained geometry.
fn triangles((points, indices): (Vec<shapes::Point>, Vec<u32>)) -> Result<draw::Handle, draw::DrawError> {
    draw::create_geometry(&draw::GeometrySpec::Triangles(draw::Mesh { vertices: points.into_iter().map(vertex).collect(), indices }))
}

/// Which uploaded shape a drawing uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Wall,
    Post,
    Ring,
    Footprint,
    Column,
    Cube,
    SpawnArea,
    Activation,
}

/// One drawing as the mod wants it; compared to what is on screen to skip unchanged ones.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Want {
    shape: Shape,
    dimension: i32,
    origin: [f64; 3],
    tint: [f32; 4],
    width: f32,
    /// Drawn through blocks (needs the "see through walls" permission).
    through: bool,
}

fn instance(shape: Shape, dimension: i32, origin: [f64; 3], tint: [f32; 4], width: f32) -> Want {
    Want { shape, dimension, origin, tint, width, through: false }
}

/// Whether drawings may show through blocks.
fn through_walls() -> bool {
    draw::capabilities().contains(draw::Permissions::RENDER_THROUGH_WALLS)
}

impl Want {
    fn build(&self, shapes: &Shapes) -> draw::Instance {
        let geometry = match self.shape {
            Shape::Wall => shapes.wall,
            Shape::Post => shapes.post,
            Shape::Ring => shapes.ring,
            Shape::Footprint => shapes.footprint,
            Shape::Column => shapes.column,
            Shape::Cube => shapes.cube,
            Shape::SpawnArea => shapes.spawn_area,
            Shape::Activation => shapes.activation,
        };
        let [r, g, b, a] = self.tint;
        draw::Instance {
            geometry,
            material: None,
            params: Vec::new(),
            anchor: draw::Anchor::World(draw::WorldAnchor {
                dimension: self.dimension,
                origin: (self.origin[0], self.origin[1], self.origin[2]),
            }),
            transform: draw::Transform {
                translation: (0.0, 0.0, 0.0),
                rotation: (0.0, 0.0, 0.0, 1.0),
                scale: (1.0, 1.0, 1.0),
            },
            // Premultiplied blending: colour channels already carry the alpha.
            tint: (r * a, g * a, b * a, a),
            depth: if self.through { draw::Depth::Always } else { draw::Depth::Test },
            blend: draw::Blend::Premultiplied,
            width: self.width,
            width_mode: draw::WidthMode::Pixels,
            join: draw::Join::Bevel,
            cap: draw::Cap::Butt,
            clip: None,
        }
    }
}

/// The touch buttons for phones and tablets.
const TOUCH_CHUNKS: &str = "technic-chunks";
/// Seconds between position updates while Armor status is dragged.
const ARMOR_DRAG_STEP: f64 = 0.03;

const TOUCH_HITBOX: &str = "technic-hitbox";
const TOUCH_ZOOM: &str = "technic-zoom";
/// Opens (or closes) Vantage's menu like its menu key, for a touch screen with no keyboard, and without Mods Core.
const TOUCH_MENU: &str = "technic-menu";
/// The touch buttons' size and the gap between them.
const TOUCH_W: f32 = 44.0;
const TOUCH_H: f32 = 18.0;
const TOUCH_GAP: f32 = 3.0;

/// Drawing imports allowed per frame; the host's ceiling is 32 including everything else.
const DRAW_CALLS: usize = 24;
const DRAW_CALLS_BUSY: usize = 0;
/// Extra calls only hitboxes may use, on top of what the other drawings leave.
const HIT_CALLS: usize = 6;
/// Frames between rereading which actors are near and their box sizes.
const HIT_REFRESH: u32 = 6;
/// Frames between asking which actor the Crosshair is on (a few times a second at 60 FPS).
const XH_REFRESH: u32 = 4;
/// The name of the Damage Indicator's group of drawings, how many digit strokes go up in a frame, and the
/// frames between two reads of the actors' health.
const DAMAGE_GROUP: &str = "technic-damage";
const TNT_GROUP: &str = "technic-tnt";
/// How long a lit TNT burns before it goes off (80 ticks).
const TNT_FUSE: f64 = 4.0;
const DAMAGE_UPLOADS: usize = 4;
const DAMAGE_REFRESH: u32 = 3;
/// The Waypoints' group of drawings and the most instances it uses (the host takes 1024 in all).
const WAYPOINT_GROUP: &str = "technic-waypoints";
/// The Block outline's frame group, and how far its lines and faces stand off the block (so they do not fight its own faces).
const BLOCK_OUTLINE_GROUP: &str = "technic-block-outline";
const BLOCK_OUTLINE_GROW: f64 = 0.001;
/// The bottom of the shape is raised this much (only the bottom, so the top edges stay on the block and nothing floats),
/// so the edges along the bottom of a block stand just above the ground they rest on instead of sinking into it
/// (depth-tested lines there are hidden by the ground's top faces).
const BLOCK_OUTLINE_LIFT: f64 = 0.004;

/// The boxes with the lowest ones' bottoms raised by `BLOCK_OUTLINE_LIFT`.
fn lift_outline(mut boxes: Vec<[f64; 6]>) -> Vec<[f64; 6]> {
    let lowest = boxes.iter().map(|b| b[1]).fold(f64::INFINITY, f64::min);
    for b in &mut boxes {
        if b[1] <= lowest + 1e-9 && b[1] + BLOCK_OUTLINE_LIFT < b[4] {
            b[1] += BLOCK_OUTLINE_LIFT;
        }
    }
    boxes
}

const WAYPOINT_LABELS: &str = "technic-waypoint-labels";
const WAYPOINT_MAX_INSTANCES: usize = 900;
/// How many labels are drawn (the nearest ones; every waypoint has its beam and box).
const WAYPOINT_LABELS_SHOWN: usize = 5;
/// Frames between two looks at who is in hitting range (Reach display's highlight).
const HL_REFRESH: u32 = 5;
/// Target HUD: frames between two reads of the actors, and how long the last target stays after you stop aiming at it.
const TARGET_REFRESH: u32 = 4;
const TARGET_HOLD: f64 = 3.0;
/// Seconds WAILA stays up after the crosshair leaves every block.
const WAILA_HOLD: f64 = 0.25;

/// One actor's hitbox: its entity reference and the box and look line drawn on it.
struct Hit {
    runtime_id: u64,
    entity: draw::Handle,
    /// Offset of the box's corner from the feet, its size, and whether it is a player.
    shape: ([f32; 3], [f32; 3], bool),
    cube: Option<draw::Handle>,
    ray: Option<draw::Handle>,
    /// Still among the actors the host offers this round.
    seen: bool,
}

/// A box when the actor list has none: a player-sized one.
const DEFAULT_BOX: ([f32; 3], [f32; 3], bool) = ([-0.3, 0.0, -0.3], [0.6, 1.8, 0.6], false);

fn entity_instance(geometry: draw::Handle, entity: draw::Handle, body: bool, transform: draw::Transform, tint: [f32; 4]) -> draw::Instance {
    let [r, g, b, a] = tint;
    draw::Instance {
        geometry,
        material: None,
        params: Vec::new(),
        anchor: draw::Anchor::Entity(draw::EntityAnchor { entity, body_rotation: body }),
        transform,
        tint: (r * a, g * a, b * a, a),
        depth: draw::Depth::Test,
        blend: draw::Blend::Premultiplied,
        width: 2.0,
        width_mode: draw::WidthMode::Pixels,
        join: draw::Join::Miter,
        cap: draw::Cap::Butt,
        clip: None,
    }
}

/// A set of retained drawings brought to the wanted list a few host calls at a time. Retained
/// drawings stay on screen without being sent every frame, which a frame's fuel could not pay.
#[derive(Default)]
struct Pool {
    handles: Vec<draw::Handle>,
    shown: Vec<Want>,
    wanted: Vec<Want>,
    cursor: usize,
    dirty: bool,
}

impl Pool {
    fn want(&mut self, wanted: Vec<Want>) {
        self.wanted = wanted;
        self.cursor = 0;
        self.dirty = true;
    }

    fn clear(&mut self) {
        if !self.wanted.is_empty() || !self.handles.is_empty() {
            self.want(Vec::new());
        }
    }

    fn sync(&mut self, shapes: &Shapes, calls: &mut usize, error: &mut Option<String>) {
        while self.dirty && *calls > 0 {
            let index = self.cursor;
            if index < self.wanted.len() {
                let want = self.wanted[index];
                if index < self.handles.len() {
                    if self.shown[index] != want {
                        *calls -= 1;
                        match draw::update(self.handles[index], &want.build(shapes)) {
                            Ok(()) => self.shown[index] = want,
                            Err(failure) => *error = Some(failure.detail),
                        }
                    }
                } else {
                    *calls -= 1;
                    match draw::create(&want.build(shapes)) {
                        Ok(handle) => {
                            self.handles.push(handle);
                            self.shown.push(want);
                        }
                        Err(failure) => {
                            *error = Some(failure.detail);
                            self.wanted.truncate(index);
                            continue;
                        }
                    }
                }
                self.cursor += 1;
            } else if self.handles.len() > self.wanted.len() {
                if let Some(handle) = self.handles.pop() {
                    self.shown.pop();
                    *calls -= 1;
                    let _ = draw::remove(handle);
                }
            } else {
                self.dirty = false;
            }
        }
    }
}

const YELLOW: [f32; 4] = [1.0, 0.9, 0.2, 0.9];
const BLUE: [f32; 4] = [0.25, 0.5, 1.0, 1.0];
const RED: [f32; 4] = [1.0, 0.2, 0.2, 1.0];
const GREEN: [f32; 4] = [0.3, 1.0, 0.3, 1.0];
const ORANGE: [f32; 4] = [1.0, 0.6, 0.1, 1.0];
/// How far around you light spots are shown, in blocks (4 chunks).
const LIGHT_REACH: i32 = 64;
const CYAN: [f32; 4] = [0.2, 0.95, 1.0, 1.0];
const FOOTPRINT: [f32; 4] = [0.2, 0.95, 1.0, 0.6];

/// A sidebar title or row as one clean line: the game's colour codes (a section sign and the character after it) and
/// control characters go, and it is cut at 40 characters.
fn clean_sidebar_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len().min(48));
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\u{a7}' {
            chars.next();
        } else if !c.is_control() {
            out.push(c);
        }
    }
    out.trim().chars().take(40).collect()
}

/// World height limits per dimension.
fn world_height(dimension: i32) -> (i32, i32) {
    match dimension {
        1 => (0, 128),
        2 => (0, 256),
        _ => (-64, 320),
    }
}

impl State {
    /// One frame: everything else first, then the Hive auto requeue with the host reads the frame has left (the host
    /// traps a callback that makes more than eight, so they are counted from here; see `hive_host`).
    fn frame(&mut self) {
        crate::hive_host::new_frame();
        self.frame_body();
        crate::hive_host::frame(&self.settings.gameplay, self.clock);
    }

    fn frame_body(&mut self) {
        let mut handlers = self.settings.gameplay.handler_config();
        handlers.subtitles = self.settings.is_on(Element::AudioSubtitles);
        HANDLERS.set(handlers);
        self.read_controls();
        // A tab was pressed in an earlier frame: its page is the whole of this one.
        if std::mem::take(&mut self.tab_build_due) {
            self.send_tab();
            return;
        }
        if std::mem::take(&mut self.patch_due) {
            self.send_patch();
            return;
        }
        // The menu closed by itself a moment after a heavy page was sent (the Mods grid, Settings > Mods), more often with
        // more per-frame work behind it (Chroma). For a second after a page change the frame is the menu's alone.
        self.tick_timers();
        // The hidden list is cheap and must follow the toggle even on a skipped frame.
        self.sync_position();
        self.sync_hidden();
        if std::mem::take(&mut self.skip_rest) {
            return;
        }
        self.sync_fullbright();
        self.sync_hurt();
        self.sync_gameplay();
        self.sync_cursor();
        let Ok(Some(world)) = crate::hive_host::counted(|| player_state::read_world()) else {
            self.have_world = false;
            self.xh_target = crate::crosshair::Target::None;
            self.publish_hud();
            return;
        };
        crate::hive_host::note_world(world.session, world.alive);
        let (x, y, z) = world.position;
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            self.publish_hud();
            return;
        }
        let feet = [x.floor() as i32, y.floor() as i32, z.floor() as i32];
        self.feet_exact = [f64::from(x), f64::from(y), f64::from(z)];
        if world.dimension != self.dimension || !self.have_world {
            self.monument = MonumentState::default();
            self.in_flight.clear();
            self.slime_known.clear();
            // A new world has its own combat history.
            self.combat.reset();
            self.combat_after = 0;
            self.combat_primed = false;
            // The host clears masks and actor references with the world: only the notes go.
            self.hl_mask = None;
            self.hl_handles.clear();
            self.hl_colour = None;
            self.dmg.clear();
            self.target = None;
            self.waila = None;
            self.waila_mob = None;
            self.waila_block = None;
            self.waila_stamp = None;
            // The host drops every drawing resource with the dimension (its handles go stale: "geometry"), so the
            // lines, the beam and the label glyphs are uploaded again.
            if self.have_world {
                self.shapes = None;
                self.wp_beam = None;
                self.wp_diamond = None;
                self.own_box = None;
                self.own_ray = None;
                self.bo_forget();
                self.dmg_glyphs.clear();
                self.wp_failed = false;
                self.wp_wait = 0.0;
            }
        }
        self.dimension = world.dimension;
        self.world_id = world.world.chars().take(128).collect();
        self.feet = feet;
        self.have_world = true;

        // One heavy step per frame: a frame has FRAME_FUEL = 1,000,000 (see `fuel.rs`).
        if self.shapes.is_none() && !self.shapes_failed {
            self.upload_shapes();
            self.publish_hud();
            return;
        }
        self.read_fps_and_facing();
        self.update_hud_slime();
        // The fuel guard: a stage that is skipped only waits for the next frame, a callback that runs out of fuel
        // traps and the mod is quarantined. Each stage starts only with the fuel it and the rest of the frame need.
        if !crate::hive_host::fuel_allows(crate::fuel::Stage::World) {
            self.publish_hud();
            return;
        }
        let mut heavy = false;
        self.update_borders();
        heavy |= self.update_slime();
        self.update_lights();
        self.update_spawners();
        // While drawings wait for host calls, every other frame goes to drawing them.
        self.frames = self.frames.wrapping_add(1);
        let backlog = self.drawing_backlog() && self.frames % 2 == 0;
        if !heavy && !backlog {
            heavy = self.update_terrain();
        }
        if !crate::hive_host::fuel_allows(crate::fuel::Stage::Targets) {
            self.publish_hud();
            return;
        }
        self.update_guardians();
        self.read_target(world.runtime_id);
        self.sync_highlight(world.runtime_id);
        self.update_damage(world.runtime_id);
        self.update_tnt();
        self.update_waypoint_world();
        self.update_death(world.runtime_id);
        self.update_target(world.runtime_id);
        self.update_waila();
        self.update_waila_mob(world.runtime_id);
        // The inventory read is a big call: it waits for a frame with nothing else heavy (and
        // menu values going out), but never longer than a second.
        if crate::hive_host::fuel_allows(crate::fuel::Stage::Inventory) {
            self.read_armor(heavy || backlog || self.menu_busy);
            self.read_potions();
        }
        // Drawings share what is left of the frame: fewer host calls after heavy work, none without the fuel.
        let draw_calls = if heavy || !crate::hive_host::fuel_allows(crate::fuel::Stage::Drawings) { DRAW_CALLS_BUSY } else { DRAW_CALLS };
        self.sync_drawings(draw_calls);
        self.publish_hud();
    }

    /// Tells the host when fullbright is switched on or off. The host keeps the override, so this
    /// is one call per change; a refusal (usually the missing permission) is said once in chat.
    fn sync_fullbright(&mut self) {
        let wanted = self.settings.fullbright;
        // The host clears its override whenever you join another world or server, without telling
        // the mod, so while it is on the call is repeated twice a second (it is one cheap write).
        if wanted && self.fullbright_sent && self.clock - self.fullbright_at >= 0.5 {
            self.fullbright_sent = false;
        }
        if wanted == self.fullbright_sent {
            self.fullbright_refused = false;
            return;
        }
        if self.fullbright_refused {
            return;
        }
        match environment::set_fullbright(wanted) {
            Ok(()) => {
                self.fullbright_sent = wanted;
                self.fullbright_at = self.clock;
            }
            Err(error) => {
                self.fullbright_refused = true;
                let clean: String = error.chars().filter(|c| !c.is_control()).take(100).collect();
                let _ = host_hud::local_message(&format!(
                    "Vantage: fullbright is off. Allow \"Fullbright\" for this mod. ({clean})"
                ));
            }
        }
    }

    /// The gameplay mods (sprint and sneak modes, time, weather, hurt camera, nametags): each is sent when its
    /// setting changes and again once a second while it is on, since the host drops its overrides when you change
    /// worlds. A refusal (usually a missing permission) is said once in chat.
    #[inline(never)]
    fn sync_gameplay(&mut self) {
        // The toggles' latches go out as held intents, renewed every gameplay frame (omitted while a screen is open,
        // where the host releases them by itself; the latches stay).
        let toggles = self.settings.gameplay.toggle_sprint || self.settings.gameplay.toggle_sneak;
        if self.in_gameplay && (toggles || self.intents_sent) {
            let intents = toggles.then(|| gameplay::MovementIntents { sprint: self.sprint_latch, sneak: self.sneak_latch });
            match gameplay::set_movement_intents(intents) {
                Ok(()) => {
                    self.intents_sent = toggles;
                    self.intents_refused = false;
                }
                Err(error) => {
                    if !self.intents_refused {
                        self.intents_refused = true;
                        let clean: String = error.chars().filter(|c| !c.is_control()).take(80).collect();
                        let _ = host_hud::local_message(&format!("Vantage: the host refused toggle sprint and sneak: {clean}."));
                    }
                }
            }
        }
        let g = &self.settings.gameplay;
        let wanted = GameplayWanted {
            modes: g.control_modes(),
            time: g.time_ticks(),
            weather: g.weather_on.then_some(g.weather),
            hurt: g.hurt_camera(),
            names: g.nametags.then(|| (g.name_no_background, (g.nametag_scale() * 100.0).round() as u32, g.name_shadow, g.name_self)),
        };
        // The camera and the time are sent every frame while on (the host's sky and camera follow the latest value
        // and the server's own updates overwrite an override sent only now and then); the rest once a second.
        let fast = wanted.time.is_some() || wanted.hurt.is_some() || wanted.modes.is_some();
        let slow = wanted != GameplayWanted::default() && self.clock - self.gameplay_at >= 1.0;
        let due = slow;
        if wanted == self.gameplay_sent && !fast && !slow {
            return;
        }
        let prev = self.gameplay_sent;
        if wanted != prev {
            self.gameplay_refused = false;
        }
        self.gameplay_sent = wanted;
        // The host refuses control modes outside focused gameplay, and sending "none" there would drop the toggled
        // sprint / sneak state. While a screen is open the call is skipped and the last sent value is kept, so the
        // change goes out on the first gameplay frame.
        if !self.in_gameplay {
            self.gameplay_sent.modes = prev.modes;
        }
        if slow || wanted != prev {
            self.gameplay_at = self.clock;
        }
        let mut failed: Vec<String> = Vec::new();
        let mut note = |what: &str, result: Result<(), String>| {
            if let Err(error) = result {
                let clean: String = error.chars().filter(|c| !c.is_control()).take(80).collect();
                failed.push(format!("{what}: {clean}"));
            }
        };
        if self.in_gameplay && (wanted.modes != prev.modes || wanted.modes.is_some()) {
            note("sprint and sneak", gameplay::set_control_modes(wanted.modes.map(|[sprint, sneak, always]| gameplay::ControlModes {
                toggle_sprint: sprint,
                toggle_sneak: sneak,
                always_sprint: always,
            })));
        }
        if wanted.time != prev.time || wanted.time.is_some() {
            note("time", environment::set_time_override(wanted.time));
        }
        if wanted.weather != prev.weather || (due && wanted.weather.is_some()) {
            note("weather", environment::set_weather_override(wanted.weather.map(|w| match w {
                0 => environment::Weather::Clear,
                1 => environment::Weather::Rain,
                _ => environment::Weather::Thunder,
            })));
        }
        if wanted.hurt != prev.hurt || wanted.hurt.is_some() {
            note("hurt camera", camera::set_view_tilt(wanted.hurt.and_then(hurt_tilt)));
        }
        if wanted.names != prev.names || (due && wanted.names.is_some()) {
            note("nametags", presentation::set_nametag_style(wanted.names.map(|(no_background, scale, shadow, _own)| presentation::NametagStyle {
                opacity: 1.0,
                scale: scale as f32 / 100.0,
                shadow,
                border_width: 0.0,
                text_color: None,
                own_text_color: None,
                border_color: None,
                background_color: no_background.then_some(render::Rgba { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }),
            })));
            // Your own tag is a rule now: `shown` draws it, none leaves the game's choice.
            let rules = match wanted.names {
                Some((_, _, _, true)) => vec![presentation::NametagRule {
                    actors: presentation::ActorSelector::LocalPlayer,
                    hud_hidden: None,
                    visibility: presentation::NametagVisibility::Shown,
                }],
                _ => Vec::new(),
            };
            note("own nametag", presentation::set_nametag_rules(&rules));
        }
        // Said once per change, naming what the host refused and why.
        if !failed.is_empty() && !self.gameplay_refused {
            self.gameplay_refused = true;
            let _ = host_hud::local_message(&format!("Vantage: the host refused {}.", failed.join("; ")));
        }
    }

    /// Inventory cursor: when an inventory screen opens (or changes to another), the pointer is put in the middle of
    /// the screen, again every frame for a third of a second because some screens move it as they finish opening.
    #[inline(never)]
    fn sync_cursor(&mut self) {
        if !self.settings.gameplay.cursor_center {
            self.cursor_screen = None;
            return;
        }
        let screen = match input::read_container_screen() {
            Ok(Some(name)) => Some(name),
            _ => None,
        };
        if screen != self.cursor_screen {
            self.cursor_until = if screen.is_some() { self.clock + 0.35 } else { 0.0 };
            self.cursor_screen = screen;
        }
        if self.cursor_screen.is_some() && self.clock < self.cursor_until {
            // The host takes the window's own 0..1 coordinates, so the middle is a half on each axis.
            let _ = input::position_pointer(0.5, 0.5);
        }
    }

    /// The colour actors flash when hurt (Lunar's Hit Color): sent when it changes and now and then
    /// after, like fullbright, since the host may drop it when you change worlds. A refusal (no
    /// permission) is said once in chat.
    fn sync_hurt(&mut self) {
        // The card's switch gates it: off is the game's own flash.
        let mode = if self.settings.gameplay.hit_on { self.settings.hit_mode } else { 0 };
        let wanted = match mode {
            1 => Some(crate::config::JAVA_HIT),
            2 => Some(crate::config::rgba(self.settings.hit_color, self.settings.hit_opacity / 100.0)),
            _ => None,
        };
        let due = wanted.is_some() && self.clock - self.hurt_at >= 1.0;
        if wanted == self.hurt_sent && !due {
            return;
        }
        // Said once: after a refusal the same colour is not sent again until it changes.
        if self.hurt_refused && wanted == self.hurt_sent {
            return;
        }
        let colour = wanted.map(|[r, g, b, a]| render::Rgba { r, g, b, a });
        let armor = if self.settings.gameplay.hit_armor { colour } else { None };
        self.hurt_sent = wanted;
        self.hurt_at = self.clock;
        self.hurt_refused = false;
        let tints = if colour.is_some() || armor.is_some() {
            vec![presentation::ActorTint { actors: presentation::ActorSelector::All, state: presentation::TintState::Hurt, body: colour, armor }]
        } else {
            Vec::new()
        };
        if let Err(error) = presentation::set_actor_tints(&tints) {
            self.hurt_refused = true;
            let clean: String = error.chars().filter(|c| !c.is_control()).take(100).collect();
            let _ = host_hud::local_message(&format!("Vantage: the hit color could not be set. ({clean})"));
        }
    }

    /// Whether the game's own position line goes: while Vantage's Coordinates element is on (and asked to).
    /// It is hidden with the other vanilla elements now (`sync_hidden`), so this only sets the wish.
    fn sync_position(&mut self) {
        self.position_hidden = self.settings.is_on(Element::Coords) && self.settings.gameplay.hide_game_coords;
    }

    /// Glass: the terrain surface rules for the chosen style are handed to the host when the style changes. The
    /// host keeps them until they are replaced; Normal sends an empty list, which restores the game's own glass.
    fn sync_glass(&mut self) {
        let g = &self.settings.gameplay;
        let want = if g.glass_on { (crate::glass::clamp(g.glass_mode), crate::glass::clamp(g.glass_coloured)) } else { (crate::glass::NORMAL, crate::glass::NORMAL) };
        if self.glass_applied == Some(want) || self.glass_refused == Some(want) {
            return;
        }
        let rules: Vec<terrain::SurfaceRule> = crate::glass::rules(want.0, want.1)
            .into_iter()
            .map(|rule| terrain::SurfaceRule {
                groups: rule.groups,
                border: rule.border,
                clauses: rule
                    .clauses
                    .into_iter()
                    .map(|clause| terrain::SurfaceClause {
                        conditions: clause.conditions.into_iter().map(|c| terrain::SurfaceCondition { offset: c.offset, present: c.present }).collect(),
                        tiles: clause.tiles,
                    })
                    .collect(),
            })
            .collect();
        match terrain::set_surface_rules(&rules) {
            Ok(()) => {
                // Say what was applied, except the quiet first call that only clears old rules.
                if self.glass_applied.is_some() || want != (crate::glass::NORMAL, crate::glass::NORMAL) {
                    let _ = host_hud::local_message(&format!("Vantage: glass {}, coloured glass {}.", crate::glass::GLASS_NAMES[want.0], crate::glass::GLASS_NAMES[want.1]));
                }
                self.glass_applied = Some(want);
                self.glass_refused = None;
            }
            Err(error) => {
                self.glass_refused = Some(want);
                let clean: String = error.chars().filter(|c| !c.is_control()).take(100).collect();
                if want != (crate::glass::NORMAL, crate::glass::NORMAL) || self.glass_applied.is_some() {
                    let _ = host_hud::local_message(&format!("Vantage: the glass style could not be set. ({clean})"));
                }
            }
        }
    }

    /// The game's own Freelook and Cycle perspective actions take the keys chosen on the Perspective page:
    /// a changed key is written to the player's Controls binding (`input::write_binding`, saved like their own
    /// edit), so the game does the looking itself, pitch included. An empty key leaves the game's own.
    fn sync_game_keys(&mut self) {
        if !self.settings.gameplay.persp_on {
            self.snap_toggled = 0;
        }
        if !self.settings.gameplay.persp_on || self.bind_refused {
            return;
        }
        for (n, (action, id)) in [("freelook", "persp_free"), ("cycle-perspective", "persp_cycle")].into_iter().enumerate() {
            let key = self.settings.key(id).to_owned();
            if key == self.bind_sent[n] {
                continue;
            }
            let change = if key.is_empty() { input::BindingChange::Unbind } else { input::BindingChange::Assign(key.clone()) };
            match input::write_binding(action, &change) {
                Ok(()) => {
                    self.bind_sent[n] = key;
                    self.bind_written_at = self.clock;
                }
                Err(error) => {
                    self.bind_refused = true;
                    let clean: String = error.chars().filter(|c| !c.is_control()).take(100).collect();
                    let _ = host_hud::local_message(&format!("Vantage: the {action} key could not be set. ({clean})"));
                }
            }
        }
    }

    /// Shows the game's own keys for Freelook and Cycle perspective on the page: its bindings are read
    /// (one page of 16 a frame, only when the revision changed, at most every 2 s) and a key that differs from
    /// the page's is taken over, so a change made in Settings > Controls shows here too. A key just written
    /// from the page is left alone for a moment while the host applies it. True when a key cap changed.
    fn poll_game_keys(&mut self) -> bool {
        if !self.settings.gameplay.persp_on || self.bind_refused {
            return false;
        }
        if self.bind_collecting.is_none() && self.clock - self.bind_polled_at < 2.0 {
            return false;
        }
        let (after, offset) = match &self.bind_collecting {
            Some((_, offset, _)) => (None, *offset),
            None => (self.bind_after, 0),
        };
        self.bind_polled_at = self.clock;
        let page = match crate::hive_host::counted(|| input::read_bindings(after, offset, 16)) {
            Ok(Some(page)) => page,
            // Unchanged since the last complete read, or not readable yet: ask again later.
            _ => {
                self.bind_collecting = None;
                return false;
            }
        };
        let (mut revision, mut offset, mut rows) = self.bind_collecting.take().unwrap_or((page.revision, 0, Vec::new()));
        if revision != page.revision {
            // The bindings changed while they were read: start over.
            self.bind_collecting = None;
            return false;
        }
        revision = page.revision;
        offset += page.entries.len() as u32;
        rows.extend(page.entries.into_iter().filter(|e| matches!(e.context, input::BindingContext::Gameplay) && !e.key.is_empty()).map(|e| (e.action, e.key)));
        if offset < page.total && offset > 0 {
            self.bind_collecting = Some((revision, offset, rows));
            return false;
        }
        self.bind_after = Some(revision);
        if self.clock - self.bind_written_at < 2.5 {
            return false;
        }
        let mut changed = false;
        for (n, (action, id)) in [("freelook", "persp_free"), ("cycle-perspective", "persp_cycle")].into_iter().enumerate() {
            let game = rows.iter().find(|(a, _)| a == action).map(|(_, k)| k.clone()).unwrap_or_default();
            // Only when the page has nothing of its own still waiting to be written.
            if game != self.settings.key(id) && self.settings.key(id) == self.bind_sent[n] {
                self.settings.set_key(id, &game);
                self.bind_sent[n] = game;
                changed = true;
            }
        }
        changed
    }

    /// Claims the mouse wheel for the zoom while its key is held and returns the whole steps it moved
    /// this frame (positive zooms in).
    fn zoom_wheel(&mut self, active: bool) -> i32 {
        let wanted = if self.settings.zoom.step_keys { self.settings.zoom.held_key().map(str::to_owned) } else { None };
        if wanted != self.scroll_claim || (wanted.is_some() && self.clock - self.scroll_claim_at >= 2.0) {
            let keys: Vec<String> = wanted.iter().cloned().collect();
            if input::reserve_scroll(&keys).is_ok() {
                self.scroll_claim = wanted.clone();
                self.scroll_claim_at = self.clock;
            }
        }
        let Some(key) = wanted else { return 0 };
        if !active || self.scroll_claim.is_none() {
            self.scroll_acc = 0.0;
            return 0;
        }
        let Ok(scroll) = crate::hive_host::counted(|| input::read_scroll()) else { return 0 };
        let _ = key;
        self.scroll_acc += scroll.line_y + scroll.pixel_y / 40.0;
        let whole = self.scroll_acc.trunc();
        self.scroll_acc -= whole;
        whole.clamp(-4.0, 4.0) as i32
    }

    /// Hides the game's scoreboard while it is switched off here. The host keeps the list, so
    /// this is one call per change; a refusal is said once in chat.
    fn sync_hidden(&mut self) {
        // The game's own crosshair goes while Vantage's Crosshair is on.
        // So does the game's own effect list while Potion status is on (and asked to).
        // The Scoreboard draws the sidebar itself, so the game's own goes while it is on.
        let wanted = (self.settings.hide_scoreboard || self.settings.is_on(Element::Scoreboard), self.settings.crosshair.enabled, self.settings.is_on(Element::Potion) && self.settings.potion_hide_vanilla, self.position_hidden);
        if self.hidden_synced && wanted == self.hidden_sent {
            self.hidden_refused = false;
            return;
        }
        if self.hidden_refused {
            return;
        }
        let mut hidden = Vec::with_capacity(4);
        if wanted.0 {
            hidden.push(host_hud::VanillaElement::Sidebar);
        }
        if wanted.1 {
            hidden.push(host_hud::VanillaElement::Crosshair);
        }
        if wanted.2 {
            hidden.push(host_hud::VanillaElement::StatusEffects);
        }
        if wanted.3 {
            hidden.push(host_hud::VanillaElement::Position);
        }
        match host_hud::set_hidden_elements(&hidden) {
            Ok(()) => {
                self.hidden_sent = wanted;
                self.hidden_synced = true;
            }
            Err(error) => {
                self.hidden_refused = true;
                let clean: String = error.chars().filter(|c| !c.is_control()).take(100).collect();
                let _ = host_hud::local_message(&format!("Vantage: the scoreboard or the game's crosshair stays. ({clean})"));
            }
        }
    }

    /// The Stopwatch and Playtime, every frame: with no world loaded too, since the Stopwatch may
    /// be left running and Playtime counts the time outside a world unless it is told not to.
    #[inline(never)]
    fn tick_timers(&mut self) {
        let step = (self.clock - self.timer_clock).max(0.0);
        self.timer_clock = self.clock;
        self.stopwatch.step(step, self.settings.is_on(Element::Stopwatch), self.settings.stopwatch_keep);
        self.playtime.step(step, self.have_world);
    }

    /// Reach and combo: the host's combat evidence since the last frame (at most two reads of
    /// four events), handed to the tracker. What happened before the element was switched on is
    /// read past without being counted.
    fn read_combat(&mut self) {
        if !(self.settings.is_on(Element::Reach) || self.settings.is_on(Element::Combo) || self.settings.is_on(Element::TotemCounter) || self.settings.is_on(Element::PvpInfo)) {
            self.combat_primed = false;
            // Nothing stale is left to show when the elements are switched on again.
            self.combat.reset();
            return;
        }
        for _ in 0..2 {
            let Ok(events) = crate::hive_host::counted(|| player_state::read_combat(self.combat_after, 4)) else { break };
            let full = events.len() >= 4;
            for event in &events {
                self.combat_after = self.combat_after.max(event.sequence);
                if !self.combat_primed {
                    continue;
                }
                // A pop of your own totem (the host marks it `local`).
                if event.local && matches!(event.kind, player_state::CombatKind::Totem) {
                    self.totems = self.totems.saturating_add(1);
                }
                let kind = match event.kind {
                    player_state::CombatKind::AttackAdmitted => crate::combat::Kind::Attack,
                    player_state::CombatKind::Hurt => crate::combat::Kind::Hurt,
                    player_state::CombatKind::Death => crate::combat::Kind::Death,
                    _ => crate::combat::Kind::Other,
                };
                // An attack the host let through is yours, whatever its `local` says.
                let local = event.local || kind == crate::combat::Kind::Attack;
                self.combat.event(crate::combat::Event { kind, local, target: event.runtime_id, distance: event.distance }, self.clock);
            }
            if !full {
                self.combat_primed = true;
                break;
            }
        }
        self.combat.tick(self.clock);
    }

    /// FPS twice a second (a steadier number to read), facing every frame.
    #[inline(never)]
    fn read_fps_and_facing(&mut self) {
        if self.settings.is_on(Element::Speed) {
            self.speed.sample(self.feet_exact, self.clock, self.settings.speed_vertical);
        } else {
            self.speed.clear();
        }
        if self.settings.is_on(Element::Ping) && self.clock - self.ping_checked >= 1.0 {
            self.ping_checked = self.clock;
            self.ping = crate::hive_host::counted(|| player_state::read_ping()).ok().flatten();
        }
        if self.settings.is_on(Element::Food) && self.clock - self.food_checked >= 0.25 {
            self.food_checked = self.clock;
            self.food = match crate::hive_host::counted(|| player_state::read_vitals()) {
                Ok(Some(vitals)) => vitals.hunger.filter(|hunger| hunger.is_finite()).map(|hunger| {
                    let saturation = vitals.saturation.filter(|s| s.is_finite()).unwrap_or(0.0).max(0.0);
                    [hunger.round().clamp(0.0, 99.0) as u32, (saturation * 10.0).round().min(999.0) as u32]
                }),
                _ => None,
            };
        }
        if self.settings.is_on(Element::Day) && self.clock - self.day_checked >= 1.0 {
            self.day_checked = self.clock;
            self.day = match crate::hive_host::counted(|| player_state::read_world_time()) {
                Ok(Some(world)) if world.time >= 0 => Some((world.time / 24_000).min(9_999_999) as u32),
                _ => None,
            };
        }
        // Audio subtitles: the sounds the handler heard, turned into lines; the box follows how many there are.
        if self.settings.is_on(Element::AudioSubtitles) {
            let heard = HEARD.with(|heard| std::mem::take(&mut *heard.borrow_mut()));
            if !heard.is_empty() {
                if let Ok(Some(motion)) = crate::hive_host::counted(|| player_state::read_motion()) {
                    let yaw = f64::from(mod_api::bedrock_view_angles(motion.yaw, motion.pitch)[0]);
                    for (name, at) in heard {
                        self.subs.hear(self.clock, &name, at, self.feet_exact, yaw);
                    }
                }
            }
            self.subs.expire(self.clock);
            let rows = self.subs.rows();
            if hud::set_subtitle_lines(rows.len()) {
                self.settings_revision += 1;
            }
            self.sub_rows = rows;
        } else if !self.sub_rows.is_empty() {
            self.subs.clear();
            self.sub_rows.clear();
            HEARD.with(|heard| heard.borrow_mut().clear());
            hud::set_subtitle_lines(1);
        }
        // The Scoreboard reads the server's sidebar four times a second; its box follows the number of lines.
        if self.settings.is_on(Element::Scoreboard) {
            if self.clock - self.sidebar_checked >= 0.25 {
                self.sidebar_checked = self.clock;
                self.sidebar = match crate::hive_host::counted(|| player_state::read_sidebar()) {
                    Ok(Some(board)) => {
                        // Servers like The Hive put their sidebar together with a resource pack (a header that is not
                        // meant to show, rows that sit side by side): the header can go and the rows can share a line.
                        let options = &self.settings.gameplay;
                        let title = if options.sb_title { clean_sidebar_text(&board.title) } else { String::new() };
                        let mut rows: Vec<(String, i32)> = board.rows.iter().take(15).map(|row| (clean_sidebar_text(&row.label), row.score)).collect();
                        if options.sb_horizontal && rows.len() > 1 {
                            let scores = self.settings.sb_scores;
                            let joined: Vec<String> = rows.iter().filter(|(label, _)| !label.is_empty()).map(|(label, score)| if scores { format!("{label}  {score}") } else { label.clone() }).collect();
                            rows = vec![(joined.join("   "), hud::NO_SCORE)];
                        }
                        Some(hud::Sidebar { title, rows })
                    }
                    _ => None,
                };
                if hud::set_sidebar_lines(hud::sidebar_line_count(self.sidebar.as_ref())) {
                    self.settings_revision += 1;
                }
            }
        } else if self.sidebar.take().is_some() {
            hud::set_sidebar_lines(1);
        }
        // Waypoints: the nearest spots of this world, and the element's box follows how many there are.
        if self.settings.is_on(Element::Facing) && self.settings.gameplay.wp_compass && self.settings.gameplay.wp_on {
            self.wp_rows = if self.settings.gameplay.wp_hidden {
                Vec::new()
            } else {
                crate::waypoints::rows(&self.settings.gameplay.waypoints, &self.world_id, self.dimension, self.feet_exact, crate::waypoints::NEAREST)
            };
            if hud::set_waypoint_lines(self.wp_rows.len().min(crate::waypoints::SHOWN)) {
                self.settings_revision += 1;
            }
        } else if !self.wp_rows.is_empty() {
            self.wp_rows.clear();
            hud::set_waypoint_lines(1);
        }
        self.read_combat();
        if self.settings.is_on(Element::Fps) && self.clock - self.fps_checked >= 0.5 {
            self.fps_checked = self.clock;
            if let Ok(Some(fps)) = crate::hive_host::counted(|| player_state::read_fps()) {
                if fps.is_finite() && fps >= 0.0 {
                    self.fps = Some(fps.round().min(9999.0) as u32);
                }
            }
        }
        // The Clock reads the host's local time twice a second (whole minutes unless it shows seconds).
        if self.settings.is_on(Element::Clock) && self.clock - self.clock_checked >= 0.5 {
            self.clock_checked = self.clock;
            self.clock_time = match clock::read_local_time() {
                Ok(now) => {
                    let local = (now.unix_ms / 1000) as i64 + i64::from(now.utc_offset_seconds);
                    let day = local.rem_euclid(86_400) as u32;
                    Some(if self.settings.clock_seconds { day } else { day / 60 * 60 })
                }
                _ => None,
            };
        }
        // View angles: yaw (-180 to 180, 0 south) and pitch, and the speed it shows.
        if self.settings.is_on(Element::ViewAngles) {
            if let Ok(Some(motion)) = crate::hive_host::counted(|| player_state::read_motion()) {
                let [yaw, pitch] = mod_api::bedrock_view_angles(motion.yaw, motion.pitch);
                let yaw = (f64::from(yaw) + 180.0).rem_euclid(360.0) - 180.0;
                self.va_angles = Some([yaw, f64::from(pitch).clamp(-90.0, 90.0)]);
            }
            if self.settings.va_speed || self.settings.va_elytra {
                self.va_meter.sample(self.feet_exact, self.clock, true);
            } else {
                self.va_meter.clear();
            }
        }
        if self.settings.is_on(Element::Facing) || (self.settings.is_on(Element::Coords) && self.settings.co_dir) {
            if let Ok(Some(motion)) = crate::hive_host::counted(|| player_state::read_motion()) {
                // Bedrock yaw: 0 faces south, 90 west, 180 north, 270 east.
                let yaw = f64::from(mod_api::bedrock_view_angles(motion.yaw, motion.pitch)[0]).rem_euclid(360.0);
                self.facing = Some((((yaw + 45.0) / 90.0) as u8) % 4);
                // The compass moves with the heading in half degrees; the text line has no use for them.
                self.yaw = if self.settings.facing_text { None } else { Some(((yaw * 2.0).round() as u16) % 720) };
            }
        }
    }

    /// Uploads the digit shapes a few per frame (a frame has few host calls to spare); true once all are up
    /// (the Damage Indicator and the Waypoints' distances share them). False while they go up, or after a refusal.
    fn glyphs_ready(&mut self) -> bool {
        if self.dmg_failed {
            return false;
        }
        if self.dmg_glyphs.len() < crate::damage::GLYPHS {
            let from = self.dmg_glyphs.len();
            for index in from..crate::damage::GLYPHS.min(from + DAMAGE_UPLOADS) {
                match triangles(crate::damage::glyph_mesh(index)) {
                    Ok(handle) => self.dmg_glyphs.push(handle),
                    Err(error) => {
                        self.dmg_failed = true;
                        self.draw_error = Some(error.detail);
                        return false;
                    }
                }
            }
            return false;
        }
        true
    }

    /// Waypoints in the world: a beam up from each shown waypoint of this world, in its colour, with the distance
    /// in blocks above it (the number faces you and grows with the distance so it stays readable). One group of
    /// instances, replaced every frame while there is something to draw and emptied once when there is not.
    fn update_waypoint_world(&mut self) {
        let clock = self.clock;
        if self.settings.gameplay.mk_on {
            self.markers.retain(|(_, until)| clock < *until);
        } else if !self.markers.is_empty() {
            self.markers.clear();
        }
        let show_waypoints = self.settings.gameplay.wp_on && self.settings.gameplay.wp_world && !self.settings.gameplay.wp_hidden && self.settings.gameplay.waypoints.iter().any(|w| w.visible);
        let wanted = show_waypoints || !self.markers.is_empty();
        let nothing = |this: &mut Self| {
            if this.wp_drawn {
                let _ = draw::replace_frame_group(WAYPOINT_GROUP, &[]);
                let _ = draw::replace_frame_group(WAYPOINT_LABELS, &[]);
                this.wp_drawn = false;
            }
        };
        if !wanted {
            nothing(self);
            return;
        }
        if self.clock < self.wp_wait {
            return;
        }
        let Some(cube) = self.shapes.as_ref().map(|shapes| shapes.cube) else { return };
        if !self.glyphs_ready() {
            return;
        }
        // The beam's mesh goes up once (a flat strip that fades towards the top).
        if self.wp_beam.is_none() {
            match beam_mesh() {
                Ok(handle) => self.wp_beam = Some(handle),
                Err(error) => {
                    self.wp_wait = self.clock + 5.0;
                    let clean: String = error.detail.chars().filter(|c| !c.is_control()).take(80).collect();
                    let _ = host_hud::local_message(&format!("Vantage: the waypoint beam could not be built ({clean})"));
                    return;
                }
            }
        }
        let Some(beam) = self.wp_beam else { return };
        if self.wp_diamond.is_none() {
            if let (Ok(rim), Ok(inside)) = (diamond_mesh(0.5), diamond_mesh(0.36)) {
                self.wp_diamond = Some([rim, inside]);
            }
        }
        let diamond = self.wp_diamond;
        let Ok(Some(camera)) = gameplay::read_camera() else { return };
        let (yaw, pitch) = (f64::from(camera.yaw).to_degrees(), f64::from(camera.pitch).to_degrees());
        if !(yaw.is_finite() && pitch.is_finite()) {
            return;
        }
        let eye = [f64::from(camera.position.x), f64::from(camera.position.y), f64::from(camera.position.z)];
        let rotation = hitbox::look_rotation(yaw, pitch);
        // The beam stands upright and turns only with the heading.
        let upright = hitbox::look_rotation(yaw, 0.0);
        let depth = if through_walls() { draw::Depth::Always } else { draw::Depth::Test };
        let linear = |c: f32| c.powf(2.2);
        let mut draws: Vec<draw::Instance> = Vec::new();
        let mut labels: Vec<draw::Instance> = Vec::new();
        let mut plate = Vec::new();
        let mut outline = Vec::new();
        let mut placed = Vec::new();
        let look = self.settings.gameplay.clone_wp_look();
        let axes = crate::damage::camera_axes(yaw, pitch);
        let (yaw_r, pitch_r) = (yaw.to_radians(), pitch.to_radians());
        let forward = [-yaw_r.sin() * pitch_r.cos(), pitch_r.sin(), -yaw_r.cos() * pitch_r.cos()];
        let mut shown: Vec<&crate::waypoints::Waypoint> = if show_waypoints {
            self.settings.gameplay.waypoints.iter().filter(|w| w.visible && w.world == self.world_id && w.dimension == self.dimension).collect()
        } else {
            Vec::new()
        };
        shown.extend(self.markers.iter().map(|(w, _)| w).filter(|w| w.world == self.world_id && w.dimension == self.dimension));
        let apart = |w: &crate::waypoints::Waypoint| (f64::from(w.pos[0]) + 0.5 - eye[0]).powi(2) + (f64::from(w.pos[1]) - eye[1]).powi(2) + (f64::from(w.pos[2]) + 0.5 - eye[2]).powi(2);
        shown.sort_by(|a, b| apart(a).total_cmp(&apart(b)));
        for (rank, waypoint) in shown.into_iter().enumerate() {
            let [r, g, b] = crate::waypoints::rgb(waypoint.color).map(linear);
            let origin = [f64::from(waypoint.pos[0]) + 0.5, f64::from(waypoint.pos[1]), f64::from(waypoint.pos[2]) + 0.5];
            let distance = ((origin[0] - eye[0]).powi(2) + (origin[1] - eye[1]).powi(2) + (origin[2] - eye[2]).powi(2)).sqrt();
            let instance = |geometry: draw::Handle, anchor: [f64; 3], translation: (f32, f32, f32), rotation: (f32, f32, f32, f32), scale: f32, tint: (f32, f32, f32, f32), width: f32| draw::Instance {
                geometry,
                material: None,
                params: Vec::new(),
                anchor: draw::Anchor::World(draw::WorldAnchor { dimension: self.dimension, origin: (anchor[0], anchor[1], anchor[2]) }),
                transform: draw::Transform { translation, rotation, scale: (scale, scale, scale) },
                tint,
                depth,
                blend: draw::Blend::Premultiplied,
                width,
                width_mode: draw::WidthMode::Pixels,
                join: draw::Join::Bevel,
                cap: draw::Cap::Butt,
                clip: None,
            };
            // Lunar's look: a soft, see-through beam in the colour (strongest at the foot), the block it marks as a
            // thin box, and a dark plate over it with the name and the distance.
            // Like Lunar's, the beam and the label fade away as you come close (gone within three blocks).
            // A marker is Lunar's: no beam, the block outlined, and the label stays at any distance.
            let marker = self.markers.iter().any(|(m, _)| m.pos == waypoint.pos && m.name == waypoint.name && m.world == waypoint.world && m.dimension == waypoint.dimension);
            if look.near && !marker {
                // Only the waypoints you are looking towards (within about 15 degrees of the crosshair).
                let d = distance.max(0.001);
                let cosine = ((origin[0] - eye[0]) * forward[0] + (origin[1] + 1.0 - eye[1]) * forward[1] + (origin[2] - eye[2]) * forward[2]) / d;
                if cosine < 0.966 {
                    continue;
                }
            }
            let near = if marker { 1.0 } else { (((distance - 3.0) / 14.0).clamp(0.0, 1.0)) as f32 };
            let fade = near * near * (3.0 - 2.0 * near);
            if fade > 0.02 && look.beams && !marker {
                let a = 0.3 * fade;
                draws.push(instance(beam, origin, (0.0, 0.0, 0.0), upright, 1.0, (r * a, g * a, b * a, a), 1.0));
            }
            if marker {
                // Lunar's: a thin outline flat on the block's top face.
                let mut flat = instance(cube, [f64::from(waypoint.pos[0]), f64::from(waypoint.pos[1]), f64::from(waypoint.pos[2])], (0.0, 1.0, 0.0), (0.0, 0.0, 0.0, 1.0), 1.0, (r * 0.8, g * 0.8, b * 0.8, 0.8), 1.5);
                flat.transform.scale = (1.0, 0.002, 1.0);
                draws.push(flat);
            } else if look.highlight {
                draws.push(instance(cube, [f64::from(waypoint.pos[0]), f64::from(waypoint.pos[1]), f64::from(waypoint.pos[2])], (0.0, 0.0, 0.0), (0.0, 0.0, 0.0, 1.0), 1.0, (r, g, b, 1.0), look.line));
            }
            // Looking at a marker (within about eight degrees) adds its distance to the label, like Lunar's "on hover".
            let hovered = marker && {
                let d = distance.max(0.001);
                ((origin[0] - eye[0]) * forward[0] + (origin[1] + 1.0 - eye[1]) * forward[1] + (origin[2] - eye[2]) * forward[2]) / d > 0.985
            };
            if fade <= 0.03 || rank >= WAYPOINT_LABELS_SHOWN {
                continue;
            }
            // A marker has a size in the world (smaller the further it is, like Lunar's); a waypoint's label keeps its size on screen.
            let height = if marker { 0.2 * look.scale } else { (0.015 * look.scale * distance as f32).clamp(0.15 * look.scale, 40.0 * look.scale) };
            let name: String = waypoint.name.chars().take(12).collect();
            let label = if marker {
                if hovered { format!("{name} ({distance:.1}m)") } else { name }
            } else if look.distance {
                format!("{name} [{}m]", distance.round() as u64)
            } else {
                name
            };
            plate.clear();
            outline.clear();
            placed.clear();
            if let (true, Some([rim, inside])) = (marker, diamond) {
                // Lunar's icon under the name: a diamond with a dark inside and a "!" (a marker's colour rim), flat to the camera.
                let size = 0.2 * look.scale;
                let at = [origin[0], origin[1] + 1.55, origin[2]];
                labels.push(instance(rim, at, (0.0, 0.0, 0.0), rotation, size, (r, g, b, 1.0), 1.0));
                let toward = [(eye[0] - at[0]) / distance.max(1.0), (eye[1] - at[1]) / distance.max(1.0), (eye[2] - at[2]) / distance.max(1.0)];
                let nearer = [at[0] + toward[0] * 0.01, at[1] + toward[1] * 0.01, at[2] + toward[2] * 0.01];
                labels.push(instance(inside, nearer, (0.0, 0.0, 0.0), rotation, size, (0.04, 0.05, 0.06, 1.0), 1.0));
                plate.clear();
                outline.clear();
                placed.clear();
                crate::damage::place_label("!", [at[0], at[1] + f64::from(size) * 0.28, at[2]], size * 0.42, axes, look.padding, &mut plate, &mut outline, &mut placed);
                let front = [at[0] + toward[0] * 0.02, at[1] + toward[1] * 0.02, at[2] + toward[2] * 0.02];
                for p in &placed {
                    labels.push(instance(self.dmg_glyphs[p.glyph], front, (p.shift[0], p.shift[1], p.shift[2]), rotation, p.scale.max(0.002), (r, g, b, 1.0), 1.0));
                }
            }
            let above = [origin[0], origin[1] + if marker { 1.9 } else { 2.2 } + f64::from(height), origin[2]];
            crate::damage::place_label(&label, above, height, axes, look.padding, &mut plate, &mut outline, &mut placed);
            let scale = |p: &crate::damage::Placed| p.scale.max(0.002);
            for p in plate.iter().filter(|_| !marker) {
                labels.push(instance(self.dmg_glyphs[p.glyph], p.origin, (p.shift[0], p.shift[1], p.shift[2]), rotation, scale(p), (0.0, 0.0, 0.0, 0.5 * fade), 1.0));
            }
            // The outline is the waypoint's colour; it and the characters sit a little nearer the camera than the plate,
            // so they are never lost in it.
            let toward = [(eye[0] - above[0]) / distance.max(1.0), (eye[1] - above[1]) / distance.max(1.0), (eye[2] - above[2]) / distance.max(1.0)];
            let lift = 0.01 + f64::from(height) * 0.02;
            let nearer = [above[0] + toward[0] * lift, above[1] + toward[1] * lift, above[2] + toward[2] * lift];
            for p in outline.iter().filter(|_| look.borders && !marker) {
                labels.push(instance(self.dmg_glyphs[p.glyph], nearer, (p.shift[0], p.shift[1], p.shift[2]), rotation, scale(p), (r * fade, g * fade, b * fade, fade), 1.0));
            }
            if look.shadow || marker {
                // The game's text shadow: the same characters in dark grey, one pixel right and down.
                let between = [above[0] + toward[0] * lift * 1.5, above[1] + toward[1] * lift * 1.5, above[2] + toward[2] * lift * 1.5];
                for p in &placed {
                    let px = p.scale;
                    let shift = (p.shift[0] + (axes.0[0] - axes.1[0]) * px, p.shift[1] + (axes.0[1] - axes.1[1]) * px, p.shift[2] + (axes.0[2] - axes.1[2]) * px);
                    labels.push(instance(self.dmg_glyphs[p.glyph], between, shift, rotation, scale(p), (0.25 * fade, 0.25 * fade, 0.25 * fade, fade), 1.0));
                }
            }
            let nearer = [above[0] + toward[0] * lift * 2.0, above[1] + toward[1] * lift * 2.0, above[2] + toward[2] * lift * 2.0];
            for p in &placed {
                labels.push(instance(self.dmg_glyphs[p.glyph], nearer, (p.shift[0], p.shift[1], p.shift[2]), rotation, scale(p), (fade, fade, fade, fade), 1.0));
            }
            if labels.len() + 150 >= WAYPOINT_MAX_INSTANCES {
                break;
            }
        }
        // Beams and labels go up as two groups, so one that the host refuses does not take the other down, and the
        // message says which it was.
        let mut failure: Option<String> = None;
        let mut stale = false;
        for (what, group, list) in [("beams", WAYPOINT_GROUP, &draws), ("labels", WAYPOINT_LABELS, &labels)] {
            if let Err(error) = draw::replace_frame_group(group, list) {
                // The host forgot our shapes (it drops them with the dimension or session, and not always on the
                // frame the dimension changes): upload them again and try once more, without a message.
                if error.detail == "geometry" {
                    stale = true;
                    continue;
                }
                let clean: String = error.detail.chars().filter(|c| !c.is_control()).take(80).collect();
                failure.get_or_insert(format!("the waypoint {what} could not be drawn ({clean})"));
            }
        }
        self.wp_drawn = true;
        if stale {
            self.shapes = None;
            self.wp_beam = None;
            self.wp_diamond = None;
            self.dmg_glyphs.clear();
            self.own_box = None;
            self.own_ray = None;
                self.bo_forget();
            self.wp_wait = self.clock + 0.3;
            return;
        }
        match failure {
            None => self.wp_failed = false,
            Some(message) => {
                // Try again in a few seconds; say so once.
                self.wp_wait = self.clock + 5.0;
                if !self.wp_failed {
                    self.wp_failed = true;
                    let _ = host_hud::local_message(&format!("Vantage: {message}"));
                }
            }
        }
    }

    /// Death waypoints: marks the block you died on (once per death), named "Death".
    fn update_death(&mut self, own: u64) {
        if !self.settings.gameplay.wp_death || !self.settings.gameplay.wp_on || self.frames % 6 != 0 {
            return;
        }
        let Ok(actors) = crate::hive_host::counted(|| player_state::read_actors_limited(mod_api::MAX_READ_ACTORS as u32)) else { return };
        let Some(health) = actors.iter().find(|o| o.actor.runtime_id == own).and_then(|o| o.actor.health).filter(|h| h.is_finite()) else { return };
        if health > 0.0 {
            self.wp_dead = false;
        } else if !self.wp_dead {
            self.wp_dead = true;
            let pos = [self.feet[0], self.feet[1] - 1, self.feet[2]];
            let message = match crate::waypoints::add_named(&mut self.settings.gameplay.waypoints, &self.world_id, self.dimension, pos, 1, "Death", false) {
                Some(name) => format!("Vantage: {name} marked at {} {} {}", pos[0], pos[1], pos[2]),
                None => format!("Vantage: no room for a death waypoint ({} is the most)", crate::waypoints::MAX_WAYPOINTS),
            };
            let _ = host_hud::local_message(&message);
            self.save_due = Some(self.clock);
        }
    }

    /// Damage Indicator: reads the health of the nearby actors a few times a second, starts a number over
    /// each one that lost some, and draws the numbers alive as camera-facing strokes (one group of
    /// instances, replaced every frame while there are numbers and emptied once when the last is gone).
    fn update_tnt(&mut self) {
        let clear = |this: &mut Self| {
            if this.tnt_drawn {
                let _ = draw::replace_frame_group(TNT_GROUP, &[]);
                this.tnt_drawn = false;
            }
            this.tnt_seen.clear();
            this.tnt_actors.clear();
        };
        if !self.settings.gameplay.tnt_on || self.tnt_failed || !self.glyphs_ready() {
            clear(self);
            return;
        }
        // The actors are read every other frame; the labels are drawn from the last read every frame.
        if self.frames % 2 == 0 {
            if let Ok(actors) = crate::hive_host::counted(|| player_state::read_actors_limited(mod_api::MAX_READ_ACTORS as u32)) {
                self.tnt_actors = actors
                    .iter()
                    .filter(|o| o.actor.identifier == "minecraft:tnt")
                    .map(|o| (o.actor.runtime_id, [f64::from(o.actor.position.0), f64::from(o.actor.position.1), f64::from(o.actor.position.2)]))
                    .collect();
                let clock = self.clock;
                for (id, _) in &self.tnt_actors {
                    if !self.tnt_seen.iter().any(|(seen, _)| seen == id) {
                        self.tnt_seen.push((*id, clock));
                    }
                }
                let alive: Vec<u64> = self.tnt_actors.iter().map(|(id, _)| *id).collect();
                self.tnt_seen.retain(|(id, _)| alive.contains(id));
            }
        }
        if self.tnt_actors.is_empty() {
            if self.tnt_drawn {
                let _ = draw::replace_frame_group(TNT_GROUP, &[]);
                self.tnt_drawn = false;
            }
            return;
        }
        let Ok(Some(camera)) = gameplay::read_camera() else { return };
        let (yaw, pitch) = (f64::from(camera.yaw).to_degrees(), f64::from(camera.pitch).to_degrees());
        if !(yaw.is_finite() && pitch.is_finite()) {
            return;
        }
        let rotation = hitbox::look_rotation(yaw, pitch);
        let axes = crate::damage::camera_axes(yaw, pitch);
        let linear = |c: f32| c.powf(2.2);
        let mut draws: Vec<draw::Instance> = Vec::new();
        let (mut plate, mut outline, mut placed) = (Vec::new(), Vec::new(), Vec::new());
        for (id, at) in &self.tnt_actors {
            let first = self.tnt_seen.iter().find(|(seen, _)| seen == id).map_or(self.clock, |(_, t)| *t);
            let left = (TNT_FUSE - (self.clock - first)).max(0.0);
            // Green while there is time, yellow, then red.
            let colour = if left > 2.0 { [0.35, 1.0, 0.35] } else if left > 1.0 { [1.0, 0.9, 0.25] } else { [1.0, 0.3, 0.3] };
            let [r, g, b] = colour.map(linear);
            plate.clear();
            outline.clear();
            placed.clear();
            let above = [at[0], at[1] + 1.5, at[2]];
            crate::damage::place_label(&format!("{left:.1}"), above, 0.3, axes, 0, &mut plate, &mut outline, &mut placed);
            for p in &placed {
                draws.push(draw::Instance {
                    geometry: self.dmg_glyphs[p.glyph],
                    material: None,
                    params: Vec::new(),
                    anchor: draw::Anchor::World(draw::WorldAnchor { dimension: self.dimension, origin: (p.origin[0], p.origin[1], p.origin[2]) }),
                    transform: draw::Transform { translation: (p.shift[0], p.shift[1], p.shift[2]), rotation, scale: (p.scale.max(0.002), p.scale.max(0.002), p.scale.max(0.002)) },
                    tint: (r, g, b, 1.0),
                    depth: draw::Depth::Test,
                    blend: draw::Blend::Premultiplied,
                    width: 1.0,
                    width_mode: draw::WidthMode::Pixels,
                    join: draw::Join::Round,
                    cap: draw::Cap::Round,
                    clip: None,
                });
            }
        }
        match draw::replace_frame_group(TNT_GROUP, &draws) {
            Ok(()) => self.tnt_drawn = true,
            Err(error) => {
                if error.detail == "geometry" {
                    // The host forgot the glyphs: they go up again.
                    self.dmg_glyphs.clear();
                } else {
                    self.tnt_failed = true;
                    let clean: String = error.detail.chars().filter(|c| !c.is_control()).take(100).collect();
                    let _ = host_hud::local_message(&format!("Vantage: the TNT countdown could not be drawn. ({clean})"));
                }
            }
        }
    }

    fn update_damage(&mut self, own: u64) {
        let Some((size, seconds, colour, hearts)) = self.settings.gameplay.damage_look() else {
            if self.dmg_drawn {
                let _ = draw::replace_frame_group(DAMAGE_GROUP, &[]);
                self.dmg_drawn = false;
            }
            self.dmg.clear();
            return;
        };
        if self.dmg_failed {
            return;
        }
        if !self.glyphs_ready() {
            return;
        }
        if self.frames % DAMAGE_REFRESH == 0 {
            if let Ok(actors) = crate::hive_host::counted(|| player_state::read_actors_limited(mod_api::MAX_READ_ACTORS as u32)) {
                let mut seen = Vec::with_capacity(actors.len());
                for observed in &actors {
                    let actor = &observed.actor;
                    if actor.runtime_id == own {
                        continue;
                    }
                    seen.push(actor.runtime_id);
                    let (x, y, z) = actor.position;
                    let bounds: Vec<([f32; 3], [f32; 3])> =
                        actor.boxes.iter().map(|b| ([b.min.0, b.min.1, b.min.2], [b.max.0, b.max.1, b.max.2])).collect();
                    let top = hitbox::boxes([x, y, z], &bounds)
                        .iter()
                        .map(|b| b.min[1] + f64::from(b.size[1]))
                        .reduce(f64::max)
                        .unwrap_or(f64::from(y) + 1.8);
                    self.dmg.note(actor.runtime_id, actor.health, [f64::from(x), top + 0.3, f64::from(z)], self.clock);
                }
                self.dmg.retain_seen(&seen);
            }
        }
        self.dmg.expire(self.clock, seconds);
        if self.dmg.floats.is_empty() {
            if self.dmg_drawn {
                let _ = draw::replace_frame_group(DAMAGE_GROUP, &[]);
                self.dmg_drawn = false;
            }
            return;
        }
        let Ok(Some(camera)) = gameplay::read_camera() else { return };
        let (yaw, pitch) = (f64::from(camera.yaw).to_degrees(), f64::from(camera.pitch).to_degrees());
        if !(yaw.is_finite() && pitch.is_finite()) {
            return;
        }
        let eye = [f64::from(camera.position.x), f64::from(camera.position.y), f64::from(camera.position.z)];
        let mut placed = Vec::new();
        for float in &self.dmg.floats {
            crate::damage::place(float, self.clock, seconds, size, hearts, eye, yaw, &mut placed);
        }
        let rotation = hitbox::look_rotation(yaw, pitch);
        // The camera's right and up in the world (the glyphs' x and y), for the shadow's offset.
        let (yaw_r, pitch_r) = (yaw.to_radians(), pitch.to_radians());
        let right = [yaw_r.cos() as f32, 0.0, -yaw_r.sin() as f32];
        let up = [(pitch_r.sin() * yaw_r.sin()) as f32, pitch_r.cos() as f32, (pitch_r.sin() * yaw_r.cos()) as f32];
        // Display colours go to the host as linear ones, like the game's own text.
        let linear = |c: f32| c.powf(2.2);
        let [r, g, b] = [linear(colour[0]), linear(colour[1]), linear(colour[2])];
        // Like the game's text: the shadow first, the same pixels one pixel right and down in a quarter of the
        // colour, a hair farther along the line of sight (never sideways, so it cannot slide off its glyph).
        let mut draws: Vec<draw::Instance> = Vec::with_capacity(placed.len() * 2);
        for pass in 0..2 {
            for p in &placed {
                let (shift, tint) = if pass == 0 {
                    let to = [
                        (p.origin[0] - eye[0]) as f32 + p.shift[0],
                        (p.origin[1] - eye[1]) as f32 + p.shift[1],
                        (p.origin[2] - eye[2]) as f32 + p.shift[2],
                    ];
                    let length = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt().max(0.01);
                    let back = p.scale * 0.5 / length;
                    let k = p.scale;
                    (
                        [
                            p.shift[0] + right[0] * k - up[0] * k + to[0] * back,
                            p.shift[1] + right[1] * k - up[1] * k + to[1] * back,
                            p.shift[2] + right[2] * k - up[2] * k + to[2] * back,
                        ],
                        (r * 0.25 * p.alpha, g * 0.25 * p.alpha, b * 0.25 * p.alpha, p.alpha),
                    )
                } else {
                    (p.shift, (r * p.alpha, g * p.alpha, b * p.alpha, p.alpha))
                };
                draws.push(draw::Instance {
                    geometry: self.dmg_glyphs[p.glyph],
                    material: None,
                    params: Vec::new(),
                    anchor: draw::Anchor::World(draw::WorldAnchor {
                        dimension: self.dimension,
                        origin: (p.origin[0], p.origin[1], p.origin[2]),
                    }),
                    transform: draw::Transform {
                        translation: (shift[0], shift[1], shift[2]),
                        rotation,
                        scale: (p.scale, p.scale, p.scale),
                    },
                    tint,
                    depth: draw::Depth::Test,
                    blend: draw::Blend::Premultiplied,
                    width: 1.0,
                    width_mode: draw::WidthMode::Pixels,
                    join: draw::Join::Round,
                    cap: draw::Cap::Round,
                    clip: None,
                });
            }
        }
        match draw::replace_frame_group(DAMAGE_GROUP, &draws) {
            Ok(()) => self.dmg_drawn = true,
            Err(error) => {
                self.dmg_failed = true;
                let clean: String = error.detail.chars().filter(|c| !c.is_control()).take(100).collect();
                let _ = host_hud::local_message(&format!("Vantage: the damage numbers could not be drawn. ({clean})"));
            }
        }
    }

    /// Target HUD: the actor an attack from here would reach (the host's `hittable`), with its name and health,
    /// read a few times a second (a frame apart from the Damage Indicator's read). It stays on screen
    /// for `TARGET_HOLD` seconds after you look away, and follows the actor's health while it is near.
    fn update_target(&mut self, own: u64) {
        if !self.settings.is_on(Element::TargetHud) {
            self.target = None;
            return;
        }
        if self.frames % TARGET_REFRESH != 1 {
            return;
        }
        if let Ok(actors) = crate::hive_host::counted(|| player_state::read_actors_limited(mod_api::MAX_READ_ACTORS as u32)) {
            let aimed = actors.iter().find(|o| o.actor.runtime_id != own && o.actor.hittable == Some(true));
            let held = actors.iter().find(|o| o.actor.runtime_id == self.target_id && o.actor.runtime_id != own);
            match (aimed, held) {
                (Some(o), _) => {
                    self.target_id = o.actor.runtime_id;
                    self.target_seen = self.clock;
                    self.target = Some(target_view(o));
                }
                (None, Some(o)) if self.target.is_some() && self.clock - self.target_seen < TARGET_HOLD => {
                    self.target = Some(target_view(o));
                }
                _ => {}
            }
        }
        if self.target.is_some() && self.clock - self.target_seen >= TARGET_HOLD {
            self.target = None;
        }
    }

    /// WAILA: what the block the crosshair is on is. The host keeps a terrain read only for the callback that made it,
    /// so while a block is aimed at its one-cell region is read every frame (with the stamp last received, so an
    /// unchanged answer costs nothing); the answer arrives a frame or two after the block changes.
    fn update_waila(&mut self) {
        let on = self.settings.is_on(Element::Waila) && self.in_gameplay;
        let block = if on {
            match gameplay::read_block_target() {
                Ok(Some(p)) => crate::waila::block_of([p.x, p.y, p.z]),
                _ => None,
            }
        } else {
            None
        };
        let Some(block) = block else {
            // Looking at nothing for a moment (between blocks, a gap in the aim) keeps the box up: it goes after a
            // quarter second, so it does not blink.
            self.waila_stamp = None;
            self.waila_block = None;
            if !on || self.clock - self.waila_seen >= WAILA_HOLD {
                self.waila = None;
            }
            self.publish_waila_lines();
            return;
        };
        self.waila_seen = self.clock;
        self.waila_eye = if self.settings.waila_dist {
            gameplay::read_camera().ok().flatten().map(|c| [c.position.x, c.position.y, c.position.z])
        } else {
            None
        };
        if self.waila_block != Some(block) {
            // Another block: the last one stays shown until the answer for this one comes (a frame or two).
            self.waila_block = Some(block);
            self.waila_stamp = None;
        }
        let query = terrain::RegionQuery {
            min: terrain::Position { x: block[0], y: block[1], z: block[2] },
            max: terrain::Position { x: block[0] + 1, y: block[1] + 1, z: block[2] + 1 },
            geometry: terrain::Geometry::SELECTION,
        };
        let after = self.waila_stamp.clone();
        match crate::arena::receiving(|| terrain::read_region(query, after)) {
            Ok(terrain::ReadResult::Changed(region)) => {
                self.waila_stamp = Some(region.stamp.clone());
                let loaded = region.loaded.first().is_some_and(|word| word & 1 != 0);
                let state = region.states.first().and_then(|i| region.palette.get(usize::from(*i)));
                self.waila = match (loaded, state) {
                    (true, Some(state)) => match state.identifier.as_deref() {
                        Some(id) if !id.is_empty() && !id.ends_with("air") => {
                            Some(crate::waila::describe(id, state.canonical_state.as_deref(), &state.tags, block))
                        }
                        _ => None,
                    },
                    _ => None,
                };
            }
            // Same answer as before, or not ready yet: keep what is shown.
            _ => {}
        }
        self.publish_waila_lines();
    }

    /// What WAILA shows now: the mob aimed at, else the block (with the held tool's verdict and the distance).
    fn waila_shown(&self) -> Option<crate::waila::WailaView> {
        if let Some(mob) = &self.waila_mob {
            return Some(mob.clone());
        }
        let mut block = self.waila.clone()?;
        if let Some(mining) = block.mining {
            block.harvest = Some(mining.can_harvest(&self.armor[4].item));
        }
        if self.settings.waila_dist {
            block.distance = self.waila_eye.and_then(|eye| crate::waila::distance_tenths(eye, block.pos));
        }
        Some(block)
    }

    /// WAILA's mob: the actor an attack from here would reach, with its health, read a few times a second like the
    /// Target HUD's (a frame apart from it) and kept for a moment after the read finds none.
    fn update_waila_mob(&mut self, own: u64) {
        if !(self.settings.is_on(Element::Waila) && self.settings.waila_entity && self.in_gameplay) {
            self.waila_mob = None;
            return;
        }
        if self.frames % TARGET_REFRESH == 2 {
            if let Ok(actors) = crate::hive_host::counted(|| player_state::read_actors_limited(mod_api::MAX_READ_ACTORS as u32)) {
                if let Some(o) = actors.iter().find(|o| o.actor.runtime_id != own && o.actor.hittable == Some(true)) {
                    let name = hud::target_name(&o.actor.name, &o.actor.identifier);
                    self.waila_mob = Some(crate::waila::entity(&name, &o.actor.identifier, o.actor.health, o.max_health));
                    self.waila_mob_seen = self.clock;
                }
            }
        }
        if self.waila_mob.is_some() && self.clock - self.waila_mob_seen >= WAILA_HOLD {
            self.waila_mob = None;
        }
    }

    /// WAILA's box follows how many lines the aimed block has to tell (while it is hidden it keeps its size, so
    /// nothing is written again when it comes back).
    fn publish_waila_lines(&mut self) {
        if self.waila.is_none() && self.waila_mob.is_none() && !self.panel.moving {
            return;
        }
        if hud::set_waila_lines(hud::waila_line_total(self.waila_shown().as_ref(), &self.settings)) {
            self.settings_revision += 1;
        }
    }

    /// What the Crosshair is on: the actor the host says an attack from here would reach. Read a few
    /// times a second, and only while the Crosshair is on and a player or mob changes it.
    fn read_target(&mut self, own: u64) {
        let c = &self.settings.crosshair;
        if !(c.enabled && c.on_target && (c.players || c.mobs)) {
            self.xh_target = crate::crosshair::Target::None;
            return;
        }
        if self.frames % XH_REFRESH != 0 {
            return;
        }
        self.xh_target = match crate::hive_host::counted(|| player_state::read_actors_limited(mod_api::MAX_READ_ACTORS as u32)) {
            Ok(actors) => crate::crosshair::target_of(
                actors.iter().map(|o| crate::crosshair::Seen { runtime_id: o.actor.runtime_id, player: o.actor.player, hittable: o.actor.hittable }),
                own,
            ),
            Err(_) => crate::crosshair::Target::None,
        };
    }

    /// Highlight Players in Range (Reach display): the players and mobs you could hit from where you stand
    /// get the host's outline, in the chosen colour. The mask is depth-tested, so nobody shows
    /// through walls. Who is near is read a few times a second, and the host is only called when
    /// that set or the colour changes.
    #[inline(never)]
    fn sync_highlight(&mut self, own: u64) {
        if !(self.settings.is_on(Element::Reach) && self.settings.reach_highlight) {
            self.hl_refused = false;
            if self.hl_mask.is_some() || !self.hl_handles.is_empty() {
                self.clear_highlight();
            }
            return;
        }
        if self.hl_refused || self.frames % HL_REFRESH != 0 {
            return;
        }
        let eye = [self.feet_exact[0], self.feet_exact[1] + 1.62, self.feet_exact[2]];
        let Ok(actors) = crate::hive_host::counted(|| player_state::read_actors_limited(mod_api::MAX_READ_ACTORS as u32)) else { return };
        let mut ids: Vec<u64> = actors
            .iter()
            .filter_map(|observed| {
                let actor = &observed.actor;
                // Players and mobs (they have health); items and the like are left alone.
                let hittable = actor.hittable == Some(true);
                if actor.runtime_id == own
                    || actor.health.is_some_and(|health| health <= 0.0)
                    || !(actor.player || actor.health.is_some() || hittable)
                {
                    return None;
                }
                let bounds: Vec<([f32; 3], [f32; 3])> =
                    actor.boxes.iter().map(|b| ([b.min.0, b.min.1, b.min.2], [b.max.0, b.max.1, b.max.2])).collect();
                let (x, y, z) = actor.position;
                let near = hitbox::distance_to(eye, &hitbox::boxes([x, y, z], &bounds));
                // What the host says an attack from here would reach counts too, whatever the distance.
                (hittable || near.is_some_and(|d| d <= hitbox::HIT_RANGE)).then_some(actor.runtime_id)
            })
            .collect();
        ids.sort_unstable();
        ids.truncate(mod_api::drawing::MAX_MASK_ENTITIES);
        let colour = self.settings.reach_highlight_color;
        let mut held: Vec<u64> = self.hl_handles.iter().map(|(id, _)| *id).collect();
        held.sort_unstable();
        if self.hl_mask.is_some() && self.hl_colour == Some(colour) && held == ids {
            return;
        }
        if let Err(error) = self.apply_highlight(&ids, colour) {
            self.hl_refused = true;
            let clean: String = error.detail.chars().filter(|c| !c.is_control()).take(100).collect();
            let _ = host_hud::local_message(&format!("Vantage: the player highlight could not be set. ({clean})"));
        }
    }

    /// Brings the mask to `ids` in `colour`: made on first use, restyled when the colour changed,
    /// with a host reference for each player and the ones no longer near released.
    fn apply_highlight(&mut self, ids: &[u64], colour: usize) -> Result<(), draw::DrawError> {
        let mask = match self.hl_mask {
            Some(mask) => mask,
            None => {
                let mask = masks::create(masks::DepthPolicy::Test)?;
                self.hl_mask = Some(mask);
                self.hl_colour = None;
                mask
            }
        };
        if self.hl_colour != Some(colour) {
            let [r, g, b, _] = crate::config::rgba(colour, 1.0);
            let tone = |a: f32| masks::Rgba { r, g, b, a };
            masks::set_style(mask, Some(masks::Style { outline: tone(0.9), outline_radius: 2.0, glow: tone(0.45), glow_radius: 6.0 }))?;
            self.hl_colour = Some(colour);
        }
        // References are tracked the moment they exist, so a failure part-way leaks none.
        for &id in ids {
            if !self.hl_handles.iter().any(|(held, _)| *held == id) {
                let handle = masks::entity(id)?;
                self.hl_handles.push((id, handle));
            }
        }
        let chosen: Vec<draw::Handle> = ids.iter().filter_map(|id| self.hl_handles.iter().find(|(held, _)| held == id).map(|(_, h)| *h)).collect();
        masks::select(mask, &chosen)?;
        let old = std::mem::take(&mut self.hl_handles);
        for (id, handle) in old {
            if ids.contains(&id) {
                self.hl_handles.push((id, handle));
            } else {
                let _ = draw::remove(handle);
            }
        }
        Ok(())
    }

    /// Takes the highlight off everyone and gives the host its mask and references back.
    fn clear_highlight(&mut self) {
        if let Some(mask) = self.hl_mask.take() {
            let _ = masks::select(mask, &[]);
            let _ = masks::remove(mask);
        }
        for (_, handle) in self.hl_handles.drain(..) {
            let _ = draw::remove(handle);
        }
        self.hl_colour = None;
    }

    /// The active effects for Potion status, four times a second while it is on.
    #[inline(never)]
    fn read_potions(&mut self) {
        if !self.settings.is_on(Element::Potion) {
            self.potions.clear();
            return;
        }
        if self.clock - self.potions_checked < 0.25 {
            return;
        }
        self.potions_checked = self.clock;
        let Ok(read) = crate::hive_host::counted(|| player_state::read_effects()) else { return };
        let mut rows = Vec::new();
        for effect in read.map(|r| r.effects).unwrap_or_default() {
            if rows.len() >= potion::MAX_ROWS {
                break;
            }
            let (name, texture) = self.potion_facts_of(effect.effect_id);
            rows.push(potion::Row {
                id: effect.effect_id,
                level: (effect.amplifier.max(0) as u32).saturating_add(1),
                // Server ticks at 20 a second, shown rounded up like the game does.
                seconds: effect.remaining_ticks.map(|ticks| ticks.div_ceil(20).min(u64::from(u32::MAX)) as u32),
                name,
                texture,
            });
        }
        rows.sort_by_key(|row| row.id);
        if rows != self.potions {
            self.potions = rows;
        }
    }

    /// An effect's name and icon texture: from the host once (a name this mod does not know is made
    /// from the host's key), and asked again later when the host could not say.
    fn potion_facts_of(&mut self, id: i32) -> (String, String) {
        if let Some(known) = self.potion_facts.get(&id) {
            return known.clone();
        }
        let read = facts::read_effect(id);
        let host = read.as_ref().ok().and_then(Option::as_ref);
        let name = potion::effect_name(id)
            .map(str::to_owned)
            .or_else(|| host.map(|f| potion::name_from_key(&f.name_key)))
            .unwrap_or_else(|| "Effect".to_owned());
        let texture = host.map(|f| f.texture.clone()).unwrap_or_default();
        let facts = (name, texture);
        if read.is_ok() {
            self.potion_facts.insert(id, facts.clone());
        }
        facts
    }

    /// What Potion status shows now: the effects, or two samples while the elements are moved and
    /// there are none (so the box can be found and dragged).
    fn potion_rows(&self) -> Vec<potion::Row> {
        if !self.settings.is_on(Element::Potion) {
            return Vec::new();
        }
        if self.potions.is_empty() && self.panel.moving {
            let sample = |id: i32, level: u32, seconds: u32, texture: &str| potion::Row {
                id,
                level,
                seconds: Some(seconds),
                name: potion::effect_name(id).unwrap_or("Effect").to_owned(),
                texture: texture.to_owned(),
            };
            return vec![sample(1, 2, 95, "textures/ui/speed_effect"), sample(10, 1, 41, "textures/ui/regeneration_effect")];
        }
        self.potions.clone()
    }

    /// The worn armour, four times a second, while the Armor status element is on.
    #[inline(never)]
    fn read_armor(&mut self, busy: bool) {
        // Reading the inventory is a big call: not in the frames where something is being dragged,
        // and a busy frame lets it wait (up to a second).
        let since = self.clock - self.armor_checked;
        // The Crosshair's weapon preset needs the held item too.
        let weapon = self.settings.crosshair.enabled && self.settings.crosshair.weapon;
        // So does Saturation's preview of the food in your hand.
        let eating = hud::saturation_parts(&self.settings).eating();
        let inventory = self.settings.is_on(Element::Inventory);
        let counter = self.settings.is_on(Element::ItemCounter);
        let potions = self.settings.is_on(Element::PotionCounter);
        // WAILA's tool mark needs the held item.
        let waila_tool = self.settings.is_on(Element::Waila) && self.settings.waila_tool;
        // View angles' elytra option looks at the chest slot.
        let elytra = self.settings.is_on(Element::ViewAngles) && self.settings.va_elytra;
        if !(self.settings.is_on(Element::Armor) || inventory || counter || potions || weapon || eating || waila_tool || elytra)
            || since < 0.25
            || (busy && since < 1.0)
            || self.grab.is_some()
        {
            return;
        }
        self.armor_checked = self.clock;
        // The Item counter's item is found by its identifier in the same read.
        self.armor_reader.count_item =
            if counter { crate::config::COUNTER_ITEMS[self.settings.counter_item.min(crate::config::COUNTER_ITEMS.len() - 1)].0 } else { "" };
        self.armor_reader.count_potions = potions;
        if let Some(worn) = self.armor_reader.read() {
            self.armor = worn;
        }
        self.item_count = self.armor_reader.counted;
        self.potion_count = self.armor_reader.potions;
        self.inventory = self.armor_reader.inventory.clone();
        self.holding_ranged = self.armor_reader.ranged;
    }

    /// Reads the player's key bindings (movement, attack, use, jump) while something needs them,
    /// a page per call, and rewrites the HUD when the names on the key cells change.
    fn poll_binds(&mut self) {
        let wanted = self.settings.is_on(Element::Keystrokes)
            || self.settings.is_on(Element::Cps)
            || self.settings.item_delay_fix
            || self.settings.gameplay.toggle_sprint
            || self.settings.gameplay.toggle_sneak;
        if !wanted || (!self.binds_collector.busy() && self.clock - self.binds_checked < BINDS_RECHECK) {
            return;
        }
        self.binds_checked = self.clock;
        let (after, offset) = self.binds_collector.request();
        let page = match input::read_bindings(after, offset, crate::binds::PAGE) {
            Ok(page) => page.map(|page| crate::binds::Page {
                revision: page.revision,
                total: page.total,
                entries: page
                    .entries
                    .into_iter()
                    .map(|entry| crate::binds::Entry {
                        gameplay: matches!(entry.context, input::BindingContext::Gameplay),
                        plain: !(entry.modifiers.shift || entry.modifiers.control || entry.modifiers.alt || entry.modifiers.super_key),
                        action: entry.action,
                        key: entry.key,
                    })
                    .collect(),
            }),
            // Not sampled yet, or no allowance left in this callback: the keys stay as they are.
            Err(_) => return,
        };
        if let Some(binds) = self.binds_collector.accept(page) {
            if binds != self.binds {
                self.binds = binds;
                // The cells show the new names: the layer is written again.
                if hud::set_key_labels(self.binds.labels()) {
                    self.settings_revision += 1;
                }
            }
        }
    }

    fn read_controls(&mut self) {
        self.menu_busy = false;
        // A native text field (the Config page's code boxes) owns the keyboard while it is edited.
        let native = self.panel_open_seen && (self.panel.on_config() || self.panel.on_mods() || self.panel.on_waypoints() || self.panel.edit_open()) && input::read_native_editing().unwrap_or(false);
        self.panel.set_native(&mut self.settings, native);
        let capturing = self.panel.is_capturing();
        let custom_menu = crate::menu::has_own_key(&self.settings.menu_key);
        // Sorting the option keys every frame is wasted work: they change only with a setting.
        if self.bound_cache.is_none() {
            let mut keys = self.settings.reserved_keys();
            if custom_menu {
                keys.push(self.settings.menu_key.clone());
            }
            self.bound_cache = Some(keys);
        }
        let mut bound = self.bound_cache.clone().unwrap_or_default();
        let keystrokes = self.settings.is_on(Element::Keystrokes);
        // CPS is counted for its own element and for the mouse keys, which show it too.
        let cps_on = self.settings.is_on(Element::Cps)
            || (keystrokes && self.settings.show_mouse && self.settings.style(Element::Keystrokes).labels);
        // Clicks are read for CPS (whatever keys or buttons the player attacks and uses with), and
        // the left button while dragging an element.
        let item_fix = self.settings.item_delay_fix;
        if cps_on {
            push_new(&mut bound, self.binds.keys(crate::binds::ATTACK));
            push_new(&mut bound, self.binds.keys(crate::binds::USE));
        }
        if self.settings.gameplay.toggle_sprint {
            push_new(&mut bound, self.binds.sprint_keys());
        }
        if self.settings.gameplay.toggle_sneak {
            push_new(&mut bound, self.binds.sneak_keys());
        }
        if self.panel.moving {
            push_new(&mut bound, &["MouseLeft".to_owned()]);
        }
        // The item delay fix reacts to a fresh use press.
        if item_fix && !cps_on {
            push_new(&mut bound, self.binds.keys(crate::binds::USE));
            push_new(&mut bound, self.binds.keys(crate::binds::ATTACK));
        }
        // Markers: a middle click takes a mark off.
        if self.settings.gameplay.mk_on && self.settings.gameplay.mk_middle {
            push_new(&mut bound, &["MouseMiddle".to_owned()]);
        }
        // An open menu reads the left button for its scroll bar (a press, and held while the thumb moves).
        let menu_mouse = self.panel_open_seen && !self.panel.moving;
        if menu_mouse && !bound.iter().any(|k| k == "MouseLeft") {
            bound.push("MouseLeft".to_owned());
        }
        let mut held: Vec<String> = if keystrokes {
            self.binds.all_keys()
        } else if self.panel.moving || menu_mouse {
            vec!["MouseLeft".to_owned()]
        } else {
            Vec::new()
        };
        if menu_mouse && !held.iter().any(|k| k == "MouseLeft") {
            held.push("MouseLeft".to_owned());
        }
        if let Some(key) = self.settings.zoom.held_key() {
            if !held.iter().any(|k| k == key) {
                held.push(key.to_owned());
            }
        }
        // Perspective: the Look behind key is read while held.
        let persp_keys: [String; 3] = if self.settings.gameplay.persp_on {
            ["persp_behind", "persp_snap", "persp_snap_fwd"].map(|id| self.settings.key(id).to_owned())
        } else {
            Default::default()
        };
        for key in &persp_keys {
            if !key.is_empty() && !held.iter().any(|k| k == key) {
                held.push(key.clone());
            }
        }
        // WAILA's Show details key, read while held.
        let details_key = if self.settings.is_on(Element::Waila) && self.settings.waila_details == 1 { self.settings.key("waila_details").to_owned() } else { String::new() };
        if !details_key.is_empty() && !held.iter().any(|k| *k == details_key) {
            held.push(details_key.clone());
        }
        let controls = match input::read_selected_controls(&input::Selection {
            keys_pressed: if capturing { None } else { Some(bound) },
            keys_held: Some(held),
            events: true,
            keys_admitted: Some(Vec::new()),
        }) {
            Ok(controls) => controls,
            Err(error) => {
                // No controls this frame: the menu's busy cue is not sent either (see `mods_core`).
                let _ = error;
                return;
            }
        };
        // Every time the menu opens, its pages start at the top (before the menu is built below).
        if controls.panel_open && !self.panel_open_seen {
            self.panel.reset_scroll();
        }
        // The frame the menu opens or closes: its key press (Right Shift by default) is not a sneak press.
        let panel_changed = controls.panel_open != self.panel_open_seen;
        let was_open = self.panel_open_seen;
        self.panel_open_seen = controls.panel_open;
        if let Some(until) = self.tab_grace {
            let own_close = controls.keys_pressed.iter().any(|k| k.as_str() == self.settings.menu_key || k.as_str() == "F10");
            if controls.panel_open {
                if self.clock > until {
                    self.tab_grace = None;
                }
            } else if was_open && !own_close && self.clock <= until {
                self.tab_grace = None;
                if panel::open().is_ok() {
                    self.panel_open_seen = true;
                }
            } else {
                self.tab_grace = None;
            }
        }
        self.in_gameplay = controls.focused && controls.gameplay && !controls.panel_open;
        // Toggle sprint / sneak: a fresh press in gameplay flips the mod's own latch.
        {
            let g = &self.settings.gameplay;
            // The menu keys (and the option keys) belong to the mod, never to the game's sprint or sneak.
            let menu_key = self.settings.menu_key.as_str();
            let reserved = self.bound_cache.as_deref().unwrap_or(&[]);
            let usable = self.in_gameplay && !panel_changed;
            let pressed: Vec<String> =
                controls.keys_pressed.iter().filter(|k| k.as_str() != menu_key && k.as_str() != "F10" && !reserved.contains(*k)).cloned().collect();
            if !g.toggle_sprint {
                self.sprint_latch = false;
            } else if usable && crate::binds::Binds::count_presses(self.binds.sprint_keys(), &pressed) % 2 == 1 {
                self.sprint_latch = !self.sprint_latch;
            }
            if !g.toggle_sneak {
                self.sneak_latch = false;
            } else if usable && crate::binds::Binds::count_presses(self.binds.sneak_keys(), &pressed) % 2 == 1 {
                self.sneak_latch = !self.sneak_latch;
            }
            // Touch controls are not keys: a new sprint or sneak hold in the admitted controls flips the latch too
            // (a key press that already flipped it this frame is not counted twice).
            if usable && (g.toggle_sprint || g.toggle_sneak) {
                if let Ok(Some(input)) = gameplay::read_movement_input() {
                    let key_sprint = crate::binds::Binds::count_presses(self.binds.sprint_keys(), &pressed) > 0;
                    let key_sneak = crate::binds::Binds::count_presses(self.binds.sneak_keys(), &pressed) > 0;
                    if g.toggle_sprint && input.sprint && !self.admitted_sprint && !key_sprint {
                        self.sprint_latch = !self.sprint_latch;
                    }
                    if g.toggle_sneak && input.sneak && !self.admitted_sneak && !key_sneak {
                        self.sneak_latch = !self.sneak_latch;
                    }
                    self.admitted_sprint = input.sprint;
                    self.admitted_sneak = input.sneak;
                }
            } else {
                self.admitted_sprint = false;
                self.admitted_sneak = false;
            }
        }
        let seconds = f64::from(controls.seconds);
        self.clock += if seconds.is_finite() { seconds.clamp(0.0, 1.0) } else { 0.0 };
        self.held = std::array::from_fn(|slot| self.binds.is_down(slot, &controls.keys_held));
        if hud::set_waila_held(!details_key.is_empty() && controls.keys_held.iter().any(|k| *k == details_key)) {
            self.settings_revision += 1;
        }
        if cps_on {
            // Clicks on a menu are not clicks in the game.
            let quiet = capturing || controls.panel_open;
            // While the CPS limiter is on, the left count is the attacks it let through, so the element shows the limit.
            let limited = LIMITED_CLICKS.replace(0) as usize;
            let left = if quiet {
                0
            } else if self.settings.gameplay.cpslim_on {
                limited
            } else {
                self.binds.presses(crate::binds::ATTACK, &controls.keys_pressed)
            };
            let right = if quiet { 0 } else { self.binds.presses(crate::binds::USE, &controls.keys_pressed) };
            let resolved = ACTION_CLICKS.replace([0, 0]);
            // One click is reported as a key press and as the game's resolved action, possibly a frame apart: count it once.
            let left = if quiet || self.settings.gameplay.cpslim_on { left } else { self.click_pairs[0].merge(self.clock, left, resolved[0] as usize) };
            let right = if quiet { 0 } else { self.click_pairs[1].merge(self.clock, right, resolved[1] as usize) };
            self.cps = [self.left.tick(self.clock, left), self.right.tick(self.clock, right)];
            // The statistics follow the button(s) the CPS element shows.
            let (shown, clicks) = match self.settings.cps_mode {
                crate::config::CpsMode::Left => (self.cps[0], left),
                crate::config::CpsMode::Right => (self.cps[1], right),
                _ => (self.cps[0] + self.cps[1], left + right),
            };
            self.cps_stats.note(shown, clicks);
        }
        // The player's bindings are read after the controls, so a refused read (the callback's
        // allowance) never costs the frame its keys.
        self.poll_binds();
        // Item delay fix: a fresh right-click soon after a use or a swing skips the game's wait between
        // item uses (the 200 ms after a use and the hold-off after a swing). A click with no recent use
        // or swing is never held back, so it is left alone: a pulse then would place or use twice. The
        // host still applies cooldowns, block rules, one use per tick and the server's checks. A refusal
        // (no permission, or no gameplay frame) is said once in chat.
        if item_fix && controls.focused && !controls.panel_open && !capturing {
            let used = self.binds.presses(crate::binds::USE, &controls.keys_pressed) > 0;
            let swung = self.binds.presses(crate::binds::ATTACK, &controls.keys_pressed) > 0;
            let held_back = self.last_use_or_swing.is_some_and(|at| self.clock - at < ITEM_DELAY_WINDOW);
            if used && held_back {
                match item_use::pulse_use() {
                    Ok(()) => self.item_fix_refused = false,
                    Err(error) if error.contains("denied") && !self.item_fix_refused => {
                        self.item_fix_refused = true;
                        let _ = host_hud::local_message("Vantage: item delay fix is off. Allow \"Item use\" for this mod.");
                    }
                    Err(_) => {}
                }
            }
            if used || swung {
                self.last_use_or_swing = Some(self.clock);
            }
        }
        // The touch buttons' taps are read here, before the menu hand-over below: the Menu button is the menu key on
        // a screen without a keyboard, and it needs the same order as the key (open cue first, `core_frame` after).
        let presses = touch::read_presses().unwrap_or_default();
        if !capturing && presses.iter().any(|id| id == TOUCH_MENU) {
            core_toggle(controls.panel_open);
        }
        // The Waypoint menu key: the menu, on the Waypoints tab.
        let wp_menu = self.settings.key("wp_menu").to_owned();
        if !capturing && controls.focused && !wp_menu.is_empty() && controls.keys_pressed.iter().any(|k| *k == wp_menu) {
            if !controls.panel_open {
                self.panel.show_waypoints_tab();
            }
            core_toggle(controls.panel_open);
        }
        // A rebound menu key opens and closes the menu (the embedded menu knows the default).
        if !capturing && custom_menu && controls.focused && controls.keys_pressed.iter().any(|k| *k == self.settings.menu_key) {
            core_toggle(controls.panel_open);
        }
        let mut effect = Effect::default();
        // Mods Core: a click on this mod in its list sets and opens the menu.
        let whole = || crate::panel::build(&self.settings, &self.panel);
        // The whole menu already carries every value, so no patch follows it, and the rest of
        // the frame is skipped: building and sending it is most of a callback's fuel.
        if core_frame(controls.panel_open, whole) == crate::mods_core::Event::Opened {
            self.skip_rest = true;
        }
        for note in core_take_notes().into_iter().take(3) {
            let clean: String = note.chars().filter(|c| !c.is_control()).take(120).collect();
            let _ = host_hud::local_message(&format!("Vantage menu: {clean}"));
        }
        // A press on the Done button is a press on the button, whatever is behind it. Anywhere
        // else, a press that lands on a resize handle or a HUD element picks it up.
        let pick_up = self.panel.moving
            && controls.keys_pressed.iter().any(|k| k == "MouseLeft")
            && !self.over_done(controls.pointer)
            && (self.button_under(controls.pointer).is_some() || self.handle_under(controls.pointer).is_some() || self.element_under(controls.pointer));
        // The pointer in the menu's own units: to open a list or the number editor under the button
        // pressed, and for the scroll bar.
        let left_down = controls.keys_pressed.iter().any(|k| k == "MouseLeft");
        let left_held = controls.keys_held.iter().any(|k| k == "MouseLeft");
        if controls.panel_open && (!controls.events.is_empty() || controls.wheel != 0.0 || left_down || left_held) {
            self.panel.pointer = match (controls.pointer, panel::read_layout()) {
                (Some((px, py)), Some(layout)) if layout.scale > 0.0 => Some((
                    (px - layout.offset.0) / layout.scale - (layout.width - crate::menu_controls::WIDTH as f32) / 2.0,
                    (py - layout.offset.1) / layout.scale - (layout.height - crate::menu_controls::HEIGHT as f32) / 2.0,
                )),
                _ => None,
            };
        }
        for event in &controls.events {
            if pick_up && event.id == "move_done" {
                continue;
            }
            effect = effect.merge(self.panel.event(&mut self.settings, &event.id, event.value));
        }
        // The Mods page is drawn by the toolkit: it reads the raw pointer, and its clicks are ordinary menu events.
        if controls.panel_open && self.settings.new_menu && crate::menu_v2::GUI_MENU && self.panel.tab_number() == crate::menu_controls::MODS_PAGE {
            if let Some(layout) = panel::read_layout().filter(|layout| layout.scale > 0.0) {
                let pointer = controls
                    .pointer
                    .map(|(px, py)| vantage_gui::geom::Vec2::new((px - layout.offset.0) / layout.scale, (py - layout.offset.1) / layout.scale));
                let frame = crate::gui::Input {
                    pointer,
                    pressed: left_down,
                    held: left_held,
                    wheel: controls.wheel,
                    dt: controls.seconds.clamp(0.0, 0.25),
                    viewport: [layout.width, layout.height],
                    dismiss: controls.keys_pressed.iter().any(|k| matches!(k.as_str(), "Enter" | "NumpadEnter" | "Escape")),
                };
                let (scene, actions) = self.gui.frame(&self.settings, &self.panel, frame);
                vantage_gui::bridge::publish(scene);
                for action in actions {
                    match action {
                        crate::gui::Action::Event(id) => effect = effect.merge(self.panel.event(&mut self.settings, &id, 1.0)),
                        crate::gui::Action::Close => {
                            let _ = panel::close();
                        }
                    }
                }
            }
        }
        // Texts typed in the Config page's fields and the hex editor's (before the keys, so Enter applies what was just typed).
        if controls.panel_open && (self.panel.on_config() || self.panel.on_mods() || self.panel.on_waypoints() || self.panel.hex_open() || (self.settings.new_menu && crate::menu_v2::handles(self.panel.tab_number()))) {
            let read = input::read_field_events();
            if let Ok(events) = read {
                for event in events {
                    match event {
                        input::FieldEvent::Change(text) => effect = effect.merge(self.panel.field(&mut self.settings, &text.id, &text.value, true)),
                        input::FieldEvent::Submit(text) => effect = effect.merge(self.panel.field(&mut self.settings, &text.id, &text.value, false)),
                        input::FieldEvent::Key(_) => {}
                    }
                }
            }
        }
        effect = effect.merge(self.panel.wheel(controls.wheel));
        if controls.panel_open {
            effect = effect.merge(self.panel.scroll_frame(self.panel.pointer, left_down, left_held, controls.wheel));
            // The picker's card is a button: the host says whether a press that began on it is still held, wherever the pointer
            // has moved, even when it keeps that press out of the key lists.
            let card_held = self.panel.hex_open() && panel::read_press().is_some_and(|press| press.held && press.id == "ed_hold");
            if card_held {
                self.panel.pointer = controls.pointer.zip(panel::read_layout()).filter(|(_, layout)| layout.scale > 0.0).map(|((px, py), layout)| {
                    (
                        (px - layout.offset.0) / layout.scale - (layout.width - crate::menu_controls::WIDTH as f32) / 2.0,
                        (py - layout.offset.1) / layout.scale - (layout.height - crate::menu_controls::HEIGHT as f32) / 2.0,
                    )
                });
            }
            effect = effect.merge(self.panel.picker_frame(self.panel.pointer, left_down, left_held || card_held));
            effect = effect.merge(self.panel.typing(&mut self.settings, &controls.keys_pressed));
        }
        effect = effect.merge(self.panel.capture(&mut self.settings, &controls.keys_pressed));
        effect = effect.merge(self.panel.tick(controls.seconds));
        if controls.panel_open {
            effect = effect.merge(self.panel.sync_waypoints(&self.settings, &self.world_id, self.dimension, [self.feet[0], self.feet[1] - 1, self.feet[2]], self.have_world));
        }
        effect = effect.merge(self.panel.work(&mut self.settings));
        // Touch controls: "Chunks" and "Hitbox" buttons toggle those options, as their keys do.
        if !self.touch_set {
            // One row along the top right edge, all the same size and evenly spaced: Menu, Zoom, Chunks, Hitbox (the
            // last against the edge). Offsets point inward from the anchored corner. The player can still move and
            // resize each button in the game's touch layout editor.
            let button = |id: &str, label: &str, slot: f32| touch::Button {
                id: id.into(),
                label: label.into(),
                anchor: touch::Anchor::TopRight,
                offset: touch::Point { x: 6.0 + slot * (TOUCH_W + TOUCH_GAP), y: 8.0 },
                size: touch::Point { x: TOUCH_W, y: TOUCH_H },
            };
            let buttons = [
                button(TOUCH_MENU, "Menu", 3.0),
                button(TOUCH_ZOOM, "Zoom", 2.0),
                button(TOUCH_CHUNKS, "Chunks", 1.0),
                button(TOUCH_HITBOX, "Hitbox", 0.0),
            ];
            self.touch_set = touch::set_buttons(&buttons).is_ok();
        }
        let zoom_tap = presses.iter().any(|id| id == TOUCH_ZOOM);
        let taps: Vec<&'static str> = presses
            .iter()
            .filter_map(|id| match id.as_str() {
                TOUCH_CHUNKS => Some("borders"),
                TOUCH_HITBOX => Some("hitboxes"),
                _ => None,
            })
            .collect();
        // The wheel steps the zoom while the zoom key is held (hold mode with step keys on). The claim
        // keeps the wheel from also scrolling the hotbar; it is sent when it changes and now and then again.
        let zoom_scroll = self.zoom_wheel(controls.focused && controls.gameplay && !controls.panel_open && !capturing);
        // The zoom works while playing with nothing open, as the option keys do. The host puts
        // the normal view back whenever it is not renewed, so a view is sent every frame it lasts.
        let view = self.zoomer.frame(
            &self.settings.zoom,
            &zoom::Input {
                active: !capturing && controls.focused && controls.gameplay && !controls.panel_open,
                pressed: &controls.keys_pressed,
                held: &controls.keys_held,
                tap: zoom_tap,
                scroll: zoom_scroll,
                seconds: f64::from(controls.seconds),
            },
        );
        if let Some(view) = view {
            let _ = camera::set_view_scale(view.fov_scale, view.look_scale);
        }
        // Perspective: Snaplook and Look behind are renewed every frame they last (the host puts the player's own
        // view back when they are not). Freelook and the view cycle are the game's own actions.
        self.sync_game_keys();
        self.sync_glass();
        if self.poll_game_keys() {
            effect = effect.merge(Effect { save: true, republish: true, rebind: false });
        }
        if self.settings.gameplay.persp_on && !capturing && controls.focused && controls.gameplay && !controls.panel_open {
            let is_held = |key: &String| !key.is_empty() && controls.keys_held.iter().any(|k| k == key);
            let was_pressed = |key: &String| !key.is_empty() && controls.keys_pressed.iter().any(|k| k == key);
            // Snaplook: the Third Person key (from behind) and the Forward View key (from the front), held, or
            // pressed to switch on and off in Toggle Press Mode.
            let snap = if self.settings.gameplay.persp_snap_toggle {
                for (n, key) in [(1u8, &persp_keys[1]), (2u8, &persp_keys[2])] {
                    if was_pressed(key) {
                        self.snap_toggled = if self.snap_toggled == n { 0 } else { n };
                    }
                }
                self.snap_toggled
            } else {
                self.snap_toggled = 0;
                if is_held(&persp_keys[2]) { 2 } else if is_held(&persp_keys[1]) { 1 } else { 0 }
            };
            match snap {
                1 => {
                    let _ = camera::set_perspective(Some(camera::Perspective::ThirdPersonBack));
                }
                2 => {
                    let _ = camera::set_perspective(Some(camera::Perspective::ThirdPersonFront));
                }
                _ => {}
            }
            if is_held(&persp_keys[0]) {
                let _ = camera::set_view_yaw_offset(180.0);
            }
        }
        // An option's own key (or touch button) toggles it while playing (not while a menu is
        // open).
        if !capturing && controls.focused && !controls.panel_open {
            let keyed = controls.keys_pressed.iter().filter_map(|key| PanelState::option_for_key(&self.settings, key));
            let ids: Vec<&str> = keyed.chain(taps.iter().copied()).collect();
            for id in ids {
                let Some(option) = self.settings.toggle_mut(id) else { continue };
                *option = !*option;
                let on = *option;
                let label = TOGGLES.iter().chain(LATER_TOGGLES.iter()).chain(MORE_TOGGLES.iter()).find(|(t, _)| *t == id).map_or(id, |(_, l)| *l);
                // The gameplay mods (sprint, sneak, time...) switch silently: no chat line.
                if !id.starts_with("gp_") {
                    let _ = host_hud::local_message(&format!("Vantage: {label} {}", if on { "ON" } else { "OFF" }));
                }
                effect = effect.merge(Effect { save: true, republish: true, rebind: false });
            }
            // The Waypoints keys: mark the spot you stand on, or remove the nearest one. They work with the
            // element on or off; the chat line says what happened.
            // Auto Text Hotkey: each bound key sends its command (a granted slash command only; the host refuses
            // any other and says why).
            if self.settings.gameplay.th_on {
                for slot in 0..4 {
                    let key = self.settings.key(["th_key0", "th_key1", "th_key2", "th_key3"][slot]).to_owned();
                    if key.is_empty() || !controls.keys_pressed.iter().any(|k| *k == key) {
                        continue;
                    }
                    let command = crate::gameplay::clean_command(&self.settings.gameplay.th_cmd[slot]);
                    if command.is_empty() {
                        continue;
                    }
                    if crate::gameplay::granted_name(&command).is_none() {
                        let _ = host_hud::local_message(&format!("Vantage: {command} is not an allowed command (spawn, home, hub, tpa, tpaccept, warp)"));
                    } else {
                        crate::hive_host::note_command();
                        if let Err(why) = gameplay::request_command(&command) {
                            let _ = host_hud::local_message(&format!("Vantage: {command} was refused: {why}"));
                        }
                    }
                }
            }
            // Hive auto requeue: its key queues the next game now.
            if self.settings.gameplay.hive_on {
                let key = self.settings.key("hive_rq").to_owned();
                if !key.is_empty() && controls.keys_pressed.iter().any(|k| *k == key) {
                    crate::hive_host::press_requeue();
                }
            }
            // The Markers keys: a short-lived mark on the block you aim at, and clearing them.
            if self.settings.gameplay.mk_on {
                let (place_key, clear_key, danger_key) =
                    (self.settings.key("mk_place").to_owned(), self.settings.key("mk_clear").to_owned(), self.settings.key("mk_danger").to_owned());
                let middle = self.settings.gameplay.mk_middle;
                for key in &controls.keys_pressed {
                    let danger = !danger_key.is_empty() && *key == danger_key;
                    if middle && key == "MouseMiddle" {
                        // A middle click on a marked block takes the mark off.
                        if let Ok(Some(p)) = gameplay::read_block_target() {
                            if p.x.is_finite() && p.y.is_finite() && p.z.is_finite() {
                                let pos = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
                                if let Some(at) = self.markers.iter().position(|(w, _)| w.pos == pos && w.world == self.world_id && w.dimension == self.dimension) {
                                    self.markers.remove(at);
                                    let _ = host_hud::local_message("Vantage: marker removed");
                                }
                            }
                        }
                    } else if (!place_key.is_empty() && *key == place_key) || danger {
                        let aimed = match gameplay::read_block_target() {
                            Ok(Some(p)) if p.x.is_finite() && p.y.is_finite() && p.z.is_finite() => Some([p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32]),
                            _ => None,
                        };
                        let message = match aimed {
                            Some(pos) => {
                                let seconds = crate::gameplay::MARKER_TIMES[self.settings.gameplay.mk_time.min(crate::gameplay::MARKER_TIMES.len() - 1)].1;
                                let until = if seconds > 0.0 { self.clock + seconds } else { f64::INFINITY };
                                // The same spot again takes the mark off; a new one past the limit replaces the oldest.
                                if let Some(at) = self.markers.iter().position(|(w, _)| w.pos == pos && w.world == self.world_id && w.dimension == self.dimension) {
                                    self.markers.remove(at);
                                    "Vantage: marker removed".to_owned()
                                } else {
                                    if self.markers.len() >= crate::gameplay::MARKERS_MAX {
                                        self.markers.remove(0);
                                    }
                                    let waypoint = crate::waypoints::Waypoint {
                                        name: if danger { "Danger" } else { "Marker" }.to_owned(),
                                        world: self.world_id.clone(),
                                        dimension: self.dimension,
                                        pos,
                                        color: if danger { self.settings.gameplay.mk_danger_color } else { self.settings.gameplay.mk_color },
                                        visible: true,
                                        group: String::new(),
                                    };
                                    self.markers.push((waypoint, until));
                                    format!("Vantage: {} marker placed at {} {} {}", if danger { "danger" } else { "a" }, pos[0], pos[1], pos[2])
                                }
                            }
                            None => "Vantage: no block in view to mark".to_owned(),
                        };
                        let _ = host_hud::local_message(&message);
                    } else if !clear_key.is_empty() && *key == clear_key {
                        self.markers.clear();
                        let _ = host_hud::local_message("Vantage: markers cleared");
                    }
                }
            }
            let (add_key, remove_key, undo_key, quick_key) = (
                self.settings.key("wp_add").to_owned(),
                self.settings.key("wp_remove").to_owned(),
                self.settings.key("wp_undo").to_owned(),
                self.settings.key("wp_quick").to_owned(),
            );
            for key in &controls.keys_pressed {
                let message = if !add_key.is_empty() && *key == add_key {
                    // The block you stand on, not the air above it.
                    let pos = [self.feet[0], self.feet[1] - 1, self.feet[2]];
                    let color = self.settings.gameplay.wp_color;
                    match crate::waypoints::add(&mut self.settings.gameplay.waypoints, &self.world_id, self.dimension, pos, color) {
                        Some(name) => format!("Vantage: {name} marked at {} {} {}", pos[0], pos[1], pos[2]),
                        None => format!("Vantage: {} waypoints is the most; remove one first", crate::waypoints::MAX_WAYPOINTS),
                    }
                } else if !quick_key.is_empty() && *key == quick_key {
                    // Quick-create: no menu, named by the "Quick Waypoints Name Format" setting.
                    let pos = [self.feet[0], self.feet[1] - 1, self.feet[2]];
                    let (color, base) = (self.settings.gameplay.wp_color, self.settings.gameplay.wp_quick_name.clone());
                    match crate::waypoints::add_named(&mut self.settings.gameplay.waypoints, &self.world_id, self.dimension, pos, color, &base, false) {
                        Some(name) => format!("Vantage: {name} marked at {} {} {}", pos[0], pos[1], pos[2]),
                        None => format!("Vantage: {} waypoints is the most; remove one first", crate::waypoints::MAX_WAYPOINTS),
                    }
                } else if !remove_key.is_empty() && *key == remove_key {
                    match crate::waypoints::remove_nearest(&mut self.settings.gameplay.waypoints, &self.world_id, self.dimension, self.feet_exact) {
                        Some(removed) => {
                            let message = format!("Vantage: {} removed (undo key within {} s)", removed.name, crate::waypoints::UNDO_SECONDS as u32);
                            self.panel.wp.set_undo(removed);
                            message
                        }
                        None => "Vantage: no waypoint here to remove".to_owned(),
                    }
                } else if !undo_key.is_empty() && *key == undo_key {
                    match self.panel.wp.restore(&mut self.settings) {
                        Ok(name) => format!("Vantage: {name} is back"),
                        Err(message) => format!("Vantage: {message}"),
                    }
                } else {
                    continue;
                };
                let _ = host_hud::local_message(&message);
                effect = effect.merge(Effect { save: true, ..Effect::default() });
            }
            // The Stopwatch's own keys: start / pause and reset. They work while it is shown, and
            // while it is left running hidden.
            let shown = self.settings.is_on(Element::Stopwatch);
            if shown || self.settings.stopwatch_keep {
                let pause = self.settings.key("sw_pause").to_owned();
                let reset = self.settings.key("sw_reset").to_owned();
                for key in &controls.keys_pressed {
                    let did = if !pause.is_empty() && *key == pause {
                        self.stopwatch.toggle();
                        if self.stopwatch.running() { "started" } else { "paused" }
                    } else if !reset.is_empty() && *key == reset {
                        self.stopwatch.reset();
                        "reset"
                    } else {
                        continue;
                    };
                    // Shown, the stopwatch itself says so; hidden, a line in the chat does.
                    if !shown {
                        let _ = host_hud::local_message(&format!("Vantage: stopwatch {did}"));
                    }
                }
            }
        }
        let revision = self.settings_revision;
        if self.panel.moving {
            if !controls.panel_open {
                // Closing the menu ends the move.
                self.panel.moving = false;
                self.grab = None;
                self.grab_piece = None;
                self.potion_drag = None;
                self.potion_resize = None;
                self.resize = None;
                self.piece_resize = None;
                self.bar_hold = 0.0;
                self.panel.dragging = false;
                effect = effect.merge(Effect { republish: true, ..Effect::default() });
            } else {
                effect = effect.merge(self.drag(controls.pointer, &controls.keys_pressed, &controls.keys_held));
                effect = effect.merge(self.bar_tick(controls.seconds));
            }
        }
        self.sync_handles(revision);
        self.apply(effect);
        // Write the settings once a slider has rested, and at once when the menu is closed.
        if let Some(due) = self.save_due {
            if self.clock >= due || !controls.panel_open {
                self.save_due = None;
                let _ = settings::save(&self.settings.to_file());
            }
        }
        if let Some(message) = self.panel.take_notice() {
            let _ = host_hud::local_message(&message);
        }
    }

    /// Whether the pointer is on the move bar's Done button (while the bar shows).
    fn over_done(&self, pointer: Option<(f32, f32)>) -> bool {
        let (Some((px, py)), Some(view)) = (pointer, host_hud::read_viewport()) else { return false };
        self.in_done_zone(view.width, px, py)
    }

    fn in_done_zone(&self, width: f32, px: f32, py: f32) -> bool {
        // The bar fades out while an element is held, and is not there to press then.
        if !self.panel.moving || self.panel.dragging {
            return false;
        }
        let dx = px - width * 0.5;
        dx >= DONE_ZONE[0] && dx <= DONE_ZONE[1] && py >= DONE_ZONE[2] && py <= DONE_ZONE[3]
    }

    /// The frame (a little outside the visible rectangle) of the element the editor is on (the one
    /// the menu edits), when it is on: the one that wears the resize handles.
    fn handled_rect(&self, view: [f32; 2]) -> Option<([f32; 4], bool)> {
        let element = Element::from_index(self.panel.selected);
        if !self.settings.is_on(element) {
            return None;
        }
        // Pieces moved apart each have their own frame and handles: the one picked last (the first at
        // the start). Their sides' middles stretch the piece.
        if element == Element::Armor && self.settings.style(element).armor_split {
            let slot = self.selected_piece()?;
            let rect = hud::armor_piece_rect(&self.settings, view, slot)?;
            return Some((hud::frame_rect(rect), true));
        }
        // Effects of Potion status moved apart each have their own frame, with corners that resize it.
        if element == Element::Potion && self.settings.potion_split {
            let id = self.selected_potion()?;
            let rect = hud::potion_pieces(&self.settings, view).into_iter().find(|(x, _)| *x == id)?.1;
            return Some((hud::frame_rect(rect), false));
        }
        // The middles of the sides stretch the box: only for an element whose box can be seen.
        let edges = element != Element::Armor && (element == Element::Keystrokes || self.settings.style(element).background);
        Some((hud::frame_rect(hud::visible_rect(&self.settings, element, view)), edges))
    }

    /// The effect of Potion status (moved apart) that wears the frame: the last one picked, or the first one.
    fn selected_potion(&self) -> Option<i32> {
        let view = host_hud::read_viewport()?;
        let pieces = hud::potion_pieces(&self.settings, [view.width, view.height]);
        if pieces.iter().any(|(id, _)| *id == self.potion_sel) { Some(self.potion_sel) } else { pieces.first().map(|(id, _)| *id) }
    }

    /// The armor piece (moved apart) that wears the handles: the last one picked, or the first one that shows.
    fn selected_piece(&self) -> Option<usize> {
        let slots = hud::armor_slots(self.settings.style(Element::Armor));
        if slots.contains(&self.armor_sel) { Some(self.armor_sel) } else { slots.first().copied() }
    }

    /// The gear or X on the handled element's frame that lies under the pointer, if any.
    fn button_under(&self, pointer: Option<(f32, f32)>) -> Option<hud::FrameButton> {
        let (Some((px, py)), Some(view)) = (pointer, host_hud::read_viewport()) else { return None };
        if !self.panel.moving {
            return None;
        }
        let (rect, _) = self.handled_rect([view.width, view.height])?;
        hud::button_at(rect, [px, py])
    }

    /// The corner of the handled element's handles that lies under the pointer, if any.
    fn handle_under(&self, pointer: Option<(f32, f32)>) -> Option<usize> {
        let (Some((px, py)), Some(view)) = (pointer, host_hud::read_viewport()) else { return None };
        if !self.panel.moving {
            return None;
        }
        let (rect, edges) = self.handled_rect([view.width, view.height])?;
        hud::handle_at(rect, edges, [px, py])
    }

    /// Keeps the resize handles on the element being edited while the menu is in its move mode,
    /// and takes them away when it is not. `revision` is the settings revision from before this
    /// frame's drag: a layer that is being rewritten anyway already carries the handles' places.
    fn sync_handles(&mut self, revision: u64) {
        // The centre lines show while elements are moved, if they are switched on.
        let guides = self.panel.moving && self.settings.show_guides;
        if guides != self.guides_sent {
            self.guides_sent = guides;
            hud::set_guides(guides);
            self.settings_revision += 1;
        }
        let wanted = if self.panel.moving {
            host_hud::read_viewport().and_then(|view| self.handled_rect([view.width, view.height]))
        } else {
            None
        };
        if wanted == self.handles_sent {
            return;
        }
        // The frame's size is in the layer's document: a different size needs a new document.
        let appeared_or_gone = wanted.map(|(r, edges)| ([r[2], r[3]], edges)) != self.handles_sent.map(|(r, edges)| ([r[2], r[3]], edges));
        hud::set_handles(wanted.map(|w| w.0), wanted.is_some_and(|w| w.1));
        self.handles_sent = wanted;
        if appeared_or_gone {
            // The layer has the handles in its document only while they show.
            self.settings_revision += 1;
        } else if let (Some((rect, edges)), true) = (wanted, self.settings_revision == revision) {
            let _ = host_hud::update_surface(&hud::handles_patch(rect, edges));
        }
    }

    /// Whether a HUD element that is on lies under the pointer.
    fn element_under(&self, pointer: Option<(f32, f32)>) -> bool {
        let (Some((px, py)), Some(view)) = (pointer, host_hud::read_viewport()) else { return false };
        let view = [view.width, view.height];
        Element::ALL.iter().any(|element| {
            if !self.settings.is_on(*element) {
                return false;
            }
            if *element == Element::Armor && self.settings.style(*element).armor_split {
                return hud::armor_piece_at(&self.settings, view, [px, py]).is_some();
            }
            if *element == Element::Potion && self.settings.potion_split {
                return hud::potion_piece_at(&self.settings, view, [px, py]).is_some();
            }
            let [x, y, w, h] = hud::rect(&self.settings, *element, view);
            px >= x && px <= x + w && py >= y && py <= y + h
        })
    }

    /// The move bar fades out while an element is held (so it never covers the place the element
    /// is going to) and back in a moment after it is let go. The host eases `#move_alpha`; this
    /// only says when it flips, with one cheap republish each way.
    fn bar_tick(&mut self, seconds: f32) -> Effect {
        let mut effect = Effect::default();
        if self.grab.is_some() || self.resize.is_some() || self.piece_resize.is_some() || self.potion_resize.is_some() {
            self.bar_hold = BAR_RETURN_DELAY;
            if !self.panel.dragging {
                self.panel.dragging = true;
                effect.republish = true;
            }
        } else if self.panel.dragging {
            self.bar_hold -= if seconds.is_finite() { seconds.clamp(0.0, 0.1) } else { 0.0 };
            if self.bar_hold <= 0.0 {
                self.panel.dragging = false;
                effect.republish = true;
            }
        }
        effect
    }

    /// Drags a HUD element with the left mouse button while the menu is in its move mode: the
    /// element under the pointer when the button goes down is the one that follows it.
    #[inline(never)]
    fn drag(&mut self, pointer: Option<(f32, f32)>, pressed: &[String], held: &[String]) -> Effect {
        let mut effect = Effect::default();
        let (Some((px, py)), Some(view)) = (pointer, host_hud::read_viewport()) else {
            return effect;
        };
        let view = [view.width, view.height];
        if pressed.iter().any(|k| k == "MouseLeft") {
            self.grab = None;
            self.grab_piece = None;
            self.potion_drag = None;
            self.potion_resize = None;
            self.resize = None;
            self.piece_resize = None;
            // The Done button is pressed, not what lies behind it.
            if self.in_done_zone(view[0], px, py) {
                return effect;
            }
            // The gear and the X on the frame come before the handles and what lies under them.
            let element = Element::from_index(self.panel.selected);
            match self.handled_rect(view).and_then(|(rect, _)| hud::button_at(rect, [px, py])) {
                Some(hud::FrameButton::Settings) => {
                    // Leave the editor and open the element's Style page.
                    self.panel.selected = element.index();
                    self.panel.dragging = false;
                    effect = self.panel.open_settings(element.index());
                    return effect;
                }
                Some(hud::FrameButton::Remove) => {
                    // The element is switched off (it comes back from the Mods tab); its frame goes with it.
                    self.settings.style_mut(element).enabled = false;
                    self.settings.sanitize();
                    self.save_due = Some(self.clock + SAVE_DELAY);
                    self.settings_revision += 1;
                    self.bound_cache = None;
                    effect.republish = true;
                    return effect;
                }
                None => {}
            }
            // A resize handle of the element being edited comes before anything under it.
            if let Some(corner) = self.handled_rect(view).and_then(|(rect, edges)| hud::handle_at(rect, edges, [px, py])) {
                if element == Element::Potion && self.settings.potion_split {
                    self.potion_resize = self.selected_potion().and_then(|id| hud::PotionResize::new(&self.settings, view, id, corner, [px, py]));
                } else if element == Element::Armor && self.settings.style(element).armor_split {
                    self.piece_resize = self.selected_piece().and_then(|slot| hud::PieceResize::new(&self.settings, view, slot, corner, [px, py]));
                } else {
                    self.resize = Some(hud::Resize::new(&self.settings, element, view, corner, [px, py]));
                }
                return effect;
            }
            // The last element drawn is on top.
            for element in Element::ALL.iter().rev().copied() {
                if !self.settings.is_on(element) {
                    continue;
                }
                if element == Element::Potion && self.settings.potion_split && !held.iter().any(|k| k == "ControlLeft" || k == "ControlRight") {
                    // Effects moved apart: each one is picked up on its own (Ctrl takes the whole list).
                    if let Some((id, [x, y, _, _])) = hud::potion_piece_at(&self.settings, view, [px, py]) {
                        self.grab = Some((element, [px - x, py - y]));
                        self.potion_drag = hud::PotionDrag::new(&self.settings, view, id);
                        self.potion_sel = id;
                        if self.panel.selected != element.index() {
                            self.panel.selected = element.index();
                            effect.republish = true;
                        }
                        break;
                    }
                    continue;
                }
                let picked = if element == Element::Armor && self.settings.style(element).armor_split {
                    // Separate pieces: each one is picked up on its own.
                    hud::armor_piece_at(&self.settings, view, [px, py]).and_then(|slot| {
                        hud::armor_pieces(&self.settings, view)
                            .into_iter()
                            .find(|(s, _)| *s == slot)
                            .map(|(_, [x, y, _, _])| (Some(slot), [px - x, py - y]))
                    })
                } else {
                    let [x, y, w, h] = hud::rect(&self.settings, element, view);
                    (px >= x && px <= x + w && py >= y && py <= y + h).then_some((None, [px - x, py - y]))
                };
                if let Some((piece, grab)) = picked {
                    self.grab = Some((element, grab));
                    self.grab_piece = piece;
                    if let Some(slot) = piece {
                        self.armor_sel = slot;
                    }
                    self.armor_drag = (element == Element::Armor).then(|| hud::ArmorDrag::new(&self.settings, view, piece));
                    if self.panel.selected != element.index() {
                        // The menu then edits the element you picked up.
                        self.panel.selected = element.index();
                        effect.republish = true;
                    }
                    break;
                }
            }
        }
        if let Some(resize) = &self.potion_resize {
            if !held.iter().any(|k| k == "MouseLeft") {
                self.potion_resize = None;
                self.settings.sanitize();
                self.save_due = Some(self.clock + SAVE_DELAY);
                effect.republish = true;
                return effect;
            }
            if resize.step(&mut self.settings, view, [px, py]) {
                self.viewport = Some(view);
                self.settings_revision += 1;
            }
            return effect;
        }
        if let Some(resize) = &self.piece_resize {
            if !held.iter().any(|k| k == "MouseLeft") {
                // Let go: keep the new size, and let the menu catch up.
                self.piece_resize = None;
                self.settings.sanitize();
                self.save_due = Some(self.clock + SAVE_DELAY);
                effect.republish = true;
                return effect;
            }
            // Armor status is the heaviest layer to write: a few rewrites a second while resized.
            if self.clock - self.drag_applied < ARMOR_RESIZE_STEP {
                return effect;
            }
            if resize.step(&mut self.settings, view, [px, py]) {
                self.drag_applied = self.clock;
                self.viewport = Some(view);
                self.settings_revision += 1;
            }
            return effect;
        }
        if let Some(resize) = &self.resize {
            if !held.iter().any(|k| k == "MouseLeft") {
                // Let go: keep the new size, and let the menu's sliders catch up with it.
                self.resize = None;
                self.settings.sanitize();
                self.save_due = Some(self.clock + SAVE_DELAY);
                effect.republish = true;
                return effect;
            }
            let element = Element::from_index(self.panel.selected);
            // Armor status is the heaviest layer to write: a few rewrites a second while resized.
            if element == Element::Armor && self.clock - self.drag_applied < ARMOR_RESIZE_STEP {
                return effect;
            }
            if resize.step(&mut self.settings, view, [px, py]) {
                self.drag_applied = self.clock;
                self.viewport = Some(view);
                self.settings_revision += 1;
                // The menu is sent its values once, when the handle is let go.
            }
            return effect;
        }
        let Some((element, grab)) = self.grab else { return effect };
        if !held.iter().any(|k| k == "MouseLeft") {
            // Let go: keep the new place, and let the menu's sliders catch up with it.
            self.grab = None;
            self.grab_piece = None;
            self.potion_drag = None;
            self.armor_drag = None;
            self.settings.sanitize();
            // The file is written a moment later, in another frame than this one's layer and menu.
            self.save_due = Some(self.clock + SAVE_DELAY);
            effect.republish = true;
            return effect;
        }
        if let Some(drag) = self.potion_drag.as_ref() {
            let wanted = [px - grab[0], py - grab[1]];
            let free = held.iter().any(|k| k == "ShiftLeft" || k == "ShiftRight");
            let snapping = (self.settings.align || self.settings.show_guides) && !free;
            if drag.step(&mut self.settings, view, wanted, snapping) {
                self.viewport = Some(view);
                self.settings_revision += 1;
            }
            return effect;
        }
        if element == Element::Armor {
            // Armor status is the heaviest layer to write (an icon and a text for every piece), so
            // while it is dragged the layer follows the pointer a few times a second, not every
            // frame, and not at all while the pointer rests.
            if self.clock - self.drag_applied < ARMOR_DRAG_STEP {
                return effect;
            }
            let wanted = [px - grab[0], py - grab[1]];
            let free = held.iter().any(|k| k == "ShiftLeft" || k == "ShiftRight");
            let snapping = (self.settings.align || self.settings.show_guides) && !free;
            if let Some(drag) = self.armor_drag.as_mut() {
                if let Some(patch) = drag.step(&mut self.settings, view, wanted, snapping) {
                    self.drag_applied = self.clock;
                    self.viewport = Some(view);
                    // Only the pieces' places are sent: the layer itself is not rewritten.
                    let _ = host_hud::update_surface(&patch);
                }
            }
            return effect;
        }
        let free = held.iter().any(|k| k == "ShiftLeft" || k == "ShiftRight");
        let wanted = [px - grab[0], py - grab[1]];
        let target = if (self.settings.align || self.settings.show_guides) && !free { hud::snap(&self.settings, element, view, wanted) } else { wanted };
        if target != hud::top_left(&self.settings, element, view) {
            self.settings.style_mut(element).position = Some(hud::place_at(&self.settings, element, view, target));
            self.viewport = Some(view);
            // The layer is redrawn every step; the menu (hidden while moving) is sent its
            // values once, when the element is let go.
            self.settings_revision += 1;
        }
        effect
    }

    /// Sends the page of the open tab: the whole document, in a frame of its own.
    #[inline(never)]
    fn send_tab(&mut self) {
        let page = crate::panel::build(&self.settings, &self.panel);
        match panel::set_content(&page) {
            Ok(()) => {
                let _ = panel::open();
            }
            Err(error) => {
                let clean: String = error.chars().filter(|c| !c.is_control()).take(120).collect();
                let _ = host_hud::local_message(&format!("Vantage tab: {clean}"));
            }
        }
    }

    #[inline(never)]
    fn apply(&mut self, effect: Effect) {
        if effect.save {
            // Written a moment later (see `SAVE_DELAY`), once the changes stop.
            self.save_due = Some(self.clock + SAVE_DELAY);
            self.settings_revision += 1;
            self.bound_cache = None;
        }
        if effect.rebind {
            self.bound_cache = None;
        }
        if effect.republish {
            // Values go as a patch: the embedded menu already has the document. The whole menu
            // is sent only if the host has none.
            if self.panel.take_tab_changed() {
                // Each tab is its own document (a whole menu is too big for one callback), and the click that
                // changed the tab already used part of this frame: the page is built at the start of the next
                // one, as when the menu opens (`send_tab`).
                self.tab_build_due = true;
                self.skip_rest = true;
                self.tab_grace = Some(self.clock + 1.0);
            } else if self.panel.tab_number() == crate::menu_controls::MODS_PAGE {
                self.patch_due = true;
                self.skip_rest = true;
            } else {
                self.send_patch();
            }
        }
        if effect.rebind {
            let mut keys = self.settings.reserved_keys();
            if crate::menu::has_own_key(&self.settings.menu_key) {
                keys.push(self.settings.menu_key.clone());
            }
            if self.reserved.as_ref() != Some(&keys) && input::reserve_keys(&keys).is_ok() {
                self.reserved = Some(keys);
            }
        }
    }

    #[inline(never)]
    fn send_patch(&mut self) {
        self.menu_busy = true;
        // The redesigned Mods page is a whole document: it is sent again instead of patched.
        let sent = if self.settings.new_menu && crate::menu_v2::handles(self.panel.tab_number()) {
            panel::set_content(&crate::panel::build(&self.settings, &self.panel))
        } else {
            panel::update_content(&crate::panel::patch(&self.settings, &self.panel))
        };
        if let Err(error) = sent {
            // No panel while the menu is closed: it is set whole when it opens. A patch the
            // host refuses while the menu is open is said once, so it is not silent.
            if self.panel_open_seen && self.patch_error.as_deref() != Some(error.as_str()) {
                let clean: String = error.chars().filter(|c| !c.is_control()).take(120).collect();
                let _ = host_hud::local_message(&format!("Vantage menu: {clean}"));
                self.patch_error = Some(error);
            }
        }
    }

    #[inline(never)]
    fn upload_shapes(&mut self) {
        let uploaded = (|| {
            Ok::<_, draw::DrawError>(Shapes {
                wall: line(shapes::chunk_wall(), false)?,
                post: line(shapes::post(64.0), false)?,
                ring: line(shapes::square(16.0), true)?,
                footprint: line(shapes::centred_square(monument::SIDE as f32), true)?,
                column: line(shapes::post((monument::TOP + 1 - monument::BOTTOM) as f32), false)?,
                cube: line(hitbox::cube(), false)?,
                ray: line(hitbox::ray(), false)?,
                spawn_area: line(crate::spawners::area(), false)?,
                activation: line(crate::spawners::circle(), false)?,
            })
        })();
        match uploaded {
            Ok(shapes) => self.shapes = Some(shapes),
            Err(error) => {
                self.draw_error = Some(error.detail);
                self.shapes_failed = true;
                let _ = host_hud::local_message(
                    "Vantage: world drawing is off. Allow \"World drawing\" for this mod to see the lines.",
                );
            }
        }
    }

    fn width(&self, scale: f64) -> f32 {
        (self.settings.line_width * scale) as f32
    }

    /// Redraws the chunk lines when you cross into another chunk or subchunk.
    fn update_borders(&mut self) {
        if self.shapes.is_none() {
            return;
        }
        let chunk = [self.feet[0].div_euclid(16), self.feet[1].div_euclid(16), self.feet[2].div_euclid(16)];
        let key = (
            self.dimension,
            chunk,
            if self.settings.borders { self.settings.grid.index() as u8 + 1 } else { 0 },
            self.settings.subchunks,
            (self.settings.line_width * 100.0) as u32,
        );
        if self.borders_key == Some(key) {
            return;
        }
        self.borders_key = Some(key);
        if !self.settings.borders {
            self.borders.clear();
            return;
        }
        let mut wanted = Vec::with_capacity(128);
        let dim = self.dimension;
        let (bottom, top) = world_height(dim);
        let [cx, sy, cz] = chunk;
        let base = [f64::from(cx * 16), f64::from(cz * 16)];
        // The current chunk's wall grid, two subchunks above and below.
        for section in sy - 2..=sy + 2 {
            let y = section * 16;
            if y < bottom || y >= top {
                continue;
            }
            wanted.push(instance(Shape::Wall, dim, [base[0], f64::from(y), base[1]], YELLOW, self.width(0.6)));
        }
        if self.settings.subchunks {
            for section in sy - 2..=sy + 3 {
                let y = section * 16;
                if y < bottom || y > top {
                    continue;
                }
                wanted.push(instance(Shape::Ring, dim, [base[0], f64::from(y), base[1]], BLUE, self.width(1.0)));
            }
        }
        // Tall corner posts: blue for this chunk, red for the ones around it.
        let reach = self.settings.grid.reach().max(0);
        let low = (sy * 16 - 64).max(bottom);
        for gx in -reach..=reach + 1 {
            for gz in -reach..=reach + 1 {
                let own = (gx == 0 || gx == 1) && (gz == 0 || gz == 1);
                if !own && reach == 0 {
                    continue;
                }
                let x = f64::from((cx + gx) * 16);
                let z = f64::from((cz + gz) * 16);
                let mut y = low;
                while y < (sy * 16 + 80).min(top) {
                    let color = if own { BLUE } else { RED };
                    wanted.push(instance(Shape::Post, dim, [x, f64::from(y), z], color, self.width(if own { 1.5 } else { 1.0 })));
                    y += 64;
                }
            }
        }
        self.borders.want(wanted);
    }

    /// Outlines slime chunks around you at your feet, a few chunks computed per frame.
    /// Returns true when it did heavy work.
    fn update_slime(&mut self) -> bool {
        if self.shapes.is_none() {
            return false;
        }
        let on = self.settings.slime_chunks && self.dimension == 0;
        let chunk = [self.feet[0].div_euclid(16), self.feet[2].div_euclid(16)];
        if on {
            if self.slime_known.len() != SLIME_SIDE * SLIME_SIDE || chunk != self.slime_center {
                // Keep the answers that are still in view.
                let old = std::mem::replace(&mut self.slime_known, vec![None; SLIME_SIDE * SLIME_SIDE]);
                let old_center = self.slime_center;
                if old.len() == SLIME_SIDE * SLIME_SIDE {
                    for (index, value) in old.into_iter().enumerate() {
                        let x = old_center[0] + (index / SLIME_SIDE) as i32 - SLIME_REACH;
                        let z = old_center[1] + (index % SLIME_SIDE) as i32 - SLIME_REACH;
                        let (dx, dz) = (x - chunk[0] + SLIME_REACH, z - chunk[1] + SLIME_REACH);
                        if (0..SLIME_SIDE as i32).contains(&dx) && (0..SLIME_SIDE as i32).contains(&dz) {
                            self.slime_known[dx as usize * SLIME_SIDE + dz as usize] = value;
                        }
                    }
                }
                self.slime_center = chunk;
            }
            let mut computed = 0;
            for index in 0..self.slime_known.len() {
                if self.slime_known[index].is_some() {
                    continue;
                }
                if computed == SLIME_PER_FRAME {
                    break;
                }
                let x = chunk[0] + (index / SLIME_SIDE) as i32 - SLIME_REACH;
                let z = chunk[1] + (index % SLIME_SIDE) as i32 - SLIME_REACH;
                self.slime_known[index] = Some(slime::is_slime_chunk(x, z));
                computed += 1;
            }
            if computed > 0 {
                return true;
            }
        }
        // Follow your height loosely, so jumping does not redraw everything.
        let y = match self.slime_key {
            Some((_, _, drawn, _)) if (drawn - self.feet[1]).abs() < 3 => drawn,
            _ => self.feet[1],
        };
        let key = (if on { 1 } else { 0 }, chunk, y, (self.settings.line_width * 100.0) as u32);
        if self.slime_key == Some(key) {
            return false;
        }
        self.slime_key = Some(key);
        if !on {
            self.slime.clear();
            return false;
        }
        let mut wanted = Vec::new();
        let height = f64::from(y) + 0.05;
        for (index, value) in self.slime_known.iter().enumerate() {
            if *value == Some(true) {
                let x = chunk[0] + (index / SLIME_SIDE) as i32 - SLIME_REACH;
                let z = chunk[1] + (index % SLIME_SIDE) as i32 - SLIME_REACH;
                let origin = [f64::from(x * 16), height, f64::from(z * 16)];
                wanted.push(instance(Shape::Ring, self.dimension, origin, GREEN, self.width(1.5)));
            }
        }
        self.slime.want(wanted);
        false
    }

    // ---- Terrain ------------------------------------------------------------------------

    fn monument_wanted(&self) -> bool {
        self.settings.wants_monument() && self.dimension == 0 && (self.feet[1] - 40).abs() < 110
    }

    /// Reads the monument's terrain a piece at a time, for the guardian columns.
    /// Returns true when it did heavy work.
    #[inline(never)]
    fn update_terrain(&mut self) -> bool {
        if !self.monument_wanted() {
            self.in_flight.clear();
            self.pending = None;
            return false;
        }
        // The arena only ever holds the one region being received or decoded.
        if self.pending.is_none() {
            crate::arena::reset();
        }
        // A region with block types not seen before is classified over several frames first:
        // each new type costs a few thousand fuel of string lookups.
        if let Some(pending) = self.pending.take() {
            if self.classify_some(&pending.region) {
                self.pending = Some(pending);
            } else {
                self.decode(pending.job, &pending.region);
            }
            return true;
        }

        let mut received = false;
        let jobs = std::mem::take(&mut self.in_flight);
        for job in jobs {
            if received {
                // Keep the subscription alive without paying for a second region this frame:
                // a region only arrives as a whole, so the read would lower it anyway.
                self.in_flight.push(job);
                continue;
            }
            match crate::arena::receiving(|| terrain::read_region(job.query(), None)) {
                Ok(terrain::ReadResult::Changed(region)) => {
                    self.accept(job, region);
                    received = true;
                }
                Ok(terrain::ReadResult::Pending) => self.in_flight.push(job),
                Ok(terrain::ReadResult::Unchanged(_)) | Err(_) => {}
            }
        }

        // One new query a frame, so at most about one region arrives per frame; never in a
        // frame that already received one.
        if !received && self.in_flight.len() < IN_FLIGHT {
            if let Some(job) = self.next_job() {
                match crate::arena::receiving(|| terrain::read_region(job.query(), None)) {
                    Ok(terrain::ReadResult::Pending) => self.in_flight.push(job),
                    Ok(terrain::ReadResult::Changed(region)) => {
                        self.accept(job, region);
                        received = true;
                    }
                    Ok(_) | Err(_) => {}
                }
            }
        }
        received
    }

    /// Decodes a region now when every block type in it is known, or keeps it for later.
    fn accept(&mut self, job: Job, region: terrain::Region) {
        // Receiving a region already costs a lot (the host copies every block type and its tags
        // into the mod), so it is decoded in later frames.
        self.pending = Some(Pending { job, region });
    }

    /// Classifies up to a few unknown block types of `region`; true while some remain.
    fn classify_some(&mut self, region: &terrain::Region) -> bool {
        if region.stamp.registry != self.registry {
            self.registry = region.stamp.registry;
            self.palette.clear();
        }
        let mut done = 0;
        for state in &region.palette {
            if self.palette.contains_key(&state.runtime_id) {
                continue;
            }
            if done == CLASSIFY_PER_FRAME {
                return true;
            }
            let info = blocks::classify(state, blocks::fixed_boxes(region, state));
            self.palette.insert(state.runtime_id, info);
            done += 1;
        }
        done > 0
    }

    fn next_job(&mut self) -> Option<Job> {
        let job = self.next_monument_job()?;
        match job {
            Job::Base { .. } => self.monument.next_tile += 1,
            Job::Column { .. } => {
                self.monument.next_column += 1;
                if self.monument.next_column == 25 {
                    self.monument.checked_at = self.clock;
                }
            }
        }
        (!self.in_flight.contains(&job)).then_some(job)
    }

    fn next_monument_job(&mut self) -> Option<Job> {
        let feet = [self.feet[0], self.feet[2]];
        let m = &mut self.monument;
        if let Some(corner) = m.corner {
            let middle = [corner[0] + monument::SIDE / 2, corner[1] + monument::SIDE / 2];
            if (middle[0] - feet[0]).abs() > 120 || (middle[1] - feet[1]).abs() > 120 {
                *m = MonumentState::default();
                return None;
            }
            if m.next_column >= 25 {
                if self.clock - m.checked_at < COLUMN_RECHECK {
                    return None;
                }
                m.next_column = 0;
            }
            return Some(Job::Column { index: m.next_column, corner });
        }
        // Start or move the search when you have walked away from where it looked.
        let center = m.center.unwrap_or(feet);
        let moved = (center[0] - feet[0]).abs() > 32 || (center[1] - feet[1]).abs() > 32;
        if m.center.is_none() || (moved && (m.none || !m.bounds.any)) {
            *m = MonumentState { center: Some(feet), tiles: monument::tiles(feet, monument::SEARCH_RADIUS), ..Default::default() };
        }
        if m.next_tile < m.tiles.len() {
            return Some(Job::Base { tile: m.tiles[m.next_tile] });
        }
        if m.none || m.next_tile > m.tiles.len() {
            return None;
        }
        // Every tile is in: did the blocks make a whole monument?
        m.next_tile += 1;
        if let Some(corner) = m.bounds.corner() {
            m.corner = Some(corner);
            m.next_column = 0;
        } else if m.bounds.any && m.attempts < 3 {
            // Part of one is in view: search again around it.
            let around = m.bounds.center();
            let attempts = m.attempts + 1;
            let bounds = m.bounds;
            *m = MonumentState {
                center: Some(feet),
                tiles: monument::tiles(around, monument::SIDE / 2 + 8),
                bounds,
                attempts,
                ..Default::default()
            };
        } else {
            m.none = true;
        }
        None
    }

    /// The reduced block info of every palette entry, from the cache where possible.
    fn palette(&mut self, region: &terrain::Region) -> Vec<BlockInfo> {
        if region.stamp.registry != self.registry {
            self.registry = region.stamp.registry;
            self.palette.clear();
        }
        region
            .palette
            .iter()
            .map(|state| {
                *self
                    .palette
                    .entry(state.runtime_id)
                    .or_insert_with(|| blocks::classify(state, blocks::fixed_boxes(region, state)))
            })
            .collect()
    }

    fn decode(&mut self, job: Job, region: &terrain::Region) {
        let palette = self.palette(region);
        let cells = region.states.len() / usize::from(region.layer_count.max(1));
        let loaded = |index: usize| region.loaded.get(index / 64).is_some_and(|word| word & (1u64 << (index % 64)) != 0);
        let cell = |index: usize| -> u8 {
            if !loaded(index) {
                return 0;
            }
            let base = match region.states[index] {
                EMPTY_SLOT => CLEAR,
                state => palette.get(usize::from(state)).map_or(15, |info| info.cell),
            } | KNOWN;
            if region.layer_count > 1 {
                if let Some(&extra) = region.states.get(cells + index) {
                    if extra != EMPTY_SLOT {
                        return blocks::combine(base, palette.get(usize::from(extra)).map_or(15, |info| info.cell));
                    }
                }
            }
            base
        };
        match job {
            Job::Base { tile } => {
                if !palette.iter().any(|info| info.monument) {
                    return;
                }
                // One flag per palette index; index 255 doubles as the empty slot.
                let mut monument = [false; 256];
                for (index, info) in palette.iter().enumerate().take(255) {
                    monument[index] = info.monument;
                }
                let size_y = (monument::BASE_HIGH - monument::BASE_LOW) as usize;
                let states = &region.states[..cells.min(region.states.len())];
                for (column, slice) in states.chunks(size_y).enumerate() {
                    if slice.iter().any(|state| monument[usize::from(*state as u8)] && *state != EMPTY_SLOT) {
                        let x = tile[0] + (column / monument::TILE as usize) as i32;
                        let z = tile[1] + (column % monument::TILE as usize) as i32;
                        self.monument.bounds.add(x, z);
                    }
                }
            }
            Job::Column { index, corner } => {
                if Some(corner) != self.monument.corner || cells < 3 {
                    return;
                }
                let solid = |y: usize| {
                    let value = cell(y);
                    value & KNOWN != 0 && value & (CLEAR | LIQUID) == 0
                };
                self.monument.blocked[index] = Some(monument::blocked(solid(2), solid(0)));
            }
        }
    }

    fn update_guardians(&mut self) {
        if self.shapes.is_none() {
            return;
        }
        let corner = if self.settings.guardians && self.monument_wanted() { self.monument.corner } else { None };
        let key = corner.map(|c| (c, self.monument.blocked, (self.settings.line_width * 100.0) as u32));
        if self.guardian_key == key {
            return;
        }
        self.guardian_key = key;
        let Some(corner) = corner else {
            self.guardians.clear();
            return;
        };
        let mut wanted = Vec::with_capacity(26);
        let dim = self.dimension;
        for (index, [x, z]) in monument::columns(corner).iter().enumerate() {
            let color = if self.monument.blocked[index] == Some(true) { RED } else { CYAN };
            let origin = [f64::from(*x) + 0.5, f64::from(monument::BOTTOM), f64::from(*z) + 0.5];
            wanted.push(instance(Shape::Column, dim, origin, color, self.width(1.5)));
        }
        let half = f64::from(monument::SIDE) / 2.0;
        let origin = [f64::from(corner[0]) + half, f64::from(monument::TOP + 1) + 0.05, f64::from(corner[1]) + half];
        wanted.push(instance(Shape::Footprint, dim, origin, FOOTPRINT, self.width(1.0)));
        self.guardians.want(wanted);
    }

    /// Drawings still waiting for host calls.
    fn drawing_backlog(&self) -> bool {
        [&self.borders, &self.guardians, &self.slime, &self.lights, &self.spawners].iter().any(|pool| pool.dirty)
    }

    /// Brings the retained drawings a few host calls closer to what is wanted.
    #[inline(never)]
    fn sync_drawings(&mut self, calls: usize) {
        let Some(shapes) = &self.shapes else {
            return;
        };
        let mut calls = calls;
        let mut error = None;
        for pool in [&mut self.borders, &mut self.guardians, &mut self.slime, &mut self.lights, &mut self.spawners] {
            pool.sync(shapes, &mut calls, &mut error);
        }
        if error.is_some() {
            self.draw_error = error;
        }
        self.update_hitboxes(calls + HIT_CALLS);
        self.update_own_hitbox();
        self.update_block_outline();

    }

    /// Everything the HUD shows now.
    fn views(&self) -> Views {
        let chunk = [self.feet[0].div_euclid(16), self.feet[2].div_euclid(16)];
        // The food in your hand, for Saturation's previews: they flash only while there is some.
        let eating = if hud::saturation_parts(&self.settings).eating() { saturation::food_value(&self.armor[4].item) } else { None };
        Views {
            chroma: if self.settings.gameplay.any_chroma() { (self.clock * 30.0) as u32 } else { 0 },
            held: self.held,
            armor: if self.settings.is_on(Element::Armor) { self.armor.clone() } else { Default::default() },
            item_count: if self.settings.is_on(Element::ItemCounter) { self.item_count } else { 0 },
            potion_count: if self.settings.is_on(Element::PotionCounter) { self.potion_count } else { 0 },
            totems: if self.settings.is_on(Element::TotemCounter) { self.totems } else { 0 },
            pvp: if self.settings.is_on(Element::PvpInfo) { [self.combat.hits, self.combat.accuracy(), self.combat.kills, self.combat.deaths] } else { [0; 4] },
            sidebar: if self.settings.is_on(Element::Scoreboard) { self.sidebar.clone() } else { None },
            target: if self.settings.is_on(Element::TargetHud) { self.target.clone() } else { None },
            target_hidden: self.settings.is_on(Element::TargetHud) && self.target.is_none() && !self.panel.moving,
            waila: if self.settings.is_on(Element::Waila) { self.waila_shown() } else { None },
            height_limit: if self.settings.is_on(Element::HeightLimit) {
                let (_, top) = world_height(self.dimension);
                Some(top - self.feet_exact[1].floor() as i32)
            } else {
                None
            },
            subtitles: if self.settings.is_on(Element::AudioSubtitles) { self.sub_rows.clone() } else { Vec::new() },
            subtitles_hidden: self.settings.is_on(Element::AudioSubtitles) && self.sub_rows.is_empty() && !self.panel.moving,
            waila_hidden: self.settings.is_on(Element::Waila) && self.waila.is_none() && self.waila_mob.is_none() && !self.panel.moving,
            sidebar_hidden: self.settings.is_on(Element::Scoreboard) && (self.sidebar.is_none() || self.settings.hide_scoreboard) && !self.panel.moving,
            clock: if self.settings.is_on(Element::Clock) { self.clock_time } else { None },
            va: if self.settings.is_on(Element::ViewAngles) { self.va_view() } else { None },
            va_hidden: self.settings.is_on(Element::ViewAngles) && !self.panel.moving && self.va_out_of_sight(),
            inventory: if self.settings.is_on(Element::Inventory) { self.inventory.clone() } else { Vec::new() },
            cps: self.cps,
            waypoints: self.wp_rows.clone(),
            cps_stats: [self.cps_stats.average_tenths(), self.cps_stats.max(), self.cps_stats.total()],
            fps: self.fps,
            ping: if self.settings.is_on(Element::Ping) { self.ping } else { None },
            reach: if self.settings.is_on(Element::Reach) { Some(self.combat.reach_hundredths()) } else { None },
            reach_hidden: self.settings.is_on(Element::Reach)
                && self.settings.reach_hide_zero
                && self.combat.reach_hundredths() == 0
                // While the elements are moved the box stays, so it can be found and dragged.
                && !self.panel.moving,
            combo: if self.settings.is_on(Element::Combo) { self.combat.combo() } else { 0 },
            food: if self.settings.is_on(Element::Food) { self.food } else { None },
            eating,
            pulse: if eating.is_some() { saturation::pulse(self.clock) } else { 0 },
            day: if self.settings.is_on(Element::Day) { self.day } else { None },
            potions: self.potion_rows(),
            blink: (self.clock * 3.0) as u64 % 2 == 0,
            speed: self
                .speed
                .blocks_per_second()
                .map(|v| timers::speed_display(v, self.settings.speed_unit == 1, self.settings.speed_decimals)),
            stopwatch: self.stopwatch.seconds(),
            playtime: self.playtime.seconds(self.settings.playtime_world_only),
            crosshair: (self.settings.crosshair.enabled && (self.settings.crosshair.third_person || !self.camera_is_away()))
                .then(|| self.settings.crosshair.frame(self.xh_target, self.holding_ranged)),
            feet: self.feet,
            feet_exact: if self.settings.is_on(Element::Coords) && self.settings.co_decimal { self.feet_exact.map(|v| (v * 100.0).round() / 100.0) } else { [0.0; 3] },
            facing: self.facing,
            yaw: self.yaw,
            overworld: self.dimension == 0,
            slime: if self.dimension != 0 {
                Some(false)
            } else {
                self.hud_slime.filter(|(known, _)| *known == chunk).map(|(_, slime)| slime)
            },
            guardians: self.settings.wants_monument().then(|| {
                if !self.monument_wanted() {
                    Guardians::Unavailable
                } else if self.monument.corner.is_some() {
                    let checked = self.monument.blocked.iter().all(Option::is_some);
                    let blocked = self.monument.blocked.iter().filter(|b| **b == Some(true)).count() as u8;
                    Guardians::Found { active: 25 - blocked, blocked, checked }
                } else if self.monument.none {
                    Guardians::Unavailable
                } else {
                    Guardians::Searching
                }
            }),
        }
    }

    /// The slime chunk answer for the chunk you stand in (one answer per chunk, a few thousand
    /// fuel).
    fn update_hud_slime(&mut self) {
        if !self.settings.is_on(Element::Slime) || self.dimension != 0 {
            return;
        }
        let chunk = [self.feet[0].div_euclid(16), self.feet[2].div_euclid(16)];
        if self.hud_slime.map(|(known, _)| known) != Some(chunk) {
            self.hud_slime = Some((chunk, slime::is_slime_chunk(chunk[0], chunk[1])));
        }
    }

    /// Keeps a box on every nearby actor. Boxes are anchored to their actors, so they move
    /// with them every frame on their own; only who is near and box sizes are reread, every
    /// few frames.
    #[inline(never)]
    fn update_hitboxes(&mut self, mut calls: usize) {
        if !self.settings.hitboxes || self.shapes.is_none() {
            self.clear_hitboxes(&mut calls);
            return;
        }
        // Drawings keep the colour they were made with, so a colour change removes them (a few a
        // frame) and they are made again below.
        let colors = (self.settings.hitbox_color, self.settings.hitbox_player_color);
        if colors != self.hit_colors {
            self.hit_colors = colors;
            self.hits_recolor = true;
        }
        if self.hits_recolor {
            if self.hits.is_empty() {
                self.hits_recolor = false;
            } else {
                self.clear_hitboxes(&mut calls);
                return;
            }
        }
        if self.frames % HIT_REFRESH == 0 {
            if let Ok(actors) = crate::hive_host::counted(|| player_state::read_actors_limited(mod_api::MAX_READ_ACTORS as u32)) {
                self.hit_sizes.clear();
                for observed in actors {
                    let actor = &observed.actor;
                    let (x, y, z) = actor.position;
                    let bounds: Vec<([f32; 3], [f32; 3])> =
                        actor.boxes.iter().map(|b| ([b.min.0, b.min.1, b.min.2], [b.max.0, b.max.1, b.max.2])).collect();
                    if let Some(boxed) = hitbox::boxes([x, y, z], &bounds).first() {
                        let feet = [x, y, z].map(f64::from);
                        let offset = [0, 1, 2].map(|i| ((boxed.min[i] - feet[i]) as f32 * 32.0).round() / 32.0);
                        let size = boxed.size.map(|v| (v * 32.0).round() / 32.0);
                        self.hit_sizes.insert(actor.runtime_id, (offset, size, actor.player));
                    }
                }
            }
            match draw::observe_entities() {
                Ok(entities) => {
                    for hit in &mut self.hits {
                        hit.seen = false;
                    }
                    for observed in entities {
                        let shape = self.hit_sizes.get(&observed.runtime_id).copied().unwrap_or(DEFAULT_BOX);
                        match self.hits.iter_mut().find(|h| h.runtime_id == observed.runtime_id) {
                            Some(hit) => {
                                hit.seen = true;
                                hit.entity = observed.entity;
                                if hit.shape != shape {
                                    hit.shape = shape;
                                    // Redrawn below with the new size.
                                    if let Some(cube) = hit.cube.take() {
                                        let _ = draw::remove(cube);
                                    }
                                }
                            }
                            None => self.hits.push(Hit {
                                runtime_id: observed.runtime_id,
                                entity: observed.entity,
                                shape,
                                cube: None,
                                ray: None,
                                seen: true,
                            }),
                        }
                    }
                }
                Err(error) => self.draw_error = Some(error.detail),
            }
        }
        let Some(shapes) = &self.shapes else { return };
        // Actors no longer offered: their drawings and reference go.
        let mut index = 0;
        while index < self.hits.len() && calls > 0 {
            if self.hits[index].seen {
                index += 1;
                continue;
            }
            let hit = &mut self.hits[index];
            for handle in [hit.cube.take(), hit.ray.take()].into_iter().flatten() {
                calls = calls.saturating_sub(1);
                let _ = draw::remove(handle);
            }
            calls = calls.saturating_sub(1);
            let _ = draw::remove(hit.entity);
            self.hits.swap_remove(index);
        }
        // New boxes and look lines.
        let mut error = None;
        for hit in &mut self.hits {
            if calls == 0 {
                break;
            }
            let (offset, size, player) = hit.shape;
            let tint = crate::config::rgba(if player { self.settings.hitbox_player_color } else { self.settings.hitbox_color }, 1.0);
            if hit.cube.is_none() {
                calls -= 1;
                let transform = draw::Transform {
                    translation: (offset[0], offset[1], offset[2]),
                    rotation: (0.0, 0.0, 0.0, 1.0),
                    scale: (size[0], size[1], size[2]),
                };
                match draw::create(&entity_instance(shapes.cube, hit.entity, false, transform, tint)) {
                    Ok(handle) => hit.cube = Some(handle),
                    Err(failure) => error = Some(failure.detail),
                }
            }
            if hit.ray.is_none() && calls > 0 {
                calls -= 1;
                // Along the way the body faces, from about the eyes.
                let transform = draw::Transform {
                    translation: (0.0, offset[1] + size[1] * 0.85, 0.0),
                    rotation: (0.0, 0.0, 0.0, 1.0),
                    scale: (1.0, 1.0, hitbox::LOOK_LENGTH),
                };
                match draw::create(&entity_instance(shapes.ray, hit.entity, true, transform, [0.3, 0.45, 1.0, 1.0])) {
                    Ok(handle) => hit.ray = Some(handle),
                    Err(failure) => error = Some(failure.detail),
                }
            }
        }
        if error.is_some() {
            self.draw_error = error;
        }
    }

    /// Spawners near you in the Nether (blaze spawners in fortresses): the block, the 9 × 3 × 9
    /// box blazes appear in, and the 16-block circle you must stand in for it to work.
    fn update_spawners(&mut self) {
        if self.shapes.is_none() {
            return;
        }
        let on = self.settings.blaze && self.dimension == 1;
        let width = (self.settings.line_width * 100.0) as u32;
        let mut found = Vec::new();
        if on {
            // A search is a subscription: results fill in while it is asked for each frame.
            if let Ok(positions) = terrain::find_blocks(
                &[crate::spawners::BLOCK.to_owned()],
                crate::spawners::SEARCH_RANGE,
                crate::spawners::MAX_SHOWN as u32,
            ) {
                found = positions.iter().map(|p| [p.x, p.y, p.z]).collect();
                found.sort_unstable();
            } else if let Some((true, shown, _)) = &self.spawners_shown {
                found = shown.clone();
            }
        }
        // Spawners sit inside fortress walls: drawn through blocks when allowed.
        let through = on && through_walls();
        let key = (on, found, width + u32::from(through));
        if self.spawners_shown.as_ref() == Some(&key) {
            return;
        }
        let wanted = key
            .1
            .iter()
            .flat_map(|&[x, y, z]| {
                let (x, y, z) = (f64::from(x), f64::from(y), f64::from(z));
                [
                    Want { through, ..instance(Shape::Cube, 1, [x, y, z], YELLOW, self.width(2.0)) },
                    Want { through, ..instance(Shape::SpawnArea, 1, [x - 4.0, y - 1.0, z - 4.0], RED, self.width(1.0)) },
                    Want { through, ..instance(Shape::Activation, 1, [x + 0.5, y + 0.5, z + 0.5], ORANGE, self.width(1.5)) },
                ]
            })
            .collect();
        self.spawners_shown = Some(key);
        if on { self.spawners.want(wanted) } else { self.spawners.clear() }
    }

    /// Where to put lights: a diagonal grid on the ground, 5 and 5 blocks apart, around you.
    fn update_lights(&mut self) {
        if self.shapes.is_none() {
            return;
        }
        let on = self.settings.lights;
        let chunk = [self.feet[0].div_euclid(16), self.feet[2].div_euclid(16)];
        // Follow your height loosely, so jumping does not redraw everything.
        let y = match self.lights_key {
            Some((true, _, drawn, _)) if (drawn - self.feet[1]).abs() < 3 => drawn,
            _ => self.feet[1],
        };
        let key = (on, chunk, y, (self.settings.line_width * 100.0) as u32);
        if self.lights_key == Some(key) {
            return;
        }
        self.lights_key = Some(key);
        if !on {
            self.lights.clear();
            return;
        }
        let spots = crate::lights::spots([self.feet[0], self.feet[2]], LIGHT_REACH);
        let wanted = spots
            .into_iter()
            .map(|[x, z]| instance(Shape::Cube, self.dimension, [f64::from(x), f64::from(y), f64::from(z)], ORANGE, self.width(1.0)))
            .collect();
        self.lights.want(wanted);
    }

    /// What View angles shows, each number times ten to the decimals (see `hud::VaView`).
    fn va_view(&self) -> Option<hud::VaView> {
        let [yaw, pitch] = self.va_angles?;
        let scale = 10f64.powi(self.settings.va_decimals.min(crate::config::VA_MAX_DECIMALS) as i32);
        let at = |v: f64| (v * scale).round().clamp(-2.0e9, 2.0e9) as i32;
        Some(hud::VaView {
            yaw: at(yaw),
            pitch: at(pitch),
            speed: at(self.va_meter.blocks_per_second().unwrap_or(0.0)),
            altitude: at(self.feet_exact[1]),
        })
    }

    /// Whether View angles' options keep it out of sight now: the camera it is not asked for, or no elytra flight.
    fn va_out_of_sight(&self) -> bool {
        let s = &self.settings;
        if !(s.va_first && s.va_third) && self.camera_is_away() != s.va_third {
            return true;
        }
        if s.va_elytra {
            // The host reports no elytra state: an elytra in the chest slot while moving faster than anyone runs.
            let worn = self.armor[1].item.contains("elytra");
            let flying = self.va_meter.blocks_per_second().is_some_and(|v| v > 8.0);
            return !(worn && flying);
        }
        false
    }

    /// The camera is away from the eyes: a third-person view (F5).
    fn camera_is_away(&self) -> bool {
        matches!(gameplay::read_camera(), Ok(Some(camera)) if {
            let eye = [self.feet_exact[0], self.feet_exact[1] + 1.62, self.feet_exact[2]];
            let (cx, cy, cz) = (camera.position.x, camera.position.y, camera.position.z);
            let d = [f64::from(cx) - eye[0], f64::from(cy) - eye[1], f64::from(cz) - eye[2]];
            d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > 0.5
        })
    }

    /// Your own hitbox and look line, like F3+B, only in third person (in first person the
    /// box would sit around the camera).
    fn update_own_hitbox(&mut self) {
        let Some(shapes) = &self.shapes else { return };
        let third_person = self.settings.hitboxes
            && matches!(gameplay::read_camera(), Ok(Some(camera)) if {
                let eye = [self.feet_exact[0], self.feet_exact[1] + 1.62, self.feet_exact[2]];
                let (cx, cy, cz) = (camera.position.x, camera.position.y, camera.position.z);
                let d = [f64::from(cx) - eye[0], f64::from(cy) - eye[1], f64::from(cz) - eye[2]];
                d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > 0.5
            });
        if !third_person {
            for handle in [self.own_box.take(), self.own_ray.take()].into_iter().flatten() {
                let _ = draw::remove(handle);
            }
            return;
        }
        // Actor YXZ degrees: forward -Z, yaw left, pitch up.
        let (yaw, pitch) = match crate::hive_host::counted(|| player_state::read_motion()) {
            Ok(Some(motion)) if motion.yaw.is_finite() && motion.pitch.is_finite() => {
                (f64::from(motion.yaw), f64::from(motion.pitch))
            }
            _ => (0.0, 0.0),
        };
        let [x, y, z] = self.feet_exact;
        let anchor = draw::Anchor::World(draw::WorldAnchor { dimension: self.dimension, origin: (x, y, z) });
        let place = |geometry, transform, tint: [f32; 4]| {
            let [r, g, b, a] = tint;
            draw::Instance {
                geometry,
                material: None,
                params: Vec::new(),
                anchor,
                transform,
                tint: (r * a, g * a, b * a, a),
                depth: draw::Depth::Test,
                blend: draw::Blend::Premultiplied,
                width: 2.0,
                width_mode: draw::WidthMode::Pixels,
                join: draw::Join::Miter,
                cap: draw::Cap::Butt,
                clip: None,
            }
        };
        let cube = place(
            shapes.cube,
            draw::Transform { translation: (-0.3, 0.0, -0.3), rotation: (0.0, 0.0, 0.0, 1.0), scale: (0.6, 1.8, 0.6) },
            crate::config::rgba(self.settings.hitbox_player_color, 1.0),
        );
        let rotation = hitbox::look_rotation(yaw, pitch);
        let ray = place(
            shapes.ray,
            draw::Transform { translation: (0.0, 1.62, 0.0), rotation, scale: (1.0, 1.0, hitbox::LOOK_LENGTH) },
            [0.3, 0.45, 1.0, 1.0],
        );
        for (slot, spec) in [(&mut self.own_box, cube), (&mut self.own_ray, ray)] {
            match slot {
                Some(handle) => {
                    let _ = draw::update(*handle, &spec);
                }
                None => {
                    if let Ok(handle) = draw::create(&spec) {
                        *slot = Some(handle);
                    }
                }
            }
        }
    }

    /// Forgets the Block outline's handles and cached shape (the host drops them with the dimension).
    fn bo_forget(&mut self) {
        self.bo_edges.clear();
        self.bo_faces.clear();
        self.bo_block = None;
        self.bo_boxes.clear();
        self.bo_segments.clear();
        self.bo_shape_known = false;
        self.bo_stamp = None;
        self.bo_drawn = false;
    }

    /// Takes the outline off the screen once.
    fn bo_clear(&mut self) {
        if self.bo_drawn {
            let _ = draw::replace_frame_group(BLOCK_OUTLINE_GROUP, &[]);
            self.bo_drawn = false;
        }
        self.bo_block = None;
    }

    /// Uploads the three one-edge lines and the six face quads (the shapes are stretched to each block's boxes).
    fn bo_upload(&mut self) {
        let mut edges = Vec::new();
        for axis in 0..3 {
            let mut end = [0.0f32; 3];
            end[axis] = 1.0;
            match line(vec![[0.0; 3], end], false) {
                Ok(handle) => edges.push(handle),
                Err(_) => {
                    for handle in edges {
                        let _ = draw::remove(handle);
                    }
                    return;
                }
            }
        }
        let mut faces = Vec::new();
        for face in outline::FACES {
            // Both windings, so the quad shows from either side.
            match triangles((face.unit_corners().to_vec(), vec![0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2])) {
                Ok(handle) => faces.push(handle),
                Err(_) => {
                    for handle in edges.into_iter().chain(faces) {
                        let _ = draw::remove(handle);
                    }
                    return;
                }
            }
        }
        self.bo_edges = edges;
        self.bo_faces = faces;
    }

    /// Reads the selection boxes of `block` (stairs, slabs, fences have their real shape); until they arrive, or when
    /// there are none, the block is a full cube. The host keeps a terrain read only while it is repeated every frame, so
    /// this is called every frame while the block is aimed at (an unchanged answer costs nothing).
    fn bo_read_shape(&mut self, block: [i32; 3]) {
        let query = terrain::RegionQuery {
            min: terrain::Position { x: block[0], y: block[1], z: block[2] },
            max: terrain::Position { x: block[0] + 1, y: block[1] + 1, z: block[2] + 1 },
            geometry: terrain::Geometry::SELECTION,
        };
        let after = self.bo_stamp.clone();
        if let Ok(terrain::ReadResult::Changed(region)) = crate::arena::receiving(|| terrain::read_region(query, after)) {
            self.bo_stamp = Some(region.stamp.clone());
            if let Some(boxes) = selection_boxes(&region) {
                self.bo_boxes = lift_outline(outline::inflate(&boxes, BLOCK_OUTLINE_GROW));
                self.bo_segments = outline::shape_edges(&self.bo_boxes);
                self.bo_shape_known = true;
            }
        }
    }

    /// Block outline: the outline of the block the crosshair is on along its real shape, with an optional fill and the
    /// game-highlight fix, all drawn as one frame group.
    fn update_block_outline(&mut self) {
        let target = if self.settings.gameplay.bo_on && self.in_gameplay {
            match gameplay::read_block_target() {
                Ok(Some(p)) if p.x.is_finite() && p.y.is_finite() && p.z.is_finite() => {
                    Some([f64::from(p.x).floor() as i32, f64::from(p.y).floor() as i32, f64::from(p.z).floor() as i32])
                }
                _ => None,
            }
        } else {
            None
        };
        let Some(block) = target else {
            self.bo_clear();
            return;
        };
        if self.bo_block != Some(block) {
            self.bo_block = Some(block);
            self.bo_boxes = lift_outline(outline::inflate(&[[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]], BLOCK_OUTLINE_GROW));
            self.bo_segments = outline::shape_edges(&self.bo_boxes);
            self.bo_shape_known = false;
            self.bo_stamp = None;
        }
        self.bo_read_shape(block);
        if self.bo_edges.len() != 3 || self.bo_faces.len() != 6 {
            if self.clock >= self.bo_upload_at {
                self.bo_upload_at = self.clock + 0.25;
                self.bo_upload();
            }
            if self.bo_edges.len() != 3 || self.bo_faces.len() != 6 {
                return;
            }
        }
        let Ok(Some(camera)) = gameplay::read_camera() else { return };
        let eye = [f64::from(camera.position.x), f64::from(camera.position.y), f64::from(camera.position.z)];
        let g = &self.settings.gameplay;
        let base = crate::config::rgba(g.bo_color.min(7), 1.0);
        let opacity = outline::OPACITIES[g.bo_opacity.min(5)] / 100.0;
        let [r, gr, b, a] = outline::tint(g.bo_mode, [base[0], base[1], base[2]], opacity, self.clock);
        let fill = f32::from(outline::FILL_PERCENTS[g.bo_fill.min(5)]) / 100.0;
        // The client drops its own outline while this one is on, so nothing needs cancelling.
        let fix = 0.0_f32;
        // The width is what the slider says, in pixels at any distance.
        let width = if g.bo_line.is_finite() { g.bo_line.clamp(1.0, 8.0) as f32 } else { 2.0 };
        let dimension = self.dimension;
        let anchor = move |origin: [f64; 3]| draw::Anchor::World(draw::WorldAnchor { dimension, origin: (origin[0], origin[1], origin[2]) });
        let place = |geometry: draw::Handle, origin: [f64; 3], scale: (f32, f32, f32), tint: (f32, f32, f32, f32), width: f32| draw::Instance {
            geometry,
            material: None,
            params: Vec::new(),
            anchor: anchor([origin[0] + f64::from(block[0]), origin[1] + f64::from(block[1]), origin[2] + f64::from(block[2])]),
            transform: draw::Transform { translation: (0.0, 0.0, 0.0), rotation: (0.0, 0.0, 0.0, 1.0), scale },
            tint,
            depth: draw::Depth::Test,
            blend: draw::Blend::Premultiplied,
            width,
            width_mode: draw::WidthMode::Pixels,
            join: draw::Join::Round,
            cap: draw::Cap::Round,
            clip: None,
        };
        // The boxes are block-local; the camera is moved into the same frame for the visibility tests.
        let local = [eye[0] - f64::from(block[0]), eye[1] - f64::from(block[1]), eye[2] - f64::from(block[2])];
        let mut instances: Vec<draw::Instance> = Vec::with_capacity(32);
        let faces = |instances: &mut Vec<draw::Instance>, tint: (f32, f32, f32, f32)| {
            for bx in &self.bo_boxes {
                let (min, max) = ([bx[0], bx[1], bx[2]], [bx[3], bx[4], bx[5]]);
                let scale = ((max[0] - min[0]) as f32, (max[1] - min[1]) as f32, (max[2] - min[2]) as f32);
                for (quad, face) in self.bo_faces.iter().zip(outline::FACES) {
                    if face.visible(min, max, local) {
                        instances.push(place(*quad, min, scale, tint, 1.0));
                    }
                }
            }
        };
        if fix > 0.0 {
            faces(&mut instances, (0.0, 0.0, 0.0, fix));
        }
        if fill > 0.0 {
            faces(&mut instances, (base[0] * fill, base[1] * fill, base[2] * fill, fill));
        }
        for segment in &self.bo_segments {
            if !segment.visible(local) {
                continue;
            }
            let mut scale = [1.0f32; 3];
            scale[segment.axis] = segment.length as f32;
            instances.push(place(self.bo_edges[segment.axis], segment.start, (scale[0], scale[1], scale[2]), (r * a, gr * a, b * a, a), width));
        }
        if draw::replace_frame_group(BLOCK_OUTLINE_GROUP, &instances).is_ok() {
            self.bo_drawn = true;
        } else {
            self.bo_forget();
        }
    }

    /// Removes every hitbox drawing and actor reference, a few calls a frame.
    fn clear_hitboxes(&mut self, calls: &mut usize) {
        while let Some(hit) = self.hits.last_mut() {
            if *calls == 0 {
                return;
            }
            if let Some(handle) = hit.cube.take().or_else(|| hit.ray.take()) {
                *calls -= 1;
                let _ = draw::remove(handle);
                continue;
            }
            *calls -= 1;
            let _ = draw::remove(hit.entity);
            self.hits.pop();
        }
    }

    /// The widths to fit the text boxes to: wider at once, narrower once the text has stayed
    /// shorter for a moment (or at once when a setting changed).
    #[inline(never)]
    fn settle_widths(&mut self, wanted: [u16; Element::COUNT]) -> [u16; Element::COUNT] {
        if self.hud_widths_revision != self.settings_revision {
            self.hud_widths_revision = self.settings_revision;
            self.hud_shrink_since = None;
            self.hud_widths = Some(wanted);
            return wanted;
        }
        let mut next = self.hud_widths.unwrap_or(wanted);
        let mut shrink = false;
        for (slot, (current, want)) in next.iter_mut().zip(wanted).enumerate() {
            // Reach and Combo change only on a hit or when it times out, never flicker, so their
            // boxes follow the text at once instead of lagging behind it.
            if slot == Element::Reach.index() || slot == Element::Combo.index() {
                *current = want;
            } else if want > *current {
                *current = want;
            } else if want < *current {
                shrink = true;
            }
        }
        if shrink {
            let since = *self.hud_shrink_since.get_or_insert(self.clock);
            if self.clock - since >= WIDTH_SHRINK_DELAY {
                next = wanted;
                self.hud_shrink_since = None;
            }
        } else {
            self.hud_shrink_since = None;
        }
        self.hud_widths = Some(next);
        next
    }

    #[inline(never)]
    fn publish_hud(&mut self) {
        // A drawing problem (usually a missing permission) is said once in chat.
        if self.draw_error != self.draw_reported {
            if let Some(error) = &self.draw_error {
                let clean: String = error.chars().filter(|c| c.is_ascii_alphanumeric() || " .,:-_()".contains(*c)).take(80).collect();
                let _ = host_hud::local_message(&format!("Vantage drawing: {clean}"));
            }
            self.draw_reported = self.draw_error.clone();
        }
        if !hud::any_on(&self.settings) {
            if self.hud_shape.take().is_some() {
                let _ = host_hud::set_surface("");
            }
            self.shown_view = None;
            return;
        }
        // Placed elements follow the screen size.
        // The crosshair needs the screen size and GUI scale, but they change rarely: look every 15th frame.
        // So does the Saturation overlay, which sits on the game's hunger bar.
        let overlay = hud::saturation_parts(&self.settings).any();
        if hud::any_placed(&self.settings) || ((self.settings.crosshair.enabled || overlay) && (self.viewport.is_none() || self.frames % 15 == 0)) {
            let view = host_hud::read_viewport();
            let viewport = view.map(|v| [v.width, v.height]);
            if viewport != self.viewport {
                self.viewport = viewport;
            }
            // The Saturation overlay sits where the game drew its hunger shanks, when the host says.
            if overlay {
                let bar = host_hud::read_elements().ok().and_then(|all| {
                    all.into_iter()
                        .find(|state| state.element == host_hud::VanillaElement::Hunger)
                        .and_then(|state| state.bounds)
                        .and_then(|r| saturation::bar_origin([r.x, r.y, r.width, r.height]))
                });
                if hud::set_hunger_at(bar) {
                    self.hud_shape = None;
                }
            }
            // The crosshair's lines sit on device pixels: a new GUI scale redraws it.
            let scale = view.map_or(1.0, |v| v.scale);
            if (scale - crate::crosshair::pixel()).abs() > 0.001 {
                crate::crosshair::set_pixel(scale);
                self.hud_shape = None;
                self.doc_cache = Default::default();
            }
        }
        let view = self.views();
        // Nothing changed since the layer was last sent: skip measuring and comparing.
        if self.hud_shrink_since.is_none() && self.hud_widths_revision == self.settings_revision {
            if let (Some(shape), Some(shown)) = (self.hud_shape, self.shown_view.as_ref()) {
                if shape.0 == self.settings_revision && shape.1 == self.viewport && *shown == view {
                    return;
                }
            }
        }
        hud::set_measurer(Some(host_text_width));
        let widths = self.settle_widths(hud::measure(&view, &self.settings));
        hud::set_widths(widths);
        // Potion status's layout follows its effects and the settings: made again only when one of
        // them changed, and the document is then written again.
        let layout_key = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            self.settings_revision.hash(&mut hasher);
            for row in &view.potions {
                (row.id, row.level, row.texture.as_str(), row.seconds.map(|s| s >= 3600)).hash(&mut hasher);
            }
            hasher.finish()
        };
        if layout_key != self.potion_key {
            self.potion_key = layout_key;
            if hud::set_potion(&view.potions, &self.settings) {
                self.hud_shape = None;
            }
        }
        let shape = (self.settings_revision, self.viewport, widths);
        let result = if self.hud_shape != Some(shape) {
            // The menu was just sent values: write the new layer in the next frame instead, so
            // one frame does not pay for both (it still goes through every second frame at most).
            if self.menu_busy && self.hud_defers < 1 {
                self.hud_defers += 1;
                return;
            }
            self.hud_defers = 0;
            let result = host_hud::set_surface(&hud::surface_cached(&view, &self.settings, self.viewport, &mut self.doc_cache));
            if result.is_ok() {
                self.hud_shape = Some(shape);
            }
            result
        } else {
            match &self.shown_view {
                Some(previous) if *previous == view => return,
                Some(previous) => host_hud::update_surface(&hud::patch_at(&view, previous, &self.settings, self.viewport)),
                None => host_hud::set_surface(&hud::surface_cached(&view, &self.settings, self.viewport, &mut self.doc_cache)),
            }
        };
        match result {
            Ok(()) => self.shown_view = Some(view),
            Err(error) => {
                // Say why once, instead of failing silently every frame.
                if self.hud_error.as_deref() != Some(error.as_str()) {
                    let clean: String = error.chars().take(120).collect();
                    let _ = host_hud::local_message(&format!("Vantage HUD: {clean}"));
                    self.hud_error = Some(error);
                }
                self.shown_view = Some(view);
                self.hud_shape = None;
                // The game's effect icons are images: if this host refuses them, Potion status
                // goes on with text only instead of taking the whole HUD down with it.
                if self.settings.is_on(Element::Potion) && hud::disable_potion_icons() {
                    self.potion_key = 0;
                    self.doc_cache = Default::default();
                    let _ = host_hud::local_message("Vantage: this game cannot draw the effect icons, so Potion status shows text only.");
                }
            }
        }
    }
}

thread_local! {
    /// Widths the host reported, by text: a line is asked about once, not every frame. The host
    /// allows only a limited number of measurements per callback, so misses are bounded.
    static WIDTHS: std::cell::RefCell<std::collections::HashMap<String, f32>> = Default::default();
}

/// The host's width of `text` in its own font (cached), for fitting HUD boxes to their text.
/// The view tilt for the hurt camera options. Removing the shake suppresses it. Otherwise the game's own shake (read
/// at full strength) is scaled: added to when stronger, taken from when weaker (a tilt on top of the native shake),
/// or, with a fixed direction, replaced by a roll of the same size. None (the game's own view) when nothing changes.
fn hurt_tilt((off, percent, fixed): (bool, u32, bool)) -> Option<camera::ViewTilt> {
    if off {
        return Some(camera::ViewTilt { roll: 0.0, pitch: 0.0, suppress_hurt_shake: true });
    }
    if percent == 100 && !fixed {
        return None;
    }
    let shake = camera::read_damage_shake().ok().flatten()?;
    let factor = percent as f32 / 100.0;
    let limit = mod_api::MAX_VIEW_TILT_DEGREES;
    let (roll, pitch, suppress) = if fixed {
        ((shake.roll.hypot(shake.pitch) * factor).min(limit), 0.0, true)
    } else {
        (shake.roll * (factor - 1.0), shake.pitch * (factor - 1.0), false)
    };
    Some(camera::ViewTilt { roll: roll.clamp(-limit, limit), pitch: pitch.clamp(-limit, limit), suppress_hurt_shake: suppress })
}

fn host_text_width(text: &str) -> Option<f32> {
    if let Some(width) = WIDTHS.with_borrow(|cache| cache.get(text).copied()) {
        return Some(width);
    }
    let width = host_hud::measure_text(text, host_hud::Font::Native).ok()?;
    WIDTHS.with_borrow_mut(|cache| {
        if cache.len() > 512 {
            cache.clear();
        }
        cache.insert(text.to_owned(), width);
    });
    Some(width)
}

#[cfg(target_arch = "wasm32")]
mod_api::handled_player_mod::export_handled_player_mod!(Mod with_types_in mod_api::handled_player_mod);

/// Announces this mod to Mods Core, and sets and opens the menu when asked (or drops it when it
/// closes, so only one menu exists at a time; see `mods_core.rs`).
fn core_frame(panel_open: bool, whole_panel: impl FnOnce() -> String) -> crate::mods_core::Event {
    CORE.with_borrow_mut(|menu| menu.frame(panel_open, whole_panel))
}

/// The player's own menu key: opens the menu through Mods Core's hand-over, or closes it.
fn core_toggle(panel_open: bool) {
    CORE.with_borrow_mut(|menu| menu.toggle(panel_open));
}

/// What Mods Core's hand-over did, so it can be shown in chat.
fn core_take_notes() -> Vec<String> {
    CORE.with_borrow_mut(|menu| menu.take_notes())
}
