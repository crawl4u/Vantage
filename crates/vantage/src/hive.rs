//! Hive auto requeue: when your game on The Hive is over for you, queue the next one of the same mode.
//!
//! Everything here is plain logic with no host calls, so it is tested natively (`hive_host.rs` is the glue).
//!
//! The facts come from chat lines ([`read_line`]) and the player's `alive` flag. A line only counts when it is a
//! server line: after the leading symbols it must start with the words looked for and hold no `»` further on (a
//! player's chat line is `Name » text`, so nobody can type "Game OVER!" to requeue everyone).
//!
//! The mode comes from two places: Hive's own "Finding you a game of BED-DUOS" when a queue starts, and the answer
//! to `/connection` ("You are connected to server name BED-DUOS12"). With Mode check on, the mod asks `/connection`
//! right before it queues, so a mode remembered from an earlier game can never requeue the wrong one, and a lobby is
//! never queued from.
//!
//! Lessons from the first Cinnabar auto queue (the thread "is there a working hive mod"):
//! - every step is said in chat, which stays; a HUD label that fades reads as "it turned itself off";
//! - a mode that is not known is asked for instead of waiting for a "Finding you a game" line that never comes when
//!   you join through an NPC or the mod is switched on in the middle of a game;
//! - no line is dropped for being long (colour codes make Hive's lines long);
//! - nothing waits on a party question whose answer may never come: the switches say what counts as "over".

/// Seconds from the game being over to the queue command, so the end-of-game lines land first.
pub const DELAY: f64 = 1.0;
/// How long a command the game refuses (a screen owns input, the respawn screen) is retried.
const RETRY_FOR: f64 = 15.0;
const RETRY_EVERY: f64 = 0.25;
/// No second queue for this long after one was sent: a "Game OVER!" after your own elimination is the same game.
pub const QUIET: f64 = 20.0;
/// How long the mode question is retried, and how long its answer is waited for.
const ASK_FOR: f64 = 4.0;
const ANSWER_WAIT: f64 = 2.5;
/// How far apart your death and a "FINAL KILL" line may be and still be the same death.
const DEATH_WINDOW: f64 = 1.5;
/// The command whose answer names the server (and so the mode).
pub const MODE_COMMAND: &str = "/connection";
/// The longest mode name kept (`SKY-CLASSIC-SQUADS` is 18).
const LONGEST_MODE: usize = 24;
/// Lines longer than this are not looked at (the host hands out at most 256 bytes a line).
const LONGEST_LINE: usize = 512;

/// The switches of the Hive page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// Queue when the game ends ("Game OVER!", your whole party is out, you finished the course).
    pub end: bool,
    /// Queue when you or your team are eliminated.
    pub eliminated: bool,
    /// Queue when you die in a solo game with no respawns (SkyWars, Survival Games).
    pub solo: bool,
    /// Ask `/connection` before queueing.
    pub check: bool,
    /// Go to the hub instead of queueing.
    pub hub: bool,
}

/// Why a queue was started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    GameOver,
    PartyOut,
    Finished,
    Eliminated,
    TeamOut,
    Death,
    Manual,
}

impl Why {
    fn label(self) -> &'static str {
        match self {
            Self::GameOver => "game over",
            Self::PartyOut => "your party is out",
            Self::Finished => "you finished",
            Self::Eliminated => "you were eliminated",
            Self::TeamOut => "your team was eliminated",
            Self::Death => "you died",
            Self::Manual => "requeue key",
        }
    }
}

/// What a chat line means to the auto requeue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    /// "Finding you a game of BED-DUOS...": the queue you are in.
    Finding(String),
    /// "You are connected to server name BED-DUOS12": the answer to `/connection`.
    Connected(String),
    GameOver,
    PartyOut,
    Finished,
    YouEliminated,
    /// "You are on the Red Team!": your team, as Hive writes it (lowercased).
    OnTeam(String),
    /// "... Team has been ELIMINATED!": the whole line (lowercased), to look your team up in.
    TeamOut(String),
    /// "You were killed by ..." or "You died".
    Killed,
    /// A "FINAL KILL" line: somebody is out for good.
    FinalKill,
}

/// The line without `§` formatting codes (the `§` and the character after it), lowercased.
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

/// A mode name as `/q` takes it: capitals and dashes, the server number taken off (`bed-duos12` is `BED-DUOS`).
/// None when nothing is left. Only letters and dashes are kept, so it can never carry another command.
fn mode_name(text: &str) -> Option<String> {
    let name: String = text.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').filter(|c| !c.is_ascii_digit()).map(|c| c.to_ascii_uppercase()).collect();
    let name = name.trim_matches('-');
    (!name.is_empty() && name.len() <= LONGEST_MODE).then(|| name.to_owned())
}

