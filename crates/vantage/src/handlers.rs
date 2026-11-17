//! The three mods that need the host's event handlers: No cam reset (`handle-teleport`), No swing sounds
//! (`handle-sound`) and the CPS limiter (`filter-actions`). The decisions are plain Rust so they are
//! unit-tested here; `guest.rs` only translates the host's records into these calls.
//!
//! The handlers cannot call host imports and run outside the frame, so the settings reach them through a
//! small copy (`Config`) that the frame refreshes.

/// What the handlers should do right now; the frame copies it from the settings.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Config {
    /// Keep the view when the server turns or moves you (rubber-banding, teleports).
    pub no_cam_reset: bool,
    /// Drop the sounds of an attack that hits nothing.
    pub no_swing_sounds: bool,
    /// The most attacks a second, or none for no limit.
    pub cps_limit: Option<u32>,
    /// Keep a note of the sounds that play, for Audio subtitles.
    pub subtitles: bool,
}

/// Sound definitions that are the swing of an attack that did not land. Hit sounds (`attack.strong`,
/// `attack.critical`) are never listed. Extend this list if a server's swing sound has another name.
const SWING_SOUNDS: [&str; 3] = ["game.player.attack.nodamage", "game.player.swing", "entity.player.attack.nodamage"];

/// Whether a sound is the swing of an attack and should be dropped.
pub fn is_swing_sound(name: &str) -> bool {
    SWING_SOUNDS.iter().any(|swing| name == *swing)
}

/// What an edge of the Attack or Use button is, as far as the limiter cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Press,
    Release,
    Repeat,
}

/// One transition of the host's action frame: whether it is the Attack button, and which edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    pub attack: bool,
    pub edge: Edge,
}

/// The CPS limiter: lets an attack through only when enough time has passed since the last one it let through.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CpsLimiter {
    epoch: u64,
    /// The earliest time the next attack may pass. It advances by one gap per attack that passes (not from the click's
    /// own time), so clicks landing a little early do not push every later one back and the rate holds at the limit.
    next_us: Option<u64>,
}

impl CpsLimiter {
    /// One verdict per transition (true = let it through). Releases and every non-attack action always pass;
    /// an attack press or repeat passes when at least 1/limit second has passed since the last one that did.
    /// `limit` None lets everything through.
    pub fn verdicts(&mut self, epoch: u64, time_us: u64, limit: Option<u32>, transitions: &[Transition]) -> Vec<bool> {
        if epoch != self.epoch {
            self.epoch = epoch;
            self.next_us = None;
        }
        let gap = limit.filter(|l| *l > 0).map(|l| 1_000_000 / u64::from(l));
        transitions
            .iter()
            .map(|t| {
                if !t.attack || t.edge == Edge::Release {
                    return true;
                }
                let Some(gap) = gap else { return true };
                let ready = self.next_us.is_none_or(|next| time_us >= next);
                if ready {
                    // Late by less than a gap: keep the cadence. Later (a pause) or the first one: start over.
                    self.next_us = Some(match self.next_us {
                        Some(next) if time_us.saturating_sub(next) < gap => next + gap,
                        _ => time_us + gap,
                    });
                }
                ready
            })
            .collect()
    }
}

/// How many attacks the game will see: the attack presses and repeats that were let through. The CPS element counts
/// these while the limiter is on, so it shows the limited rate and not the raw clicking.
pub fn accepted_attacks(transitions: &[Transition], accepted: &[bool]) -> u32 {
    transitions.iter().zip(accepted).filter(|(t, ok)| t.attack && t.edge != Edge::Release && **ok).count() as u32
}

/// The limits the dropdown offers, in attacks per second.
pub const CPS_LIMITS: [(&str, u32); 8] =
    [("5", 5), ("6", 6), ("8", 8), ("10", 10), ("12", 12), ("14", 14), ("16", 16), ("20", 20)];
pub const CPS_LIMIT_NAMES: [&str; 8] = ["5", "6", "8", "10", "12", "14", "16", "20"];

#[cfg(test)]
mod tests {
    use super::*;

    fn press() -> Transition {
        Transition { attack: true, edge: Edge::Press }
    }

    #[test]
    fn swing_sounds_are_dropped_but_hits_are_not() {
        assert!(is_swing_sound("game.player.attack.nodamage"));
        assert!(!is_swing_sound("game.player.attack.strong"));
        assert!(!is_swing_sound("game.player.attack.critical"));
    }

    #[test]
    fn the_limiter_spaces_attacks() {
        let mut limiter = CpsLimiter::default();
        // 10 a second: 100 ms apart.
        assert_eq!(limiter.verdicts(1, 1_000_000, Some(10), &[press()]), [true]);
        assert_eq!(limiter.verdicts(1, 1_050_000, Some(10), &[press()]), [false]);
        assert_eq!(limiter.verdicts(1, 1_100_000, Some(10), &[press()]), [true]);
    }

    #[test]
    fn releases_other_buttons_and_no_limit_always_pass() {
        let mut limiter = CpsLimiter::default();
        let release = Transition { attack: true, edge: Edge::Release };
        let use_item = Transition { attack: false, edge: Edge::Press };
        assert_eq!(limiter.verdicts(1, 10, Some(5), &[press(), release, use_item]), [true, true, true]);
        assert_eq!(limiter.verdicts(1, 11, Some(5), &[press()]), [false]);
        assert_eq!(limiter.verdicts(1, 12, None, &[press()]), [true]);
    }

    #[test]
    fn only_attacks_that_got_through_are_counted() {
        let mut limiter = CpsLimiter::default();
        let batch = [press(), press(), Transition { attack: true, edge: Edge::Release }];
        let accepted = limiter.verdicts(1, 1_000_000, Some(10), &batch);
        assert_eq!(accepted_attacks(&batch, &accepted), 1);
    }

    #[test]
    fn early_clicks_do_not_lower_the_rate() {
        let mut limiter = CpsLimiter::default();
        // Clicking every 45 ms (22 a second) against a limit of 10: one second must let 10 through, not 8.
        let mut passed = 0;
        let mut t = 1_000_000;
        while t < 2_000_000 {
            passed += limiter.verdicts(1, t, Some(10), &[press()]).iter().filter(|ok| **ok).count();
            t += 45_000;
        }
        assert!(passed >= 10, "{passed}");
    }

    #[test]
    fn a_new_epoch_forgets_the_last_attack() {
        let mut limiter = CpsLimiter::default();
        assert_eq!(limiter.verdicts(1, 100, Some(5), &[press()]), [true]);
        assert_eq!(limiter.verdicts(2, 101, Some(5), &[press()]), [true]);
    }
}
