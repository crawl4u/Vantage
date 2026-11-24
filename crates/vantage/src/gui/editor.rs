//! The HUD editor's chrome, drawn by the toolkit over the game while HUD elements are being moved: a toolbar across the
//! top, a library of HUDs to add on the left, the selected HUD's settings on the right and a pill of shortcuts at the
//! bottom. The elements themselves, their frames and handles are the HUD layer's; this only draws and handles what
//! surrounds them, and says which rectangles take a press so a press on one does not pick up the HUD beneath.
//! While the pointer is held anywhere else the whole chrome slides off the screen, and comes back when it is let go.

use super::*;
use crate::config::Element;

/// The toolbar's place and size, in design units.
const BAR_TOP: f32 = 12.0;
const BAR_HEIGHT: f32 = 44.0;
const BUTTON_HEIGHT: f32 = 30.0;
/// The side docks.
const DOCK_WIDTH: f32 = 252.0;
/// The tallest a side panel gets, in design units; a shorter screen makes it shorter still.
const PANEL_MAX_HEIGHT: f32 = 540.0;
const DOCK_EDGE: f32 = 12.0;
/// The library's cards.
const CARD_HEIGHT: f32 = 86.0;
const LIBRARY_WIDTH: f32 = 228.0;
/// The shortcuts pill and its sheet.
const PILL_SIZE: [f32; 2] = [190.0, 28.0];
const PILL_BOTTOM: f32 = 14.0;
/// The shortcuts: the key or gesture, shown as a key cap, and what it does.
const SHORTCUTS: [(&str, &str); 7] = [
    ("Drag", "Move a HUD"),
    ("Drag a corner", "Resize a HUD"),
    ("Shift", "Hold to place a HUD freely, without snapping"),
    ("Ctrl + drag", "On Potion status, move the whole list"),
    ("Gear", "Open the HUD's settings page"),
    ("X", "Take the HUD off the screen"),
    ("Done", "Finish editing; the menu key does too"),
];
/// The shortcuts sheet's head, row and tail heights.
const SHEET_HEAD: f32 = 40.0;
const SHEET_ROW: f32 = 28.0;
const SHEET_TAIL: f32 = 8.0;
const SHEET_WIDTH: f32 = 380.0;
/// The guided tour's steps: a title and what it says.
const TOUR: [(&str, &str); 6] = [
    ("Welcome to the HUD editor", "This is where you put your HUDs where you want them. A short tour shows what is here."),
    ("Adding HUDs", "HUDs opens the library. Click a card to add that HUD, or drag it to where it should go. The search field finds one by name."),
    ("Moving and resizing", "Drag a HUD to move it and drag its corners to resize it. Hold Shift to move it freely, and use the arrow keys to nudge it a pixel."),
    ("Settings", "The bar over the picked HUD locks it, opens its page or takes it off the screen. Settings opens its layout and style beside the screen."),
    ("Lines, the grid and undo", "Lines and Grid help you line things up, and the grid is what HUDs snap to. Undo and redo are in the toolbar too."),
    ("That is all", "Right-click for more, and Keyboard shortcuts at the bottom lists the keys. Press Done when you are finished."),
];
/// How far the chrome must be shown before it takes presses, so a panel sliding in or out is not pressed by mistake.
const SHOWN: f32 = 0.95;

/// What the library and the dock say under their titles.
const LIBRARY_HINT: &str = "Click one to add it to the screen";
const DOCK_HINT: &str = "Drag a HUD on the screen to pick it";

/// How many GUI pixels apart the grid's lines are, which the HUDs snap to while it shows.
pub const GRID_STEP: f32 = 8.0;

/// One button of the toolbar.
struct Tool {
    label: &'static str,
    art: Art,
    id: &'static str,
    lit: bool,
    primary: bool,
    enabled: bool,
}

/// Whether a group of a page belongs on the dock's open tab: Layout has the place and the size, Style the text, box and
/// border, and Settings the rest, which is what the HUD itself offers (and the armor and chunk options of those HUDs).
fn dock_group_shown(tab: u8, page: usize, title: &str, element: Element) -> bool {
    let style = matches!(title, "Text" | "Box" | "Chroma" | "Extras" | "Border");
    match (tab, page) {
        (0, 1) => matches!(title, "Position" | "Size"),
        (1, 2) => style,
        (_, 2) if tab == 2 => !style,
        (2, 1) => (title == "Armor" && element == Element::Armor) || (title == "Chunk info" && element == Element::Chunk),
        _ => false,
    }
}

/// The box a line between two points fills, so a line is drawn in a view as big as the screen it crosses.
fn ctx_extent(line: &[[f32; 2]; 2]) -> [f32; 2] {
    [line[0][0].max(line[1][0]).max(1.0), line[0][1].max(line[1][1]).max(1.0)]
}

/// A colour with no opacity, so a fill eases in from nothing without passing through another colour.
fn clear(c: Rgba) -> Rgba {
    [c[0], c[1], c[2], 0.0]
}

/// A colour a little lighter and as opaque as it was, for what is under the pointer.
fn lift(c: Rgba) -> Rgba {
    [(c[0] + 0.12).min(1.0), (c[1] + 0.12).min(1.0), (c[2] + 0.14).min(1.0), c[3]]
}

/// Drags one list's scroll bar: a press on the thumb holds it, a press on the track beside it centres the thumb there.
/// Returns the scroll the pointer asks for while the bar is held.
fn drag_bar(grab: &mut Option<f32>, bar: Option<(Rect, Rect)>, input: &Input, max: f32) -> Option<f32> {
    let (track, thumb) = bar?;
    if !input.held {
        *grab = None;
    }
    if input.pressed {
        let area = Rect::new(track.x - px(10.0), track.y, track.w + px(20.0), track.h);
        if let Some(p) = input.pointer.filter(|p| area.contains(*p)) {
            *grab = Some(if p.y >= thumb.y && p.y <= thumb.bottom() { p.y - thumb.y } else { thumb.h / 2.0 });
        }
    }
    let (grab, p) = ((*grab)?, input.pointer?);
    let room = (track.h - thumb.h).max(1.0);
    Some(((p.y - grab - track.y) / room).clamp(0.0, 1.0) * max)
}

/// What the editor's panels are doing between frames.
#[derive(Default)]
pub(super) struct EditorState {
    library: bool,
    dock: bool,
    sheet: bool,
    /// The dock's open tab: Layout (0), Style (1) or Settings (2).
    dock_tab: u8,
    /// What the dock's search field holds.
    dock_query: String,
    /// Where each list is drawn now, where it is heading, and the spring that eases one to the other.
    library_scroll: f32,
    dock_scroll: f32,
    goals: [f32; 2],
    springs: [Option<Spring>; 2],
    /// How far each panel has slid in: the library, the dock and the shortcuts sheet (0 closed, 1 open).
    panels: [Option<Spring>; 3],
    /// How tall each list's content was and how tall its window, as last drawn, for the scroll limits.
    library_extent: [f32; 2],
    dock_extent: [f32; 2],
    /// Where the library and the dock are, as last drawn.
    library_rect: Option<Rect>,
    dock_rect: Option<Rect>,
    /// The scroll bars (track and thumb) of the library and the dock, as last drawn, and where each is held.
    bars: [Option<(Rect, Rect)>; 2],
    grabs: [Option<f32>; 2],
    /// The rectangles that take a press, as last drawn.
    blocks: Vec<Rect>,
    /// How much of the chrome is on the screen: 1 at rest, 0 once it has slid away.
    fade: Option<Spring>,
    /// A press began outside the chrome and is still held, which sends the chrome away.
    hold_outside: bool,
    /// What the library's search field holds.
    query: String,
    /// A card pressed in the library, which dragged out onto the screen puts its HUD where it is let go.
    place: Option<PlaceDrag>,
    /// The context menu: where it was opened, and where it was drawn.
    menu: Option<Vec2>,
    menu_rect: Option<Rect>,
    /// The guided tour's step while it shows, and whether it has started for this opening of the editor.
    tour: Option<usize>,
    tour_started: bool,
}

/// A card of the library held down: which HUD it adds, where the pointer went down and whether it has moved far enough to be a drag.
struct PlaceDrag {
    id: String,
    start: Vec2,
    active: bool,
}

/// What the game tells the editor each frame about the HUD being edited, in the scene's units where it is a place.
#[derive(Clone, Copy, Debug, Default)]
pub struct EditorContext {
    /// The picked HUD's frame, when it is on the screen.
    pub selection: Option<Rect>,
    /// Its visible size in GUI pixels, for the badge.
    pub size: [f32; 2],
    pub locked: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    /// There is a copied look to paste.
    pub can_paste: bool,
    /// No HUD is picked: a press on nothing let go of it.
    pub deselected: bool,
    /// How many HUDs are picked, and whether the one the editor is on is in a group.
    pub picked: usize,
    pub grouped: bool,
    /// A marquee being drawn, in the scene's units.
    pub marquee: Option<Rect>,
    /// What can be seen of the picked HUD and of the one under the pointer, for measuring the gap between them.
    pub selected_box: Option<Rect>,
    pub hover_box: Option<Rect>,
    /// How many of the scene's units are one GUI pixel.
    pub unit: f32,
}

impl EditorState {
    /// The dock's list starts again from the top, with no easing.
    fn reset_dock(&mut self) {
        self.dock_scroll = 0.0;
        self.goals[1] = 0.0;
        self.springs[1] = None;
    }
}

impl GuiMenu {
    /// Whether `point` (in the scene's units) is on part of the editor's chrome, so a press there is the chrome's.
    #[must_use]
    pub fn editor_takes(&self, point: Vec2) -> bool {
        self.editor.fade.as_ref().is_some_and(|fade| fade.value() > SHOWN) && self.editor.blocks.iter().any(|block| block.contains(point))
    }

    /// The editor was left: its panels close, so the next time it opens it is just the toolbar.
    pub fn editor_closed(&mut self) {
        self.editor = EditorState::default();
        self.drag = None;
        self.field = None;
    }

    /// Runs one frame of the editor's chrome. `dragging` is whether a HUD is held; that, or the pointer held anywhere
    /// outside the chrome, slides the chrome off the screen.
    pub fn editor_frame(&mut self, settings: &Settings, state: &PanelState, dragging: bool, input: Input) -> (Scene, Vec<Action>) {
        self.editor_frame_with(settings, state, dragging, &EditorContext::default(), input)
    }