/// Whether a mode is a lobby (or the replay cinema): nothing to queue again from there.
pub fn is_lobby(mode: &str) -> bool {
    mode == "HUB" || mode.starts_with("HUB-") || mode == "REPLAY"
}

/// Whether dying ends your game: SkyWars and Survival Games played alone (no DUOS, TRIOS, SQUADS or MEGA).
pub fn solo_no_respawn(mode: &str) -> bool {
    let game = mode.split('-').next().unwrap_or("");
    matches!(game, "SKY" | "SG") && !["DUOS", "TRIOS", "SQUADS", "MEGA"].iter().any(|team| mode.split('-').any(|part| part == *team))
}

/// What a chat line says, if it is one the auto requeue acts on.
pub fn read_line(text: &str) -> Option<Line> {
    if text.len() > LONGEST_LINE {
        return None;
    }
    let lower = plain(text);
    // The symbols Hive puts in front (», a glyph, spaces) go; what is left must be the server speaking.
    let body = lower.trim_start_matches(|c: char| !c.is_alphanumeric()).trim_end();
    // The mod's own chat lines (the log echoes what it reads) and players' lines ("name » text") never count.
    if body.starts_with("vantage") || body.contains('»') {
        return None;
    }
    if let Some(rest) = body.strip_prefix("you are connected to server name ") {
        return mode_name(rest).map(Line::Connected);
    }
    if let Some(at) = body.find("finding you a game of ") {
        return mode_name(&body[at + "finding you a game of ".len()..]).map(Line::Finding);
    }
    if body == "game over!" || body == "game over" {
        return Some(Line::GameOver);
    }
    if body.starts_with("all members of your party are now spectating") {
        return Some(Line::PartyOut);
    }
    if body.starts_with("you finished all maps and came in") || body.starts_with("you finished in") {
        return Some(Line::Finished);
    }
    if body.starts_with("you were eliminated") || body.starts_with("you have been eliminated") {
        return Some(Line::YouEliminated);
    }
    if let Some(team) = body.strip_prefix("you are on the ") {
        let team = team.trim_end_matches(|c: char| !c.is_alphanumeric());
        return (!team.is_empty()).then(|| Line::OnTeam(team.to_owned()));
    }
    if body.contains("has been eliminated") {
        return Some(Line::TeamOut(body.to_owned()));
    }
    if body.starts_with("you were killed by") || body == "you died!" || body == "you died" {
        return Some(Line::Killed);
    }
    if body.contains("final kill") {
        return Some(Line::FinalKill);
    }
    None
}

/// Where a queue stands.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// `/connection` is to be sent from `at`; given up at `until`.
    Ask { at: f64, until: f64 },
    /// Its answer is waited for.
    Await { until: f64 },
    /// The queue command is to be sent from `at`; given up at `until`.
    Send { at: f64, until: f64 },
}

/// What the glue should ask the game to do now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Send [`MODE_COMMAND`].
    Ask,
    /// Send this command.
    Send(String),
}

/// What the auto requeue knows and is waiting to do.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hive {
    /// The mode last named by Hive.
    pub mode: Option<String>,
    team: Option<String>,
    pending: Option<(Why, Phase)>,
    quiet_until: f64,
    died_at: Option<f64>,
    kill_at: Option<f64>,
    was_alive: Option<bool>,
}

impl Hive {
    /// A line read while the old chat is skipped: the mode and the team are learned, nothing is acted on.
    pub fn learn(&mut self, line: Line) {
        match line {
            Line::Finding(mode) | Line::Connected(mode) => self.set_mode(mode),
            Line::OnTeam(team) => self.team = Some(team),
            _ => {}
        }
    }

    fn set_mode(&mut self, mode: String) {
        if self.mode.as_deref() != Some(mode.as_str()) {
            self.team = None;
        }
        self.mode = Some(mode);
    }

