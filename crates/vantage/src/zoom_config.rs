//! Zoom settings, keys and presets, ported from Zoomify's `ZoomifySettings.kt` and `Presets.kt`
//! (LGPL-3.0-or-later, see `LICENSE-ZOOMIFY`).

use crate::config::valid_key;
use crate::transition::{PANEL_TRANSITIONS, Transition};
use crate::zoom::{MAX_DIVISOR, MIN_DIVISOR, ZoomParams};
use serde::{Deserialize, Serialize};

pub const MAX_TIME: f64 = 5.0;
pub const MIN_STEPS: u32 = 1;
pub const MAX_STEPS: u32 = 20;
pub const MIN_PER_STEP: f64 = 100.0;
pub const MAX_PER_STEP: f64 = 300.0;

/// Whether the zoom key is held down to zoom, or pressed once to switch the zoom on and off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyMode {
    #[default]
    Hold,
    Toggle,
}

impl KeyMode {
    pub const LABELS: [&'static str; 2] = ["Hold", "Toggle"];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(index: usize) -> Self {
        if index == 1 { Self::Toggle } else { Self::Hold }
    }
}

/// The three keys of the zoom. Each is a control of the menu (`id`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomKey {
    Zoom,
    StepIn,
    StepOut,
}

impl ZoomKey {
    pub const ALL: [ZoomKey; 3] = [Self::Zoom, Self::StepIn, Self::StepOut];

    pub fn id(self) -> &'static str {
        match self {
            Self::Zoom => "zoom_key",
            Self::StepIn => "zoom_step_in",
            Self::StepOut => "zoom_step_out",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.id() == id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ZoomKeys {
    pub zoom: String,
    pub step_in: String,
    pub step_out: String,
}

#[cfg(test)]
impl ZoomKeys {
    /// Every field written, as the derived `Serialize` wrote them before only changed fields were.
    pub fn whole_json(&self) -> serde_json::Value {
        let ZoomKeys { zoom, step_in, step_out } = self;
        let mut map = serde_json::Map::new();
        map.insert("zoom".into(), serde_json::to_value(zoom).unwrap());
        map.insert("step_in".into(), serde_json::to_value(step_in).unwrap());
        map.insert("step_out".into(), serde_json::to_value(step_out).unwrap());
        serde_json::Value::Object(map)
    }
}


/// Written with only the fields that differ from `ZoomKeys::default()`: a missing field reads back as
/// that same default (`#[serde(default)]`), so the settings load exactly as when every field was
/// written, by this version or an earlier one, from a much smaller file (see `Style`'s `Serialize`).
impl serde::Serialize for ZoomKeys {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let defaults = ZoomKeys::default();
        // Every field, by name: a field added to `ZoomKeys` and not listed here does not compile.
        let ZoomKeys { zoom, step_in, step_out } = self;
        // serde_json writes a struct announced with no fields as `{}` at once, so the count is exact.
        let count = 0
            + usize::from(*zoom != defaults.zoom)
            + usize::from(*step_in != defaults.step_in)
            + usize::from(*step_out != defaults.step_out);
        let mut out = serializer.serialize_struct("ZoomKeys", count)?;
        if *zoom != defaults.zoom {
            out.serialize_field("zoom", zoom)?;
        }
        if *step_in != defaults.step_in {
            out.serialize_field("step_in", step_in)?;
        }
        if *step_out != defaults.step_out {
            out.serialize_field("step_out", step_out)?;
        }
        out.end()
    }
}

impl Default for ZoomKeys {
    fn default() -> Self {
        Self { zoom: "KeyC".into(), step_in: "Equal".into(), step_out: "Minus".into() }
    }
}

impl ZoomKeys {
    pub fn get(&self, key: ZoomKey) -> &str {
        match key {
            ZoomKey::Zoom => &self.zoom,
            ZoomKey::StepIn => &self.step_in,
            ZoomKey::StepOut => &self.step_out,
        }
    }

