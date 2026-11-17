//! The gameplay mods that have no HUD element: sprint and sneak toggles, the time and weather
//! changers, the hurt camera, nametags and the inventory cursor. They are plain settings; the game
//! side (`guest.rs`) sends them to the host. They are in share codes (the last section, `code::put_last`; the colour picker's lists and the marked spots are not) (they change how the
//! game plays, not how the HUD looks).

use serde::{Deserialize, Serialize};

/// Local times of day for the Time changer: (name, game ticks).
pub const TIME_PRESETS: [(&str, u32); 6] =
    [("Sunrise", 23_000), ("Day", 1_000), ("Noon", 6_000), ("Sunset", 12_000), ("Night", 14_000), ("Midnight", 18_000)];
/// The names of `TIME_PRESETS`, for the menu's dropdown.
pub const TIME_NAMES: [&str; 6] = ["Sunrise", "Day", "Noon", "Sunset", "Night", "Midnight"];
/// How many recent and saved colours the picker shows each (`PICKER_SWATCHES` in tools/menukit.py).
pub const PICKER_SWATCHES: usize = 6;
/// The most the host blurs the world behind the menu (GUI pixels).
pub const MAX_BLUR: f64 = 32.0;
/// The weather the Weather changer shows.
pub const WEATHER_NAMES: [&str; 3] = ["Clear", "Rain", "Thunder"];
/// Nametag sizes: (name, scale).
pub const NAME_SCALES: [(&str, f32); 5] = [("75%", 0.75), ("100%", 1.0), ("125%", 1.25), ("150%", 1.5), ("200%", 2.0)];
pub const NAME_SCALE_NAMES: [&str; 5] = ["75%", "100%", "125%", "150%", "200%"];
/// Hurt camera shake strengths: (name, percent of the game's own shake).
pub const HURT_STRENGTHS: [(&str, u32); 6] = [("25%", 25), ("50%", 50), ("75%", 75), ("100%", 100), ("150%", 150), ("200%", 200)];
pub const HURT_STRENGTH_NAMES: [&str; 6] = ["25%", "50%", "75%", "100%", "150%", "200%"];

/// How long a marker stays: the dropdown's text and the seconds (0 keeps it until it is cleared).
pub const MARKER_TIMES: [(&str, f64); 6] = [("10 s", 10.0), ("20 s", 20.0), ("30 s", 30.0), ("1 min", 60.0), ("5 min", 300.0), ("Until cleared", 0.0)];
pub const MARKER_TIME_NAMES: [&str; 6] = ["10 s", "20 s", "30 s", "1 min", "5 min", "Until cleared"];
/// The command names the pack grants (`mods/technic/pack/grants.json`, "commands"): the host sends no other.
/// A name here must be in that file, and the file allows at most 8.
pub const HOTKEY_COMMANDS: [&str; 6] = ["spawn", "home", "hub", "tpa", "tpaccept", "warp"];

/// What a hotkey's text may be: kept to printable ASCII, 24 characters, starting with a slash.
pub fn clean_command(text: &str) -> String {
    text.chars().filter(|c| (' '..='~').contains(c)).take(24).collect()
}

/// The name of a command ("/home base" is "home"), when it is a granted one.
pub fn granted_name(command: &str) -> Option<&str> {
    let name = command.strip_prefix('/')?.split(' ').next()?;
    HOTKEY_COMMANDS.contains(&name).then_some(name)
}

