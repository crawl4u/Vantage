//! The Java-style menus: the same screens and actions as the Bedrock look, drawn with sprites
//! read from the player's own Java Edition install (see `images::java`). A Java screen draws
//! only when the player chose the Java style and those sprites are installed; otherwise the
//! Bedrock screen is drawn, so nothing is ever left without a menu.

mod button;
mod font;
pub(super) mod home;
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
/// pixels per logical pixel, when the player's GUI scale is `gui_scale` physical pixels per GUI
/// pixel.
///
/// Java's own scale is the starting point: Auto is the largest whole number at which [`FIT`] still
/// fits, so at the same window the screen is as large as in the game. The player's GUI scale then
/// lowers it, one step for every step it is below the client's own automatic scale, so the setting
/// that sizes the Bedrock menus sizes these too. The result is a whole number of physical pixels,
/// so texels stay crisp, and never less than one.
pub(super) fn unit_for(size: [f32; 2], gui_scale: f32, dpi: f32) -> f32 {
    let physical = size.map(|side| (side * dpi).round() as u32);
    let java_auto = (physical[0] / FIT[0] as u32).min(physical[1] / FIT[1] as u32).max(1) as f32;
    let client_auto = ui::gui_scale(physical, None) as f32;
    let steps_down = (client_auto - gui_scale.floor()).max(0.0);
    (java_auto - steps_down).max(1.0) / dpi
}

/// [`unit_for`] with the GUI scale and display scale the canvas is drawing at.
pub(super) fn unit(canvas: &Canvas<'_>, size: [f32; 2]) -> f32 {
    unit_for(size, canvas.metrics.gui_scale, canvas.metrics.dpi_scale.get())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod font_tests;
