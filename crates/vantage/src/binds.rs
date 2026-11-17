//! The player's own key bindings for the actions Keystrokes and CPS show.
//!
//! Keystrokes used to watch W, A, S, D, the left and right mouse buttons and Space, and CPS only
//! the two mouse buttons, so anyone who remapped movement, swapped the mouse buttons or attacks
//! with a key saw dead boxes and a CPS of zero. The host lists the player's chords by action
//! (`input.read-bindings`); this module turns that list into the physical keys to watch for
//! each of the seven slots, and into the short names the key cells show.
//!
//! The slots keep the names of the keys they started with (`KEY_NAMES` in `hud.rs`): slot 4 is
//! "MouseLeft" there but follows the *attack* binding, slot 5 follows *use*, slot 6 *jump*.

/// How many actions are watched: forward, left, back, right, attack, use, jump.
pub const SLOTS: usize = 7;
/// Slots with a name in their cell (everything but jump, which shows a bar).
pub const LABELLED: usize = 6;
pub const ATTACK: usize = 4;
pub const USE: usize = 5;

/// The host's action name for each slot.
const ACTIONS: [&str; SLOTS] = ["move-forward", "move-left", "move-backward", "move-right", "attack", "use", "jump"];
/// What each slot watches until the player's bindings have been read (the game's defaults).
const DEFAULTS: [&str; SLOTS] = ["KeyW", "KeyA", "KeyS", "KeyD", "MouseLeft", "MouseRight", "Space"];
/// Most chords of one action that are watched together (a second binding is a spare key).
const PER_ACTION: usize = 4;
/// Entries a page of bindings holds at most (`MAX_INPUT_BINDINGS_PER_READ`).
pub const PAGE: u32 = 16;

/// One chord, reduced to what is used here.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub action: String,
    pub key: String,
    /// Bound for playing, not for menus.
    pub gameplay: bool,
    /// No Shift, Control, Alt or Super needed with it.
    pub plain: bool,
}

/// The physical keys behind each slot.
#[derive(Clone, Debug, PartialEq)]
pub struct Binds {
    keys: [Vec<String>; SLOTS],
    /// The sprint and sneak keys, which the toggle modes watch (the host resets its own toggles when a screen opens).
    sprint: Vec<String>,
    sneak: Vec<String>,
}

impl Default for Binds {
    fn default() -> Self {
        Self { keys: std::array::from_fn(|slot| vec![DEFAULTS[slot].to_owned()]), sprint: vec!["ControlLeft".to_owned()], sneak: vec!["ShiftLeft".to_owned()] }
    }
}

impl Binds {
    /// The slots from a complete list of chords. An action with no plain gameplay chord is
    /// unbound: its slot watches nothing and its cell says so.
    pub fn from_entries(entries: &[Entry]) -> Self {
        let keys = std::array::from_fn(|slot| {
            let mut keys: Vec<String> = Vec::new();
            for entry in entries {
                if entry.action == ACTIONS[slot] && entry.gameplay && entry.plain && !keys.contains(&entry.key) && keys.len() < PER_ACTION {
                    keys.push(entry.key.clone());
                }
            }
            keys
        });
        let named = |action: &str| -> Vec<String> {
            let mut keys: Vec<String> = Vec::new();
            for entry in entries {
                if entry.action == action && entry.gameplay && entry.plain && !keys.contains(&entry.key) && keys.len() < PER_ACTION {
                    keys.push(entry.key.clone());
                }
            }
            keys
        };
        Self { keys, sprint: named("sprint"), sneak: named("sneak") }
    }

    /// The physical keys of the sprint action.
    pub fn sprint_keys(&self) -> &[String] {
        &self.sprint
    }

    /// The physical keys of the sneak action.
    pub fn sneak_keys(&self) -> &[String] {
        &self.sneak
    }

    /// Fresh presses among `pressed` of the given keys.
    pub fn count_presses(keys: &[String], pressed: &[String]) -> usize {
        pressed.iter().filter(|key| keys.contains(key)).count()
    }

    /// The physical keys of a slot.
    pub fn keys(&self, slot: usize) -> &[String] {
        match self.keys.get(slot) {
            Some(keys) => keys.as_slice(),
            None => &[],
        }
    }

    /// Every key watched, once each.
    pub fn all_keys(&self) -> Vec<String> {
        let mut all: Vec<String> = Vec::new();
        for key in self.keys.iter().flatten() {
            if !all.contains(key) {
                all.push(key.clone());
            }
        }
        all
    }

