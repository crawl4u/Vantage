//! The Vantage menu on the GUI toolkit (`vantage-gui`): the Mods page and the settings pages are scenes built
//! here each frame from the settings, with hover, press, drag, scrolling and clicks handled from raw pointer
//! input, and the scene is handed to the host's painter. Clicks become the same events the JSON menu's controls
//! sent, so the settings logic is shared.

use std::collections::HashMap;

use vantage_gui::icon_art::Art;
use vantage_gui::icons::{GRID, Glyph, Icon};
use vantage_gui::{
    anim::Spring,
    geom::{Rect, Vec2},
    layout,
    scene::{Align, Primitive, Scene},
    style::{Border, Color, Fill, Radii},
};
use vantage_ui::{
    options::{Kind, Opt},
    shell::{SIDEBAR_WIDTH, window_px},
    tokens::{Palette, RADII, Rgba, px, space, text},
};

use crate::config::Settings;
use crate::menu_v2;
use crate::panel::PanelState;

/// The face the menu's text uses.
const FONT: &str = "mod_panel_display";
/// Cards per row, as the JSON Mods page has them.
const COLUMNS: usize = 4;

/// The search pill, relative to the window's top left.
#[must_use]
pub fn search_rect() -> Rect {
    let [w, _] = window_px();
    let close = Rect::new(w - px(space::XXL + 28.0), px(space::XL), px(28.0), px(28.0));
    let width = px(200.0);
    Rect::new(close.x - px(space::M) - width, close.y, width, close.h)
}

/// The text keys pressed this frame, as characters (a few per frame is all a person can type).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Typed {
    pub chars: [char; 8],
    pub len: u8,
    pub backspaces: u8,
    pub submit: bool,
    pub cancel: bool,
}

/// The character a physical key types on a US keyboard, with Shift held or not.
fn key_char(key: &str, shift: bool) -> Option<char> {
    let pick = |plain: char, shifted: char| Some(if shift { shifted } else { plain });
    if let Some(letter) = key.strip_prefix("Key").filter(|k| k.len() == 1) {
        let c = letter.chars().next()?.to_ascii_lowercase();
        return Some(if shift { c.to_ascii_uppercase() } else { c });
    }
    if let Some(d) = key.strip_prefix("Digit").or_else(|| key.strip_prefix("Numpad")).filter(|k| k.len() == 1) {
        let c = d.chars().next()?;
        if c.is_ascii_digit() {
            return if key.starts_with("Digit") {
                match c {
                    '1' => pick('1', '!'),
                    '2' => pick('2', '@'),
                    '3' => pick('3', '#'),
                    '4' => pick('4', '$'),
                    '5' => pick('5', '%'),
                    '6' => pick('6', '^'),
                    '7' => pick('7', '&'),
                    '8' => pick('8', '*'),
                    '9' => pick('9', '('),
                    _ => pick('0', ')'),
                }
            } else {
                Some(c)
            };
        }
    }
    match key {
        "Space" => Some(' '),
        "Minus" => pick('-', '_'),
        "Equal" => pick('=', '+'),
        "Period" | "NumpadDecimal" => pick('.', '>'),
        "Comma" => pick(',', '<'),
        "Slash" => pick('/', '?'),
        "Semicolon" => pick(';', ':'),
        "Quote" => pick('\'', '"'),
        "BracketLeft" => pick('[', '{'),
        "BracketRight" => pick(']', '}'),
        "Backslash" => pick('\\', '|'),
        "Backquote" => pick('`', '~'),
        _ => None,
    }
}

/// The keys that type text, which the host only reports to the menu while it has reserved them.
#[must_use]
pub fn text_keys() -> Vec<String> {
    let mut keys: Vec<String> = ('A'..='Z').map(|c| format!("Key{c}")).collect();
    keys.extend((0..=9).map(|d| format!("Digit{d}")));
    keys.extend(["Space", "Minus", "Equal", "Period", "Comma", "Slash", "Semicolon", "Quote", "BracketLeft", "BracketRight", "Backslash", "Backquote", "NumpadDecimal"].map(str::to_owned));
    keys.extend(["Backspace", "Enter", "NumpadEnter", "Escape", "ShiftLeft", "ShiftRight"].map(str::to_owned));
    keys
}

/// What this frame's key presses type.
#[must_use]
pub fn typed_from_keys(keys: &[String], shift: bool) -> Typed {
    let mut out = Typed::default();
    for key in keys {
        match key.as_str() {
            "Backspace" => out.backspaces = out.backspaces.saturating_add(1),
            "Enter" | "NumpadEnter" => out.submit = true,
            "Escape" => out.cancel = true,
            other => {
                if let Some(c) = key_char(other, shift) {
                    if usize::from(out.len) < out.chars.len() {
                        out.chars[usize::from(out.len)] = c;
                        out.len += 1;
                    }
                }
            }
        }
    }
    out
}

/// What the pointer did this frame, in the scene's units (the viewport's top left is the origin).
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// The keys typed this frame.
    pub typed: Typed,
    pub pointer: Option<Vec2>,
    /// The primary button went down this frame.
    pub pressed: bool,
    /// The primary button is down.
    pub held: bool,
    /// The wheel's movement this frame, positive scrolling up.
    pub wheel: f32,
    /// Seconds since the last frame.
    pub dt: f32,
    /// The viewport's size.
    pub viewport: [f32; 2],
    /// Enter or Escape was pressed, which ends editing the search text.
    pub dismiss: bool,
}

/// Something the page wants done.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// An event for the settings logic, by the id the JSON menu's control had.
    Event(String),
    /// An event with a value (a switch's new state, a slider's value).
    EventValue(String, f32),
    /// The text of a field, as typed so far (`submit` when Enter ended the edit).
    Text { id: String, text: String, submit: bool },
    Close,
}

/// A mod card's corner radius, in design units.
const CARD_RADIUS: f32 = 12.0;

/// The window's corner radius, in design units.
const WINDOW_RADIUS: f32 = 14.0;

/// What a region of the page is, packed into the scene's region ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hit {
    Card(usize),
    Switch(usize),
    Star(usize),
    Hide(usize),
    Chip(usize),
    Sort(usize),
    Nav(usize),
    Close,
    Back,
    /// A control of a settings page, by its place in this frame's list.
    Act(usize),
    /// The notifications bell at the foot of the sidebar.
    Bell,
}

impl Hit {
    fn id(self) -> u64 {
        let (kind, n) = match self {
            Hit::Card(n) => (1, n),
            Hit::Switch(n) => (2, n),
            Hit::Star(n) => (3, n),
            Hit::Hide(n) => (4, n),
            Hit::Chip(n) => (5, n),
            Hit::Sort(n) => (6, n),
            Hit::Nav(n) => (7, n),
            Hit::Close => (8, 0),
            Hit::Back => (9, 0),
            Hit::Act(n) => (10, n),
            Hit::Bell => (11, 0),
        };
        (kind << 32) | n as u64
    }

    fn from_id(id: u64) -> Option<Hit> {
        let n = (id & 0xFFFF_FFFF) as usize;
        Some(match id >> 32 {
            1 => Hit::Card(n),
            2 => Hit::Switch(n),
            3 => Hit::Star(n),
            4 => Hit::Hide(n),
            5 => Hit::Chip(n),
            6 => Hit::Sort(n),
            7 => Hit::Nav(n),
            8 => Hit::Close,
            9 => Hit::Back,
            10 => Hit::Act(n),
            11 => Hit::Bell,
            _ => return None,
        })
    }
}

/// What a control of a settings page does when used.
#[derive(Clone, Debug)]
enum Act {
    /// Flips a switch: the event carries the new state.
    Toggle { id: String, on: bool },
    /// A press that sends an event.
    Press(String),
    /// A slider: pressing it starts a drag along `track`.
    Slider { id: String, track: Rect, min: f32, max: f32, step: f32 },
    /// A text field: pressing it starts typing into it.
    Field { id: String, text: String },
}

fn color(c: Rgba) -> Color {
    Color::rgba(c[0], c[1], c[2], c[3])
}

/// Draws into a scene at the window's place in the viewport, in the window's own units.
struct Painter {
    scene: Scene,
    origin: Vec2,
    p: Palette,
    hover: Option<Hit>,
    /// How far each recently hovered region is lit, easing in and out.
    lit_amounts: Vec<(Hit, f32)>,
}

impl Painter {
    /// How lit region `h` is, from 0 (untouched) to 1 (under the pointer).
    fn amount(&self, h: Hit) -> f32 {
        self.lit_amounts.iter().find(|(k, _)| *k == h).map_or(0.0, |(_, v)| *v)
    }

    /// `base` eased toward `hot` as the pointer arrives over `h`.
    fn hot(&self, h: Hit, base: Rgba, hot: Rgba) -> Rgba {
        let t = self.amount(h);
        std::array::from_fn(|i| base[i] + (hot[i] - base[i]) * t)
    }

    fn at(&self, r: Rect) -> Rect {
        Rect::new(self.origin.x + r.x, self.origin.y + r.y, r.w, r.h)
    }

    fn lit(&self, h: Hit) -> bool {
        self.hover == Some(h)
    }

