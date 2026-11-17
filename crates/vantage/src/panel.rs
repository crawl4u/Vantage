//! The Vantage menu: a sidebar of pages (`tools/gen_menu.py`). Cinnabar draws it; the
//! mod sends the values and reacts to events. Every option and every HUD element has its own
//! key, and the Layout and Style pages edit the HUD element picked in the strip at their top.

use crate::config::{
    ARMOR_SIDE, ARMOR_TEXT, CpsMode, Corner, Element, Grid, KEY_SLOTS, MAX_ARMOR_GAP, MAX_SCALE, MAX_BORDER, MAX_SPEED_DECIMALS, MAX_STRETCH, MAX_WIDTH, MIN_BORDER, MIN_SCALE, MIN_STRETCH, MIN_WIDTH,
    COLOR_CHOICES, COUNTER_ITEMS, COUNTER_ITEM_NAMES, CUSTOM_ROW, LATER_TOGGLES, MORE_TOGGLES, SPEED_DECIMALS, SPEED_UNITS, Settings, TIME_FORMATS, TOGGLES, color_rgb, custom_color, key_slot_id, key_slot_position, pick, valid_key,
};
use crate::menu::{self, Value};
use crate::menu_controls::{CONTROLS, DD_PAD, DD_ROW, DD_ROWS, DD_W, DROPDOWNS, HEIGHT, Kind, MAX_SCROLL, SHIFTS, VIEW_LEFT, VIEW_TOP, VIEW_W, WIDTH};
use crate::setups::Setups;
use crate::transition::PANEL_LABELS;
use crate::crosshair::SHAPES;
use crate::crosshair_config::{CrosshairSettings, MAX_GAP, MAX_SIZE, MAX_TARGET_SCALE, MAX_OUTLINE_WIDTH, MAX_THICKNESS, MIN_OUTLINE_WIDTH, MIN_SIZE, MIN_TARGET_SCALE, MIN_THICKNESS};
use crate::zoom::{MAX_DIVISOR, MIN_DIVISOR};
use crate::zoom_config::{
    KeyMode, MAX_PER_STEP, MAX_STEPS, MAX_TIME, MIN_PER_STEP, MIN_STEPS, Preset, ZoomKey, transition_at,
    transition_index,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Effect {
    pub save: bool,
    pub republish: bool,
    /// The option keys changed: reserve them again.
    pub rebind: bool,
}

impl Effect {
    pub fn merge(self, other: Effect) -> Effect {
        Effect {
            save: self.save || other.save,
            republish: self.republish || other.republish,
            rebind: self.rebind || other.rebind,
        }
    }
}

/// How finely a HUD element's size can be set: two decimals (1.25, 1.07).
const SCALE_STEP: f64 = 0.01;

/// Every slider moves in hundredths (Max steps stays whole), and its number can be clicked and typed.
const SLIDER_STEP: f64 = 0.01;
/// A border's thickness moves in quarter pixels: finer would not show.
const BORDER_STEP: f64 = 0.25;

/// A crosshair slider: the value always lies inside its range. The range and the step never
/// depend on the GUI scale (the host fixes them when the page is built, and refuses an update that
/// leaves them); the setting is snapped to a whole device pixel when it is stored instead.
fn crosshair_slider<'a>(value: f64, min: f64, max: f64) -> Value<'a> {
    Value::Slider(value.clamp(min, max), min, max, SLIDER_STEP)
}

/// A length from a crosshair slider, on the device pixel grid (the Circle's ring is free).
fn on_pixels(value: f64, shape: usize, least: i32) -> f64 {
    if shape == 3 {
        value
    } else {
        round2(f64::from(crate::crosshair::snap(value as f32, least)))
    }
}

/// A size from the slider, to two decimals and within the limits (the slider sends a single-precision number, so a
/// 1.3 arrives as 1.2999999 and would otherwise be saved and shown that way).
fn round_scale(value: f64) -> f64 {
    round2(value).clamp(MIN_SCALE, MAX_SCALE)
}

/// A slider number to two decimals. Every slider moves in hundredths and its value can be typed in, and the
/// single-precision number the menu sends back (1.3 arrives as 1.2999999) must not be stored that way.
fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// A key being rebound: the menu's, an option's or an action's (a key slot: `TOGGLES`, then `ACTION_KEYS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bind {
    Menu,
    Option(usize),
    Zoom(ZoomKey),
}

impl Bind {
    fn from_id(id: &str) -> Option<Self> {
        if id == "menu_key" {
            return Some(Self::Menu);
        }
        if let Some(key) = ZoomKey::from_id(id) {
            return Some(Self::Zoom(key));
        }
        let option = id.strip_prefix("key_")?;
        key_slot_position(option).map(Self::Option)
    }

    /// The menu control's id.
    fn id(self) -> &'static str {
        match self {
            Self::Menu => "menu_key",
            Self::Option(i) => KEY_IDS[i],
            Self::Zoom(key) => key.id(),
        }
    }
}

/// The key caps' control ids, in the order of the key slots (`TOGGLES`, `ACTION_KEYS`, `LATER_TOGGLES`).
const KEY_IDS: [&str; KEY_SLOTS] = [
    "key_borders",
    "key_subchunks",
    "key_slime_chunks",
    "key_guardians",
    "key_lights",
    "key_blaze",
    "key_hitboxes",
    "key_hud_keystrokes",
    "key_hud_cps",
    "key_hud_fps",
    "key_hud_coords",
    "key_hud_facing",
    "key_hud_chunk",
    "key_hud_slime",
    "key_hud_monument",
    "key_hud_armor",
    "key_hud_arrows",
    "key_hud_ping",
    "key_hud_speed",
    "key_hud_stopwatch",
    "key_hud_playtime",
    "key_fullbright",
    "key_hide_scoreboard",
    "key_sw_pause",
    "key_sw_reset",
    "key_hud_reach",
    "key_hud_combo",
    "key_hud_food",
    "key_hud_day",
    "key_hud_potion",
    "key_hud_arraylist",
    "key_hud_itemcounter",
    "key_hud_inventory",
    "key_hud_clock",
    "key_hud_viewangles",
    "key_hud_potioncount",
    "key_hud_totems",
    "key_item_delay_fix",
    "key_ch_on",
    "key_zoom_on",
    "key_gp_toggle_sprint",
    "key_gp_toggle_sneak",
    "key_gp_always_sprint",
    "key_gp_time_on",
    "key_gp_weather_on",
    "key_gp_no_hurt_cam",
    "key_gp_cursor_center",
    "key_gp_nametags",
    "key_hud_scoreboard",
    "key_gp_hit_on",
    "key_gp_blur_on",
    "key_gp_dmg_on",
    "key_hud_target",
    "key_hud_waypoints",
    "key_wp_add",
    "key_wp_remove",
    "key_wp_undo",
    "key_gp_wp_hidden",
    "key_wp_quick",
    "key_wp_menu",
    "key_gp_nocam_on",
    "key_gp_noswing_on",
    "key_gp_cpslim_on",
    "key_gp_bo_on",
    "key_hud_waila",
    "key_hud_height",
    "key_hud_subtitles",
    "key_gp_mk_on",
    "key_mk_place",
    "key_mk_clear",
    "key_mk_danger",
    "key_waila_details",
    "key_gp_tnt_on",
    "key_hud_pvp",
    "key_gp_th_on",
    "key_th_key0",
    "key_th_key1",
    "key_th_key2",
    "key_th_key3",
    "key_gp_hive_on",
    "key_hive_rq",
    "key_gp_persp_on",
    "key_persp_cycle",
    "key_persp_behind",
    "key_persp_free",
    "key_persp_snap",
    "key_persp_snap_fwd",
    "key_gp_glass_on",
];

const DONE_SECONDS: f32 = 2.0;

/// The Style page's dropdowns in the cards under the Colors card (the element-specific ones).
const RIDING: [&str; 33] = ["inv_grid_color", "pvp_hits_lc", "pvp_acc_lc", "pvp_kills_lc", "pvp_deaths_lc", "pvp_num_tc", "pvp_pct_tc", "co_x_lc", "co_x_tc", "co_y_lc", "co_y_tc", "co_z_lc", "co_z_tc", "co_dir_tc", "co_dir_mx", "co_dir_mz", "va_label_color", "va_value_color", "waila_mod_color", "waila_info_color", "waila_good_color", "waila_bad_color", "gp_wp_color", "cps_mode", "bar_color", "speed_unit", "speed_decimals", "playtime_format", "reach_highlight_color", "sat_color", "sat_layout", "potion_mode", "counter_item"];
/// The same for the dropdowns in cards under the right column (they ride with the Box card's rows).
const RIDING_RIGHT: [&str; 1] = ["border_color"];

/// How long the dropdown list keeps drawing while it fades out (a little over its ease).
const DD_FADE_SECONDS: f32 = 0.2;
/// The list's height of a dropdown button, and the gap between the two.
const DD_BUTTON: f32 = 16.0;
const DD_GAP: f32 = 3.0;

/// The dropdown list that is open (or fading out): the control it chooses for, where it is in window
/// units, which way it opens and how many rows it has.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Open {
    id: &'static str,
    pos: [f32; 2],
    up: bool,
    /// Rows drawn at once, the first option shown and how many options there are (a list longer
    /// than its rows scrolls).
    rows: usize,
    first: usize,
    total: usize,
}

/// Each document's scroll. More documents than the 32 that `Default` covers for a plain array (the Style page
/// has one per element kind), so the slots are a wrapper with room to spare.
#[derive(Clone, Debug)]
struct Scroll([f32; 64]);

impl Default for Scroll {
    fn default() -> Self {
        Scroll([0.0; 64])
    }
}

impl std::ops::Index<usize> for Scroll {
    type Output = f32;
    fn index(&self, document: usize) -> &f32 {
        &self.0[document.min(63)]
    }
}

impl std::ops::IndexMut<usize> for Scroll {
    fn index_mut(&mut self, document: usize) -> &mut f32 {
        &mut self.0[document.min(63)]
    }
}

