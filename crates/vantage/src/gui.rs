//! The Mods page on the Vantage GUI toolkit (`vantage-gui`): the whole page is a scene built here each frame
//! from the settings, with hover, press, scrolling and clicks handled from raw pointer input, and the scene is
//! handed to the host's painter. Clicks become the same events the JSON menu's controls sent, so the settings
//! logic is shared.

use vantage_gui::{
    anim::Spring,
    geom::{Rect, Vec2},
    layout,
    scene::{Align, Primitive, Scene},
    style::{Color, Fill, Radii},
};
use vantage_ui::{
    icons,
    shell::{SIDEBAR_WIDTH, window_px},
    tokens::{Palette, RADII, Rgba, space, text, px},
};

use crate::config::Settings;
use crate::menu_v2;
use crate::panel::PanelState;

/// The face the menu's text uses.
const FONT: &str = "mod_panel_display";
/// Cards per row, as the JSON Mods page has them.
const COLUMNS: usize = 4;

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
}

/// Something the page wants done.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// An event for the settings logic, by the id the JSON menu's control had.
    Event(String),
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
            _ => return None,
        })
    }
}

fn color(c: Rgba) -> Color {
    Color::rgba(c[0], c[1], c[2], c[3])
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
}

impl Default for GuiMenu {
    fn default() -> Self {
        Self { scroll: Spring::new(0.0, 22.0), scroll_target: 0.0, hover: None, pressed: None, glow: Vec::new(), content: 0.0, view: 0.0 }
    }
}

/// Where the window sits in a viewport of `viewport`, centred.
fn window_origin(viewport: [f32; 2]) -> Vec2 {
    let [w, h] = window_px();
    Vec2::new(((viewport[0] - w) / 2.0).max(0.0), ((viewport[1] - h) / 2.0).max(0.0))
}

impl GuiMenu {
    /// Runs one frame: moves the scroll and the glows, builds the scene for `input`, and returns it with the
    /// actions the pointer asked for.
    pub fn frame(&mut self, settings: &Settings, state: &PanelState, input: Input) -> (Scene, Vec<Action>) {
        let (cards, order, chips, active_chip, active_sort) = menu_v2::grid(settings, state);
        if self.glow.len() < cards.len() {
            self.glow.resize(cards.len(), Spring::new(0.0, 18.0));
        }
        // Wheel and scroll limits.
        let max = (self.content - self.view).max(0.0);
        self.scroll_target = (self.scroll_target - input.wheel * px(60.0)).clamp(0.0, max);
        self.scroll.set_target(self.scroll_target);
        self.scroll.step(input.dt);

        // Last frame's hover decides which cards glow; this frame's scene is hit-tested after it is built.
        for (i, glow) in self.glow.iter_mut().enumerate() {
            let on = matches!(self.hover, Some(Hit::Card(n) | Hit::Switch(n) | Hit::Star(n) | Hit::Hide(n)) if n == i);
            glow.set_target(if on { 1.0 } else { 0.0 });
            glow.step(input.dt);
        }

        let scene = self.build(settings, state, &cards, &order, &chips, active_chip, active_sort, input);

        // Hit testing against the scene just built, then clicks.
        let mut actions = Vec::new();
        let hit = input.pointer.and_then(|p| scene.hit(p)).and_then(Hit::from_id);
        self.hover = hit;
        if input.pressed {
            self.pressed = hit;
        }
        if !input.held {
            if let (Some(down), Some(up)) = (self.pressed.take(), hit) {
                if down == up {
                    actions.extend(self.click(up, &cards, &order, &chips, state));
                }
            }
        }
        (scene, actions)
    }

