//! A Java-style button: the widget sprite stretched as a nine-slice, white label with shadow.

use launcher::menu::{MenuAction, MenuView};
use ui_engine::engine::screen::PaintError;

use crate::{
    images::java::{border, border_units, key},
    paint::{Bounds, Canvas},
    theme::{BODY, TEXT, Type},
};

const REST: &str = "sprites/widget/button.png";
const HOVER: &str = "sprites/widget/button_highlighted.png";
const DISABLED: &str = "sprites/widget/button_disabled.png";
/// Label colour of a button that cannot be used.
const DISABLED_TEXT: [u8; 4] = [160, 160, 160, 255];

/// The label style at `unit` logical pixels per Java pixel: Java text is 8 pixels tall.
pub(crate) fn label_style(canvas: &Canvas<'_>, unit: f32) -> Type {
    let size = 8.0 * unit / canvas.rem.max(f32::EPSILON) * 1.35;
    Type {
        size,
        line: size * 1.25,
        ..BODY
    }
}

/// `label` cut short with an ellipsis until Java's font draws it in at most `max` Java pixels, as
/// Java does for a button whose text is too long. A label that fits, or that Java's font cannot
/// measure, is returned as it is.
pub(super) fn fitted(label: &str, max: f32) -> String {
    let Some(font) = crate::images::java::font() else {
        return label.to_owned();
    };
    let width = |text: &str| super::font::width_with(&font, text);
    if width(label).is_none_or(|width| width <= max) {
        return label.to_owned();
    }
    let mut kept: Vec<char> = label.chars().collect();
    while kept.pop().is_some() {
        let candidate: String = kept.iter().collect::<String>().trim_end().to_owned() + "...";
        if width(&candidate).is_none_or(|width| width <= max) {
            return candidate;
        }
    }
    "...".to_owned()
}

/// How far, in Java pixels, a label wider than its button by `overflow` has scrolled `seconds`
/// into the animation: Java's own curve, which eases from the start of the text to its end and
/// back, spending a moment at each end.
fn scroll_offset(overflow: f32, seconds: f64) -> f32 {
    let period = f64::from(overflow * 0.5).max(3.0);
    let wave = (std::f64::consts::FRAC_PI_2 * (std::f64::consts::TAU * seconds / period).cos())
        .sin()
        / 2.0
        + 0.5;
    (wave * f64::from(overflow)) as f32
}

/// Draws a label too wide for its button as Java does: the text starts at the left of the button
/// and slides to the right to show the rest, then back, instead of being cut short. Returns false,
/// drawing nothing, for a label that fits or that Java's font cannot draw.
fn scrolling_label(
    canvas: &mut Canvas<'_>,
    label: &str,
    bounds: Bounds,
    unit: f32,
    color: [u8; 4],
) -> Result<bool, PaintError> {
    // Java keeps two pixels clear at each side of a button's text.
    let room = (bounds[2] - bounds[0]) / unit - 4.0;
    let Some(width) = crate::java::text_width(label).filter(|width| *width > room) else {
        return Ok(false);
    };
    let offset = scroll_offset(width - room, canvas.seconds).round();
    let top = (bounds[1] + ((bounds[3] - bounds[1]) - 8.0 * unit) * 0.5).round();
    let clip = canvas.begin_clip([
        bounds[0] + 2.0 * unit,
        bounds[1],
        bounds[2] - 2.0 * unit,
        bounds[3],
    ])?;
    let drawn = crate::java::draw_text(
        canvas,
        label,
        [bounds[0] + (2.0 - offset) * unit, top],
        unit,
        color,
    )?;
    canvas.end_clip(clip);
    if drawn && let Some(transitions) = canvas.transitions.as_deref_mut() {
        transitions.marquee_drawn = true;
    }
    Ok(drawn)
}