/// The most markers at once (a new one replaces the oldest).
pub const MARKERS_MAX: usize = 8;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameplaySettings {
    /// Toggle sprint / toggle sneak: a key press switches the action on and it stays on.
    pub toggle_sprint: bool,
    pub toggle_sneak: bool,
    /// Always sprint: sprint whenever you move forward.
    pub always_sprint: bool,
    /// Time changer: show this local time of day (index into `TIME_PRESETS`) while the server's time runs on.
    pub time_on: bool,
    pub time_preset: usize,
    /// Weather changer: show this weather (index into `WEATHER_NAMES`).
    pub weather_on: bool,
    pub weather: usize,
    /// Hurt cam: no camera shake when you are hurt.
    pub no_hurt_cam: bool,
    /// Hurt cam options: remove the shake altogether (the old behaviour), else scale it (index into `HURT_STRENGTHS`)
    /// and optionally always tilt the same way.
    pub hurt_off: bool,
    pub hurt_strength: usize,
    pub hurt_fixed: bool,
    /// The colour picker's last colours (newest first) and the ones saved with Save, as 24-bit red, green, blue.
    pub recent_colors: Vec<u32>,
    pub favorite_colors: Vec<u32>,
    /// Hide the game's own position line while the Coordinates element is on (Cinnabar's `set-position-hidden`).
    pub hide_game_coords: bool,
    /// CPS statistics: add the average (while clicking), the highest and the session's total clicks to the CPS element.
    pub cps_avg: bool,
    pub cps_max: bool,
    pub cps_total: bool,
    /// Waypoints: the marked spots (all worlds; the element lists the ones of the world you are in).
    pub waypoints: Vec<crate::waypoints::Waypoint>,
    /// Waypoints: markers on the Direction compass, beams and distances in the world, hide them all, and the colour
    /// (index into `waypoints::COLORS`) a new one gets.
    pub wp_compass: bool,
    pub wp_world: bool,
    pub wp_hidden: bool,
    /// Handler mods: No cam reset, No swing sounds, and the CPS limiter with its limit (index into `handlers::CPS_LIMITS`).
    /// Block outline: the box around the block you point at; width, mode, colour (palette index) and opacity are
    /// line width in pixels (a slider, 1 to 8) and indexes into `MODE_NAMES`, the palette and `OPACITIES`.
    pub bo_on: bool,
    /// Markers: quick, short-lived marks on the block you aim at. The colour is an index into `waypoints::COLORS`, the
    /// time an index into `MARKER_TIMES`.
    pub mk_on: bool,
    /// Glass: on, and the style (index into `glass::GLASS_NAMES`).
    pub glass_on: bool,
    pub glass_mode: usize,
    /// The coloured glass section's style (index into `glass::GLASS_NAMES`).
    pub glass_coloured: usize,
    pub mk_color: usize,
    pub mk_time: usize,
    /// A middle click on a marked block takes the mark off, and the colour of the danger marks (an index into `waypoints::COLORS`).
    pub mk_middle: bool,
    /// TNT countdown: the seconds left over each lit TNT.
    pub tnt_on: bool,
    /// Auto Text Hotkey: four commands, each sent by its own key (`th_key0`..`th_key3`). Only slash commands whose
    /// name is granted in the pack's `grants.json` go through (see `HOTKEY_COMMANDS`).
    pub th_on: bool,
    pub th_cmd: [String; 4],
    pub mk_danger_color: usize,
    pub bo_line: f64,
    pub bo_mode: usize,
    pub bo_color: usize,
    pub bo_opacity: usize,
    /// The fill over the block's faces and the game-highlight fix (indexes into `outline::FILL_PERCENTS`, `FIX_PERCENTS`).
    pub bo_fill: usize,
    pub bo_fix: usize,
    pub nocam_on: bool,
    pub noswing_on: bool,
    pub cpslim_on: bool,
    pub cpslim_max: usize,
    /// Chroma, per HUD element (indexed by `Element::index`; a short list means the defaults): the text colour
    /// cycles through the rainbow. On or off, speed (1 to 100) and type (0 wave, 1 solid).
    pub chroma_on: Vec<bool>,
    pub chroma_speed: Vec<f64>,
    pub chroma_type: Vec<usize>,
    /// The Waypoints mod itself (its card in the Mods tab): off draws nothing, on or off the marks are kept.
    pub wp_on: bool,
    pub wp_color: usize,
    /// Waypoints, Lunar's settings: ask twice before deleting; the name of a quick waypoint; beams; box borders (the
    /// outline of the label); text shadow; the label's padding (0 to 8); label scale; the highlighted block and its line
    /// width; the distance in the label; show only the waypoints you look near.
    pub wp_confirm: bool,
    /// Mark where you die.
    pub wp_death: bool,
    pub wp_quick_name: String,
    pub wp_beams: bool,
    pub wp_borders: bool,
    pub wp_shadow: bool,
    pub wp_padding: f64,
    pub wp_label_scale: f64,
    pub wp_highlight: bool,
    pub wp_line: f64,
    pub wp_distance: bool,
    pub wp_near: bool,
    /// Menu blur: blur behind the menu, in pixels (0 to `MAX_BLUR`).
    pub blur_on: bool,
    pub blur_amount: f64,
    /// How much the world behind the menu is darkened (percent) and its color saturation (percent, 100 is unchanged).
    pub blur_dim: f64,
    pub blur_sat: f64,
    /// Hit color: the card's switch (off is the game's own flash; the colour itself is `Settings::hit_mode`) and whether
    /// the armor flashes too.
    pub hit_on: bool,
    pub hit_armor: bool,
    /// Damage Indicator: floating numbers over hurt players and mobs; size, time and colour are indexes into
    /// `damage::SIZES`, `TIMES` and `COLORS`; with `dmg_hearts` the numbers are hearts, not health points.
    pub dmg_on: bool,
    pub dmg_size: usize,
    pub dmg_time: usize,
    pub dmg_color: usize,
    pub dmg_hearts: bool,
    /// Nametags: restyle the name above players.
    pub nametags: bool,
    pub name_no_background: bool,
    pub name_scale: usize,
    pub name_shadow: bool,
    pub name_self: bool,
    /// Inventory cursor: put the cursor in the middle of the screen when an inventory opens.
    pub cursor_center: bool,
    /// Hive auto requeue (see `hive.rs`): the card's switch; what counts as the game being over for you (it ended,
    /// you or your team were eliminated, you died in a solo game with no respawns); asking `/connection` for the
    /// mode before queueing; going to the hub instead; and echoing every chat line read (to find a line's exact
    /// text). None of it travels in a share code.
    pub hive_on: bool,
    pub hive_end: bool,
    pub hive_elim: bool,
    pub hive_solo: bool,
    pub hive_check: bool,
    pub hive_hub: bool,
    pub hive_log: bool,
    /// Whether the requeue says what it does in chat ("Vantage Hive: ..."); a refused permission is always said.
    pub hive_say: bool,
    /// Perspective card (see `guest.rs::sync_game_keys`): the card's switch.
    pub persp_on: bool,
    /// The Scoreboard element: whether the server's header (title) shows, and whether the rows share one line.
    pub sb_title: bool,
    pub sb_horizontal: bool,
    /// Snaplook's Toggle Press Mode: a press switches the view on and off instead of holding the key.
    pub persp_snap_toggle: bool,
}

