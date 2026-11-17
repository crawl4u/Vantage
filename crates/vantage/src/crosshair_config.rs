//! The Crosshair's settings, and what they turn into for one frame: which drawing to show and in
//! which colour. The shapes themselves are in `crosshair.rs`.
//!
//! Modelled on Lunar Client's Crosshair mod (size, gap, thickness, a centre dot, a static colour
//! that changes over a target) and on the Redline crosshair mod (a ring and a dot shape, a different
//! size on target, and a separate look while a bow or crossbow is held).

use crate::config::{PALETTE, WHITE, rgba};
use crate::crosshair::{Look, SHAPES, Target, groups, state};
use serde::Deserialize;

pub const MIN_SIZE: f64 = 0.25;
pub const MAX_SIZE: f64 = 16.0;
pub const MAX_GAP: f64 = 12.0;
pub const MIN_THICKNESS: f64 = 0.25;
pub const MAX_THICKNESS: f64 = 16.0;
pub const MIN_TARGET_SCALE: f64 = 0.25;
pub const MAX_TARGET_SCALE: f64 = 2.0;
pub const MIN_OUTLINE_WIDTH: f64 = 0.25;
pub const MAX_OUTLINE_WIDTH: f64 = 8.0;

const BLACK: usize = 2;
const RED: usize = 3;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct CrosshairSettings {
    /// Draws this crosshair, and hides the game's own while it does.
    pub enabled: bool,
    /// Keep the crosshair on screen in third person (the game's own goes away there).
    pub third_person: bool,
    /// Index into `crosshair::SHAPES`.
    pub shape: usize,
    pub size: f64,
    pub gap: f64,
    pub thickness: f64,
    /// A small square in the middle of the Gapped and Circle shapes.
    pub dot: bool,
    /// The centre dot is round.
    pub dot_round: bool,
    /// Index into `PALETTE`, and the opacity in percent.
    pub color: usize,
    pub opacity: f64,
    pub outline: bool,
    pub outline_color: usize,
    /// Round corners on the outline of the Gapped and Plus shapes.
    pub outline_round: bool,
    /// How far the outline reaches past the crosshair, in GUI pixels.
    pub outline_width: f64,
    /// The arms of the Gapped and Plus shapes have round ends.
    pub round_lines: bool,
    /// Changes colour, size and opacity while the crosshair is on a player or mob.
    pub on_target: bool,
    pub players: bool,
    pub mobs: bool,
    pub player_color: usize,
    pub mob_color: usize,
    /// How much bigger (or smaller) the crosshair is on target, as a multiple.
    pub target_scale: f64,
    pub target_opacity: f64,
    /// A look of its own while a bow or crossbow is in the hand.
    pub weapon: bool,
    pub weapon_shape: usize,
    pub weapon_size: f64,
    pub weapon_gap: f64,
    pub weapon_thickness: f64,
}