    /// What a click on `hit` asks for.
    fn click(&mut self, hit: Hit, cards: &[vantage_ui::mods::Card<'_>], order: &[usize], chips: &[(&str, usize)], _state: &PanelState) -> Vec<Action> {
        let card = |n: usize| order.get(n).and_then(|i| cards.get(*i));
        match hit {
            Hit::Card(n) => card(n).map(|c| Action::Event(c.open.to_owned())).into_iter().collect(),
            Hit::Switch(n) => card(n).map(|c| Action::Event(c.id.to_owned())).into_iter().collect(),
            Hit::Star(n) => card(n).map(|c| Action::Event(format!("fav_{}", c.id))).into_iter().collect(),
            Hit::Hide(n) => card(n).map(|c| Action::Event(format!("hide_{}", c.id))).into_iter().collect(),
            Hit::Chip(n) => chips.get(n).map(|(_, number)| Action::Event(format!("flt_{number}"))).into_iter().collect(),
            Hit::Sort(n) => vec![Action::Event(format!("sort_{n}"))],
            Hit::Nav(tab) => vec![Action::Event(format!("tab_{tab}"))],
            Hit::Close => vec![Action::Close],
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        &mut self,
        _settings: &Settings,
        _state: &PanelState,
        cards: &[vantage_ui::mods::Card<'_>],
        order: &[usize],
        chips: &[(&str, usize)],
        active_chip: usize,
        active_sort: usize,
        input: Input,
    ) -> Scene {
        let p = Palette::dark();
        let mut scene = Scene::new();
        let [w, h] = window_px();
        let origin = window_origin(input.viewport);
        let at = |r: Rect| Rect::new(origin.x + r.x, origin.y + r.y, r.w, r.h);
        let rect = |scene: &mut Scene, r: Rect, fill: Rgba, radii: Radii| {
            scene.push(Primitive::Rect { rect: at(r), fill: Fill::Solid(color(fill)), radii, border: None, shadow: None });
        };
        let label = |scene: &mut Scene, r: Rect, s: &str, size: f32, c: Rgba, align: Align| {
            scene.push(Primitive::Text { rect: at(r), text: s.to_owned(), size, color: color(c), align, font: FONT });
        };
        let icon = |scene: &mut Scene, r: Rect, glyph: &str, size: f32, c: Rgba| {
            if let Some(g) = glyph.chars().next() {
                scene.push(Primitive::Icon { rect: at(r), glyph: g, size, color: color(c) });
            }
        };
        let hover = self.hover;
        let lit = |h: Hit| hover == Some(h);

        // The window.
        rect(&mut scene, Rect::new(0.0, 0.0, w, h), p.panel, Radii::all(px(RADII.lg)));

        // Sidebar.
        let side = px(SIDEBAR_WIDTH);
        let mut y = px(space::XXL * 3.0);
        label(&mut scene, Rect::new(px(space::XXL), px(space::XL), side - px(space::XXL * 2.0), px(28.0)), "VANTAGE", px(text::TITLE.size), p.highlight, Align::Start);
        for (heading, entries) in menu_v2::sidebar(0) {
            label(&mut scene, Rect::new(px(space::XXL), y, side, px(20.0)), &heading.to_uppercase(), px(text::CAPTION.size), p.muted, Align::Start);
            y += px(24.0);
            for (id, title, glyph, active) in entries {
                let tab = id.strip_prefix("tab_").and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
                let row = Rect::new(px(space::XL), y, side - px(space::XL * 2.0), px(36.0));
                let hit = Hit::Nav(tab);
                if active {
                    rect(&mut scene, row, p.accent, Radii::all(px(RADII.sm)));
                } else if lit(hit) {
                    rect(&mut scene, row, [1.0, 1.0, 1.0, 0.06], Radii::all(px(RADII.sm)));
                }
                icon(&mut scene, Rect::new(row.x + px(space::M), row.y, px(28.0), row.h), glyph, px(30.0), if active { p.on_accent } else { p.text });
                label(&mut scene, Rect::new(row.x + px(54.0), row.y + px(7.0), row.w - px(60.0), px(22.0)), title, px(text::LABEL.size), if active { p.on_accent } else { p.text }, Align::Start);
                scene.region(hit.id(), at(row));
                y += px(40.0);
            }
            y += px(space::M);
        }

        // Header.
        let pane = vantage_ui::shell::content_rect();
        label(&mut scene, Rect::new(pane[0], px(space::XL), px(300.0), px(28.0)), "Mods", px(text::HEADING.size), p.highlight, Align::Start);
        let close = Rect::new(w - px(space::XXL + 28.0), px(space::XL), px(28.0), px(28.0));
        if lit(Hit::Close) {
            rect(&mut scene, close, [1.0, 1.0, 1.0, 0.08], Radii::all(px(RADII.xs)));
        }
        icon(&mut scene, close, icons::X, px(30.0), p.text);
        scene.region(Hit::Close.id(), at(close));

        // Chips and the sort segments.
        let chip_y = pane[1];
        let chip_h = px(28.0);
        let mut x = pane[0];
        for (n, (name, number)) in chips.iter().enumerate() {
            let cw = name.chars().count() as f32 * px(text::LABEL.size) * 0.62 + px(24.0);
            let on = *number == active_chip;
            let r = Rect::new(x, chip_y, cw, chip_h);
            let hit = Hit::Chip(n);
            let fill = if on { p.accent } else if lit(hit) { p.dim } else { p.raised };
            rect(&mut scene, r, fill, Radii::all(px(RADII.xs)));
            label(&mut scene, Rect::new(r.x, r.y + (chip_h - px(text::LABEL.size) * 1.5) / 2.0, cw, px(text::LABEL.size) * 1.5), name, px(text::LABEL.size), if on { p.on_accent } else { p.text }, Align::Centre);
            scene.region(hit.id(), at(r));
            x += cw + px(space::XS);
        }
        let sorts = ["Custom", "A-Z", "On"];
        let mut sx = pane[0] + pane[2] - sorts.iter().map(|s| s.len() as f32 * px(text::LABEL.size) * 0.62 + px(18.0)).sum::<f32>();
        for (n, name) in sorts.iter().enumerate() {
            let sw = name.len() as f32 * px(text::LABEL.size) * 0.62 + px(18.0);
            let r = Rect::new(sx, chip_y, sw, chip_h);
            let hit = Hit::Sort(n);
            let on = n == active_sort;
            rect(&mut scene, r, if on { p.accent } else if lit(hit) { p.dim } else { p.raised }, Radii::all(px(RADII.xs)));
            label(&mut scene, Rect::new(r.x, r.y + (chip_h - px(text::LABEL.size) * 1.5) / 2.0, sw, px(text::LABEL.size) * 1.5), name, px(text::LABEL.size), if on { p.on_accent } else { p.muted }, Align::Centre);
            scene.region(hit.id(), at(r));
            sx += sw;
        }

        // The grid, scrolled and clipped.
        let grid_top = chip_y + chip_h + px(space::L);
        let view = Rect::new(pane[0], grid_top, pane[2], pane[1] + pane[3] - grid_top);
        let gap = px(space::M);
        let cell_w = (view.w - gap * (COLUMNS as f32 - 1.0)) / COLUMNS as f32;
        let (cells, content) = layout::grid(order.len(), [cell_w, cell_w * 0.78], gap, view.w);
        self.content = content;
        self.view = view.h;
        scene.push(Primitive::PushClip { rect: at(view), radii: Radii::default() });
        let scroll = self.scroll.value();
        for (n, cell) in cells.iter().enumerate() {
            let Some(card) = order.get(n).and_then(|i| cards.get(*i)) else { continue };
            let r = Rect::new(view.x + cell.x, view.y + cell.y - scroll, cell.w, cell.h);
            if r.bottom() < view.y || r.y > view.bottom() {
                continue;
            }
            let glow = self.glow.get(order[n]).map_or(0.0, Spring::value);
            let bar_h = px(26.0);
            let body = Color::rgba(p.raised[0], p.raised[1], p.raised[2], p.raised[3]).mix(Color::rgba(1.0, 1.0, 1.0, 0.12), glow * 0.5);
            scene.push(Primitive::Rect { rect: at(r), fill: Fill::Solid(body), radii: Radii::all(px(RADII.md)), border: None, shadow: None });
            let bar = Rect::new(r.x, r.bottom() - bar_h, r.w, bar_h);
            let bar_color = if card.on { p.accent } else { p.dim };
            scene.push(Primitive::Rect { rect: at(bar), fill: Fill::Solid(color(bar_color)), radii: Radii::bottom(px(RADII.md)), border: None, shadow: None });
            label(&mut scene, Rect::new(bar.x, bar.y + (bar_h - px(text::LABEL.size) * 1.5) / 2.0, bar.w, px(text::LABEL.size) * 1.5), card.name, px(text::LABEL.size), if card.on { p.on_accent } else { p.text }, Align::Centre);
            icon(&mut scene, Rect::new(r.x, r.y + px(14.0), r.w, r.h - bar_h - px(14.0)), card.icon, px(46.0), p.text);
            // The status dot, and the star and the eye that show while the card is lit.
            let dot = px(8.0);
            rect(&mut scene, Rect::new(r.right() - px(space::L) - dot, r.y + px(space::L), dot, dot), if card.on { p.accent } else { p.dim }, Radii::all(dot / 2.0));
            let reveal = glow;
            if reveal > 0.02 {
                let fade = |c: Rgba| [c[0], c[1], c[2], c[3] * reveal];
                icon(&mut scene, Rect::new(r.x + px(space::M), r.y + px(space::S), px(20.0), px(20.0)), icons::STAR, px(21.0), if card.favorite { fade(p.star) } else { fade(p.muted) });
                icon(&mut scene, Rect::new(r.x + px(space::M + 22.0), r.y + px(space::S), px(20.0), px(20.0)), icons::EYE, px(21.0), fade(p.muted));
            } else if card.favorite {
                icon(&mut scene, Rect::new(r.x + px(space::M), r.y + px(space::S), px(20.0), px(20.0)), icons::STAR, px(21.0), p.star);
            }
            // Regions: the card opens, the bar switches, the corner controls act.
            let (card_hit, switch_hit) = (Hit::Card(n), Hit::Switch(n));
            scene.region(card_hit.id(), at(Rect::new(r.x, r.y, r.w, r.h - bar_h)));
            scene.region(switch_hit.id(), at(bar));
            scene.region(Hit::Star(n).id(), at(Rect::new(r.x + px(space::M - 1.0), r.y + px(space::S - 1.0), px(22.0), px(22.0))));
            scene.region(Hit::Hide(n).id(), at(Rect::new(r.x + px(space::M + 21.0), r.y + px(space::S - 1.0), px(22.0), px(22.0))));
        }
        scene.push(Primitive::PopClip);
        scene
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
        let down = Input { pressed: true, held: true, ..input(at) };
        menu.frame(&settings, &state, down);
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
        for hit in [Hit::Card(3), Hit::Switch(7), Hit::Star(1), Hit::Hide(2), Hit::Chip(4), Hit::Sort(2), Hit::Nav(26), Hit::Close] {
            assert_eq!(Hit::from_id(hit.id()), Some(hit));
        }
    }
}
