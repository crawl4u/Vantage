//! Smooth values: a critically damped spring that moves a number toward a target without overshoot.
//! Time comes from the caller, so tests step it by hand.

/// A number that eases toward a target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    value: f32,
    velocity: f32,
    target: f32,
    /// How fast it settles: larger is quicker (the response time is about `4 / speed` seconds).
    speed: f32,
}

impl Spring {
    #[must_use]
    pub fn new(value: f32, speed: f32) -> Self {
        Self { value, velocity: 0.0, target: value, speed: speed.max(0.1) }
    }

    #[must_use]
    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Jumps to `value` with no motion.
    pub fn snap(&mut self, value: f32) {
        self.value = value;
        self.target = value;
        self.velocity = 0.0;
    }

    /// Whether it has reached its target.
    #[must_use]
    pub fn settled(&self) -> bool {
        (self.value - self.target).abs() < 1e-3 && self.velocity.abs() < 1e-3
    }

    /// Moves `dt` seconds on, using the exact solution of a critically damped spring, so any step is stable.
    pub fn step(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        let w = self.speed;
        let x = self.value - self.target;
        let e = (-w * dt).exp();
        let a = x + (self.velocity + w * x) * dt;
        self.value = self.target + a * e;
        self.velocity = (self.velocity - (self.velocity + w * x) * w * dt) * e;
        if self.settled() {
            self.value = self.target;
            self.velocity = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spring_reaches_its_target_without_overshooting_whatever_the_step() {
        for dt in [0.001, 0.016, 0.1, 1.0] {
            let mut s = Spring::new(0.0, 20.0);
            s.set_target(1.0);
            let mut prev = 0.0;
            for _ in 0..((6.0 / dt) as usize + 1) {
                s.step(dt);
                assert!(s.value() <= 1.0 + 1e-4, "overshoot at dt {dt}");
                assert!(s.value() >= prev - 1e-4, "went backwards at dt {dt}");
                prev = s.value();
            }
            assert!(s.settled() && s.value() == 1.0, "dt {dt} did not settle");
        }
    }

    #[test]
    fn snapping_stops_all_motion() {
        let mut s = Spring::new(0.0, 10.0);
        s.set_target(5.0);
        s.step(0.05);
        s.snap(2.0);
        assert!(s.settled() && s.value() == 2.0);
    }
}
