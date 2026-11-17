//! The clocks behind the Stopwatch, Playtime and Speed elements. Plain Rust with no host calls, so
//! the rules (when the Stopwatch starts, what Playtime counts, how Speed is smoothed) are tested
//! here; `guest` feeds them the frame's time and position.

/// The Stopwatch: a time and whether it is counting.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stopwatch {
    /// Seconds counted.
    time: f64,
    running: bool,
    /// Whether the element was shown at the last step: switching it on is what starts it.
    was_on: bool,
    /// Switching the element on has started it, or the player has used a key on it. A stopwatch
    /// that keeps running starts by itself only the first time it is shown.
    touched: bool,
}

impl Stopwatch {
    /// One step of `seconds`. `on` is whether the element is switched on, `keep` the "Keep running
    /// when off" option.
    ///
    /// Without `keep`, switching the element on starts it from zero and switching it off throws the
    /// time away (as it always did). With `keep`, the time and whether it is counting stay as they
    /// are while the element is off, and it goes on counting there.
    pub fn step(&mut self, seconds: f64, on: bool, keep: bool) {
        let seconds = if seconds.is_finite() { seconds.max(0.0) } else { 0.0 };
        if on && !self.was_on && (!keep || !self.touched) {
            self.time = 0.0;
            self.running = true;
            self.touched = true;
        }
        if !on && !keep {
            self.time = 0.0;
            self.running = false;
        }
        self.was_on = on;
        if self.running && (on || keep) {
            self.time += seconds;
        }
    }

    /// The start / pause key.
    pub fn toggle(&mut self) {
        self.running = !self.running;
        self.touched = true;
    }

    /// The reset key: back to zero, still counting or still paused.
    pub fn reset(&mut self) {
        self.time = 0.0;
        self.touched = true;
    }

    pub fn running(&self) -> bool {
        self.running
    }

    /// Whole seconds, as shown.
    pub fn seconds(&self) -> u32 {
        self.time.min(f64::from(u32::MAX)) as u32
    }
}

/// Playtime: the time since Vantage started, and the part of it spent in a world.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Playtime {
    total: f64,
    in_world: f64,
}

impl Playtime {
    pub fn step(&mut self, seconds: f64, in_world: bool) {
        let seconds = if seconds.is_finite() { seconds.max(0.0) } else { 0.0 };
        self.total += seconds;
        if in_world {
            self.in_world += seconds;
        }
    }

    /// Whole seconds, as shown: only the time in a world, or all of it.
    pub fn seconds(&self, world_only: bool) -> u32 {
        let t = if world_only { self.in_world } else { self.total };
        t.min(f64::from(u32::MAX)) as u32
    }
}

/// A jump this long between two samples (blocks) is a teleport, a respawn or another dimension,
/// not speed.
const JUMP: f64 = 40.0;
/// The shortest time between two samples (seconds): a longer one steadies the number.
const SAMPLE: f64 = 0.25;

/// Speed in blocks per second, from the player's position now and then.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpeedMeter {
    speed: Option<f64>,
    /// The last sample: where, and the clock then.
    from: Option<([f64; 3], f64)>,
    /// Whether that sample counted height.
    vertical: bool,
}

impl SpeedMeter {
    /// Adds the position `here` at `clock` seconds. With `vertical` climbing, falling and flying
    /// count; without, only movement along the ground does.
    pub fn sample(&mut self, here: [f64; 3], clock: f64, vertical: bool) {
        if self.vertical != vertical {
            // Another way of measuring: the old sample says nothing about it.
            self.vertical = vertical;
            self.from = None;
        }
        let here = if vertical { here } else { [here[0], 0.0, here[2]] };
        match self.from {
            Some((from, at)) if clock - at >= SAMPLE => {
                let moved = ((here[0] - from[0]).powi(2) + (here[1] - from[1]).powi(2) + (here[2] - from[2]).powi(2)).sqrt();
                let wanted = if moved > JUMP { 0.0 } else { moved / (clock - at) };
                // Eased, so the number does not jitter.
                self.speed = Some(self.speed.map_or(wanted, |old| old * 0.5 + wanted * 0.5));
                self.from = Some((here, clock));
            }
            Some(_) => {}
            None => self.from = Some((here, clock)),
        }
    }

    /// Forgets everything (the element is off).
    pub fn clear(&mut self) {
        self.speed = None;
        self.from = None;
    }

    pub fn blocks_per_second(&self) -> Option<f64> {
        self.speed
    }
}

/// A block is a metre: km/h is blocks per second times 3.6.
const KMH: f64 = 3.6;
/// The most a speed shows as, in its own unit and decimals.
const MOST: f64 = 9_999_999.0;

