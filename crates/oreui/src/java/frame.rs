//! The frame the Java-style list screens share, as Java Edition builds them: the world dimmed
//! behind a header with the title and a footer of button rows, separated from a shaded list by
//! Java's separator sprites, with Back leaving the screen. Each screen draws only its list and
//! names its buttons.

use launcher::menu::{MenuAction, MenuView};
use ui_engine::engine::screen::PaintError;

use super::{button, font, unit};
use crate::{
    focus,
    paint::{Bounds, Canvas, SpriteScrollbar},
    theme::TEXT,
};

/// Sizes in Java pixels.
/// The header's height when a screen asks for no other.
pub(super) const HEADER: f32 = 33.0;
const FOOTER: f32 = 64.0;
const TITLE_TOP: f32 = 10.0;
const TEXT_HEIGHT: f32 = 8.0;
const SEPARATOR: f32 = 2.0;
const BUTTON_HEIGHT: f32 = 20.0;
const BUTTON_WIDTH: f32 = 100.0;
const BUTTON_GAP: f32 = 5.0;
/// The width a row of buttons shares, as Java's three and four button rows do.
const ROW_WIDTH: f32 = 310.0;
/// Where the rows of buttons start below the footer's top edge, and the distance between rows.
const FIRST_ROW: f32 = 12.0;
const ROW_STEP: f32 = 24.0;
/// The corner button that opens the Bedrock-style page: its size and distance from the edge.
const MORE: [f32; 2] = [50.0, 16.0];
const MORE_EDGE: f32 = 4.0;
const SCROLLBAR_WIDTH: f32 = 6.0;
const SCROLLER_MIN: f32 = 32.0;
const DIM: [u8; 4] = [16, 16, 16, 170];
const LIST_SHADE: [u8; 4] = [0, 0, 0, 90];
const SUBTITLE: [u8; 4] = [128, 128, 128, 255];

/// One footer button.
#[derive(Clone, Copy)]
pub(super) struct Btn<'a> {
    pub label: &'a str,
    /// What it does; none draws it disabled.
    pub action: Option<MenuAction>,
    /// Drawn highlighted, as a chosen option is.
    pub lit: bool,
}

impl<'a> Btn<'a> {
    pub(super) fn new(label: &'a str, action: Option<MenuAction>) -> Self {
        Self {
            label,
            action,
            lit: false,
        }
    }
}

/// What a screen puts in the frame.
pub(super) struct Spec<'a> {
    pub title: &'a str,
    pub subtitle: Option<&'a str>,
    /// The header's height in Java pixels, taller where the screen puts controls in it.
    pub header: f32,
    /// Rows of buttons, top to bottom, each centred and sharing its width.
    pub rows: &'a [&'a [Btn<'a>]],
    /// Opens the Bedrock-style page of the same settings, which keeps what the screen leaves out.
    pub more: Option<MenuAction>,
}

/// Where the list sits once the frame is drawn.
pub(super) struct Frame {
    pub unit: f32,
    pub centre: f32,
    pub viewport: Bounds,
}

impl Frame {
    /// A column of `width` Java pixels whose left edge is `left` Java pixels from the centre,
    /// as logical pixels.
    pub(super) fn column(&self, left: f32, width: f32) -> [f32; 2] {
        let x = (self.centre + left) * self.unit;
        [x, x + width * self.unit]
    }
}