#[derive(Default)]
pub struct PanelState {
    capturing: Option<Bind>,
    done: Option<(&'static str, f32)>,
    notice: Option<String>,
    /// The open tab of the menu.
    tab: usize,
    /// The Mods grid's filter chip (0 All, 1 On, 2 HUD, 3 Visual, 4 Mechanic, 5 Favorites), its sort
    /// (`menu::SORTS`) and the text in its search box.
    filter: usize,
    sort: usize,
    search: String,
    /// The tab changed: the host needs that tab's document.
    tab_changed: bool,
    /// The page was opened from the Crosshair page while its window was grown: the window eases back.
    from_wide: bool,
    /// The HUD element the Layout and Look tabs edit (index into `Element::ALL`).
    pub selected: usize,
    /// "Move with the mouse": the menu hides and the HUD follows a drag.
    pub moving: bool,
    /// An element is held (or was let go a moment ago): the move bar is faded out of the way.
    pub dragging: bool,
    /// The dropdown list that is open.
    dropdown: Option<Open>,
    /// The list that was just closed, and how long it keeps drawing while it fades out.
    fading: Option<(Open, f32)>,
    /// The pointer in window units this frame, to put a list under the button that was pressed.
    pub pointer: Option<(f32, f32)>,
    /// The key prompt (shown while a key is bound), the number editor, the reset confirmation and the toast.
    cap: Fade,
    ed: Fade,
    cf: Fade,
    toast: Fade,
    /// The number being typed for a slider, and where its card sits (kept while it fades out).
    edit: Option<Edit>,
    /// A hex colour just typed, on its way into `apply` as the stored colour.
    hex: Option<usize>,
    ed_pos: [f32; 2],
    /// How far the window's left edge lies left of the one the pointer is measured from: half of what the Crosshair page grows
    /// by while its preview is out (0 elsewhere). A popup placed from the pointer is placed in the window's own units.
    ed_shift: f32,
    /// The open editor is the colour picker (not the number editor), its hue / saturation / brightness, and which of its
    /// boxes is being dragged (0 none, 1 the square, 2 the hue strip, 3 the opacity strip).
    ed_is_hex: bool,
    hsv: [f32; 3],
    pk_drag: u8,
    /// Whether the pointer button was held in the last picker frame (a press is the frame it turns on).
    pk_held: bool,
    /// The picked colour's transparency in percent (0 is opaque) and the text the picker shows for it.
    transparency: u32,
    op_text: String,
    /// The reset the confirmation asks about, and what it says.
    confirm: Option<&'static str>,
    confirm_text: String,
    /// A mod's reset the player confirmed: its page and element. It runs in a frame of its own (`work`), after the click's.
    reset_due: Option<(&'static str, Element)>,
    reset_armed: bool,
    toast_text: String,
    toast_left: f32,
    /// How far each page's scroll view is scrolled, by document, and where the thumb is held.
    scroll: Scroll,
    grab: Option<f32>,
    /// The Config page: saved setups and shared codes.
    setups: Setups,
    /// A native text field owns the keyboard (set by the mod each frame).
    pub native_editing: bool,
    was_native: bool,
    /// What a click that left a field did, until the next tick reports it.
    carry: Effect,
    /// A slot name changed while it was typed and is not saved yet (saved when the field closes).
    names_dirty: bool,
    /// The Waypoints page: the list, the editor's draft and the undo.
    pub wp: crate::waypoint_ui::WpUi,
}

/// How long a popup keeps drawing while it fades out (a little over its ease).
const POP_FADE_SECONDS: f32 = 0.22;
/// How long the toast stays.
const TOAST_SECONDS: f32 = 2.2;
/// How far a wheel notch scrolls a page, as the host's own scroll view did.
const SCROLL_STEP: f32 = 24.0;
/// The number editor's card, for putting it near the pointer.
const ED_W: f32 = 112.0;
const ED_H: f32 = 58.0;

/// A popup that eases open and keeps drawing while it fades out.
#[derive(Clone, Copy, Debug, Default)]
struct Fade {
    open: bool,
    left: f32,
}

impl Fade {
    fn set(&mut self, open: bool) {
        if self.open && !open {
            self.left = POP_FADE_SECONDS;
        }
        if open {
            self.left = 0.0;
        }
        self.open = open;
    }

    /// Counts the fade down; true when it has just finished (the menu then needs a republish).
    fn tick(&mut self, dt: f32) -> bool {
        if self.left <= 0.0 {
            return false;
        }
        self.left -= dt;
        self.left <= 0.0
    }

    fn pop(self) -> menu::Pop {
        menu::Pop { shown: self.open || self.left > 0.0, open: self.open }
    }
}

/// A number being typed into a slider's editor.
#[derive(Clone, Debug)]
struct Edit {
    id: String,
    title: String,
    text: String,
    /// Nothing typed yet: the first key replaces the number shown.
    fresh: bool,
}

impl PanelState {
    /// Keys go to the mod: one is being bound or a number is being typed.
    pub fn is_capturing(&self) -> bool {
        self.capturing.is_some() || self.edit.is_some() || self.native_editing
    }

    /// Records whether a native text field has the keyboard. A click on Apply while the hex field is
    /// focused only takes the focus away (the host swallows it), so leaving the field over Apply applies.
    #[inline(never)]
    pub fn set_native(&mut self, settings: &mut Settings, now: bool) {
        let blurred = self.was_native && !now;
        self.was_native = now;
        self.native_editing = now;
        if blurred && self.edit.as_ref().is_some_and(|e| !e.id.starts_with("hex:")) {
            if let Some((px, py)) = self.window_pointer() {
                let [x, y] = self.ed_pos;
                if px >= x + ED_W - 54.0 && px <= x + ED_W - 8.0 && py >= y + 38.0 && py <= y + 52.0 {
                    let applied = self.commit_edit(settings);
                    self.carry = self.carry.merge(applied);
                }
            }
        }
        if blurred && self.hex_open() {
            if let Some((px, py)) = self.window_pointer() {
                let [x, y] = self.ed_pos;
                if px >= x + menu::PICKER_W - 62.0 && px <= x + menu::PICKER_W - 8.0 && py >= y + menu::PICKER_H - 22.0 && py <= y + menu::PICKER_H - 8.0 {
                    let applied = self.commit_edit(settings);
                    self.carry = self.carry.merge(applied);
                }
            }
        }
    }

    /// Whether a number or colour editor is open (its native field sends text events).
    pub fn edit_open(&self) -> bool {
        self.edit.is_some()
    }

    /// Whether the hex colour editor is open (its native field sends text events).
    pub fn hex_open(&self) -> bool {
        self.edit.as_ref().is_some_and(|e| e.id.starts_with("hex:"))
    }

    /// Makes the Waypoints tab the one the menu opens on.
    pub fn show_waypoints_tab(&mut self) {
        if let Some(tab) = crate::menu_controls::TABS.iter().position(|t| *t == "Waypoints") {
            self.tab = tab;
            self.tab_changed = true;
            self.reset_scroll();
        }
    }

    /// Whether the Waypoints page is the open one (its fields send their text).
    pub fn on_waypoints(&self) -> bool {
        crate::menu_controls::TABS.get(self.tab).is_some_and(|name| *name == "Waypoints")
    }

    /// Whether the Config page is the open one.
    pub fn on_config(&self) -> bool {
        crate::menu_controls::TABS.get(self.tab).is_some_and(|name| *name == "Config")
    }

    /// Tells the Waypoints page where the player is (the menu being open); a republish when the page's texts changed.
    pub fn sync_waypoints(&mut self, settings: &Settings, world: &str, dimension: i32, pos: [i32; 3], in_world: bool) -> Effect {
        let changed = self.wp.sync(settings, world, dimension, pos, in_world);
        let open = crate::menu_controls::TABS.get(self.tab).is_some_and(|name| *name == "Waypoints");
        Effect { republish: changed && open, ..Effect::default() }
    }

    /// Whether the tab changed since last asked, so the whole menu must be sent again.
    /// The gear on an element's frame in the layout editor: leave the editor and open that element's Style page.
    pub fn open_settings(&mut self, element: usize) -> Effect {
        let style = crate::menu_controls::TABS.iter().position(|t| *t == "Style").unwrap_or(0);
        self.moving = false;
        self.dragging = false;
        self.selected = element.min(Element::COUNT - 1);
        self.tab_changed = true;
        self.tab = style;
        self.reset_scroll();
        Effect { republish: true, ..Effect::default() }
    }

    /// The Mods grid starts unfiltered each time it is shown: a search typed there is dropped when another page opens
    /// (coming back to a one-card grid is what the menu sent in the build that closed on the way back).
    fn leave_grid(&mut self, to: usize) {
        if self.tab == crate::menu_controls::MODS_PAGE && to != self.tab {
            self.search.clear();
        }
    }

    /// The page's number, for the debug trail.
    pub fn tab_number(&self) -> usize {
        self.tab
    }

    pub fn take_tab_changed(&mut self) -> bool {
        std::mem::take(&mut self.tab_changed)
    }

    pub fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }

    /// What the scroll position and the scroll limits are kept by: the element being edited.
    fn scroll_selected(&self) -> usize {
        self.selected
    }

    /// What the menu's documents are chosen by (the element being edited: the Mods grid is one document and scrolls itself).
    fn doc_selected(&self) -> usize {
        self.scroll_selected()
    }

    /// Whether the Mods grid is the open page.
    pub fn on_mods(&self) -> bool {
        self.tab == crate::menu_controls::MODS_PAGE
    }

    /// The cards the Mods grid shows now, as indices into `MODS`: the filter chip, then the search words, then the sort.
    pub fn grid_order(&self, settings: &Settings) -> Vec<usize> {
        let words = self.search.to_lowercase();
        let on = |id: &str| matches!(value(settings, self, id), Value::Toggle(true));
        let mut order: Vec<usize> = (0..crate::menu_controls::MODS.len())
            .filter(|n| {
                let (id, name, category) = crate::menu_controls::MODS[*n];
                let in_filter = match self.filter {
                    1 => on(id),
                    2 => category == 0,
                    3 => category == 1,
                    4 => category == 2,
                    5 => settings.favorites.iter().any(|f| f == id),
                    _ => true,
                };
                in_filter && words.split_whitespace().all(|w| name.contains(w) || id.contains(w))
            })
            .collect();
        match self.sort {
            1 => order.sort_by_key(|n| crate::menu_controls::MODS[*n].1),
            2 => order.sort_by_key(|n| (!on(crate::menu_controls::MODS[*n].0), *n)),
            _ => {}
        }
        order
    }

    /// How many of the grid's mods are on.
    pub fn grid_enabled(&self, settings: &Settings) -> usize {
        crate::menu_controls::MODS.iter().filter(|m| matches!(value(settings, self, m.0), Value::Toggle(true))).count()
    }

    /// The HUD element being edited.
    pub fn element(&self) -> Element {
        Element::from_index(self.selected)
    }

    /// Counts down the "Done!" label; republishes once when it goes back to normal.
    #[inline(never)]
    pub fn tick(&mut self, seconds: f32) -> Effect {
        let dt = if seconds.is_finite() { seconds.max(0.0) } else { 0.0 };
        let mut effect = std::mem::take(&mut self.carry);
        if self.wp.tick(dt) {
            effect.republish = true;
        }
        for fade in [&mut self.cap, &mut self.ed, &mut self.cf, &mut self.toast] {
            if fade.tick(dt) {
                effect.republish = true;
            }
        }
        if self.toast_left > 0.0 {
            self.toast_left -= dt;
            if self.toast_left <= 0.0 {
                self.toast.set(false);
                effect.republish = true;
            }
        }
        // A closed dropdown list keeps drawing while it fades out, then goes.
        if let Some((open, left)) = self.fading {
            if left - dt > 0.0 {
                self.fading = Some((open, left - dt));
            } else {
                self.fading = None;
                effect.republish = true;
            }
        }
        let Some((id, left)) = self.done else {
            return effect;
        };
        let left = left - dt;
        if left > 0.0 {
            self.done = Some((id, left));
        } else {
            self.done = None;
            effect.republish = true;
        }
        effect
    }

    /// The dropdown list for the menu: drawn while open or fading, and the row it highlights.
    fn dd(&self, settings: &Settings) -> menu::Dd {
        let list = match (self.dropdown, self.fading) {
            (Some(open), _) => Some((open, true)),
            (None, Some((open, _))) => Some((open, false)),
            _ => None,
        };
        let base = match list {
            Some((open, is_open)) => {
                let selected = match value(settings, self, open.id) {
                    Value::Choice(index, options) => index.min(options.len().saturating_sub(1)),
                    _ => 0,
                };
                menu::Dd { shown: true, open: is_open, pos: open.pos, up: open.up, rows: open.rows, first: open.first, total: open.total, selected, ..menu::Dd::default() }
            }
            None => menu::Dd::default(),
        };
        menu::Dd {
            cap: self.cap.pop(),
            ed: if self.ed_is_hex { menu::Pop::default() } else { self.ed.pop() },
            cp: if self.ed_is_hex { self.ed.pop() } else { menu::Pop::default() },
            hsv: self.hsv,
            transparency: self.transparency,
            recent: swatch_colours(&settings.gameplay.recent_colors),
            saved: swatch_colours(&settings.gameplay.favorite_colors),
            ed_pos: self.ed_pos,
            hex: self.ed_is_hex,
            swatch: self.edit.as_ref().filter(|e| e.id.starts_with("hex:")).map_or([0.3; 3], |e| {
                parse_hex(&e.text).map_or([0.3; 3], |v| [(v >> 16 & 255) as f32 / 255.0, (v >> 8 & 255) as f32 / 255.0, (v & 255) as f32 / 255.0])
            }),
            cf: self.cf.pop(),
            toast: self.toast.pop(),
            bar: self.bar(),
            typing: self.edit.is_some(),
            ..base
        }
    }

    /// The scroll bar of the open page: where its thumb is and how long.
    fn bar(&self) -> menu::Bar {
        let document = menu::document_index(self.tab, self.scroll_selected());
        let most = MAX_SCROLL[document];
        if most <= 0.5 {
            return menu::Bar::default();
        }
        let view = HEIGHT as f32 - 6.0 - VIEW_TOP[document];
        let len = (view * view / (view + most)).clamp(18.0, view);
        let scroll = self.scroll[document].clamp(0.0, most);
        menu::Bar { visible: true, scroll, thumb: scroll / most * (view - len), thumb_len: len, hot: self.grab.is_some() }
    }

    /// Puts every page's scroll back at the top, so a page always starts from its first row.
    pub fn reset_scroll(&mut self) {
        self.scroll = Scroll::default();
        self.grab = None;
    }

    /// Where the open page is scrolled to.
    fn scrolled(&self) -> f32 {
        let document = menu::document_index(self.tab, self.scroll_selected());
        self.scroll[document].clamp(0.0, MAX_SCROLL[document].max(0.0))
    }

    /// The wheel, a click on the scroll bar and a dragged thumb scroll the open page. `pointer` is in
    /// window units; `held` says whether the left button is down.
    #[inline(never)]
    pub fn scroll_frame(&mut self, pointer: Option<(f32, f32)>, pressed: bool, held: bool, wheel: f32) -> Effect {
        // An open list takes the wheel itself (see `wheel`).
        let wheel = if self.dropdown.is_some() { 0.0 } else { wheel };
        let document = menu::document_index(self.tab, self.scroll_selected());
        let most = MAX_SCROLL[document];
        if most <= 0.5 || self.edit.is_some() || self.confirm.is_some() || self.moving {
            self.grab = None;
            return Effect::default();
        }
        let top = VIEW_TOP[document];
        let view = HEIGHT as f32 - 6.0 - top;
        let bar = self.bar();
        let before = (self.scroll[document], self.grab.is_some());
        let mut scroll = self.scroll[document].clamp(0.0, most);
        if let Some((px, py)) = pointer {
            let over_view = px >= VIEW_LEFT - 2.0 && px <= VIEW_LEFT + VIEW_W + 10.0 && py >= top && py <= top + view;
            if wheel.is_finite() && wheel != 0.0 && over_view {
                scroll = (scroll - wheel * SCROLL_STEP).clamp(0.0, most);
            }
            let bar_x = VIEW_LEFT + VIEW_W + 4.0;
            if pressed && px >= bar_x - 10.0 && px <= bar_x + 16.0 && py >= top && py <= top + view {
                let thumb_top = top + bar.thumb;
                self.grab = Some(if py >= thumb_top && py <= thumb_top + bar.thumb_len { py - thumb_top } else { bar.thumb_len / 2.0 });
            }
            if let Some(grab) = self.grab {
                if held || pressed {
                    let travel = (view - bar.thumb_len).max(1.0);
                    scroll = ((py - grab - top) / travel).clamp(0.0, 1.0) * most;
                } else {
                    self.grab = None;
                }
            }
        } else if !held {
            self.grab = None;
        }
        self.scroll[document] = scroll;
        let changed = (scroll - before.0).abs() > 0.01 || before.1 != self.grab.is_some();
        Effect { republish: changed, ..Effect::default() }
    }

    /// Closes the dropdown list (it fades out first).
    fn close_dropdown(&mut self) -> Effect {
        if let Some(open) = self.dropdown.take() {
            self.fading = Some((open, DD_FADE_SECONDS));
        }
        Effect { republish: true, ..Effect::default() }
    }

    /// The wheel scrolls the page under an open list, so the list closes rather than stay behind.
    #[inline(never)]
    pub fn wheel(&mut self, wheel: f32) -> Effect {
        if self.dropdown.is_some() && wheel.is_finite() && wheel != 0.0 {
            if let Some(open) = self.dropdown.as_mut().filter(|open| open.total > open.rows) {
                // A long list scrolls a row a notch (the wheel's sign is up for positive).
                let last = open.total - open.rows;
                let first = if wheel > 0.0 { open.first.saturating_sub(1) } else { (open.first + 1).min(last) };
                if first == open.first {
                    return Effect::default();
                }
                open.first = first;
                return Effect { republish: true, ..Effect::default() };
            }
            return self.close_dropdown();
        }
        Effect::default()
    }

    /// Opens the list of the dropdown `id` under its button (or closes it when it is already open).
    fn open_dropdown(&mut self, settings: &Settings, id: &str) -> Effect {
        if self.dropdown.is_some_and(|open| open.id == id) {
            return self.close_dropdown();
        }
        let (rows, total, chosen) = match value(settings, self, id) {
            Value::Choice(index, options) => (options.len().min(DD_ROWS), options.len(), index.min(options.len().saturating_sub(1))),
            _ => return Effect::default(),
        };
        // A long list starts with the chosen option in view, near the middle.
        let first = chosen.saturating_sub(rows / 2).min(total.saturating_sub(rows));
        let Some(&(id, strip, x, y, y_keys)) = DROPDOWNS.iter().find(|d| d.0 == id) else {
            return Effect::default();
        };
        let document = menu::document_index(self.tab, self.scroll_selected());
        // The button's top left in window units. The picker strip stands still; a page's buttons
        // move with its scroll view, whose offset the mod is not told. A page that cannot scroll
        // needs no guess; otherwise the offset is the one that puts the button under the pointer
        // that pressed it, nearest the middle of the button.
        let (bx, by) = if strip {
            (x, y)
        } else {
            let y = if self.selected == Element::Keystrokes.index() { y_keys } else { y };
            // The cards under the Colors card ride up by its box rows while the background is off
            // (`SHIFTS`): the buttons of these dropdowns go with them.
            let column = if RIDING.contains(&id) { Some("s0") } else if RIDING_RIGHT.contains(&id) { Some("s1") } else { None };
            let rides = column.filter(|_| !settings.style(self.element()).background);
            let y = match rides {
                Some(name) => y - SHIFTS.iter().find(|s| s.0 == document && s.1 == name && s.3 == 0).map_or(0.0, |s| s.2),
                None => y,
            };
            let top = VIEW_TOP[document];
            let scroll = self.scrolled();
            (VIEW_LEFT + x, top + y - scroll)
        };
        let height = 2.0 * DD_PAD + rows as f32 * DD_ROW;
        let window = HEIGHT as f32;
        let below = by + DD_BUTTON + DD_GAP;
        let up = below + height > window - 4.0;
        let top = if up { by - DD_GAP - height } else { below };
        let left = bx.clamp(4.0, (WIDTH as f32 - 4.0 - DD_W).max(4.0));
        self.fading = None;
        self.dropdown = Some(Open { id, pos: [left, top.clamp(4.0, (window - 4.0 - height).max(4.0))], up, rows, first, total });
        Effect { republish: true, ..Effect::default() }
    }

    fn label(&self, id: &str, base: &'static str) -> &'static str {
        if self.done.is_some_and(|(done, _)| done == id) { "Done!" } else { base }
    }

    fn mark_done(&mut self, id: &'static str, notice: String) {
        self.done = Some((id, DONE_SECONDS));
        self.toast(&notice);
        self.notice = Some(notice);
    }

    #[inline(never)]
    pub fn event(&mut self, settings: &mut Settings, id: &str, value: f32) -> Effect {
        let effect = self.handle(settings, id, value);
        // The key prompt shows while a key is being bound.
        self.cap.set(self.capturing.is_some());
        effect
    }

    /// A control's label (its name in the menu).
    fn label_of(id: &str) -> &'static str {
        CONTROLS.iter().find(|(_, control, _)| *control == id).map_or("Value", |(_, _, label)| label)
    }

    /// Shows the toast for a moment.
    fn toast(&mut self, text: &str) {
        self.toast_text = text.trim_end_matches(". Done!").trim_end_matches('!').to_owned();
        self.toast_left = TOAST_SECONDS;
        self.toast.set(true);
    }

    /// Opens the number editor of the slider `id`, under the pointer that pressed its value box.
    fn open_edit(&mut self, settings: &Settings, id: &str) -> Effect {
        let Some(&(_, control, label)) = CONTROLS.iter().find(|(kind, control, _)| *kind == Kind::Slider && *control == id) else {
            return Effect::default();
        };
        let Value::Slider(current, ..) = value(settings, self, control) else {
            return Effect::default();
        };
        self.ed_shift = self.window_shift(settings);
        let (px, py) = self.window_pointer().unwrap_or((WIDTH as f32 / 2.0 + self.ed_shift, HEIGHT as f32 / 2.0));
        let x = (px - ED_W / 2.0).clamp(4.0, (WIDTH as f32 + 2.0 * self.ed_shift - 4.0 - ED_W).max(4.0));
        let below = py + 9.0;
        let y = if below + ED_H > HEIGHT as f32 - 4.0 { (py - 9.0 - ED_H).max(4.0) } else { below };
        let mut text = String::new();
        menu::num(&mut text, current);
        self.dropdown = None;
        self.fading = None;
        self.ed_pos = [x, y];
        self.ed_is_hex = false;
        self.edit = Some(Edit { id: control.to_owned(), title: label.to_owned(), text, fresh: true });
        self.ed.set(true);
        Effect { republish: true, ..Effect::default() }
    }

    /// Half of what the window has grown by on the open page (the Crosshair page with its preview out).
    fn window_shift(&self, settings: &Settings) -> f32 {
        crate::menu_controls::GROWTH.iter().find(|g| g.0 == self.tab).filter(|_| settings.crosshair.enabled).map_or(0.0, |g| g.1 as f32 / 2.0)
    }

    /// The pointer in the window's own units, which is where a popup is drawn and pressed.
    fn window_pointer(&self) -> Option<(f32, f32)> {
        self.pointer.map(|(px, py)| (px + self.ed_shift, py))
    }

    /// Opens the editor on the hex colour of the colour setting `id`.
    fn open_hex(&mut self, settings: &Settings, id: &str) -> Effect {
        let Value::Choice(current, _) = value(settings, self, id) else {
            return Effect::default();
        };
        self.ed_shift = self.window_shift(settings);
        let (px, py) = self.window_pointer().unwrap_or((WIDTH as f32 / 2.0 + self.ed_shift, HEIGHT as f32 / 2.0));
        let x = (px - menu::PICKER_W / 2.0).clamp(4.0, (WIDTH as f32 + 2.0 * self.ed_shift - 4.0 - menu::PICKER_W).max(4.0));
        let below = py + 9.0;
        let y = if below + menu::PICKER_H > HEIGHT as f32 - 4.0 { (py - 9.0 - menu::PICKER_H).max(4.0) } else { below };
        self.dropdown = None;
        self.fading = None;
        self.ed_pos = [x, y];
        self.ed_is_hex = true;
        self.pk_drag = 0;
        let bits = if id.starts_with("gp_wp_") { crate::waypoints::rgb24(current) } else { color_rgb(current) };
        self.hsv = menu::rgb_hsv(rgb_unit(bits));
        // A waypoint's colour has no opacity of its own; any other colour starts from the one it has.
        self.set_transparency(if id.starts_with("gp_wp_") { 0 } else { crate::config::custom_transparency(current) });
        self.edit = Some(Edit { id: format!("hex:{id}"), title: "Color".to_owned(), text: format!("{bits:06X}"), fresh: true });
        self.ed.set(true);
        Effect { republish: true, ..Effect::default() }
    }

    fn close_edit(&mut self) -> Effect {
        self.edit = None;
        self.ed.set(false);
        self.grab = None;
        Effect { republish: true, ..Effect::default() }
    }

    fn set_transparency(&mut self, transparency: u32) {
        self.transparency = transparency.min(crate::config::MAX_TRANSPARENCY);
        self.op_text = format!("Opacity {}%", 100 - self.transparency);
    }

    /// Whether the open picker's colour can carry opacity (a waypoint's cannot).
    fn picker_has_opacity(&self) -> bool {
        self.edit.as_ref().and_then(|e| e.id.strip_prefix("hex:")).is_some_and(|id| !id.starts_with("gp_wp_"))
    }

    /// A press and drag on the colour picker's square, hue strip or opacity strip. `pointer` is in window units. The colour moves with the
    /// pointer; Apply keeps it.
    #[inline(never)]
    pub fn picker_frame(&mut self, pointer: Option<(f32, f32)>, pressed: bool, held: bool) -> Effect {
        if !self.hex_open() {
            self.pk_drag = 0;
            self.pk_held = held;
            return Effect::default();
        }
        // A press over the card can reach the mod as a button press only, so the edge is found here too.
        let pressed = pressed || (held && !self.pk_held);
        self.pk_held = held;
        let [x, y] = self.ed_pos;
        let before = (self.hsv, self.pk_drag, self.transparency);
        let [sx, sy, sw, sh] = menu::PICKER_SV;
        let [hx, hy, hw, hh] = menu::PICKER_HUE;
        let [ox, oy, ow, oh] = menu::PICKER_OP;
        if let Some((px, py)) = pointer.map(|(px, py)| (px + self.ed_shift, py)).filter(|(px, py)| px.is_finite() && py.is_finite()) {
            if pressed {
                let in_square = px >= x + sx && px <= x + sx + sw && py >= y + sy && py <= y + sy + sh;
                let in_hue = px >= x + hx - 2.0 && px <= x + hx + hw + 2.0 && py >= y + hy - 3.0 && py <= y + hy + hh + 3.0;
                let in_op = self.picker_has_opacity() && px >= x + ox - 3.0 && px <= x + ox + ow + 3.0 && py >= y + oy - 3.0 && py <= y + oy + oh + 3.0;
                self.pk_drag = if in_square { 1 } else if in_hue { 2 } else if in_op { 3 } else { 0 };
            }
            if self.pk_drag != 0 && (held || pressed) {
                if self.pk_drag == 1 {
                    self.hsv[1] = ((px - x - sx) / sw).clamp(0.0, 1.0);
                    self.hsv[2] = 1.0 - ((py - y - sy) / sh).clamp(0.0, 1.0);
                } else if self.pk_drag == 2 {
                    self.hsv[0] = ((px - x - hx) / hw).clamp(0.0, 1.0);
                } else {
                    self.set_transparency((((py - y - oy) / oh).clamp(0.0, 1.0) * 100.0).round() as u32);
                }
            } else if !held {
                self.pk_drag = 0;
            }
        } else if !held {
            self.pk_drag = 0;
        }
        if self.hsv != before.0 {
            let [r, g, b] = menu::hsv_rgb(self.hsv[0], self.hsv[1], self.hsv[2]);
            let rgb = (((r * 255.0).round() as u32) << 16) | (((g * 255.0).round() as u32) << 8) | (b * 255.0).round() as u32;
            if let Some(edit) = self.edit.as_mut() {
                edit.text = format!("{rgb:06X}");
                edit.fresh = false;
            }
        }
        Effect { republish: self.hsv != before.0 || self.pk_drag != before.1 || self.transparency != before.2, ..Effect::default() }
    }

    /// Applies the typed number, snapped to the slider's range and step, as the host's editor did.
    fn commit_edit(&mut self, settings: &mut Settings) -> Effect {
        let Some(edit) = self.edit.take() else {
            return Effect::default();
        };
        self.ed.set(false);
        let mut effect = Effect { republish: true, ..Effect::default() };
        if let Some(id) = edit.id.strip_prefix("hex:") {
            if let Some(rgb) = parse_hex(&edit.text) {
                settings.gameplay.remember_color(rgb);
                let transparency = if id.starts_with("gp_wp_") { 0 } else { self.transparency };
                self.hex = Some(crate::config::custom_color_alpha(rgb, transparency));
                effect = effect.merge(self.apply(settings, id, 0.0));
                self.hex = None;
            }
            return effect;
        }
        let typed = edit.text.parse::<f64>().ok().filter(|v| v.is_finite());
        if let (Some(typed), Value::Slider(_, min, max, step)) = (typed, value(settings, self, &edit.id)) {
            let snapped = if step > 0.0 { ((typed.clamp(min, max) - min) / step).round() * step + min } else { typed };
            effect = effect.merge(self.apply(settings, &edit.id, snapped.clamp(min, max) as f32));
        }
        effect
    }

    /// The keys of the number editor: digits, a point, a minus, Backspace, Enter and Escape.
    #[inline(never)]
    pub fn typing(&mut self, settings: &mut Settings, pressed: &[String]) -> Effect {
        let mut effect = Effect::default();
        let native = self.native_editing;
        for key in pressed {
            let Some(edit) = self.edit.as_mut() else {
                break;
            };
            // The hex editor's own field takes the letters while it is focused: only Enter and Escape are ours.
            if native && !matches!(key.as_str(), "Enter" | "NumpadEnter" | "Escape") {
                continue;
            }
            let digit = key.strip_prefix("Digit").or_else(|| key.strip_prefix("Numpad")).filter(|d| d.len() == 1 && d.as_bytes()[0].is_ascii_digit());
            let before = edit.text.clone();
            match key.as_str() {
                "Enter" | "NumpadEnter" => return effect.merge(self.commit_edit(settings)),
                "Escape" => return effect.merge(self.close_edit()),
                "Backspace" => {
                    if edit.fresh {
                        edit.text.clear();
                        edit.fresh = false;
                    } else {
                        edit.text.pop();
                    }
                }
                _ if edit.id.starts_with("hex:") => {
                    let digit = digit.map(str::to_owned).or_else(|| key.strip_prefix("Key").filter(|k| k.len() == 1 && matches!(k.as_bytes()[0], b'A'..=b'F')).map(str::to_owned));
                    if let Some(d) = digit {
                        if edit.fresh {
                            edit.text.clear();
                            edit.fresh = false;
                        }
                        if edit.text.len() < 6 {
                            edit.text.push_str(&d);
                        }
                    }
                }
                "Period" | "NumpadDecimal" => {
                    if edit.fresh {
                        edit.text = "0.".to_owned();
                        edit.fresh = false;
                    } else if !edit.text.contains('.') && edit.text.len() < 8 {
                        if edit.text.is_empty() || edit.text == "-" {
                            edit.text.push('0');
                        }
                        edit.text.push('.');
                    }
                }
                "Minus" | "NumpadSubtract" => {
                    if edit.fresh {
                        edit.text = "-".to_owned();
                        edit.fresh = false;
                    } else if edit.text.is_empty() {
                        edit.text.push('-');
                    }
                }
                _ => {
                    if let Some(d) = digit {
                        if edit.fresh {
                            edit.text.clear();
                            edit.fresh = false;
                        }
                        if edit.text.len() < 8 {
                            edit.text.push_str(d);
                        }
                    }
                }
            }
            if edit.text != before {
                effect.republish = true;
            }
        }
        effect
    }

    fn handle(&mut self, settings: &mut Settings, id: &str, value: f32) -> Effect {
        // The Waypoints page: its fields open their editor on a press, its buttons act on the list.
        if id.starts_with("wp_") {
            if matches!(id, "wp_name" | "wp_x" | "wp_y" | "wp_z" | "wp_group") {
                return Effect::default();
            }
            let out = self.wp.event(settings, id);
            self.dropdown = None;
            self.fading = None;
            return Effect { save: out.save, republish: out.republish, rebind: false };
        }
        // The popups the menu draws: the number editor and the reset confirmation.
        match id {
            "ed_hold" | "cf_hold" | "cap_name" | "ed_title" | "cf_text" | "toast_text" | "cp_op" => return Effect::default(),
            "ed_ok" => return self.commit_edit(settings),
            // The picker's Save button and its recent and saved colours.
            "cp_star" => {
                let Some(rgb) = self.edit.as_ref().filter(|e| e.id.starts_with("hex:")).and_then(|e| parse_hex(&e.text)) else {
                    return Effect::default();
                };
                settings.gameplay.toggle_favorite(rgb);
                return Effect { save: true, republish: true, ..Effect::default() };
            }
            _ if id.starts_with("cp_r") || id.starts_with("cp_f") || id.starts_with("cp_p") => {
                let n = id[4..].parse::<usize>().unwrap_or(usize::MAX);
                let rgb = match id.as_bytes()[3] {
                    b'p' => menu::PRESETS.get(n).copied(),
                    b'r' => settings.gameplay.recent_colors.get(n).copied(),
                    _ => settings.gameplay.favorite_colors.get(n).copied(),
                };
                let Some(rgb) = rgb else {
                    return Effect::default();
                };
                if let Some(edit) = self.edit.as_mut().filter(|e| e.id.starts_with("hex:")) {
                    edit.text = format!("{rgb:06X}");
                    edit.fresh = false;
                    let typed = menu::rgb_hsv(rgb_unit(rgb));
                    self.hsv = [if typed[1] > 0.0 && typed[2] > 0.0 { typed[0] } else { self.hsv[0] }, typed[1], typed[2]];
                }
                return Effect { republish: true, ..Effect::default() };
            }
            "ed_no" | "ed_close" => return self.close_edit(),
            "cf_no" | "cf_close" => {
                self.confirm = None;
                self.cf.set(false);
                return Effect { republish: true, ..Effect::default() };
            }
            "cf_yes" => {
                let Some(action) = self.confirm.take() else {
                    return Effect::default();
                };
                self.cf.set(false);
                return self.apply(settings, action, 1.0).merge(Effect { republish: true, ..Effect::default() });
            }
            _ => {}
        }
        // A card's body only lights up while the pointer is over it (its own control, so the OPTIONS button does not light it).
        if id.starts_with("card_") {
            return Effect::default();
        }
        if let Some(effect) = self.setup_event(settings, id) {
            return effect;
        }
        if let Some(slider) = id.strip_prefix("edit_") {
            return self.open_edit(settings, slider);
        }
        // Resetting a mod's settings asks first.
        if id == "reset_mod" && self.confirm.is_none() {
            self.confirm = Some("reset_mod");
            self.confirm_text = format!("Reset {}?", self.reset_name());
            self.dropdown = None;
            self.fading = None;
            self.cf.set(true);
            return Effect { republish: true, ..Effect::default() };
        }
        // Resetting the size and place asks first.
        if id == "reset_layout" && self.confirm.is_none() {
            self.confirm = Some("reset_layout");
            self.confirm_text = format!("Reset {}?", self.element().label());
            self.dropdown = None;
            self.fading = None;
            self.cf.set(true);
            return Effect { republish: true, ..Effect::default() };
        }
        // The dropdown list: a row chooses, the clear button behind it (or a press anywhere else) closes.
        if id == "dd_close" {
            return self.close_dropdown();
        }
        if let Some(row) = id.strip_prefix("dd_opt").and_then(|n| n.parse::<usize>().ok()) {
            let Some(open) = self.dropdown.filter(|open| row < open.rows) else {
                return Effect::default();
            };
            self.close_dropdown();
            return self.apply(settings, open.id, (open.first + row) as f32).merge(Effect { republish: true, ..Effect::default() });
        }
        if CONTROLS.iter().any(|(kind, control, _)| *kind == Kind::Choice && *control == id) {
            return self.open_dropdown(settings, id);
        }
        self.dropdown = None;
        self.fading = None;
        self.apply(settings, id, value)
    }

    /// The Config page's buttons. None when `id` is not one of them.
    fn setup_event(&mut self, settings: &mut Settings, id: &str) -> Option<Effect> {
        let mut effect = Effect::default();
        if let Some(slot) = id.strip_prefix("cfg_slot_").and_then(|n| n.parse::<usize>().ok()) {
            self.setups.select(slot);
        } else {
            match id {
                "cfg_save" => self.setups.save(),
                "cfg_load" => self.setups.load(settings),
                "cfg_clear" => {
                    if self.setups.delete(settings) {
                        effect.save = true;
                    }
                }
                "cfg_undo" => self.setups.undo(),
                "cfg_export" => self.setups.export(),
                // The fields open their native editor on a press; the label under them does nothing.
                "cfg_info" => self.setups.info(),
                "cfg_out" | "cfg_in" | "cfg_name" | "module_search" | "gp_wp_quick" | "gp_th_cmd0" | "gp_th_cmd1" | "gp_th_cmd2" | "gp_th_cmd3" => return Some(Effect::default()),
                _ => return None,
            }
        }
        self.dropdown = None;
        self.fading = None;
        self.setups.refresh(settings);
        if let Some(message) = self.setups.take_message() {
            self.toast(&message);
        }
        effect.republish = true;
        Some(effect)
    }

    /// A text committed in one of the page's fields.
    #[inline(never)]
    pub fn field(&mut self, settings: &mut Settings, id: &str, text: &str, live: bool) -> Effect {
        let mut effect = Effect { republish: true, ..Effect::default() };
        match id {
            // The name is applied as it is typed; the file is written when the editor closes.
            "cfg_name" => {
                if self.setups.rename(settings, text) {
                    self.names_dirty = true;
                }
                if live {
                    self.setups.take_message();
                } else if self.names_dirty {
                    self.names_dirty = false;
                    effect.save = true;
                }
            }
            "cfg_in" => {
                self.setups.paste(text);
            }
            "wp_name" | "wp_x" | "wp_y" | "wp_z" | "wp_group" => self.wp.set_field(id, text),
            "gp_wp_quick" => {
                settings.gameplay.wp_quick_name = text.chars().filter(|c| !c.is_control()).take(24).collect();
                effect.save = !live;
            }
            "gp_th_cmd0" | "gp_th_cmd1" | "gp_th_cmd2" | "gp_th_cmd3" => {
                let slot = usize::from(id.as_bytes()[id.len() - 1] - b'0');
                settings.gameplay.th_cmd[slot] = crate::gameplay::clean_command(text);
                effect.save = !live;
            }
            // The Mods grid's search: the cards narrow as it is typed.
            "module_search" => {
                self.search = text.chars().filter(|c| !c.is_control()).take(48).collect();
            }
            // The number editor's field (a touch keyboard types here): digits, one point and a leading minus.
            "ed_text" => {
                if let Some(edit) = self.edit.as_mut().filter(|e| !e.id.starts_with("hex:")) {
                    let mut out = String::new();
                    for c in text.chars() {
                        if c.is_ascii_digit() || (c == '.' && !out.contains('.')) || (c == '-' && out.is_empty()) {
                            out.push(c);
                        }
                    }
                    out.truncate(8);
                    // The field starts with the number in it: what comes after that is the new text.
                    if edit.fresh {
                        if let Some(rest) = out.strip_prefix(edit.text.as_str()).filter(|r| !r.is_empty()) {
                            out = rest.to_owned();
                        }
                    }
                    edit.text = out;
                    edit.fresh = false;
                    effect.republish = true;
                    if !live {
                        return effect.merge(self.commit_edit(settings));
                    }
                }
                return effect;
            }
            // The hex editor's field: what was typed or pasted, kept to the six digits it can hold.
            "hex_in" => {
                if let Some(edit) = self.edit.as_mut().filter(|e| e.id.starts_with("hex:")) {
                    let mut digits: String = text.chars().filter(char::is_ascii_hexdigit).map(|c| c.to_ascii_uppercase()).collect();
                    // The field starts with the colour in it: what comes after that is the new text.
                    if edit.fresh {
                        if let Some(rest) = digits.strip_prefix(edit.text.as_str()).filter(|r| !r.is_empty()) {
                            digits = rest.to_owned();
                        } else if let Some(rest) = digits.strip_suffix(edit.text.as_str()).filter(|r| !r.is_empty()) {
                            digits = rest.to_owned();
                        }
                    }
                    digits.truncate(6);
                    if digits.len() == 6 {
                        if let Some(rgb) = parse_hex(&digits) {
                            let typed = menu::rgb_hsv(rgb_unit(rgb));
                            // A grey or black has no hue of its own: keep the one on the strip.
                            self.hsv = [if typed[1] > 0.0 && typed[2] > 0.0 { typed[0] } else { self.hsv[0] }, typed[1], typed[2]];
                        }
                    }
                    edit.text = digits;
                    edit.fresh = false;
                }
                return effect;
            }
            // The code is for copying: whatever is typed over it is put back.
            "cfg_out" => {}
            _ => return Effect::default(),
        }
        self.setups.refresh(settings);
        if let Some(message) = self.setups.take_message() {
            self.toast(&message);
        }
        effect
    }

    /// One step of a waiting save, load, export or import (each pass over the settings text gets a
    /// frame of its own, see `setups.rs`).
    #[inline(never)]
    pub fn work(&mut self, settings: &mut Settings) -> Effect {
        if let Some((page, element)) = self.reset_due {
            // The click's own frame is over: the reset (it rewrites the whole settings text) takes the next one.
            if !self.reset_armed {
                self.reset_armed = true;
                return Effect::default();
            }
            self.reset_due = None;
            return self.reset_now(settings, page, element);
        }
        let saved_names = self.names_dirty && !self.native_editing;
        self.names_dirty &= self.native_editing;
        let (changed, page) = self.setups.work(settings);
        let fresh = self.setups.refresh(settings);
        let message = self.setups.take_message();
        let shown = message.is_some();
        if let Some(message) = message {
            self.toast(&message);
        }
        // The saved setups are listed down the side of every page, so their labels refresh wherever the menu is.
        let open = self.on_config();
        Effect { save: changed || saved_names, rebind: changed, republish: shown || (page && open) || fresh || changed }
    }

    /// What the reset button on this page names: the element edited (Style, Layout) or the mod's page.
    fn reset_name(&self) -> &'static str {
        let page = crate::menu_controls::TABS[self.tab.min(crate::menu_controls::TABS.len() - 1)];
        if page == "Style" || page == "Layout" { self.element().label() } else { page }
    }

