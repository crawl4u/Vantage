//! The Hive auto requeue's host calls: reading chat, and sending `/connection`, `/q <mode>` or `/hub`.
//! What to do with them is in [`crate::hive`], which is tested natively.
//!
//! Three host limits trap (and so quarantine) the whole mod, so all three are kept here:
//! - a callback may make 8 player-state reads; every such read in the mod goes through [`counted`], this runs after
//!   the rest of the frame and reads chat only when the frame has reads to spare;
//! - a callback may ask for 4 commands; the Auto Text Hotkey calls [`note_command`], and this sends only with room;
//! - a callback may make 8 HUD writes (a chat line of the mod's own is one, like the HUD's surface update), so the
//!   lines said here wait in a queue and at most [`LINES_A_FRAME`] go out each frame.

use crate::gameplay::GameplaySettings;
use crate::hive::{self, Action, Hive, Options};
use crate::fuel::{self, Stage};
use crate::host::{budget, gameplay, hud as host_hud, player_state};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

/// The player-state reads the host allows one callback, less one kept spare.
const READS_ROOM: u32 = 7;
/// The commands the host allows one callback, less one kept spare.
const COMMANDS_ROOM: u32 = 3;
/// Seconds between chat reads once the old chat is read.
const READ_EVERY: f64 = 0.25;
/// The longest chat line echoed by the log, in bytes.
const LOG_BYTES: usize = 180;
/// The mod's own chat lines sent in one frame, and how many may wait (the oldest go first when it is full).
const LINES_A_FRAME: usize = 2;
const LINES_WAITING: usize = 16;

thread_local! {
    /// Player-state reads and command requests made so far this frame.
    static READS: Cell<u32> = const { Cell::new(0) };
    static COMMANDS: Cell<u32> = const { Cell::new(0) };
    /// The world this frame is in: its session and whether the player is alive (none: no world).
    static WORLD: Cell<Option<(u64, bool)>> = const { Cell::new(None) };
    /// The requeue key was pressed this frame.
    static MANUAL: Cell<bool> = const { Cell::new(false) };
    static GLUE: RefCell<Glue> = RefCell::new(Glue::default());
    /// The chat lines waiting to be said.
    static LINES: RefCell<VecDeque<String>> = const { RefCell::new(VecDeque::new()) };
}

#[derive(Default)]
struct Glue {
    hive: Hive,
    /// Whether the feature has started in this session, and the session.
    started: bool,
    session: u64,
    /// The newest chat line read.
    after: u64,
    /// The chat retained from before is being read past: the mode is learned from it, nothing is acted on.
    priming: bool,
    next_read: f64,
    /// Said once each: "on", a chat read failure, a missing permission.
    said_on: bool,
    said_chat: bool,
    said_permission: bool,
}

/// Starts a frame's counts.
pub fn new_frame() {
    READS.set(0);
    COMMANDS.set(0);
    WORLD.set(None);
}

/// Makes one player-state read and counts it. Every `player_state::read_*` call in the mod goes through here.
pub fn counted<T>(read: impl FnOnce() -> T) -> T {
    READS.set(READS.get().saturating_add(1));
    read()
}

thread_local! {
    /// Whether the frame's fuel ran short in this session (said once).
    static FUEL_SAID: Cell<bool> = const { Cell::new(false) };
}

/// The fuel guard (see [`crate::fuel`]): whether the frame has the fuel left to start `stage`. A refusal is said once
/// in chat, since a mod that quietly skips work looks broken. One cheap host call.
pub fn fuel_allows(stage: Stage) -> bool {
    if fuel::allows(budget::remaining_fuel(), stage) {
        return true;
    }
    if !FUEL_SAID.replace(true) {
        let _ = host_hud::local_message("Vantage: a frame ran short of fuel, so some extras were delayed to keep the mod running.");
    }
    false
}

/// Counts a command request made elsewhere in the frame.
pub fn note_command() {
    COMMANDS.set(COMMANDS.get().saturating_add(1));
}

/// The world the frame read: its session and the player's life state.
pub fn note_world(session: u64, alive: bool) {
    WORLD.set(Some((session, alive)));
}

/// The requeue key was pressed.
pub fn press_requeue() {
    MANUAL.set(true);
}

thread_local! {
    /// Whether the Hive's status lines are said (the "Show chat messages" switch).
    static CHATTY: Cell<bool> = const { Cell::new(true) };
}

/// A status line, said only while the Show chat messages switch is on.
fn say(text: &str) {
    if CHATTY.get() {
        warn(text);
    }
}

/// A line that is always said: what stops the requeue working (a permission refused).
fn warn(text: &str) {
    queue(format!("Vantage Hive: {text}"));
}

fn queue(line: String) {
    LINES.with_borrow_mut(|lines| {
        if lines.len() >= LINES_WAITING {
            lines.pop_front();
        }
        lines.push_back(line);
    });
}