    /// A chat line at `now`. Returns what to tell the player, if anything.
    pub fn line(&mut self, line: Line, now: f64, o: Options) -> Option<String> {
        match line {
            Line::Finding(mode) => {
                self.set_mode(mode);
                None
            }
            Line::Connected(mode) => {
                self.set_mode(mode);
                match self.pending {
                    Some((why, Phase::Ask { .. } | Phase::Await { .. })) => self.decide(why, now, o, true),
                    _ => None,
                }
            }
            Line::OnTeam(team) => {
                self.team = Some(team);
                None
            }
            Line::TeamOut(body) => {
                if self.team.as_deref().is_some_and(|team| body.contains(team)) {
                    self.trigger(Why::TeamOut, now, o)
                } else {
                    None
                }
            }
            Line::GameOver => self.trigger(Why::GameOver, now, o),
            Line::PartyOut => self.trigger(Why::PartyOut, now, o),
            Line::Finished => self.trigger(Why::Finished, now, o),
            Line::YouEliminated => self.trigger(Why::Eliminated, now, o),
            Line::Killed => self.trigger(Why::Death, now, o),
            Line::FinalKill => {
                self.kill_at = Some(now);
                if self.death_was_final() { self.trigger(Why::Eliminated, now, o) } else { None }
            }
        }
    }

    /// The player's life state at `now` (none: not known). Only alive turning into dead is a death.
    pub fn alive(&mut self, alive: Option<bool>, now: f64, o: Options) -> Option<String> {
        let died = self.was_alive == Some(true) && alive == Some(false);
        self.was_alive = alive;
        if !died {
            return None;
        }
        self.died_at = Some(now);
        if self.death_was_final() { self.trigger(Why::Eliminated, now, o) } else { self.trigger(Why::Death, now, o) }
    }

    /// The life state is unknown again (a new world): the next reading is not a death.
    pub fn forget_life(&mut self) {
        self.was_alive = None;
        self.died_at = None;
        self.kill_at = None;
    }

    fn death_was_final(&self) -> bool {
        matches!((self.died_at, self.kill_at), (Some(died), Some(kill)) if (died - kill).abs() <= DEATH_WINDOW)
    }

    /// The requeue key.
    pub fn manual(&mut self, now: f64, o: Options) -> Option<String> {
        self.pending = None;
        self.trigger(Why::Manual, now, o)
    }

    fn enabled(why: Why, o: Options) -> bool {
        match why {
            Why::GameOver | Why::PartyOut | Why::Finished => o.end,
            Why::Eliminated | Why::TeamOut => o.eliminated,
            Why::Death => o.solo,
            Why::Manual => true,
        }
    }

    fn trigger(&mut self, why: Why, now: f64, o: Options) -> Option<String> {
        if why != Why::Manual && (!Self::enabled(why, o) || now < self.quiet_until) {
            return None;
        }
        if let Some((pending, _)) = &mut self.pending {
            // A death that turns out to be an elimination (or the game ending) is the stronger reason.
            if *pending == Why::Death && why != Why::Death {
                *pending = why;
            }
            return None;
        }
        if why == Why::Death {
            // Dying only ends a solo game with no respawns. A known other mode is not asked about again.
            match self.mode.as_deref() {
                Some(mode) if !solo_no_respawn(mode) => return None,
                None if !o.check => return None,
                _ => {}
            }
        }
        if o.check {
            self.pending = Some((why, Phase::Ask { at: now, until: now + ASK_FOR }));
            return (why != Why::Death).then(|| format!("{}, checking the mode", why.label()));
        }
        self.decide(why, now, o, false)
    }

    /// The mode is as known as it will get: queue, or say why not. `answered` is whether Hive just named the mode.
    fn decide(&mut self, why: Why, now: f64, o: Options, answered: bool) -> Option<String> {
        self.pending = None;
        let mode = self.mode.clone();
        if why == Why::Death && !mode.as_deref().is_some_and(solo_no_respawn) {
            return None;
        }
        if answered && mode.as_deref().is_some_and(is_lobby) {
            return (why == Why::Manual).then(|| "you are in a lobby, nothing to queue".to_owned());
        }
        if mode.is_none() && !o.hub {
            return Some(if o.check {
                "Hive did not answer /connection and I do not know the mode. Queue once with /q <mode>".to_owned()
            } else {
                "I do not know the mode yet. Turn on Mode check, or queue once with /q <mode>".to_owned()
            });
        }
        let at = if why == Why::Manual { now } else { now + DELAY };
        self.pending = Some((why, Phase::Send { at, until: at + RETRY_FOR }));
        Some(format!("{}, sending {}", why.label(), self.command(o)))
    }

    fn command(&self, o: Options) -> String {
        match &self.mode {
            Some(mode) if !o.hub => format!("/q {mode}"),
            _ => "/hub".to_owned(),
        }
    }