    /// `editor_frame` with what the game knows about the HUD being edited.
    pub fn editor_frame_with(&mut self, settings: &Settings, state: &PanelState, dragging: bool, ctx: &EditorContext, input: Input) -> (Scene, Vec<Action>) {
        let origin = Vec2::new(0.0, 0.0);
        self.clock += input.dt;
        if input.pressed {
            self.editor.hold_outside = input.pointer.is_some_and(|p| !self.editor.blocks.iter().any(|block| block.contains(p)));
        }
        if !input.held {
            self.editor.hold_outside = false;
        }
        let placing = self.editor.place.as_ref().is_some_and(|drag| drag.active);
        let away = dragging || self.editor.hold_outside || placing;
        if !settings.editor_tour_seen && !self.editor.tour_started {
            self.editor.tour = Some(0);
            self.editor.tour_started = true;
        }
        // A press outside the context menu closes it; a right press outside the chrome opens it where the pointer is.
        let over_menu = |editor: &EditorState, p: Vec2| editor.menu_rect.is_some_and(|r| r.contains(p));
        if input.pressed && input.pointer.is_some_and(|p| !over_menu(&self.editor, p)) {
            self.editor.menu = None;
        }
        if input.secondary && self.editor.tour.is_none() {
            self.editor.menu = input.pointer.filter(|p| !self.editor.blocks.iter().any(|b| b.contains(*p)) || over_menu(&self.editor, *p));
        }
        let fade = self.editor.fade.get_or_insert_with(|| Spring::new(0.0, 12.0));
        fade.set_target(if away { 0.0 } else { 1.0 });
        fade.step(input.dt);
        let slide = 1.0 - fade.value().clamp(0.0, 1.0);
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
        self.editor_scroll(&input);
        let mut actions = Vec::new();
        self.pre_input(&input, origin, &mut actions);

        let light = settings.menu_light;
        let p = if light { Palette::light() } else { Palette::dark() };
        let mut d = Painter { scene: Scene::new(), origin, p, hover: self.hover, lit_amounts: self.lit_amounts(), zoom: 1.0, light };
        self.acts.clear();
        self.editor.blocks.clear();
        self.editor.library_rect = None;
        self.editor.dock_rect = None;
        self.editor.bars = [None, None];
        let [width, height] = input.viewport;
        // The panels slide in from their own sides as they open and out as they close.
        let flags = [self.editor.library, self.editor.dock, self.editor.sheet];
        for (spring, open) in self.editor.panels.iter_mut().zip(flags) {
            let spring = spring.get_or_insert_with(|| Spring::new(0.0, 14.0));
            spring.set_target(if open { 1.0 } else { 0.0 });
            spring.step(input.dt);
        }
        let open = self.editor.panels.map(|spring| spring.map_or(0.0, |s| s.value().clamp(0.0, 1.0)));
        let shown = 1.0 - slide;
        if settings.show_grid {
            self.draw_grid(&mut d, [width, height], ctx.unit);
        }
        if input.alt {
            self.draw_measure(&mut d, ctx);
        }
        if let Some(marquee) = ctx.marquee {
            let accent = d.p.accent;
            d.rect(marquee, [accent[0], accent[1], accent[2], 0.15], Radii::all(px(2.0)));
            d.ring(marquee, Radii::all(px(2.0)), 1.0, [accent[0], accent[1], accent[2], 0.9]);
        }
        // Each part slides off towards its own edge: the toolbar up, the library left, the dock right, the pill down.
        d.origin = Vec2::new(0.0, -slide * (px(BAR_TOP + BAR_HEIGHT) + px(8.0)));
        let top = self.draw_toolbar(&mut d, settings, ctx, width);
        self.draw_action_bar(&mut d, settings, state, ctx, top, [width, height]);
        if open[0] > 0.003 {
            d.origin = Vec2::new(-(1.0 - shown * open[0]) * (px(DOCK_EDGE + LIBRARY_WIDTH) + px(8.0)), 0.0);
            self.draw_library(&mut d, settings, state, top, height, open[0] > SHOWN);
        }
        if open[1] > 0.003 {
            d.origin = Vec2::new((1.0 - shown * open[1]) * (px(DOCK_EDGE + DOCK_WIDTH) + px(8.0)), 0.0);
            self.draw_dock(&mut d, settings, state, ctx, top, [width, height], input.dt, open[1] > SHOWN);
        }
        // The sheet comes up from behind the pill, and goes back behind it.
        if open[2] > 0.003 {
            d.origin = Vec2::new(0.0, (1.0 - shown * open[2]) * (px(SHEET_HEAD + SHORTCUTS.len() as f32 * SHEET_ROW + SHEET_TAIL) + px(PILL_BOTTOM + PILL_SIZE[1] + 20.0)));
            self.draw_sheet(&mut d, [width, height], open[2] > SHOWN);
        }
        d.origin = Vec2::new(0.0, slide * (px(PILL_BOTTOM + PILL_SIZE[1]) + px(12.0)));
        self.draw_pill(&mut d, [width, height]);
        d.origin = origin;
        self.draw_ghost(&mut d, input.pointer);
        self.editor.menu_rect = None;
        if let Some(at) = self.editor.menu {
            self.draw_menu(&mut d, settings, state, ctx, at, [width, height]);
        }
        if let Some(step) = self.editor.tour {
            self.draw_tour(&mut d, step, [width, height]);
        }
        let scene = d.scene;
        self.post_input(&scene, &input, origin, &mut actions);
        self.editor_place(&input, &mut actions);
        if self.editor.menu.is_some() && actions.iter().any(|a| matches!(a, Action::Event(_) | Action::EventValue(..))) {
            self.editor.menu = None;
        }
        (scene, self.editor_actions(actions, settings))
    }

    /// The wheel moves the goal of whichever list is under the pointer, a held scroll bar sets it outright, and each list
    /// eases towards its goal as the main menu's does.
    fn editor_scroll(&mut self, input: &Input) {
        let editor = &mut self.editor;
        let max = [(editor.library_extent[0] - editor.library_extent[1]).max(0.0), (editor.dock_extent[0] - editor.dock_extent[1]).max(0.0)];
        for list in 0..2 {
            editor.goals[list] = editor.goals[list].clamp(0.0, max[list]);
            if let Some(scroll) = drag_bar(&mut editor.grabs[list], editor.bars[list], input, max[list]) {
                editor.goals[list] = scroll;
                // A dragged bar is the pointer's own motion: the list is where it asks, with no easing behind.
                editor.springs[list] = None;
                if list == 0 { editor.library_scroll = scroll } else { editor.dock_scroll = scroll }
            }
        }
        if let Some(pointer) = input.pointer {
            let step = input.wheel * px(60.0);
            let over = [editor.library_rect, editor.dock_rect].map(|rect| rect.is_some_and(|r| r.contains(pointer)));
            let list = if over[0] { Some(0) } else if over[1] { Some(1) } else { None };
            if let Some(list) = list {
                editor.goals[list] = (editor.goals[list] - step).clamp(0.0, max[list]);
            }
        }
        for list in 0..2 {
            let shown = if list == 0 { editor.library_scroll } else { editor.dock_scroll };
            let spring = editor.springs[list].get_or_insert_with(|| Spring::new(shown, 22.0));
            spring.set_target(editor.goals[list]);
            spring.step(input.dt);
            let value = spring.value().clamp(0.0, max[list]);
            if list == 0 { editor.library_scroll = value } else { editor.dock_scroll = value }
        }
    }

    /// Takes the editor's own buttons out of the actions and turns "add this HUD" into the actions that do it.
    fn editor_actions(&mut self, actions: Vec<Action>, settings: &Settings) -> Vec<Action> {
        let mut out = Vec::with_capacity(actions.len());
        for action in actions {
            match &action {
                Action::Event(id) if id == "editor_library" => self.editor.library = !self.editor.library,
                Action::Event(id) if id == "editor_dock" => self.editor.dock = !self.editor.dock,
                Action::Event(id) if id == "editor_sheet" => self.editor.sheet = !self.editor.sheet,
                Action::Event(id) if id == "editor_tour" => {
                    self.editor.tour = Some(0);
                    self.editor.menu = None;
                }
                Action::Event(id) if id == "editor_tour_next" => {
                    self.editor.tour = self.editor.tour.map(|step| step + 1).filter(|step| *step < TOUR.len());
                    if self.editor.tour.is_none() {
                        out.push(Action::Event("editor_tour_done".to_owned()));
                    }
                }
                Action::Event(id) if id == "editor_tour_back" => self.editor.tour = self.editor.tour.map(|step| step.saturating_sub(1)),
                Action::Event(id) if id == "editor_tour_skip" => {
                    self.editor.tour = None;
                    out.push(Action::Event("editor_tour_done".to_owned()));
                }
                // The library's search field keeps what is typed in it.
                Action::Text { id, text, .. } if id == "editor_search" => {
                    self.editor.query = text.clone();
                    self.editor.library_scroll = 0.0;
                    self.editor.goals[0] = 0.0;
                }
                // A card let go on the screen: its HUD is added and picked, and the game then puts it where the pointer is.
                Action::Event(id) if id.starts_with("editor_drop_") => {
                    let mod_id = &id["editor_drop_".len()..];
                    out.push(Action::EventValue(mod_id.to_owned(), 1.0));
                    if let Some(element) = Element::ALL.iter().find(|e| e.toggle_id() == mod_id) {
                        out.push(Action::Event(format!("editor_pick_{}", element.index())));
                        out.push(Action::Event(format!("editor_place_{}", element.index())));
                    }
                }
                Action::Event(id) if id.starts_with("editor_tab_") => {
                    self.editor.dock_tab = match &id["editor_tab_".len()..] {
                        "style" => 1,
                        "settings" => 2,
                        _ => 0,
                    };
                    self.editor.reset_dock();
                }
                // The dock's search field keeps what is typed in it.
                Action::Text { id, text, .. } if id == "editor_dock_search" => {
                    self.editor.dock_query = text.clone();
                    self.editor.reset_dock();
                }
                Action::Event(id) if id.starts_with("editor_add_") => {
                    // The HUD is switched on and picked, so the dock shows it.
                    let mod_id = &id["editor_add_".len()..];
                    out.push(Action::EventValue(mod_id.to_owned(), 1.0));
                    if let Some(element) = Element::ALL.iter().find(|e| e.toggle_id() == mod_id) {
                        out.push(Action::Event(format!("editor_pick_{}", element.index())));
                    }
                    self.editor.dock = true;
                }
                _ => out.push(action),
            }
        }
        let _ = settings;
        out
    }

    /// A toolbar button: an icon and a label (none for an icon alone), lit while its panel is open and dimmed when it
    /// cannot be used. Returns its width.
    fn tool_button(&mut self, d: &mut Painter, at: [f32; 2], button: &Tool) -> f32 {
        let p = d.p;
        let width = Self::tool_width(button);
        let rect = Rect::new(at[0], at[1], width, px(BUTTON_HEIGHT));
        let hit = if button.enabled {
            let n = self.acts.len();
            self.acts.push(Act::Press(button.id.to_owned()));
            Some(Hit::Act(n))
        } else {
            None
        };
        let hot = |d: &Painter, base: Rgba, over: Rgba| hit.map_or(base, |hit| d.hot(hit, base, over));
        let fill = if button.primary {
            hot(d, p.accent, lift(p.accent))
        } else if button.lit {
            hot(d, p.dim, lift(p.dim))
        } else {
            hot(d, clear(p.dim), p.dim)
        };
        d.rect(rect, fill, Radii::all(px(RADII.xs)));
        let ink = if button.primary {
            p.on_accent
        } else if button.enabled {
            p.text
        } else {
            p.muted
        };
        if button.label.is_empty() {
            d.glyph(rect, button.art, px(16.0), ink);
        } else {
            d.glyph(Rect::new(rect.x + px(14.0), rect.y, px(16.0), rect.h), button.art, px(16.0), ink);
            d.line(Rect::new(rect.x + px(14.0) + px(22.0), rect.y, width, rect.h), button.label, px(text::SMALL.size), ink, Align::Start);
        }
        if let Some(hit) = hit {
            d.region(hit, rect);
        }
        width
    }

    /// How wide a toolbar button is.
    fn tool_width(button: &Tool) -> f32 {
        if button.label.is_empty() {
            px(BUTTON_HEIGHT)
        } else {
            px(14.0) * 2.0 + px(16.0) + px(6.0) + button.label.chars().count() as f32 * px(text::SMALL.size) * 0.62
        }
    }