    /// An icon centred in `r`, `size` across, in colour `c`.
    fn glyph(&mut self, r: Rect, icon: impl Into<Glyph>, size: f32, c: Rgba) {
        let box_ = self.at(Rect::new(r.x + (r.w - size) / 2.0, r.y + (r.h - size) / 2.0, size, size));
        match icon.into() {
            Glyph::Line(icon) => {
                self.scene.push(Primitive::Line { rect: box_, view: [GRID, GRID], contours: icon.contours(), color: color(c), width: icon.weight() });
            }
            Glyph::Pixel(art) => {
                // Each lit run is a one-pixel-high rectangle; all of them go in one hard-edged fill so the
                // pixels stay square and no seams show between neighbours.
                let rects: Vec<Vec<[f32; 2]>> = art
                    .runs
                    .iter()
                    .map(|[x, y, w]| {
                        let (x0, y0, x1, y1) = (f32::from(*x), f32::from(*y), f32::from(*x) + f32::from(*w), f32::from(*y) + 1.0);
                        vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
                    })
                    .collect();
                let side = f32::from(art.size);
                self.scene.push(Primitive::Pixels { rect: box_, view: [side, side], contours: rects, color: color(c) });
            }
            Glyph::Art(art) => {
                for shape in art.shapes() {
                    let rings: Vec<Vec<[f32; 2]>> = shape.iter().map(|ring| ring.to_vec()).collect();
                    self.scene.push(Primitive::Path { rect: box_, view: [GRID, GRID], contours: rings, color: color(c), stroke: None });
                }
            }
        }
    }

    fn rect(&mut self, r: Rect, fill: Rgba, radii: Radii) {
        let rect = self.at(r);
        self.scene.push(Primitive::Rect { rect, fill: Fill::Solid(color(fill)), radii, border: None, shadow: None });
    }

    fn ring(&mut self, r: Rect, radii: Radii, width: f32, c: Rgba) {
        let rect = self.at(r);
        self.scene.push(Primitive::Rect { rect, fill: Fill::None, radii, border: Some(Border { width, color: color(c) }), shadow: None });
    }

    fn text(&mut self, r: Rect, s: &str, size: f32, c: Rgba, align: Align) {
        let rect = self.at(r);
        self.scene.push(Primitive::Text { rect, text: s.to_owned(), size, color: color(c), align, font: FONT });
    }

    /// A line of text vertically centred in a box.
    fn line(&mut self, r: Rect, s: &str, size: f32, c: Rgba, align: Align) {
        let h = size * 1.5;
        // The face sits a little high in its line box, so the box drops by that much to centre the letters.
        self.text(Rect::new(r.x, r.y + (r.h - h) / 2.0 + size * 0.12, r.w, h), s, size, c, align);
    }

    fn icon(&mut self, r: Rect, glyph: &str, size: f32, c: Rgba) {
        let rect = self.at(r);
        if let Some(g) = glyph.chars().next() {
            self.scene.push(Primitive::Icon { rect, glyph: g, size, color: color(c) });
        }
    }

    fn region(&mut self, h: Hit, r: Rect) {
        let rect = self.at(r);
        self.scene.region(h.id(), rect);
    }
}

/// The page's state between frames.
pub struct GuiMenu {
    scroll: Spring,
    scroll_target: f32,
    hover: Option<Hit>,
    /// Hover easing for every region the pointer has lately been over.
    hot: Vec<(Hit, Spring)>,
    /// How far the page change has run (0 just changed, 1 settled), for the cross-fade.
    page: Spring,
    /// The previous frame scene and where its page content starts, kept for the cross-fade.
    last: Option<(Scene, usize)>,
    /// The scene the old page is fading out from, and where its page content starts.
    outgoing: Option<(Scene, usize)>,
    /// Where the page content starts in the scene being built (after the window and sidebar).
    chrome: usize,
    /// Seconds the pointer has stayed over the same region, for tooltips.
    hover_clock: f32,
    hover_seen: Option<Hit>,
    /// The sort switch highlight position (0 custom order, 1 A to Z).
    sort_slide: Spring,
    /// The page of the sidebar the toolkit draws itself that is open, if one is.
    virt: Option<usize>,
    /// Whether the notifications popover is open, and how far it has opened.
    notes_open: bool,
    notes: Spring,
    /// The bell in the sidebar footer.
    bell: Rect,
    pressed: Option<Hit>,
    /// How lit each card is (0 to 1), easing with the pointer.
    glow: Vec<Spring>,
    /// The scene's content height last frame, for the scroll limit.
    content: f32,
    view: f32,
    /// The search text is being edited: the host's editor draws it, so the page does not.
    searching: bool,
    /// The tab drawn last frame, so the scroll starts over on a new page.
    tab: usize,
    /// The controls of the settings page drawn this frame.
    acts: Vec<Act>,
    /// A slider being dragged: its id, its track and range, and the last value sent.
    drag: Option<(String, Rect, f32, f32, f32, f32)>,
    /// Each switch's knob position (0 off, 1 on), by control id.
    knobs: HashMap<String, Spring>,
    /// A card being dragged to a new place in the grid.
    card_drag: Option<CardDrag>,
    /// Where each card is drawn, easing toward its cell (so the grid re-flows smoothly), by card index.
    card_pos: Vec<[Spring; 2]>,
    placed: Vec<bool>,
    /// The grid's cells and area as last drawn, in window units, for dropping a card on one.
    cells: Vec<Rect>,
    grid_view: Rect,
    /// A press on a card turned into a drag, so its release is not a click.
    dragged: bool,
    /// How open the menu is (0 just opened, 1 settled), for its fade and slide in.
    open: Spring,
    /// The text field being typed into: its id and its text so far.
    field: Option<(String, String)>,
    /// Seconds the menu has been open, for the caret's blink.
    clock: f32,
}

/// A press on a card that may become a drag.
#[derive(Clone, Copy, Debug)]
struct CardDrag {
    /// The card's index among all cards, and its place in the grid when the press began.
    card: usize,
    from: usize,
    /// The pointer when it began, and where it grabbed the card (from the card's top left).
    start: Vec2,
    grab: Vec2,
    lifted: bool,
    /// The place the card would take if dropped now.
    target: usize,
}

/// How far the pointer travels before a press on a card becomes a drag, in window units.
const DRAG_START: f32 = 6.0;

impl Default for GuiMenu {
    fn default() -> Self {
        Self {
            scroll: Spring::new(0.0, 22.0),
            scroll_target: 0.0,
            hover: None,
            hot: Vec::new(),
            page: Spring::new(1.0, 14.0),
            last: None,
            outgoing: None,
            chrome: 0,
            hover_clock: 0.0,
            hover_seen: None,
            sort_slide: Spring::new(0.0, 22.0),
            virt: None,
            notes_open: false,
            notes: Spring::new(0.0, 22.0),
            bell: Rect::default(),
            pressed: None,
            glow: Vec::new(),
            content: 0.0,
            view: 0.0,
            searching: false,
            tab: usize::MAX,
            acts: Vec::new(),
            drag: None,
            knobs: HashMap::new(),
            card_drag: None,
            card_pos: Vec::new(),
            placed: Vec::new(),
            cells: Vec::new(),
            grid_view: Rect::default(),
            dragged: false,
            open: Spring::new(0.0, 16.0),
            field: None,
            clock: 0.0,
        }
    }
}

/// Where the window sits in a viewport of `viewport`, centred.
fn window_origin(viewport: [f32; 2]) -> Vec2 {
    let [w, h] = window_px();
    Vec2::new(((viewport[0] - w) / 2.0).max(0.0), ((viewport[1] - h) / 2.0).max(0.0))
}

/// A number as a slider shows it: with as many decimals as its step has.
fn number(v: f32, step: f32) -> String {
    let decimals = if step >= 1.0 || step <= 0.0 { 0 } else { (-step.log10()).ceil().clamp(0.0, 3.0) as usize };
    format!("{v:.decimals$}")
}

/// A lighter version of a colour, for a hovered accent button.
fn color_up(c: Rgba) -> Rgba {
    [(c[0] + 0.12).min(1.0), (c[1] + 0.12).min(1.0), (c[2] + 0.12).min(1.0), c[3]]
}

/// The value a pointer at `x` picks along a slider's track, snapped to its step.
fn slider_value(x: f32, track: Rect, min: f32, max: f32, step: f32) -> f32 {
    let t = ((x - track.x) / track.w).clamp(0.0, 1.0);
    let raw = min + t * (max - min);
    if step > 0.0 { (min + ((raw - min) / step).round() * step).clamp(min, max) } else { raw }
}

impl GuiMenu {
    /// Moves the scroll for the wheel and this frame's time, over content of the last frame's size.
    /// The menu was closed: it fades in again the next time it opens.
    pub fn closed(&mut self) {
        self.open.snap(0.0);
        self.searching = false;
        self.card_drag = None;
        self.drag = None;
        self.field = None;
        self.last = None;
        self.outgoing = None;
        self.virt = None;
        self.notes_open = false;
    }

    /// The window's place now: centred, a little low while it fades in.
    fn window_place(&self, viewport: [f32; 2]) -> Vec2 {
        let o = window_origin(viewport);
        Vec2::new(o.x, o.y + (1.0 - self.open.value()) * px(14.0))
    }