impl Default for GameplaySettings {
    fn default() -> Self {
        GameplaySettings {
            toggle_sprint: false,
            toggle_sneak: false,
            always_sprint: false,
            time_on: false,
            time_preset: 1,
            weather_on: false,
            weather: 0,
            no_hurt_cam: false,
            hurt_off: true,
            hurt_strength: 3,
            hurt_fixed: false,
            recent_colors: Vec::new(),
            favorite_colors: Vec::new(),
            hide_game_coords: true,
            cps_avg: false,
            cps_max: false,
            cps_total: false,
            waypoints: Vec::new(),
            wp_compass: true,
            wp_world: true,
            wp_hidden: false,
            bo_on: false,
            mk_on: false,
            glass_on: false,
            glass_mode: crate::glass::CONNECTED,
            glass_coloured: crate::glass::CONNECTED,
            mk_color: 0,
            mk_time: 1,
            mk_middle: true,
            tnt_on: false,
            th_on: false,
            th_cmd: ["/spawn".to_owned(), "/home".to_owned(), "/hub".to_owned(), "/tpaccept".to_owned()],
            mk_danger_color: 1,
            bo_line: 2.0,
            bo_mode: 0,
            bo_color: crate::config::BLACK,
            bo_opacity: 1,
            bo_fill: 0,
            bo_fix: 0,
            nocam_on: false,
            noswing_on: false,
            cpslim_on: false,
            cpslim_max: 3,
            chroma_on: Vec::new(),
            chroma_speed: Vec::new(),
            chroma_type: Vec::new(),
            wp_on: true,
            wp_color: 0,
            wp_confirm: false,
            wp_death: false,
            wp_quick_name: "Quick Waypoint".to_owned(),
            wp_beams: true,
            wp_borders: true,
            wp_shadow: false,
            wp_padding: 4.0,
            wp_label_scale: 1.0,
            wp_highlight: true,
            wp_line: 4.0,
            wp_distance: true,
            wp_near: false,
            blur_on: true,
            blur_amount: 8.0,
            blur_dim: 30.0,
            blur_sat: 100.0,
            hit_on: false,
            hit_armor: true,
            dmg_on: false,
            dmg_size: 2,
            dmg_time: 2,
            dmg_color: 0,
            dmg_hearts: false,
            nametags: false,
            name_no_background: false,
            name_scale: 1,
            name_shadow: true,
            name_self: false,
            cursor_center: false,
            hive_on: false,
            hive_end: true,
            hive_elim: true,
            hive_solo: true,
            hive_check: true,
            hive_hub: false,
            hive_log: false,
            hive_say: true,
            persp_on: false,
            sb_title: true,
            sb_horizontal: false,
            persp_snap_toggle: false,
        }
    }
}

