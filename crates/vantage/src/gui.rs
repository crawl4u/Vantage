//! The Vantage menu on the GUI toolkit (`vantage-gui`): the Mods page and the settings pages are scenes built
//! here each frame from the settings, with hover, press, drag, scrolling and clicks handled from raw pointer
//! input, and the scene is handed to the host's painter. Clicks become the same events the JSON menu's controls
//! sent, so the settings logic is shared.

use std::collections::HashMap;

use vantage_gui::{
    anim::Spring,
    geom::{Rect, Vec2},
    layout,
    scene::{Align, Primitive, Scene},
    style::{Border, Color, Fill, Radii},
};
use vantage_ui::{
    icons,
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

/// What the pointer did this frame, in the scene's units (the viewport's top left is the origin).
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
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
    Close,
}

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
}

impl Painter {
    fn at(&self, r: Rect) -> Rect {
        Rect::new(self.origin.x + r.x, self.origin.y + r.y, r.w, r.h)
    }

    fn lit(&self, h: Hit) -> bool {
        self.hover == Some(h)
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
        self.text(Rect::new(r.x, r.y + (r.h - h) / 2.0, r.w, h), s, size, c, align);
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
}

impl Default for GuiMenu {
    fn default() -> Self {
        Self {
            scroll: Spring::new(0.0, 22.0),
            scroll_target: 0.0,
            hover: None,
            pressed: None,
            glow: Vec::new(),
            content: 0.0,
            view: 0.0,
            searching: false,
            tab: usize::MAX,
            acts: Vec::new(),
            drag: None,
            knobs: HashMap::new(),
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
    fn scroll_step(&mut self, input: &Input, tab: usize) {
        if self.tab != tab {
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
        let scene = self.build_mods(state, &cards, &order, &chips, active_chip, active_sort, input);
        let mut actions = Vec::new();
        if let Some(up) = self.pointer(&scene, &input) {
            actions.extend(mods_click(up, &cards, &order, &chips));
        }
        (scene, actions)
    }

    /// The window, sidebar, header and search pill common to every page. Returns the content pane.
    fn shell(&mut self, d: &mut Painter, title: &str, tab: usize, hint: &str, query: &str) -> [f32; 4] {
        let p = d.p;
        let [w, h] = window_px();
        d.rect(Rect::new(0.0, 0.0, w, h), p.panel, Radii::all(px(RADII.lg)));

        // Sidebar and logo.
        let side = px(SIDEBAR_WIDTH);
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
        let mut y = px(space::XXL * 3.0);
        for (heading, entries) in menu_v2::sidebar(tab) {
            d.text(Rect::new(px(space::XXL), y, side, px(20.0)), &heading.to_uppercase(), px(text::CAPTION.size), p.muted, Align::Start);
            y += px(24.0);
            for (id, label, glyph, active) in entries {
                let target = id.strip_prefix("tab_").and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
                let row = Rect::new(px(space::XL), y, side - px(space::XL * 2.0), px(36.0));
                let hit = Hit::Nav(target);
                if active {
                    d.rect(row, p.accent, Radii::all(px(RADII.sm)));
                } else if d.lit(hit) {
                    d.rect(row, [1.0, 1.0, 1.0, 0.06], Radii::all(px(RADII.sm)));
                }
                let ink = if active { p.on_accent } else { p.text };
                d.icon(Rect::new(row.x + px(space::M), row.y, px(28.0), row.h), glyph, px(30.0), ink);
                d.line(Rect::new(row.x + px(54.0), row.y, row.w - px(60.0), row.h), label, px(text::LABEL.size), ink, Align::Start);
                d.region(hit, row);
                y += px(40.0);
            }
            y += px(space::M);
        }

        // Header: arrow, title, search pill, close.
        let pane = vantage_ui::shell::content_rect();
        let back = Rect::new(pane[0] - px(space::S), px(space::XL), px(24.0), px(28.0));
        d.icon(back, icons::CHEVRON_LEFT, px(26.0), if d.lit(Hit::Back) { p.text } else { p.muted });
        d.region(Hit::Back, back);
        d.line(Rect::new(pane[0] + px(space::XXL), px(space::XL), px(300.0), px(28.0)), title, px(text::HEADING.size), p.highlight, Align::Start);
        let close = Rect::new(w - px(space::XXL + 28.0), px(space::XL), px(28.0), px(28.0));
        if d.lit(Hit::Close) {
            d.rect(close, [1.0, 1.0, 1.0, 0.08], Radii::all(px(RADII.xs)));
        }
        d.icon(close, icons::X, px(30.0), p.text);
        d.region(Hit::Close, close);
        let pill = search_rect();
        d.rect(pill, if self.searching { p.dim } else { p.field }, Radii::all(px(RADII.xs)));
        let lens = Rect::new(pill.x + px(space::L), pill.y + (pill.h - px(10.0)) / 2.0 - px(1.0), px(10.0), px(10.0));
        d.ring(lens, Radii::all(px(5.0)), px(1.4), p.muted);
        let handle = d.at(Rect::new(lens.right() - px(2.0), lens.bottom() - px(2.0), px(5.0), px(5.0)));
        d.scene.push(Primitive::Path { rect: handle, view: [1.0, 1.0], contours: vec![vec![[0.0, 0.0], [1.0, 1.0]]], color: color(p.muted), stroke: Some(0.3) });
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
        let mut d = Painter { scene: Scene::new(), origin: window_origin(input.viewport), p, hover: self.hover };
        let pane = self.shell(&mut d, "Mods", crate::menu_controls::MODS_PAGE, "Search mods...", state.search_text());

        // Chips and the sort segments.
        let chip_y = pane[1];
        let chip_h = px(28.0);
        let char_w = px(text::LABEL.size) * 0.62;
        let mut x = pane[0];
        for (n, (name, number)) in chips.iter().enumerate() {
            let cw = name.chars().count() as f32 * char_w + px(24.0);
            let on = *number == active_chip;
            let r = Rect::new(x, chip_y, cw, chip_h);
            let hit = Hit::Chip(n);
            let fill = if on { p.accent } else if d.lit(hit) { p.dim } else { p.raised };
            d.rect(r, fill, Radii::all(px(RADII.xs)));
            d.line(r, name, px(text::LABEL.size), if on { p.on_accent } else { p.text }, Align::Centre);
            d.region(hit, r);
            x += cw + px(space::XS);
        }
        let sorts = ["Custom", "A-Z", "On"];
        let widths: Vec<f32> = sorts.iter().map(|s| s.len() as f32 * char_w + px(18.0)).collect();
        let mut sx = pane[0] + pane[2] - widths.iter().sum::<f32>();
        for (n, name) in sorts.iter().enumerate() {
            let r = Rect::new(sx, chip_y, widths[n], chip_h);
            let hit = Hit::Sort(n);
            let on = n == active_sort;
            d.rect(r, if on { p.accent } else if d.lit(hit) { p.dim } else { p.raised }, Radii::all(px(RADII.xs)));
            d.line(r, name, px(text::LABEL.size), if on { p.on_accent } else { p.muted }, Align::Centre);
            d.region(hit, r);
            sx += widths[n];
        }

        // The grid, scrolled and clipped.
        let grid_top = chip_y + chip_h + px(space::L);
        let view = Rect::new(pane[0], grid_top, pane[2], pane[1] + pane[3] - grid_top);
        let gap = px(space::M);
        let cell_w = (view.w - gap * (COLUMNS as f32 - 1.0)) / COLUMNS as f32;
        let (cells, content) = layout::grid(order.len(), [cell_w, cell_w * 0.78], gap, view.w);
        self.content = content;
        self.view = view.h;
        let clip = d.at(view);
        d.scene.push(Primitive::PushClip { rect: clip, radii: Radii::default() });
        let scroll = self.scroll.value();
        for (n, cell) in cells.iter().enumerate() {
            let Some(card) = order.get(n).and_then(|i| cards.get(*i)) else { continue };
            let r = Rect::new(view.x + cell.x, view.y + cell.y - scroll, cell.w, cell.h);
            if r.bottom() < view.y || r.y > view.bottom() {
                continue;
            }
            let glow = self.glow.get(order[n]).map_or(0.0, Spring::value);
            let bar_h = px(26.0);
            let body = color(p.raised).mix(Color::rgba(1.0, 1.0, 1.0, 0.12), glow * 0.5);
            let body_rect = d.at(r);
            d.scene.push(Primitive::Rect { rect: body_rect, fill: Fill::Solid(body), radii: Radii::all(px(RADII.md)), border: None, shadow: None });
            let bar = Rect::new(r.x, r.bottom() - bar_h, r.w, bar_h);
            let bar_rect = d.at(bar);
            d.scene.push(Primitive::Rect {
                rect: bar_rect,
                fill: Fill::Solid(color(if card.on { p.accent } else { p.dim })),
                radii: Radii::bottom(px(RADII.md)),
                border: None,
                shadow: None,
            });
            d.line(bar, card.name, px(text::LABEL.size), if card.on { p.on_accent } else { p.text }, Align::Centre);
            d.icon(Rect::new(r.x, r.y + px(14.0), r.w, r.h - bar_h - px(14.0)), card.icon, px(46.0), p.text);
            let dot = px(8.0);
            d.rect(Rect::new(r.right() - px(space::L) - dot, r.y + px(space::L), dot, dot), if card.on { p.accent } else { p.dim }, Radii::all(dot / 2.0));
            if glow > 0.02 {
                let fade = |c: Rgba| [c[0], c[1], c[2], c[3] * glow];
                d.icon(Rect::new(r.x + px(space::M), r.y + px(space::S), px(20.0), px(20.0)), icons::STAR, px(21.0), if card.favorite { fade(p.star) } else { fade(p.muted) });
                d.icon(Rect::new(r.x + px(space::M + 22.0), r.y + px(space::S), px(20.0), px(20.0)), icons::EYE, px(21.0), fade(p.muted));
            } else if card.favorite {
                d.icon(Rect::new(r.x + px(space::M), r.y + px(space::S), px(20.0), px(20.0)), icons::STAR, px(21.0), p.star);
            }
            d.region(Hit::Card(n), Rect::new(r.x, r.y, r.w, r.h - bar_h));
            d.region(Hit::Switch(n), bar);
            d.region(Hit::Star(n), Rect::new(r.x + px(space::M - 1.0), r.y + px(space::S - 1.0), px(22.0), px(22.0)));
            d.region(Hit::Hide(n), Rect::new(r.x + px(space::M + 21.0), r.y + px(space::S - 1.0), px(22.0), px(22.0)));
        }
        d.scene.push(Primitive::PopClip);
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

        let scene = menu_v2::with_page(settings, state, tab, &mut |page| self.build_page(state, tab, page, input));
        let was_pressed = self.pressed;
        let click = self.pointer(&scene, &input);
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
            actions.extend(match up {
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

    fn build_page(&mut self, state: &PanelState, tab: usize, page: &menu_v2::PageView<'_>, input: Input) -> Scene {
        let p = Palette::dark();
        let mut d = Painter { scene: Scene::new(), origin: window_origin(input.viewport), p, hover: self.hover };
        let pane = self.shell(&mut d, page.title, tab, "Search options...", state.option_query());
        self.acts.clear();

        let view = Rect::new(pane[0], pane[1], pane[2], pane[3]);
        let clip = d.at(view);
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
            d.line(Rect::new(heading.x + px(space::S), heading.y, heading.w - px(40.0), heading.h), &group.title.to_uppercase(), px(text::CAPTION.size), if d.lit(hit) { p.text } else { p.muted }, Align::Start);
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
                d.rect(r, if on { p.accent } else if d.lit(hit) { p.dim } else { [0.0; 4] }, Radii::all(px(RADII.xs)));
                d.line(Rect::new(r.x + px(space::M), r.y, r.w, r.h), name, px(text::SMALL.size), if on { p.on_accent } else { p.text }, Align::Start);
                d.region(hit, r);
            }
        }
        d.scene
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
                d.rect(pill, if d.lit(hit) { p.dim } else { p.field }, Radii::all(px(RADII.xs)));
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
                d.rect(pill, if d.lit(hit) { p.dim } else { p.field }, Radii::all(px(RADII.xs)));
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
                d.rect(pill, if d.lit(hit) { color_up(p.accent) } else { p.accent }, Radii::all(px(RADII.xs)));
                d.line(pill, label, px(text::SMALL.size), p.on_accent, Align::Centre);
                d.region(hit, pill);
                control_left = pill.x;
            }
            Kind::Text { .. } | Kind::Info => {}
        }
        if opt.changed && !matches!(opt.kind, Kind::Info | Kind::Text { .. }) {
            let reset = Rect::new(control_left - px(52.0), row.y, px(46.0), row.h);
            let n = self.acts.len();
            self.acts.push(Act::Press(format!("{real}_reset")));
            let hit = Hit::Act(n);
            d.line(reset, "Reset", px(text::CAPTION.size), if d.lit(hit) { p.text } else { p.muted }, Align::Centre);
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
        Hit::Act(_) => Vec::new(),
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
    fn numbers_show_as_many_decimals_as_the_step() {
        assert_eq!(number(5.0, 1.0), "5");
        assert_eq!(number(0.25, 0.01), "0.25");
        assert_eq!(number(1.5, 0.1), "1.5");
    }
}