    /// What is due at `now`: a command to send, and a line for the player when something was given up.
    pub fn due(&mut self, now: f64, o: Options) -> (Option<Action>, Option<String>) {
        match self.pending {
            Some((why, Phase::Ask { until, .. } | Phase::Await { until })) if now >= until => (None, self.decide(why, now, o, false)),
            Some((_, Phase::Ask { at, .. })) if now >= at => (Some(Action::Ask), None),
            Some((_, Phase::Send { until, .. })) if now >= until => {
                self.pending = None;
                (None, Some("gave up: the game kept refusing the command".to_owned()))
            }
            Some((_, Phase::Send { at, .. })) if now >= at => (Some(Action::Send(self.command(o))), None),
            _ => (None, None),
        }
    }

    /// The game took the mode question.
    pub fn asked(&mut self, now: f64) {
        if let Some((why, Phase::Ask { .. })) = self.pending {
            self.pending = Some((why, Phase::Await { until: now + ANSWER_WAIT }));
        }
    }

    /// The game took the queue command.
    pub fn sent(&mut self, now: f64) {
        self.pending = None;
        self.quiet_until = now + QUIET;
        self.team = None;
        self.died_at = None;
        self.kill_at = None;
    }

    /// The game refused a command: try again a moment later.
    pub fn refused(&mut self, now: f64) {
        match &mut self.pending {
            Some((_, Phase::Ask { at, .. } | Phase::Send { at, .. })) => *at = now + RETRY_EVERY,
            _ => {}
        }
    }

    /// Whether something is waiting to happen.
    #[cfg(test)]
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: Options = Options { end: true, eliminated: true, solo: true, check: true, hub: false };
    const NO_CHECK: Options = Options { check: false, ..ALL };

    fn line(text: &str) -> Option<Line> {
        read_line(text)
    }

    /// Runs the queue forward: answers the mode question with `answer` and returns the commands sent.
    fn run(hive: &mut Hive, from: f64, o: Options, answer: Option<&str>) -> Vec<String> {
        let mut sent = Vec::new();
        let mut now = from;
        for _ in 0..400 {
            let (action, _) = hive.due(now, o);
            match action {
                Some(Action::Ask) => {
                    sent.push(MODE_COMMAND.to_owned());
                    hive.asked(now);
                    if let Some(answer) = answer {
                        hive.line(line(answer).unwrap(), now + 0.1, o);
                    }
                }
                Some(Action::Send(command)) => {
                    sent.push(command);
                    hive.sent(now);
                }
                None => {}
            }
            now += 0.05;
        }
        sent
    }

    #[test]
    fn reads_hives_lines_with_their_colour_codes() {
        assert_eq!(line("§c§l» §r§c§lGame OVER!"), Some(Line::GameOver));
        assert_eq!(line("You are connected to server name BED-DUOS12"), Some(Line::Connected("BED-DUOS".into())));
        assert_eq!(line("You are connected to server name HUB-BED3"), Some(Line::Connected("HUB-BED".into())));
        assert_eq!(line("§b§l» §r§3Finding you a game of SKY-CLASSIC-SQUADS..."), Some(Line::Finding("SKY-CLASSIC-SQUADS".into())));
        assert_eq!(line("§b§l» §rYou are on the §cRed Team§7!"), Some(Line::OnTeam("red team".into())));
        assert_eq!(line("§c§l» §r§cRed Team §7has been §cELIMINATED§7!"), Some(Line::TeamOut("red team has been eliminated!".into())));
        assert_eq!(line("§a§l» §r§eYou finished in 3rd place"), Some(Line::Finished));
        assert_eq!(line("§c§l» §r§cYou were killed by §fSomeone§c. They had §c4 hearts"), Some(Line::Killed));
        assert_eq!(line("§c§l» §r§cYou died!"), Some(Line::Killed));
        assert_eq!(line("§6§l» §r§6FINAL KILL! §7Someone was killed"), Some(Line::FinalKill));
        assert_eq!(line("§e§l» §rAll members of your party are now spectating."), Some(Line::PartyOut));
        assert_eq!(line("hello there"), None);
    }

    #[test]
    fn a_long_line_full_of_colour_codes_still_counts() {
        let long = format!("§c§l» §r§c{}You were killed by §f§lSomeLongPlayerName§r§c. §7They had §c9.5 §7hearts left and §e12 §7kills", "§r".repeat(20));
        assert!(long.len() > 96);
        assert_eq!(line(&long), Some(Line::Killed));
    }

