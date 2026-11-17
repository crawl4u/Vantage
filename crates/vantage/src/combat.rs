//! Reach and combo, worked out from the host's combat evidence.
//!
//! The host reports three things a mod can use (`player-state.read-combat`): an attack of yours
//! that it let through to the server (with the distance to the target), an actor that was hurt,
//! and your own hurts. It does not say who hurt whom, and an admitted attack is what the client
//! sent, not what the server accepted. So a hit counts as *landed* when the actor you attacked
//! is hurt a moment later, which is the best the evidence allows; a hit from a teammate or a
//! splash at the same moment can still be mistaken for yours. Reach and combo are readouts only.

/// What happened, reduced to what is used here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// You attacked `target`; the distance to it, if the host knew it.
    Attack,
    /// An actor was hurt (yours when `local`).
    Hurt,
    Death,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub kind: Kind,
    /// The event is about you (you attacked, you were hurt).
    pub local: bool,
    /// The actor concerned: the target of an attack, the one hurt.
    pub target: u64,
    pub distance: Option<f32>,
}

/// How long after an attack the hurt of its target still counts as its result, in seconds.
const CONFIRM_WINDOW: f64 = 0.6;
/// A combo ends when no hit lands for this long, in seconds.
const COMBO_TIMEOUT: f64 = 2.0;
/// After the target of a combo dies, its final count stays up this long, in seconds.
const KILL_LINGER: f64 = 3.0;
/// The reach readout falls back to 0 when no hit has landed for this long, in seconds.
const REACH_TIMEOUT: f64 = 3.0;
/// A death counts as your kill when its actor was hit this recently, in seconds.
const KILL_WINDOW: f64 = 6.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tracker {
    /// The last attack not yet matched to a hurt: target, when, distance.
    pending: Option<(u64, f64, Option<f32>)>,
    /// Hits in a row on `combo_target`, and when the last one landed.
    combo: u32,
    combo_target: Option<u64>,
    last_hit: f64,
    /// The distance of the last hit that landed.
    reach: Option<f32>,
    /// Hits that landed, for the whole session.
    pub hits: u32,
    /// Attacks you made, kills (an actor you hit just before died) and your own deaths, for the
    /// session. The host does not say who killed whom, so a kill is the death of an actor you hit
    /// within `KILL_WINDOW`.
    pub swings: u32,
    pub kills: u32,
    pub deaths: u32,
    /// The last actor hit and when.
    last_victim: Option<(u64, f64)>,
}

impl Tracker {
    /// Takes one event at time `now` (seconds on the mod's clock).
    pub fn event(&mut self, event: Event, now: f64) {
        if event.kind == Kind::Death {
            if event.local {
                self.deaths = self.deaths.saturating_add(1);
                self.last_victim = None;
            } else if self.last_victim.is_some_and(|(target, when)| target == event.target && now - when <= KILL_WINDOW) {
                self.kills = self.kills.saturating_add(1);
                self.last_victim = None;
            }
        }
        match event.kind {
            Kind::Attack if event.local => {
                self.swings = self.swings.saturating_add(1);
                self.pending = Some((event.target, now, event.distance));
            }
            Kind::Hurt if event.local => {
                // You were hit: the streak is over.
                self.combo = 0;
                self.combo_target = None;
            }
            Kind::Hurt => {
                if let Some((target, when, distance)) = self.pending {
                    if target == event.target && now - when <= CONFIRM_WINDOW {
                        self.landed(target, now, distance);
                    }
                }
            }
            Kind::Death if self.combo_target == Some(event.target) && self.combo > 0 => {
                // The kill is not the end of the readout: the count stays up for a moment, as if
                // a hit had just landed that long before the timeout.
                self.last_hit = now + (KILL_LINGER - COMBO_TIMEOUT);
            }
            _ => {}
        }
    }

    fn landed(&mut self, target: u64, now: f64, distance: Option<f32>) {
        self.pending = None;
        self.hits += 1;
        self.last_victim = Some((target, now));
        if self.combo_target == Some(target) && self.combo > 0 && now - self.last_hit <= COMBO_TIMEOUT {
            self.combo += 1;
        } else {
            self.combo = 1;
            self.combo_target = Some(target);
        }
        self.last_hit = now;
        if distance.is_some() {
            self.reach = distance;
        }
    }

    /// Lets a stale attack and a finished combo go as time passes.
    pub fn tick(&mut self, now: f64) {
        if self.pending.is_some_and(|(_, when, _)| now - when > CONFIRM_WINDOW) {
            self.pending = None;
        }
        if self.combo > 0 && now - self.last_hit > COMBO_TIMEOUT {
            self.combo = 0;
            self.combo_target = None;
        }
        if self.reach.is_some() && now - self.last_hit > REACH_TIMEOUT {
            self.reach = None;
        }
    }

    /// The hits in a row now.
    pub fn combo(&self) -> u32 {
        self.combo
    }