#[cfg(test)]
impl CrosshairSettings {
    /// Every field written, as the derived `Serialize` wrote them before only changed fields were.
    pub fn whole_json(&self) -> serde_json::Value {
        let CrosshairSettings { enabled, third_person, shape, size, gap, thickness, dot, dot_round, color, opacity, outline, outline_color, outline_round, outline_width, round_lines, on_target, players, mobs, player_color, mob_color, target_scale, target_opacity, weapon, weapon_shape, weapon_size, weapon_gap, weapon_thickness } = self;
        let mut map = serde_json::Map::new();
        map.insert("enabled".into(), serde_json::to_value(enabled).unwrap());
        map.insert("third_person".into(), serde_json::to_value(third_person).unwrap());
        map.insert("shape".into(), serde_json::to_value(shape).unwrap());
        map.insert("size".into(), serde_json::to_value(size).unwrap());
        map.insert("gap".into(), serde_json::to_value(gap).unwrap());
        map.insert("thickness".into(), serde_json::to_value(thickness).unwrap());
        map.insert("dot".into(), serde_json::to_value(dot).unwrap());
        map.insert("dot_round".into(), serde_json::to_value(dot_round).unwrap());
        map.insert("color".into(), serde_json::to_value(color).unwrap());
        map.insert("opacity".into(), serde_json::to_value(opacity).unwrap());
        map.insert("outline".into(), serde_json::to_value(outline).unwrap());
        map.insert("outline_color".into(), serde_json::to_value(outline_color).unwrap());
        map.insert("outline_round".into(), serde_json::to_value(outline_round).unwrap());
        map.insert("outline_width".into(), serde_json::to_value(outline_width).unwrap());
        map.insert("round_lines".into(), serde_json::to_value(round_lines).unwrap());
        map.insert("on_target".into(), serde_json::to_value(on_target).unwrap());
        map.insert("players".into(), serde_json::to_value(players).unwrap());
        map.insert("mobs".into(), serde_json::to_value(mobs).unwrap());
        map.insert("player_color".into(), serde_json::to_value(player_color).unwrap());
        map.insert("mob_color".into(), serde_json::to_value(mob_color).unwrap());
        map.insert("target_scale".into(), serde_json::to_value(target_scale).unwrap());
        map.insert("target_opacity".into(), serde_json::to_value(target_opacity).unwrap());
        map.insert("weapon".into(), serde_json::to_value(weapon).unwrap());
        map.insert("weapon_shape".into(), serde_json::to_value(weapon_shape).unwrap());
        map.insert("weapon_size".into(), serde_json::to_value(weapon_size).unwrap());
        map.insert("weapon_gap".into(), serde_json::to_value(weapon_gap).unwrap());
        map.insert("weapon_thickness".into(), serde_json::to_value(weapon_thickness).unwrap());
        serde_json::Value::Object(map)
    }
}


/// Written with only the fields that differ from `CrosshairSettings::default()`: a missing field reads back as
/// that same default (`#[serde(default)]`), so the settings load exactly as when every field was
/// written, by this version or an earlier one, from a much smaller file (see `Style`'s `Serialize`).
impl serde::Serialize for CrosshairSettings {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let defaults = CrosshairSettings::default();
        // Every field, by name: a field added to `CrosshairSettings` and not listed here does not compile.
        let CrosshairSettings { enabled, third_person, shape, size, gap, thickness, dot, dot_round, color, opacity, outline, outline_color, outline_round, outline_width, round_lines, on_target, players, mobs, player_color, mob_color, target_scale, target_opacity, weapon, weapon_shape, weapon_size, weapon_gap, weapon_thickness } = self;
        // serde_json writes a struct announced with no fields as `{}` at once, so the count is exact.
        let count = 0
            + usize::from(*enabled != defaults.enabled)
            + usize::from(*third_person != defaults.third_person)
            + usize::from(*shape != defaults.shape)
            + usize::from(*size != defaults.size)
            + usize::from(*gap != defaults.gap)
            + usize::from(*thickness != defaults.thickness)
            + usize::from(*dot != defaults.dot)
            + usize::from(*dot_round != defaults.dot_round)
            + usize::from(*color != defaults.color)
            + usize::from(*opacity != defaults.opacity)
            + usize::from(*outline != defaults.outline)
            + usize::from(*outline_color != defaults.outline_color)
            + usize::from(*outline_round != defaults.outline_round)
            + usize::from(*outline_width != defaults.outline_width)
            + usize::from(*round_lines != defaults.round_lines)
            + usize::from(*on_target != defaults.on_target)
            + usize::from(*players != defaults.players)
            + usize::from(*mobs != defaults.mobs)
            + usize::from(*player_color != defaults.player_color)
            + usize::from(*mob_color != defaults.mob_color)
            + usize::from(*target_scale != defaults.target_scale)
            + usize::from(*target_opacity != defaults.target_opacity)
            + usize::from(*weapon != defaults.weapon)
            + usize::from(*weapon_shape != defaults.weapon_shape)
            + usize::from(*weapon_size != defaults.weapon_size)
            + usize::from(*weapon_gap != defaults.weapon_gap)
            + usize::from(*weapon_thickness != defaults.weapon_thickness);
        let mut out = serializer.serialize_struct("CrosshairSettings", count)?;
        if *enabled != defaults.enabled {
            out.serialize_field("enabled", enabled)?;
        }
        if *third_person != defaults.third_person {
            out.serialize_field("third_person", third_person)?;
        }
        if *shape != defaults.shape {
            out.serialize_field("shape", shape)?;
        }
        if *size != defaults.size {
            out.serialize_field("size", size)?;
        }
        if *gap != defaults.gap {
            out.serialize_field("gap", gap)?;
        }
        if *thickness != defaults.thickness {
            out.serialize_field("thickness", thickness)?;
        }
        if *dot != defaults.dot {
            out.serialize_field("dot", dot)?;
        }
        if *dot_round != defaults.dot_round {
            out.serialize_field("dot_round", dot_round)?;
        }
        if *color != defaults.color {
            out.serialize_field("color", color)?;
        }
        if *opacity != defaults.opacity {
            out.serialize_field("opacity", opacity)?;
        }
        if *outline != defaults.outline {
            out.serialize_field("outline", outline)?;
        }
        if *outline_color != defaults.outline_color {
            out.serialize_field("outline_color", outline_color)?;
        }
        if *outline_round != defaults.outline_round {
            out.serialize_field("outline_round", outline_round)?;
        }
        if *outline_width != defaults.outline_width {
            out.serialize_field("outline_width", outline_width)?;
        }
        if *round_lines != defaults.round_lines {
            out.serialize_field("round_lines", round_lines)?;
        }
        if *on_target != defaults.on_target {
            out.serialize_field("on_target", on_target)?;
        }
        if *players != defaults.players {
            out.serialize_field("players", players)?;
        }
        if *mobs != defaults.mobs {
            out.serialize_field("mobs", mobs)?;
        }
        if *player_color != defaults.player_color {
            out.serialize_field("player_color", player_color)?;
        }
        if *mob_color != defaults.mob_color {
            out.serialize_field("mob_color", mob_color)?;
        }
        if *target_scale != defaults.target_scale {
            out.serialize_field("target_scale", target_scale)?;
        }
        if *target_opacity != defaults.target_opacity {
            out.serialize_field("target_opacity", target_opacity)?;
        }
        if *weapon != defaults.weapon {
            out.serialize_field("weapon", weapon)?;
        }
        if *weapon_shape != defaults.weapon_shape {
            out.serialize_field("weapon_shape", weapon_shape)?;
        }
        if *weapon_size != defaults.weapon_size {
            out.serialize_field("weapon_size", weapon_size)?;
        }
        if *weapon_gap != defaults.weapon_gap {
            out.serialize_field("weapon_gap", weapon_gap)?;
        }
        if *weapon_thickness != defaults.weapon_thickness {
            out.serialize_field("weapon_thickness", weapon_thickness)?;
        }
        out.end()
    }
}