    fn slot(&mut self, key: ZoomKey) -> &mut String {
        match key {
            ZoomKey::Zoom => &mut self.zoom,
            ZoomKey::StepIn => &mut self.step_in,
            ZoomKey::StepOut => &mut self.step_out,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Default,
    Optifine,
    OkZoomer,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct ZoomSettings {
    /// Master switch. Off (the default) the zoom never acts and its keys stay with the game.
    pub enabled: bool,
    /// Zoom divisor reached once the key is held: 4 means a quarter of the field of view.
    pub initial_zoom: f64,
    pub zoom_in_time: f64,
    pub zoom_out_time: f64,
    pub zoom_in_transition: Transition,
    pub zoom_out_transition: Transition,
    /// Zoomify's `scrollZoom`. The wheel is not exposed to mods, so these are the step keys.
    pub step_keys: bool,
    pub scroll_step_count: u32,
    /// Percent; 150 means every step multiplies the zoom by 1.5.
    pub zoom_per_step: f64,
    pub scroll_zoom_smoothness: f64,
    pub retain_zoom_steps: bool,
    /// How much the look speed follows the zoom, in percent (0 keeps the normal speed).
    pub relative_sensitivity: f64,
    pub key_mode: KeyMode,
    pub keys: ZoomKeys,
}

#[cfg(test)]
impl ZoomSettings {
    /// Every field written, as the derived `Serialize` wrote them before only changed fields were.
    pub fn whole_json(&self) -> serde_json::Value {
        let ZoomSettings { enabled, initial_zoom, zoom_in_time, zoom_out_time, zoom_in_transition, zoom_out_transition, step_keys, scroll_step_count, zoom_per_step, scroll_zoom_smoothness, retain_zoom_steps, relative_sensitivity, key_mode, keys } = self;
        let mut map = serde_json::Map::new();
        map.insert("enabled".into(), serde_json::to_value(enabled).unwrap());
        map.insert("initial_zoom".into(), serde_json::to_value(initial_zoom).unwrap());
        map.insert("zoom_in_time".into(), serde_json::to_value(zoom_in_time).unwrap());
        map.insert("zoom_out_time".into(), serde_json::to_value(zoom_out_time).unwrap());
        map.insert("zoom_in_transition".into(), serde_json::to_value(zoom_in_transition).unwrap());
        map.insert("zoom_out_transition".into(), serde_json::to_value(zoom_out_transition).unwrap());
        map.insert("step_keys".into(), serde_json::to_value(step_keys).unwrap());
        map.insert("scroll_step_count".into(), serde_json::to_value(scroll_step_count).unwrap());
        map.insert("zoom_per_step".into(), serde_json::to_value(zoom_per_step).unwrap());
        map.insert("scroll_zoom_smoothness".into(), serde_json::to_value(scroll_zoom_smoothness).unwrap());
        map.insert("retain_zoom_steps".into(), serde_json::to_value(retain_zoom_steps).unwrap());
        map.insert("relative_sensitivity".into(), serde_json::to_value(relative_sensitivity).unwrap());
        map.insert("key_mode".into(), serde_json::to_value(key_mode).unwrap());
        map.insert("keys".into(), keys.whole_json());
        serde_json::Value::Object(map)
    }
}


/// Written with only the fields that differ from `ZoomSettings::default()`: a missing field reads back as
/// that same default (`#[serde(default)]`), so the settings load exactly as when every field was
/// written, by this version or an earlier one, from a much smaller file (see `Style`'s `Serialize`).
impl serde::Serialize for ZoomSettings {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let defaults = ZoomSettings::default();
        // Every field, by name: a field added to `ZoomSettings` and not listed here does not compile.
        let ZoomSettings { enabled, initial_zoom, zoom_in_time, zoom_out_time, zoom_in_transition, zoom_out_transition, step_keys, scroll_step_count, zoom_per_step, scroll_zoom_smoothness, retain_zoom_steps, relative_sensitivity, key_mode, keys } = self;
        // serde_json writes a struct announced with no fields as `{}` at once, so the count is exact.
        let count = 0
            + usize::from(*enabled != defaults.enabled)
            + usize::from(*initial_zoom != defaults.initial_zoom)
            + usize::from(*zoom_in_time != defaults.zoom_in_time)
            + usize::from(*zoom_out_time != defaults.zoom_out_time)
            + usize::from(*zoom_in_transition != defaults.zoom_in_transition)
            + usize::from(*zoom_out_transition != defaults.zoom_out_transition)
            + usize::from(*step_keys != defaults.step_keys)
            + usize::from(*scroll_step_count != defaults.scroll_step_count)
            + usize::from(*zoom_per_step != defaults.zoom_per_step)
            + usize::from(*scroll_zoom_smoothness != defaults.scroll_zoom_smoothness)
            + usize::from(*retain_zoom_steps != defaults.retain_zoom_steps)
            + usize::from(*relative_sensitivity != defaults.relative_sensitivity)
            + usize::from(*key_mode != defaults.key_mode)
            + usize::from(*keys != defaults.keys);
        let mut out = serializer.serialize_struct("ZoomSettings", count)?;
        if *enabled != defaults.enabled {
            out.serialize_field("enabled", enabled)?;
        }
        if *initial_zoom != defaults.initial_zoom {
            out.serialize_field("initial_zoom", initial_zoom)?;
        }
        if *zoom_in_time != defaults.zoom_in_time {
            out.serialize_field("zoom_in_time", zoom_in_time)?;
        }
        if *zoom_out_time != defaults.zoom_out_time {
            out.serialize_field("zoom_out_time", zoom_out_time)?;
        }
        if *zoom_in_transition != defaults.zoom_in_transition {
            out.serialize_field("zoom_in_transition", zoom_in_transition)?;
        }
        if *zoom_out_transition != defaults.zoom_out_transition {
            out.serialize_field("zoom_out_transition", zoom_out_transition)?;
        }
        if *step_keys != defaults.step_keys {
            out.serialize_field("step_keys", step_keys)?;
        }
        if *scroll_step_count != defaults.scroll_step_count {
            out.serialize_field("scroll_step_count", scroll_step_count)?;
        }
        if *zoom_per_step != defaults.zoom_per_step {
            out.serialize_field("zoom_per_step", zoom_per_step)?;
        }
        if *scroll_zoom_smoothness != defaults.scroll_zoom_smoothness {
            out.serialize_field("scroll_zoom_smoothness", scroll_zoom_smoothness)?;
        }
        if *retain_zoom_steps != defaults.retain_zoom_steps {
            out.serialize_field("retain_zoom_steps", retain_zoom_steps)?;
        }
        if *relative_sensitivity != defaults.relative_sensitivity {
            out.serialize_field("relative_sensitivity", relative_sensitivity)?;
        }
        if *key_mode != defaults.key_mode {
            out.serialize_field("key_mode", key_mode)?;
        }
        if *keys != defaults.keys {
            out.serialize_field("keys", keys)?;
        }
        out.end()
    }
}

impl Default for ZoomSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            initial_zoom: 4.0,
            zoom_in_time: 1.0,
            zoom_out_time: 0.5,
            zoom_in_transition: Transition::EaseOutExp,
            zoom_out_transition: Transition::EaseOutExp,
            step_keys: true,
            scroll_step_count: 10,
            zoom_per_step: 150.0,
            scroll_zoom_smoothness: 70.0,
            retain_zoom_steps: false,
            relative_sensitivity: 100.0,
            key_mode: KeyMode::Hold,
            keys: ZoomKeys::default(),
        }
    }
}