    /// The bar across the top: the title, the switches for the lines, the grid and the panels, undo and redo, Reset and Done.
    /// Returns where its bottom edge is.
    fn draw_toolbar(&mut self, d: &mut Painter, settings: &Settings, ctx: &EditorContext, viewport_width: f32) -> f32 {
        let p = d.p;
        let tool = |label: &'static str, art: Art, id: &'static str, lit: bool, primary: bool, enabled: bool| Tool { label, art, id, lit, primary, enabled };
        let items = [
            tool("Lines", Art::AlignCenter, "move_guides", settings.show_guides, false, true),
            tool("Grid", Art::DotsGrid, "move_grid", settings.show_grid, false, true),
            tool("HUDs", Art::Layers, "editor_library", self.editor.library, false, true),
            tool("Settings", Art::Cog, "editor_dock", self.editor.dock, false, true),
            tool("", Art::Undo, "editor_undo", false, false, ctx.can_undo),
            tool("", Art::Redo, "editor_redo", false, false, ctx.can_redo),
            tool("Reset", Art::Refresh, "move_reset", false, false, true),
            tool("Done", Art::Tick, "move_done", false, true, true),
        ];
        let title = "Edit HUD";
        let title_w = title.chars().count() as f32 * px(text::HEADING.size) * 0.62 + px(space::XL);
        let gap = px(space::XS);
        let widths: Vec<f32> = items.iter().map(Self::tool_width).collect();
        let total = px(space::L) * 2.0 + title_w + widths.iter().sum::<f32>() + gap * (items.len() as f32 - 1.0);
        let bar = Rect::new((viewport_width - total) / 2.0, px(BAR_TOP), total, px(BAR_HEIGHT));
        d.rect(bar, p.panel, Radii::all(px(RADII.md)));
        d.ring(bar, Radii::all(px(RADII.md)), 0.3, d.veil(0.14));
        self.editor.blocks.push(bar);
        d.line(Rect::new(bar.x + px(space::L) + px(space::XS), bar.y, title_w, bar.h), title, px(text::HEADING.size), p.highlight, Align::Start);
        let mut x = bar.x + px(space::L) + title_w;
        let y = bar.y + (bar.h - px(BUTTON_HEIGHT)) / 2.0;
        for button in &items {
            x += self.tool_button(d, [x, y], button) + gap;
        }
        bar.bottom()
    }

    /// The bar over the picked HUD: lock, settings, remove and its size.
    fn draw_action_bar(&mut self, d: &mut Painter, settings: &Settings, state: &PanelState, ctx: &EditorContext, toolbar_bottom: f32, viewport: [f32; 2]) {
        let Some(frame) = ctx.selection.filter(|_| settings.is_on(state.element()) && !ctx.deselected) else { return };
        let p = d.p;
        let badge = format!("{} \u{d7} {}", ctx.size[0].round() as i32, ctx.size[1].round() as i32);
        let badge_w = badge.chars().count() as f32 * px(text::CAPTION.size) * 0.62 + px(space::L);
        let (button, pad) = (px(26.0), px(space::XS));
        let width = pad * 2.0 + button * 3.0 + px(space::S) + badge_w;
        let height = button + pad * 2.0;
        let x = (frame.centre().x - width / 2.0).clamp(px(8.0), (viewport[0] - width - px(8.0)).max(px(8.0)));
        // Above the frame, or below it when the toolbar is in the way.
        let above = frame.y - height - px(8.0);
        let y = if above >= toolbar_bottom + px(4.0) { above } else { frame.bottom() + px(8.0) };
        let bar = Rect::new(x, y, width, height);
        d.rect(bar, p.panel, Radii::all(height / 2.0));
        d.ring(bar, Radii::all(height / 2.0), 0.3, d.veil(0.14));
        self.editor.blocks.push(bar);
        let lock = if ctx.locked { Art::Lock } else { Art::Unlock };
        for (i, (art, id, lit)) in [(lock, "editor_lock", ctx.locked), (Art::Cog, "editor_gear", false), (Art::Trash, "editor_remove", false)].into_iter().enumerate() {
            let r = Rect::new(bar.x + pad + i as f32 * button, bar.y + pad, button, button);
            let n = self.acts.len();
            self.acts.push(Act::Press(id.to_owned()));
            let hit = Hit::Act(n);
            d.rect(r, d.hot(hit, clear(p.dim), p.dim), Radii::all(button / 2.0));
            d.glyph(r, art, px(15.0), if lit { p.accent } else if id == "editor_remove" { p.danger } else { p.text });
            d.region(hit, r);
        }
        let badge_rect = Rect::new(bar.x + pad + button * 3.0 + px(space::S), bar.y + pad, badge_w, button);
        d.line(badge_rect, &badge, px(text::CAPTION.size), p.muted, Align::Centre);
    }

    /// Thin lines every eight GUI pixels over the whole screen, which the HUDs snap to while the grid shows. They are
    /// counted out from the middle of the screen, so two of them are the centre lines.
    fn draw_grid(&mut self, d: &mut Painter, viewport: [f32; 2], unit: f32) {
        let step = GRID_STEP * unit;
        if step < px(3.0) {
            return;
        }
        let mut contours: Vec<Vec<[f32; 2]>> = Vec::new();
        let (cx, cy) = (viewport[0] / 2.0, viewport[1] / 2.0);
        let mut n = 0.0_f32;
        while cx - n * step >= 0.0 || cx + n * step <= viewport[0] {
            for (i, x) in [cx - n * step, cx + n * step].into_iter().enumerate() {
                if (0.0..=viewport[0]).contains(&x) && (i == 0 || n > 0.0) {
                    contours.push(vec![[x, 0.0], [x, viewport[1]]]);
                }
            }
            n += 1.0;
        }
        let mut n = 0.0_f32;
        while cy - n * step >= 0.0 || cy + n * step <= viewport[1] {
            for (i, y) in [cy - n * step, cy + n * step].into_iter().enumerate() {
                if (0.0..=viewport[1]).contains(&y) && (i == 0 || n > 0.0) {
                    contours.push(vec![[0.0, y], [viewport[0], y]]);
                }
            }
            n += 1.0;
        }
        let rect = d.at(Rect::new(0.0, 0.0, viewport[0], viewport[1]));
        d.scene.push(Primitive::Line { rect, view: viewport, contours, color: color(d.veil(0.10)), width: 0.6 });
    }

