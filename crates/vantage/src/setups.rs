//! Saved setups: the whole configuration kept in a few slots, swapped between, and shared as a
//! short code (see `code.rs`; a code looks like `VNT1-4F7QK-M2X9A-…`).
//!
//! A slot holds the code of the setup. The host gives a mod no clipboard or file access, only
//! single-line text fields of at most 256 characters, so a code that is longer (a setup with many
//! elements changed a lot) is cut into numbered parts that are pasted one after the other, in any
//! order.
//!
//! A frame has little fuel, so saving, loading, making a code and importing run as small steps,
//! one per frame: the options, then each HUD element on its own.

use crate::code::{self, Reader, Writer};
use crate::config::{Element, Settings};
use crate::share;

pub const SLOTS: usize = 5;
/// The most characters of one part's text (the field takes 256, a part's header adds up to 7).
pub const PART_CHARS: usize = 240;
/// The most parts a shared code may have.
pub const MAX_PARTS: usize = 10;
/// The longest code kept in a slot or shared (characters): the heaviest setup is about 1,100.
pub const MAX_PAYLOAD: usize = 1_500;
/// The settings file may not grow past this (the host refuses 16 KiB and over, and a refused save
/// would lose the session's changes).
pub const MAX_FILE: usize = 15_600;

/// The most characters of a slot's name.
pub const NAME_CHARS: usize = 16;

/// A slot name as kept: no control characters, trimmed, at most `NAME_CHARS` characters.
pub fn clean_name(name: &str) -> String {
    name.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(NAME_CHARS).collect()
}

/// A code as the texts for the fields: itself when it fits one, else numbered parts
/// (`P2/3:VNT1-…`) cut between groups.
pub fn parts(code: &str) -> Vec<String> {
    if code.len() <= 256 {
        return vec![code.to_owned()];
    }
    let mut bodies: Vec<String> = Vec::new();
    for group in code.split('-') {
        match bodies.last_mut() {
            Some(last) if last.len() + 1 + group.len() <= PART_CHARS => {
                last.push('-');
                last.push_str(group);
            }
            _ => bodies.push(group.to_owned()),
        }
    }
    let n = bodies.len();
    bodies.into_iter().enumerate().map(|(k, body)| format!("P{}/{n}:{body}", k + 1)).collect()
}

/// A pasted part: (its number from 1, how many parts there are, its text).
pub fn read_part(text: &str) -> Option<(usize, usize, &str)> {
    let rest = text.trim().strip_prefix(['P', 'p'])?;
    let (k, rest) = rest.split_once('/')?;
    let (n, body) = rest.split_once(':')?;
    let (k, n) = (k.parse::<usize>().ok()?, n.parse::<usize>().ok()?);
    (k >= 1 && k <= n && n <= MAX_PARTS && !body.trim().is_empty()).then_some((k, n, body.trim()))
}

/// What a press on the page asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Ask {
    Save(usize),
    Export,
    Load(usize),
    Import(String),
    Undo,
}

/// A job under way: one `step` per frame.
struct Run {
    ask: Ask,
    stage: usize,
    writer: Option<Writer>,
    reader: Option<Reader>,
    /// A load: the settings being read. A shared code that leaves mods out: the copy it is made from.
    next: Option<Box<Settings>>,
    /// The mods left out (their numbers in `share::MODS`): by the code being made, or by the code being loaded.
    skip: Vec<u8>,
    /// Those of them still to be dealt with, a few a frame (`MODS_PER_STEP`).
    left: Vec<u8>,
    /// The copy to share is ready, or the loaded settings are read to their end.
    ready: bool,
}

/// How many left-out mods one frame deals with: copying a mod's settings compares many names, and a frame has
/// little fuel.
const MODS_PER_STEP: usize = 2;

/// The steps of a save or a load: stage 0 is the options, then one per first element, then the end.
/// The elements added later are written with the extras at the end (see `code.rs`).
const END: usize = 1 + Element::FIRST;

