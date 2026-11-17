//! Zoom state machine, ported from Zoomify's `Interpolator.kt`, `ZoomHelper.kt` and
//! `DefaultZoomHelpers.kt` (LGPL-3.0-or-later, see `LICENSE-ZOOMIFY`).
//!
//! Zoomify ticks at 20 Hz and lerps between ticks. Cinnabar calls `frame` once per rendered
//! frame with the elapsed seconds, so here every frame is one step of that many seconds and
//! no tick lerp is needed.

use crate::transition::Transition;
use crate::zoom_config::{KeyMode, ZoomSettings};

/// Cinnabar's camera API limits the FOV multiplier to 0.1..=1, so zoom is 1x..=10x.
pub const MIN_DIVISOR: f64 = 1.0;
pub const MAX_DIVISOR: f64 = 10.0;
/// Zoomify's scroll smoothing moves a fixed fraction of the remaining distance per 50 ms tick.
const TICK_SECONDS: f64 = 0.05;

#[derive(Clone, Copy, Debug)]
pub struct ZoomParams {
    /// Zoom divisor reached once the key is held (`initialZoom`).
    pub initial_zoom: f64,
    pub transition_in: Transition,
    pub transition_out: Transition,
    pub time_in: f64,
    pub time_out: f64,
    /// Percent; 150 means every step multiplies the zoom by 1.5.
    pub zoom_per_step: f64,
    pub max_steps: u32,
    /// Fraction of the remaining step distance covered per 50 ms; 1.0 is instant.
    pub step_smoothness: f64,
}

#[derive(Debug)]
struct ZoomHelper {
    initial: f64,
    active: Transition,
    prev_target: f64,
    zooming_last_tick: bool,
    scroll: f64,
    last_step: u32,
    resetting: bool,
    reset_multiplier: f64,
}

impl ZoomHelper {
    fn new() -> Self {
        Self {
            initial: 0.0,
            active: Transition::Linear,
            prev_target: 0.0,
            zooming_last_tick: false,
            scroll: 0.0,
            last_step: 0,
            resetting: false,
            reset_multiplier: 1.0,
        }
    }

    fn tick(&mut self, zooming: bool, steps: u32, dt: f64, p: &ZoomParams) {
        // Initial zoom (TransitionInterpolator).
        if zooming && !self.zooming_last_tick {
            self.resetting = false;
        }
        let target = if zooming { 1.0 } else { 0.0 };
        self.initial = self.tick_transition(target, self.initial, dt, p);
        self.zooming_last_tick = zooming;

        // Step zoom (SmoothInterpolator).
        if steps > self.last_step {
            self.resetting = false;
        }
        let target = if p.max_steps > 0 {
            f64::from(steps) / f64::from(p.max_steps)
        } else {
            0.0
        };
        self.scroll = if p.step_smoothness >= 1.0 {
            target
        } else {
            let k = 1.0 - (1.0 - p.step_smoothness).powf(dt / TICK_SECONDS);
            let next = self.scroll + (target - self.scroll) * k;
            if (target - next).abs() < 1e-4 { target } else { next }
        };
        self.last_step = steps;
    }

    fn tick_transition(&mut self, target: f64, mut current: f64, dt: f64, p: &ZoomParams) -> f64 {
        let t_in = p.transition_in;
        let t_out = p.transition_out.opposite();
        if target > current {
            self.active = t_in;
            // The target just flipped upwards: keep the visible zoom continuous when the
            // curve changes, as Zoomify does.
            if self.prev_target < target {
                if let Some(t) = t_in.inverse(t_out.apply(current)) {
                    current = t;
                }
            }
        } else if target < current {
            self.active = t_out;
            if self.prev_target > target {
                if let Some(t) = t_out.inverse(t_in.apply(current)) {
                    current = t;
                }
            }
        }
        self.prev_target = target;

        if self.active == Transition::Instant {
            return target;
        }
        if target > current {
            if p.time_in <= 0.0 {
                target
            } else {
                (current + dt / p.time_in).min(target)
            }
        } else if target < current {
            if p.time_out <= 0.0 {
                target
            } else {
                (current - dt / p.time_out).max(target)
            }
        } else {
            target
        }
    }