    /// The context menu opened by a right press: what can be done to the picked HUD, then the editor's own switches.
    fn draw_menu(&mut self, d: &mut Painter, settings: &Settings, state: &PanelState, ctx: &EditorContext, at: Vec2, viewport: [f32; 2]) {
        let p = d.p;
        let on_hud = ctx.selection.is_some() && settings.is_on(state.element()) && !ctx.deselected;
        // Each row: its label, what it sends, and whether it can be used now. A separator has no label.
        let mut rows: Vec<(String, &'static str, bool)> = Vec::new();
        if on_hud {
            rows.push(("Settings".to_owned(), "editor_gear", true));
            rows.push((if ctx.locked { "Unlock" } else { "Lock" }.to_owned(), "editor_lock", true));
            rows.push(("Copy look".to_owned(), "editor_copy_look", true));
            rows.push(("Paste look".to_owned(), "editor_paste_look", ctx.can_paste && !ctx.locked));
            rows.push(("Reset position".to_owned(), "move_reset", !ctx.locked));
            rows.push(("Remove".to_owned(), "editor_remove", true));
            rows.push((String::new(), "", false));
            if ctx.picked > 1 {
                rows.push((format!("Group {} HUDs", ctx.picked), "editor_group", true));
            }
            if ctx.grouped {
                rows.push(("Ungroup".to_owned(), "editor_ungroup", true));
            }
            if ctx.picked > 1 || ctx.grouped {
                rows.push((String::new(), "", false));
            }
        }
        rows.push(("Undo".to_owned(), "editor_undo", ctx.can_undo));
        rows.push(("Redo".to_owned(), "editor_redo", ctx.can_redo));
        rows.push((String::new(), "", false));
        rows.push((if settings.show_grid { "Hide grid" } else { "Show grid" }.to_owned(), "move_grid", true));
        rows.push((if settings.show_guides { "Hide lines" } else { "Show lines" }.to_owned(), "move_guides", true));
        rows.push(("Editor tour".to_owned(), "editor_tour", true));
        let (row_h, sep_h, width, pad) = (px(28.0), px(9.0), px(190.0), px(space::XS));
        let height = pad * 2.0 + rows.iter().map(|(label, ..)| if label.is_empty() { sep_h } else { row_h }).sum::<f32>();
        let menu = Rect::new(at.x.min(viewport[0] - width - px(6.0)).max(px(6.0)), at.y.min(viewport[1] - height - px(6.0)).max(px(6.0)), width, height);
        d.rect(menu, p.panel, Radii::all(px(RADII.sm)));
        d.ring(menu, Radii::all(px(RADII.sm)), 0.3, d.veil(0.16));
        self.editor.menu_rect = Some(menu);
        self.editor.blocks.push(menu);
        let mut y = menu.y + pad;
        for (label, id, enabled) in rows {
            if label.is_empty() {
                d.rect(Rect::new(menu.x + px(space::M), y + sep_h / 2.0, menu.w - px(space::M) * 2.0, 0.35), d.veil(0.16), Radii::default());
                y += sep_h;
                continue;
            }
            let row = Rect::new(menu.x + pad, y, menu.w - pad * 2.0, row_h);
            y += row_h;
            if !enabled {
                d.line(Rect::new(row.x + px(space::M), row.y, row.w, row.h), &label, px(text::SMALL.size), p.muted, Align::Start);
                continue;
            }
            let n = self.acts.len();
            self.acts.push(Act::Press(id.to_owned()));
            let hit = Hit::Act(n);
            d.rect(row, d.hot(hit, clear(p.dim), p.dim), Radii::all(px(RADII.xs)));
            d.line(Rect::new(row.x + px(space::M), row.y, row.w, row.h), &label, px(text::SMALL.size), if id == "editor_remove" { p.danger } else { p.text }, Align::Start);
            d.region(hit, row);
        }
    }

    /// The guided tour: the screen dimmed under a card with the step's title and text, its dots and its buttons.
    fn draw_tour(&mut self, d: &mut Painter, step: usize, viewport: [f32; 2]) {
        let p = d.p;
        let full = Rect::new(0.0, 0.0, viewport[0], viewport[1]);
        d.rect(full, [0.0, 0.0, 0.0, 0.45], Radii::default());
        self.editor.blocks.push(full);
        let (title, body) = TOUR[step.min(TOUR.len() - 1)];
        let (width, height) = (px(420.0), px(190.0));
        let card = Rect::new((viewport[0] - width) / 2.0, (viewport[1] - height) / 2.0 + px(60.0), width, height);
        d.rect(card, p.panel, Radii::all(px(RADII.md)));
        d.ring(card, Radii::all(px(RADII.md)), 0.3, d.veil(0.16));
        d.line(Rect::new(card.x + px(space::XL), card.y + px(space::S), card.w - px(space::XL) * 2.0, px(32.0)), title, px(text::HEADING.size), p.highlight, Align::Start);
        // The text, broken into lines of about the card's width.
        let per_line = ((card.w - px(space::XL) * 2.0) / (px(text::SMALL.size) * 0.55)) as usize;
        let mut lines: Vec<String> = Vec::new();
        for word in body.split_whitespace() {
            match lines.last_mut() {
                Some(line) if line.chars().count() + 1 + word.chars().count() <= per_line => {
                    line.push(' ');
                    line.push_str(word);
                }
                _ => lines.push(word.to_owned()),
            }
        }
        for (i, line) in lines.iter().enumerate() {
            d.line(Rect::new(card.x + px(space::XL), card.y + px(44.0) + i as f32 * px(20.0), card.w - px(space::XL) * 2.0, px(20.0)), line, px(text::SMALL.size), p.muted, Align::Start);
        }
        // The dots, one for each step.
        for i in 0..TOUR.len() {
            let dot = Rect::new(card.x + px(space::XL) + i as f32 * px(14.0), card.bottom() - px(52.0), px(7.0), px(7.0));
            d.rect(dot, if i == step { p.accent } else { p.dim }, Radii::all(px(3.5)));
        }
        let last = step + 1 >= TOUR.len();
        let buttons: [(&str, &'static str, bool, bool); 3] = [("Skip tour", "editor_tour_skip", false, !last), ("Back", "editor_tour_back", false, step > 0), (if last { "Done" } else { "Next" }, "editor_tour_next", true, true)];
        let mut right = card.right() - px(space::XL);
        for (label, id, primary, shown) in buttons.into_iter().rev() {
            if !shown {
                continue;
            }
            let w = label.chars().count() as f32 * px(text::SMALL.size) * 0.62 + px(28.0);
            let r = Rect::new(right - w, card.bottom() - px(space::L) - px(30.0), w, px(30.0));
            right = r.x - px(space::S);
            let n = self.acts.len();
            self.acts.push(Act::Press(id.to_owned()));
            let hit = Hit::Act(n);
            d.rect(r, if primary { d.hot(hit, p.accent, lift(p.accent)) } else { d.hot(hit, p.dim, lift(p.dim)) }, Radii::all(px(RADII.xs)));
            d.line(r, label, px(text::SMALL.size), if primary { p.on_accent } else { p.text }, Align::Centre);
            d.region(hit, r);
        }
    }

    /// With Alt held: the gap between the picked HUD and the one under the pointer, as a line with the distance in GUI
    /// pixels on it, across the space between them on each axis along which they do not overlap.
    fn draw_measure(&mut self, d: &mut Painter, ctx: &EditorContext) {
        let (Some(a), Some(b)) = (ctx.selected_box, ctx.hover_box) else { return };
        let p = d.p;
        let unit = if ctx.unit > 0.0 { ctx.unit } else { 1.0 };
        let mut gaps: Vec<([f32; 2], [f32; 2])> = Vec::new();
        // Side by side: the gap runs across the middle of what the two share vertically, or of the picked one.
        let (top, bottom) = (a.y.max(b.y), a.bottom().min(b.bottom()));
        let y = if top < bottom { (top + bottom) / 2.0 } else { a.centre().y };
        if a.right() <= b.x {
            gaps.push(([a.right(), y], [b.x, y]));
        } else if b.right() <= a.x {
            gaps.push(([b.right(), y], [a.x, y]));
        }
        // One above the other likewise.
        let (left, right) = (a.x.max(b.x), a.right().min(b.right()));
        let x = if left < right { (left + right) / 2.0 } else { a.centre().x };
        if a.bottom() <= b.y {
            gaps.push(([x, a.bottom()], [x, b.y]));
        } else if b.bottom() <= a.y {
            gaps.push(([x, b.bottom()], [x, a.y]));
        }
        for (from, to) in gaps {
            let line = [from, to];
            d.scene.push(Primitive::Line { rect: d.at(Rect::new(0.0, 0.0, ctx_extent(&line)[0], ctx_extent(&line)[1])), view: ctx_extent(&line), contours: vec![vec![from, to]], color: color([p.accent[0], p.accent[1], p.accent[2], 0.9]), width: 1.5 });
            for end in line {
                d.rect(Rect::new(end[0] - px(2.0), end[1] - px(2.0), px(4.0), px(4.0)), p.accent, Radii::all(px(2.0)));
            }
            let distance = (to[0] - from[0]).abs().max((to[1] - from[1]).abs()) / unit;
            let label = format!("{}", distance.round() as i32);
            let width = label.chars().count() as f32 * px(text::CAPTION.size) * 0.62 + px(space::L);
            let middle = [(from[0] + to[0]) / 2.0, (from[1] + to[1]) / 2.0];
            let badge = Rect::new(middle[0] - width / 2.0, middle[1] - px(10.0), width, px(20.0));
            d.rect(badge, p.panel, Radii::all(px(RADII.xs)));
            d.ring(badge, Radii::all(px(RADII.xs)), 0.5, p.accent);
            d.line(badge, &label, px(text::CAPTION.size), p.text, Align::Centre);
        }
    }

    /// The card being dragged out of the library, following the pointer.
    fn draw_ghost(&mut self, d: &mut Painter, pointer: Option<Vec2>) {
        let (Some(drag), Some(at)) = (self.editor.place.as_ref().filter(|drag| drag.active), pointer) else { return };
        let p = d.p;
        let card = Rect::new(at.x - px(60.0), at.y - px(32.0), px(120.0), px(64.0));
        d.rect(card, p.panel, Radii::all(px(RADII.sm)));
        d.ring(card, Radii::all(px(RADII.sm)), 1.0, p.accent);
        d.scene.push(Primitive::PushClip { rect: d.at(card), radii: Radii::default() });
        previews::paint(d, &drag.id, card, [0.92, 0.93, 1.0, 1.0]);
        d.scene.push(Primitive::PopClip);
    }

    /// A card pressed in the library may be dragged: once the pointer is far enough from where it went down the card is
    /// a drag, and letting go puts its HUD on the screen where the pointer is.
    fn editor_place(&mut self, input: &Input, actions: &mut Vec<Action>) {
        if input.pressed {
            self.editor.place = match (self.pressed, input.pointer) {
                (Some(Hit::Act(n)), Some(start)) => match self.acts.get(n) {
                    Some(Act::Press(id)) => id.strip_prefix("editor_add_").map(|mod_id| PlaceDrag { id: mod_id.to_owned(), start, active: false }),
                    _ => None,
                },
                _ => None,
            };
        }
        let Some(drag) = self.editor.place.as_mut() else { return };
        if input.held {
            if input.pointer.is_some_and(|p| (p.x - drag.start.x).hypot(p.y - drag.start.y) > px(12.0)) {
                drag.active = true;
            }
            return;
        }
        if let Some(drag) = self.editor.place.take().filter(|drag| drag.active) {
            actions.push(Action::Event(format!("editor_drop_{}", drag.id)));
        }
    }

    /// The pill at the bottom, which opens the sheet of shortcuts.
    fn draw_pill(&mut self, d: &mut Painter, viewport: [f32; 2]) {
        let p = d.p;
        let pill = Rect::new((viewport[0] - px(PILL_SIZE[0])) / 2.0, viewport[1] - px(PILL_BOTTOM) - px(PILL_SIZE[1]), px(PILL_SIZE[0]), px(PILL_SIZE[1]));
        let n = self.acts.len();
        self.acts.push(Act::Press("editor_sheet".to_owned()));
        let hit = Hit::Act(n);
        d.rect(pill, d.hot(hit, p.panel, lift(p.panel)), Radii::all(pill.h / 2.0));
        d.ring(pill, Radii::all(pill.h / 2.0), 0.3, d.veil(0.14));
        let label_w = "Keyboard shortcuts".len() as f32 * px(text::SMALL.size) * 0.62;
        let left = pill.x + (pill.w - label_w - px(24.0)) / 2.0;
        d.glyph(Rect::new(left, pill.y, px(18.0), pill.h), Art::Keyboard, px(16.0), p.muted);
        d.line(Rect::new(left + px(24.0), pill.y, label_w + px(10.0), pill.h), "Keyboard shortcuts", px(text::SMALL.size), p.text, Align::Start);
        d.region(hit, pill);
        self.editor.blocks.push(pill);
    }

    /// The sheet of shortcuts: a title over a rule, then a row for each with its key cap and what it does.
    fn draw_sheet(&mut self, d: &mut Painter, viewport: [f32; 2], live: bool) {
        let p = d.p;
        let pill = Rect::new((viewport[0] - px(PILL_SIZE[0])) / 2.0, viewport[1] - px(PILL_BOTTOM) - px(PILL_SIZE[1]), px(PILL_SIZE[0]), px(PILL_SIZE[1]));
        let (row_h, head_h, pad) = (px(SHEET_ROW), px(SHEET_HEAD), px(space::XL));
        let sheet_w = px(SHEET_WIDTH);
        let sheet_h = head_h + SHORTCUTS.len() as f32 * row_h + px(SHEET_TAIL);
        let sheet = Rect::new((viewport[0] - sheet_w) / 2.0, pill.y - px(space::M) - sheet_h, sheet_w, sheet_h);
        d.rect(sheet, p.panel, Radii::all(px(RADII.md)));
        d.ring(sheet, Radii::all(px(RADII.md)), 0.3, d.veil(0.14));
        if live {
            self.editor.blocks.push(sheet);
        }
        d.line(Rect::new(sheet.x + pad, sheet.y, sheet.w - pad * 2.0, head_h), "Keyboard shortcuts", px(text::HEADING.size), p.highlight, Align::Start);
        d.rect(Rect::new(sheet.x + pad, sheet.y + head_h - px(2.0), sheet.w - pad * 2.0, 0.35), d.veil(0.14), Radii::default());
        let cap_h = px(22.0);
        let text_x = sheet.x + pad + px(112.0);
        for (i, (keys, what)) in SHORTCUTS.iter().enumerate() {
            let row = Rect::new(sheet.x + pad, sheet.y + head_h + i as f32 * row_h, sheet.w - pad * 2.0, row_h);
            let cap_w = keys.chars().count() as f32 * px(text::CAPTION.size) * 0.62 + px(20.0);
            let cap = Rect::new(row.x, row.y + (row_h - cap_h) / 2.0, cap_w, cap_h);
            d.rect(cap, p.raised, Radii::all(px(RADII.xs)));
            d.ring(cap, Radii::all(px(RADII.xs)), 0.3, d.veil(0.2));
            d.line(cap, keys, px(text::CAPTION.size), p.text, Align::Centre);
            d.line(Rect::new(text_x, row.y, sheet.right() - pad - text_x, row_h), what, px(text::SMALL.size), p.muted, Align::Start);
        }
    }

    /// The panel's shell: its glass, its title and the hint under it. Returns the rectangle left for its content.
    fn dock_shell(&mut self, d: &mut Painter, rect: Rect, title: &str, hint: &str, live: bool) -> Rect {
        let p = d.p;
        d.rect(rect, p.panel, Radii::all(px(RADII.md)));
        d.ring(rect, Radii::all(px(RADII.md)), 0.3, d.veil(0.14));
        if live {
            self.editor.blocks.push(rect);
        }
        d.line(Rect::new(rect.x + px(space::XL), rect.y + px(space::S), rect.w, px(28.0)), title, px(text::HEADING.size), p.highlight, Align::Start);
        d.line(Rect::new(rect.x + px(space::XL), rect.y + px(32.0), rect.w - px(space::XL * 2.0), px(18.0)), hint, px(text::CAPTION.size), p.muted, Align::Start);
        Rect::new(rect.x + px(space::L), rect.y + px(58.0), rect.w - px(space::L) * 2.0 - px(10.0), rect.h - px(58.0) - px(space::L))
    }

    /// The library: a search field and a card for every HUD with its preview, to click to put it on the screen or to
    /// drag onto it.
    fn draw_library(&mut self, d: &mut Painter, settings: &Settings, state: &PanelState, top: f32, viewport_height: f32, live: bool) {
        let p = d.p;
        let tall = (viewport_height - top - px(space::M) - px(PILL_BOTTOM + PILL_SIZE[1] + 10.0)).min(px(PANEL_MAX_HEIGHT));
        let rect = Rect::new(px(DOCK_EDGE), top + px(space::M), px(LIBRARY_WIDTH), tall);
        self.editor.library_rect = live.then_some(rect);
        let head = self.dock_shell(d, rect, "HUDs", LIBRARY_HINT, live);
        // The search field, over the cards.
        let search = Rect::new(head.x, head.y, head.w, px(26.0));
        let n = self.acts.len();
        self.acts.push(Act::Field { id: "editor_search".to_owned(), text: self.editor.query.clone() });
        let hit = Hit::Act(n);
        let editing = self.field.as_ref().is_some_and(|(id, _)| id == "editor_search");
        d.rect(search, if editing { p.dim } else { d.hot(hit, p.field, p.dim) }, Radii::all(px(RADII.xs)));
        if editing {
            d.ring(search, Radii::all(px(RADII.xs)), 1.0, p.accent);
        }
        d.glyph(Rect::new(search.x + px(space::S), search.y, px(18.0), search.h), Art::Search, px(14.0), p.muted);
        let typed = self.field.as_ref().filter(|(id, _)| id == "editor_search").map_or(self.editor.query.as_str(), |(_, text)| text.as_str()).to_owned();
        let caret = editing && (self.clock * 2.0) as u32 % 2 == 0;
        let (shown, ink) = if typed.is_empty() && !editing { ("Search HUDs...".to_owned(), p.muted) } else { (format!("{typed}{}", if caret { "|" } else { "" }), p.text) };
        d.line(Rect::new(search.x + px(28.0), search.y, search.w - px(32.0), search.h), &shown, px(text::SMALL.size), ink, Align::Start);
        d.region(hit, search);
        let list = Rect::new(head.x, head.y + px(34.0), head.w, head.h - px(34.0));
        let cards = menu_v2::cards(settings, state);
        let terms: Vec<String> = typed.split_whitespace().map(str::to_lowercase).collect();
        let huds: Vec<&vantage_ui::mods::Card<'_>> = cards
            .iter()
            .filter(|c| c.category == "HUD" && !c.hidden)
            .filter(|c| terms.iter().all(|term| c.name.to_lowercase().contains(term)))
            .collect();
        let gap = px(space::M);
        let card_w = (list.w - gap) / 2.0;
        let card_h = px(CARD_HEIGHT);
        let rows = huds.len().div_ceil(2);
        let content = rows as f32 * (card_h + gap);
        self.editor.library_extent = [content, list.h];
        self.editor.library_scroll = self.editor.library_scroll.clamp(0.0, (content - list.h).max(0.0));
        if huds.is_empty() {
            d.line(Rect::new(list.x, list.y, list.w, px(30.0)), "No HUDs match", px(text::SMALL.size), p.muted, Align::Centre);
        }
        let clip = d.at(list);
        d.scene.push(Primitive::PushClip { rect: clip, radii: Radii::default() });
        for (i, card) in huds.iter().enumerate() {
            let at = Rect::new(list.x + (i % 2) as f32 * (card_w + gap), list.y + (i / 2) as f32 * (card_h + gap) - self.editor.library_scroll, card_w, card_h);
            if at.bottom() < list.y || at.y > list.bottom() {
                continue;
            }
            // Everything of a card is drawn inside its own clip, so whatever lies outside the list is dropped whole
            // (a text cut off by the list's edge, with none of it in view, would fail the panel).
            d.scene.push(Primitive::PushClip { rect: d.at(at), radii: Radii::default() });
            let n = self.acts.len();
            // A HUD already on the screen has nothing to add; its card only shows that it is there.
            self.acts.push(if card.on { Act::Press("editor_none".to_owned()) } else { Act::Press(format!("editor_add_{}", card.id)) });
            let hit = Hit::Act(n);
            d.rect(at, if card.on { p.dim } else { d.hot(hit, p.raised, p.dim) }, Radii::all(px(RADII.sm)));
            let area = Rect::new(at.x, at.y + px(4.0), at.w, at.h - px(30.0));
            d.scene.push(Primitive::PushClip { rect: d.at(Rect::new(at.x + px(4.0), area.y, at.w - px(8.0), area.h)), radii: Radii::default() });
            previews::paint(d, card.id, area, [0.92, 0.93, 1.0, 1.0]);
            d.scene.push(Primitive::PopClip);
            d.line(Rect::new(at.x, at.bottom() - px(26.0), at.w, px(24.0)), card.name, px(text::CAPTION.size), p.text, Align::Centre);
            if card.on {
                d.glyph(Rect::new(at.right() - px(24.0), at.y + px(4.0), px(18.0), px(18.0)), Art::CheckCircle, px(16.0), p.accent);
            }
            d.region(hit, at);
            d.scene.push(Primitive::PopClip);
        }
        d.scene.push(Primitive::PopClip);
        self.editor.bars[0] = d.scrollbar(list, content, self.editor.library_scroll);
    }

    /// The dock: the picked HUD's settings, as the menu's own option rows, on three tabs: Layout (its place and size),
    /// Style (its text, box and border) and Settings (what the HUD itself offers), with a search over the rows.
    #[allow(clippy::too_many_arguments)]
    fn draw_dock(&mut self, d: &mut Painter, settings: &Settings, state: &PanelState, ctx: &EditorContext, top: f32, viewport: [f32; 2], dt: f32, live: bool) {
        let p = d.p;
        let element = state.element();
        let tall = (viewport[1] - top - px(space::M) - px(PILL_BOTTOM + PILL_SIZE[1] + 10.0)).min(px(PANEL_MAX_HEIGHT));
        let rect = Rect::new(viewport[0] - px(DOCK_EDGE) - px(DOCK_WIDTH), top + px(space::M), px(DOCK_WIDTH), tall);
        self.editor.dock_rect = live.then_some(rect);
        let picked = settings.is_on(element) && !ctx.deselected;
        let name = if picked { element.label() } else { "Pick a HUD" };
        let list = self.dock_shell(d, rect, name, DOCK_HINT, live);
        // The three pages as tabs under the title.
        let tab_h = px(26.0);
        let tab_w = (list.w - px(space::XS) * 2.0) / 3.0;
        for (i, (label, id)) in [("Layout", "editor_tab_layout"), ("Style", "editor_tab_style"), ("Settings", "editor_tab_settings")].into_iter().enumerate() {
            let on = usize::from(self.editor.dock_tab) == i;
            let r = Rect::new(list.x + i as f32 * (tab_w + px(space::XS)), list.y, tab_w, tab_h);
            let n = self.acts.len();
            self.acts.push(Act::Press(id.to_owned()));
            let hit = Hit::Act(n);
            d.rect(r, if on { p.accent } else { d.hot(hit, p.raised, p.dim) }, Radii::all(px(RADII.xs)));
            d.line(r, label, px(text::SMALL.size), if on { p.on_accent } else { p.text }, Align::Centre);
            d.region(hit, r);
        }
        if !picked {
            return;
        }
        // The search field under the tabs.
        let search = Rect::new(list.x, list.y + tab_h + px(space::S), list.w, px(26.0));
        let n = self.acts.len();
        self.acts.push(Act::Field { id: "editor_dock_search".to_owned(), text: self.editor.dock_query.clone() });
        let hit = Hit::Act(n);
        let editing = self.field.as_ref().is_some_and(|(id, _)| id == "editor_dock_search");
        d.rect(search, if editing { p.dim } else { d.hot(hit, p.field, p.dim) }, Radii::all(px(RADII.xs)));
        if editing {
            d.ring(search, Radii::all(px(RADII.xs)), 1.0, p.accent);
        }
        d.glyph(Rect::new(search.x + px(space::S), search.y, px(18.0), search.h), Art::Search, px(14.0), p.muted);
        let typed = self.field.as_ref().filter(|(id, _)| id == "editor_dock_search").map_or(self.editor.dock_query.as_str(), |(_, text)| text.as_str()).to_owned();
        let caret = editing && (self.clock * 2.0) as u32 % 2 == 0;
        let (shown, ink) = if typed.is_empty() && !editing { ("Search settings...".to_owned(), p.muted) } else { (format!("{typed}{}", if caret { "|" } else { "" }), p.text) };
        d.line(Rect::new(search.x + px(28.0), search.y, search.w - px(32.0), search.h), &shown, px(text::SMALL.size), ink, Align::Start);
        d.region(hit, search);
        let body = Rect::new(list.x, search.bottom() + px(space::M), list.w, list.bottom() - search.bottom() - px(space::M));
        let terms: Vec<String> = typed.split_whitespace().map(str::to_lowercase).collect();
        let pages: &[usize] = match self.editor.dock_tab {
            0 => &[1],
            1 => &[2],
            _ => &[2, 1],
        };
        let scroll = self.editor.dock_scroll;
        d.scene.push(Primitive::PushClip { rect: d.at(body), radii: Radii::default() });
        let mut y = body.y - scroll;
        self.dock_rows(d, settings, state, pages, body, &mut y, &terms, dt);
        d.scene.push(Primitive::PopClip);
        let content = y + scroll - body.y;
        if content <= px(space::M) * 2.0 {
            d.line(Rect::new(body.x, body.y, body.w, px(30.0)), if terms.is_empty() { "Nothing here for this HUD" } else { "No settings match" }, px(text::SMALL.size), p.muted, Align::Centre);
        }
        self.editor.dock_extent = [content, body.h];
        self.editor.bars[1] = d.scrollbar(body, content, scroll);
        // The open choice's list sits over the whole screen; a press on it is not a press on a HUD.
        menu_v2::with_page(settings, state, pages[0], &mut |page| {
            if let Some(popup) = page.popup {
                self.draw_popup(d, popup);
                if live {
                    self.editor.blocks.push(Rect::new(0.0, 0.0, viewport[0], viewport[1]));
                }
            }
        });
    }

    /// Draws the rows of the dock's pages in turn, `y` running down the list: each page's groups that belong on the open
    /// tab, with their headings, keeping the rows whose name has every word searched for.
    #[allow(clippy::too_many_arguments)]
    fn dock_rows(&mut self, d: &mut Painter, settings: &Settings, state: &PanelState, pages: &[usize], body: Rect, y: &mut f32, terms: &[String], dt: f32) {
        let Some((&tab, rest)) = pages.split_first() else { return };
        let p = d.p;
        let gap = px(space::S);
        let (dock_tab, element) = (self.editor.dock_tab, state.element());
        menu_v2::with_page(settings, state, tab, &mut |page| {
            for group in page.groups.iter().filter(|g| dock_group_shown(dock_tab, tab, g.title, element)) {
                let options: Vec<&vantage_ui::options::Opt<'_>> = group.options.iter().filter(|opt| terms.iter().all(|term| opt.label.to_lowercase().contains(term))).collect();
                if options.is_empty() {
                    continue;
                }
                let heading = Rect::new(body.x, *y, body.w, px(22.0));
                if heading.bottom() >= body.y && heading.y <= body.bottom() {
                    d.scene.push(Primitive::PushClip { rect: d.at(heading), radii: Radii::default() });
                    d.line(Rect::new(heading.x + px(space::S), heading.y, heading.w, heading.h), &group.title.to_uppercase(), px(text::CAPTION.size), p.muted, Align::Start);
                    d.scene.push(Primitive::PopClip);
                }
                *y += heading.h + gap;
                for opt in options {
                    let h = if opt.help.is_some() { px(48.0) } else { px(40.0) };
                    let row = Rect::new(body.x, *y, body.w, h);
                    if row.bottom() >= body.y && row.y <= body.bottom() {
                        // A row is drawn inside its own clip: one cut off by the dock's edge drops whole.
                        d.scene.push(Primitive::PushClip { rect: d.at(row), radii: Radii::default() });
                        self.draw_row(d, opt, row, dt);
                        d.scene.push(Primitive::PopClip);
                    }
                    *y += h + gap;
                }
                *y += px(space::M);
            }
        });
        self.dock_rows(d, settings, state, rest, body, y, terms, dt);
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    /// The settings of a player who has had the tour, so it is not over the chrome.
    fn seen() -> Settings {
        Settings { editor_tour_seen: true, ..Settings::default() }
    }

    fn input(pointer: Option<Vec2>) -> Input {
        Input { pointer, viewport: [1280.0, 720.0], dt: 0.016, ..Input::default() }
    }

    /// Runs frames until the chrome has slid fully into place.
    fn settle(menu: &mut GuiMenu, settings: &Settings, state: &PanelState) {
        for _ in 0..120 {
            menu.editor_frame(settings, state, false, input(None));
        }
    }

    /// Presses and releases the control that sends `id`, returning what the editor sent.
    fn press(menu: &mut GuiMenu, settings: &Settings, state: &PanelState, id: &str) -> Vec<Action> {
        settle(menu, settings, state);
        let n = menu.acts.iter().position(|a| matches!(a, Act::Press(p) if p == id)).unwrap_or_else(|| panic!("no control sends {id}"));
        let region = menu.editor_frame(settings, state, false, input(None)).0.regions.iter().find(|r| Hit::from_id(r.id) == Some(Hit::Act(n))).unwrap().rect;
        let at = Some(Vec2::new(region.x + region.w / 2.0, region.y + region.h / 2.0));
        menu.editor_frame(settings, state, false, Input { pressed: true, held: true, ..input(at) });
        menu.editor_frame(settings, state, false, input(at)).1
    }

    #[test]
    fn done_and_reset_send_the_original_events_and_the_toolbar_takes_the_press() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        assert_eq!(press(&mut menu, &settings, &state, "move_done"), vec![Action::Event("move_done".to_owned())]);
        assert_eq!(press(&mut menu, &settings, &state, "move_reset"), vec![Action::Event("move_reset".to_owned())]);
        // The toolbar is across the top middle; a point far from every panel is the HUD layer's.
        let (scene, _) = menu.editor_frame(&settings, &state, false, input(None));
        let done = scene.regions.iter().map(|r| r.rect).next().expect("a region");
        assert!(menu.editor_takes(done.centre()));
        assert!(!menu.editor_takes(Vec2::new(640.0, 360.0)));
    }

    #[test]
    fn a_hud_card_switches_its_hud_on_and_picks_it_and_the_dock_opens() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        // Open the library, then press the card of a HUD that is off.
        assert!(press(&mut menu, &settings, &state, "editor_library").is_empty());
        assert!(menu.editor.library);
        let cards = menu_v2::cards(&settings, &state);
        let off = cards.iter().find(|c| c.category == "HUD" && !c.on && !c.hidden).expect("a HUD that is off");
        let element = Element::ALL.iter().find(|e| e.toggle_id() == off.id).expect("an element");
        let actions = press(&mut menu, &settings, &state, &format!("editor_add_{}", off.id));
        assert_eq!(actions, vec![Action::EventValue(off.id.to_owned(), 1.0), Action::Event(format!("editor_pick_{}", element.index()))]);
        assert!(menu.editor.dock);
    }

    #[test]
    fn the_chrome_fades_out_while_a_hud_is_held_and_closing_the_editor_forgets_its_panels() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        press(&mut menu, &settings, &state, "editor_dock");
        assert!(menu.editor.dock);
        for _ in 0..200 {
            menu.editor_frame(&settings, &state, true, input(None));
        }
        assert!(menu.editor.fade.as_ref().is_some_and(|fade| fade.value() < 0.05));
        menu.editor_closed();
        assert!(!menu.editor.dock && menu.editor.blocks.is_empty());
    }

    #[test]
    fn the_dock_lists_the_picked_huds_options_on_its_layout_and_style_pages() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        assert!(settings.is_on(state.element()), "the first HUD starts on");
        let without = menu.editor_frame(&settings, &state, false, input(None)).0.primitives.len();
        press(&mut menu, &settings, &state, "editor_dock");
        let layout = menu.editor_frame(&settings, &state, false, input(None)).0.primitives.len();
        assert!(layout > without + 20, "{layout} primitives with the dock, {without} without");
        press(&mut menu, &settings, &state, "editor_tab_style");
        assert_eq!(menu.editor.dock_tab, 1);
        assert!(menu.editor_frame(&settings, &state, false, input(None)).0.primitives.len() > without + 20);
        assert!(menu.editor.dock_rect.is_some_and(|dock| menu.editor_takes(dock.centre())));
    }