#[derive(Default)]
pub struct Setups {
    /// The slot the buttons act on.
    pub slot: usize,
    run: Option<Run>,
    ask: Option<Ask>,
    /// The code being shown (its parts) and which part the field holds.
    out: Vec<String>,
    out_at: usize,
    /// The parts pasted so far, by number.
    incoming: Vec<Option<String>>,
    /// The settings from before the last load or import, for taking it back.
    undo: Option<Box<Settings>>,
    /// Texts the page shows; rebuilt by `refresh`.
    pub labels: [String; SLOTS],
    /// The selected slot's name, for the name field.
    pub name: String,
    pub code: String,
    pub import_label: String,
    pub undo_label: String,
    /// A message for the toast, taken by the panel.
    message: Option<String>,
    /// The page texts are up to date.
    fresh: bool,
}

enum Step {
    More,
    /// The same stage again next frame.
    Again,
    Done(bool),
}

impl Setups {
    /// The message for the toast, once.
    pub fn take_message(&mut self) -> Option<String> {
        self.message.take()
    }

    pub fn select(&mut self, slot: usize) {
        self.slot = slot.min(SLOTS - 1);
        self.fresh = false;
    }

    /// Asks for the selected slot to be filled with the current settings.
    pub fn save(&mut self) {
        self.ask = Some(Ask::Save(self.slot));
    }

    pub fn load(&mut self, settings: &Settings) {
        if settings.profiles.get(self.slot).is_some_and(|p| !p.is_empty()) {
            self.ask = Some(Ask::Load(self.slot));
        } else {
            self.message = Some("That slot is empty".to_owned());
        }
    }

    pub fn delete(&mut self, settings: &mut Settings) -> bool {
        match settings.profiles.get_mut(self.slot) {
            Some(slot) if !slot.is_empty() => {
                slot.clear();
                if let Some(name) = settings.names.get_mut(self.slot) {
                    name.clear();
                }
                self.message = Some(format!("Slot {} cleared", self.slot + 1));
                self.fresh = false;
                true
            }
            _ => false,
        }
    }

    /// Makes the code. A code in several parts steps to its next part on each press, and makes
    /// the code afresh after the last one.
    pub fn export(&mut self) {
        if self.out_at + 1 < self.out.len() {
            self.out_at += 1;
            self.message = Some(format!("Part {} of {}", self.out_at + 1, self.out.len()));
            self.fresh = false;
        } else {
            self.ask = Some(Ask::Export);
        }
    }

    pub fn undo(&mut self) {
        if self.undo.is_some() {
            self.ask = Some(Ask::Undo);
        } else {
            self.message = Some("Nothing to undo".to_owned());
        }
    }

    /// Sets the selected slot's name; true when the settings changed.
    pub fn rename(&mut self, settings: &mut Settings, text: &str) -> bool {
        let name = clean_name(text);
        settings.names.resize(SLOTS, String::new());
        self.fresh = false;
        if settings.names[self.slot] == name {
            return false;
        }
        self.message = Some(if name.is_empty() { format!("Slot {} name cleared", self.slot + 1) } else { format!("Slot {} is now \"{name}\"", self.slot + 1) });
        settings.names[self.slot] = name;
        true
    }

    /// The status button under the paste field: starts a half-pasted code over, else says how.
    pub fn info(&mut self) {
        self.message = Some(if self.incoming.iter().any(Option::is_some) {
            self.incoming.clear();
            "Pasted parts cleared".to_owned()
        } else {
            "Tap the field above, paste the code, press Enter".to_owned()
        });
        self.fresh = false;
    }

    /// A text committed in the code field: a whole code, or a part of one.
    pub fn paste(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        if let Some((k, n, body)) = read_part(text) {
            if self.incoming.len() != n {
                self.incoming = vec![None; n];
            }
            self.incoming[k - 1] = Some(body.to_owned());
            if self.incoming.iter().all(Option::is_some) {
                let whole = self.incoming.iter().flatten().map(String::as_str).collect::<Vec<_>>().join("-");
                self.incoming.clear();
                self.ask = Some(Ask::Import(whole));
            }
        } else if text.get(..code::PREFIX.len()).is_some_and(|p| p.eq_ignore_ascii_case(code::PREFIX)) {
            self.incoming.clear();
            self.ask = Some(Ask::Import(text.to_owned()));
        } else {
            self.message = Some("That is not a Vantage code".to_owned());
        }
        self.fresh = false;
    }

