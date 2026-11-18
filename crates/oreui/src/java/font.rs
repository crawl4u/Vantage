//! Java text: glyphs from Java's own bitmap sheets, placed as the game places them. Each glyph
//! moves the pen by its advance, the text sits on an 8-pixel line, and a shadow one Java pixel
//! down and right is drawn first at a quarter of the colour. Text with a character the font
//! lacks is not drawn here; the caller falls back to the client's own font for it.

use ui_engine::engine::screen::PaintError;

use crate::{
    images::{
        java::{self, key},
        java_font::{Glyph, JavaFont},
    },
    paint::{Bounds, Canvas},
};

/// Height of a line of Java text, in Java pixels.
pub(super) const LINE: f32 = 8.0;

/// A glyph and where the pen was before it, in Java pixels.
struct Placed<'a> {
    glyph: &'a Glyph,
    pen: f32,
}

/// Lays `text` out left to right; none when any character has no glyph.
fn layout<'a>(font: &'a JavaFont, text: &str) -> Option<(Vec<Placed<'a>>, f32)> {
    let mut pen = 0.0;
    let mut placed = Vec::new();
    for character in text.chars() {
        let glyph = font.glyph(character)?;
        placed.push(Placed { glyph, pen });
        pen += glyph.advance;
    }
    Some((placed, pen))
}

/// The width of `text` in Java pixels, none when the font cannot draw all of it.
pub(super) fn width_with(font: &JavaFont, text: &str) -> Option<f32> {
    layout(font, text).map(|(_, width)| width)
}

/// The shadow's colour: a quarter of each colour channel.
fn shadow_colour(colour: [u8; 4]) -> [u8; 4] {
    [colour[0] / 4, colour[1] / 4, colour[2] / 4, colour[3]]
}

/// Draws `text` with its top left corner at `at` (logical pixels), `unit` logical pixels per Java
/// pixel. Returns the width drawn in logical pixels, or none, drawing nothing, when the font or
/// its sheets cannot draw every character.
pub(super) fn draw_with(
    font: &JavaFont,
    canvas: &mut Canvas<'_>,
    text: &str,
    at: [f32; 2],
    unit: f32,
    colour: [u8; 4],
    shadow: bool,
) -> Result<Option<f32>, PaintError> {
    let Some((placed, width)) = layout(font, text) else {
        return Ok(None);
    };
    let sheet_key = |glyph: &Glyph| {
        glyph
            .sheet
            .as_ref()
            .map(|name| key(&format!("font/{name}")))
    };
    if placed
        .iter()
        .filter_map(|placed| sheet_key(placed.glyph))
        .any(|sheet| !canvas.has_sprite(&sheet))
    {
        return Ok(None);
    }
    // Whole logical pixels keep the glyph texels crisp.
    let origin = [at[0].round(), at[1].round()];
    let passes = if shadow {
        vec![(unit, shadow_colour(colour)), (0.0, colour)]
    } else {
        vec![(0.0, colour)]
    };
    for (offset, pass_colour) in passes {
        for placed in &placed {
            let Some(sheet) = sheet_key(placed.glyph) else {
                continue;
            };
            let glyph = placed.glyph;
            let left = origin[0] + offset + placed.pen * unit;
            let top = origin[1] + offset + glyph.top * unit;
            let bounds = [
                left,
                top,
                left + glyph.size[0] * unit,
                top + glyph.size[1] * unit,
            ];
            canvas.sprite_region(&sheet, glyph.cell, bounds, pass_colour)?;
        }
    }
    Ok(Some(width * unit))
}