impl Default for CrosshairSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            third_person: false,
            shape: 0,
            // Arms of 5 starting 2 from the middle, one pixel thick: the size of the game's own.
            size: 5.0,
            gap: 2.0,
            thickness: 1.0,
            dot: false,
            dot_round: false,
            color: WHITE,
            opacity: 90.0,
            outline: false,
            outline_color: BLACK,
            outline_round: false,
            outline_width: 1.0,
            round_lines: false,
            on_target: true,
            players: true,
            mobs: true,
            player_color: RED,
            mob_color: RED,
            target_scale: 1.0,
            target_opacity: 100.0,
            weapon: false,
            weapon_shape: 0,
            weapon_size: 3.0,
            weapon_gap: 0.0,
            weapon_thickness: 1.5,
        }
    }
}

/// What the HUD needs to draw one frame of the crosshair. Equal frames draw the same, so the HUD
/// only sends a change when this changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// Which of the document's drawings is shown (see `CrosshairSettings::drawings`).
    pub drawing: usize,
    pub fill: [f32; 4],
    pub outline: [f32; 4],
}

impl CrosshairSettings {
    pub fn sanitize(&mut self) {
        fn fix(value: &mut f64, min: f64, max: f64, default: f64) {
            *value = if value.is_finite() { value.clamp(min, max) } else { default };
        }
        let d = Self::default();
        fix(&mut self.size, MIN_SIZE, MAX_SIZE, d.size);
        fix(&mut self.gap, 0.0, MAX_GAP, d.gap);
        fix(&mut self.thickness, MIN_THICKNESS, MAX_THICKNESS, d.thickness);
        fix(&mut self.outline_width, MIN_OUTLINE_WIDTH, MAX_OUTLINE_WIDTH, d.outline_width);
        fix(&mut self.opacity, 0.0, 100.0, d.opacity);
        fix(&mut self.target_scale, MIN_TARGET_SCALE, MAX_TARGET_SCALE, d.target_scale);
        fix(&mut self.target_opacity, 0.0, 100.0, d.target_opacity);
        fix(&mut self.weapon_size, MIN_SIZE, MAX_SIZE, d.weapon_size);
        fix(&mut self.weapon_gap, 0.0, MAX_GAP, d.weapon_gap);
        fix(&mut self.weapon_thickness, MIN_THICKNESS, MAX_THICKNESS, d.weapon_thickness);
        self.shape = self.shape.min(SHAPES.len() - 1);
        self.weapon_shape = self.weapon_shape.min(SHAPES.len() - 1);
        for color in [&mut self.color, &mut self.outline_color, &mut self.player_color, &mut self.mob_color] {
            if !crate::config::valid_color(*color) {
                *color = WHITE;
            }
        }
    }