    /// Rebuilds the page's texts; true when they changed.
    pub fn refresh(&mut self, settings: &Settings) -> bool {
        if self.fresh {
            return false;
        }
        self.fresh = true;
        for (n, label) in self.labels.iter_mut().enumerate() {
            let full = settings.profiles.get(n).is_some_and(|p| !p.is_empty());
            let name = settings.names.get(n).filter(|t| !t.is_empty());
            let what = name.map(String::as_str).unwrap_or(if full { "saved" } else { "empty" });
            *label = format!("{}Slot {}: {what}", if n == self.slot { "> " } else { "" }, n + 1);
        }
        self.code = self.out.get(self.out_at).cloned().unwrap_or_default();
        self.name = settings.names.get(self.slot).cloned().unwrap_or_default();
        self.import_label = match self.incoming.iter().filter(|p| p.is_some()).count() {
            0 => "Tap the field above and paste".to_owned(),
            got => format!("Got {got} of {}: tap to start over", self.incoming.len()),
        };
        self.undo_label = if self.undo.is_some() { "Undo last load".to_owned() } else { "Nothing to undo".to_owned() };
        true
    }

    /// Runs one small step of the job under way (or starts the one asked for). Returns what
    /// changed: (the settings or the saved slots changed, the page's texts changed).
    pub fn work(&mut self, settings: &mut Settings) -> (bool, bool) {
        if self.run.is_none() {
            let Some(ask) = self.ask.take() else {
                return (false, false);
            };
            if ask == Ask::Undo {
                if let Some(before) = self.undo.take() {
                    let (kept, names) = (std::mem::take(&mut settings.profiles), std::mem::take(&mut settings.names));
                    *settings = *before;
                    settings.profiles = kept;
                    settings.names = names;
                    self.message = Some("Setup restored".to_owned());
                    self.fresh = false;
                    return (true, true);
                }
                return (false, false);
            }
            self.run = Some(Run { ask, stage: 0, writer: None, reader: None, next: None, skip: Vec::new(), left: Vec::new(), ready: false });
        }
        let Some(mut run) = self.run.take() else {
            return (false, false);
        };
        match self.step(&mut run, settings) {
            Step::More => {
                run.stage += 1;
                self.run = Some(run);
                (false, false)
            }
            Step::Again => {
                self.run = Some(run);
                (false, false)
            }
            Step::Done(changed) => {
                self.fresh = false;
                (changed, true)
            }
        }
    }

    fn fail(&mut self, text: &str) -> Step {
        self.message = Some(text.to_owned());
        Step::Done(false)
    }