/// Draws `text` centred in `bounds`: across the middle, and on the line that Java centres in a
/// button, whose top is half of what is left over after an 8-pixel line. False when the font
/// cannot draw all of it.
pub(super) fn centred_with(
    font: &JavaFont,
    canvas: &mut Canvas<'_>,
    text: &str,
    bounds: Bounds,
    unit: f32,
    colour: [u8; 4],
    shadow: bool,
) -> Result<bool, PaintError> {
    let Some(width) = width_with(font, text) else {
        return Ok(false);
    };
    let left = (bounds[0] + bounds[2] - width * unit) * 0.5;
    let top = bounds[1] + (bounds[3] - bounds[1] - LINE * unit) * 0.5;
    Ok(draw_with(font, canvas, text, [left, top], unit, colour, shadow)?.is_some())
}

/// Draws `text` turned `angle` radians about `pivot` and `scale` logical pixels per Java pixel, the
/// way the title's splash is drawn: centred across the pivot, its line running from 8 Java pixels
/// above it to the pivot, the shadow one scaled Java pixel down and right. Returns false, drawing
/// nothing, when the font cannot draw all of it.
pub(super) fn draw_turned_with(
    font: &JavaFont,
    canvas: &mut Canvas<'_>,
    text: &str,
    pivot: [f32; 2],
    scale: f32,
    angle: f32,
    colour: [u8; 4],
) -> Result<bool, PaintError> {
    let Some((placed, width)) = layout(font, text) else {
        return Ok(false);
    };
    let sheet_key = |glyph: &Glyph| {
        glyph
            .sheet
            .as_ref()
            .map(|name| key(&format!("font/{name}")))
    };
    if placed
        .iter()
        .filter_map(|placed| sheet_key(placed.glyph))
        .any(|sheet| !canvas.has_sprite(&sheet))
    {
        return Ok(false);
    }
    let (sin, cos) = angle.sin_cos();
    for (offset, pass_colour) in [(scale, shadow_colour(colour)), (0.0, colour)] {
        for placed in &placed {
            let Some(sheet) = sheet_key(placed.glyph) else {
                continue;
            };
            let glyph = placed.glyph;
            // The glyph's box before turning, with the pivot as the origin.
            let left = (placed.pen - width * 0.5) * scale + offset;
            let top = (glyph.top - LINE) * scale + offset;
            let (w, h) = (glyph.size[0] * scale, glyph.size[1] * scale);
            let (cx, cy) = (left + w * 0.5, top + h * 0.5);
            let centre = [
                pivot[0] + cx * cos - cy * sin,
                pivot[1] + cx * sin + cy * cos,
            ];
            let bounds = [
                centre[0] - w * 0.5,
                centre[1] - h * 0.5,
                centre[0] + w * 0.5,
                centre[1] + h * 0.5,
            ];
            canvas.sprite_region_rotated(&sheet, glyph.cell, bounds, pass_colour, angle)?;
        }
    }
    Ok(true)
}

/// [`draw_with`] on the game's font from the player's Java install; none without it.
pub(super) fn draw(
    canvas: &mut Canvas<'_>,
    text: &str,
    at: [f32; 2],
    unit: f32,
    colour: [u8; 4],
    shadow: bool,
) -> Result<Option<f32>, PaintError> {
    match java::font() {
        Some(font) => draw_with(&font, canvas, text, at, unit, colour, shadow),
        None => Ok(None),
    }
}

/// [`draw_turned_with`] on the game's font from the player's Java install; false without it.
pub(super) fn draw_turned(
    canvas: &mut Canvas<'_>,
    text: &str,
    pivot: [f32; 2],
    scale: f32,
    angle: f32,
    colour: [u8; 4],
) -> Result<bool, PaintError> {
    match java::font() {
        Some(font) => draw_turned_with(&font, canvas, text, pivot, scale, angle, colour),
        None => Ok(false),
    }
}

/// [`centred_with`] on the game's font from the player's Java install; false without it.
pub(super) fn centred(
    canvas: &mut Canvas<'_>,
    text: &str,
    bounds: Bounds,
    unit: f32,
    colour: [u8; 4],
    shadow: bool,
) -> Result<bool, PaintError> {
    match java::font() {
        Some(font) => centred_with(&font, canvas, text, bounds, unit, colour, shadow),
        None => Ok(false),
    }
}