    /// The look in one state: with the weapon preset or without, on target or not. With the preset
    /// switched off, a bow in the hand changes nothing, so those states look like the others.
    pub fn look(&self, weapon: bool, targeted: bool) -> Look {
        let (shape, size, gap, thickness) = if weapon && self.weapon {
            (self.weapon_shape, self.weapon_size, self.weapon_gap, self.weapon_thickness)
        } else {
            (self.shape, self.size, self.gap, self.thickness)
        };
        Look {
            shape,
            size: size as f32,
            gap: gap as f32,
            thickness: thickness as f32,
            dot: self.dot,
            round_dot: self.dot_round,
            round_lines: self.round_lines,
            scale: if targeted && self.on_target { self.target_scale as f32 } else { 1.0 },
        }
    }

    /// The distinct drawings the document holds, and which one each of the four states
    /// (`crosshair::state`) shows. The target states only differ when a target size is set.
    pub fn drawings(&self) -> (Vec<Look>, [usize; 4]) {
        groups([self.look(false, false), self.look(false, true), self.look(true, false), self.look(true, true)])
    }

    /// Whether the weapon preset is in use: switched on and a bow or crossbow in the hand.
    pub fn weapon_active(&self, holding_ranged: bool) -> bool {
        self.weapon && holding_ranged
    }

    /// Whether a target changes the crosshair at all.
    pub fn marks(&self, target: Target) -> bool {
        self.on_target
            && match target {
                Target::None => false,
                Target::Player => self.players,
                Target::Mob => self.mobs,
            }
    }

    /// The frame to draw for a target and a held item.
    pub fn frame(&self, target: Target, holding_ranged: bool) -> Frame {
        let targeted = self.marks(target);
        let (_, which) = self.drawings();
        let (colour, opacity) = match (targeted, target) {
            (true, Target::Player) => (self.player_color, self.target_opacity),
            (true, _) => (self.mob_color, self.target_opacity),
            (false, _) => (self.color, self.opacity),
        };
        let fill = rgba(colour, opacity / 100.0);
        let mut outline = rgba(self.outline_color, opacity_of(targeted, self));
        // The outline is drawn only when asked for: a clear one costs nothing and shows nothing.
        if !self.outline {
            outline[3] = 0.0;
        }
        Frame { drawing: which[state(self.weapon_active(holding_ranged), targeted)], fill, outline }
    }

    /// Whether a shape that has a centre dot option (Gapped or Circle) can be on screen: the Dot shape
    /// is a dot already and Plus has none.
    pub fn has_dot(&self) -> bool {
        let takes = |shape: usize| shape == 0 || shape == 3;
        takes(self.shape) || (self.weapon && takes(self.weapon_shape))
    }

    /// Whether a shape with straight arms (Gapped or Plus) can be on screen: the only shapes the
    /// outline's corner style applies to.
    pub fn has_lines(&self) -> bool {
        self.shape < 2 || (self.weapon && self.weapon_shape < 2)
    }

    /// Whether the document needs the outline's bars at all.
    pub fn outlined(&self) -> bool {
        self.outline
    }
}