    /// Puts one mod's settings back to the start: a HUD element's style and options, the Zoom page, or a mod's own page.
    /// Whether the mod is on stays as it is.
    #[inline(never)]
    fn reset_now(&mut self, settings: &mut Settings, page: &'static str, element: Element) -> Effect {
        let defaults = Settings::default();
        let name = if page == "Style" || page == "Layout" {
            let enabled = settings.style(element).enabled;
            let style = settings.style_mut(element);
            *style = defaults.style(element).clone();
            style.enabled = enabled;
            let at = element.index();
            settings.gameplay.set_chroma_on(at, false);
            settings.gameplay.set_chroma_speed(at, crate::gameplay::CHROMA_SPEED.2);
            settings.gameplay.set_chroma_type(at, 0);
            settings.reset_fields(&defaults, crate::config::element_keys(element));
            element.label()
        } else {
            if page == "Zoom" {
                let enabled = settings.zoom.enabled;
                settings.zoom = defaults.zoom.clone();
                settings.zoom.enabled = enabled;
            } else {
                settings.reset_fields(&defaults, crate::config::page_keys(page));
            }
            page
        };
        let notice = format!("{name}: settings reset. Done!");
        self.toast(&notice);
        self.notice = Some(notice);
        Effect { save: true, rebind: true, republish: true }
    }