    /// Whether any key of the slot is among the held ones.
    pub fn is_down(&self, slot: usize, held: &[String]) -> bool {
        self.keys(slot).iter().any(|key| held.contains(key))
    }

    /// Fresh presses of the slot's keys in `pressed` (a key listed twice counts twice).
    pub fn presses(&self, slot: usize, pressed: &[String]) -> usize {
        let keys = self.keys(slot);
        pressed.iter().filter(|key| keys.contains(key)).count()
    }

    /// The names the cells show, for the slots that have one.
    pub fn labels(&self) -> [String; LABELLED] {
        std::array::from_fn(|slot| match self.keys[slot].first() {
            Some(key) => short_name(key),
            None => "-".to_owned(),
        })
    }
}

/// A short name for a physical key, small enough for a key cell (at most four characters for
/// the common keys).
pub fn short_name(key: &str) -> String {
    let fixed = match key {
        "MouseLeft" => "LMB",
        "MouseRight" => "RMB",
        "MouseMiddle" => "MMB",
        "Space" => "Spc",
        "Enter" | "NumpadEnter" => "Ent",
        "Backspace" => "Bks",
        "Tab" => "Tab",
        "Escape" => "Esc",
        "CapsLock" => "Cap",
        "ShiftLeft" | "ShiftRight" => "Shf",
        "ControlLeft" | "ControlRight" => "Ctl",
        "AltLeft" | "AltRight" => "Alt",
        "MetaLeft" | "MetaRight" | "SuperLeft" | "SuperRight" => "Sup",
        "ArrowUp" => "Up",
        "ArrowDown" => "Dn",
        "ArrowLeft" => "Lt",
        "ArrowRight" => "Rt",
        "Backquote" => "`",
        "Minus" => "-",
        "Equal" => "=",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Backslash" => "\\",
        "Semicolon" => ";",
        "Quote" => "'",
        "Comma" => ",",
        "Period" => ".",
        "Slash" => "/",
        "Insert" => "Ins",
        "Delete" => "Del",
        "PageUp" => "PgU",
        "PageDown" => "PgD",
        _ => "",
    };
    if !fixed.is_empty() {
        return fixed.to_owned();
    }
    if let Some(letter) = key.strip_prefix("Key").filter(|rest| rest.len() == 1) {
        return letter.to_owned();
    }
    if let Some(digit) = key.strip_prefix("Digit").filter(|rest| rest.len() == 1) {
        return digit.to_owned();
    }
    if let Some(digit) = key.strip_prefix("Numpad") {
        let mut out = String::from("N");
        out.extend(digit.chars().take(2));
        return out;
    }
    // F1 to F24, and anything else: its own name, cut short.
    key.chars().take(3).collect()
}

/// Reads the player's chords page by page. The host answers "unchanged" while the revision
/// last completed is still current; a change mid-way starts again from the first page.
#[derive(Debug, Default)]
pub struct Collector {
    /// The revision of the last complete list.
    done: Option<u64>,
    /// The revision being collected now.
    collecting: Option<u64>,
    offset: u32,
    entries: Vec<Entry>,
}

/// One page of the host's answer.
pub struct Page {
    pub revision: u64,
    pub total: u32,
    pub entries: Vec<Entry>,
}

impl Collector {
    /// Whether a list is half read (the next call should follow at once).
    pub fn busy(&self) -> bool {
        self.collecting.is_some()
    }

    /// What to ask the host next: the revision known (none while collecting) and the offset.
    pub fn request(&self) -> (Option<u64>, u32) {
        if self.collecting.is_some() { (None, self.offset) } else { (self.done, 0) }
    }