    /// Cross-fades from the page that was open to the one now built: the window and sidebar stay, everything
    /// else of the old page fades out as the new one fades in.
    fn finish(&mut self, scene: Scene) -> Scene {
        let t = self.page.value().clamp(0.0, 1.0);
        let chrome = self.chrome.min(scene.primitives.len());
        self.last = Some((scene.clone(), chrome));
        if t >= 0.99 {
            self.outgoing = None;
        }
        let Some((old, old_chrome)) = self.outgoing.as_ref().filter(|_| t < 0.99) else {
            return scene;
        };
        let old_end = old.primitives.len().saturating_sub(1);
        let new_end = scene.primitives.len().saturating_sub(1);
        if *old_chrome > old_end || chrome > new_end {
            return scene;
        }
        let mut out = scene.clone();
        let mut prims: Vec<Primitive> = scene.primitives[..chrome].to_vec();
        prims.push(Primitive::PushOpacity(1.0 - t));
        prims.extend_from_slice(&old.primitives[*old_chrome..old_end]);
        prims.push(Primitive::PopOpacity);
        prims.push(Primitive::PushOpacity(t));
        prims.extend_from_slice(&scene.primitives[chrome..new_end]);
        prims.push(Primitive::PopOpacity);
        prims.extend_from_slice(&scene.primitives[new_end..]);
        out.primitives = prims;
        out
    }

    /// What floats above the page: the notifications popover.
    fn overlay(&mut self, d: &mut Painter) {
        let t = self.notes.value().clamp(0.0, 1.0);
        if t < 0.01 {
            return;
        }
        let p = d.p;
        let w = px(190.0);
        let h = px(74.0);
        let r = Rect::new(self.bell.right() - w + px(6.0), self.bell.y - h - px(10.0) + (1.0 - t) * px(8.0), w, h);
        d.scene.push(Primitive::PushOpacity(t));
        d.rect(r, [0.05, 0.05, 0.08, 1.0], Radii::all(px(RADII.sm)));
        d.ring(r, Radii::all(px(RADII.sm)), 0.3, [1.0, 1.0, 1.0, 0.14]);
        d.line(Rect::new(r.x + px(14.0), r.y + px(10.0), r.w - px(20.0), px(20.0)), "Notifications", px(text::LABEL.size), p.text, Align::Start);
        d.line(Rect::new(r.x + px(14.0), r.y + px(34.0), r.w - px(20.0), px(20.0)), "You are all caught up.", px(text::SMALL.size), p.muted, Align::Start);
        d.scene.push(Primitive::PopOpacity);
    }

    /// Where a click on a sidebar entry, the back arrow or the bell goes, when the toolkit handles it itself.
    fn route(&mut self, settings: &Settings, state: &PanelState, hit: Hit) -> Option<Vec<Action>> {
        match hit {
            Hit::Nav(n) => {
                self.notes_open = false;
                if n == menu_v2::EDIT_HUD {
                    self.virt = None;
                    let hud = menu_v2::grid(settings, state).2.into_iter().find(|(name, _)| *name == "HUD").map(|(_, number)| number);
                    let mut actions = vec![Action::Event("tab_0".to_owned())];
                    actions.extend(hud.map(|number| Action::Event(format!("flt_{number}"))));
                    Some(actions)
                } else if n >= menu_v2::VIRTUAL_BASE {
                    self.virt = Some(n);
                    Some(vec![Action::Event("tab_6".to_owned())])
                } else {
                    self.virt = None;
                    None
                }
            }
            Hit::Back if self.virt.is_some() => {
                self.virt = None;
                Some(vec![Action::Event("tab_0".to_owned())])
            }
            Hit::Bell => {
                self.notes_open = !self.notes_open;
                Some(Vec::new())
            }
            _ => None,
        }
    }

    fn lit_amounts(&self) -> Vec<(Hit, f32)> {
        self.hot.iter().map(|(k, s)| (*k, s.value().clamp(0.0, 1.0))).collect()
    }

    fn scroll_step(&mut self, input: &Input, tab: usize) {
        let tab = self.virt.map_or(tab, |n| 1000 + n);
        self.notes.set_target(if self.notes_open { 1.0 } else { 0.0 });
        self.notes.step(input.dt);
        self.open.set_target(1.0);
        self.open.step(input.dt);
        self.clock += input.dt;
        self.page.set_target(1.0);
        self.page.step(input.dt);
        if self.hover != self.hover_seen {
            self.hover_seen = self.hover;
            self.hover_clock = 0.0;
        } else {
            self.hover_clock += input.dt;
        }
        if let Some(h) = self.hover {
            if !self.hot.iter().any(|(k, _)| *k == h) {
                self.hot.push((h, Spring::new(0.0, 26.0)));
            }
        }
        let hover = self.hover;
        for (k, s) in &mut self.hot {
            s.set_target(if hover == Some(*k) { 1.0 } else { 0.0 });
            s.step(input.dt);
        }
        self.hot.retain(|(k, s)| hover == Some(*k) || s.value() > 0.01);
        if self.tab != tab {
            if self.tab != usize::MAX {
                self.page.snap(0.0);
                self.outgoing = self.last.take();
            }
            self.tab = tab;
            self.scroll.snap(0.0);
            self.scroll_target = 0.0;
            self.searching = false;
        }
        let max = (self.content - self.view).max(0.0);
        self.scroll_target = (self.scroll_target - input.wheel * px(60.0)).clamp(0.0, max);
        self.scroll.set_target(self.scroll_target);
        self.scroll.step(input.dt);
    }

    /// Search pill editing: a press on the pill starts it, any other press or Enter or Escape ends it.
    fn search_focus(&mut self, input: &Input) {
        let origin = window_origin(input.viewport);
        let pill = search_rect();
        let on_pill = input.pointer.is_some_and(|p| Rect::new(origin.x + pill.x, origin.y + pill.y, pill.w, pill.h).contains(p));
        if input.pressed {
            self.searching = on_pill;
        }
        if input.dismiss {
            self.searching = false;
        }
    }

    /// Hit tests the finished scene, tracks the press, and returns the control released on.
    fn pointer(&mut self, scene: &Scene, input: &Input) -> Option<Hit> {
        let hit = input.pointer.and_then(|p| scene.hit(p)).and_then(Hit::from_id);
        self.hover = hit;
        if input.pressed {
            self.pressed = hit;
        }
        if input.held {
            return None;
        }
        match (self.pressed.take(), hit) {
            (Some(down), Some(up)) if down == up => Some(up),
            _ => None,
        }
    }

    /// Whether a text field has the keyboard, so the text keys must be reserved for the menu.
    #[must_use]
    pub fn wants_text(&self) -> bool {
        self.field.is_some()
    }

    /// Runs one frame of the Mods page.
    pub fn frame(&mut self, settings: &Settings, state: &PanelState, input: Input) -> (Scene, Vec<Action>) {
        let (cards, order, chips, active_chip, active_sort) = menu_v2::grid(settings, state);
        if self.glow.len() < cards.len() {
            self.glow.resize(cards.len(), Spring::new(0.0, 18.0));
        }
        self.scroll_step(&input, crate::menu_controls::MODS_PAGE);
        for (i, glow) in self.glow.iter_mut().enumerate() {
            let on = matches!(self.hover, Some(Hit::Card(n) | Hit::Switch(n) | Hit::Star(n) | Hit::Hide(n)) if n == i);
            glow.set_target(if on { 1.0 } else { 0.0 });
            glow.step(input.dt);
        }
        self.search_focus(&input);
        let mut actions = Vec::new();
        self.update_card_drag(&input, &cards, &order, &mut actions);
        let lifted = self.card_drag.filter(|d| d.lifted);
        // While a card is lifted the others re-flow around the place it would take.
        let show: Vec<usize> = match lifted {
            Some(d) => {
                let mut shown = order.clone();
                if d.from < shown.len() {
                    let card = shown.remove(d.from);
                    shown.insert(d.target.min(shown.len()), card);
                }
                shown
            }
            None => order.clone(),
        };
        let scene = self.build_mods(state, &cards, &show, &chips, active_chip, active_sort, input);
        let scene = self.finish(scene);
        let click = self.pointer(&scene, &input);
        // A press that became a drag is not a click.
        if let Some(up) = click.filter(|_| !self.dragged) {
            let routed = self.route(settings, state, up);
            actions.extend(routed.unwrap_or_else(|| mods_click(up, &cards, &order, &chips)));
        }
        if !input.held {
            self.dragged = false;
        }
        (scene, actions)
    }