/// Says the waiting lines, a few a frame. A line the host has no room for stays for the next frame.
fn flush() {
    LINES.with_borrow_mut(|lines| {
        for _ in 0..LINES_A_FRAME {
            let Some(line) = lines.pop_front() else { return };
            if host_hud::local_message(&line).is_err() {
                lines.push_front(line);
                return;
            }
        }
    });
}

fn note(text: Option<String>) {
    if let Some(text) = text {
        say(&text);
    }
}

/// A chat line as the log shows it: `§` written as `&` (a local line may not hold `§`), no control characters, cut
/// to `LOG_BYTES` on a character boundary.
fn loggable(text: &str) -> String {
    let mut out = String::with_capacity(text.len().min(LOG_BYTES));
    for c in text.chars() {
        let c = if c == '§' { '&' } else if c.is_control() { ' ' } else { c };
        if out.len() + c.len_utf8() > LOG_BYTES {
            break;
        }
        out.push(c);
    }
    out
}

/// One frame of the auto requeue, after everything else in the frame. Off, it costs one comparison.
pub fn frame(settings: &GameplaySettings, now: f64) {
    CHATTY.set(settings.hive_say);
    let manual = MANUAL.replace(false);
    if !settings.hive_on {
        GLUE.with_borrow_mut(|glue| {
            if glue.started {
                *glue = Glue::default();
                LINES.with_borrow_mut(VecDeque::clear);
            }
        });
        return;
    }
    GLUE.with_borrow_mut(|glue| {
        let Some((session, alive)) = WORLD.get() else {
            // No world this frame (loading, or the frame ended early): the next life reading is not a death.
            return;
        };
        let o = Options { end: settings.hive_end, eliminated: settings.hive_elim, solo: settings.hive_solo, check: settings.hive_check, hub: settings.hive_hub };
        if !glue.started || glue.session != session {
            // A new session may number its chat lines from the start again: read from the top, acting on nothing
            // that was already there.
            glue.started = true;
            glue.session = session;
            glue.after = 0;
            glue.priming = true;
            glue.hive.forget_life();
        }
        if manual {
            note(glue.hive.manual(now, o));
        }
        if !glue.priming {
            note(glue.hive.alive(Some(alive), now, o));
        }
        if READS.get() < READS_ROOM && (glue.priming || now >= glue.next_read) {
            glue.read_chat(settings.hive_log, now, o);
        }
        if COMMANDS.get() < COMMANDS_ROOM {
            glue.act(now, o);
        }
    });
    flush();
}

impl Glue {
    fn read_chat(&mut self, log: bool, now: f64, o: Options) {
        // The log writes a chat line of its own for each one read, and only four fit a callback.
        let limit: u32 = if log && !self.priming { 2 } else { 4 };
        let after = self.after;
        let batch = match counted(|| player_state::read_chat(after, limit)) {
            Ok(batch) => batch,
            Err(why) => {
                if !std::mem::replace(&mut self.said_chat, true) {
                    warn(&format!("cannot read chat ({why}). Allow the Player status permission"));
                }
                self.next_read = now + 1.0;
                return;
            }
        };
        for message in &batch {
            self.after = self.after.max(message.sequence);
            let line = hive::read_line(&message.text);
            if self.priming {
                if let Some(line) = line {
                    self.hive.learn(line);
                }
                continue;
            }
            if log && !message.text.trim_start().starts_with("Vantage") {
                queue(format!("Vantage Hive log: {}", loggable(&message.text)));
            }
            if let Some(line) = line {
                note(self.hive.line(line, now, o));
            }
        }
        if batch.len() >= limit as usize {
            // More may be waiting: read again next frame.
            return;
        }
        if self.priming {
            self.priming = false;
            // Life readings start now: whatever state the player joined in is not a death.
            self.hive.forget_life();
            if !std::mem::replace(&mut self.said_on, true) {
                let text = match &self.hive.mode {
                    Some(mode) => format!("on, watching chat (mode {mode}). This line is said once; the mod stays on"),
                    None => "on, watching chat. This line is said once; the mod stays on".to_owned(),
                };
                say(&text);
            }
        }
        self.next_read = now + READ_EVERY;
    }

    fn act(&mut self, now: f64, o: Options) {
        let (action, said) = self.hive.due(now, o);
        note(said);
        let Some(action) = action else { return };
        let command = match &action {
            Action::Ask => hive::MODE_COMMAND.to_owned(),
            Action::Send(command) => command.clone(),
        };
        note_command();
        match gameplay::request_command(&command) {
            Ok(()) => match action {
                Action::Ask => self.hive.asked(now),
                Action::Send(_) => {
                    self.hive.sent(now);
                    say(&format!("sent {command}"));
                }
            },
            Err(why) => {
                self.hive.refused(now);
                // A screen owning input or the rate limit passes by itself; a missing permission does not.
                if (why.contains("denied") || why.contains("not granted")) && !std::mem::replace(&mut self.said_permission, true) {
                    warn(&format!("{command} was refused ({why}). In Settings > Mods allow Vantage's Commands permission (q, connection, hub)"));
                }
            }
        }
    }
}