    /// Applies a control's event to the settings.
    fn apply(&mut self, settings: &mut Settings, id: &str, value: f32) -> Effect {
        if menu::tab_event(id).is_none() {
            self.from_wide = false;
        }
        let on = value >= 0.5;
        let index = if value.is_finite() { value.max(0.0) as usize } else { 0 };
        // A typed hex colour comes in as the stored value; the dropdown's last row opens the editor.
        let index = self.hex.take().unwrap_or(index);
        if index == CUSTOM_ROW && is_colour(id) {
            return self.open_hex(settings, id);
        }
        let element = self.element();
        if let Some(tab) = menu::tab_event(id) {
            self.leave_grid(tab);
            self.tab_changed = tab != self.tab;
            self.from_wide = self.tab_changed && crate::menu_controls::GROWTH.iter().any(|g| g.0 == self.tab) && settings.crosshair.enabled;
            self.tab = tab;
            // A page always starts at the top, never where it was left.
            self.reset_scroll();
            return Effect { republish: true, ..Effect::default() };
        }
        // The Mods grid's filter chips, its sort button and a card's star.
        if let Some(n) = id.strip_prefix("flt_").and_then(|n| n.parse::<usize>().ok()) {
            if n < menu::FILTER_CHIPS {
                self.filter = n;
                return Effect { republish: true, ..Effect::default() };
            }
        }
        if id == "sort" {
            self.sort = (self.sort + 1) % menu::SORTS.len();
            return Effect { republish: true, ..Effect::default() };
        }
        if let Some(card) = id.strip_prefix("fav_") {
            if let Some(m) = crate::menu_controls::MODS.iter().find(|m| m.0 == card) {
                match settings.favorites.iter().position(|f| f == m.0) {
                    Some(at) => {
                        settings.favorites.remove(at);
                    }
                    None => settings.favorites.push(m.0.to_owned()),
                }
                return Effect { save: true, republish: true, ..Effect::default() };
            }
        }
        // The Sharing page: a switch takes its mod out of the codes made here (or puts it back), the buttons all at once.
        if let Some(name) = id.strip_prefix("share_") {
            match (name, crate::share::position(name)) {
                ("all", _) => settings.share_skip.clear(),
                ("none", _) => settings.share_skip = (0..crate::share::MODS.len() as u8).collect(),
                (_, Some(n)) => match settings.share_skip.iter().position(|m| usize::from(*m) == n) {
                    Some(at) => {
                        settings.share_skip.remove(at);
                    }
                    None => {
                        settings.share_skip.push(n as u8);
                        settings.share_skip.sort_unstable();
                    }
                },
                _ => return Effect::default(),
            }
            return Effect { save: true, republish: true, ..Effect::default() };
        }
        // The gear of a card opens that element's Layout page, its Options button the Style page.
        if let Some(n) = id.strip_prefix("lay_").and_then(|n| n.parse::<usize>().ok()) {
            if n < Element::COUNT {
                let layout = crate::menu_controls::TABS.iter().position(|t| *t == "Layout").unwrap_or(0);
                self.selected = n;
                self.leave_grid(layout);
                self.tab = layout;
                self.tab_changed = true;
                self.reset_scroll();
                return Effect { republish: true, ..Effect::default() };
            }
        }
        // An Options button of the Elements grid: edit that element, on its Style page.
        if let Some(n) = id.strip_prefix("opt_").and_then(|n| n.parse::<usize>().ok()) {
            if n < Element::COUNT {
                let style = crate::menu_controls::TABS.iter().position(|t| *t == "Style").unwrap_or(0);
                self.selected = n;
                self.leave_grid(style);
                self.tab = style;
                self.tab_changed = true;
                self.reset_scroll();
                return Effect { republish: true, ..Effect::default() };
            }
        }
        // The element picker's dropdowns: the Layout and Style pages edit that element.
        if let Some(chosen) = pick_element(id, index) {
            // Armor status has a document of its own on some pages: send it (or the usual one back).
            self.tab_changed = menu::document_index(self.tab, chosen.index()) != menu::document_index(self.tab, self.doc_selected());
            self.selected = chosen.index();
            self.reset_scroll();
            return Effect { republish: true, ..Effect::default() };
        }
        if let Some(bind) = Bind::from_id(id) {
            self.capturing = if self.capturing == Some(bind) { None } else { Some(bind) };
            return Effect { republish: true, ..Effect::default() };
        }
        // Copied only here: a tab press, the grid and the pickers above change no setting, and a copy of every setting
        // costs a good part of the frame's fuel (the Elements page is sent in the same one).
        let before = settings.clone();
        match id {
            // The move bar's switch for the centre lines.
            "move_guides" => {
                settings.show_guides = !settings.show_guides;
                return Effect { save: true, republish: true, ..Effect::default() };
            }
            "move" | "move_done" => {
                // A bar that has faded away is not there to be pressed.
                if id == "move_done" && self.dragging {
                    return Effect::default();
                }
                self.moving = id == "move";
                self.dragging = false;
                return Effect { republish: true, ..Effect::default() };
            }
            "reset_mod" => {
                let page = crate::menu_controls::TABS[self.tab.min(crate::menu_controls::TABS.len() - 1)];
                self.reset_due = Some((page, element));
                self.reset_armed = false;
                return Effect { republish: true, ..Effect::default() };
            }
            "reset_layout" | "move_reset" => {
                // Size, stretch and place go back to what the element starts with.
                let start = Settings::default().style(element).clone();
                let style = settings.style_mut(element);
                style.scale = start.scale;
                style.stretch = start.stretch;
                style.corner = start.corner;
                style.position = start.position;
                // Armor status with its pieces moved apart: each piece goes back to its own place and size
                // (where the row or column would have it), not only the element as a whole.
                style.armor_sizes = start.armor_sizes;
                if element == Element::Armor && style.armor_split {
                    crate::hud::armor_split_from_layout(style);
                }
                if element == Element::Potion {
                    settings.potion_offsets = vec![[0.0; 2]; crate::config::POTION_SLOTS];
                    settings.potion_scales = vec![1.0; crate::config::POTION_SLOTS];
                }
                let notice = format!("{}: size and place reset. Done!", element.label());
                if id == "reset_layout" {
                    self.mark_done("reset_layout", notice);
                } else {
                    self.notice = Some(notice);
                }
            }
            "top_left" | "top_right" | "bottom_left" | "bottom_right" => {
                let (corner, done, place) = match id {
                    "top_left" => (Corner::TopLeft, "top_left", "top left"),
                    "top_right" => (Corner::TopRight, "top_right", "top right"),
                    "bottom_left" => (Corner::BottomLeft, "bottom_left", "bottom left"),
                    _ => (Corner::BottomRight, "bottom_right", "bottom right"),
                };
                settings.style_mut(element).snap_to(corner);
                self.mark_done(done, format!("{}: moved to the {place}. Done!", element.label()));
            }
            "pos_x" | "pos_y" => {
                let style = settings.style_mut(element);
                let current = style.position.unwrap_or(style.corner.fractions());
                let v = (round2(f64::from(value)) / 100.0).clamp(0.0, 1.0) as f32;
                style.position = Some(if id == "pos_x" { [v, current[1]] } else { [current[0], v] });
            }
            "scale" => settings.style_mut(element).scale = round_scale(f64::from(value)),
            "stretch_x" => settings.style_mut(element).stretch[0] = round2(f64::from(value)).clamp(MIN_STRETCH, MAX_STRETCH),
            "stretch_y" => settings.style_mut(element).stretch[1] = round2(f64::from(value)).clamp(MIN_STRETCH, MAX_STRETCH),
            "opacity" => settings.style_mut(element).opacity = round2(f64::from(value)).clamp(0.0, 100.0),
            "bg_color" => settings.style_mut(element).bg_color = pick(index),
            "text_color" => settings.style_mut(element).text_color = pick(index),
            "pressed_color" => settings.style_mut(element).pressed_color = pick(index),
            "pressed_text_color" => settings.style_mut(element).pressed_text_color = pick(index),
            "labels" => settings.style_mut(element).labels = on,
            "smart" => settings.style_mut(element).smart = on,
            "chroma_on" => settings.gameplay.set_chroma_on(element.index(), on),
            "chroma_speed" => settings.gameplay.set_chroma_speed(element.index(), f64::from(value)),
            "chroma_type" => settings.gameplay.set_chroma_type(element.index(), index),
            "armor_chroma_type" => settings.gameplay.set_chroma_type(Element::Armor.index(), index),
            "rounded" => settings.style_mut(element).rounded = on,
            "background" => settings.style_mut(element).background = on,
            "shadow" => settings.style_mut(element).shadow = on,
            "brackets" => settings.style_mut(element).brackets = on,
            "reverse" => settings.style_mut(element).reverse = on,
            "auto_width" => settings.style_mut(element).auto_width = Some(on),
            "active_color" => settings.style_mut(element).active_color = Some(pick(index)),
            "box_border" => settings.style_mut(element).border = on,
            "border_width" => settings.style_mut(element).border_width = round2(f64::from(value)).clamp(MIN_BORDER, MAX_BORDER),
            "border_color" => settings.style_mut(element).border_color = pick(index),
            "border_opacity" => settings.style_mut(element).border_opacity = round2(f64::from(value)).clamp(0.0, 100.0),
            // The CPS bar's colour belongs to CPS, whichever element the pickers show.
            "bar_color" => settings.style_mut(Element::Cps).bar_color = Some(pick(index)),
            "show_chunk" => settings.style_mut(element).show_chunk = on,
            "show_inchunk" => settings.style_mut(element).show_inchunk = on,
            "show_sub" => settings.style_mut(element).show_sub = on,
            "sub_range" => settings.style_mut(element).sub_range = on,
            // Armor status options act on that element whichever one the pickers show.
            "armor_horizontal" => {
                let style = settings.style_mut(Element::Armor);
                if style.armor_horizontal != (index == 1) {
                    style.armor_horizontal = index == 1;
                    crate::hud::armor_reflow(style);
                }
            }
            "armor_side" => {
                let style = settings.style_mut(Element::Armor);
                let now = index.min(ARMOR_SIDE.len() - 1);
                if style.armor_side != now {
                    style.armor_side = now;
                    crate::hud::armor_reflow(style);
                }
            }
            "armor_text" => {
                let style = settings.style_mut(Element::Armor);
                if style.armor_text != index {
                    style.armor_text = index;
                    crate::hud::armor_reflow(style);
                }
            }
            "armor_held" => settings.style_mut(Element::Armor).armor_held = on,
            "armor_helmet" => settings.style_mut(Element::Armor).armor_pieces[0] = on,
            "armor_chest" => settings.style_mut(Element::Armor).armor_pieces[1] = on,
            "armor_legs" => settings.style_mut(Element::Armor).armor_pieces[2] = on,
            "armor_boots" => settings.style_mut(Element::Armor).armor_pieces[3] = on,
            "armor_split" => {
                let style = settings.style_mut(Element::Armor);
                if on && !style.armor_split {
                    // The pieces start where the row has them, so nothing jumps.
                    crate::hud::armor_split_from_layout(style);
                }
                style.armor_split = on;
            }
            "armor_gap" => {
                let style = settings.style_mut(Element::Armor);
                let gap = round2(f64::from(value));
                if style.armor_gap != gap {
                    style.armor_gap = gap;
                    crate::hud::armor_reflow(style);
                }
            }
            "armor_good" => settings.style_mut(Element::Armor).armor_good = pick(index),
            "armor_mid" => settings.style_mut(Element::Armor).armor_mid = pick(index),
            "armor_low" => settings.style_mut(Element::Armor).armor_low = pick(index),
            "armor_text_color" => settings.style_mut(Element::Armor).text_color = pick(index),
            "armor_smart" => settings.style_mut(Element::Armor).smart = on,
            "armor_shadow" => settings.style_mut(Element::Armor).shadow = on,
            "align" => settings.align = on,
            "item_delay_fix" => settings.item_delay_fix = on,
            "guides" => settings.show_guides = on,
            "show_mouse" => settings.show_mouse = on,
            "show_space" => settings.show_space = on,
            "cps_mode" => settings.cps_mode = CpsMode::from_index(index),
            "speed_unit" => settings.speed_unit = index.min(SPEED_UNITS.len() - 1),
            "counter_item" => settings.counter_item = index.min(COUNTER_ITEMS.len() - 1),
            "speed_decimals" => settings.speed_decimals = index.min(MAX_SPEED_DECIMALS),
            "speed_vertical" => settings.speed_vertical = on,
            "facing_text" => settings.facing_text = on,
            "clock_12h" => settings.clock_12h = on,
            "clock_seconds" => settings.clock_seconds = on,
            "sb_scores" => settings.sb_scores = on,
            "waila_id" => settings.waila_id = on,
            "waila_pos" => settings.waila_pos = on,
            "waila_mod" => settings.waila_mod = on,
            "waila_growth" => settings.waila_growth = on,
            "waila_tool" => settings.waila_tool = on,
            "waila_dist" => settings.waila_dist = on,
            "waila_state" => settings.waila_state = on,
            "waila_entity" => settings.waila_entity = on,
            "waila_mod_color" => settings.waila_mod_color = pick(index),
            "waila_info_color" => settings.waila_info_color = pick(index),
            "waila_good_color" => settings.waila_good_color = pick(index),
            "waila_bad_color" => settings.waila_bad_color = pick(index),
            "va_yaw" => settings.va_yaw = on,
            "va_pitch" => settings.va_pitch = on,
            "va_speed" => settings.va_speed = on,
            "va_altitude" => settings.va_altitude = on,
            "va_first" => settings.va_first = on,
            "va_third" => settings.va_third = on,
            "va_elytra" => settings.va_elytra = on,
            "va_reverse" => settings.va_reverse = on,
            "va_target" => settings.va_target = on,
            "inv_empty" => settings.inv_empty = on,
            "inv_show_bg" => settings.inv_show_bg = on,
            "inv_mc" => settings.inv_mc = on,
            "facing_label_chroma" => settings.facing_label_chroma = on,
            "inv_grid" => settings.inv_grid = on,
            "inv_grid_color" => settings.inv_grid_color = pick(index),
            "va_decimals" => settings.va_decimals = (value.round().max(0.0) as usize).min(crate::config::VA_MAX_DECIMALS),
            "va_align" => settings.va_align = index.min(crate::config::VA_ALIGN.len() - 1),
            "va_label_color" => settings.va_label_color = pick(index),
            "va_value_color" => settings.va_value_color = pick(index),
            "co_x" => settings.co_x = on,
            "co_y" => settings.co_y = on,
            "co_z" => settings.co_z = on,
            "co_dir" => settings.co_dir = on,
            "co_decimal" => settings.co_decimal = on,
            "co_horizontal" => settings.co_horizontal = on,
            "co_x_label" => settings.co_x_label = on,
            "co_y_label" => settings.co_y_label = on,
            "co_z_label" => settings.co_z_label = on,
            "co_dir_cardinal" => settings.co_dir_cardinal = on,
            "co_dir_mod" => settings.co_dir_mod = on,
            "co_x_lc" => settings.co_x_lc = pick(index),
            "co_x_tc" => settings.co_x_tc = pick(index),
            "co_y_lc" => settings.co_y_lc = pick(index),
            "co_y_tc" => settings.co_y_tc = pick(index),
            "co_z_lc" => settings.co_z_lc = pick(index),
            "co_z_tc" => settings.co_z_tc = pick(index),
            "co_dir_tc" => settings.co_dir_tc = pick(index),
            "co_dir_mx" => settings.co_dir_mx = pick(index),
            "co_dir_mz" => settings.co_dir_mz = pick(index),
            "pvp_hits_lc" => settings.pvp_hits_lc = pick(index),
            "pvp_acc_lc" => settings.pvp_acc_lc = pick(index),
            "pvp_kills_lc" => settings.pvp_kills_lc = pick(index),
            "pvp_deaths_lc" => settings.pvp_deaths_lc = pick(index),
            "pvp_num_tc" => settings.pvp_num_tc = pick(index),
            "pvp_pct_tc" => settings.pvp_pct_tc = pick(index),
            "waila_details" => settings.waila_details = index.min(crate::config::WAILA_DETAILS.len() - 1),
            "stopwatch_keep" => settings.stopwatch_keep = on,
            "playtime_format" => settings.playtime_format = index.min(TIME_FORMATS.len() - 1),
            "playtime_world" => settings.playtime_world_only = on,
            "reach_hide_zero" => settings.reach_hide_zero = on,
            "reach_highlight" => settings.reach_highlight = on,
            "reach_highlight_color" => settings.reach_highlight_color = pick(index),
            "sat_overlay" => settings.sat_overlay = on,
            "sat_hunger" => settings.sat_hunger = on,
            "sat_potential" => settings.sat_potential = on,
            "sat_inverse" => settings.sat_inverse = on,
            "sat_color" => settings.sat_color = pick(index),
            "sat_layout" => settings.sat_layout = index.min(crate::saturation::Layout::NAMES.len() - 1),
            "potion_mode" => settings.potion_mode = index.min(crate::potion::MODES.len() - 1),
            "potion_horizontal" => settings.potion_horizontal = on,
            "potion_per_line" => settings.potion_per_line = (value.round().max(1.0) as usize).min(crate::potion::MAX_PER_LINE),
            "potion_name" => settings.potion_name = on,
            "potion_level" => settings.potion_level = on,
            "potion_duration" => settings.potion_duration = on,
            "potion_formatted" => settings.potion_formatted = on,
            "potion_upper" => settings.potion_upper = on,
            "potion_blink" => settings.potion_blink = on,
            "potion_color_name" => settings.potion_color_name = on,
            "potion_color_info" => settings.potion_color_info = on,
            "potion_icons" => settings.potion_icons = on,
            "potion_hide_vanilla" => settings.potion_hide_vanilla = on,
            "potion_split" => settings.potion_split = on,
            "grid" => settings.grid = Grid::from_index(index),
            "hit_mode" => settings.hit_mode = index.min(crate::config::HIT_MODES.len() - 1),
            "hit_color" => settings.hit_color = pick(index),
            "gp_blur_dim" => settings.gameplay.blur_dim = f64::from(value).round().clamp(0.0, 100.0),
            "gp_bo_line" => settings.gameplay.bo_line = round2(f64::from(value)).clamp(1.0, 8.0),
            "gp_wp_padding" => settings.gameplay.wp_padding = f64::from(value).round().clamp(2.0, 8.0),
            "gp_wp_scale" => settings.gameplay.wp_label_scale = (f64::from(value) * 20.0).round() / 20.0,
            "gp_wp_line" => settings.gameplay.wp_line = (f64::from(value) * 4.0).round() / 4.0,
            "gp_blur_sat" => settings.gameplay.blur_sat = f64::from(value).round().clamp(0.0, 200.0),
            "gp_blur" => settings.gameplay.blur_amount = f64::from(value).round().clamp(0.0, crate::gameplay::MAX_BLUR),
            "hit_opacity" => settings.hit_opacity = round2(f64::from(value)).clamp(0.0, 100.0),
            "inv_grid_opacity" => settings.inv_grid_opacity = round2(f64::from(value)).clamp(0.0, 100.0),
            "hitbox_color" => settings.hitbox_color = pick(index),
            "hitbox_player_color" => settings.hitbox_player_color = pick(index),
            "line_width" => settings.line_width = round2(f64::from(value)),
            "zoom_p_default" => self.preset(settings, Preset::Default, "zoom_p_default", "Default"),
            "zoom_p_optifine" => self.preset(settings, Preset::Optifine, "zoom_p_optifine", "OptiFine"),
            "zoom_p_ok" => self.preset(settings, Preset::OkZoomer, "zoom_p_ok", "OK Zoomer"),
            _ if id.starts_with("zoom_") => {
                if !zoom_event(settings, id, value, on, index) {
                    return Effect::default();
                }
            }
            _ if id.starts_with("gp_") => {
                if let Some(option) = settings.gameplay.toggle_mut(id) {
                    *option = on;
                } else if !settings.gameplay.set_choice(id, index) {
                    return Effect::default();
                }
            }
            _ if id.starts_with("ch_") => {
                if !crosshair_event(&mut settings.crosshair, id, value, on, index) {
                    return Effect::default();
                }
            }
            _ => match settings.toggle_mut(id) {
                Some(option) => *option = on,
                None => return Effect::default(),
            },
        }
        settings.sanitize();
        Effect {
            save: *settings != before,
            republish: true,
            // The zoom's keys are reserved only while it is on.
            rebind: settings.zoom.reserved_keys() != before.zoom.reserved_keys(),
        }
    }

    /// Applies a zoom preset (its keys and switch stay) and says so on the button.
    fn preset(&mut self, settings: &mut Settings, preset: Preset, id: &'static str, name: &str) {
        settings.zoom.apply_preset(preset);
        // The button says "Done!" and the menu shows its toast; no line in chat.
        self.done = Some((id, DONE_SECONDS));
        self.toast(&format!("Zoom: {name} preset applied. Done!"));
    }

    /// While a key is being rebound, the next key pressed becomes it. Escape cancels the menu
    /// key and clears an option's key.
    #[inline(never)]
    pub fn capture(&mut self, settings: &mut Settings, pressed: &[String]) -> Effect {
        let effect = self.capture_keys(settings, pressed);
        self.cap.set(self.capturing.is_some());
        effect
    }