fn opacity_of(targeted: bool, s: &CrosshairSettings) -> f64 {
    (if targeted { s.target_opacity } else { s.opacity }) / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_off_and_look_like_the_games_own() {
        let d = CrosshairSettings::default();
        assert!(!d.enabled && !d.outline && !d.weapon);
        assert_eq!((d.size, d.gap, d.thickness), (5.0, 2.0, 1.0));
        let mut again = d.clone();
        again.sanitize();
        assert_eq!(again, d);
    }

    #[test]
    fn a_damaged_file_is_repaired() {
        let mut s: CrosshairSettings =
            serde_json::from_str(r#"{"size": 99, "gap": -4, "thickness": 0, "shape": 9, "color": 77, "target_scale": 50, "opacity": 400}"#).unwrap();
        s.sanitize();
        assert_eq!((s.size, s.gap, s.thickness), (MAX_SIZE, 0.0, MIN_THICKNESS));
        assert_eq!((s.shape, s.color, s.target_scale, s.opacity), (SHAPES.len() - 1, WHITE, MAX_TARGET_SCALE, 100.0));
        // Anything missing keeps its default.
        let partial: CrosshairSettings = serde_json::from_str(r#"{"enabled": true}"#).unwrap();
        assert!(partial.enabled && partial.size == 5.0 && partial.player_color == RED);
        // A number that is not one falls back to the default.
        let mut odd = CrosshairSettings { size: f64::NAN, ..Default::default() };
        odd.sanitize();
        assert_eq!(odd.size, 5.0);
    }

    #[test]
    fn a_target_changes_the_colour_and_only_the_colour_by_default() {
        let s = CrosshairSettings::default();
        let rest = s.frame(Target::None, false);
        let on_player = s.frame(Target::Player, false);
        assert_eq!(rest.fill, rgba(WHITE, 0.9));
        assert_eq!(on_player.fill, rgba(RED, 1.0));
        // With no target size set, rest and target share one drawing.
        assert_eq!(rest.drawing, on_player.drawing);
        assert_eq!(s.drawings().0.len(), 1);
    }

    #[test]
    fn a_target_size_adds_a_second_drawing() {
        let s = CrosshairSettings { target_scale: 1.4, ..Default::default() };
        let (looks, which) = s.drawings();
        assert_eq!((looks.len(), which), (2, [0, 1, 0, 1]));
        assert_ne!(s.frame(Target::None, false).drawing, s.frame(Target::Mob, false).drawing);
        // Turned off, a target changes nothing, size included.
        let off = CrosshairSettings { target_scale: 1.4, on_target: false, ..Default::default() };
        assert_eq!(off.drawings().0.len(), 1);
        assert_eq!(off.frame(Target::Player, false), off.frame(Target::None, false));
    }

    #[test]
    fn players_and_mobs_are_switched_separately() {
        let s = CrosshairSettings { mobs: false, mob_color: 4, ..Default::default() };
        assert_eq!(s.frame(Target::Mob, false).fill, rgba(WHITE, 0.9));
        assert_eq!(s.frame(Target::Player, false).fill, rgba(RED, 1.0));
        let s = CrosshairSettings { mob_color: 4, ..Default::default() };
        assert_eq!(s.frame(Target::Mob, false).fill, rgba(4, 1.0));
    }

    #[test]
    fn the_weapon_preset_needs_both_the_switch_and_a_bow() {
        let s = CrosshairSettings { weapon: true, weapon_shape: 2, ..Default::default() };
        let (looks, _) = s.drawings();
        assert_eq!(looks.len(), 2);
        assert_ne!(s.frame(Target::None, true).drawing, s.frame(Target::None, false).drawing);
        assert_eq!(looks[s.frame(Target::None, true).drawing].shape, 2);
        // With the switch off, a bow changes nothing.
        let plain = CrosshairSettings::default();
        assert_eq!(plain.frame(Target::None, true), plain.frame(Target::None, false));
    }

    #[test]
    fn the_outline_is_clear_unless_it_is_on() {
        let mut s = CrosshairSettings::default();
        assert_eq!(s.frame(Target::None, false).outline[3], 0.0);
        s.outline = true;
        assert_eq!(s.frame(Target::None, false).outline, rgba(BLACK, 0.9));
        assert_eq!(s.frame(Target::Player, false).outline, rgba(BLACK, 1.0));
    }
}