    /// Starts, follows and ends a drag of a card: dropping it sends its new place.
    fn update_card_drag(&mut self, input: &Input, cards: &[vantage_ui::mods::Card<'_>], order: &[usize], actions: &mut Vec<Action>) {
        let origin = window_origin(input.viewport);
        let pointer = input.pointer.map(|p| Vec2::new(p.x - origin.x, p.y - origin.y));
        if input.pressed {
            self.card_drag = match (self.hover, pointer) {
                (Some(Hit::Card(n)), Some(p)) if n < order.len() => {
                    let cell = self.cells.get(n).copied().unwrap_or_default();
                    let top = cell.y + self.grid_view.y - self.scroll.value();
                    Some(CardDrag { card: order[n], from: n, start: p, grab: Vec2::new(p.x - (self.grid_view.x + cell.x), p.y - top), lifted: false, target: n })
                }
                _ => None,
            };
        }
        let Some(mut drag) = self.card_drag else { return };
        if !input.held {
            if drag.lifted && drag.target != drag.from {
                if let Some(card) = cards.get(drag.card) {
                    actions.push(Action::EventValue(format!("move_{}", card.id), drag.target as f32));
                }
            }
            self.card_drag = None;
            return;
        }
        let Some(p) = pointer else { return };
        if !drag.lifted && (p.x - drag.start.x).hypot(p.y - drag.start.y) >= DRAG_START {
            drag.lifted = true;
            self.dragged = true;
        }
        if drag.lifted {
            // The cell whose middle is nearest the pointer.
            let scroll = self.scroll.value();
            let nearest = self
                .cells
                .iter()
                .take(order.len())
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    let d = |r: &Rect| {
                        let c = Vec2::new(self.grid_view.x + r.x + r.w / 2.0, self.grid_view.y + r.y - scroll + r.h / 2.0);
                        (c.x - p.x).powi(2) + (c.y - p.y).powi(2)
                    };
                    d(a).total_cmp(&d(b))
                })
                .map_or(drag.from, |(i, _)| i);
            drag.target = nearest;
        }
        self.card_drag = Some(drag);
    }

    /// The window, sidebar, header and search pill common to every page. Returns the content pane.
    fn shell(&mut self, d: &mut Painter, title: &str, tab: usize, hint: &str, query: &str) -> [f32; 4] {
        let p = d.p;
        let [w, h] = window_px();
        d.rect(Rect::new(0.0, 0.0, w, h), p.panel, Radii::all(px(WINDOW_RADIUS)));
        d.ring(Rect::new(0.0, 0.0, w, h), Radii::all(px(WINDOW_RADIUS)), 0.3, [1.0, 1.0, 1.0, 0.10]);

        // Sidebar and logo, divided from the pages by a hairline.
        let side = px(SIDEBAR_WIDTH);
        // The sidebar is a lighter pane than the pages beside it.
        let lg = px(WINDOW_RADIUS);
        d.rect(Rect::new(0.0, 0.0, side, h), [0.09, 0.09, 0.11, 0.62], Radii { tl: lg, tr: 0.0, br: 0.0, bl: lg });
        d.rect(Rect::new(side, 0.0, 0.35, h), [1.0, 1.0, 1.0, 0.12], Radii::default());
        let logo_w = side - px(space::XXL * 2.0);
        let logo = Rect::new(px(space::XXL), px(space::XL), logo_w, logo_w * vantage_ui::brand::SIZE[1] / vantage_ui::brand::SIZE[0]);
        let logo_rect = d.at(logo);
        for layer in vantage_ui::brand::LAYERS {
            d.scene.push(Primitive::Path {
                rect: logo_rect,
                view: vantage_ui::brand::SIZE,
                contours: layer.contours.iter().map(|c| c.to_vec()).collect(),
                color: Color::hex(layer.rgb, 1.0),
                stroke: layer.stroke,
            });
        }
        let mut y = px(space::XXL * 3.0 - 6.0);
        for (heading, entries) in menu_v2::gui_sidebar(tab, self.virt) {
            if !heading.is_empty() {
                d.text(Rect::new(px(space::XXL), y, side, px(20.0)), &heading.to_uppercase(), px(text::CAPTION.size), p.muted, Align::Start);
                y += px(18.0);
            }
            for (target, label, glyph, active) in entries {
                let row = Rect::new(px(space::XL), y, side - px(space::XL * 2.0), px(26.0));
                let hit = Hit::Nav(target);
                if active {
                    d.rect(row, p.accent, Radii::all(px(RADII.sm)));
                } else {
                    d.rect(row, [1.0, 1.0, 1.0, 0.07 * d.amount(hit)], Radii::all(px(RADII.sm)));
                }
                let ink = if active { p.on_accent } else { p.text };
                d.glyph(Rect::new(row.x + px(space::M), row.y, px(24.0), row.h), glyph, px(22.0), ink);
                d.line(Rect::new(row.x + px(48.0), row.y, row.w - px(54.0), row.h), label, px(text::LABEL.size), ink, Align::Start);
                d.region(hit, row);
                y += px(28.0);
            }
            y += px(6.0);
        }

        // The player at the bottom of the sidebar.
        let foot = Rect::new(px(space::XL), h - px(52.0), side - px(space::XL * 2.0), px(40.0));
        d.rect(Rect::new(0.0, foot.y - px(space::S), side, 0.35), [1.0, 1.0, 1.0, 0.12], Radii::default());
        d.rect(Rect::new(foot.x, foot.y, px(32.0), px(32.0)), p.dim, Radii::all(px(16.0)));
        let bell = Rect::new(foot.right() - px(26.0), foot.y + px(3.0), px(26.0), px(26.0));
        self.bell = bell;
        let bell_ink = d.hot(Hit::Bell, p.muted, p.text);
        d.rect(bell, [1.0, 1.0, 1.0, 0.08 * d.amount(Hit::Bell)], Radii::all(px(RADII.xs)));
        d.glyph(bell, Art::Bell, px(20.0), bell_ink);
        d.region(Hit::Bell, bell);
        d.line(Rect::new(foot.x + px(42.0), foot.y - px(4.0), foot.w, px(20.0)), "Player", px(text::LABEL.size), p.text, Align::Start);
        d.line(Rect::new(foot.x + px(42.0), foot.y + px(12.0), foot.w, px(16.0)), "Vantage", px(text::CAPTION.size), p.muted, Align::Start);

        self.chrome = d.scene.primitives.len();
        // Header: arrow, title, search pill, close.
        let pane = vantage_ui::shell::content_rect();
        let back = Rect::new(pane[0] - px(space::S), px(space::XL), px(24.0), px(28.0));
        d.glyph(back, Art::LeftArrow, px(24.0), d.hot(Hit::Back, p.muted, p.text));
        d.region(Hit::Back, back);
        d.line(Rect::new(pane[0] + px(space::XXL), px(space::XL), px(300.0), px(28.0)), title, px(text::HEADING.size), p.highlight, Align::Start);
        let close = Rect::new(w - px(space::XXL + 28.0), px(space::XL), px(28.0), px(28.0));
        d.rect(close, [1.0, 1.0, 1.0, 0.09 * d.amount(Hit::Close)], Radii::all(px(RADII.xs)));
        d.glyph(close, Art::Close, px(26.0), p.text);
        d.region(Hit::Close, close);
        let pill = search_rect();
        d.rect(pill, if self.searching { p.dim } else { p.field }, Radii::all(px(RADII.xs)));
        d.ring(pill, Radii::all(px(RADII.xs)), 0.3, if self.searching { p.accent } else { [1.0, 1.0, 1.0, 0.10] });
        d.glyph(Rect::new(pill.x + px(space::M), pill.y, px(18.0), pill.h), Art::Search, px(15.0), p.muted);
        if !self.searching {
            let (shown, ink) = if query.is_empty() { (hint, p.muted) } else { (query, p.text) };
            d.line(Rect::new(pill.x + px(30.0), pill.y, pill.w - px(36.0), pill.h), shown, px(text::SMALL.size), ink, Align::Start);
        }
        pane
    }

    #[allow(clippy::too_many_arguments)]
    fn build_mods(
        &mut self,
        state: &PanelState,
        cards: &[vantage_ui::mods::Card<'_>],
        order: &[usize],
        chips: &[(&str, usize)],
        active_chip: usize,
        active_sort: usize,
        input: Input,
    ) -> Scene {
        let p = Palette::dark();
        let mut d = Painter { scene: Scene::new(), origin: self.window_place(input.viewport), p, hover: self.hover, lit_amounts: self.lit_amounts() };
        d.scene.push(Primitive::PushOpacity(self.open.value()));
        let pane = self.shell(&mut d, "Mods", crate::menu_controls::MODS_PAGE, "Search mods...", state.search_text());

        // Chips and the sort segments.
        let chip_y = pane[1];
        let chip_h = px(28.0);
        let char_w = px(text::LABEL.size) * 0.62;
        let mut x = pane[0];
        for (n, (name, number)) in chips.iter().enumerate() {
            let glyph = match *name {
                "All" => Glyph::Art(Art::Layers),
                "Enabled" => Glyph::Art(Art::CheckCircle),
                "Favorites" => Glyph::Art(Art::Star),
                "HUD" => Glyph::Art(Art::Hud),
                "Visual" => Glyph::Art(Art::Paintbrush),
                "Mechanic" => Glyph::Art(Art::Qol),
                _ => Glyph::Art(Art::Lightning01),
            };
            let cw = name.chars().count() as f32 * char_w * 0.92 + px(20.0) + px(26.0);
            let on = *number == active_chip;
            let r = Rect::new(x, chip_y, cw, chip_h);
            let hit = Hit::Chip(n);
            let fill = if on { p.accent } else { d.hot(hit, p.raised, p.dim) };
            d.rect(r, fill, Radii::all(px(RADII.xs)));
            if !on {
                d.ring(r, Radii::all(px(RADII.xs)), 0.3, [1.0, 1.0, 1.0, 0.10]);
            }
            let ink = if on { p.on_accent } else { p.text };
            // The icon and the label are centred in the chip as one group.
            let label_w = name.chars().count() as f32 * char_w * 0.92;
            let group = px(18.0) + px(6.0) + label_w;
            let gx = r.x + (r.w - group) / 2.0;
            d.glyph(Rect::new(gx, r.y, px(18.0), r.h), glyph, px(17.0), ink);
            d.line(Rect::new(gx + px(24.0), r.y, label_w + px(12.0), r.h), name, px(text::LABEL.size), ink, Align::Start);
            d.region(hit, r);
            x += cw + px(space::XS);
        }
        // The sort switch: the grid own order, or A to Z, as a two-part box the highlight slides along.
        let seg_w = px(34.0);
        let switch = Rect::new(pane[0] + pane[2] - seg_w * 2.0 - px(4.0), chip_y, seg_w * 2.0 + px(4.0), chip_h);
        self.sort_slide.set_target(if active_sort == 1 { 1.0 } else { 0.0 });
        self.sort_slide.step(input.dt);
        d.rect(switch, p.field, Radii::all(px(RADII.xs)));
        d.ring(switch, Radii::all(px(RADII.xs)), 0.3, [1.0, 1.0, 1.0, 0.10]);
        let slide = self.sort_slide.value();
        d.rect(Rect::new(switch.x + px(2.0) + seg_w * slide, switch.y + px(2.0), seg_w, switch.h - px(4.0)), p.dim, Radii::all(px(RADII.xs - 2.0)));
        let mut tip: Option<(Rect, &str)> = None;
        for (n, tip_text) in ["Custom order", "A to Z"].iter().enumerate() {
            let r = Rect::new(switch.x + px(2.0) + seg_w * n as f32, switch.y, seg_w, switch.h);
            let hit = Hit::Sort(n);
            let on = (n == 1) == (active_sort == 1);
            let ink = if on { p.text } else { p.muted };
            if n == 0 {
                d.glyph(r, Art::DotsGrid, px(17.0), ink);
            } else {
                d.line(r, "Aa", px(text::LABEL.size), ink, Align::Centre);
            }
            d.region(hit, r);
            if self.hover == Some(hit) {
                tip = Some((r, tip_text));
            }
        }

        // The grid, scrolled and clipped.
        let grid_top = chip_y + chip_h + px(space::L);
        let view = Rect::new(pane[0], grid_top, pane[2], pane[1] + pane[3] - grid_top);
        let gap = px(space::M);
        let cell_w = (view.w - gap * (COLUMNS as f32 - 1.0)) / COLUMNS as f32;
        let (cells, content) = layout::grid(order.len(), [cell_w, cell_w * 0.56], gap, view.w);
        self.content = content;
        self.view = view.h;
        self.cells = cells.clone();
        self.grid_view = view;
        if self.card_pos.len() < cards.len() {
            self.card_pos.resize(cards.len(), [Spring::new(0.0, 20.0), Spring::new(0.0, 20.0)]);
            self.placed.resize(cards.len(), false);
        }
        let clip = d.at(view);
        d.scene.push(Primitive::PushClip { rect: clip, radii: Radii::default() });
        let scroll = self.scroll.value();
        let lifted = self.card_drag.filter(|c| c.lifted);
        let mut ghost: Option<(usize, usize)> = None;
        for (n, cell) in cells.iter().enumerate() {
            let Some(card) = order.get(n).and_then(|i| cards.get(*i)) else { continue };
            let index = order[n];
            // Each card eases toward its cell, so a filter, a sort or a drag re-flows the grid smoothly.
            let (tx, ty) = (view.x + cell.x, view.y + cell.y);
            let [sx, sy] = &mut self.card_pos[index];
            if !self.placed[index] {
                sx.snap(tx);
                sy.snap(ty);
                self.placed[index] = true;
            }
            sx.set_target(tx);
            sy.set_target(ty);
            sx.step(input.dt);
            sy.step(input.dt);
            if lifted.is_some_and(|c| c.card == index) {
                ghost = Some((n, index));
                continue;
            }
            let mut r = Rect::new(sx.value(), sy.value() - scroll, cell.w, cell.h);
            // A card being pressed sinks a little under the pointer.
            if self.pressed == Some(Hit::Card(n)) && input.held && !self.dragged {
                r = Rect::new(r.x + r.w * 0.012, r.y + r.h * 0.012, r.w * 0.976, r.h * 0.976);
            }
            if r.bottom() < view.y || r.y > view.bottom() {
                continue;
            }
            let glow = self.glow.get(index).map_or(0.0, Spring::value);
            let bar_h = r.h * 0.26;
            // The card is see-through: the blurred world shows through its body, which only brightens under the pointer.
            let lift = |c: Rgba, t: f32, alpha: f32| Color::rgba(c[0] + (1.0 - c[0]) * t, c[1] + (1.0 - c[1]) * t, c[2] + (1.0 - c[2]) * t, alpha);
            let top = lift(p.raised, 0.05 + glow * 0.08, 0.40 + glow * 0.08);
            let bottom = lift(p.raised, 0.02 + glow * 0.06, 0.32 + glow * 0.08);
            let body_rect = d.at(r);
            d.scene.push(Primitive::Rect {
                rect: body_rect,
                fill: Fill::Gradient { from: top, to: bottom, angle: 0.0 },
                radii: Radii::all(px(CARD_RADIUS)),
                border: Some(Border { width: 0.35, color: Color::rgba(1.0, 1.0, 1.0, 0.13 + glow * 0.08) }),
                shadow: None,
            });
            let bar = Rect::new(r.x, r.bottom() - bar_h, r.w, bar_h);
            let bar_rect = d.at(bar);
            d.scene.push(Primitive::Rect {
                rect: bar_rect,
                fill: Fill::Solid(color(if card.on { p.accent } else { p.dim })),
                radii: Radii::bottom(px(CARD_RADIUS)),
                border: None,
                shadow: None,
            });
            d.line(bar, card.name, px(text::LABEL.size * 1.15), if card.on { p.on_accent } else { p.text }, Align::Centre);
            // A soft shadow under the icon lifts it off the card, and an enabled card's dot glows.
            let icon_area = Rect::new(r.x, r.y + px(8.0), r.w, r.h - bar_h - px(8.0));
            let centre = icon_area.centre();
            let halo = d.at(Rect::new(centre.x - px(3.0), centre.y - px(3.0), px(6.0), px(6.0)));
            d.scene.push(Primitive::Rect {
                rect: halo,
                fill: Fill::Solid(Color::rgba(0.0, 0.0, 0.0, 0.0)),
                radii: Radii::all(px(3.0)),
                border: None,
                shadow: Some(vantage_gui::style::Shadow { offset: [0.0, 0.0], blur: px(64.0), color: Color::rgba(0.0, 0.0, 0.0, 0.26) }),
            });
            d.glyph(icon_area, menu_v2::mod_icon(card.id), px(44.0), [0.74, 0.75, 0.88, 1.0]);
            let dot = px(7.0);
            let dot_rect = d.at(Rect::new(r.right() - px(space::L) - dot, r.y + px(space::L), dot, dot));
            d.scene.push(Primitive::Rect {
                rect: dot_rect,
                fill: Fill::Solid(color(if card.on { p.accent } else { p.dim })),
                radii: Radii::all(dot / 2.0),
                border: None,
                shadow: card.on.then(|| vantage_gui::style::Shadow { offset: [0.0, 0.0], blur: px(7.0), color: Color::rgba(p.accent[0], p.accent[1], p.accent[2], 0.5) }),
            });
            if glow > 0.02 {
                let fade = |c: Rgba| [c[0], c[1], c[2], c[3] * glow];
                d.glyph(Rect::new(r.x + px(space::M), r.y + px(space::S), px(20.0), px(20.0)), if card.favorite { Art::StarFilled } else { Art::Star }, px(18.0), if card.favorite { fade(p.star) } else { fade(p.muted) });
                d.glyph(Rect::new(r.x + px(space::M + 22.0), r.y + px(space::S), px(20.0), px(20.0)), if card.hidden { Art::EyeOff } else { Art::Eye }, px(18.0), fade(p.muted));
            } else if card.favorite {
                d.glyph(Rect::new(r.x + px(space::M), r.y + px(space::S), px(20.0), px(20.0)), Art::StarFilled, px(18.0), p.star);
            }
            d.region(Hit::Card(n), Rect::new(r.x, r.y, r.w, r.h - bar_h));
            d.region(Hit::Switch(n), bar);
            d.region(Hit::Star(n), Rect::new(r.x + px(space::M - 1.0), r.y + px(space::S - 1.0), px(22.0), px(22.0)));
            d.region(Hit::Hide(n), Rect::new(r.x + px(space::M + 21.0), r.y + px(space::S - 1.0), px(22.0), px(22.0)));
        }
        // The lifted card follows the pointer, a little see-through, over the others.
        if let (Some((_, index)), Some(drag), Some(pointer)) = (ghost, lifted, input.pointer) {
            if let Some(card) = cards.get(index) {
                let o = d.origin;
                let cell = cells.first().copied().unwrap_or_default();
                let r = Rect::new(pointer.x - o.x - drag.grab.x, pointer.y - o.y - drag.grab.y, cell.w, cell.h);
                d.scene.push(Primitive::PushOpacity(0.92));
                let bar_h = px(26.0);
                let rr = d.at(r);
                d.scene.push(Primitive::Rect {
                    rect: rr,
                    fill: Fill::Solid(color(p.raised)),
                    radii: Radii::all(px(RADII.md)),
                    border: Some(Border { width: 1.0, color: color(p.accent) }),
                    shadow: Some(vantage_gui::style::Shadow { offset: [0.0, 6.0], blur: 14.0, color: Color::rgba(0.0, 0.0, 0.0, 0.5) }),
                });
                let bar = Rect::new(r.x, r.bottom() - bar_h, r.w, bar_h);
                d.line(bar, card.name, px(text::LABEL.size), p.text, Align::Centre);
                d.icon(Rect::new(r.x, r.y + px(14.0), r.w, r.h - bar_h - px(14.0)), card.icon, px(46.0), p.text);
                d.scene.push(Primitive::PopOpacity);
            }
        }
        d.scene.push(Primitive::PopClip);
        if let Some((anchor, text_shown)) = tip {
            let t = ((self.hover_clock - 0.35) / 0.15).clamp(0.0, 1.0);
            if t > 0.0 {
                let w = text_shown.chars().count() as f32 * char_w * 0.9 + px(26.0);
                let r = Rect::new(anchor.right() - w + px(10.0), anchor.bottom() + px(8.0), w, px(28.0));
                d.scene.push(Primitive::PushOpacity(t));
                d.rect(r, [0.05, 0.05, 0.08, 0.96], Radii::all(px(RADII.xs)));
                d.ring(r, Radii::all(px(RADII.xs)), 0.3, [1.0, 1.0, 1.0, 0.14]);
                d.line(r, text_shown, px(text::SMALL.size), p.text, Align::Centre);
                d.scene.push(Primitive::PopOpacity);
            }
        }
        self.overlay(&mut d);
        d.scene.push(Primitive::PopOpacity);
        d.scene
    }

    /// Runs one frame of a settings page.
    pub fn page_frame(&mut self, settings: &Settings, state: &PanelState, tab: usize, input: Input) -> (Scene, Vec<Action>) {
        self.scroll_step(&input, tab);
        self.search_focus(&input);
        let origin = window_origin(input.viewport);

        // A slider being dragged follows the pointer until the button goes up.
        let mut actions = Vec::new();
        if let Some((id, track, min, max, step, last)) = self.drag.clone() {
            if input.held {
                if let Some(pointer) = input.pointer {
                    let value = slider_value(pointer.x - origin.x, track, min, max, step);
                    if (value - last).abs() > f32::EPSILON {
                        actions.push(Action::EventValue(id.clone(), value));
                        self.drag = Some((id, track, min, max, step, value));
                    }
                }
            } else {
                self.drag = None;
            }
        }

        // Keys typed into the focused field.
        if let Some((id, text)) = self.field.as_mut() {
            let typed = input.typed;
            let before = text.clone();
            for _ in 0..typed.backspaces {
                text.pop();
            }
            for c in &typed.chars[..usize::from(typed.len)] {
                if text.chars().count() < 64 {
                    text.push(*c);
                }
            }
            if typed.submit || typed.cancel {
                actions.push(Action::Text { id: id.clone(), text: text.clone(), submit: true });
                self.field = None;
            } else if *text != before {
                actions.push(Action::Text { id: id.clone(), text: text.clone(), submit: false });
            }
        }
        let scene = match self.virt {
            Some(n) => menu_v2::with_virtual(settings, state, n, &mut |page| self.build_page(settings, state, tab, page, input)),
            None => menu_v2::with_page(settings, state, tab, &mut |page| self.build_page(settings, state, tab, page, input)),
        };
        let scene = self.finish(scene);
        let was_pressed = self.pressed;
        let click = self.pointer(&scene, &input);
        // A press anywhere but the field ends typing in it, and a press on a field starts it.
        if input.pressed {
            let on_field = match self.pressed {
                Some(Hit::Act(n)) => match self.acts.get(n) {
                    Some(Act::Field { id, text }) => Some((id.clone(), text.clone())),
                    _ => None,
                },
                _ => None,
            };
            match (on_field, self.field.take()) {
                (Some((id, text)), previous) => {
                    if let Some((old_id, old_text)) = previous.filter(|(old, _)| *old != id) {
                        actions.push(Action::Text { id: old_id, text: old_text, submit: true });
                    }
                    let keep = self.field_text(&id).unwrap_or(text);
                    self.field = Some((id, keep));
                }
                (None, Some((old_id, old_text))) => actions.push(Action::Text { id: old_id, text: old_text, submit: true }),
                (None, None) => {}
            }
        }
        // A press on a slider starts a drag at once.
        if input.pressed {
            if let Some(Hit::Act(n)) = self.pressed {
                if let Some(Act::Slider { id, track, min, max, step }) = self.acts.get(n).cloned() {
                    if let Some(pointer) = input.pointer {
                        let value = slider_value(pointer.x - origin.x, track, min, max, step);
                        actions.push(Action::EventValue(id.clone(), value));
                        self.drag = Some((id, track, min, max, step, value));
                    }
                }
            }
        }
        let _ = was_pressed;
        if let Some(up) = click {
            let routed = self.route(settings, state, up);
            actions.extend(match up {
                _ if routed.is_some() => routed.unwrap_or_default(),
                Hit::Act(n) => match self.acts.get(n) {
                    Some(Act::Toggle { id, on }) => vec![Action::EventValue(id.clone(), if *on { 0.0 } else { 1.0 })],
                    Some(Act::Press(id)) => vec![Action::Event(id.clone())],
                    _ => Vec::new(),
                },
                other => mods_click(other, &[], &[], &[]),
            });
        }
        (scene, actions)
    }

    /// The text typed so far into field `id`, if it is the one being edited.
    fn field_text(&self, id: &str) -> Option<String> {
        self.field.as_ref().filter(|(f, _)| f == id).map(|(_, t)| t.clone())
    }

    fn build_page(&mut self, settings: &Settings, state: &PanelState, tab: usize, page: &menu_v2::PageView<'_>, input: Input) -> Scene {
        let p = Palette::dark();
        let mut d = Painter { scene: Scene::new(), origin: self.window_place(input.viewport), p, hover: self.hover, lit_amounts: self.lit_amounts() };
        d.scene.push(Primitive::PushOpacity(self.open.value()));
        // An element page is titled with the element, by Vantage, with its Style and Layout pages as sub-tabs.
        let sub_tabs = matches!(tab, 1 | 2) && self.virt.is_none();
        let title = if sub_tabs && !page.element.is_empty() { page.element } else { page.title };
        let pane = self.shell(&mut d, title, tab, "Search options...", state.option_query());
        self.acts.clear();
        let pane = if sub_tabs {
            let chip_h = px(28.0);
            let title_w = title.chars().count() as f32 * px(text::HEADING.size) * 0.56;
            d.line(Rect::new(pane[0] + px(space::XXL) + title_w + px(10.0), px(space::XL), px(120.0), px(28.0)), "by Vantage", px(text::CAPTION.size), p.muted, Align::Start);
            let mut x = pane[0];
            for (target, name, glyph) in [(2, "Style", Art::Paintbrush), (1, "Layout", Art::Hud)] {
                let cw = name.chars().count() as f32 * px(text::LABEL.size) * 0.62 * 0.84 + px(46.0);
                let r = Rect::new(x, pane[1], cw, chip_h);
                let hit = Hit::Nav(target);
                let on = tab == target;
                d.rect(r, if on { p.accent } else { d.hot(hit, p.raised, p.dim) }, Radii::all(px(RADII.xs)));
                if !on {
                    d.ring(r, Radii::all(px(RADII.xs)), 0.3, [1.0, 1.0, 1.0, 0.10]);
                }
                let ink = if on { p.on_accent } else { p.text };
                let label_w = name.chars().count() as f32 * px(text::LABEL.size) * 0.62 * 0.92;
                let group = px(18.0) + px(6.0) + label_w;
                let gx = r.x + (r.w - group) / 2.0;
                d.glyph(Rect::new(gx, r.y, px(18.0), r.h), glyph, px(17.0), ink);
                d.line(Rect::new(gx + px(24.0), r.y, label_w + px(12.0), r.h), name, px(text::LABEL.size), ink, Align::Start);
                d.region(hit, r);
                x += cw + px(space::XS);
            }
            [pane[0], pane[1] + chip_h + px(space::L), pane[2], pane[3] - chip_h - px(space::L)]
        } else {
            pane
        };

        // The Crosshair page has a preview card beside its rows.
        let side = if tab == 5 { px(160.0) + px(space::L) } else { 0.0 };
        if tab == 5 {
            self.preview_card(&mut d, Rect::new(pane[0] + pane[2] - px(160.0), pane[1], px(160.0), px(160.0)), crate::menu_v2::crosshair_shapes(settings));
        }
        let view = Rect::new(pane[0], pane[1], pane[2] - side, pane[3]);
        let clip = d.at(Rect::new(pane[0], pane[1], pane[2], pane[3]));
        d.scene.push(Primitive::PushClip { rect: clip, radii: Radii::default() });
        let scroll = self.scroll.value();
        let mut y = view.y - scroll;
        let gap = px(space::S);
        for group in page.groups {
            // The heading collapses its section.
            let heading = Rect::new(view.x, y, view.w, px(24.0));
            let slug = vantage_ui::options::group_id(group.title);
            let n = self.acts.len();
            self.acts.push(Act::Press(slug));
            let hit = Hit::Act(n);
            d.line(Rect::new(heading.x + px(space::S), heading.y, heading.w - px(40.0), heading.h), &group.title.to_uppercase(), px(text::CAPTION.size), d.hot(hit, p.muted, p.text), Align::Start);
            d.line(Rect::new(heading.right() - px(28.0), heading.y, px(20.0), heading.h), if group.collapsed { ">" } else { "v" }, px(text::CAPTION.size), p.muted, Align::Centre);
            d.region(hit, heading);
            y += heading.h + gap;
            if group.collapsed {
                continue;
            }
            for opt in group.options {
                let h = if opt.help.is_some() { px(48.0) } else { px(40.0) };
                let row = Rect::new(view.x, y, view.w, h);
                if row.bottom() >= view.y && row.y <= view.bottom() {
                    self.draw_row(&mut d, opt, row, input.dt);
                }
                y += h + gap;
            }
            y += px(space::M);
        }
        self.content = y + scroll - view.y;
        self.view = view.h;
        d.scene.push(Primitive::PopClip);

        // The open choice's list over everything.
        if let Some(popup) = page.popup {
            let [w, h] = window_px();
            let dim = self.acts.len();
            self.acts.push(Act::Press("pick_close".to_owned()));
            d.rect(Rect::new(0.0, 0.0, w, h), [0.0, 0.0, 0.0, 0.55], Radii::all(px(RADII.lg)));
            d.region(Hit::Act(dim), Rect::new(0.0, 0.0, w, h));
            let rows = popup.options.len() as f32;
            let card_w = px(220.0);
            let card_h = (px(44.0) + rows * px(28.0) + px(space::M)).min(h - px(space::XXL));
            let card = Rect::new((w - card_w) / 2.0, (h - card_h) / 2.0, card_w, card_h);
            d.rect(card, p.raised, Radii::all(px(RADII.md)));
            d.line(Rect::new(card.x + px(space::XL), card.y, card.w, px(44.0)), popup.title, px(text::HEADING.size), p.highlight, Align::Start);
            for (i, name) in popup.options.iter().enumerate() {
                let r = Rect::new(card.x + px(space::M), card.y + px(44.0) + i as f32 * px(28.0), card.w - px(space::M * 2.0), px(26.0));
                if r.bottom() > card.bottom() {
                    break;
                }
                let n = self.acts.len();
                self.acts.push(Act::Press(format!("pick_{i}")));
                let on = i == popup.selected;
                let hit = Hit::Act(n);
                d.rect(r, if on { p.accent } else { d.hot(hit, [0.0; 4], p.dim) }, Radii::all(px(RADII.xs)));
                d.line(Rect::new(r.x + px(space::M), r.y, r.w, r.h), name, px(text::SMALL.size), if on { p.on_accent } else { p.text }, Align::Start);
                d.region(hit, r);
            }
        }
        self.overlay(&mut d);
        d.scene.push(Primitive::PopOpacity);
        d.scene
    }

    /// The crosshair preview: a card with a dark box and the crosshair's shapes in it.
    fn preview_card(&mut self, d: &mut Painter, card: Rect, shapes: Vec<crate::menu_v2::PreviewShape>) {
        let p = d.p;
        d.rect(card, p.raised, Radii::all(px(RADII.md)));
        d.text(Rect::new(card.x + px(space::L), card.y + px(space::M), card.w, px(text::SMALL.size) * 1.5), "Preview", px(text::SMALL.size), p.muted, Align::Start);
        let side = px(crate::menu_v2::PREVIEW_BOX);
        let boxr = Rect::new(card.x + (card.w - side) / 2.0, card.y + px(36.0), side, side);
        d.rect(boxr, [0.0, 0.0, 0.0, 0.35], Radii::all(px(RADII.xs)));
        for shape in shapes {
            match shape {
                crate::menu_v2::PreviewShape::Bar { rect, radius, color } => {
                    d.rect(Rect::new(boxr.x + px(rect[0]), boxr.y + px(rect[1]), px(rect[2]), px(rect[3])), color, Radii::all(px(radius)));
                }
                crate::menu_v2::PreviewShape::Ring { centre, outer, inner, color } => {
                    let r = Rect::new(boxr.x + px(centre[0] - outer), boxr.y + px(centre[1] - outer), px(outer * 2.0), px(outer * 2.0));
                    if inner <= 0.0 {
                        d.rect(r, color, Radii::all(px(outer)));
                    } else {
                        d.ring(r, Radii::all(px(outer)), px(outer - inner), color);
                    }
                }
            }
        }
    }

    /// One option as a row card: its name and help on the left, its control on the right.
    fn draw_row(&mut self, d: &mut Painter, opt: &Opt<'_>, row: Rect, dt: f32) {
        let p = d.p;
        d.rect(row, p.raised, Radii::all(px(RADII.sm)));
        let name_h = px(text::LABEL.size) * 1.5;
        if let Some(help) = opt.help {
            d.text(Rect::new(row.x + px(space::L), row.y + px(6.0), row.w * 0.55, name_h), opt.label, px(text::LABEL.size), p.text, Align::Start);
            d.text(Rect::new(row.x + px(space::L), row.y + px(6.0) + name_h, row.w * 0.6, px(text::CAPTION.size) * 1.5), help, px(text::CAPTION.size), p.muted, Align::Start);
        } else {
            d.line(Rect::new(row.x + px(space::L), row.y, row.w * 0.55, row.h), opt.label, px(text::LABEL.size), p.text, Align::Start);
        }
        let right = row.right() - px(space::L);
        let mid = row.y + row.h / 2.0;
        let real = opt.id.strip_prefix("choose_").unwrap_or(opt.id).to_owned();
        let mut control_left = right;
        match opt.kind {
            Kind::Toggle { on } => {
                let (tw, th) = (px(40.0), px(22.0));
                let track = Rect::new(right - tw, mid - th / 2.0, tw, th);
                let spring = self.knobs.entry(opt.id.to_owned()).or_insert_with(|| Spring::new(if on { 1.0 } else { 0.0 }, 26.0));
                spring.set_target(if on { 1.0 } else { 0.0 });
                spring.step(dt);
                let t = spring.value();
                let fill = color(p.dim).mix(color(p.accent), t);
                let track_rect = d.at(track);
                d.scene.push(Primitive::Rect { rect: track_rect, fill: Fill::Solid(fill), radii: Radii::all(th / 2.0), border: None, shadow: None });
                let knob = px(16.0);
                d.rect(Rect::new(track.x + px(3.0) + (tw - knob - px(6.0)) * t, mid - knob / 2.0, knob, knob), p.highlight, Radii::all(knob / 2.0));
                d.line(Rect::new(track.x - px(34.0), row.y, px(30.0), row.h), if on { "On" } else { "Off" }, px(text::CAPTION.size), p.muted, Align::End);
                control_left = track.x - px(40.0);
                let n = self.acts.len();
                self.acts.push(Act::Toggle { id: opt.id.to_owned(), on });
                d.region(Hit::Act(n), Rect::new(track.x - px(36.0), row.y, tw + px(36.0), row.h));
            }
            Kind::Slider { value, min, max, step } => {
                let box_w = px(46.0);
                let value_box = Rect::new(right - box_w, mid - px(12.0), box_w, px(24.0));
                d.rect(value_box, p.field, Radii::all(px(RADII.xs)));
                d.line(value_box, &number(value, step), px(text::SMALL.size), p.text, Align::Centre);
                let track = Rect::new(value_box.x - px(space::M) - px(150.0), mid - px(2.0), px(150.0), px(4.0));
                let t = if max > min { ((value - min) / (max - min)).clamp(0.0, 1.0) } else { 0.0 };
                d.rect(track, p.dim, Radii::all(px(2.0)));
                d.rect(Rect::new(track.x, track.y, track.w * t, track.h), p.accent, Radii::all(px(2.0)));
                let knob = px(12.0);
                d.rect(Rect::new(track.x + track.w * t - knob / 2.0, mid - knob / 2.0, knob, knob), p.highlight, Radii::all(knob / 2.0));
                control_left = track.x;
                let n = self.acts.len();
                let hit_track = Rect::new(track.x - px(6.0), row.y, track.w + px(12.0), row.h);
                self.acts.push(Act::Slider { id: opt.id.to_owned(), track, min, max, step });
                d.region(Hit::Act(n), hit_track);
            }
            Kind::Choice { options, selected } => {
                let pill = Rect::new(right - px(120.0), mid - px(12.0), px(120.0), px(24.0));
                let n = self.acts.len();
                self.acts.push(Act::Press(opt.id.to_owned()));
                let hit = Hit::Act(n);
                d.rect(pill, d.hot(hit, p.field, p.dim), Radii::all(px(RADII.xs)));
                d.line(Rect::new(pill.x + px(space::M), pill.y, pill.w - px(space::XL), pill.h), options.get(selected).copied().unwrap_or(""), px(text::SMALL.size), p.text, Align::Start);
                d.line(Rect::new(pill.right() - px(20.0), pill.y, px(16.0), pill.h), "v", px(text::CAPTION.size), p.muted, Align::Centre);
                d.region(hit, pill);
                control_left = pill.x;
            }
            Kind::Keybind { key } => {
                let pill = Rect::new(right - px(90.0), mid - px(12.0), px(90.0), px(24.0));
                let n = self.acts.len();
                self.acts.push(Act::Press(opt.id.to_owned()));
                let hit = Hit::Act(n);
                d.rect(pill, d.hot(hit, p.field, p.dim), Radii::all(px(RADII.xs)));
                d.line(pill, key, px(text::SMALL.size), p.text, Align::Centre);
                d.region(hit, pill);
                control_left = pill.x;
            }
            Kind::Button { label } => {
                let w = (label.chars().count() as f32 * px(text::SMALL.size) * 0.62 + px(28.0)).max(px(70.0));
                let pill = Rect::new(right - w, mid - px(12.0), w, px(24.0));
                let n = self.acts.len();
                self.acts.push(Act::Press(opt.id.to_owned()));
                let hit = Hit::Act(n);
                d.rect(pill, d.hot(hit, p.accent, color_up(p.accent)), Radii::all(px(RADII.xs)));
                d.line(pill, label, px(text::SMALL.size), p.on_accent, Align::Centre);
                d.region(hit, pill);
                control_left = pill.x;
            }
            Kind::Text { value } => {
                let w = px(170.0);
                let pill = Rect::new(right - w, mid - px(12.0), w, px(24.0));
                let n = self.acts.len();
                self.acts.push(Act::Field { id: opt.id.to_owned(), text: value.to_owned() });
                let hit = Hit::Act(n);
                let editing = self.field.as_ref().filter(|(id, _)| id == opt.id).map(|(_, t)| t.clone());
                d.rect(pill, if editing.is_some() { p.dim } else { d.hot(hit, p.field, p.dim) }, Radii::all(px(RADII.xs)));
                if editing.is_some() {
                    d.ring(pill, Radii::all(px(RADII.xs)), 1.0, p.accent);
                }
                let shown = editing.as_deref().unwrap_or(value);
                // The end of a long value stays in view while typing.
                let max_chars = ((pill.w - px(space::XL)) / (px(text::SMALL.size) * 0.55)) as usize;
                let count = shown.chars().count();
                let tail: String = shown.chars().skip(count.saturating_sub(max_chars.max(4))).collect();
                let caret = editing.is_some() && (self.clock * 2.0) as u32 % 2 == 0;
                let line = if caret { format!("{tail}|") } else { tail };
                d.line(Rect::new(pill.x + px(space::M), pill.y, pill.w - px(space::M), pill.h), &line, px(text::SMALL.size), p.text, Align::Start);
                d.region(hit, pill);
                control_left = pill.x;
            }
            Kind::Info => {}
        }
        if opt.changed && !matches!(opt.kind, Kind::Info) {
            let reset = Rect::new(control_left - px(52.0), row.y, px(46.0), row.h);
            let n = self.acts.len();
            self.acts.push(Act::Press(format!("{real}_reset")));
            let hit = Hit::Act(n);
            d.line(reset, "Reset", px(text::CAPTION.size), d.hot(hit, p.muted, p.text), Align::Centre);
            d.region(hit, reset);
        }
    }
}