/// The Waypoints settings the world drawing reads, bounded.
#[derive(Clone, Copy, Debug)]
pub struct WpLook {
    pub beams: bool,
    pub borders: bool,
    pub shadow: bool,
    pub padding: usize,
    pub scale: f32,
    pub highlight: bool,
    /// The box's line width in pixels.
    pub line: f32,
    pub distance: bool,
    pub near: bool,
}

/// Chroma's types, for the menu's dropdown.
pub const CHROMA_TYPE_NAMES: [&str; 2] = ["Wave", "Solid"];
pub const CHROMA_SPEED: (f64, f64, f64) = (1.0, 100.0, 40.0);

/// One element's chroma.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chroma {
    pub on: bool,
    pub speed: f64,
    pub wave: bool,
}

impl GameplaySettings {
    /// The chroma of the element with this index.
    pub fn chroma(&self, element: usize) -> Chroma {
        let speed = self.chroma_speed.get(element).copied().filter(|v| v.is_finite()).unwrap_or(CHROMA_SPEED.2);
        Chroma {
            on: self.chroma_on.get(element).copied().unwrap_or(false),
            speed: speed.clamp(CHROMA_SPEED.0, CHROMA_SPEED.1),
            wave: self.chroma_type.get(element).copied().unwrap_or(0) == 0,
        }
    }

    /// Whether any element has chroma on.
    pub fn any_chroma(&self) -> bool {
        self.chroma_on.iter().any(|on| *on)
    }

    fn chroma_room(&mut self, element: usize) {
        if self.chroma_on.len() <= element {
            self.chroma_on.resize(element + 1, false);
        }
        if self.chroma_speed.len() <= element {
            self.chroma_speed.resize(element + 1, CHROMA_SPEED.2);
        }
        if self.chroma_type.len() <= element {
            self.chroma_type.resize(element + 1, 0);
        }
    }

    pub fn set_chroma_on(&mut self, element: usize, on: bool) {
        self.chroma_room(element);
        self.chroma_on[element] = on;
    }

    pub fn set_chroma_speed(&mut self, element: usize, speed: f64) {
        self.chroma_room(element);
        self.chroma_speed[element] = speed.round().clamp(CHROMA_SPEED.0, CHROMA_SPEED.1);
    }