    fn step(&mut self, run: &mut Run, settings: &mut Settings) -> Step {
        let saving = matches!(run.ask, Ask::Save(_) | Ask::Export);
        // A code to share can leave mods out (the Sharing page): it is made from a copy of the settings with
        // those mods at their defaults. The copy takes a frame, and the mods a few frames more.
        if run.stage == 0 && run.ask == Ask::Export && !run.ready {
            match run.next.as_mut() {
                None => {
                    run.skip = share::clean(&settings.share_skip);
                    if run.skip.is_empty() {
                        run.ready = true;
                    } else {
                        run.left = run.skip.clone();
                        run.next = Some(Box::new(settings.clone()));
                        return Step::Again;
                    }
                }
                Some(part) => {
                    let defaults = Settings::default();
                    for _ in 0..MODS_PER_STEP {
                        if let Some(n) = run.left.pop() {
                            share::copy(part, &defaults, share::MODS[usize::from(n)]);
                        }
                    }
                    run.ready = run.left.is_empty();
                    return Step::Again;
                }
            }
        }
        if run.stage == 0 {
            if saving {
                let mut writer = Writer::new();
                writer.skip = run.skip.clone();
                code::put_options(&mut writer, run.next.as_deref().unwrap_or(&*settings));
                run.writer = Some(writer);
            } else {
                let text = match &run.ask {
                    Ask::Load(slot) => settings.profiles.get(*slot).cloned().unwrap_or_default(),
                    Ask::Import(text) => text.clone(),
                    _ => String::new(),
                };
                let Some(mut reader) = Reader::new(&text) else {
                    return self.fail("That code is damaged");
                };
                match code::get_options(&mut reader) {
                    Some(mut options) => {
                        options.elements.clear();
                        run.next = Some(Box::new(options));
                    }
                    None => return self.fail("That code is damaged"),
                }
                run.reader = Some(reader);
            }
            return Step::More;
        }
        if run.stage < END {
            let element = Element::ALL[run.stage - 1];
            if saving {
                if let Some(writer) = run.writer.as_mut() {
                    code::put_element(writer, element, run.next.as_deref().unwrap_or(&*settings).style(element));
                }
            } else if let (Some(reader), Some(next)) = (run.reader.as_mut(), run.next.as_mut()) {
                match code::get_element(reader, element) {
                    Some(style) => next.elements.push(style),
                    None => return self.fail("That code is damaged"),
                }
            }
            return Step::More;
        }
        // The end.
        if saving {
            let Some(mut writer) = run.writer.take() else {
                return self.fail("Could not read the settings");
            };
            // The options that came after the elements in the first codes go last.
            code::put_extras(&mut writer, run.next.as_deref().unwrap_or(&*settings));
            let text = writer.finish();
            if let Ask::Save(slot) = run.ask {
                if text.len() > MAX_PAYLOAD {
                    return self.fail("That setup is too large to keep");
                }
                settings.profiles.resize(SLOTS, String::new());
                settings.profiles[slot] = text;
                self.message = Some(format!("Saved to slot {}", slot + 1));
                return Step::Done(true);
            }
            self.out = parts(&text);
            self.out_at = 0;
            let made = if run.skip.is_empty() { "Code made".to_owned() } else { format!("Code made with {} of {} mods", share::MODS.len() - run.skip.len(), share::MODS.len()) };
            self.message = Some(if self.out.len() > 1 { format!("{made} in {} parts: press again for the next", self.out.len()) } else { made });
            return Step::Done(false);
        }
        // A load or import keeps the saved slots, the menu key (a code from someone else must not take the
        // menu away) and the list of mods left out of the codes made here.
        if !run.ready {
            let Some(next) = run.next.as_mut() else {
                return self.fail("That code is damaged");
            };
            // A code made before the extras has none: they stay at their defaults.
            if let Some(reader) = run.reader.as_mut() {
                if code::get_extras(reader, next).is_none() {
                    return self.fail("That code is damaged");
                }
            }
            let version = run.reader.as_ref().map_or(0, Reader::version);
            next.menu_key = settings.menu_key.clone();
            next.menu_light = settings.menu_light;
            next.share_skip = settings.share_skip.clone();
            // The person's own lists are not in a code, so a load must not empty them: marked spots, starred mods
            // and the colour picker's recent and saved colours.
            next.favorites = settings.favorites.clone();
            next.hidden_mods = settings.hidden_mods.clone();
            next.card_order = settings.card_order.clone();
            next.gameplay.waypoints = settings.gameplay.waypoints.clone();
            next.gameplay.recent_colors = settings.gameplay.recent_colors.clone();
            next.gameplay.favorite_colors = settings.gameplay.favorite_colors.clone();
            // A code made before version 8 has no Hive requeue, Perspective or Scoreboard header options: a load keeps the
            // person's own, so it cannot switch them off. From version 8 on they travel in the code like everything else.
            if version < 8 {
                let (now, then) = (&mut next.gameplay, &settings.gameplay);
                (now.hive_on, now.hive_end, now.hive_elim, now.hive_solo) = (then.hive_on, then.hive_end, then.hive_elim, then.hive_solo);
                (now.hive_check, now.hive_hub, now.hive_log) = (then.hive_check, then.hive_hub, then.hive_log);
                (now.persp_on, now.persp_snap_toggle) = (then.persp_on, then.persp_snap_toggle);
                (now.sb_title, now.sb_horizontal) = (then.sb_title, then.sb_horizontal);
                now.hive_say = then.hive_say;
            }
            // Version 10 added Glass (one style for both kinds of glass, which the reader copies across): an older
            // code keeps the person's own.
            if version < 10 {
                let (now, then) = (&mut next.gameplay, &settings.gameplay);
                (now.glass_on, now.glass_mode, now.glass_coloured) = (then.glass_on, then.glass_mode, then.glass_coloured);
            }
            next.sanitize();
            // The mods the code leaves out keep the person's own settings: a few of them a frame.
            run.skip = run.reader.as_ref().map(|reader| reader.skipped().to_vec()).unwrap_or_default();
            run.left = run.skip.clone();
            run.ready = true;
            if !run.left.is_empty() {
                return Step::Again;
            }
        }
        if let Some(next) = run.next.as_mut() {
            for _ in 0..MODS_PER_STEP {
                if let Some(n) = run.left.pop() {
                    share::copy(next, &*settings, share::MODS[usize::from(n)]);
                }
            }
            if !run.left.is_empty() {
                return Step::Again;
            }
        }
        let Some(mut next) = run.next.take() else {
            return self.fail("That code is damaged");
        };
        next.profiles = std::mem::take(&mut settings.profiles);
        next.names = std::mem::take(&mut settings.names);
        let done = match &run.ask {
            Ask::Load(slot) => format!("Loaded slot {}", slot + 1),
            _ if !run.skip.is_empty() => format!("Imported {} of {} mods, the rest stay yours", share::MODS.len() - run.skip.len(), share::MODS.len()),
            _ => "Setup imported".to_owned(),
        };
        self.undo = Some(Box::new(std::mem::replace(settings, *next)));
        self.message = Some(done);
        Step::Done(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_code_is_one_text_and_a_long_one_numbered_parts() {
        let short = "VNT1-30000-10456";
        assert_eq!(parts(short), vec![short.to_owned()]);
        let long = format!("VNT1-{}", vec!["ABCDE"; 150].join("-"));
        let cut = parts(&long);
        assert!(cut.len() >= 3 && cut.iter().all(|p| p.len() <= 250), "{}", cut.len());
        assert_eq!(read_part(&cut[1]).map(|(k, n, _)| (k, n)), Some((2, cut.len())));
        let mut got = Setups::default();
        for part in cut[1..].iter().rev() {
            got.paste(part);
        }
        assert!(got.ask.is_none(), "one part is still missing");
        got.paste(&cut[0]);
        assert_eq!(got.ask, Some(Ask::Import(long)));
    }

    #[test]
    fn a_whole_code_or_junk() {
        let mut got = Setups::default();
        got.paste("hello");
        assert!(got.ask.is_none() && got.take_message().is_some());
        got.paste("vnt1-30000-10456");
        assert!(matches!(got.ask, Some(Ask::Import(_))));
        assert!(read_part("P4/3:abc").is_none() && read_part("VNT1-abc").is_none());
    }

    /// Runs the setups' work frame by frame until a page or a change is ready; returns the frames used.
    fn run(got: &mut Setups, settings: &mut Settings) -> u32 {
        let mut frames = 0;
        loop {
            let (changed, page) = got.work(settings);
            frames += 1;
            assert!(frames < 40);
            if page || changed {
                break;
            }
        }
        frames
    }

    #[test]
    fn save_export_and_load_run_in_steps() {
        let mut settings = Settings::default();
        settings.elements[0].scale = 1.7;
        settings.elements[3].opacity = 0.4;
        let want = settings.clone();
        let mut got = Setups::default();
        got.select(2);
        got.save();
        assert!(run(&mut got, &mut settings) > 2, "work is spread over frames");
        assert!(settings.profiles[2].starts_with("VNT1"));
        got.export();
        run(&mut got, &mut settings);
        assert_eq!(got.out.len(), 1);
        got.select(1);
        assert!(got.rename(&mut settings, "  Pvp\u{7} look that is far too long "));
        assert_eq!(settings.names[1], "Pvp look that is");
        got.refresh(&settings);
        assert!(got.labels[1].contains("Pvp look"));
        got.select(2);
        settings.elements[0].scale = 1.0;
        got.load(&settings);
        run(&mut got, &mut settings);
        assert_eq!(settings.elements, want.elements);
        got.undo();
        run(&mut got, &mut settings);
        assert_eq!(settings.elements[0].scale, 1.0);
        got.paste(&got.out[0].to_lowercase());
        run(&mut got, &mut settings);
        assert_eq!(settings.elements, want.elements);
    }

    #[test]
    fn a_shared_code_leaves_mods_out_and_an_import_keeps_your_own() {
        let run = |got: &mut Setups, settings: &mut Settings| {
            for _ in 0..200 {
                let (changed, page) = got.work(settings);
                if page || changed {
                    return;
                }
            }
            panic!("the job never ended");
        };
        let fps = share::position("hud_fps").unwrap() as u8;
        let glass = share::position("gp_glass_on").unwrap() as u8;
        // Theirs: FPS and Glass changed, and both left out of the code; the Clock changed and shared.
        let mut theirs = Settings::default();
        theirs.style_mut(Element::Fps).scale = 2.0;
        theirs.gameplay.glass_on = true;
        theirs.set_key("gp_glass_on", "KeyG");
        theirs.style_mut(Element::Clock).scale = 1.5;
        theirs.share_skip = vec![glass, fps];
        let before = theirs.clone();
        let mut got = Setups::default();
        got.export();
        run(&mut got, &mut theirs);
        assert_eq!(theirs, before, "making a code changes nothing");
        assert!(got.take_message().unwrap().contains("62 of 64"));
        let code = got.out[0].clone();
        // A saved slot always holds everything.
        got.select(0);
        got.save();
        run(&mut got, &mut theirs);
        assert!(code::decode_part(&theirs.profiles[0]).unwrap().1.is_empty());
        // Mine: my own FPS size and Glass key stay, their Clock arrives, and my own list of left-out mods stays.
        let mut mine = Settings::default();
        mine.style_mut(Element::Fps).scale = 0.75;
        mine.set_key("gp_glass_on", "KeyH");
        mine.share_skip = vec![3];
        let mut got = Setups::default();
        got.paste(&code);
        run(&mut got, &mut mine);
        assert_eq!(mine.style(Element::Fps).scale, 0.75);
        assert_eq!(mine.key("gp_glass_on"), "KeyH");
        assert!(!mine.gameplay.glass_on);
        assert_eq!(mine.style(Element::Clock).scale, 1.5);
        assert_eq!(mine.share_skip, vec![3]);
        assert!(got.take_message().unwrap().contains("62 of 64"));
    }

    #[test]
    fn an_import_keeps_your_waypoints_stars_and_colour_lists() {
        let mut theirs = Settings::default();
        theirs.style_mut(Element::Clock).scale = 1.5;
        let mut got = Setups::default();
        got.export();
        run(&mut got, &mut theirs);
        let code = got.out[0].clone();
        let mut mine = Settings::default();
        mine.favorites = vec!["hud_fps".to_owned()];
        mine.gameplay.recent_colors = vec![1, 2];
        mine.gameplay.favorite_colors = vec![3];
        mine.gameplay.waypoints = vec![crate::waypoints::Waypoint { name: "Home".to_owned(), world: String::new(), dimension: 0, pos: [1, 64, 2], color: 0, visible: true, group: String::new() }];
        let mut got = Setups::default();
        got.paste(&code);
        run(&mut got, &mut mine);
        assert_eq!(mine.style(Element::Clock).scale, 1.5);
        assert_eq!(mine.favorites, vec!["hud_fps".to_owned()]);
        assert_eq!(mine.gameplay.recent_colors, vec![1, 2]);
        assert_eq!(mine.gameplay.favorite_colors, vec![3]);
        assert_eq!(mine.gameplay.waypoints.len(), 1);
    }
}