    fn capture_keys(&mut self, settings: &mut Settings, pressed: &[String]) -> Effect {
        let Some(bind) = self.capturing else {
            return Effect::default();
        };
        for key in pressed {
            if key == "Escape" {
                self.capturing = None;
                match bind {
                    // Escape puts the menu key back to Right Shift (sanitising refills it).
                    Bind::Menu => settings.menu_key.clear(),
                    Bind::Option(i) => settings.set_key(key_slot_id(i).unwrap_or(""), ""),
                    Bind::Zoom(zoom_key) => settings.zoom.set_key(zoom_key, ""),
                }
                return Effect { save: true, republish: true, rebind: true };
            }
            // Left click operates the panel, including the keycap that started the capture.
            if key.starts_with("Mouse") {
                continue;
            }
            if valid_key(key) {
                match bind {
                    Bind::Menu => settings.menu_key = key.clone(),
                    Bind::Option(i) => {
                        let option = key_slot_id(i).unwrap_or("");
                        // A key toggles one option: take it from any other.
                        settings.keys.retain(|id, bound| id.as_str() == option || bound.as_str() != key.as_str());
                        settings.set_key(option, key);
                    }
                    Bind::Zoom(zoom_key) => {
                        // Likewise for the zoom: the key leaves any option that had it.
                        settings.keys.retain(|_, bound| bound.as_str() != key.as_str());
                        settings.zoom.set_key(zoom_key, key);
                    }
                }
                settings.sanitize();
                self.capturing = None;
                return Effect { save: true, republish: true, rebind: true };
            }
        }
        Effect::default()
    }

    /// The option a pressed key toggles, if any.
    pub fn option_for_key<'a>(settings: &Settings, key: &str) -> Option<&'a str> {
        TOGGLES.iter().chain(LATER_TOGGLES.iter()).chain(MORE_TOGGLES.iter()).find(|(id, _)| settings.key(id) == key).map(|(id, _)| *id)
    }
}

/// Whether `id` is a colour dropdown.
fn is_colour(id: &str) -> bool {
    matches!(
        id,
        "bg_color" | "text_color" | "pressed_color" | "pressed_text_color" | "active_color" | "border_color" | "bar_color" | "armor_good" | "armor_mid" | "armor_low" | "armor_text_color" | "hit_color" | "hitbox_color" | "reach_highlight_color" | "sat_color" | "gp_wp_color" | "gp_wp_edit_color"
            | "hitbox_player_color" | "va_label_color" | "va_value_color" | "co_x_lc" | "co_x_tc" | "co_y_lc" | "co_y_tc" | "co_z_lc" | "co_z_tc" | "co_dir_tc" | "co_dir_mx" | "co_dir_mz" | "pvp_hits_lc" | "pvp_acc_lc" | "pvp_kills_lc" | "pvp_deaths_lc" | "pvp_num_tc" | "pvp_pct_tc" | "inv_grid_color" | "waila_mod_color" | "waila_info_color" | "waila_good_color" | "waila_bad_color" | "ch_color" | "ch_outline_color" | "ch_player_color" | "ch_mob_color"
    )
}

/// `RRGGBB` or `RGB` (as typed in the hex editor) as 24 bits.
/// A colour's 24 bits as red, green and blue from 0 to 1.
/// The picker's swatch row for a list of colours: each colour, then faint boxes for the empty slots.
fn swatch_colours(list: &[u32]) -> [[f32; 4]; 6] {
    let mut out = [[0.78, 0.66, 1.0, 0.1]; 6];
    for (slot, rgb) in out.iter_mut().zip(list) {
        let [r, g, b] = rgb_unit(*rgb);
        *slot = [r, g, b, 1.0];
    }
    out
}

fn rgb_unit(rgb: u32) -> [f32; 3] {
    [(rgb >> 16 & 255) as f32 / 255.0, (rgb >> 8 & 255) as f32 / 255.0, (rgb & 255) as f32 / 255.0]
}

fn parse_hex(text: &str) -> Option<u32> {
    let digits = text.trim_start_matches('#');
    match digits.len() {
        6 => u32::from_str_radix(digits, 16).ok(),
        3 => {
            let v = u32::from_str_radix(digits, 16).ok()?;
            Some(((v >> 8 & 15) * 17) << 16 | ((v >> 4 & 15) * 17) << 8 | (v & 15) * 17)
        }
        _ => None,
    }
}

/// The element a picker dropdown chose: `element_group` jumps to the first element of that group,
/// `element_<group>` picks one of its elements. None for any other id or a place that is not there.
fn pick_element(id: &str, index: usize) -> Option<Element> {
    if id == "element" {
        return Element::ALL.get(index).copied();
    }
    let groups = &Element::GROUPS;
    if id == "element_group" {
        return groups.get(index)?.elements.first().copied();
    }
    let group = id.strip_prefix("element_")?.parse::<usize>().ok()?;
    groups.get(group)?.elements.get(index).copied()
}

/// The whole menu for the current settings (the bundle, or a changed menu key).
#[inline(never)]
pub fn build(settings: &Settings, state: &PanelState) -> String {
    let value = |id: &str| value(settings, state, id);
    let order = state.grid_order(settings);
    let flags = menu::Flags { order: &order, ..flags(settings, state) };
    menu::build(&settings.menu_key, state.capturing.map(Bind::id), state.tab, state.doc_selected(), state.moving, state.dragging, flags, state.dd(settings), &value)
}

/// Only the values, for a menu the host already has.
#[inline(never)]
pub fn patch(settings: &Settings, state: &PanelState) -> String {
    let value = |id: &str| value(settings, state, id);
    let order = state.grid_order(settings);
    let flags = menu::Flags { order: &order, ..flags(settings, state) };
    menu::patch(state.capturing.map(Bind::id), state.tab, state.doc_selected(), state.moving, state.dragging, flags, state.dd(settings), &value)
}

/// Which optional rows of the menu show for the current settings.
fn flags<'a>(settings: &'a Settings, state: &'a PanelState) -> menu::Flags<'a> {
    menu::Flags {
        background: settings.style(state.element()).background,
        border: settings.style(state.element()).border,
        hit_custom: settings.hit_mode == 2,
        blur: settings.gameplay.menu_blur(),
        backdrop: settings.gameplay.menu_backdrop(),
        lines: settings.crosshair.has_lines(),
        dots: settings.crosshair.has_dot(),
        crosshair: Some(&settings.crosshair),
        from_wide: state.from_wide,
        name_empty: state.setups.name.is_empty(),
        paste_empty: true,
        search_empty: state.search.is_empty(),
        quick_empty: settings.gameplay.wp_quick_name.is_empty(),
        filter: state.filter,
        sort: state.sort,
        order: &[],
        enabled: state.grid_enabled(settings),
        slot: state.setups.slot,
        wp: Some(&state.wp),
    }
}

/// One control's value.
fn value<'a>(s: &'a Settings, state: &'a PanelState, id: &str) -> Value<'a> {
    // Fast paths for ids with no arm of their own below. The Mods page has about 350 controls, and these ids fell through
    // every arm and then three toggle tables, once each: that alone came close to a whole frame's fuel, and a page sent
    // in a busy frame ran out ("all fuel consumed by WebAssembly": the host quarantined the mod and the menu closed).
    if id.starts_with("opt_") || id.starts_with("lay_") || id.starts_with("flt_") || id == "reset_mod" {
        return Value::Button(None);
    }
    if let Some(option) = id.strip_prefix("key_") {
        return Value::Key(s.key(option));
    }
    if let Some(name) = id.strip_prefix("fav_") {
        return Value::Toggle(s.favorites.iter().any(|f| f == name));
    }
    // The Sharing page: a mod's switch is on while the codes made here carry it; the two buttons keep their labels.
    if let Some(name) = id.strip_prefix("share_") {
        return match crate::share::position(name) {
            Some(n) => Value::Toggle(!s.share_skip.contains(&(n as u8))),
            None => Value::Button(None),
        };
    }
    // The Crosshair page (the widest, with the live preview) is about a hundred controls: straight to its own table.
    if id.starts_with("ch_") {
        return crosshair_value(&s.crosshair, id);
    }
    if let Some(v) = state.wp.value(id) {
        return v;
    }
    let style = s.style(state.element());
    let armor = s.style(Element::Armor);
    let place = style.position.unwrap_or(style.corner.fractions());
    match id {
        "element" => Value::Choice(state.element().index(), &Element::LABELS),
        "element_group" => Value::Choice(state.element().group_position().0, &Element::GROUP_NAMES),
        "element_0" | "element_1" => {
            let g = usize::from(id == "element_1");
            let (group, at) = state.element().group_position();
            Value::Choice(if group == g { at } else { 0 }, Element::GROUPS[g].labels)
        }
        "grid" => Value::Choice(s.grid.index(), &Grid::LABELS),
        "hit_mode" => Value::Choice(s.hit_mode, &crate::config::HIT_MODES),
        "hit_color" => Value::Choice(s.hit_color, &COLOR_CHOICES),
        "gp_wp_padding" => Value::Slider(s.gameplay.wp_padding.clamp(2.0, 8.0), 2.0, 8.0, 1.0),
        "gp_wp_scale" => Value::Slider(s.gameplay.wp_label_scale.clamp(0.5, 3.0), 0.5, 3.0, 0.05),
        "gp_wp_line" => Value::Slider(s.gameplay.wp_line.clamp(1.0, 8.0), 1.0, 8.0, 0.25),
        "gp_blur_dim" => Value::Slider(s.gameplay.blur_dim.clamp(0.0, 100.0), 0.0, 100.0, SLIDER_STEP),
        "gp_blur_sat" => Value::Slider(s.gameplay.blur_sat.clamp(0.0, 200.0), 0.0, 200.0, SLIDER_STEP),
        "gp_blur" => Value::Slider(s.gameplay.blur_amount.clamp(0.0, crate::gameplay::MAX_BLUR), 0.0, crate::gameplay::MAX_BLUR, SLIDER_STEP),
        "hit_opacity" => Value::Slider(s.hit_opacity, 0.0, 100.0, SLIDER_STEP),
        "inv_grid_opacity" => Value::Slider(s.inv_grid_opacity, 0.0, 100.0, SLIDER_STEP),
        "hitbox_color" => Value::Choice(s.hitbox_color, &COLOR_CHOICES),
        "hitbox_player_color" => Value::Choice(s.hitbox_player_color, &COLOR_CHOICES),
        "line_width" => Value::Slider(s.line_width, MIN_WIDTH, MAX_WIDTH, SLIDER_STEP),
        "scale" => Value::Slider(style.scale, MIN_SCALE, MAX_SCALE, SCALE_STEP),
        "stretch_x" => Value::Slider(style.stretch[0], MIN_STRETCH, MAX_STRETCH, SLIDER_STEP),
        "stretch_y" => Value::Slider(style.stretch[1], MIN_STRETCH, MAX_STRETCH, SLIDER_STEP),
        "pos_x" => Value::Slider(f64::from(place[0]) * 100.0, 0.0, 100.0, SLIDER_STEP),
        "pos_y" => Value::Slider(f64::from(place[1]) * 100.0, 0.0, 100.0, SLIDER_STEP),
        "opacity" => Value::Slider(style.opacity, 0.0, 100.0, SLIDER_STEP),
        "bg_color" => Value::Choice(style.bg_color, &COLOR_CHOICES),
        "text_color" => Value::Choice(style.text_color, &COLOR_CHOICES),
        "pressed_color" => Value::Choice(style.pressed_color, &COLOR_CHOICES),
        "pressed_text_color" => Value::Choice(style.pressed_text_color, &COLOR_CHOICES),
        "labels" => Value::Toggle(style.labels),
        "smart" => Value::Toggle(style.smart),
        "chroma_on" => Value::Toggle(s.gameplay.chroma(state.element().index()).on),
        "chroma_speed" => Value::Slider(s.gameplay.chroma(state.element().index()).speed, crate::gameplay::CHROMA_SPEED.0, crate::gameplay::CHROMA_SPEED.1, SLIDER_STEP),
        "chroma_type" => Value::Choice(usize::from(!s.gameplay.chroma(state.element().index()).wave), &crate::gameplay::CHROMA_TYPE_NAMES),
        "armor_chroma_type" => Value::Choice(usize::from(!s.gameplay.chroma(Element::Armor.index()).wave), &crate::gameplay::CHROMA_TYPE_NAMES),
        "rounded" => Value::Toggle(style.rounded),
        "background" => Value::Toggle(style.background),
        "shadow" => Value::Toggle(style.shadow),
        "brackets" => Value::Toggle(style.brackets),
        "reverse" => Value::Toggle(style.reverse),
        "auto_width" => Value::Toggle(s.auto_width(state.element())),
        "active_color" => Value::Choice(s.active_color(state.element()), &COLOR_CHOICES),
        "box_border" => Value::Toggle(style.border),
        "border_width" => Value::Slider(style.border_width, MIN_BORDER, MAX_BORDER, BORDER_STEP),
        "border_color" => Value::Choice(style.border_color, &COLOR_CHOICES),
        "border_opacity" => Value::Slider(style.border_opacity, 0.0, 100.0, SLIDER_STEP),
        "bar_color" => Value::Choice(s.bar_color(), &COLOR_CHOICES),
        "show_chunk" => Value::Toggle(style.show_chunk),
        "show_inchunk" => Value::Toggle(style.show_inchunk),
        "show_sub" => Value::Toggle(style.show_sub),
        "sub_range" => Value::Toggle(style.sub_range),
        "armor_horizontal" => Value::Choice(usize::from(armor.armor_horizontal), &["Vertical", "Horizontal"]),
        "armor_side" => Value::Choice(armor.armor_side, &ARMOR_SIDE),
        "armor_text" => Value::Choice(armor.armor_text, &ARMOR_TEXT),
        "armor_held" => Value::Toggle(armor.armor_held),
        "armor_helmet" => Value::Toggle(armor.armor_pieces[0]),
        "armor_chest" => Value::Toggle(armor.armor_pieces[1]),
        "armor_legs" => Value::Toggle(armor.armor_pieces[2]),
        "armor_boots" => Value::Toggle(armor.armor_pieces[3]),
        "armor_split" => Value::Toggle(armor.armor_split),
        "armor_gap" => Value::Slider(armor.armor_gap, 0.0, MAX_ARMOR_GAP, SLIDER_STEP),
        "armor_good" => Value::Choice(armor.armor_good, &COLOR_CHOICES),
        "armor_mid" => Value::Choice(armor.armor_mid, &COLOR_CHOICES),
        "armor_low" => Value::Choice(armor.armor_low, &COLOR_CHOICES),
        "armor_text_color" => Value::Choice(armor.text_color, &COLOR_CHOICES),
        "armor_smart" => Value::Toggle(armor.smart),
        "armor_shadow" => Value::Toggle(armor.shadow),
        "align" => Value::Toggle(s.align),
        "item_delay_fix" => Value::Toggle(s.item_delay_fix),
        "guides" => Value::Toggle(s.show_guides),
        // The Config page; a button with no text yet keeps its generated label.
        "cfg_out" => Value::Text(&state.setups.code),
        "cfg_in" => Value::Text(""),
        "hex_in" => Value::Text(state.edit.as_ref().filter(|e| e.id.starts_with("hex:")).map_or("", |e| e.text.as_str())),
        "cfg_name" => Value::Text(&state.setups.name),
        "module_search" => Value::Text(&state.search),
        _ if id.starts_with("fav_") => Value::Toggle(s.favorites.iter().any(|f| f == &id[4..])),
        "cfg_info" => Value::Button(Some(&state.setups.import_label).filter(|t| !t.is_empty()).map(|t| t.as_str())),
        "cfg_undo" => Value::Button(Some(&state.setups.undo_label).filter(|t| !t.is_empty()).map(|t| t.as_str())),
        _ if id.starts_with("cfg_slot_") => {
            let n = id["cfg_slot_".len()..].parse::<usize>().unwrap_or(usize::MAX);
            Value::Button(state.setups.labels.get(n).filter(|t| !t.is_empty()).map(|t| t.as_str()))
        }
        "show_mouse" => Value::Toggle(s.show_mouse),
        "show_space" => Value::Toggle(s.show_space),
        "cps_mode" => Value::Choice(s.cps_mode.index(), &CpsMode::LABELS),
        "speed_unit" => Value::Choice(s.speed_unit, &SPEED_UNITS),
        "counter_item" => Value::Choice(s.counter_item, &COUNTER_ITEM_NAMES),
        "speed_decimals" => Value::Choice(s.speed_decimals, &SPEED_DECIMALS),
        "speed_vertical" => Value::Toggle(s.speed_vertical),
        "facing_text" => Value::Toggle(s.facing_text),
        "clock_12h" => Value::Toggle(s.clock_12h),
        "clock_seconds" => Value::Toggle(s.clock_seconds),
        "sb_scores" => Value::Toggle(s.sb_scores),
        "waila_id" => Value::Toggle(s.waila_id),
        "waila_pos" => Value::Toggle(s.waila_pos),
        "waila_mod" => Value::Toggle(s.waila_mod),
        "waila_growth" => Value::Toggle(s.waila_growth),
        "waila_tool" => Value::Toggle(s.waila_tool),
        "waila_dist" => Value::Toggle(s.waila_dist),
        "waila_state" => Value::Toggle(s.waila_state),
        "waila_entity" => Value::Toggle(s.waila_entity),
        "waila_mod_color" => Value::Choice(s.waila_mod_color, &COLOR_CHOICES),
        "waila_info_color" => Value::Choice(s.waila_info_color, &COLOR_CHOICES),
        "waila_good_color" => Value::Choice(s.waila_good_color, &COLOR_CHOICES),
        "waila_bad_color" => Value::Choice(s.waila_bad_color, &COLOR_CHOICES),
        "va_yaw" => Value::Toggle(s.va_yaw),
        "va_pitch" => Value::Toggle(s.va_pitch),
        "va_speed" => Value::Toggle(s.va_speed),
        "va_altitude" => Value::Toggle(s.va_altitude),
        "va_first" => Value::Toggle(s.va_first),
        "va_third" => Value::Toggle(s.va_third),
        "va_elytra" => Value::Toggle(s.va_elytra),
        "va_reverse" => Value::Toggle(s.va_reverse),
        "va_target" => Value::Toggle(s.va_target),
        "inv_empty" => Value::Toggle(s.inv_empty),
        "inv_show_bg" => Value::Toggle(s.inv_show_bg),
        "inv_mc" => Value::Toggle(s.inv_mc),
        "facing_label_chroma" => Value::Toggle(s.facing_label_chroma),
        "inv_grid" => Value::Toggle(s.inv_grid),
        "inv_grid_color" => Value::Choice(s.inv_grid_color, &COLOR_CHOICES),
        "va_decimals" => Value::Slider(s.va_decimals as f64, 0.0, crate::config::VA_MAX_DECIMALS as f64, 1.0),
        "va_align" => Value::Choice(s.va_align, &crate::config::VA_ALIGN),
        "va_label_color" => Value::Choice(s.va_label_color, &COLOR_CHOICES),
        "va_value_color" => Value::Choice(s.va_value_color, &COLOR_CHOICES),
        "co_x" => Value::Toggle(s.co_x),
        "co_y" => Value::Toggle(s.co_y),
        "co_z" => Value::Toggle(s.co_z),
        "co_dir" => Value::Toggle(s.co_dir),
        "co_decimal" => Value::Toggle(s.co_decimal),
        "co_horizontal" => Value::Toggle(s.co_horizontal),
        "co_x_label" => Value::Toggle(s.co_x_label),
        "co_y_label" => Value::Toggle(s.co_y_label),
        "co_z_label" => Value::Toggle(s.co_z_label),
        "co_dir_cardinal" => Value::Toggle(s.co_dir_cardinal),
        "co_dir_mod" => Value::Toggle(s.co_dir_mod),
        "co_x_lc" => Value::Choice(s.co_x_lc, &COLOR_CHOICES),
        "co_x_tc" => Value::Choice(s.co_x_tc, &COLOR_CHOICES),
        "co_y_lc" => Value::Choice(s.co_y_lc, &COLOR_CHOICES),
        "co_y_tc" => Value::Choice(s.co_y_tc, &COLOR_CHOICES),
        "co_z_lc" => Value::Choice(s.co_z_lc, &COLOR_CHOICES),
        "co_z_tc" => Value::Choice(s.co_z_tc, &COLOR_CHOICES),
        "co_dir_tc" => Value::Choice(s.co_dir_tc, &COLOR_CHOICES),
        "co_dir_mx" => Value::Choice(s.co_dir_mx, &COLOR_CHOICES),
        "co_dir_mz" => Value::Choice(s.co_dir_mz, &COLOR_CHOICES),
        "pvp_hits_lc" => Value::Choice(s.pvp_hits_lc, &COLOR_CHOICES),
        "pvp_acc_lc" => Value::Choice(s.pvp_acc_lc, &COLOR_CHOICES),
        "pvp_kills_lc" => Value::Choice(s.pvp_kills_lc, &COLOR_CHOICES),
        "pvp_deaths_lc" => Value::Choice(s.pvp_deaths_lc, &COLOR_CHOICES),
        "pvp_num_tc" => Value::Choice(s.pvp_num_tc, &COLOR_CHOICES),
        "pvp_pct_tc" => Value::Choice(s.pvp_pct_tc, &COLOR_CHOICES),
        "waila_details" => Value::Choice(s.waila_details, &crate::config::WAILA_DETAILS),
        "stopwatch_keep" => Value::Toggle(s.stopwatch_keep),
        "playtime_format" => Value::Choice(s.playtime_format, &TIME_FORMATS),
        "playtime_world" => Value::Toggle(s.playtime_world_only),
        "reach_hide_zero" => Value::Toggle(s.reach_hide_zero),
        "reach_highlight" => Value::Toggle(s.reach_highlight),
        "reach_highlight_color" => Value::Choice(s.reach_highlight_color, &COLOR_CHOICES),
        "sat_overlay" => Value::Toggle(s.sat_overlay),
        "sat_hunger" => Value::Toggle(s.sat_hunger),
        "sat_potential" => Value::Toggle(s.sat_potential),
        "sat_inverse" => Value::Toggle(s.sat_inverse),
        "sat_color" => Value::Choice(s.sat_color, &COLOR_CHOICES),
        "sat_layout" => Value::Choice(s.sat_layout, &crate::saturation::Layout::NAMES),
        "potion_mode" => Value::Choice(s.potion_mode, &crate::potion::MODES),
        "potion_horizontal" => Value::Toggle(s.potion_horizontal),
        "potion_per_line" => Value::Slider(s.potion_per_line as f64, 1.0, crate::potion::MAX_PER_LINE as f64, 1.0),
        "potion_name" => Value::Toggle(s.potion_name),
        "potion_level" => Value::Toggle(s.potion_level),
        "potion_duration" => Value::Toggle(s.potion_duration),
        "potion_formatted" => Value::Toggle(s.potion_formatted),
        "potion_upper" => Value::Toggle(s.potion_upper),
        "potion_blink" => Value::Toggle(s.potion_blink),
        "potion_color_name" => Value::Toggle(s.potion_color_name),
        "potion_color_info" => Value::Toggle(s.potion_color_info),
        "potion_icons" => Value::Toggle(s.potion_icons),
        "potion_hide_vanilla" => Value::Toggle(s.potion_hide_vanilla),
        "potion_split" => Value::Toggle(s.potion_split),
        "reset_layout" => Value::Button(Some(state.label("reset_layout", "Reset size and place"))),
        "move_guides" => Value::Button(Some(if s.show_guides { "Lines: On" } else { "Lines: Off" })),
        "top_left" => Value::Button(Some(state.label("top_left", "Top Left"))),
        "top_right" => Value::Button(Some(state.label("top_right", "Top Right"))),
        "bottom_left" => Value::Button(Some(state.label("bottom_left", "Bottom Left"))),
        "bottom_right" => Value::Button(Some(state.label("bottom_right", "Bottom Right"))),
        "menu_key" => Value::Key(&s.menu_key),
        // The popups' texts: what the key prompt names, the editor's title and number, the confirmation, the toast.
        "cap_name" => Value::Button(Some(state.capturing.map_or("Key", |bind| PanelState::label_of(bind.id())))),
        "cp_star" => Value::Button(Some(
            if state.edit.as_ref().filter(|e| e.id.starts_with("hex:")).and_then(|e| parse_hex(&e.text)).is_some_and(|rgb| s.gameplay.favorite_colors.contains(&rgb)) { "Saved" } else { "Save" },
        )),
        "cp_op" => Value::Button(Some(if state.op_text.is_empty() { "Opacity 100%" } else { state.op_text.as_str() })),
        "ed_title" => Value::Button(Some(state.edit.as_ref().map_or("Value", |e| e.title.as_str()))),
        "ed_text" => Value::Text(state.edit.as_ref().filter(|e| !e.id.starts_with("hex:")).map_or("", |e| e.text.as_str())),
        "cf_text" => Value::Button(Some(if state.confirm_text.is_empty() { "Reset?" } else { state.confirm_text.as_str() })),
        "toast_text" => Value::Button(Some(if state.toast_text.is_empty() { "Saved" } else { state.toast_text.as_str() })),
        // The dropdown list's rows: the open dropdown's options, one on each button.
        _ if id.starts_with("dd_opt") => {
            let row = id["dd_opt".len()..].parse::<usize>().unwrap_or(usize::MAX);
            let open = state.dropdown.or(state.fading.map(|(open, _)| open));
            match open.map(|open| value(s, state, open.id)) {
                // The host rejects an empty label, so a row with no option shows a space.
                Some(Value::Choice(_, options)) => Value::Button(Some(options.get(open.map_or(0, |o| o.first) + row).copied().filter(|o| !o.is_empty()).unwrap_or(" "))),
                _ => Value::Button(Some(" ")),
            }
        }
        _ if id.starts_with("zoom_") => zoom_value(s, state, id),
        _ if id.starts_with("gp_") => match id {
            "gp_time" => Value::Choice(s.gameplay.time_preset, &crate::gameplay::TIME_NAMES),
            "gp_weather" => Value::Choice(s.gameplay.weather, &crate::gameplay::WEATHER_NAMES),
            "gp_name_scale" => Value::Choice(s.gameplay.name_scale, &crate::gameplay::NAME_SCALE_NAMES),
            "gp_hurt_strength" => Value::Choice(s.gameplay.hurt_strength, &crate::gameplay::HURT_STRENGTH_NAMES),
            "gp_bo_line" => Value::Slider(s.gameplay.bo_line.clamp(1.0, 8.0), 1.0, 8.0, SLIDER_STEP),
            "gp_mk_color" => Value::Choice(s.gameplay.mk_color, &crate::waypoints::COLOR_NAMES),
            "gp_glass_mode" => Value::Choice(s.gameplay.glass_mode, &crate::glass::GLASS_NAMES),
            "gp_glass_coloured" => Value::Choice(s.gameplay.glass_coloured, &crate::glass::GLASS_NAMES),
            "gp_mk_time" => Value::Choice(s.gameplay.mk_time, &crate::gameplay::MARKER_TIME_NAMES),
            "gp_mk_danger_color" => Value::Choice(s.gameplay.mk_danger_color, &crate::waypoints::COLOR_NAMES),
            "gp_bo_mode" => Value::Choice(s.gameplay.bo_mode, &crate::outline::MODE_NAMES),
            "gp_bo_color" => Value::Choice(s.gameplay.bo_color.min(7), &COLOR_CHOICES[..8]),
            "gp_bo_fill" => Value::Choice(s.gameplay.bo_fill, &crate::outline::FILL_NAMES),
            "gp_bo_fix" => Value::Choice(s.gameplay.bo_fix, &crate::outline::FIX_NAMES),
            "gp_bo_opacity" => Value::Choice(s.gameplay.bo_opacity, &crate::outline::OPACITY_NAMES),
            "gp_cpslim_max" => Value::Choice(s.gameplay.cpslim_max, &crate::handlers::CPS_LIMIT_NAMES),
            "gp_dmg_size" => Value::Choice(s.gameplay.dmg_size, &crate::damage::SIZE_NAMES),
            "gp_dmg_time" => Value::Choice(s.gameplay.dmg_time, &crate::damage::TIME_NAMES),
            "gp_dmg_color" => Value::Choice(s.gameplay.dmg_color, &crate::damage::COLOR_NAMES),
            "gp_wp_color" | "gp_wp_edit_color" => Value::Choice(s.gameplay.wp_color, &crate::waypoints::COLOR_CHOICES),
            "gp_wp_quick" => Value::Text(&s.gameplay.wp_quick_name),
            "gp_th_cmd0" => Value::Text(&s.gameplay.th_cmd[0]),
            "gp_th_cmd1" => Value::Text(&s.gameplay.th_cmd[1]),
            "gp_th_cmd2" => Value::Text(&s.gameplay.th_cmd[2]),
            "gp_th_cmd3" => Value::Text(&s.gameplay.th_cmd[3]),
            _ => s.gameplay.toggle(id).map_or(Value::Button(None), Value::Toggle),
        },
        _ if id.starts_with("ch_") => crosshair_value(&s.crosshair, id),
        _ => {
            if let Some(option) = id.strip_prefix("key_") {
                return Value::Key(s.key(option));
            }
            if TOGGLES.iter().chain(LATER_TOGGLES.iter()).chain(MORE_TOGGLES.iter()).any(|(toggle, _)| *toggle == id) {
                return Value::Toggle(s.toggle(id));
            }
            Value::Button(None)
        }
    }
}