impl ZoomSettings {
    /// Keeps every value inside what the menu and the camera accept. A key that is not a
    /// plain key name becomes unbound; so does one that another zoom key already has.
    pub fn sanitize(&mut self) {
        fn fix(value: &mut f64, min: f64, max: f64, default: f64) {
            *value = if value.is_finite() { value.clamp(min, max) } else { default };
        }
        let d = Self::default();
        fix(&mut self.initial_zoom, MIN_DIVISOR, MAX_DIVISOR, d.initial_zoom);
        fix(&mut self.zoom_in_time, 0.0, MAX_TIME, d.zoom_in_time);
        fix(&mut self.zoom_out_time, 0.0, MAX_TIME, d.zoom_out_time);
        fix(&mut self.zoom_per_step, MIN_PER_STEP, MAX_PER_STEP, d.zoom_per_step);
        fix(&mut self.scroll_zoom_smoothness, 0.0, 100.0, d.scroll_zoom_smoothness);
        fix(&mut self.relative_sensitivity, 0.0, 100.0, d.relative_sensitivity);
        self.scroll_step_count = self.scroll_step_count.clamp(MIN_STEPS, MAX_STEPS);
        for key in ZoomKey::ALL {
            if !valid_key(self.keys.get(key)) {
                self.keys.slot(key).clear();
            }
        }
        let (zoom, step_in) = (self.keys.zoom.clone(), self.keys.step_in.clone());
        if self.keys.step_in == zoom {
            self.keys.step_in.clear();
        }
        if self.keys.step_out == zoom || self.keys.step_out == step_in {
            self.keys.step_out.clear();
        }
    }

