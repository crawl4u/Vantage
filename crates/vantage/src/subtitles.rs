//! Audio Subtitles: a short list of the sounds you just heard, with an arrow for the side they came from (like the
//! game's own subtitles). Pure logic, no host calls: `guest.rs` feeds it the sounds the host announces.

/// How long a caption stays, in seconds.
pub const LIFE: f64 = 3.0;
/// The most captions shown at once.
pub const SHOWN: usize = 6;
/// Sounds farther than this many blocks are not listed.
pub const RANGE: f64 = 24.0;

/// Where a sound came from, from the player's point of view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Here,
    Front,
    Behind,
    Left,
    Right,
}

/// One line of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub text: String,
    pub side: Side,
}

impl Row {
    /// The line as drawn: "< Zombie hurts", "Zombie hurts >", or just the caption.
    pub fn line(&self) -> String {
        match self.side {
            Side::Left => format!("< {}", self.text),
            Side::Right => format!("{} >", self.text),
            _ => self.text.clone(),
        }
    }
}

/// The side of `sound` as seen from `player` looking along the Bedrock `yaw` (0 faces south, 90 west).
pub fn side(player: [f64; 3], yaw: f64, sound: [f64; 3]) -> Side {
    let (dx, dz) = (sound[0] - player[0], sound[2] - player[2]);
    if dx * dx + dz * dz < 2.25 {
        return Side::Here;
    }
    let a = yaw.to_radians();
    let (forward, right) = ((-a.sin() * dx + a.cos() * dz), (-a.cos() * dx - a.sin() * dz));
    let angle = right.atan2(forward).to_degrees();
    if angle.abs() <= 45.0 {
        Side::Front
    } else if angle.abs() >= 135.0 {
        Side::Behind
    } else if angle > 0.0 {
        Side::Right
    } else {
        Side::Left
    }
}

fn title(word: &str) -> String {
    word.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            chars.next().map_or(String::new(), |c| c.to_uppercase().collect::<String>() + chars.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// What a sound definition ("mob.zombie.hurt", "entity.zombie.hurt", "step.grass") is called on screen; none for the
/// ones not worth listing.
pub fn caption(name: &str) -> Option<String> {
    let name = name.rsplit(':').next().unwrap_or(name);
    let parts: Vec<&str> = name.split('.').filter(|p| !p.is_empty()).collect();
    let first = *parts.first()?;
    match first {
        "step" => return Some("Footsteps".to_owned()),
        "dig" | "block" if parts.len() == 2 => return Some("Block broken".to_owned()),
        "ui" | "music" | "record" => return None,
        _ => {}
    }
    const CATEGORY: [&str; 8] = ["mob", "entity", "game", "random", "block", "item", "ambient", "liquid"];
    const VERBS: [(&str, &str); 22] = [
        ("say", "calls"),
        ("ambient", "calls"),
        ("hurt", "hurts"),
        ("death", "dies"),
        ("die", "dies"),
        ("step", "steps"),
        ("attack", "attacks"),
        ("explode", "explodes"),
        ("fuse", "hisses"),
        ("splash", "splashes"),
        ("open", "opens"),
        ("close", "closes"),
        ("click", "clicks"),
        ("pop", "pops"),
        ("burp", "burps"),
        ("eat", "eats"),
        ("drink", "drinks"),
        ("shoot", "shoots"),
        ("hit", "hits"),
        ("break", "breaks"),
        ("place", "placed"),
        ("fizz", "fizzes"),
    ];
    let words: Vec<&str> = parts.iter().copied().filter(|p| !CATEGORY.contains(p)).collect();
    let (subject, verb) = match words.as_slice() {
        [] => return None,
        [one] => match VERBS.iter().find(|(key, _)| key == one) {
            Some((_, text)) => return Some(format!("Sound {text}")),
            None => (title(one), None),
        },
        [subject, rest @ ..] => {
            let last = rest.last().copied().unwrap_or("");
            (title(subject), VERBS.iter().find(|(key, _)| *key == last).map(|(_, text)| *text).or(Some(last)))
        }
    };
    Some(match verb {
        Some(verb) => format!("{subject} {}", verb.replace('_', " ")),
        None => subject,
    })
}

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    text: String,
    side: Side,
    born: f64,
}

/// The sounds heard lately.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Subtitles {
    entries: Vec<Entry>,
}