    /// Zoom divisor for this frame, clamped to what Cinnabar can display.
    fn divisor(&mut self, p: &ZoomParams) -> f64 {
        let eased = self.active.apply(self.initial).clamp(0.0, 1.0);
        let target_multiplier = if self.resetting {
            self.reset_multiplier
        } else {
            1.0 / p.initial_zoom.max(1.0)
        };
        let initial_multiplier = 1.0 + eased * (target_multiplier - 1.0);
        let base_divisor = 1.0 / initial_multiplier;

        let scroll = if self.resetting { 0.0 } else { self.scroll };
        let current_step = scroll * f64::from(p.max_steps);
        let step_multiplier = p.zoom_per_step / 100.0;
        let divisor = (base_divisor * step_multiplier.powf(current_step))
            .clamp(MIN_DIVISOR, MAX_DIVISOR);

        if self.initial == 0.0 && self.scroll == 0.0 {
            self.resetting = false;
        }
        if !self.resetting {
            self.reset_multiplier = 1.0 / divisor;
        }
        divisor
    }

    /// Fully zoomed out with no step zoom left: nothing to animate.
    fn at_rest(&self) -> bool {
        self.initial == 0.0 && self.scroll == 0.0
    }

    /// Releasing the key drops the step zoom at once and animates the whole zoom out
    /// from where it currently is.
    fn reset(&mut self) {
        if !self.resetting && self.scroll > 0.0 {
            self.resetting = true;
            self.scroll = 0.0;
        }
    }
}

/// What the camera should do this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Total zoom divisor (1.0 = no zoom). Only the tests read it.
    #[allow(dead_code)]
    pub divisor: f64,
    /// FOV multiplier for `camera.set-view-scale`.
    pub fov_scale: f32,
    /// Look-sensitivity multiplier for `camera.set-view-scale`.
    pub look_scale: f32,
}

impl View {
    pub fn is_neutral(&self) -> bool {
        self.fov_scale >= 0.9995 && self.look_scale >= 0.9995
    }
}

/// What the player did this frame.
pub struct Input<'a> {
    /// The player can act: the game has focus, it is in gameplay and no menu is open.
    pub active: bool,
    pub pressed: &'a [String],
    pub held: &'a [String],
    /// The touch button was tapped.
    pub tap: bool,
    /// Wheel steps since the last frame (positive zooms in); the host only hands them over while the zoom key is held.
    pub scroll: i32,
    pub seconds: f64,
}

pub struct Zoomer {
    helper: ZoomHelper,
    steps: u32,
    zooming: bool,
    /// Zoom switched on with the touch button (a button cannot be held).
    touch: bool,
}

impl Default for Zoomer {
    fn default() -> Self {
        Self { helper: ZoomHelper::new(), steps: 0, zooming: false, touch: false }
    }
}

impl Zoomer {
    #[cfg(test)]
    pub fn steps(&self) -> u32 {
        self.steps
    }

    /// Reads the keys and moves the zoom one frame. Returns the view to send to the camera, or
    /// `None` when there is no zoom: the host restores the normal view when it is not renewed.
    pub fn frame(&mut self, s: &ZoomSettings, input: &Input) -> Option<View> {
        let mut step_delta = 0;
        if s.enabled && input.active {
            let keys = &s.keys;
            let pressed = |key: &str| !key.is_empty() && input.pressed.iter().any(|k| k == key);
            match s.key_mode {
                KeyMode::Hold => {
                    if input.tap {
                        self.touch = !self.touch;
                    }
                    let down = !keys.zoom.is_empty() && input.held.iter().any(|k| *k == keys.zoom);
                    self.zooming = down || self.touch;
                }
                KeyMode::Toggle => {
                    if pressed(&keys.zoom) || input.tap {
                        self.zooming = !self.zooming;
                    }
                }
            }
            if s.step_keys {
                step_delta = i32::from(pressed(&keys.step_in)) - i32::from(pressed(&keys.step_out)) + input.scroll.clamp(-4, 4);
            }
        } else {
            self.zooming = false;
            self.touch = false;
        }
        // At rest and not zooming: nothing to animate, nothing to send.
        if !self.zooming && self.helper.at_rest() {
            if !s.retain_zoom_steps {
                self.steps = 0;
            }
            return None;
        }
        let seconds = if input.seconds.is_finite() { input.seconds.clamp(0.0, 0.25) } else { 0.0 };
        let view = self.update(self.zooming, step_delta, seconds, s);
        (!view.is_neutral()).then_some(view)
    }