    /// The distance of the last hit that landed, in hundredths of a block; 0 when there has been
    /// no hit for a few seconds (and before the first one).
    pub fn reach_hundredths(&self) -> u32 {
        self.reach.filter(|r| r.is_finite() && *r >= 0.0).map_or(0, |r| (r * 100.0).round().min(99_999.0) as u32)
    }

    /// Hits per hundred attacks (0 before the first attack).
    pub fn accuracy(&self) -> u32 {
        if self.swings == 0 {
            0
        } else {
            (u64::from(self.hits.min(self.swings)) * 100 / u64::from(self.swings)) as u32
        }
    }

    /// Forgets everything (a new world).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attack(target: u64, distance: f32) -> Event {
        Event { kind: Kind::Attack, local: true, target, distance: Some(distance) }
    }
    fn hurt(target: u64) -> Event {
        Event { kind: Kind::Hurt, local: false, target, distance: None }
    }
    fn me_hurt() -> Event {
        Event { kind: Kind::Hurt, local: true, target: 1, distance: None }
    }

    #[test]
    fn a_hurt_after_an_attack_is_a_hit_with_its_reach() {
        let mut t = Tracker::default();
        t.event(attack(7, 2.96), 1.0);
        assert_eq!(t.combo(), 0, "an attack alone is not a hit");
        t.event(hurt(7), 1.1);
        assert_eq!((t.combo(), t.reach_hundredths(), t.hits), (1, 296, 1));
    }

    #[test]
    fn a_hurt_of_someone_else_or_too_late_is_not_yours() {
        let mut t = Tracker::default();
        t.event(attack(7, 3.0), 1.0);
        t.event(hurt(8), 1.1);
        t.event(hurt(7), 1.9);
        assert_eq!(t.combo(), 0);
        assert_eq!(t.reach_hundredths(), 0);
    }

    #[test]
    fn hits_in_a_row_make_a_combo_until_you_are_hit_or_it_times_out() {
        let mut t = Tracker::default();
        for i in 0..3 {
            let now = 1.0 + f64::from(i) * 0.5;
            t.event(attack(7, 3.0), now);
            t.event(hurt(7), now + 0.1);
        }
        assert_eq!(t.combo(), 3);
        t.event(me_hurt(), 3.0);
        assert_eq!(t.combo(), 0);
        t.event(attack(7, 3.0), 4.0);
        t.event(hurt(7), 4.1);
        assert_eq!(t.combo(), 1);
        t.tick(6.5);
        assert_eq!(t.combo(), 0, "no hit for two seconds");
    }

    #[test]
    fn pvp_counts_swings_hits_kills_and_deaths() {
        let mut t = Tracker::default();
        t.event(attack(7, 3.0), 1.0);
        t.event(hurt(7), 1.1);
        t.event(attack(7, 3.0), 1.5);
        assert_eq!((t.swings, t.hits, t.accuracy()), (2, 1, 50));
        t.event(Event { kind: Kind::Death, local: false, target: 8, distance: None }, 2.0);
        assert_eq!(t.kills, 0, "someone you did not hit");
        t.event(Event { kind: Kind::Death, local: false, target: 7, distance: None }, 2.0);
        assert_eq!(t.kills, 1);
        t.event(Event { kind: Kind::Death, local: true, target: 1, distance: None }, 9.0);
        assert_eq!(t.deaths, 1);
    }

    #[test]
    fn a_new_target_starts_a_new_combo() {
        let mut t = Tracker::default();
        t.event(attack(7, 3.0), 1.0);
        t.event(hurt(7), 1.1);
        t.event(attack(9, 3.0), 1.3);
        t.event(hurt(9), 1.4);
        assert_eq!(t.combo(), 1);
        t.event(Event { kind: Kind::Death, local: false, target: 9, distance: None }, 1.5);
        assert_eq!(t.combo(), 1, "the kill's count stays up");
        t.tick(4.4);
        assert_eq!(t.combo(), 1, "for three seconds after the kill");
        t.tick(4.6);
        assert_eq!(t.combo(), 0);
    }

    #[test]
    fn reach_starts_at_zero_and_returns_to_it_after_a_while() {
        let mut t = Tracker::default();
        assert_eq!(t.reach_hundredths(), 0, "nothing hit yet");
        t.event(attack(7, 2.96), 1.0);
        t.event(hurt(7), 1.1);
        t.tick(3.0);
        assert_eq!(t.reach_hundredths(), 296, "still recent");
        t.tick(4.2);
        assert_eq!(t.reach_hundredths(), 0, "no hit for three seconds");
    }

    #[test]
    fn one_attack_lands_once() {
        let mut t = Tracker::default();
        t.event(attack(7, 3.0), 1.0);
        t.event(hurt(7), 1.1);
        t.event(hurt(7), 1.2);
        assert_eq!(t.hits, 1);
    }
}