    /// Takes the host's answer (none: unchanged). Returns the new slots when a complete list is in.
    pub fn accept(&mut self, page: Option<Page>) -> Option<Binds> {
        let page = page?;
        if self.collecting != Some(page.revision) {
            // A new list, or the list changed under a half-read one: start from the first page.
            let mid_way = self.offset != 0;
            self.collecting = Some(page.revision);
            self.entries.clear();
            if mid_way {
                self.offset = 0;
                return None;
            }
        }
        let added = page.entries.len() as u32;
        self.offset += added;
        self.entries.extend(page.entries);
        if added == 0 || self.offset >= page.total {
            let binds = Binds::from_entries(&self.entries);
            self.done = Some(page.revision);
            self.collecting = None;
            self.offset = 0;
            self.entries.clear();
            return Some(binds);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(action: &str, key: &str) -> Entry {
        Entry { action: action.into(), key: key.into(), gameplay: true, plain: true }
    }

    #[test]
    fn defaults_are_the_games_keys() {
        let binds = Binds::default();
        assert_eq!(binds.keys(0), ["KeyW"]);
        assert_eq!(binds.keys(ATTACK), ["MouseLeft"]);
        assert_eq!(binds.keys(USE), ["MouseRight"]);
        assert_eq!(binds.keys(6), ["Space"]);
        assert_eq!(binds.labels(), ["W", "A", "S", "D", "LMB", "RMB"].map(String::from));
    }

    #[test]
    fn rebound_keys_are_followed() {
        let binds = Binds::from_entries(&[
            entry("move-forward", "ArrowUp"),
            entry("move-left", "KeyJ"),
            entry("attack", "MouseRight"),
            entry("use", "MouseLeft"),
            entry("jump", "KeyK"),
        ]);
        assert_eq!(binds.keys(0), ["ArrowUp"]);
        assert_eq!(binds.keys(ATTACK), ["MouseRight"]);
        assert_eq!(binds.labels()[0], "Up");
        assert_eq!(binds.labels()[ATTACK], "RMB");
        assert!(binds.is_down(6, &["KeyK".to_owned()]));
        assert!(!binds.is_down(6, &["Space".to_owned()]));
    }

    #[test]
    fn unbound_actions_watch_nothing_and_say_so() {
        let binds = Binds::from_entries(&[entry("move-forward", "KeyW")]);
        assert!(binds.keys(1).is_empty());
        assert_eq!(binds.labels()[1], "-");
        assert!(!binds.is_down(1, &["KeyA".to_owned()]));
    }

    #[test]
    fn menu_and_chorded_bindings_are_ignored() {
        let mut ui = entry("attack", "KeyQ");
        ui.gameplay = false;
        let mut chord = entry("attack", "KeyE");
        chord.plain = false;
        let binds = Binds::from_entries(&[ui, chord, entry("attack", "KeyX")]);
        assert_eq!(binds.keys(ATTACK), ["KeyX"]);
    }

    #[test]
    fn spare_bindings_count_too() {
        let binds = Binds::from_entries(&[entry("attack", "MouseLeft"), entry("attack", "KeyX"), entry("attack", "KeyX")]);
        assert_eq!(binds.keys(ATTACK), ["MouseLeft", "KeyX"]);
        let pressed = ["KeyX".to_owned(), "MouseLeft".to_owned(), "KeyQ".to_owned()];
        assert_eq!(binds.presses(ATTACK, &pressed), 2);
        assert_eq!(binds.all_keys(), ["MouseLeft", "KeyX"]);
    }

    #[test]
    fn short_names_fit_a_cell() {
        for (key, name) in [("KeyW", "W"), ("Digit3", "3"), ("ShiftLeft", "Shf"), ("F5", "F5"), ("Numpad7", "N7"), ("MouseMiddle", "MMB"), ("Semicolon", ";")] {
            assert_eq!(short_name(key), name, "{key}");
        }
        assert!(short_name("SomethingVeryLong").chars().count() <= 3);
    }

    #[test]
    fn collector_reads_pages_and_notices_changes() {
        let mut collector = Collector::default();
        assert_eq!(collector.request(), (None, 0));
        let page = |revision, total, keys: &[&str]| Page { revision, total, entries: keys.iter().map(|k| entry("attack", k)).collect() };
        // Two pages of a list of three.
        assert!(collector.accept(Some(page(1, 3, &["KeyA", "KeyB"]))).is_none());
        assert!(collector.busy());
        assert_eq!(collector.request(), (None, 2));
        let done = collector.accept(Some(page(1, 3, &["KeyC"]))).expect("complete");
        assert_eq!(done.keys(ATTACK).len(), 3);
        assert!(!collector.busy());
        // Unchanged: the host says none.
        assert_eq!(collector.request(), (Some(1), 0));
        assert!(collector.accept(None).is_none());
        // The list changes while the second page is being read: start again.
        assert!(collector.accept(Some(page(2, 3, &["KeyA", "KeyB"]))).is_none());
        assert!(collector.accept(Some(page(3, 3, &["KeyC"]))).is_none());
        assert_eq!(collector.request(), (None, 0));
        let again = collector.accept(Some(page(3, 1, &["KeyZ"]))).expect("complete");
        assert_eq!(again.keys(ATTACK), ["KeyZ"]);
    }
}