    fn update(&mut self, zooming: bool, step_delta: i32, seconds: f64, s: &ZoomSettings) -> View {
        let p = s.primary_params();
        let helper = &mut self.helper;
        let max = i64::from(p.max_steps);
        self.steps = (i64::from(self.steps) + i64::from(step_delta)).clamp(0, max) as u32;

        helper.tick(zooming, self.steps, seconds, &p);
        if !zooming {
            if !s.retain_zoom_steps {
                self.steps = 0;
            }
            helper.reset();
        }
        let divisor = helper.divisor(&p);

        // Mouse sensitivity divided by lerp(relativeSensitivity, 1, divisor), as in MouseMixin.
        let sensitivity = (s.relative_sensitivity / 100.0).clamp(0.0, 1.0);
        let look = 1.0 / (1.0 + sensitivity * (divisor - 1.0));
        View {
            divisor,
            fov_scale: (1.0 / divisor).clamp(0.1, 1.0) as f32,
            look_scale: look.clamp(0.05, 1.0) as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 60.0;

    fn settings() -> ZoomSettings {
        ZoomSettings { enabled: true, ..ZoomSettings::default() }
    }

    fn run(z: &mut Zoomer, frames: usize, zooming: bool) -> View {
        let s = settings();
        let mut view = None;
        for _ in 0..frames {
            view = Some(z.update(zooming, 0, DT, &s));
        }
        view.unwrap()
    }

    fn keys(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    fn frame(z: &mut Zoomer, s: &ZoomSettings, pressed: &[&str], held: &[&str], active: bool) -> Option<View> {
        let (pressed, held) = (keys(pressed), keys(held));
        z.frame(s, &Input { active, pressed: &pressed, held: &held, tap: false, scroll: 0, seconds: DT })
    }

    #[test]
    fn idle_is_neutral() {
        let mut z = Zoomer::default();
        assert!(run(&mut z, 10, false).is_neutral());
    }

    #[test]
    fn zooms_in_to_initial_zoom_then_out() {
        let mut z = Zoomer::default();
        let v = run(&mut z, 60 * 2, true);
        assert!((v.divisor - 4.0).abs() < 1e-6, "got {}", v.divisor);
        assert!((v.fov_scale - 0.25).abs() < 1e-6);
        assert!((v.look_scale - 0.25).abs() < 1e-6);
        let v = run(&mut z, 60, false);
        assert!(v.is_neutral(), "got {v:?}");
    }

    #[test]
    fn zoom_is_monotonic_while_zooming_in() {
        let mut z = Zoomer::default();
        let s = settings();
        let mut prev = 1.0;
        for _ in 0..120 {
            let v = z.update(true, 0, DT, &s);
            assert!(v.divisor + 1e-9 >= prev);
            prev = v.divisor;
        }
    }

    #[test]
    fn reversing_mid_zoom_keeps_the_visible_zoom_continuous() {
        // Reversing swaps curves; Zoomify remaps progress so the eased value is unchanged.
        let p = settings().primary_params();
        for (zoom_in, zoom_out) in [
            (Transition::EaseOutExp, Transition::EaseOutExp),
            (Transition::EaseOutSine, Transition::EaseOutQuad),
            (Transition::EaseOutCubic, Transition::EaseOutSine),
        ] {
            let p = ZoomParams { transition_in: zoom_in, transition_out: zoom_out, ..p };
            let mut helper = ZoomHelper::new();
            for _ in 0..15 {
                helper.tick(true, 0, DT, &p);
            }
            let before = helper.active.apply(helper.initial);
            assert!(before > 0.05 && before < 0.95, "{zoom_in:?}: {before}");
            // A zero-length step changes direction without moving.
            helper.tick(false, 0, 0.0, &p);
            let after = helper.active.apply(helper.initial);
            assert!((after - before).abs() < 1e-6, "{zoom_in:?}/{zoom_out:?}: {before} -> {after}");
        }
    }

    #[test]
    fn zoom_out_after_partial_zoom_in_is_monotonic() {
        let mut z = Zoomer::default();
        let s = settings();
        run(&mut z, 15, true);
        let mut prev = f64::MAX;
        for _ in 0..120 {
            let v = z.update(false, 0, DT, &s);
            assert!(v.divisor <= prev + 1e-9);
            prev = v.divisor;
        }
        assert_eq!(prev, 1.0);
    }

    #[test]
    fn steps_zoom_further_and_are_clamped() {
        let mut z = Zoomer::default();
        let s = settings();
        run(&mut z, 120, true);
        for _ in 0..3 {
            z.update(true, 1, DT, &s);
        }
        let v = run(&mut z, 240, true);
        assert!(v.divisor > 9.99 && v.divisor <= MAX_DIVISOR + 1e-9, "got {}", v.divisor);
        assert_eq!(z.steps(), 3);
        for _ in 0..50 {
            z.update(true, 1, DT, &s);
        }
        assert_eq!(z.steps(), 10);
    }

    #[test]
    fn releasing_after_steps_animates_out_from_current_zoom() {
        let mut z = Zoomer::default();
        let s = settings();
        run(&mut z, 120, true);
        for _ in 0..2 {
            z.update(true, 1, DT, &s);
        }
        let zoomed = run(&mut z, 240, true).divisor;
        assert!(zoomed > 8.0);
        let first = z.update(false, 0, DT, &s);
        // It starts from the zoomed-in view and eases out; it does not jump to 1x.
        assert!(first.divisor < zoomed && first.divisor > 2.0, "went to {}", first.divisor);
        let mut prev = first.divisor;
        for _ in 0..120 {
            let v = z.update(false, 0, DT, &s);
            assert!(v.divisor <= prev + 1e-9);
            prev = v.divisor;
        }
        assert_eq!(prev, 1.0);
        assert_eq!(z.steps(), 0);
    }

    #[test]
    fn instant_transitions_snap() {
        let s = ZoomSettings {
            zoom_in_transition: Transition::Instant,
            zoom_out_transition: Transition::Instant,
            ..settings()
        };
        let mut z = Zoomer::default();
        let v = z.update(true, 0, DT, &s);
        assert!((v.divisor - 4.0).abs() < 1e-9);
        assert!(z.update(false, 0, DT, &s).is_neutral());
    }

    #[test]
    fn zero_time_does_not_divide_by_zero() {
        let s = ZoomSettings { zoom_in_time: 0.0, zoom_out_time: 0.0, ..settings() };
        let mut z = Zoomer::default();
        let v = z.update(true, 0, DT, &s);
        assert!(v.divisor.is_finite() && (v.divisor - 4.0).abs() < 1e-9);
    }

    #[test]
    fn relative_sensitivity_zero_keeps_look_unscaled() {
        let s = ZoomSettings { relative_sensitivity: 0.0, ..settings() };
        let mut z = Zoomer::default();
        let mut v = z.update(true, 0, DT, &s);
        for _ in 0..200 {
            v = z.update(true, 0, DT, &s);
        }
        assert_eq!(v.look_scale, 1.0);
        assert!(v.fov_scale < 1.0);
    }

    #[test]
    fn the_zoom_key_holds_and_releases() {
        let mut z = Zoomer::default();
        let s = settings();
        assert!(frame(&mut z, &s, &[], &[], true).is_none(), "idle sends nothing");
        let mut view = None;
        for _ in 0..120 {
            view = frame(&mut z, &s, &[], &["KeyC"], true);
        }
        assert!((view.unwrap().divisor - 4.0).abs() < 1e-6);
        let mut frames = 0;
        while frame(&mut z, &s, &[], &[], true).is_some() {
            frames += 1;
            assert!(frames < 200, "the zoom never ended");
        }
        assert!(frame(&mut z, &s, &[], &[], true).is_none());
    }

    #[test]
    fn toggle_mode_flips_on_each_press() {
        let mut z = Zoomer::default();
        let s = ZoomSettings { key_mode: KeyMode::Toggle, ..settings() };
        assert!(frame(&mut z, &s, &["KeyC"], &[], true).is_some());
        for _ in 0..10 {
            assert!(frame(&mut z, &s, &[], &[], true).is_some(), "stays on without the key");
        }
        frame(&mut z, &s, &["KeyC"], &[], true);
        assert!(!z.zooming);
    }

    #[test]
    fn a_menu_or_the_switch_stops_the_zoom() {
        let mut z = Zoomer::default();
        let s = settings();
        for _ in 0..30 {
            frame(&mut z, &s, &[], &["KeyC"], true);
        }
        // With a menu open the key does nothing and the zoom eases out.
        assert!(frame(&mut z, &s, &[], &["KeyC"], false).is_some());
        assert!(!z.zooming);
        // Switched off: the key is ignored.
        let off = ZoomSettings { enabled: false, ..settings() };
        let mut z = Zoomer::default();
        assert!(frame(&mut z, &off, &["KeyC"], &["KeyC"], true).is_none());
    }

    #[test]
    fn step_keys_work_only_while_enabled() {
        let mut z = Zoomer::default();
        let mut s = settings();
        for _ in 0..5 {
            frame(&mut z, &s, &[], &["KeyC"], true);
        }
        frame(&mut z, &s, &["Equal"], &["KeyC"], true);
        frame(&mut z, &s, &["Equal"], &["KeyC"], true);
        frame(&mut z, &s, &["Minus"], &["KeyC"], true);
        assert_eq!(z.steps(), 1);
        s.step_keys = false;
        frame(&mut z, &s, &["Equal"], &["KeyC"], true);
        assert_eq!(z.steps(), 0, "no steps without the step keys");
    }

    #[test]
    fn the_touch_button_toggles_the_zoom() {
        let mut z = Zoomer::default();
        let s = settings();
        let tap = |z: &mut Zoomer| z.frame(&s, &Input { active: true, pressed: &[], held: &[], tap: true, scroll: 0, seconds: DT });
        assert!(tap(&mut z).is_some());
        assert!(z.zooming);
        tap(&mut z);
        assert!(!z.zooming);
    }
}