    pub fn set_chroma_type(&mut self, element: usize, kind: usize) {
        self.chroma_room(element);
        self.chroma_type[element] = kind.min(CHROMA_TYPE_NAMES.len() - 1);
    }

    /// The look of the waypoints in the world: Lunar's 4.00 line width is 1.5 pixels here.
    pub fn clone_wp_look(&self) -> WpLook {
        WpLook {
            beams: self.wp_beams,
            borders: self.wp_borders,
            shadow: self.wp_shadow,
            padding: self.wp_padding.round().clamp(2.0, 8.0) as usize,
            scale: self.wp_label_scale.clamp(0.5, 3.0) as f32,
            highlight: self.wp_highlight,
            line: (self.wp_line.clamp(1.0, 8.0) * 0.375) as f32,
            distance: self.wp_distance,
            near: self.wp_near,
        }
    }

    /// A switch by its menu id.
    pub fn toggle_mut(&mut self, id: &str) -> Option<&mut bool> {
        Some(match id {
            "gp_toggle_sprint" => &mut self.toggle_sprint,
            "gp_toggle_sneak" => &mut self.toggle_sneak,
            "gp_always_sprint" => &mut self.always_sprint,
            "gp_time_on" => &mut self.time_on,
            "gp_weather_on" => &mut self.weather_on,
            "gp_no_hurt_cam" => &mut self.no_hurt_cam,
            "gp_hurt_off" => &mut self.hurt_off,
            "gp_hurt_fixed" => &mut self.hurt_fixed,
            "gp_hide_coords" => &mut self.hide_game_coords,
            "gp_cps_avg" => &mut self.cps_avg,
            "gp_cps_max" => &mut self.cps_max,
            "gp_cps_total" => &mut self.cps_total,
            "gp_wp_compass" => &mut self.wp_compass,
            "gp_wp_world" => &mut self.wp_world,
            "gp_wp_hidden" => &mut self.wp_hidden,
            "gp_nocam_on" => &mut self.nocam_on,
            "gp_bo_on" => &mut self.bo_on,
            "gp_mk_on" => &mut self.mk_on,
            "gp_mk_middle" => &mut self.mk_middle,
            "gp_tnt_on" => &mut self.tnt_on,
            "gp_th_on" => &mut self.th_on,
            "gp_noswing_on" => &mut self.noswing_on,
            "gp_cpslim_on" => &mut self.cpslim_on,
            "gp_wp_on" => &mut self.wp_on,
            "gp_wp_confirm" => &mut self.wp_confirm,
            "gp_wp_death" => &mut self.wp_death,
            "gp_wp_beams" => &mut self.wp_beams,
            "gp_wp_borders" => &mut self.wp_borders,
            "gp_wp_shadow" => &mut self.wp_shadow,
            "gp_wp_highlight" => &mut self.wp_highlight,
            "gp_wp_distance" => &mut self.wp_distance,
            "gp_wp_near" => &mut self.wp_near,
            "gp_blur_on" => &mut self.blur_on,
            "gp_hit_on" => &mut self.hit_on,
            "gp_hit_armor" => &mut self.hit_armor,
            "gp_dmg_on" => &mut self.dmg_on,
            "gp_dmg_hearts" => &mut self.dmg_hearts,
            "gp_nametags" => &mut self.nametags,
            "gp_name_no_bg" => &mut self.name_no_background,
            "gp_name_shadow" => &mut self.name_shadow,
            "gp_name_self" => &mut self.name_self,
            "gp_cursor_center" => &mut self.cursor_center,
            "gp_hive_on" => &mut self.hive_on,
            "gp_hive_end" => &mut self.hive_end,
            "gp_hive_elim" => &mut self.hive_elim,
            "gp_hive_solo" => &mut self.hive_solo,
            "gp_hive_check" => &mut self.hive_check,
            "gp_hive_hub" => &mut self.hive_hub,
            "gp_hive_log" => &mut self.hive_log,
            "gp_hive_say" => &mut self.hive_say,
            "gp_persp_on" => &mut self.persp_on,
            "gp_glass_on" => &mut self.glass_on,
            "gp_sb_title" => &mut self.sb_title,
            "gp_sb_horizontal" => &mut self.sb_horizontal,
            "gp_persp_snap_toggle" => &mut self.persp_snap_toggle,
            _ => return None,
        })
    }