    /// Binds a zoom key (an empty `key` unbinds it), taking it from the other zoom keys.
    pub fn set_key(&mut self, key: ZoomKey, value: &str) {
        self.release(value);
        *self.keys.slot(key) = value.to_owned();
    }

    /// Unbinds whichever zoom key is `taken`, because something else now uses it.
    pub fn release(&mut self, taken: &str) {
        if taken.is_empty() {
            return;
        }
        for key in ZoomKey::ALL {
            if self.keys.get(key) == taken {
                self.keys.slot(key).clear();
            }
        }
    }

    pub fn apply_preset(&mut self, preset: Preset) {
        let (enabled, keys) = (self.enabled, self.keys.clone());
        *self = Self { enabled, keys, ..Self::default() };
        match preset {
            Preset::Default => {}
            Preset::Optifine => {
                self.zoom_in_transition = Transition::Instant;
                self.zoom_out_transition = Transition::Instant;
                self.step_keys = false;
                self.relative_sensitivity = 0.0;
            }
            Preset::OkZoomer => {
                self.zoom_in_time = 0.25;
                self.zoom_out_time = 0.25;
                self.relative_sensitivity = 50.0;
                self.scroll_zoom_smoothness = 25.0;
            }
        }
    }

    pub fn primary_params(&self) -> ZoomParams {
        ZoomParams {
            initial_zoom: self.initial_zoom,
            transition_in: self.zoom_in_transition,
            transition_out: self.zoom_out_transition,
            time_in: self.zoom_in_time,
            time_out: self.zoom_out_time,
            zoom_per_step: self.zoom_per_step,
            max_steps: if self.step_keys { self.scroll_step_count } else { 0 },
            // Zoomify: lerp(scrollZoomSmoothness / 100, 1.0, 0.1)
            step_smoothness: 1.0 - (self.scroll_zoom_smoothness / 100.0) * 0.9,
        }
    }

    /// The keys the mod must keep away from ordinary gameplay: none while the zoom is off.
    pub fn reserved_keys(&self) -> Vec<String> {
        if !self.enabled {
            return Vec::new();
        }
        let mut keys = vec![self.keys.zoom.clone()];
        if self.step_keys {
            keys.push(self.keys.step_in.clone());
            keys.push(self.keys.step_out.clone());
        }
        keys.retain(|key| !key.is_empty());
        keys.sort();
        keys.dedup();
        keys
    }

    /// The key to ask the game to report as held, when the zoom works by holding it.
    pub fn held_key(&self) -> Option<&str> {
        let key = self.keys.zoom.as_str();
        (self.enabled && self.key_mode == KeyMode::Hold && !key.is_empty()).then_some(key)
    }
}

/// The menu's index of a curve. The choice control holds at most eight options, so a curve
/// set only in the settings file shows as Linear until it is changed.
pub fn transition_index(transition: Transition) -> usize {
    PANEL_TRANSITIONS.iter().position(|t| *t == transition).unwrap_or(1)
}

