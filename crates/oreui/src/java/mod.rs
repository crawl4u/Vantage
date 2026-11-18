//! The Java-style menus: the same screens and actions as the Bedrock look, drawn with sprites
//! read from the player's own Java Edition install (see `images::java`). A Java screen draws
//! only when the player chose the Java style and those sprites are installed; otherwise the
//! Bedrock screen is drawn, so nothing is ever left without a menu.

mod button;
mod font;
pub(super) mod frame;
pub(crate) use button::label_style;
pub(super) mod home;
pub(super) mod options;
pub(crate) mod pack_drag;
mod packs;
pub(super) mod pause;
pub(super) mod servers;
mod shaders;
mod splash;

use launcher::menu::{MenuView, settings_options::menu_style::MenuStyle};

use super::paint::Canvas;

/// Sprite of a button at rest; its presence says the Java sprites are installed.
const BUTTON: &str = "sprites/widget/button.png";

/// Whether the Java style is chosen and its sprites are installed, so Java screens draw.
pub(super) fn selected(view: &MenuView, canvas: &Canvas<'_>) -> bool {
    view.settings_options.menu_style() == MenuStyle::Java
        && canvas.has_sprite(&super::images::java::key(BUTTON))
}

/// Java pixels a screen needs across and down at least, so it fits whole.
const FIT: [f32; 2] = [320.0, 240.0];

/// Logical pixels in one Java GUI pixel for a `size` window of logical pixels, at `dpi` physical
/// pixels per logical pixel, when the player chose `scale` physical pixels per Java pixel (none
/// for Auto).
///
/// This is Java's own rule. Auto is the largest whole number at which [`FIT`] still fits. A fixed
/// number is kept as it is, whatever the window, and only dropped to the largest that fits when
/// the window is too small for it, so it does not change between windowed and fullscreen. The
/// result is a whole number of physical pixels, so texels stay crisp, and never less than one.
pub(super) fn unit_for(size: [f32; 2], scale: Option<u8>, dpi: f32) -> f32 {
    let physical = size.map(|side| (side * dpi).round() as u32);
    let fit = (physical[0] / FIT[0] as u32)
        .min(physical[1] / FIT[1] as u32)
        .max(1);
    scale.map_or(fit, |chosen| u32::from(chosen).clamp(1, fit)) as f32 / dpi
}

/// [`unit_for`] with the view's Java GUI scale and the display scale the canvas is drawing at.
pub(super) fn unit(view: &MenuView, canvas: &Canvas<'_>, size: [f32; 2]) -> f32 {
    unit_for(
        size,
        view.settings_options.java_gui_scale(),
        canvas.metrics.dpi_scale.get(),
    )
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod font_tests;