    pub fn toggle(&self, id: &str) -> Option<bool> {
        let mut copy = self.clone();
        copy.toggle_mut(id).map(|v| *v)
    }

    /// A dropdown by its menu id: its value index, or None for another id.
    pub fn choice(&self, id: &str) -> Option<usize> {
        Some(match id {
            "gp_time" => self.time_preset,
            "gp_weather" => self.weather,
            "gp_name_scale" => self.name_scale,
            "gp_hurt_strength" => self.hurt_strength,
            "gp_cpslim_max" => self.cpslim_max,
            "gp_mk_color" => self.mk_color,
            "gp_mk_time" => self.mk_time,
            "gp_glass_mode" => self.glass_mode,
            "gp_glass_coloured" => self.glass_coloured,
            "gp_mk_danger_color" => self.mk_danger_color,
            "gp_bo_mode" => self.bo_mode,
            "gp_bo_color" => self.bo_color,
            "gp_bo_opacity" => self.bo_opacity,
            "gp_bo_fill" => self.bo_fill,
            "gp_bo_fix" => self.bo_fix,
            "gp_dmg_size" => self.dmg_size,
            "gp_dmg_time" => self.dmg_time,
            "gp_dmg_color" => self.dmg_color,
            "gp_wp_color" | "gp_wp_edit_color" => self.wp_color,
            _ => return None,
        })
    }

    /// Picks a dropdown's option; false for another id.
    pub fn set_choice(&mut self, id: &str, index: usize) -> bool {
        match id {
            "gp_time" => self.time_preset = index.min(TIME_PRESETS.len() - 1),
            "gp_weather" => self.weather = index.min(WEATHER_NAMES.len() - 1),
            "gp_name_scale" => self.name_scale = index.min(NAME_SCALES.len() - 1),
            "gp_hurt_strength" => self.hurt_strength = index.min(HURT_STRENGTHS.len() - 1),
            "gp_cpslim_max" => self.cpslim_max = index.min(crate::handlers::CPS_LIMITS.len() - 1),
            "gp_mk_color" => self.mk_color = index.min(crate::waypoints::COLORS.len() - 1),
            "gp_mk_time" => self.mk_time = index.min(MARKER_TIMES.len() - 1),
            "gp_glass_mode" => self.glass_mode = crate::glass::clamp(index),
            "gp_glass_coloured" => self.glass_coloured = crate::glass::clamp(index),
            "gp_mk_danger_color" => self.mk_danger_color = index.min(crate::waypoints::COLORS.len() - 1),
            "gp_bo_mode" => self.bo_mode = index.min(crate::outline::MODE_NAMES.len() - 1),
            "gp_bo_color" => self.bo_color = index.min(7),
            "gp_bo_opacity" => self.bo_opacity = index.min(crate::outline::OPACITIES.len() - 1),
            "gp_bo_fill" => self.bo_fill = index.min(crate::outline::FILL_PERCENTS.len() - 1),
            "gp_bo_fix" => self.bo_fix = index.min(crate::outline::FIX_PERCENTS.len() - 1),
            "gp_dmg_size" => self.dmg_size = index.min(crate::damage::SIZES.len() - 1),
            "gp_dmg_time" => self.dmg_time = index.min(crate::damage::TIMES.len() - 1),
            "gp_dmg_color" => self.dmg_color = index.min(crate::damage::COLORS.len() - 1),
            "gp_wp_color" | "gp_wp_edit_color" => self.wp_color = crate::waypoints::clamp_color(index),
            _ => return false,
        }
        true
    }