    #[test]
    fn players_cannot_type_a_trigger() {
        assert_eq!(line("§7Steve §7§l» §rGame OVER!"), None);
        assert_eq!(line("§7Game OVER §7§l» §rhi"), None);
        assert_eq!(line("§7Steve §8[§a+§8] §7§l» §rYou are connected to server name SKY"), None);
        assert_eq!(line("Vantage Hive log: & Game OVER!"), None);
    }

    #[test]
    fn modes() {
        assert!(solo_no_respawn("SKY") && solo_no_respawn("SKY-CLASSIC") && solo_no_respawn("SKY-KITS") && solo_no_respawn("SG"));
        assert!(!solo_no_respawn("SKY-DUOS") && !solo_no_respawn("SKY-CLASSIC-SQUADS") && !solo_no_respawn("BED") && !solo_no_respawn("SG-DUOS"));
        assert!(is_lobby("HUB") && is_lobby("HUB-BED") && is_lobby("REPLAY") && !is_lobby("BED"));
        assert_eq!(mode_name("bed-duos12 extra"), Some("BED-DUOS".into()));
        assert_eq!(mode_name("; /op me"), None);
    }

    #[test]
    fn game_over_checks_the_mode_then_queues_it() {
        let mut hive = Hive::default();
        assert!(hive.line(Line::GameOver, 10.0, ALL).is_some());
        assert_eq!(run(&mut hive, 10.0, ALL, Some("You are connected to server name BED-DUOS12")), ["/connection", "/q BED-DUOS"]);
        assert!(!hive.busy());
    }

    #[test]
    fn a_stale_mode_is_corrected_by_the_check() {
        let mut hive = Hive::default();
        hive.learn(Line::Finding("SKY".into()));
        hive.line(Line::GameOver, 10.0, ALL);
        assert_eq!(run(&mut hive, 10.0, ALL, Some("You are connected to server name BED4")), ["/connection", "/q BED"]);
    }

    #[test]
    fn with_no_answer_the_remembered_mode_is_used_and_an_unknown_one_is_said() {
        let mut hive = Hive::default();
        hive.learn(Line::Finding("BED".into()));
        hive.line(Line::GameOver, 10.0, ALL);
        assert_eq!(run(&mut hive, 10.0, ALL, None), ["/connection", "/q BED"]);
        let mut hive = Hive::default();
        hive.line(Line::GameOver, 10.0, ALL);
        assert_eq!(run(&mut hive, 10.0, ALL, None), ["/connection"]);
        assert!(!hive.busy());
        // Without the check an unknown mode is said at once and nothing is sent.
        let mut hive = Hive::default();
        assert!(hive.line(Line::GameOver, 10.0, NO_CHECK).unwrap().contains("do not know the mode"));
        assert!(!hive.busy());
    }

    #[test]
    fn nothing_is_queued_from_a_lobby() {
        let mut hive = Hive::default();
        hive.line(Line::GameOver, 10.0, ALL);
        assert_eq!(run(&mut hive, 10.0, ALL, Some("You are connected to server name HUB-SKY2")), ["/connection"]);
    }

    #[test]
    fn dying_queues_only_in_a_solo_game_without_respawns() {
        // SkyWars solo, mode known: the death is checked and queued.
        let mut hive = Hive::default();
        hive.learn(Line::Finding("SKY".into()));
        hive.line(Line::Killed, 10.0, ALL);
        assert_eq!(run(&mut hive, 10.0, ALL, Some("You are connected to server name SKY7")), ["/connection", "/q SKY"]);
        // BedWars, mode known: a death is nothing, and nothing is asked.
        let mut hive = Hive::default();
        hive.learn(Line::Finding("BED".into()));
        assert_eq!(hive.line(Line::Killed, 10.0, ALL), None);
        assert!(run(&mut hive, 10.0, ALL, None).is_empty());
        // Mode unknown: asked once; BedWars is then known and later deaths ask nothing.
        let mut hive = Hive::default();
        hive.line(Line::Killed, 10.0, ALL);
        assert_eq!(run(&mut hive, 10.0, ALL, Some("You are connected to server name BED-SQUADS1")), ["/connection"]);
        hive.line(Line::Killed, 40.0, ALL);
        assert!(run(&mut hive, 40.0, ALL, None).is_empty());
        // A remembered SkyWars mode, but the game is really BedWars: the check stops the queue.
        let mut hive = Hive::default();
        hive.learn(Line::Finding("SKY".into()));
        hive.line(Line::Killed, 10.0, ALL);
        assert_eq!(run(&mut hive, 10.0, ALL, Some("You are connected to server name BED1")), ["/connection"]);
    }