impl Subtitles {
    /// A sound announced by the host. Ones with no place, out of range, or not worth a line are left out; a caption
    /// heard again only refreshes its line.
    pub fn hear(&mut self, now: f64, name: &str, at: Option<[f64; 3]>, player: [f64; 3], yaw: f64) {
        let Some(at) = at else { return };
        let (dx, dy, dz) = (at[0] - player[0], at[1] - player[1], at[2] - player[2]);
        if dx * dx + dy * dy + dz * dz > RANGE * RANGE {
            return;
        }
        let Some(text) = caption(name) else { return };
        let side = side(player, yaw, at);
        match self.entries.iter_mut().find(|e| e.text == text) {
            Some(entry) => {
                entry.born = now;
                entry.side = side;
            }
            None => self.entries.push(Entry { text, side, born: now }),
        }
    }

    /// Forgets what has faded.
    pub fn expire(&mut self, now: f64) {
        self.entries.retain(|e| now - e.born < LIFE);
    }

    /// The lines to show, newest first, at most `SHOWN`.
    pub fn rows(&self) -> Vec<Row> {
        let mut entries: Vec<&Entry> = self.entries.iter().collect();
        entries.sort_by(|a, b| b.born.total_cmp(&a.born));
        entries.into_iter().take(SHOWN).map(|e| Row { text: e.text.clone(), side: e.side }).collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captions_read_like_the_games() {
        assert_eq!(caption("mob.zombie.hurt").as_deref(), Some("Zombie hurts"));
        assert_eq!(caption("entity.creeper.death").as_deref(), Some("Creeper dies"));
        assert_eq!(caption("step.grass").as_deref(), Some("Footsteps"));
        assert_eq!(caption("dig.stone").as_deref(), Some("Block broken"));
        assert_eq!(caption("mob.cow.say").as_deref(), Some("Cow calls"));
        assert_eq!(caption("music.menu"), None);
        assert_eq!(caption("random.click").as_deref(), Some("Sound clicks"));
    }

    #[test]
    fn sides_follow_where_you_look() {
        // Facing south (+z): a sound at +x is on the left (west is on the right).
        let p = [0.0, 0.0, 0.0];
        assert_eq!(side(p, 0.0, [0.0, 0.0, 10.0]), Side::Front);
        assert_eq!(side(p, 0.0, [0.0, 0.0, -10.0]), Side::Behind);
        assert_eq!(side(p, 0.0, [-10.0, 0.0, 0.0]), Side::Right);
        assert_eq!(side(p, 0.0, [10.0, 0.0, 0.0]), Side::Left);
        // Facing west (yaw 90): north (-z) is on the right.
        assert_eq!(side(p, 90.0, [0.0, 0.0, -10.0]), Side::Right);
        assert_eq!(side(p, 0.0, [0.5, 0.0, 0.5]), Side::Here);
    }

    #[test]
    fn the_list_keeps_the_latest_and_forgets_the_old() {
        let mut s = Subtitles::default();
        let p = [0.0, 0.0, 0.0];
        s.hear(0.0, "mob.zombie.hurt", Some([10.0, 0.0, 0.0]), p, 0.0);
        s.hear(1.0, "mob.cow.say", Some([0.0, 0.0, 10.0]), p, 0.0);
        s.hear(1.5, "mob.zombie.hurt", Some([-10.0, 0.0, 0.0]), p, 0.0);
        s.hear(1.5, "mob.cow.say", None, p, 0.0);
        s.hear(1.5, "mob.pig.say", Some([100.0, 0.0, 0.0]), p, 0.0);
        let rows = s.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].line(), "Zombie hurts >");
        s.expire(4.1);
        assert_eq!(s.rows().len(), 1);
        s.expire(10.0);
        assert!(s.rows().is_empty());
    }
}
