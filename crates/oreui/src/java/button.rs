//! A Java-style button: the widget sprite stretched as a nine-slice, white label with shadow.

use launcher::menu::{MenuAction, MenuView};
use ui_engine::engine::screen::PaintError;

use crate::{
    images::java::{border, key},
    paint::{Bounds, Canvas},
    theme::{BODY, TEXT, Type},
};

const REST: &str = "sprites/widget/button.png";
const HOVER: &str = "sprites/widget/button_highlighted.png";
const DISABLED: &str = "sprites/widget/button_disabled.png";
/// Label colour of a button that cannot be used.
const DISABLED_TEXT: [u8; 4] = [160, 160, 160, 255];

/// The label style at `unit` logical pixels per Java pixel: Java text is 8 pixels tall.
pub(super) fn label_style(canvas: &Canvas<'_>, unit: f32) -> Type {
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
fn fitted(label: &str, max: f32) -> String {
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

/// Draws `label` centred in `bounds` in Java's font, or in the client's own font when Java's
/// cannot draw all of it (a character its sheets lack, or no Java font installed).
pub(super) fn label_centred(
    canvas: &mut Canvas<'_>,
    label: &str,
    bounds: Bounds,
    unit: f32,
    color: [u8; 4],
) -> Result<(), PaintError> {
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
    let widths = slices.map(|texels| f32::from(texels) * unit / canvas.rem.max(f32::EPSILON));
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
    let state = canvas.interaction(view, action);
    let sprite = match (action, state.hovered || state.focused) {
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
    if let Some([width, height]) = canvas.sprite_size(&key) {
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
