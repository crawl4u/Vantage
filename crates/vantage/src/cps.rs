//! Clicks per second: presses within the last second, on the frame clock the host provides.

/// The window CPS is counted over.
const WINDOW: f64 = 1.0;
/// More presses than this in a second are not a human clicking; the buffer stays bounded.
const MAX_PRESSES: usize = 64;

#[derive(Debug, Default)]
pub struct Counter {
    /// Times of recent presses, oldest first.
    presses: Vec<f64>,
}

impl Counter {
    /// Records `count` presses at `now`, forgets those older than the window, and returns
    /// how many remain.
    pub fn tick(&mut self, now: f64, count: usize) -> u32 {
        for _ in 0..count {
            if self.presses.len() >= MAX_PRESSES {
                self.presses.remove(0);
            }
            self.presses.push(now);
        }
        let keep = self.presses.iter().position(|time| now - time < WINDOW).unwrap_or(self.presses.len());
        self.presses.drain(..keep);
        self.presses.len() as u32
    }
}

/// How long a click's two reports may be apart and still be the one click.
const PAIR_WINDOW: f64 = 0.25;

/// One click reaches the mod twice: as a key press in the controls and as the game's resolved attack or use action, on the
/// same frame or a frame apart (nightly 157 delivers them a frame apart). A touch button only has the second report.
/// This pairs the two reports of one side, so a click counts once however they arrive.
#[derive(Debug, Default)]
pub struct Pairer {
    /// Reports of each kind already counted and still waiting for their other half, oldest first.
    keys: Vec<f64>,
    actions: Vec<f64>,
}

impl Pairer {
    /// This frame's key presses and resolved actions at `now`; returns how many clicks they make.
    pub fn merge(&mut self, now: f64, keys: usize, actions: usize) -> usize {
        self.keys.retain(|time| now - time < PAIR_WINDOW);
        self.actions.retain(|time| now - time < PAIR_WINDOW);
        let mut clicks = 0;
        for _ in 0..actions {
            if self.keys.is_empty() {
                self.actions.push(now);
                clicks += 1;
            } else {
                self.keys.remove(0);
            }
        }
        for _ in 0..keys {
            if self.actions.is_empty() {
                self.keys.push(now);
                clicks += 1;
            } else {
                self.actions.remove(0);
            }
        }
        self.keys.truncate(MAX_PRESSES);
        self.actions.truncate(MAX_PRESSES);
        clicks
    }
}

/// Statistics over a session: the average CPS while clicking, the highest CPS and the clicks made.
#[derive(Debug, Default)]
pub struct Stats {
    /// Sum of the CPS seen on frames that had any clicking, and how many such frames.
    sum: u64,
    frames: u64,
    max: u32,
    total: u64,
}

impl Stats {
    /// One frame: the CPS shown now and the clicks made this frame.
    pub fn note(&mut self, shown: u32, clicks: usize) {
        self.total += clicks as u64;
        if shown > 0 {
            self.sum += u64::from(shown);
            self.frames += 1;
            self.max = self.max.max(shown);
        }
    }

    /// The average in tenths (0 before any click).
    pub fn average_tenths(&self) -> u32 {
        if self.frames == 0 { 0 } else { ((self.sum * 10 + self.frames / 2) / self.frames) as u32 }
    }

    pub fn max(&self) -> u32 {
        self.max
    }

    pub fn total(&self) -> u32 {
        self.total.min(u64::from(u32::MAX)) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_reported_twice_counts_once() {
        let mut pairer = Pairer::default();
        // The key press, then the action a frame later.
        assert_eq!(pairer.merge(1.00, 1, 0), 1);
        assert_eq!(pairer.merge(1.01, 0, 1), 0);
        // The same frame.
        assert_eq!(pairer.merge(2.00, 1, 1), 1);
        // The action first, then the key.
        assert_eq!(pairer.merge(3.00, 0, 1), 1);
        assert_eq!(pairer.merge(3.01, 1, 0), 0);
        // A touch press has no key.
        assert_eq!(pairer.merge(4.00, 0, 1), 1);
        // Two fast clicks, each reported twice, a frame apart.
        assert_eq!(pairer.merge(5.00, 2, 0), 2);
        assert_eq!(pairer.merge(5.01, 0, 2), 0);
        // An old report does not swallow a new click.
        assert_eq!(pairer.merge(6.00, 1, 0), 1);
        assert_eq!(pairer.merge(7.00, 0, 1), 1);
    }

    #[test]
    fn statistics_follow_the_clicking_frames() {
        let mut stats = Stats::default();
        stats.note(0, 0);
        stats.note(4, 1);
        stats.note(7, 2);
        stats.note(0, 0);
        assert_eq!((stats.average_tenths(), stats.max(), stats.total()), (55, 7, 3));
    }

    #[test]
    fn presses_count_for_one_second_then_expire() {
        let mut counter = Counter::default();
        assert_eq!(counter.tick(0.0, 1), 1);
        assert_eq!(counter.tick(0.3, 2), 3);
        assert_eq!(counter.tick(0.9, 0), 3);
        assert_eq!(counter.tick(1.05, 0), 2, "the first press has aged out");
        assert_eq!(counter.tick(2.0, 0), 0);
    }

    #[test]
    fn the_buffer_never_grows_without_bound() {
        let mut counter = Counter::default();
        assert_eq!(counter.tick(0.0, 500), MAX_PRESSES as u32);
    }
}
