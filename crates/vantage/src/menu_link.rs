//! Vantage's own menu hand-over: the menu key sets the whole menu on the client and opens it, and
//! once the player has closed it and the client has played its fade-out, the menu is dropped.
//! No other mod takes part.

use crate::host::panel;
use crate::mods_core::Event;

/// Frames a closed menu keeps its panel, so the client can play the fade-out. Dropping it at once
/// would remove the menu without the fade.
const CLOSE_FADE_FRAMES: u32 = 20;
/// Frames a menu set to open may take to show before it is opened again.
const RETRY_AFTER: u32 = 4;
/// Frames after which a menu that never showed is dropped.
const GIVE_UP_AFTER: u32 = 120;

/// The menu's life: not set, opening, open, and fading out.
#[derive(Default)]
pub struct Menu {
    /// The menu is set on the client (opening, open or fading out).
    has_panel: bool,
    /// The menu has been seen open since it was set.
    seen_open: bool,
    /// Frames since the menu was set to open.
    opening: u32,
    /// Frames since the player closed the menu.
    closing: u32,
    /// An open was asked for; the next `frame` sets and opens the menu.
    pending: bool,
    /// What went wrong, until taken: shown in chat so a stuck menu is not silent.
    notes: Vec<String>,
}

impl Menu {
    pub fn new() -> Self {
        Self::default()
    }

    /// What the hand-over noted since last asked.
    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }

    /// The player's menu key: closes an open menu, or asks for it to open.
    pub fn toggle(&mut self, panel_open: bool) {
        if panel_open && self.has_panel {
            let _ = panel::close();
        } else {
            self.pending = true;
        }
    }

    /// Call once per frame with whether the shown menu is open and how to build the whole menu.
    pub fn frame(&mut self, panel_open: bool, whole_panel: impl FnOnce() -> String) -> Event {
        if self.pending {
            self.pending = false;
            if !self.has_panel {
                return match panel::set_content(&whole_panel()) {
                    Err(error) => {
                        self.notes.push(format!("set_content refused: {error}"));
                        Event::Nothing
                    }
                    Ok(()) => {
                        self.has_panel = true;
                        self.seen_open = false;
                        self.opening = 0;
                        self.closing = 0;
                        if let Err(error) = panel::open() {
                            self.notes.push(format!("open refused: {error}"));
                        }
                        Event::Opened
                    }
                };
            }
            // Asked again while it fades out or waits to show: keep the panel and show it.
            self.closing = 0;
            let _ = panel::open();
            return Event::Nothing;
        }
        if !self.has_panel {
            return Event::Nothing;
        }
        if panel_open {
            self.seen_open = true;
            self.opening = 0;
            self.closing = 0;
            return Event::Nothing;
        }
        if self.seen_open {
            self.closing += 1;
            if self.closing >= CLOSE_FADE_FRAMES {
                return self.drop_panel();
            }
            return Event::Nothing;
        }
        // Set but not shown yet: ask again every few frames, and give up after a while.
        self.opening += 1;
        if self.opening >= GIVE_UP_AFTER {
            self.notes.push("menu never showed, gave up".to_owned());
            return self.drop_panel();
        }
        if self.opening % RETRY_AFTER == 0 {
            let _ = panel::open();
        }
        Event::Nothing
    }

    fn drop_panel(&mut self) -> Event {
        let was_seen = self.seen_open;
        if panel::set_content("").is_ok() {
            self.has_panel = false;
            self.seen_open = false;
            self.opening = 0;
            self.closing = 0;
        }
        if was_seen { Event::Closed } else { Event::Nothing }
    }
}