    #[test]
    fn the_alive_flag_turning_off_is_a_death_and_with_a_final_kill_an_elimination() {
        let mut hive = Hive::default();
        hive.learn(Line::Finding("BED".into()));
        hive.alive(Some(true), 1.0, ALL);
        assert_eq!(hive.alive(Some(false), 2.0, ALL), None);
        assert!(!hive.busy());
        hive.alive(Some(true), 5.0, ALL);
        hive.alive(Some(false), 9.0, ALL);
        assert!(hive.line(Line::FinalKill, 9.4, ALL).is_some());
        assert_eq!(run(&mut hive, 9.4, ALL, Some("You are connected to server name BED1")), ["/connection", "/q BED"]);
        // Joining a world dead or unknown is not a death.
        let mut hive = Hive::default();
        hive.learn(Line::Finding("SKY".into()));
        hive.alive(None, 1.0, ALL);
        assert_eq!(hive.alive(Some(false), 2.0, ALL), None);
        assert!(!hive.busy());
    }

    #[test]
    fn a_death_being_checked_becomes_the_elimination_that_follows() {
        let mut hive = Hive::default();
        hive.line(Line::Killed, 10.0, ALL);
        hive.line(Line::YouEliminated, 10.1, ALL);
        assert_eq!(run(&mut hive, 10.1, ALL, Some("You are connected to server name BED-DUOS1")), ["/connection", "/q BED-DUOS"]);
    }

    #[test]
    fn your_team_out_queues_and_another_team_does_not() {
        let mut hive = Hive::default();
        hive.learn(Line::Finding("BED-SQUADS".into()));
        hive.line(line("§b§l» §rYou are on the §cRed Team§7!").unwrap(), 1.0, NO_CHECK);
        assert_eq!(hive.line(line("§c§l» §r§9Blue Team §7has been §cELIMINATED§7!").unwrap(), 5.0, NO_CHECK), None);
        assert!(hive.line(line("§c§l» §r§cRed Team §7has been §cELIMINATED§7!").unwrap(), 6.0, NO_CHECK).is_some());
        assert_eq!(run(&mut hive, 6.0, NO_CHECK, None), ["/q BED-SQUADS"]);
    }

    #[test]
    fn one_game_queues_once_and_switches_are_respected() {
        let mut hive = Hive::default();
        hive.learn(Line::Finding("BED".into()));
        hive.line(Line::YouEliminated, 10.0, NO_CHECK);
        assert_eq!(run(&mut hive, 10.0, NO_CHECK, None), ["/q BED"]);
        // The "Game OVER!" of the game just left.
        assert_eq!(hive.line(Line::GameOver, 15.0, NO_CHECK), None);
        // The next game.
        assert!(hive.line(Line::GameOver, 60.0, NO_CHECK).is_some());
        let off = Options { end: false, eliminated: false, solo: false, ..NO_CHECK };
        let mut hive = Hive::default();
        hive.learn(Line::Finding("SKY".into()));
        assert_eq!(hive.line(Line::GameOver, 10.0, off), None);
        assert_eq!(hive.line(Line::YouEliminated, 10.0, off), None);
        assert_eq!(hive.line(Line::Killed, 10.0, off), None);
        // The key always works.
        assert!(hive.manual(10.0, off).is_some());
        assert_eq!(run(&mut hive, 10.0, off, None), ["/q SKY"]);
    }

    #[test]
    fn the_hub_option_needs_no_mode() {
        let hub = Options { hub: true, ..NO_CHECK };
        let mut hive = Hive::default();
        hive.line(Line::GameOver, 10.0, hub);
        assert_eq!(run(&mut hive, 10.0, hub, None), ["/hub"]);
    }

    #[test]
    fn a_refused_command_is_retried_and_then_given_up() {
        let mut hive = Hive::default();
        hive.learn(Line::Finding("BED".into()));
        hive.line(Line::GameOver, 0.0, NO_CHECK);
        let mut tries = 0;
        let mut now = 0.0;
        let mut gave_up = false;
        while now < 30.0 {
            let (action, note) = hive.due(now, NO_CHECK);
            if action.is_some() {
                tries += 1;
                hive.refused(now);
            }
            gave_up |= note.is_some();
            now += 0.05;
        }
        assert!(tries > 20 && tries < 80, "{tries}");
        assert!(gave_up && !hive.busy());
    }
}