/// The speed as the HUD is told it: in the unit asked for, times ten to the `decimals`, rounded
/// (56 with one decimal is 5.6). Rounding here, not when writing, means the view only changes when
/// the number on screen does.
pub fn speed_display(blocks_per_second: f64, kmh: bool, decimals: usize) -> u32 {
    let unit = if kmh { KMH } else { 1.0 };
    let scale = 10f64.powi(decimals.min(3) as i32);
    let v = blocks_per_second * unit * scale;
    if v.is_finite() { v.max(0.0).round().min(MOST) as u32 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(watch: &mut Stopwatch, seconds: u32, on: bool, keep: bool) {
        for _ in 0..seconds {
            watch.step(1.0, on, keep);
        }
    }

    #[test]
    fn switching_the_stopwatch_on_starts_it_and_switching_it_off_throws_the_time_away() {
        let mut w = Stopwatch::default();
        run(&mut w, 5, false, false);
        assert_eq!((w.seconds(), w.running()), (0, false), "off: nothing counts");
        run(&mut w, 3, true, false);
        assert_eq!((w.seconds(), w.running()), (3, true), "switching it on starts it at zero, and that step counts");
        run(&mut w, 4, false, false);
        assert_eq!((w.seconds(), w.running()), (0, false));
        run(&mut w, 2, true, false);
        assert_eq!(w.seconds(), 2, "on again: from zero");
    }

    #[test]
    fn the_keys_pause_resume_and_reset_it() {
        let mut w = Stopwatch::default();
        run(&mut w, 10, true, false);
        assert_eq!(w.seconds(), 10);
        w.toggle();
        assert!(!w.running());
        run(&mut w, 30, true, false);
        assert_eq!(w.seconds(), 10, "paused");
        w.toggle();
        run(&mut w, 5, true, false);
        assert_eq!(w.seconds(), 15, "resumed where it was");
        w.reset();
        assert_eq!((w.seconds(), w.running()), (0, true), "a running watch restarts from zero");
        run(&mut w, 2, true, false);
        w.toggle();
        w.reset();
        run(&mut w, 9, true, false);
        assert_eq!((w.seconds(), w.running()), (0, false), "a paused watch stays paused at zero");
    }

    #[test]
    fn a_kept_stopwatch_goes_on_counting_while_it_is_off() {
        let mut w = Stopwatch::default();
        run(&mut w, 10, true, true);
        assert_eq!(w.seconds(), 10);
        run(&mut w, 60, false, true);
        assert_eq!((w.seconds(), w.running()), (70, true), "hidden, it counts on");
        run(&mut w, 1, true, true);
        assert_eq!(w.seconds(), 71, "switched on again: not started over");
        // Paused, it stays paused and keeps its time while hidden.
        w.toggle();
        run(&mut w, 20, false, true);
        assert_eq!((w.seconds(), w.running()), (71, false));
        run(&mut w, 1, true, true);
        assert_eq!((w.seconds(), w.running()), (71, false), "shown again, still paused");
    }

    #[test]
    fn a_kept_stopwatch_can_be_started_and_reset_with_the_keys_while_hidden() {
        let mut w = Stopwatch::default();
        run(&mut w, 5, false, true);
        assert_eq!((w.seconds(), w.running()), (0, false), "hidden and never started: it waits");
        w.toggle();
        run(&mut w, 8, false, true);
        assert_eq!(w.seconds(), 8);
        w.reset();
        run(&mut w, 3, false, true);
        assert_eq!(w.seconds(), 3);
        run(&mut w, 1, true, true);
        assert_eq!((w.seconds(), w.running()), (4, true), "shown: it carries on, it does not start from zero");
    }

    #[test]
    fn a_kept_stopwatch_starts_by_itself_the_first_time_it_is_shown() {
        let mut w = Stopwatch::default();
        run(&mut w, 100, false, true);
        run(&mut w, 4, true, true);
        assert_eq!((w.seconds(), w.running()), (4, true), "it starts at zero when first shown, and hidden time before that is not counted");
    }

    #[test]
    fn turning_keep_off_again_throws_away_what_was_kept() {
        let mut w = Stopwatch::default();
        run(&mut w, 10, true, true);
        run(&mut w, 10, false, true);
        assert_eq!(w.seconds(), 20);
        run(&mut w, 1, false, false);
        assert_eq!((w.seconds(), w.running()), (0, false));
    }

    #[test]
    fn bad_steps_change_nothing() {
        let mut w = Stopwatch::default();
        run(&mut w, 2, true, false);
        let before = w;
        w.step(f64::NAN, true, false);
        w.step(-5.0, true, false);
        w.step(f64::INFINITY, true, false);
        assert_eq!(w, before);
        let mut p = Playtime::default();
        p.step(f64::NAN, true);
        p.step(-1.0, true);
        assert_eq!(p.seconds(false), 0);
    }

    #[test]
    fn playtime_counts_everything_or_only_the_time_in_a_world() {
        let mut p = Playtime::default();
        p.step(30.0, false);
        p.step(90.0, true);
        p.step(10.5, false);
        assert_eq!(p.seconds(false), 130);
        assert_eq!(p.seconds(true), 90);
        p.step(70.0, true);
        assert_eq!((p.seconds(false), p.seconds(true)), (200, 160));
    }

    /// Walks `blocks` along each axis in `ticks` samples of a quarter second.
    fn walk(meter: &mut SpeedMeter, from: [f64; 3], per_second: [f64; 3], seconds: u32, vertical: bool) -> [f64; 3] {
        let mut at = from;
        meter.sample(at, 0.0, vertical);
        for i in 1..=seconds * 4 {
            let t = f64::from(i) * 0.25;
            at = [from[0] + per_second[0] * t, from[1] + per_second[1] * t, from[2] + per_second[2] * t];
            meter.sample(at, t, vertical);
        }
        at
    }

    #[test]
    fn speed_is_blocks_per_second_along_the_ground_unless_height_counts() {
        let mut flat = SpeedMeter::default();
        walk(&mut flat, [0.0, 64.0, 0.0], [3.0, 0.0, 4.0], 6, false);
        assert!((flat.blocks_per_second().unwrap() - 5.0).abs() < 0.01);
        // Falling straight down is no speed along the ground...
        let mut ground = SpeedMeter::default();
        walk(&mut ground, [0.0, 200.0, 0.0], [0.0, -12.0, 0.0], 6, false);
        assert!(ground.blocks_per_second().unwrap().abs() < 0.001);
        // ...but it is with height.
        let mut all = SpeedMeter::default();
        walk(&mut all, [0.0, 200.0, 0.0], [0.0, -12.0, 0.0], 6, true);
        assert!((all.blocks_per_second().unwrap() - 12.0).abs() < 0.01);
        let mut diagonal = SpeedMeter::default();
        walk(&mut diagonal, [0.0, 0.0, 0.0], [2.0, 6.0, 3.0], 6, true);
        assert!((diagonal.blocks_per_second().unwrap() - 7.0).abs() < 0.01, "2, 6 and 3 make 7");
    }

    #[test]
    fn a_teleport_is_not_speed_and_changing_the_measure_starts_a_new_sample() {
        let mut m = SpeedMeter::default();
        walk(&mut m, [0.0, 64.0, 0.0], [4.0, 0.0, 0.0], 4, false);
        m.sample([5000.0, 64.0, 0.0], 100.0, false);
        m.sample([5000.0, 64.0, 0.0], 100.5, false);
        assert!(m.blocks_per_second().unwrap() < 4.0, "the jump counted as standing still");
        let mut v = SpeedMeter::default();
        v.sample([0.0, 100.0, 0.0], 0.0, false);
        // Height was ignored in the first sample: switching it on must not read the whole climb.
        v.sample([0.0, 130.0, 0.0], 1.0, true);
        assert_eq!(v.blocks_per_second(), None, "no number from a sample taken another way");
        v.sample([0.0, 131.0, 0.0], 2.0, true);
        assert!((v.blocks_per_second().unwrap() - 1.0).abs() < 0.001);
        v.clear();
        assert_eq!(v.blocks_per_second(), None);
    }

    #[test]
    fn speed_is_rounded_for_the_unit_and_the_decimals() {
        assert_eq!(speed_display(5.6, false, 1), 56);
        assert_eq!(speed_display(5.64, false, 1), 56);
        assert_eq!(speed_display(5.66, false, 1), 57);
        assert_eq!(speed_display(5.6, false, 0), 6);
        assert_eq!(speed_display(5.6, false, 2), 560);
        assert_eq!(speed_display(5.6, true, 1), 202, "5.6 blocks a second is 20.16 km/h");
        assert_eq!(speed_display(5.6, true, 3), 20_160);
        assert_eq!(speed_display(4.3175, false, 3), 4318);
        assert_eq!(speed_display(0.0, true, 3), 0);
        assert_eq!(speed_display(-1.0, false, 1), 0);
        assert_eq!(speed_display(f64::NAN, false, 1), 0);
        assert_eq!(speed_display(1e12, true, 3), 9_999_999);
        assert_eq!(speed_display(5.6, false, 9), 5600, "no more than three decimals");
    }
}
