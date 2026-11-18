//! The Java-style title screen: the Vantage logo over a column of 200-pixel buttons, with the
//! version at the bottom, as Java Edition lays its title out. It offers every action the Bedrock
//! home does: play, servers, realms, settings, quit, wardrobe, account, friends and inbox.

use launcher::menu::{MenuAction, MenuScreen, MenuView, auth::AuthState};
use ui::IconRef;
use ui_engine::engine::screen::PaintError;

use super::{button, unit};
use crate::{focus, paint::Canvas, theme::TEXT};

/// Width of a full-width button and of half of a pair, in Java pixels.
const WIDE: f32 = 200.0;
const HALF: f32 = 98.0;
const HEIGHT: f32 = 20.0;
const STEP: f32 = 24.0;
/// The logo's widest and tallest, in Java pixels.
const LOGO_WIDTH: f32 = 256.0;
const LOGO_HEIGHT: f32 = 64.0;

/// Draws the Java title with the update notice every home shows.
pub(in crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<(), PaintError> {
    draw_screen(canvas, view, size)?;
    crate::updates::notice(canvas, view, size)
}

fn draw_screen(canvas: &mut Canvas<'_>, view: &MenuView, size: [f32; 2]) -> Result<(), PaintError> {
    let unit = unit(size);
    let screen =
        canvas.begin_focus_region(focus::SCREEN, [0.0, 0.0, size[0], size[1]], None, true)?;
    canvas.focus_delegate(Some(MenuAction::Navigate(MenuScreen::Play)), None);
    let mid = size[0] * 0.5;
    let logo_bottom = logo(canvas, mid, unit * 24.0, unit)?;
    let mut y = logo_bottom + unit * 12.0;
    let wide = |y: f32| {
        [
            mid - WIDE * 0.5 * unit,
            y,
            mid + WIDE * 0.5 * unit,
            y + HEIGHT * unit,
        ]
    };
    let half = |y: f32, right: bool| {
        let left = if right {
            mid + 2.0 * unit
        } else {
            mid - 2.0 * unit - HALF * unit
        };
        [left, y, left + HALF * unit, y + HEIGHT * unit]
    };
    let go = |screen: MenuScreen| Some(MenuAction::Navigate(screen));
    for (label, action) in [
        ("Play", go(MenuScreen::Play)),
        ("Servers", go(MenuScreen::Servers)),
        ("Realms", go(MenuScreen::Social)),
    ] {
        button::draw(canvas, view, wide(y), label, action, unit)?;
        y += STEP * unit;
    }
    button::draw(canvas, view, half(y, false), "Settings", go(MenuScreen::Settings), unit)?;
    button::draw(
        canvas,
        view,
        half(y, true),
        "Quit Game",
        Some(MenuAction::OpenExitDialog),
        unit,
    )?;
    y += STEP * unit;
    let (account_label, account_action) = match view.auth_state {
        AuthState::SignedOut | AuthState::Failed(_) => ("Sign in", MenuAction::StartSignIn),
        AuthState::Checking => ("Signing in...", MenuAction::OpenAccounts),
        _ => ("Account", MenuAction::OpenAccounts),
    };
    button::draw(
        canvas,
        view,
        half(y, false),
        "Dressing Room",
        go(MenuScreen::DressingRoom),
        unit,
    )?;
    button::draw(canvas, view, half(y, true), account_label, Some(account_action), unit)?;
    y += STEP * unit;
    let friends = if view.friends.is_empty() {
        "Friends".to_owned()
    } else {
        format!("Friends ({})", view.friends.len())
    };
    let unread = view.feeds.home.inbox_unread;
    let inbox = if unread == 0 {
        "Inbox".to_owned()
    } else {
        format!(
            "Inbox ({}{})",
            unread.min(99),
            if unread > 99 { "+" } else { "" }
        )
    };
    button::draw(canvas, view, half(y, false), &friends, go(MenuScreen::Friends), unit)?;
    button::draw(canvas, view, half(y, true), &inbox, go(MenuScreen::Inbox), unit)?;
    footer(canvas, size, unit)?;
    style_toggle(canvas, view, size, unit)?;
    canvas.end_focus_region(screen);
    Ok(())
}

/// The Vantage logo, centred under `top`, fitted to its box; returns where it ends.
fn logo(canvas: &mut Canvas<'_>, mid: f32, top: f32, unit: f32) -> Result<f32, PaintError> {
    let Some(icon) = canvas.title_artwork else {
        let style = button::label_style(canvas, unit * 3.0);
        let bounds = [
            mid - LOGO_WIDTH * 0.5 * unit,
            top,
            mid + LOGO_WIDTH * 0.5 * unit,
            top + 40.0 * unit,
        ];
        canvas.text_centred(launcher::PRODUCT_NAME, bounds, style, TEXT, true)?;
        return Ok(top + 40.0 * unit);
    };
    let ratio = icon_ratio(icon);
    let width = (LOGO_HEIGHT * unit * ratio).min(LOGO_WIDTH * unit);
    let height = width / ratio;
    canvas.icon_ref(icon, [mid - width * 0.5, top, mid + width * 0.5, top + height])?;
    Ok(top + height)
}

fn icon_ratio(icon: IconRef) -> f32 {
    f32::from(icon.uv[2] - icon.uv[0]) / f32::from((icon.uv[3] - icon.uv[1]).max(1))
}

/// The product version at the bottom left.
fn footer(canvas: &mut Canvas<'_>, size: [f32; 2], unit: f32) -> Result<(), PaintError> {
    let y = size[1] - 10.0 * unit;
    let app = format!("{} {}", launcher::PRODUCT_NAME, launcher::PRODUCT_VERSION);
    if super::font::draw(canvas, &app, [2.0 * unit, y], unit, TEXT, true)?.is_some() {
        return Ok(());
    }
    let style = button::label_style(canvas, unit);
    canvas.text_line(&app, [2.0 * unit, y - unit], size[0] * 0.5, style, TEXT)?;
    Ok(())
}

/// The corner button that switches back to the Bedrock style, drawn as a Java button.
fn style_toggle(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
    unit: f32,
) -> Result<(), PaintError> {
    let options = &view.settings_options;
    let (index, value) = options.menu_style_switch();
    let label = format!("Menu style: {}", options.menu_style().label());
    let width = 120.0 * unit;
    let bounds = [
        size[0] - 2.0 * unit - width,
        2.0 * unit,
        size[0] - 2.0 * unit,
        2.0 * unit + HEIGHT * unit,
    ];
    button::draw(
        canvas,
        view,
        bounds,
        &label,
        Some(MenuAction::SettingsOption(index, value)),
        unit,
    )
}