/// Applies a Crosshair page control. False when `id` is not one of them. Numbers are kept to
/// hundredths like every slider; `Settings::sanitize` holds them to their ranges afterwards.
fn crosshair_event(c: &mut CrosshairSettings, id: &str, value: f32, on: bool, index: usize) -> bool {
    let number = round2(f64::from(value));
    let colour = pick(index);
    let shape = index.min(SHAPES.len() - 1);
    match id {
        "ch_on" => c.enabled = on,
        "ch_third" => c.third_person = on,
        "ch_shape" => c.shape = shape,
        "ch_size" => c.size = on_pixels(number, c.shape, 1),
        "ch_gap" => c.gap = on_pixels(number, c.shape, 0),
        "ch_thickness" => c.thickness = on_pixels(number, c.shape, 1),
        "ch_dot" => c.dot = on,
        "ch_dot_round" => c.dot_round = on,
        "ch_color" => c.color = colour,
        "ch_opacity" => c.opacity = number,
        "ch_outline" => c.outline = on,
        "ch_outline_color" => c.outline_color = colour,
        "ch_outline_round" => c.outline_round = on,
        "ch_round" => c.round_lines = on,
        "ch_outline_width" => c.outline_width = on_pixels(number, 0, 1),
        "ch_target" => c.on_target = on,
        "ch_players" => c.players = on,
        "ch_mobs" => c.mobs = on,
        "ch_player_color" => c.player_color = colour,
        "ch_mob_color" => c.mob_color = colour,
        "ch_target_scale" => c.target_scale = number,
        "ch_target_opacity" => c.target_opacity = number,
        "ch_weapon" => c.weapon = on,
        "ch_weapon_shape" => c.weapon_shape = shape,
        "ch_weapon_size" => c.weapon_size = on_pixels(number, c.weapon_shape, 1),
        "ch_weapon_gap" => c.weapon_gap = on_pixels(number, c.weapon_shape, 0),
        "ch_weapon_thickness" => c.weapon_thickness = on_pixels(number, c.weapon_shape, 1),
        _ => return false,
    }
    true
}

/// One of the Crosshair page's values.
fn crosshair_value<'a>(c: &CrosshairSettings, id: &str) -> Value<'a> {
    match id {
        "ch_on" => Value::Toggle(c.enabled),
        "ch_third" => Value::Toggle(c.third_person),
        "ch_shape" => Value::Choice(c.shape, &SHAPES),
        "ch_size" => crosshair_slider(c.size, MIN_SIZE, MAX_SIZE),
        "ch_gap" => crosshair_slider(c.gap, 0.0, MAX_GAP),
        "ch_thickness" => crosshair_slider(c.thickness, MIN_THICKNESS, MAX_THICKNESS),
        "ch_dot" => Value::Toggle(c.dot),
        "ch_dot_round" => Value::Toggle(c.dot_round),
        "ch_color" => Value::Choice(c.color, &COLOR_CHOICES),
        "ch_opacity" => Value::Slider(c.opacity, 0.0, 100.0, SLIDER_STEP),
        "ch_outline" => Value::Toggle(c.outline),
        "ch_outline_width" => crosshair_slider(c.outline_width, MIN_OUTLINE_WIDTH, MAX_OUTLINE_WIDTH),
        "ch_round" => Value::Toggle(c.round_lines),
        "ch_outline_color" => Value::Choice(c.outline_color, &COLOR_CHOICES),
        "ch_outline_round" => Value::Toggle(c.outline_round),
        "ch_target" => Value::Toggle(c.on_target),
        "ch_players" => Value::Toggle(c.players),
        "ch_mobs" => Value::Toggle(c.mobs),
        "ch_player_color" => Value::Choice(c.player_color, &COLOR_CHOICES),
        "ch_mob_color" => Value::Choice(c.mob_color, &COLOR_CHOICES),
        "ch_target_scale" => Value::Slider(c.target_scale, MIN_TARGET_SCALE, MAX_TARGET_SCALE, SLIDER_STEP),
        "ch_target_opacity" => Value::Slider(c.target_opacity, 0.0, 100.0, SLIDER_STEP),
        "ch_weapon" => Value::Toggle(c.weapon),
        "ch_weapon_shape" => Value::Choice(c.weapon_shape, &SHAPES),
        "ch_weapon_size" => crosshair_slider(c.weapon_size, MIN_SIZE, MAX_SIZE),
        "ch_weapon_gap" => crosshair_slider(c.weapon_gap, 0.0, MAX_GAP),
        "ch_weapon_thickness" => crosshair_slider(c.weapon_thickness, MIN_THICKNESS, MAX_THICKNESS),
        _ => Value::Button(None),
    }
}

/// Applies a Zoom page control. False when `id` is not one of them.
fn zoom_event(settings: &mut Settings, id: &str, value: f32, on: bool, index: usize) -> bool {
    let z = &mut settings.zoom;
    let number = round2(f64::from(value));
    match id {
        "zoom_on" => z.enabled = on,
        "zoom_initial" => z.initial_zoom = number,
        "zoom_in_time" => z.zoom_in_time = number,
        "zoom_out_time" => z.zoom_out_time = number,
        "zoom_in_tr" => z.zoom_in_transition = transition_at(index),
        "zoom_out_tr" => z.zoom_out_transition = transition_at(index),
        "zoom_steps_on" => z.step_keys = on,
        "zoom_steps" => z.scroll_step_count = number.round().max(0.0) as u32,
        "zoom_per_step" => z.zoom_per_step = number,
        "zoom_smooth" => z.scroll_zoom_smoothness = number,
        "zoom_retain" => z.retain_zoom_steps = on,
        "zoom_sens" => z.relative_sensitivity = number,
        "zoom_hold" => z.key_mode = KeyMode::from_index(index),
        _ => return false,
    }
    true
}