    #[test]
    fn holding_the_pointer_outside_the_chrome_slides_it_away_and_letting_go_brings_it_back() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        settle(&mut menu, &settings, &state);
        let toolbar = menu.editor.blocks[0];
        assert!(menu.editor_takes(toolbar.centre()));
        // A press in the middle of the screen, which is the HUD layer's, sends the chrome off.
        let middle = Some(Vec2::new(640.0, 360.0));
        menu.editor_frame(&settings, &state, false, Input { pressed: true, held: true, ..input(middle) });
        for _ in 0..120 {
            menu.editor_frame(&settings, &state, false, Input { held: true, ..input(middle) });
        }
        assert!(!menu.editor_takes(toolbar.centre()), "the chrome is off the screen, so it takes no press");
        // The toolbar has slid up, out of the top of the screen.
        let (scene, _) = menu.editor_frame(&settings, &state, false, Input { held: true, ..input(middle) });
        assert!(scene.regions.iter().all(|r| r.rect.bottom() < 0.0 || r.rect.y > 0.0 && r.rect.y > 700.0 || r.rect.right() < 0.0 || r.rect.x > 1280.0), "every control is off the screen");
        menu.editor_frame(&settings, &state, false, input(middle));
        settle(&mut menu, &settings, &state);
        assert!(menu.editor_takes(toolbar.centre()));
    }

    #[test]
    fn a_press_on_the_chrome_keeps_it_in_place() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        settle(&mut menu, &settings, &state);
        let on_bar = Some(menu.editor.blocks[0].centre());
        menu.editor_frame(&settings, &state, false, Input { pressed: true, held: true, ..input(on_bar) });
        for _ in 0..60 {
            menu.editor_frame(&settings, &state, false, Input { held: true, ..input(on_bar) });
        }
        assert!(menu.editor_takes(on_bar.unwrap_or_default()));
    }

    #[test]
    fn the_library_scrolls_with_the_wheel_and_a_dragged_bar_and_never_past_its_ends() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        press(&mut menu, &settings, &state, "editor_library");
        settle(&mut menu, &settings, &state);
        let library = menu.editor.library_rect.expect("the library is open");
        let inside = Some(library.centre());
        let [content, view] = menu.editor.library_extent;
        assert!(content > view, "the cards do not all fit, so the list scrolls ({content} in {view})");
        for _ in 0..50 {
            menu.editor_frame(&settings, &state, false, Input { wheel: -3.0, ..input(inside) });
        }
        assert!((menu.editor.library_scroll - (content - view)).abs() < 1.0, "the wheel stops at the end");
        for _ in 0..50 {
            menu.editor_frame(&settings, &state, false, Input { wheel: 3.0, ..input(inside) });
        }
        assert!(menu.editor.library_scroll < 1.0, "the wheel stops at the start");
        // The bar's thumb, dragged to the bottom of its track, scrolls to the end.
        menu.editor_frame(&settings, &state, false, input(None));
        let (track, thumb) = menu.editor.bars[0].expect("a bar shows");
        let grab = Some(Vec2::new(thumb.centre().x, thumb.centre().y));
        menu.editor_frame(&settings, &state, false, Input { pressed: true, held: true, ..input(grab) });
        let low = Some(Vec2::new(thumb.centre().x, track.bottom() + 40.0));
        menu.editor_frame(&settings, &state, false, Input { held: true, ..input(low) });
        assert!((menu.editor.library_scroll - (content - view)).abs() < 1.0, "{} of {}", menu.editor.library_scroll, content - view);
    }

    #[test]
    fn a_card_cut_off_by_the_lists_edge_never_leaves_an_unclipped_label() {
        // Text with none of it inside the list's clip fails the whole panel, so every label must sit in a card's own clip.
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        press(&mut menu, &settings, &state, "editor_library");
        settle(&mut menu, &settings, &state);
        let inside = Some(menu.editor.library_rect.expect("open").centre());
        menu.editor_frame(&settings, &state, false, Input { wheel: -0.37, ..input(inside) });
        let (scene, _) = menu.editor_frame(&settings, &state, false, input(inside));
        let mut depth = 0;
        for primitive in &scene.primitives {
            match primitive {
                Primitive::PushClip { .. } => depth += 1,
                Primitive::PopClip => depth -= 1,
                Primitive::Text { text, .. } if menu.editor.library && matches!(text.as_str(), "CPS" | "FPS" | "Ping") => assert!(depth >= 2, "{text} is drawn outside its card's clip"),
                _ => {}
            }
        }
        assert_eq!(depth, 0, "every clip is closed");
    }

    #[test]
    fn a_press_inside_the_open_library_or_dock_keeps_the_chrome_where_it_is() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        press(&mut menu, &settings, &state, "editor_library");
        press(&mut menu, &settings, &state, "editor_dock");
        settle(&mut menu, &settings, &state);
        for rect in [menu.editor.library_rect.expect("library"), menu.editor.dock_rect.expect("dock")] {
            let at = Some(rect.centre());
            assert!(menu.editor_takes(rect.centre()));
            menu.editor_frame(&settings, &state, false, Input { pressed: true, held: true, ..input(at) });
            assert!(!menu.editor.hold_outside);
            for _ in 0..60 {
                menu.editor_frame(&settings, &state, false, Input { held: true, ..input(at) });
            }
            assert!(menu.editor_takes(rect.centre()), "the chrome stayed");
            menu.editor_frame(&settings, &state, false, input(at));
        }
    }

    #[test]
    fn with_the_chrome_away_nothing_of_the_toolbar_the_pill_or_the_open_sheet_is_left_on_the_screen() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        press(&mut menu, &settings, &state, "editor_sheet");
        settle(&mut menu, &settings, &state);
        assert!(menu.editor.sheet);
        let middle = Some(Vec2::new(640.0, 360.0));
        menu.editor_frame(&settings, &state, false, Input { pressed: true, held: true, ..input(middle) });
        let mut scene = Scene::new();
        for _ in 0..300 {
            scene = menu.editor_frame(&settings, &state, false, Input { held: true, ..input(middle) }).0;
        }
        for primitive in &scene.primitives {
            let rect = match primitive {
                Primitive::Rect { rect, .. } | Primitive::Text { rect, .. } | Primitive::Icon { rect, .. } | Primitive::Path { rect, .. } => *rect,
                _ => continue,
            };
            assert!(rect.bottom() <= 1.0 || rect.y >= 719.0, "{primitive:?} is still on the screen");
        }
    }

    #[test]
    fn a_wheel_notch_eases_the_list_to_its_place_instead_of_jumping() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        press(&mut menu, &settings, &state, "editor_library");
        settle(&mut menu, &settings, &state);
        let inside = Some(menu.editor.library_rect.expect("open").centre());
        menu.editor_frame(&settings, &state, false, Input { wheel: -1.0, ..input(inside) });
        let goal = menu.editor.goals[0];
        assert!(goal > 0.0);
        let first = menu.editor.library_scroll;
        assert!(first > 0.0 && first < goal, "one frame in, the list is part of the way ({first} of {goal})");
        let mut last = first;
        for _ in 0..90 {
            menu.editor_frame(&settings, &state, false, input(inside));
            assert!(menu.editor.library_scroll >= last - 1e-3, "it never turns back");
            last = menu.editor.library_scroll;
        }
        assert!((last - goal).abs() < 0.5, "and it arrives ({last} of {goal})");
    }

    /// Where the title of the panel called `title` is drawn, if the scene draws it.
    fn title_x(scene: &Scene, title: &str) -> Option<f32> {
        scene.primitives.iter().find_map(|p| match p {
            Primitive::Text { text, rect, .. } if text == title => Some(rect.x),
            _ => None,
        })
    }

    #[test]
    fn panels_slide_in_from_their_sides_when_opened_and_out_when_closed() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        settle(&mut menu, &settings, &state);
        assert_eq!(title_x(&menu.editor_frame(&settings, &state, false, input(None)).0, LIBRARY_HINT), None, "closed, it is not drawn");
        // Opening: the next frame has it part of the way in from the left, and it ends where it belongs.
        press(&mut menu, &settings, &state, "editor_library");
        let early = title_x(&menu.editor_frame(&settings, &state, false, input(None)).0, LIBRARY_HINT).expect("sliding in");
        settle(&mut menu, &settings, &state);
        let at_rest = title_x(&menu.editor_frame(&settings, &state, false, input(None)).0, LIBRARY_HINT).expect("open");
        assert!(early < at_rest, "it comes from the left ({early} then {at_rest})");
        assert!(at_rest > 0.0);
        // Closing: it slides back out and is no longer drawn.
        press(&mut menu, &settings, &state, "editor_library");
        let leaving = title_x(&menu.editor_frame(&settings, &state, false, input(None)).0, LIBRARY_HINT).expect("sliding out");
        assert!(leaving < at_rest, "it goes left again ({leaving})");
        settle(&mut menu, &settings, &state);
        assert_eq!(title_x(&menu.editor_frame(&settings, &state, false, input(None)).0, LIBRARY_HINT), None);
        // The dock comes from the right.
        press(&mut menu, &settings, &state, "editor_dock");
        let early = title_x(&menu.editor_frame(&settings, &state, false, input(None)).0, DOCK_HINT).expect("the dock sliding in");
        settle(&mut menu, &settings, &state);
        let at_rest = title_x(&menu.editor_frame(&settings, &state, false, input(None)).0, DOCK_HINT).expect("the dock open");
        assert!(early > at_rest, "it comes from the right ({early} then {at_rest})");
    }

    /// A context with the picked HUD's frame in the middle of the screen.
    fn context() -> EditorContext {
        EditorContext { selection: Some(Rect::new(500.0, 300.0, 120.0, 40.0)), size: [118.0, 36.0], locked: false, can_undo: true, can_redo: false, can_paste: false, deselected: false, picked: 1, grouped: false, marquee: None, selected_box: None, hover_box: None, unit: 1.0 }
    }

    /// Presses and releases the control that sends `id` with the editor given `ctx`.
    fn press_with(menu: &mut GuiMenu, settings: &Settings, state: &PanelState, ctx: &EditorContext, id: &str) -> Vec<Action> {
        for _ in 0..120 {
            menu.editor_frame_with(settings, state, false, ctx, input(None));
        }
        let n = menu.acts.iter().position(|a| matches!(a, Act::Press(p) if p == id)).unwrap_or_else(|| panic!("no control sends {id}"));
        let region = menu.editor_frame_with(settings, state, false, ctx, input(None)).0.regions.iter().find(|r| Hit::from_id(r.id) == Some(Hit::Act(n))).unwrap().rect;
        let at = Some(region.centre());
        menu.editor_frame_with(settings, state, false, ctx, Input { pressed: true, held: true, ..input(at) });
        menu.editor_frame_with(settings, state, false, ctx, input(at)).1
    }

    #[test]
    fn the_action_bar_over_the_picked_hud_locks_opens_its_settings_removes_it_and_shows_its_size() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        assert!(settings.is_on(state.element()));
        let ctx = context();
        assert_eq!(press_with(&mut menu, &settings, &state, &ctx, "editor_lock"), vec![Action::Event("editor_lock".to_owned())]);
        assert_eq!(press_with(&mut menu, &settings, &state, &ctx, "editor_gear"), vec![Action::Event("editor_gear".to_owned())]);
        assert_eq!(press_with(&mut menu, &settings, &state, &ctx, "editor_remove"), vec![Action::Event("editor_remove".to_owned())]);
        let (scene, _) = menu.editor_frame_with(&settings, &state, false, &ctx, input(None));
        assert!(scene.primitives.iter().any(|p| matches!(p, Primitive::Text { text, .. } if text == "118 \u{d7} 36")), "the size badge");
        // The bar sits above the frame and takes a press there; with no picked HUD on the screen there is no bar.
        let bar = menu.editor.blocks.iter().copied().find(|b| b.centre().x > 500.0 && b.centre().x < 620.0 && b.bottom() <= 300.0).expect("a bar above the frame");
        assert!(menu.editor_takes(bar.centre()));
        let none = EditorContext { selection: None, ..context() };
        menu.editor_frame_with(&settings, &state, false, &none, input(None));
        assert!(!menu.acts.iter().any(|a| matches!(a, Act::Press(p) if p == "editor_lock")));
    }

    #[test]
    fn undo_and_redo_buttons_are_only_there_when_there_is_something_to_do() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        let has = |menu: &GuiMenu, id: &str| menu.acts.iter().any(|a| matches!(a, Act::Press(p) if p == id));
        menu.editor_frame_with(&settings, &state, false, &EditorContext::default(), input(None));
        assert!(!has(&menu, "editor_undo") && !has(&menu, "editor_redo"));
        let ctx = EditorContext { can_undo: true, can_redo: true, ..context() };
        assert_eq!(press_with(&mut menu, &settings, &state, &ctx, "editor_undo"), vec![Action::Event("editor_undo".to_owned())]);
        assert_eq!(press_with(&mut menu, &settings, &state, &ctx, "editor_redo"), vec![Action::Event("editor_redo".to_owned())]);
    }

    #[test]
    fn the_grid_shows_as_lines_only_while_it_is_on() {
        let (mut settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        let lines = |menu: &mut GuiMenu, settings: &Settings| {
            let ctx = EditorContext { unit: 1.0, ..EditorContext::default() };
            menu.editor_frame_with(settings, &state, false, &ctx, input(None)).0.primitives.iter().filter(|p| matches!(p, Primitive::Line { .. })).count()
        };
        assert_eq!(lines(&mut menu, &settings), 0);
        settings.show_grid = true;
        assert_eq!(lines(&mut menu, &settings), 1, "one primitive holds every line");
        // The grid's step in GUI pixels is what the snapping uses.
        assert_eq!(GRID_STEP, 8.0);
    }

    #[test]
    fn the_library_search_keeps_the_huds_whose_name_has_every_word() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        press(&mut menu, &settings, &state, "editor_library");
        let adds = |menu: &GuiMenu| -> Vec<String> { menu.acts.iter().filter_map(|a| match a { Act::Press(p) => p.strip_prefix("editor_add_").map(str::to_owned), _ => None }).collect() };
        settle(&mut menu, &settings, &state);
        let everything = adds(&menu);
        assert!(everything.len() > 3, "{everything:?}");
        menu.editor.query = "fps".to_owned();
        settle(&mut menu, &settings, &state);
        assert_eq!(adds(&menu), ["hud_fps"]);
        menu.editor.query = "zzz nothing".to_owned();
        let (scene, _) = menu.editor_frame(&settings, &state, false, input(None));
        assert!(adds(&menu).is_empty());
        assert!(scene.primitives.iter().any(|p| matches!(p, Primitive::Text { text, .. } if text == "No HUDs match")));
    }

    #[test]
    fn typing_in_the_search_field_filters_and_scrolls_back_to_the_top() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        press(&mut menu, &settings, &state, "editor_library");
        menu.editor.goals[0] = 80.0;
        let actions = menu.editor_actions(vec![Action::Text { id: "editor_search".to_owned(), text: "ping".to_owned(), submit: false }], &settings);
        assert!(actions.is_empty(), "the field's text stays with the editor");
        assert_eq!(menu.editor.query, "ping");
        assert_eq!(menu.editor.goals[0], 0.0);
    }

    #[test]
    fn a_card_dragged_out_of_the_library_drops_its_hud_where_it_is_let_go() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        press(&mut menu, &settings, &state, "editor_library");
        settle(&mut menu, &settings, &state);
        let n = menu.acts.iter().position(|a| matches!(a, Act::Press(p) if p == "editor_add_hud_fps")).expect("the FPS card");
        let card = menu.editor_frame(&settings, &state, false, input(None)).0.regions.iter().find(|r| Hit::from_id(r.id) == Some(Hit::Act(n))).unwrap().rect;
        let down = Some(card.centre());
        menu.editor_frame(&settings, &state, false, Input { pressed: true, held: true, ..input(down) });
        // Moved out onto the screen, the card is a drag and the chrome slides away.
        let out = Some(Vec2::new(700.0, 400.0));
        for _ in 0..60 {
            menu.editor_frame(&settings, &state, false, Input { held: true, ..input(out) });
        }
        assert!(menu.editor.place.as_ref().is_some_and(|drag| drag.active));
        assert!(!menu.editor_takes(menu.editor.blocks[0].centre()), "the chrome is out of the way");
        let (_, actions) = menu.editor_frame(&settings, &state, false, input(out));
        let element = Element::ALL.iter().find(|e| e.toggle_id() == "hud_fps").expect("an element").index();
        assert_eq!(actions, vec![Action::EventValue("hud_fps".to_owned(), 1.0), Action::Event(format!("editor_pick_{element}")), Action::Event(format!("editor_place_{element}"))]);
        assert!(menu.editor.place.is_none());
    }

    #[test]
    fn a_card_pressed_and_let_go_where_it_was_is_just_a_click_and_adds_without_placing() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        let actions = press(&mut menu, &settings, &state, "editor_library");
        assert!(actions.is_empty());
        let actions = press(&mut menu, &settings, &state, "editor_add_hud_fps");
        assert!(actions.iter().all(|a| !matches!(a, Action::Event(id) if id.starts_with("editor_place_"))), "{actions:?}");
        assert!(actions.contains(&Action::EventValue("hud_fps".to_owned(), 1.0)));
    }

    #[test]
    fn the_grids_lines_are_counted_from_the_middle_so_two_of_them_are_the_centre_lines() {
        let (mut settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        settings.show_grid = true;
        let ctx = EditorContext { unit: 2.0, ..EditorContext::default() };
        let (scene, _) = menu.editor_frame_with(&settings, &state, false, &ctx, input(None));
        let contours = scene.primitives.iter().find_map(|p| match p { Primitive::Line { contours, .. } => Some(contours.clone()), _ => None }).expect("the grid");
        let verticals: Vec<f32> = contours.iter().filter(|c| c[0][0] == c[1][0]).map(|c| c[0][0]).collect();
        let horizontals: Vec<f32> = contours.iter().filter(|c| c[0][1] == c[1][1]).map(|c| c[0][1]).collect();
        // The viewport is 1280 by 720 and a GUI pixel is 2 units, so the lines are 16 apart from 640 and from 360.
        assert!(verticals.contains(&640.0) && verticals.contains(&656.0) && verticals.contains(&624.0), "{verticals:?}");
        assert!(horizontals.contains(&360.0) && horizontals.contains(&376.0) && horizontals.contains(&344.0), "{horizontals:?}");
        // Each line is there once.
        let mut sorted = verticals.clone();
        sorted.sort_by(f32::total_cmp);
        sorted.dedup();
        assert_eq!(sorted.len(), verticals.len());
    }

    #[test]
    fn the_tour_opens_by_itself_until_it_is_finished_or_skipped() {
        let (settings, state, mut menu) = (Settings::default(), PanelState::default(), GuiMenu::default());
        assert!(!settings.editor_tour_seen);
        let (scene, _) = menu.editor_frame(&settings, &state, false, input(None));
        assert_eq!(menu.editor.tour, Some(0));
        assert!(scene.primitives.iter().any(|p| matches!(p, Primitive::Text { text, .. } if text == TOUR[0].0)));
        // Next goes on a step, Back goes back.
        assert!(press(&mut menu, &settings, &state, "editor_tour_next").is_empty());
        assert_eq!(menu.editor.tour, Some(1));
        assert!(press(&mut menu, &settings, &state, "editor_tour_back").is_empty());
        assert_eq!(menu.editor.tour, Some(0));
        // Skip ends it and tells the game it was seen.
        assert_eq!(press(&mut menu, &settings, &state, "editor_tour_skip"), vec![Action::Event("editor_tour_done".to_owned())]);
        assert_eq!(menu.editor.tour, None);
        // And it does not come back during this opening.
        menu.editor_frame(&settings, &state, false, input(None));
        assert_eq!(menu.editor.tour, None);
    }

    #[test]
    fn the_last_step_of_the_tour_is_done_and_finishing_tells_the_game() {
        let (settings, state, mut menu) = (Settings::default(), PanelState::default(), GuiMenu::default());
        menu.editor_frame(&settings, &state, false, input(None));
        menu.editor.tour = Some(TOUR.len() - 1);
        assert_eq!(press(&mut menu, &settings, &state, "editor_tour_next"), vec![Action::Event("editor_tour_done".to_owned())]);
        assert_eq!(menu.editor.tour, None);
    }

    #[test]
    fn a_player_who_has_seen_the_tour_gets_it_back_from_the_context_menu() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        settle(&mut menu, &settings, &state);
        assert_eq!(menu.editor.tour, None);
        let at = Some(Vec2::new(640.0, 360.0));
        menu.editor_frame(&settings, &state, false, Input { secondary: true, ..input(at) });
        assert!(menu.editor.menu.is_some(), "a right press on the screen opens the menu");
        press(&mut menu, &settings, &state, "editor_tour");
        assert_eq!(menu.editor.tour, Some(0));
        assert!(menu.editor.menu.is_none());
    }

    #[test]
    fn the_context_menu_has_the_picked_huds_commands_and_closes_after_one_or_a_press_elsewhere() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        let ctx = EditorContext { can_paste: true, ..context() };
        for _ in 0..120 {
            menu.editor_frame_with(&settings, &state, false, &ctx, input(None));
        }
        let at = Some(Vec2::new(700.0, 420.0));
        menu.editor_frame_with(&settings, &state, false, &ctx, Input { secondary: true, ..input(at) });
        menu.editor_frame_with(&settings, &state, false, &ctx, input(at));
        for id in ["editor_gear", "editor_lock", "editor_copy_look", "editor_paste_look", "move_reset", "editor_remove", "editor_undo", "move_grid", "move_guides", "editor_tour"] {
            assert!(menu.acts.iter().any(|a| matches!(a, Act::Press(p) if p == id)), "the menu has {id}");
        }
        assert!(!menu.acts.iter().any(|a| matches!(a, Act::Press(p) if p == "editor_redo")), "nothing to redo");
        // A command sends its event and closes the menu.
        let n = menu.acts.iter().position(|a| matches!(a, Act::Press(p) if p == "editor_lock")).unwrap();
        let row = menu.editor_frame_with(&settings, &state, false, &ctx, input(at)).0.regions.iter().find(|r| Hit::from_id(r.id) == Some(Hit::Act(n))).unwrap().rect;
        let on_row = Some(row.centre());
        menu.editor_frame_with(&settings, &state, false, &ctx, Input { pressed: true, held: true, ..input(on_row) });
        let (_, actions) = menu.editor_frame_with(&settings, &state, false, &ctx, input(on_row));
        assert_eq!(actions, vec![Action::Event("editor_lock".to_owned())]);
        assert!(menu.editor.menu.is_none());
        // A press away from it closes it too.
        menu.editor_frame_with(&settings, &state, false, &ctx, Input { secondary: true, ..input(at) });
        assert!(menu.editor.menu.is_some());
        menu.editor_frame_with(&settings, &state, false, &ctx, Input { pressed: true, held: true, ..input(Some(Vec2::new(40.0, 700.0))) });
        assert!(menu.editor.menu.is_none());
    }

    #[test]
    fn without_a_picked_hud_the_context_menu_has_only_the_editors_own_commands() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        settle(&mut menu, &settings, &state);
        let at = Some(Vec2::new(300.0, 300.0));
        menu.editor_frame(&settings, &state, false, Input { secondary: true, ..input(at) });
        menu.editor_frame(&settings, &state, false, input(at));
        assert!(!menu.acts.iter().any(|a| matches!(a, Act::Press(p) if p == "editor_lock" || p == "editor_remove")));
        assert!(menu.acts.iter().any(|a| matches!(a, Act::Press(p) if p == "move_grid")));
    }

    /// Every text the scene draws.
    fn texts(scene: &Scene) -> Vec<String> {
        scene.primitives.iter().filter_map(|p| match p { Primitive::Text { text, .. } => Some(text.clone()), _ => None }).collect()
    }

    #[test]
    fn the_docks_tabs_split_a_huds_options_into_layout_style_and_settings() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        assert!(settings.is_on(state.element()));
        press(&mut menu, &settings, &state, "editor_dock");
        settle(&mut menu, &settings, &state);
        let shown = |menu: &mut GuiMenu| texts(&menu.editor_frame(&settings, &state, false, input(None)).0);
        let layout = shown(&mut menu);
        assert!(layout.contains(&"POSITION".to_owned()) && layout.contains(&"SIZE".to_owned()), "{layout:?}");
        assert!(!layout.contains(&"BOX".to_owned()) && !layout.contains(&"DISPLAY".to_owned()));
        press(&mut menu, &settings, &state, "editor_tab_style");
        let style = shown(&mut menu);
        assert!(style.contains(&"BOX".to_owned()) && style.contains(&"TEXT".to_owned()), "{style:?}");
        assert!(!style.contains(&"POSITION".to_owned()) && !style.contains(&"DISPLAY".to_owned()));
        press(&mut menu, &settings, &state, "editor_tab_settings");
        let own = shown(&mut menu);
        assert!(own.contains(&"DISPLAY".to_owned()), "{own:?}");
        assert!(!own.contains(&"POSITION".to_owned()) && !own.contains(&"BOX".to_owned()));
    }

    #[test]
    fn the_docks_search_keeps_the_rows_with_every_word_and_says_when_none_match() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        press(&mut menu, &settings, &state, "editor_dock");
        settle(&mut menu, &settings, &state);
        let rows = |menu: &mut GuiMenu| menu.editor_frame(&settings, &state, false, input(None)).0.regions.len();
        let all = rows(&mut menu);
        menu.editor.dock_query = "scale".to_owned();
        let some = rows(&mut menu);
        assert!(some < all, "{some} controls of {all}");
        menu.editor.dock_query = "zzzz".to_owned();
        let texts_now = texts(&menu.editor_frame(&settings, &state, false, input(None)).0);
        assert!(texts_now.contains(&"No settings match".to_owned()));
        // What is typed in the field stays with the editor and takes the list back to the top.
        menu.editor.dock_scroll = 50.0;
        let actions = menu.editor_actions(vec![Action::Text { id: "editor_dock_search".to_owned(), text: "size".to_owned(), submit: false }], &settings);
        assert!(actions.is_empty());
        assert_eq!((menu.editor.dock_query.as_str(), menu.editor.dock_scroll), ("size", 0.0));
    }

    #[test]
    fn a_marquee_is_drawn_as_a_box_and_picking_several_huds_offers_to_group_them() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        let plain = EditorContext::default();
        let boxes = |menu: &mut GuiMenu, ctx: &EditorContext| menu.editor_frame_with(&settings, &state, false, ctx, input(None)).0.primitives.len();
        let without = boxes(&mut menu, &plain);
        let marquee = EditorContext { marquee: Some(Rect::new(100.0, 100.0, 200.0, 120.0)), ..plain };
        assert!(boxes(&mut menu, &marquee) > without, "the marquee adds its fill and its outline");
        // Two picked HUDs can be grouped; one that is in a group can be taken out of it.
        let open_menu = |menu: &mut GuiMenu, ctx: &EditorContext| {
            for _ in 0..120 {
                menu.editor_frame_with(&settings, &state, false, ctx, input(None));
            }
            let at = Some(Vec2::new(700.0, 300.0));
            menu.editor_frame_with(&settings, &state, false, ctx, Input { secondary: true, ..input(at) });
            menu.editor_frame_with(&settings, &state, false, ctx, input(at));
        };
        let has = |menu: &GuiMenu, id: &str| menu.acts.iter().any(|a| matches!(a, Act::Press(p) if p == id));
        open_menu(&mut menu, &EditorContext { picked: 2, ..context() });
        assert!(has(&menu, "editor_group") && !has(&menu, "editor_ungroup"));
        let mut again = GuiMenu::default();
        open_menu(&mut again, &EditorContext { picked: 1, grouped: true, ..context() });
        assert!(has(&again, "editor_ungroup") && !has(&again, "editor_group"));
        let mut alone = GuiMenu::default();
        open_menu(&mut alone, &context());
        assert!(!has(&alone, "editor_group") && !has(&alone, "editor_ungroup"));
    }

    #[test]
    fn holding_alt_measures_the_gap_between_the_picked_hud_and_the_one_under_the_pointer() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        // The picked HUD and one 40 units to its right and a little lower: a GUI pixel is 2 units, so the gap is 20.
        let ctx = EditorContext { selected_box: Some(Rect::new(100.0, 100.0, 60.0, 20.0)), hover_box: Some(Rect::new(200.0, 110.0, 60.0, 20.0)), unit: 2.0, ..EditorContext::default() };
        let drawn = |menu: &mut GuiMenu, alt: bool| menu.editor_frame_with(&settings, &state, false, &ctx, Input { alt, ..input(None) }).0;
        let measured = drawn(&mut menu, true);
        assert!(measured.primitives.iter().any(|p| matches!(p, Primitive::Text { text, .. } if text == "20")), "the gap in GUI pixels");
        let plain = drawn(&mut menu, false);
        assert!(!plain.primitives.iter().any(|p| matches!(p, Primitive::Text { text, .. } if text == "20")), "nothing is measured without Alt");
        // One above the other measures the vertical gap; overlapping HUDs have no gap to measure.
        let above = EditorContext { selected_box: Some(Rect::new(100.0, 100.0, 60.0, 20.0)), hover_box: Some(Rect::new(110.0, 160.0, 60.0, 20.0)), unit: 2.0, ..EditorContext::default() };
        let (scene, _) = menu.editor_frame_with(&settings, &state, false, &above, Input { alt: true, ..input(None) });
        assert!(scene.primitives.iter().any(|p| matches!(p, Primitive::Text { text, .. } if text == "20")));
        let touching = EditorContext { hover_box: Some(Rect::new(120.0, 110.0, 60.0, 20.0)), ..ctx };
        let (scene, _) = menu.editor_frame_with(&settings, &state, false, &touching, Input { alt: true, ..input(None) });
        assert!(!scene.primitives.iter().any(|p| matches!(p, Primitive::Line { color, .. } if color.a < 0.95 && color.a > 0.8)), "no gap line");
    }

    #[test]
    fn with_nothing_picked_the_dock_asks_for_a_hud_and_there_is_no_action_bar() {
        let (settings, state, mut menu) = (seen(), PanelState::default(), GuiMenu::default());
        let picked = context();
        let nothing = EditorContext { deselected: true, ..context() };
        for _ in 0..120 {
            menu.editor_frame_with(&settings, &state, false, &picked, input(None));
        }
        menu.editor.dock = true;
        for _ in 0..120 {
            menu.editor_frame_with(&settings, &state, false, &picked, input(None));
        }
        let has = |menu: &GuiMenu, id: &str| menu.acts.iter().any(|a| matches!(a, Act::Press(p) if p == id) || matches!(a, Act::Field { id: f, .. } if f == id));
        let scene = menu.editor_frame_with(&settings, &state, false, &picked, input(None)).0;
        assert!(has(&menu, "editor_lock") && has(&menu, "editor_dock_search"));
        assert!(!texts(&scene).contains(&"Pick a HUD".to_owned()));
        let scene = menu.editor_frame_with(&settings, &state, false, &nothing, input(None)).0;
        assert!(!has(&menu, "editor_lock") && !has(&menu, "editor_dock_search"), "no bar and no rows");
        assert!(texts(&scene).contains(&"Pick a HUD".to_owned()));
    }
}