    /// The Damage Indicator as drawn: (size factor, seconds, colour, hearts), or None while it is off.
    pub fn damage_look(&self) -> Option<(f32, f64, [f32; 3], bool)> {
        use crate::damage::{COLORS, SIZES, TIMES};
        self.dmg_on.then(|| {
            (
                SIZES[self.dmg_size.min(SIZES.len() - 1)].1,
                TIMES[self.dmg_time.min(TIMES.len() - 1)].1,
                COLORS[self.dmg_color.min(COLORS.len() - 1)].1,
                self.dmg_hearts,
            )
        })
    }

    /// The control modes the host should have, or None for the game's own. Only Always sprint goes to the host: the
    /// host resets its own sprint and sneak toggles whenever a screen opens, so the two toggles are kept by the mod
    /// (`guest.rs` holds the latches and sends them as movement intents).
    pub fn control_modes(&self) -> Option<[bool; 3]> {
        self.always_sprint.then_some([false, false, true])
    }

    /// The local time the Time changer shows, in game ticks.
    pub fn time_ticks(&self) -> Option<u32> {
        self.time_on.then(|| TIME_PRESETS[self.time_preset.min(TIME_PRESETS.len() - 1)].1)
    }

    /// The hurt camera as the host should have it: (remove the shake, strength in percent, fixed direction), or None
    /// for the game's own.
    pub fn hurt_camera(&self) -> Option<(bool, u32, bool)> {
        self.no_hurt_cam.then(|| (self.hurt_off, HURT_STRENGTHS[self.hurt_strength.min(HURT_STRENGTHS.len() - 1)].1, self.hurt_fixed))
    }

    /// What the host's event handlers should do (see `handlers.rs`).
    pub fn handler_config(&self) -> crate::handlers::Config {
        crate::handlers::Config {
            no_cam_reset: self.nocam_on,
            no_swing_sounds: self.noswing_on,
            subtitles: false,
            cps_limit: self.cpslim_on.then(|| crate::handlers::CPS_LIMITS[self.cpslim_max.min(crate::handlers::CPS_LIMITS.len() - 1)].1),
        }
    }

    /// The blur behind the menu in pixels: 0 while Menu blur is off.
    pub fn menu_blur(&self) -> f64 {
        if self.blur_on && self.blur_amount.is_finite() { self.blur_amount.clamp(0.0, MAX_BLUR) } else { 0.0 }
    }

    /// The brightness and saturation factors of the world behind the menu (1 is unchanged); they follow the blur's switch.
    pub fn menu_backdrop(&self) -> (f64, f64) {
        if !self.blur_on {
            return (1.0, 1.0);
        }
        let dim = if self.blur_dim.is_finite() { self.blur_dim.clamp(0.0, 100.0) } else { 30.0 };
        let sat = if self.blur_sat.is_finite() { self.blur_sat.clamp(0.0, 200.0) } else { 100.0 };
        ((100.0 - dim) / 100.0, sat / 100.0)
    }

    /// Puts a colour first in the picker's recent colours (once, at most `PICKER_SWATCHES`).
    pub fn remember_color(&mut self, rgb: u32) {
        let rgb = rgb & 0xFF_FFFF;
        self.recent_colors.retain(|c| *c != rgb);
        self.recent_colors.insert(0, rgb);
        self.recent_colors.truncate(PICKER_SWATCHES);
    }

    /// Saves a colour, or takes it out again when it is already saved. A full list drops its oldest colour.
    pub fn toggle_favorite(&mut self, rgb: u32) {
        let rgb = rgb & 0xFF_FFFF;
        if let Some(at) = self.favorite_colors.iter().position(|c| *c == rgb) {
            self.favorite_colors.remove(at);
        } else {
            self.favorite_colors.push(rgb);
            if self.favorite_colors.len() > PICKER_SWATCHES {
                self.favorite_colors.remove(0);
            }
        }
    }

    pub fn nametag_scale(&self) -> f32 {
        NAME_SCALES[self.name_scale.min(NAME_SCALES.len() - 1)].1
    }
}