/// One of the Zoom page's values. The key caps' own events open the capture (see `Bind`).
fn zoom_value<'a>(s: &'a Settings, state: &PanelState, id: &str) -> Value<'a> {
    let z = &s.zoom;
    match id {
        "zoom_on" => Value::Toggle(z.enabled),
        "zoom_initial" => Value::Slider(z.initial_zoom, MIN_DIVISOR, MAX_DIVISOR, SLIDER_STEP),
        "zoom_in_time" => Value::Slider(z.zoom_in_time, 0.0, MAX_TIME, SLIDER_STEP),
        "zoom_out_time" => Value::Slider(z.zoom_out_time, 0.0, MAX_TIME, SLIDER_STEP),
        "zoom_in_tr" => Value::Choice(transition_index(z.zoom_in_transition), &PANEL_LABELS),
        "zoom_out_tr" => Value::Choice(transition_index(z.zoom_out_transition), &PANEL_LABELS),
        "zoom_steps_on" => Value::Toggle(z.step_keys),
        "zoom_steps" => Value::Slider(f64::from(z.scroll_step_count), f64::from(MIN_STEPS), f64::from(MAX_STEPS), 1.0),
        "zoom_per_step" => Value::Slider(z.zoom_per_step, MIN_PER_STEP, MAX_PER_STEP, SLIDER_STEP),
        "zoom_smooth" => Value::Slider(z.scroll_zoom_smoothness, 0.0, 100.0, SLIDER_STEP),
        "zoom_retain" => Value::Toggle(z.retain_zoom_steps),
        "zoom_sens" => Value::Slider(z.relative_sensitivity, 0.0, 100.0, SLIDER_STEP),
        "zoom_hold" => Value::Choice(z.key_mode.index(), &KeyMode::LABELS),
        "zoom_p_default" => Value::Button(Some(state.label("zoom_p_default", "Default"))),
        "zoom_p_optifine" => Value::Button(Some(state.label("zoom_p_optifine", "OptiFine"))),
        "zoom_p_ok" => Value::Button(Some(state.label("zoom_p_ok", "OK Zoomer"))),
        _ => match ZoomKey::from_id(id) {
            Some(key) => Value::Key(z.keys.get(key)),
            None => Value::Button(None),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PALETTE;
    use serde_json::Value;

    /// Picks element `i` the way the dropdowns do.
    fn pick(state: &mut PanelState, settings: &mut Settings, i: usize) -> Effect {
        let (group, at) = Element::from_index(i).group_position();
        state.event(settings, &format!("element_{group}"), at as f32)
    }

    #[test]
    fn the_menu_lists_every_option_with_its_key() {
        let json: Value = serde_json::from_str(&build(&Settings::default(), &PanelState::default())).unwrap();
        // The host rejects a screen with a field it does not know: only these may be sent.
        let allowed = ["surface", "reference_size", "controls", "widget_colors", "capture_key", "show_gameplay_hud", "motion"];
        for key in json.as_object().unwrap().keys() {
            assert!(allowed.contains(&key.as_str()), "unknown field {key}");
        }
        let controls = json["controls"].as_array().unwrap();
        let ids: Vec<&str> = controls.iter().map(|c| c["id"].as_str().unwrap()).collect();
        for (id, _) in TOGGLES.iter().chain(LATER_TOGGLES.iter()).copied().filter(|(id, _)| *id != "hud_arrows" && *id != "hide_scoreboard") {
            assert!(ids.contains(&id), "{id}");
            assert!(ids.contains(&format!("key_{id}").as_str()), "key of {id}");
        }
        for id in [
            "grid", "hitbox_color", "hitbox_player_color", "hit_mode", "hit_color", "hit_opacity", "line_width", "scale", "stretch_x", "stretch_y", "pos_x", "pos_y", "opacity", "bg_color", "text_color",
            "pressed_color", "pressed_text_color", "labels", "smart", "rounded", "background", "shadow", "brackets", "reverse", "auto_width", "active_color", "show_chunk", "show_inchunk", "show_sub", "sub_range", "show_mouse", "show_space", "cps_mode", "align", "guides", "item_delay_fix",
            "box_border", "border_width", "border_color", "border_opacity", "bar_color", "chroma_on", "chroma_speed", "chroma_type",
            "speed_unit", "speed_decimals", "speed_vertical", "facing_text", "clock_12h", "clock_seconds", "stopwatch_keep", "playtime_format", "playtime_world", "counter_item", "reach_hide_zero", "reach_highlight", "reach_highlight_color", "sat_overlay", "sat_hunger", "sat_potential", "sat_inverse", "sat_color", "sat_layout", "potion_mode", "potion_horizontal", "potion_per_line", "potion_name", "potion_level", "potion_duration", "potion_formatted", "potion_upper", "potion_blink", "potion_color_name", "potion_color_info", "potion_icons", "potion_hide_vanilla", "potion_split", "key_sw_pause", "key_sw_reset",
            "menu_key", "move", "move_done", "move_reset", "move_guides", "reset_layout", "tab_0", "tab_4", "element", "armor_horizontal", "armor_held", "armor_helmet", "armor_chest", "armor_legs", "armor_boots", "armor_split", "armor_good",
            "zoom_on", "zoom_initial", "zoom_in_time", "zoom_out_time", "zoom_in_tr", "zoom_out_tr", "zoom_steps_on", "zoom_steps", "zoom_per_step",
            "zoom_smooth", "zoom_retain", "zoom_sens", "zoom_hold", "zoom_key", "zoom_step_in", "zoom_step_out", "zoom_p_default", "zoom_p_optifine", "zoom_p_ok",
            "ch_on", "ch_third", "ch_shape", "ch_size", "ch_gap", "ch_thickness", "ch_dot", "ch_dot_round", "ch_color", "ch_opacity", "ch_outline", "ch_outline_color", "ch_outline_width", "ch_round", "ch_outline_round", "ch_target", "ch_players", "ch_mobs",
            "ch_player_color", "ch_mob_color", "ch_target_scale", "ch_target_opacity", "ch_weapon", "ch_weapon_shape", "ch_weapon_size", "ch_weapon_gap", "ch_weapon_thickness",
            "cfg_slot_0", "cfg_slot_4", "cfg_save", "cfg_load", "cfg_clear", "cfg_undo", "cfg_export", "cfg_name", "cfg_out", "cfg_in", "cfg_info",
        ] {
            assert!(ids.contains(&id), "{id}");
        }
        let field = controls.iter().find(|c| c["id"] == "cfg_in").unwrap();
        assert_eq!((field["kind"].as_str(), field["max_chars"].as_u64()), (Some("text"), Some(256)));
        let key = controls.iter().find(|c| c["id"] == "key_hitboxes").unwrap();
        assert_eq!((key["kind"].as_str(), key["key"].as_str()), (Some("keybind"), Some("None")));
        let patch: Value = serde_json::from_str(&patch(&Settings::default(), &PanelState::default())).unwrap();
        assert!(patch["controls"].as_array().unwrap().iter().all(|c| c["id"] != "tab_0"));
        if std::env::var_os("TECHNIC_DUMP").is_some() {
            std::fs::write("/home/claude/technic-panel.json", build(&Settings::default(), &PanelState::default())).unwrap();
        }
    }

    #[test]
    fn key_ids_follow_the_toggles() {
        for (i, (id, _)) in TOGGLES.iter().enumerate() {
            assert_eq!(KEY_IDS[i], format!("key_{id}"));
            assert_eq!(Bind::from_id(KEY_IDS[i]), Some(Bind::Option(i)));
        }
        // The stopwatch's actions come after the options.
        for (n, (id, _)) in crate::config::ACTION_KEYS.iter().enumerate() {
            let i = TOGGLES.len() + n;
            assert_eq!(KEY_IDS[i], format!("key_{id}"));
            assert_eq!(Bind::from_id(KEY_IDS[i]), Some(Bind::Option(i)));
        }
        // The elements added later come after the actions.
        for (n, (id, _)) in LATER_TOGGLES.iter().enumerate() {
            let i = TOGGLES.len() + crate::config::ACTION_KEYS.len() + n;
            assert_eq!(KEY_IDS[i], format!("key_{id}"));
            assert_eq!(Bind::from_id(KEY_IDS[i]), Some(Bind::Option(i)));
        }
        for (n, (id, _)) in MORE_TOGGLES.iter().enumerate() {
            let i = TOGGLES.len() + crate::config::ACTION_KEYS.len() + LATER_TOGGLES.len() + n;
            assert_eq!(KEY_IDS[i], format!("key_{id}"));
            assert_eq!(Bind::from_id(KEY_IDS[i]), Some(Bind::Option(i)));
        }
        assert_eq!(crate::config::PALETTE_NAMES.len(), PALETTE.len());
    }

    #[test]
    fn sizes_go_to_two_decimals() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        for (sent, saved) in [(1.3_f32, 1.3), (1.07, 1.07), (0.999, 1.0), (2.346, 2.35), (4.9999, 5.0), (1.2345, 1.23)] {
            state.event(&mut settings, "scale", sent);
            assert_eq!(settings.style(state.element()).scale, saved, "sent {sent}");
        }
        // The slider moves in hundredths, over the whole range.
        let menu::Value::Slider(_, min, max, step) = value(&settings, &state, "scale") else { panic!("not a slider") };
        assert_eq!((min, max, step), (MIN_SCALE, MAX_SCALE, 0.01));
        // Out of range is clamped, not rounded past the ends.
        state.event(&mut settings, "scale", 99.0);
        assert_eq!(settings.style(state.element()).scale, MAX_SCALE);
    }

    #[test]
    fn every_slider_moves_in_hundredths_and_keeps_typed_values_clean() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        for id in ["line_width", "pos_x", "pos_y", "opacity", "border_opacity", "armor_gap", "zoom_initial", "zoom_in_time", "zoom_out_time",
                   "zoom_per_step", "zoom_smooth", "zoom_sens"] {
            let menu::Value::Slider(_, _, _, step) = value(&settings, &state, id) else {
                panic!("{id} is not a slider")
            };
            assert_eq!(step, 0.01, "{id}");
        }
        // A typed 37.3 arrives as 37.299999 and is stored as 37.3.
        state.event(&mut settings, "opacity", 37.3_f32);
        assert_eq!(settings.style(state.element()).opacity, 37.3);
        state.event(&mut settings, "line_width", 1.15_f32);
        assert_eq!(settings.line_width, 1.15);
        state.event(&mut settings, "zoom_sens", 12.34_f32);
        assert_eq!(settings.zoom.relative_sensitivity, 12.34);
    }

    #[test]
    fn each_element_is_edited_on_its_own() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        pick(&mut state, &mut settings, 2);
        assert_eq!(state.element(), Element::Fps);
        assert!(state.event(&mut settings, "hud_fps", 1.0).save);
        state.event(&mut settings, "scale", 2.0);
        state.event(&mut settings, "text_color", 5.0);
        state.event(&mut settings, "opacity", 20.0);
        state.event(&mut settings, "labels", 0.0);
        state.event(&mut settings, "pos_x", 40.0);
        let fps = settings.style(Element::Fps);
        assert!(fps.enabled && fps.scale == 2.0 && fps.text_color == 5 && fps.opacity == 20.0 && !fps.labels);
        assert_eq!(fps.position.map(|p| p[0]), Some(0.4));
        let keys = settings.style(Element::Keystrokes);
        assert!(keys.scale == 1.0 && keys.text_color == 0 && keys.opacity == 50.0, "the others are untouched");
        // Presets say Done for the element they changed.
        state.event(&mut settings, "bottom_right", 1.0);
        assert_eq!(settings.style(Element::Fps).corner, Corner::BottomRight);
        assert_eq!(settings.style(Element::Fps).position, None);
        assert_eq!(state.take_notice().as_deref(), Some("FPS: moved to the bottom right. Done!"));
        assert!(build(&settings, &state).contains("Done!"));
        assert!(state.tick(1.0) == Effect::default());
        assert!(state.tick(1.5).republish);
    }

    #[test]
    fn the_picker_dropdowns_choose_the_element() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        assert!(pick(&mut state, &mut settings, 5).republish);
        assert_eq!(state.element(), Element::Chunk);
        // A place that is not there picks nothing.
        assert_eq!(state.event(&mut settings, "element_1", 9.0), Effect::default());
        assert_eq!(state.event(&mut settings, "element_7", 0.0), Effect::default());
        // The group dropdown jumps to the first element of its group.
        state.event(&mut settings, "element_group", 0.0);
        assert_eq!(state.element(), Element::Keystrokes);
        state.event(&mut settings, "element_group", 1.0);
        assert_eq!(state.element(), Element::Coords);
        pick(&mut state, &mut settings, 5);
        assert_eq!(state.element(), Element::Chunk);
        // Picking changes nothing but the editor's target.
        assert!(!pick(&mut state, &mut settings, 1).save);
        assert_eq!(state.element(), Element::Cps);
        // The patch shows the open page and which element dropdown is on view.
        let p = patch(&settings, &state);
        assert!(p.contains("\"#grp_0\":true") && p.contains("\"#grp_1\":false"), "{p}");
        assert!(p.contains("\"#tab_0_on\":true") && p.contains("\"#tab_1_on\":false"), "{p}");
        // Each element's own options show for that element only.
        assert!(p.contains("\"#sec_counter\":true") && p.contains("\"#sec_cps\":true"), "{p}");
        assert!(p.contains("\"#sec_keys\":false") && p.contains("\"#sec_other\":true"), "{p}");
        pick(&mut state, &mut settings, 2);
        let fps = patch(&settings, &state);
        assert!(fps.contains("\"#sec_counter\":true") && fps.contains("\"#sec_cps\":false"), "{fps}");
        pick(&mut state, &mut settings, 0);
        let keys = patch(&settings, &state);
        assert!(keys.contains("\"#sec_keys\":true") && keys.contains("\"#sec_counter\":false"), "{keys}");
        // The border's rows show only while the edited element's border is on, and that follows the element.
        pick(&mut state, &mut settings, 1);
        assert!(patch(&settings, &state).contains("\"#border_on\":false,\"#border_off\":true"));
        state.event(&mut settings, "box_border", 1.0);
        assert!(settings.style(Element::Cps).border);
        assert!(patch(&settings, &state).contains("\"#border_on\":true,\"#border_off\":false"));
        pick(&mut state, &mut settings, 2);
        assert!(patch(&settings, &state).contains("\"#border_on\":false"), "FPS has its own border switch");
        pick(&mut state, &mut settings, 0);
        let keys = patch(&settings, &state);
        assert!(keys.contains("\"#sec_border\":false") && keys.contains("\"#sec_border_keys\":true"), "Keystrokes has its own placement of the card");
        assert!(keys.contains("\"#sec_other\":false"), "{keys}");
        pick(&mut state, &mut settings, 3);
        assert!(patch(&settings, &state).contains("\"#sec_keys\":false"));
        pick(&mut state, &mut settings, 1);
        // Every element can be reached through its group's dropdown, and the dropdowns show it.
        for (g, group) in Element::GROUPS.iter().enumerate() {
            assert!(group.elements.len() <= 8 && group.elements.len() == group.labels.len());
            for (at, element) in group.elements.iter().enumerate() {
                assert_eq!(element.group_position(), (g, at));
                state.event(&mut settings, &format!("element_{g}"), at as f32);
                assert_eq!(state.element(), *element);
            }
        }
        assert_eq!(Element::GROUPS.iter().map(|g| g.elements.len()).sum::<usize>(), Element::COUNT - 2, "all but the retired Arrow counter and Waypoints");
        // Armor status shows its own cards, and the usual ones go.
        pick(&mut state, &mut settings, Element::Armor.index());
        let armor = patch(&settings, &state);
        assert!(armor.contains("\"#sec_armor\":true"), "{armor}");
        // On the Style page it also swaps the document; on the other pages it does not.
        let style = crate::menu_controls::TABS.iter().position(|t| *t == "Style").unwrap();
        state.event(&mut settings, &format!("tab_{style}"), 1.0);
        state.take_tab_changed();
        pick(&mut state, &mut settings, 0);
        assert!(state.take_tab_changed(), "leaving Armor status on Style sends the usual document");
        pick(&mut state, &mut settings, Element::Armor.index());
        assert!(state.take_tab_changed());
        assert_ne!(menu::document_index(style, Element::Armor.index()), menu::document_index(style, 0));
        assert_eq!(menu::document_index(0, Element::Armor.index()), 0);
    }

    #[test]
    fn the_style_page_sends_only_the_cards_of_the_element_edited() {
        let documents: Vec<String> = (0..crate::menu_controls::DOC_LENS.len()).map(menu::document_text).collect();
        let style = crate::menu_controls::TABS.iter().position(|t| *t == "Style").unwrap();
        let doc = |element: Element| menu::document_index(style, element.index());
        // Elements that share their cards share a document; the others differ.
        assert_eq!(doc(Element::Cps), doc(Element::Fps));
        assert_eq!(doc(Element::Reach), doc(Element::Food));
        assert_eq!(doc(Element::Speed), doc(Element::Playtime));
        assert_eq!(doc(Element::Coords), doc(Element::Day), "plain elements have the page's own document");
        assert_eq!(doc(Element::Coords), style);
        let distinct: std::collections::BTreeSet<usize> = Element::ALL.iter().map(|e| doc(*e)).collect();
        assert_eq!(
            distinct.len(),
            9,
            "page, Keystrokes, counters, timers, Reach and Saturation, Potion status, Array list, Item counter, Armor status: {distinct:?}"
        );
        assert_eq!(doc(Element::Inventory), style, "Inventory HUD has only the page's Border card");
        assert!(documents[doc(Element::ItemCounter)].contains("Item counter") && documents[doc(Element::ArrayList)].contains("Brackets [ ]"));
        // Each document holds its element's cards and not the others'.
        let has = |element: Element, text: &str| documents[doc(element)].contains(text);
        assert!(has(Element::Food, "Show saturation overlay") && has(Element::Food, "Hide 0 blocks"));
        assert!(!has(Element::Coords, "Show saturation overlay") && !has(Element::Speed, "Show saturation overlay"));
        assert!(has(Element::Speed, "Include vertical") && !has(Element::Food, "Include vertical"));
        assert!(has(Element::Cps, "Brackets [ ]") && !has(Element::Reach, "Include vertical"));
        // Every document stays inside what the host takes (192 KiB, escaped text is a bit bigger).
        for (n, text) in documents.iter().enumerate() {
            assert!(text.len() < 190_000, "document {n} is {} bytes", text.len());
        }
        // Switching to an element with another document asks for it; staying within one does not.
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.event(&mut settings, &format!("tab_{style}"), 1.0);
        state.take_tab_changed();
        pick(&mut state, &mut settings, Element::Reach.index());
        assert!(state.take_tab_changed());
        pick(&mut state, &mut settings, Element::Food.index());
        assert!(!state.take_tab_changed(), "Reach and Saturation share a document");
    }

    #[test]
    fn the_world_blurs_and_dims_behind_the_menu_except_while_moving() {
        let settings = Settings::default();
        let mut state = PanelState::default();
        let whole = build(&settings, &state);
        assert!(whole.contains("\"transitions\":{\"#world_blur\":{\"duration\":0.25,\"from\":0,\"exit\":0}"), "{whole}");
        assert!(whole.contains("\"#world_brightness\":{\"duration\":0.25,\"from\":1,\"exit\":1}"), "{whole}");
        let p = patch(&settings, &state);
        assert!(p.contains("\"#world_blur\":8") && p.contains("\"#world_brightness\":0.7"), "{p}");
        state.moving = true;
        let m = patch(&settings, &state);
        assert!(m.contains("\"#world_blur\":0") && m.contains("\"#world_brightness\":1,"), "{m}");
    }

    #[test]
    fn the_move_bar_fades_while_an_element_is_held() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.event(&mut settings, "move", 1.0);
        assert!(state.moving && !state.dragging);
        let whole = build(&settings, &state);
        assert!(whole.contains("\"#move_alpha\":{\"duration\""), "{whole}");
        assert!(patch(&settings, &state).contains("\"#move_alpha\":1"));
        // Held: the bar is out of the way, and it is not there to be pressed.
        state.dragging = true;
        assert!(patch(&settings, &state).contains("\"#move_alpha\":0"));
        state.event(&mut settings, "move_done", 1.0);
        assert!(state.moving);
        // Let go: the bar is back, and Done ends the move.
        state.dragging = false;
        state.event(&mut settings, "move_done", 1.0);
        assert!(!state.moving);
    }

    #[test]
    fn every_element_can_be_turned_off_on_its_own() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        for element in Element::ALL {
            state.event(&mut settings, element.toggle_id(), 1.0);
            assert!(settings.is_on(element));
        }
        state.event(&mut settings, "hud_cps", 0.0);
        assert!(!settings.is_on(Element::Cps));
        assert_eq!(Element::ALL.iter().filter(|e| settings.is_on(**e)).count(), Element::COUNT - 1);
        state.event(&mut settings, "show_mouse", 0.0);
        state.event(&mut settings, "cps_mode", 3.0);
        assert!(!settings.show_mouse);
        assert_eq!(settings.cps_mode, CpsMode::Total);
    }

    #[test]
    fn the_zoom_page_edits_the_zoom() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        let on = state.event(&mut settings, "zoom_on", 1.0);
        assert!(on.save && on.rebind && settings.zoom.enabled, "the zoom's keys are reserved once it is on");
        state.event(&mut settings, "zoom_initial", 6.0);
        state.event(&mut settings, "zoom_in_tr", 1.0);
        state.event(&mut settings, "zoom_steps", 99.0);
        state.event(&mut settings, "zoom_hold", 1.0);
        let z = &settings.zoom;
        assert_eq!((z.initial_zoom, z.zoom_in_transition, z.scroll_step_count), (6.0, crate::transition::Transition::Linear, 20));
        assert_eq!(z.key_mode, KeyMode::Toggle);
        assert_eq!(state.event(&mut settings, "zoom_nope", 1.0), Effect::default());
        // A zoom key is rebound like any other, and takes the key from an option.
        settings.set_key("borders", "KeyG");
        state.event(&mut settings, "zoom_step_in", 1.0);
        assert!(state.is_capturing());
        assert!(state.capture(&mut settings, &["KeyG".into()]).rebind);
        assert_eq!((settings.zoom.keys.step_in.as_str(), settings.key("borders")), ("KeyG", ""));
        state.event(&mut settings, "zoom_step_in", 1.0);
        state.capture(&mut settings, &["Escape".into()]);
        assert_eq!(settings.zoom.keys.step_in, "");
        // The other way round: an option's key is taken from the zoom.
        state.event(&mut settings, "key_borders", 1.0);
        state.capture(&mut settings, &["KeyC".into()]);
        assert_eq!((settings.key("borders"), settings.zoom.keys.zoom.as_str()), ("KeyC", ""));
        // A preset keeps the switch and the keys, and says Done on its button.
        state.event(&mut settings, "zoom_p_optifine", 1.0);
        assert!(settings.zoom.enabled && !settings.zoom.step_keys);
        assert!(build(&settings, &state).contains("Done!"));
        assert!(state.take_notice().is_some());
        let turned_off = state.event(&mut settings, "zoom_on", 0.0);
        assert!(turned_off.rebind && settings.zoom.reserved_keys().is_empty());
    }

    #[test]
    fn the_crosshair_page_edits_the_crosshair() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        assert!(!settings.crosshair.enabled);
        let on = state.event(&mut settings, "ch_on", 1.0);
        assert!(on.save && on.republish && settings.crosshair.enabled);
        state.event(&mut settings, "ch_shape", 3.0);
        state.event(&mut settings, "ch_size", 7.5);
        state.event(&mut settings, "ch_gap", 99.0);
        state.event(&mut settings, "ch_color", 4.0);
        state.event(&mut settings, "ch_dot", 1.0);
        state.event(&mut settings, "ch_mobs", 0.0);
        state.event(&mut settings, "ch_weapon_shape", 9.0);
        let c = &settings.crosshair;
        assert_eq!((c.shape, c.size, c.color, c.dot, c.mobs), (3, 7.5, 4, true, false));
        assert_eq!(c.gap, MAX_GAP, "a number out of range is held to it");
        assert_eq!(c.weapon_shape, SHAPES.len() - 1);
        // The slider and dropdown values come back as set.
        let menu::Value::Slider(v, min, max, step) = value(&settings, &state, "ch_size") else { panic!("not a slider") };
        assert_eq!((v, min, max, step), (7.5, MIN_SIZE, MAX_SIZE, 0.01));
        assert!(matches!(value(&settings, &state, "ch_shape"), menu::Value::Choice(3, _)));
        assert!(matches!(value(&settings, &state, "ch_mobs"), menu::Value::Toggle(false)));
        // Typed numbers lose the single-precision tail, like every slider's.
        state.event(&mut settings, "ch_opacity", 37.3_f32);
        assert_eq!(settings.crosshair.opacity, 37.3);
        assert_eq!(state.event(&mut settings, "ch_nope", 1.0), Effect::default());
        // Nothing else was touched.
        assert_eq!(settings.zoom, Settings::default().zoom);
    }

    #[test]
    fn events_keys_and_tabs() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        assert!(state.event(&mut settings, "slime_chunks", 1.0).save);
        assert!(settings.slime_chunks);
        assert!(state.event(&mut settings, "hitboxes", 1.0).save && settings.hitboxes);
        state.event(&mut settings, "grid", 2.0);
        assert_eq!(settings.grid, Grid::Five);
        state.event(&mut settings, "tab_2", 1.0);
        assert_eq!(state.tab, 2);
        assert_eq!(state.event(&mut settings, "nope", 1.0), Effect::default());
        state.event(&mut settings, "move", 1.0);
        assert!(state.moving);
        state.event(&mut settings, "move_done", 1.0);
        assert!(!state.moving);
        // The menu key.
        state.event(&mut settings, "menu_key", 1.0);
        assert!(state.is_capturing());
        state.capture(&mut settings, &["KeyG".into()]);
        assert_eq!(settings.menu_key, "KeyG");
        // An option's key, then cleared with Escape.
        state.event(&mut settings, "key_borders", 1.0);
        assert!(state.capture(&mut settings, &["MouseLeft".into()]) == Effect::default(), "clicks do not bind");
        assert!(state.capture(&mut settings, &["KeyB".into()]).rebind);
        assert_eq!(settings.key("borders"), "KeyB");
        assert_eq!(PanelState::option_for_key(&settings, "KeyB"), Some("borders"));
        // Another option takes the key over.
        state.event(&mut settings, "key_hud_fps", 1.0);
        state.capture(&mut settings, &["KeyB".into()]);
        assert_eq!(settings.key("borders"), "");
        assert_eq!(PanelState::option_for_key(&settings, "KeyB"), Some("hud_fps"));
        state.event(&mut settings, "key_hud_fps", 1.0);
        state.capture(&mut settings, &["Escape".into()]);
        assert_eq!(settings.key("hud_fps"), "");
        assert_eq!(PanelState::option_for_key(&settings, "KeyB"), None);
    }

    #[test]
    fn the_stopwatch_keys_and_the_speed_and_playtime_options_come_from_the_menu() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        // Start / pause and reset are bound like the option keys, and a key does one job.
        state.event(&mut settings, "key_sw_pause", 1.0);
        assert!(state.is_capturing());
        assert!(state.capture(&mut settings, &["KeyP".into()]).rebind);
        assert_eq!(settings.key("sw_pause"), "KeyP");
        state.event(&mut settings, "key_sw_reset", 1.0);
        state.capture(&mut settings, &["KeyP".into()]);
        assert_eq!((settings.key("sw_pause"), settings.key("sw_reset")), ("", "KeyP"));
        assert_eq!(PanelState::option_for_key(&settings, "KeyP"), None, "an action key switches no option");
        state.event(&mut settings, "key_sw_reset", 1.0);
        state.capture(&mut settings, &["Escape".into()]);
        assert_eq!(settings.key("sw_reset"), "");
        // A dropdown opens its list, and a row of the list chooses.
        state.event(&mut settings, "speed_unit", 0.0);
        state.event(&mut settings, "dd_opt1", 1.0);
        assert_eq!(settings.speed_unit, 1);
        state.event(&mut settings, "speed_decimals", 0.0);
        state.event(&mut settings, "dd_opt3", 1.0);
        assert_eq!(settings.speed_decimals, 3);
        state.event(&mut settings, "playtime_format", 0.0);
        state.event(&mut settings, "dd_opt1", 1.0);
        assert_eq!(settings.playtime_format, 1);
        // The switches.
        for id in ["speed_vertical", "stopwatch_keep", "playtime_world"] {
            assert!(state.event(&mut settings, id, 1.0).save, "{id}");
        }
        assert!(settings.speed_vertical && settings.stopwatch_keep && settings.playtime_world_only);
        let menu::Value::Choice(at, options) = value(&settings, &state, "speed_unit") else { panic!("not a dropdown") };
        assert_eq!((at, options[at]), (1, "km/h"));
        let menu::Value::Key(key) = value(&settings, &state, "key_sw_pause") else { panic!("not a key cap") };
        assert_eq!(key, "");
    }

    #[test]
    fn a_reset_puts_separate_armor_pieces_back() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.selected = Element::Armor.index();
        {
            let style = settings.style_mut(Element::Armor);
            crate::hud::armor_split_from_layout(style);
            style.armor_split = true;
        }
        let laid_out = settings.style(Element::Armor).armor_offsets;
        {
            let style = settings.style_mut(Element::Armor);
            style.armor_offsets[1] = [40.0, 30.0];
            style.armor_sizes[1] = [2.0, 1.5];
            style.position = Some([0.5, 0.5]);
        }
        state.event(&mut settings, "reset_layout", 1.0);
        state.event(&mut settings, "cf_yes", 1.0);
        let style = settings.style(Element::Armor);
        assert_eq!(style.armor_offsets, laid_out);
        assert_eq!(style.armor_sizes, [[1.0; 2]; 5]);
        assert_eq!(style.position, Settings::default().style(Element::Armor).position);
        assert!(style.armor_split, "the pieces stay separate");
    }

    #[test]
    fn a_typed_number_reaches_the_slider_and_a_reset_asks_first() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.event(&mut settings, "tab_1", 1.0);
        state.pointer = Some((300.0, 100.0));
        state.event(&mut settings, "edit_scale", 1.0);
        assert!(state.is_capturing(), "the editor takes the keys");
        for key in ["Digit2", "Period", "Digit5"] {
            state.typing(&mut settings, &[key.to_string()]);
        }
        assert!(state.typing(&mut settings, &["Enter".to_string()]).save);
        assert_eq!(settings.style(state.element()).scale, 2.5);
        assert!(!state.is_capturing());
        // A reset waits for the confirmation.
        let before = settings.clone();
        state.event(&mut settings, "reset_layout", 1.0);
        assert!(settings == before, "nothing changes until it is confirmed");
        state.event(&mut settings, "cf_yes", 1.0);
        assert_eq!(settings.style(state.element()).scale, Settings::default().style(state.element()).scale);
        // Escape leaves a number as it was.
        state.event(&mut settings, "edit_scale", 1.0);
        state.typing(&mut settings, &["Digit9".to_string()]);
        state.typing(&mut settings, &["Escape".to_string()]);
        assert!(!state.is_capturing());
        assert_eq!(settings.style(state.element()).scale, Settings::default().style(state.element()).scale);
    }

    #[test]
    fn a_hex_colour_is_typed_in_the_editor_and_kept() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.event(&mut settings, "tab_5", 1.0);
        state.pointer = Some((300.0, 100.0));
        // The dropdown's last row opens the editor instead of choosing a colour.
        let before = settings.crosshair.color;
        let mut choose = |state: &mut PanelState, settings: &mut Settings, row: usize| {
            state.event(settings, "ch_color", 1.0);
            state.event(settings, &format!("dd_opt{row}"), 1.0);
        };
        choose(&mut state, &mut settings, CUSTOM_ROW);
        assert!(state.is_capturing(), "the hex editor takes the keys");
        assert_eq!(settings.crosshair.color, before);
        for key in ["KeyF", "Digit0", "KeyA", "Digit1", "KeyB", "Digit2"] {
            state.typing(&mut settings, &[key.to_string()]);
        }
        state.typing(&mut settings, &["KeyC".to_string()]);
        state.typing(&mut settings, &["Enter".to_string()]);
        assert_eq!(crate::config::color_rgb(settings.crosshair.color), 0xF0A1B2);
        assert!(!state.is_capturing());
        // A palette colour replaces it again, and a bad entry changes nothing.
        let custom = settings.crosshair.color;
        choose(&mut state, &mut settings, CUSTOM_ROW);
        state.typing(&mut settings, &["Backspace".to_string()]);
        state.typing(&mut settings, &["Digit1".to_string()]);
        state.typing(&mut settings, &["Digit2".to_string()]);
        state.typing(&mut settings, &["Enter".to_string()]);
        assert_eq!(settings.crosshair.color, custom, "two digits are not a colour");
        choose(&mut state, &mut settings, 5);
        assert_eq!(settings.crosshair.color, 5);
        // Black is a custom colour too, not the dropdown row.
        choose(&mut state, &mut settings, CUSTOM_ROW);
        for key in ["Digit0", "Digit0", "Digit0"] {
            state.typing(&mut settings, &[key.to_string()]);
        }
        state.typing(&mut settings, &["Enter".to_string()]);
        assert_eq!(crate::config::color_rgb(settings.crosshair.color), 0);
        assert!(crate::config::custom_rgb(settings.crosshair.color).is_some());
        settings.sanitize();
        assert!(crate::config::custom_rgb(settings.crosshair.color).is_some(), "a saved custom colour survives repair");
    }

    #[test]
    fn a_pasted_hex_code_reaches_the_colour() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.event(&mut settings, "tab_5", 1.0);
        state.pointer = Some((300.0, 100.0));
        let mut choose = |state: &mut PanelState, settings: &mut Settings, row: usize| {
            state.event(settings, "ch_color", 1.0);
            state.event(settings, &format!("dd_opt{row}"), 1.0);
        };
        // Pasted over the whole field, with a # in front.
        choose(&mut state, &mut settings, CUSTOM_ROW);
        assert!(state.hex_open());
        state.field(&mut settings, "hex_in", "#a1b2c3", true);
        state.typing(&mut settings, &["Enter".to_string()]);
        assert_eq!(crate::config::color_rgb(settings.crosshair.color), 0xA1B2C3);
        // Pasted at the end of the colour the field started with.
        choose(&mut state, &mut settings, CUSTOM_ROW);
        state.field(&mut settings, "hex_in", "A1B2C3 0F0F0F", true);
        state.typing(&mut settings, &["Enter".to_string()]);
        assert_eq!(crate::config::color_rgb(settings.crosshair.color), 0x0F0F0F);
        // While the field has the keyboard, key presses are its own; leaving it over Apply applies.
        choose(&mut state, &mut settings, CUSTOM_ROW);
        state.set_native(&mut settings, true);
        state.typing(&mut settings, &["KeyA".to_string(), "Backspace".to_string()]);
        state.field(&mut settings, "hex_in", "123456", true);
        state.pointer = Some((state.ed_pos[0] - state.ed_shift + menu::PICKER_W - 30.0, state.ed_pos[1] + menu::PICKER_H - 15.0));
        state.set_native(&mut settings, false);
        assert!(state.tick(0.0).save);
        assert_eq!(crate::config::color_rgb(settings.crosshair.color), 0x123456);
        assert!(!state.hex_open());
        // Leaving it anywhere else only takes the focus away.
        choose(&mut state, &mut settings, CUSTOM_ROW);
        state.set_native(&mut settings, true);
        state.field(&mut settings, "hex_in", "654321", true);
        state.pointer = Some((state.ed_pos[0] - state.ed_shift + 4.0, state.ed_pos[1] + 4.0));
        state.set_native(&mut settings, false);
        assert!(state.hex_open());
        assert_eq!(crate::config::color_rgb(settings.crosshair.color), 0x123456);
    }

    #[test]
    fn the_colour_picker_converts_and_follows_the_pointer() {
        for rgb in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.2, 0.4, 0.8], [1.0, 1.0, 1.0]] {
            let [h, s, v] = menu::rgb_hsv(rgb);
            let back = menu::hsv_rgb(h, s, v);
            for i in 0..3 {
                assert!((back[i] - rgb[i]).abs() < 0.01, "{rgb:?} -> {back:?}");
            }
        }
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.event(&mut settings, "tab_5", 1.0);
        state.pointer = Some((300.0, 100.0));
        state.event(&mut settings, "ch_color", 1.0);
        state.event(&mut settings, &format!("dd_opt{CUSTOM_ROW}"), 1.0);
        assert!(state.hex_open());
        let [x, y] = state.ed_pos;
        // A press at the square's top right is full saturation and brightness; a drag to the strip's middle moves the hue.
        let [sx, sy, sw, _] = menu::PICKER_SV;
        assert!(state.picker_frame(Some((x - state.ed_shift + sx + sw, y + sy)), true, true).republish);
        assert_eq!(&state.hsv[1..], &[1.0, 1.0]);
        let [hx, hy, hw, hh] = menu::PICKER_HUE;
        state.picker_frame(Some((x - state.ed_shift + hx + hw / 2.0, y + hy + hh / 2.0)), true, true);
        state.picker_frame(None, false, false);
        assert!((state.hsv[0] - 0.5).abs() < 0.01 && state.pk_drag == 0);
        assert_eq!(state.edit.as_ref().map(|e| e.text.as_str()), Some("00FFFF"));
    }

    #[test]
    fn the_colour_picker_sets_opacity_and_takes_presets() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.event(&mut settings, "tab_5", 1.0);
        state.pointer = Some((300.0, 100.0));
        state.event(&mut settings, "ch_color", 1.0);
        state.event(&mut settings, &format!("dd_opt{CUSTOM_ROW}"), 1.0);
        assert!(state.hex_open());
        assert_eq!(state.transparency, 0, "a colour starts opaque");
        // A preset fills the field; the opacity strip's middle is half transparent.
        state.event(&mut settings, "cp_p0", 1.0);
        assert_eq!(state.edit.as_ref().map(|e| e.text.as_str()), Some("C62828"));
        let [x, y] = state.ed_pos;
        let [ox, oy, ow, oh] = menu::PICKER_OP;
        assert!(state.picker_frame(Some((x - state.ed_shift + ox + ow / 2.0, y + oy + oh / 2.0)), true, true).republish);
        assert_eq!(state.transparency, 50);
        state.picker_frame(None, false, false);
        state.event(&mut settings, "ed_ok", 1.0);
        let stored = settings.crosshair.color;
        assert_eq!(crate::config::custom_rgb(stored), Some(0xC62828));
        assert_eq!(crate::config::custom_transparency(stored), 50);
        assert!((crate::config::rgba(stored, 1.0)[3] - 0.5).abs() < 0.001);
        // The element's own opacity multiplies with it.
        assert!((crate::config::rgba(stored, 0.5)[3] - 0.25).abs() < 0.001);
        // Opening it again starts from the stored opacity.
        state.event(&mut settings, "ch_color", 1.0);
        state.event(&mut settings, &format!("dd_opt{CUSTOM_ROW}"), 1.0);
        assert_eq!(state.transparency, 50);
    }

    #[test]
    fn the_mods_grid_searches_filters_sorts_and_keeps_favorites() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        let all = crate::menu_controls::MODS.len();
        assert_eq!(state.grid_order(&settings).len(), all);
        // Search: words match the card's name, every word must.
        state.field(&mut settings, "module_search", "chunk", true);
        let found = state.grid_order(&settings);
        assert!(!found.is_empty() && found.len() < all);
        assert!(found.iter().all(|n| crate::menu_controls::MODS[*n].1.contains("chunk") || crate::menu_controls::MODS[*n].0.contains("chunk")));
        state.field(&mut settings, "module_search", "zzzz", true);
        assert!(state.grid_order(&settings).is_empty());
        state.field(&mut settings, "module_search", "", true);
        // A star adds the card to Favorites and a second press takes it off; the Favorites chip shows only starred cards.
        let (id, ..) = crate::menu_controls::MODS[3];
        assert!(state.event(&mut settings, &format!("fav_{id}"), 1.0).save);
        assert_eq!(settings.favorites, vec![id.to_owned()]);
        state.event(&mut settings, "flt_5", 1.0);
        assert_eq!(state.grid_order(&settings), vec![3]);
        state.event(&mut settings, &format!("fav_{id}"), 0.0);
        assert!(settings.favorites.is_empty() && state.grid_order(&settings).is_empty());
        // The chips by category, and On.
        state.event(&mut settings, "flt_2", 1.0);
        assert!(state.grid_order(&settings).iter().all(|n| crate::menu_controls::MODS[*n].2 == 0));
        state.event(&mut settings, "flt_1", 1.0);
        let on = state.grid_order(&settings);
        assert_eq!(on.len(), state.grid_enabled(&settings));
        // Sort: A to Z orders by name; On first lists the mods that are on before the others.
        state.event(&mut settings, "flt_0", 1.0);
        state.event(&mut settings, "sort", 1.0);
        let names: Vec<&str> = state.grid_order(&settings).iter().map(|n| crate::menu_controls::MODS[*n].1).collect();
        assert!(names.windows(2).all(|w| w[0] <= w[1]));
        // The page sends the grid and its order, and patches change only the order.
        state.event(&mut settings, "tab_0", 1.0);
        let built: serde_json::Value = serde_json::from_str(&build(&settings, &state)).unwrap();
        assert_eq!(built["surface"]["collections"]["modules"]["order"].as_array().map(Vec::len), Some(all));
        state.field(&mut settings, "module_search", "zzzz", true);
        let patched: serde_json::Value = serde_json::from_str(&patch(&settings, &state)).unwrap();
        assert_eq!(patched["collection_order"]["modules"].as_array().map(Vec::len), Some(0));
        assert_eq!(patched["bindings"]["#empty"], true);
    }
}