/// Draws `label` centred in `bounds` in Java's font, or in the client's own font when Java's
/// cannot draw all of it (a character its sheets lack, or no Java font installed).
pub(super) fn label_centred(
    canvas: &mut Canvas<'_>,
    label: &str,
    bounds: Bounds,
    unit: f32,
    color: [u8; 4],
) -> Result<(), PaintError> {
    if scrolling_label(canvas, label, bounds, unit, color)? {
        return Ok(());
    }
    let label = fitted(label, (bounds[2] - bounds[0]) / unit - 4.0);
    let label = label.as_str();
    if super::font::centred(canvas, label, bounds, unit, color, true)? {
        return Ok(());
    }
    let style = label_style(canvas, unit);
    canvas.text_centred(label, bounds, style, color, true)
}

/// Draws `sprite_name` over `bounds` as a nine-slice with the borders its `.mcmeta` declared.
pub(super) fn nine_slice(
    canvas: &mut Canvas<'_>,
    sprite_name: &str,
    bounds: Bounds,
    unit: f32,
) -> Result<bool, PaintError> {
    let key = key(sprite_name);
    // A sprite without borders is stretched whole.
    let slices = border(&key).unwrap_or([0; 4]);
    let widths = border_units(&key)
        .unwrap_or([0.0; 4])
        .map(|pixels| pixels * unit / canvas.rem.max(f32::EPSILON));
    canvas.nine_slice(&key, bounds, slices, widths, true, [255; 4])
}

/// Draws a button over `bounds`; with no action it is drawn disabled.
pub(super) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    bounds: Bounds,
    label: &str,
    action: Option<MenuAction>,
    unit: f32,
) -> Result<(), PaintError> {
    draw_lit(canvas, view, bounds, label, action, false, unit)
}

/// [`draw`] with the button drawn highlighted while `lit`, as a chosen tab is.
pub(super) fn draw_lit(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    bounds: Bounds,
    label: &str,
    action: Option<MenuAction>,
    lit: bool,
    unit: f32,
) -> Result<(), PaintError> {
    let state = canvas.interaction(view, action);
    let sprite = match (action, lit || state.hovered || state.focused) {
        (None, _) => DISABLED,
        (Some(_), true) => HOVER,
        (Some(_), false) => REST,
    };
    nine_slice(canvas, sprite, bounds, unit)?;
    let color = if action.is_some() {
        TEXT
    } else {
        DISABLED_TEXT
    };
    label_centred(canvas, label, bounds, unit, color)?;
    if let Some(action) = action {
        canvas.hit(action, bounds)?;
    }
    Ok(())
}

/// Draws a square button showing the sprite `icon_name` at its own size, centred, as Java's icon
/// buttons do. With no action it is drawn disabled.
pub(super) fn icon(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    bounds: Bounds,
    icon_name: &str,
    action: Option<MenuAction>,
    unit: f32,
) -> Result<(), PaintError> {
    let state = canvas.interaction(view, action);
    let sprite = match (action, state.hovered || state.focused) {
        (None, _) => DISABLED,
        (Some(_), true) => HOVER,
        (Some(_), false) => REST,
    };
    nine_slice(canvas, sprite, bounds, unit)?;
    let key = key(icon_name);
    if let Some([width, height]) = canvas.java_sprite_size(&key) {
        let (width, height) = (width * unit, height * unit);
        let left = ((bounds[0] + bounds[2] - width) * 0.5).round();
        let top = ((bounds[1] + bounds[3] - height) * 0.5).round();
        canvas.sprite(&key, [left, top, left + width, top + height], [255; 4])?;
    }
    if let Some(action) = action {
        canvas.hit(action, bounds)?;
    }
    Ok(())
}

#[cfg(test)]
mod scroll_tests {
    use super::*;

    #[test]
    fn a_long_label_sweeps_between_its_start_and_its_end_and_never_past_them() {
        let overflow = 20.0;
        let period = 10.0;
        assert!((scroll_offset(overflow, 0.0) - overflow).abs() < 0.01);
        assert!(scroll_offset(overflow, period / 2.0).abs() < 0.01);
        for step in 0..200 {
            let offset = scroll_offset(overflow, f64::from(step) * 0.173);
            assert!((0.0..=overflow).contains(&offset), "{offset}");
        }
        // A barely long label still takes its three seconds to sweep.
        assert!((scroll_offset(1.0, 0.0) - 1.0).abs() < 0.01);
        assert!(scroll_offset(1.0, 1.5).abs() < 0.01);
    }
}
