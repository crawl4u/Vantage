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

/// Logical pixels in one Java GUI pixel for a `size` window: the largest whole number at which
/// [`FIT`] still fits, as the game's automatic GUI scale picks, so texels stay crisp.
pub(super) fn unit(size: [f32; 2]) -> f32 {
    (size[0] / FIT[0]).min(size[1] / FIT[1]).floor().clamp(1.0, 6.0)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod font_tests;