/// Draws the frame and, between its shade and its separators, `body`.
pub(super) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
    spec: &Spec<'_>,
    body: impl FnOnce(&mut Canvas<'_>, &Frame) -> Result<(), PaintError>,
) -> Result<(), PaintError> {
    let unit = unit(view, canvas, size);
    let java_width = (size[0] / unit).ceil();
    let centre = (java_width / 2.0).floor();
    canvas.fill([0.0, 0.0, size[0], size[1]], DIM)?;
    let screen =
        canvas.begin_focus_region(focus::SCREEN, [0.0, 0.0, size[0], size[1]], None, true)?;
    canvas.focus_delegate(Some(MenuAction::AddBack), None);
    let frame = Frame {
        unit,
        centre,
        viewport: [0.0, spec.header * unit, size[0], size[1] - FOOTER * unit],
    };
    canvas.fill(frame.viewport, LIST_SHADE)?;
    body(canvas, &frame)?;
    separators(canvas, size, spec.header, unit)?;
    let line = |canvas: &mut Canvas<'_>, text: &str, top: f32, colour: [u8; 4]| {
        button::label_centred(
            canvas,
            text,
            [0.0, top * unit, size[0], (top + TEXT_HEIGHT) * unit],
            unit,
            colour,
        )
    };
    match spec.subtitle {
        Some(subtitle) => {
            line(canvas, spec.title, TITLE_TOP - 4.0, TEXT)?;
            line(canvas, subtitle, TITLE_TOP + 6.0, SUBTITLE)?;
        }
        None => line(canvas, spec.title, TITLE_TOP + 4.0, TEXT)?,
    }
    let top = size[1] / unit - FOOTER;
    for (index, row) in spec.rows.iter().enumerate() {
        let count = row.len().max(1) as f32;
        let width = BUTTON_WIDTH.min((ROW_WIDTH - BUTTON_GAP * (count - 1.0)) / count);
        let total = count * width + (count - 1.0) * BUTTON_GAP;
        let y = top + FIRST_ROW + index as f32 * ROW_STEP;
        for (place, item) in row.iter().enumerate() {
            let x = centre - total / 2.0 + place as f32 * (width + BUTTON_GAP);
            let bounds = [x, y, x + width, y + BUTTON_HEIGHT].map(|value| value * unit);
            button::draw_lit(
                canvas,
                view,
                bounds,
                item.label,
                item.action,
                item.lit,
                unit,
            )?;
        }
    }
    if let Some(action) = spec.more {
        let x = java_width - MORE[0] - MORE_EDGE;
        let bounds = [x, MORE_EDGE, x + MORE[0], MORE_EDGE + MORE[1]].map(|value| value * unit);
        button::draw(canvas, view, bounds, "More...", Some(action), unit)?;
    }
    canvas.end_focus_region(screen);
    Ok(())
}

/// The header and footer's inner edges, from Java's separator sprites.
fn separators(
    canvas: &mut Canvas<'_>,
    size: [f32; 2],
    header: f32,
    unit: f32,
) -> Result<(), PaintError> {
    for (name, top) in [
        ("header_separator.png", (header - SEPARATOR) * unit),
        ("footer_separator.png", size[1] - FOOTER * unit),
    ] {
        let key = crate::images::java::key(name);
        let Some([width, _]) = canvas.sprite_size(&key) else {
            continue;
        };
        let tile = (width * unit).max(1.0);
        let mut x = 0.0;
        while x < size[0] {
            canvas.sprite(
                &key,
                [x, top, (x + tile).min(size[0]), top + SEPARATOR * unit],
                [255; 4],
            )?;
            x += tile;
        }
    }
    Ok(())
}

/// A scrolling list inside `viewport` (logical pixels): draws `rows` through `draw`, which gets
/// the scroll offset, with Java's scroll bar `bar_offset` Java pixels right of `bar_from`.
pub(super) fn scrolled(
    canvas: &mut Canvas<'_>,
    key: &str,
    viewport: Bounds,
    content: f32,
    bar_x: f32,
    unit: f32,
    draw: impl FnOnce(&mut Canvas<'_>, f32) -> Result<(), PaintError>,
) -> Result<(), PaintError> {
    let max = (content - (viewport[3] - viewport[1])).max(0.0);
    if let Some(offset) = canvas.offsets.get_mut(key) {
        *offset = offset.min(max);
    }
    canvas.sprite_scrollbar = Some(SpriteScrollbar {
        track: crate::images::java::key("sprites/widget/scroller_background.png"),
        thumb: crate::images::java::key("sprites/widget/scroller.png"),
        x: bar_x,
        width: SCROLLBAR_WIDTH * unit,
        min_thumb: SCROLLER_MIN * unit,
    });
    let scroll = canvas.begin_scroll(key, viewport)?;
    draw(canvas, scroll.offset)?;
    canvas.end_scroll(scroll, content)
}

/// `text` drawn with its top left at `at` in Java's font, else the client's, and the width the
/// text takes in Java pixels when Java's font draws it.
pub(super) fn put(
    canvas: &mut Canvas<'_>,
    text: &str,
    at: [f32; 2],
    unit: f32,
    colour: [u8; 4],
    room: f32,
) -> Result<(), PaintError> {
    // Text too long for its room ends in an ellipsis, as Java cuts a long name.
    let text = button::fitted(text, room / unit);
    if font::draw(canvas, &text, at, unit, colour, true)?.is_some() {
        return Ok(());
    }
    let style = button::label_style(canvas, unit);
    canvas.text_line(&text, [at[0], at[1] - unit], room, style, colour)?;
    Ok(())
}