pub fn transition_at(index: usize) -> Transition {
    PANEL_TRANSITIONS[index.min(PANEL_TRANSITIONS.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_zoomify_and_start_off() {
        let s = ZoomSettings::default();
        assert!(!s.enabled);
        assert_eq!((s.initial_zoom, s.zoom_in_time, s.zoom_out_time), (4.0, 1.0, 0.5));
        assert_eq!((s.scroll_step_count, s.zoom_per_step, s.scroll_zoom_smoothness), (10, 150.0, 70.0));
        assert!((s.primary_params().step_smoothness - 0.37).abs() < 1e-9);
        let mut s = s;
        s.scroll_zoom_smoothness = 0.0;
        assert_eq!(s.primary_params().step_smoothness, 1.0);
        s.scroll_zoom_smoothness = 100.0;
        assert!((s.primary_params().step_smoothness - 0.1).abs() < 1e-9);
    }

    #[test]
    fn sanitize_clamps_and_repairs_keys() {
        let mut s: ZoomSettings = serde_json::from_str(
            r#"{"initial_zoom": 99, "zoom_in_time": -3, "scroll_step_count": 0,
                "keys": {"zoom": "Mouse Left!", "step_in": "KeyV", "step_out": "KeyV"}}"#,
        )
        .unwrap();
        s.sanitize();
        assert_eq!(s.initial_zoom, MAX_DIVISOR);
        assert_eq!(s.zoom_in_time, 0.0);
        assert_eq!(s.scroll_step_count, MIN_STEPS);
        assert_eq!(s.keys.zoom, "");
        assert_eq!((s.keys.step_in.as_str(), s.keys.step_out.as_str()), ("KeyV", ""));
    }

    #[test]
    fn a_key_goes_to_one_zoom_control_only() {
        let mut s = ZoomSettings::default();
        s.set_key(ZoomKey::StepIn, "KeyZ");
        assert_eq!((s.keys.zoom.as_str(), s.keys.step_in.as_str()), ("", "KeyZ"));
        s.release("KeyZ");
        assert_eq!(s.keys.step_in, "");
        s.set_key(ZoomKey::Zoom, "");
        assert_eq!(s.keys.zoom, "");
    }

    #[test]
    fn reserved_keys_follow_the_switches() {
        let mut s = ZoomSettings::default();
        assert!(s.reserved_keys().is_empty() && s.held_key().is_none());
        s.enabled = true;
        assert_eq!(s.reserved_keys(), ["Equal", "KeyZ", "Minus"]);
        assert_eq!(s.held_key(), Some("KeyZ"));
        s.step_keys = false;
        s.key_mode = KeyMode::Toggle;
        assert_eq!(s.reserved_keys(), ["KeyZ"]);
        assert_eq!(s.held_key(), None);
    }

    #[test]
    fn presets_follow_zoomify_and_keep_keys_and_switch() {
        let mut s = ZoomSettings::default();
        s.enabled = true;
        s.keys.zoom = "KeyV".into();
        s.initial_zoom = 8.0;
        s.apply_preset(Preset::Optifine);
        assert_eq!(s.zoom_in_transition, Transition::Instant);
        assert!(!s.step_keys && s.relative_sensitivity == 0.0);
        assert_eq!(s.initial_zoom, 4.0);
        assert!(s.enabled && s.keys.zoom == "KeyV");
        s.apply_preset(Preset::OkZoomer);
        assert_eq!((s.zoom_in_time, s.relative_sensitivity, s.scroll_zoom_smoothness), (0.25, 50.0, 25.0));
        assert!(s.step_keys);
        s.apply_preset(Preset::Default);
        assert_eq!(s, ZoomSettings { enabled: true, keys: s.keys.clone(), ..ZoomSettings::default() });
    }

    #[test]
    fn menu_curves_round_trip() {
        for (i, curve) in PANEL_TRANSITIONS.iter().enumerate() {
            assert_eq!(transition_index(*curve), i);
            assert_eq!(transition_at(i), *curve);
        }
        assert_eq!(transition_at(99), PANEL_TRANSITIONS[7]);
    }
}
