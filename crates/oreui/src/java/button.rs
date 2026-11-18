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

/// Draws `label` centred in `bounds` in Java's font, or in the client's own font when Java's
/// cannot draw all of it (a character its sheets lack, or no Java font installed).
pub(super) fn label_centred(
    canvas: &mut Canvas<'_>,
    label: &str,
    bounds: Bounds,
    unit: f32,
    color: [u8; 4],
) -> Result<(), PaintError> {
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
    let slices = border(&key).unwrap_or([3; 4]);
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
    let color = if action.is_some() { TEXT } else { DISABLED_TEXT };
    label_centred(canvas, label, bounds, unit, color)?;
    if let Some(action) = action {
        canvas.hit(action, bounds)?;
    }
    Ok(())
}