/// What a click on `hit` of the Mods page, or of the shell, asks for.
fn mods_click(hit: Hit, cards: &[vantage_ui::mods::Card<'_>], order: &[usize], chips: &[(&str, usize)]) -> Vec<Action> {
    let card = |n: usize| order.get(n).and_then(|i| cards.get(*i));
    let one = |id: String| vec![Action::Event(id)];
    match hit {
        Hit::Card(n) => card(n).map(|c| one(c.open.to_owned())).unwrap_or_default(),
        Hit::Switch(n) => card(n).map(|c| one(c.id.to_owned())).unwrap_or_default(),
        Hit::Star(n) => card(n).map(|c| one(format!("fav_{}", c.id))).unwrap_or_default(),
        Hit::Hide(n) => card(n).map(|c| one(format!("hide_{}", c.id))).unwrap_or_default(),
        Hit::Chip(n) => chips.get(n).map(|(_, number)| one(format!("flt_{number}"))).unwrap_or_default(),
        Hit::Sort(n) => one(format!("sort_{n}")),
        Hit::Nav(tab) => one(format!("tab_{tab}")),
        Hit::Back => one("back".to_owned()),
        Hit::Close => vec![Action::Close],
        Hit::Act(_) | Hit::Bell => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(pointer: Option<Vec2>) -> Input {
        Input { pointer, viewport: [560.0, 360.0], dt: 0.016, ..Input::default() }
    }

    #[test]
    fn a_frame_draws_the_window_the_cards_and_their_regions() {
        let settings = Settings::default();
        let state = PanelState::default();
        let mut menu = GuiMenu::default();
        let (scene, actions) = menu.frame(&settings, &state, input(None));
        assert!(actions.is_empty());
        assert!(scene.primitives.len() > 100, "{} primitives", scene.primitives.len());
        assert!(scene.regions.iter().any(|r| matches!(Hit::from_id(r.id), Some(Hit::Card(0)))));
        assert!(scene.regions.iter().any(|r| matches!(Hit::from_id(r.id), Some(Hit::Close))));
    }

    #[test]
    fn a_click_on_a_cards_bar_switches_that_mod() {
        let settings = Settings::default();
        let state = PanelState::default();
        let mut menu = GuiMenu::default();
        let (scene, _) = menu.frame(&settings, &state, input(None));
        let region = scene.regions.iter().find(|r| Hit::from_id(r.id) == Some(Hit::Switch(0))).unwrap().rect;
        let at = Some(Vec2::new(region.x + 4.0, region.y + 4.0));
        menu.frame(&settings, &state, Input { pressed: true, held: true, ..input(at) });
        let (_, actions) = menu.frame(&settings, &state, input(at));
        assert_eq!(actions, vec![Action::Event(crate::menu_controls::MODS[0].0.to_owned())]);
    }

    #[test]
    fn releasing_off_the_pressed_control_does_nothing() {
        let settings = Settings::default();
        let state = PanelState::default();
        let mut menu = GuiMenu::default();
        let (scene, _) = menu.frame(&settings, &state, input(None));
        let region = scene.regions.iter().find(|r| Hit::from_id(r.id) == Some(Hit::Close)).unwrap().rect;
        let on = Some(Vec2::new(region.x + 2.0, region.y + 2.0));
        menu.frame(&settings, &state, Input { pressed: true, held: true, ..input(on) });
        let (_, actions) = menu.frame(&settings, &state, input(Some(Vec2::new(1.0, 1.0))));
        assert!(actions.is_empty());
    }

    #[test]
    fn region_ids_round_trip() {
        for hit in [Hit::Card(3), Hit::Switch(7), Hit::Star(1), Hit::Hide(2), Hit::Chip(4), Hit::Sort(2), Hit::Nav(26), Hit::Close, Hit::Back, Hit::Act(40)] {
            assert_eq!(Hit::from_id(hit.id()), Some(hit));
        }
    }

    #[test]
    fn a_settings_page_draws_rows_and_a_switch_click_sends_its_new_state() {
        let mut settings = Settings::default();
        let mut state = PanelState::default();
        state.event(&mut settings, "tab_3", 1.0);
        let mut menu = GuiMenu::default();
        let (scene, _) = menu.page_frame(&settings, &state, 3, input(None));
        assert!(scene.primitives.len() > 40);
        let first_toggle = menu.acts.iter().position(|a| matches!(a, Act::Toggle { .. })).expect("a switch");
        let region = scene.regions.iter().find(|r| Hit::from_id(r.id) == Some(Hit::Act(first_toggle))).unwrap().rect;
        let at = Some(Vec2::new(region.x + 4.0, region.y + 4.0));
        menu.page_frame(&settings, &state, 3, Input { pressed: true, held: true, ..input(at) });
        let (_, actions) = menu.page_frame(&settings, &state, 3, input(at));
        assert!(matches!(actions.as_slice(), [Action::EventValue(_, v)] if *v == 1.0 || *v == 0.0), "{actions:?}");
    }

    #[test]
    fn a_slider_picks_its_value_along_the_track_and_snaps_to_the_step() {
        let track = Rect::new(100.0, 0.0, 200.0, 4.0);
        assert_eq!(slider_value(100.0, track, 0.0, 10.0, 1.0), 0.0);
        assert_eq!(slider_value(300.0, track, 0.0, 10.0, 1.0), 10.0);
        assert_eq!(slider_value(203.0, track, 0.0, 10.0, 1.0), 5.0);
        assert_eq!(slider_value(-50.0, track, 0.0, 10.0, 1.0), 0.0);
    }

    #[test]
    fn keys_type_characters_and_shift_changes_them() {
        let keys = |names: &[&str], shift| typed_from_keys(&names.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(), shift);
        let t = keys(&["KeyA", "Digit1", "Space", "Minus"], false);
        assert_eq!(&t.chars[..usize::from(t.len)], &['a', '1', ' ', '-']);
        let t = keys(&["KeyA", "Digit1", "Minus"], true);
        assert_eq!(&t.chars[..usize::from(t.len)], &['A', '!', '_']);
        let t = keys(&["Backspace", "Enter", "ShiftLeft"], false);
        assert!(t.backspaces == 1 && t.submit && t.len == 0);
    }

    #[test]
    fn numbers_show_as_many_decimals_as_the_step() {
        assert_eq!(number(5.0, 1.0), "5");
        assert_eq!(number(0.25, 0.01), "0.25");
        assert_eq!(number(1.5, 0.1), "1.5");
    }

    #[test]
    fn dragging_a_card_over_another_cell_and_letting_go_sends_its_new_place_and_is_not_a_click() {
        let settings = Settings::default();
        let state = PanelState::default();
        let mut menu = GuiMenu::default();
        // Two frames lay the grid out and let the hover settle on the first card.
        let (scene, _) = menu.frame(&settings, &state, input(None));
        let card = |n: usize| scene.regions.iter().find(|r| Hit::from_id(r.id) == Some(Hit::Card(n))).unwrap().rect;
        let (from, to) = (card(0), card(2));
        let start = Vec2::new(from.x + from.w / 2.0, from.y + from.h / 3.0);
        menu.frame(&settings, &state, input(Some(start)));
        menu.frame(&settings, &state, Input { pressed: true, held: true, ..input(Some(start)) });
        let over = Vec2::new(to.x + to.w / 2.0, to.y + to.h / 3.0);
        menu.frame(&settings, &state, Input { held: true, ..input(Some(over)) });
        let (_, actions) = menu.frame(&settings, &state, input(Some(over)));
        assert_eq!(actions, vec![Action::EventValue(format!("move_{}", crate::menu_controls::MODS[0].0), 2.0)], "{actions:?}");
    }

    #[test]
    fn the_text_keys_fit_the_hosts_reserved_key_limit_with_room_for_the_menu_key() {
        let keys = text_keys();
        assert!(keys.len() < 64, "{} keys", keys.len());
        let mut unique = keys.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), keys.len());
        for needed in ["Backspace", "Enter", "Escape", "ShiftLeft", "KeyA", "Digit0", "Space"] {
            assert!(keys.iter().any(|k| k == needed), "{needed} missing");
        }
    }
}
