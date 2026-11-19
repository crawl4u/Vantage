//! The Java-style title screen, laid out as Java Edition lays its title out: the logo from 30
//! Java pixels down, three 200-pixel buttons from a quarter of the height plus 48, a row of
//! options and quit buttons with a language and an accessibility button beside them, and the
//! version at the bottom left. Everything Bedrock's home offers stays reachable: play, servers,
//! realms, mods, settings, quit, skin presets, account, friends and inbox. The Bedrock-only buttons sit in
//! the margins beside the column, so the column itself keeps Java's exact places.

use launcher::menu::{
    MenuAction, MenuScreen, MenuView, auth::AuthState, settings_sections::SettingSection,
};
use ui::IconRef;
use ui_engine::engine::screen::PaintError;

use super::{button, font, splash, unit};
use crate::{CharacterPreview, focus, paint::Canvas, theme::TEXT};

/// Sizes in Java pixels.
const WIDE: f32 = 200.0;
const HALF: f32 = 98.0;
const HEIGHT: f32 = 20.0;
const STEP: f32 = 24.0;
/// Where the logo starts, and the widest it is drawn.
const LOGO_TOP: f32 = 30.0;
const LOGO_WIDTH: f32 = 256.0;
/// The side buttons' width, and the gap between them and the window's edge.
const SIDE_WIDTH: f32 = 56.0;
const EDGE: f32 = 2.0;

/// A rectangle in Java pixels: left, top, right, bottom.
type Rect = [f32; 4];

/// Where every control of the Java title sits for a window `width` by `height` Java pixels. The
/// window is at least 320 by 240 Java pixels.
struct Layout {
    logo: Rect,
    wide: [Rect; 4],
    options: Rect,
    quit: Rect,
    language: Rect,
    accessibility: Rect,
    /// The Bedrock-only buttons stacked in the left margin.
    left: [Rect; 3],
    /// The Skin Presets button under the character.
    skins: Rect,
    /// Where the player's character stands, right of the column.
    character: Rect,
    toggle: Rect,
    version: [f32; 2],
    /// The splash text's pivot: 123 right of the middle and 69 down.
    splash: [f32; 2],
}

impl Layout {
    /// The game's layout: `j` is a quarter of the height plus 48, and the middle is the width's half.
    fn new(width: f32, height: f32, logo_ratio: f32) -> Self {
        let centre = (width / 2.0).floor();
        let j = (height / 4.0).floor() + 48.0;
        let rect = |x: f32, y: f32, w: f32, h: f32| [x, y, x + w, y + h];
        let logo_width = LOGO_WIDTH.min((j - LOGO_TOP - 4.0) * logo_ratio);
        let logo_height = logo_width / logo_ratio;
        // The fourth button pushes the row below it down by one step, as Java with a mod's button.
        let row = j + 96.0;
        let side = SIDE_WIDTH.min(centre - 100.0 - 2.0 * EDGE);
        let stage = centre + 110.0;
        let stage_width = (width - EDGE - stage).clamp(0.0, SIDE_WIDTH);
        Self {
            logo: rect(centre - logo_width / 2.0, LOGO_TOP, logo_width, logo_height),
            wide: [0.0, 1.0, 2.0, 3.0].map(|k| rect(centre - 100.0, j + STEP * k, WIDE, HEIGHT)),
            options: rect(centre - 100.0, row, HALF, HEIGHT),
            quit: rect(centre + 2.0, row, HALF, HEIGHT),
            language: rect(centre - 124.0, row, HEIGHT, HEIGHT),
            accessibility: rect(centre + 104.0, row, HEIGHT, HEIGHT),
            left: [0.0, 1.0, 2.0].map(|k| rect(EDGE, j + STEP * k, side, HEIGHT)),
            skins: rect(stage, j + 70.0, stage_width, HEIGHT),
            character: rect(stage, j - 4.0, stage_width, 70.0),
            toggle: rect(width - EDGE - 120.0, EDGE, 120.0, HEIGHT),
            version: [EDGE, height - 10.0],
            splash: [centre + 123.0, 69.0],
        }
    }

    /// The same rectangle in logical pixels.
    fn px(rect: Rect, unit: f32) -> Rect {
        rect.map(|value| value * unit)
    }
}

/// Draws the Java title with the update notice every home shows, and returns where the player's
/// character stands for the adapter to draw.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<Option<CharacterPreview>, PaintError> {
    let preview = draw_screen(canvas, view, size)?;
    crate::updates::notice(canvas, view, size)?;
    Ok(preview)
}

fn draw_screen(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<Option<CharacterPreview>, PaintError> {
    let unit = unit(view, canvas, size);
    let ratio = canvas.title_artwork.map_or(4.0, icon_ratio);
    let layout = Layout::new((size[0] / unit).ceil(), (size[1] / unit).ceil(), ratio);
    let px = |rect| Layout::px(rect, unit);
    let screen =
        canvas.begin_focus_region(focus::SCREEN, [0.0, 0.0, size[0], size[1]], None, true)?;
    canvas.focus_delegate(Some(MenuAction::Navigate(MenuScreen::Play)), None);
    logo(canvas, px(layout.logo), unit)?;
    splash::draw(
        canvas,
        [layout.splash[0] * unit, layout.splash[1] * unit],
        unit,
    )?;
    let go = |screen: MenuScreen| Some(MenuAction::Navigate(screen));
    for (rect, label, action) in [
        (layout.wide[0], "Singleplayer", go(MenuScreen::Play)),
        (layout.wide[1], "Multiplayer", go(MenuScreen::Servers)),
        (layout.wide[2], "Realms", go(MenuScreen::Social)),
        (
            layout.wide[3],
            "Mods",
            Some(MenuAction::OpenSettingsSection(
                SettingSection::Mods.definition().index,
            )),
        ),
        (layout.options, "Options", go(MenuScreen::Settings)),
        (layout.quit, "Quit Game", Some(MenuAction::OpenExitDialog)),
    ] {
        button::draw(canvas, view, px(rect), label, action, unit)?;
    }
    let section =
        |section: SettingSection| Some(MenuAction::OpenSettingsSection(section.definition().index));
    button::icon(
        canvas,
        view,
        px(layout.language),
        "sprites/icon/language.png",
        section(SettingSection::Language),
        unit,
    )?;
    button::icon(
        canvas,
        view,
        px(layout.accessibility),
        "sprites/icon/accessibility.png",
        section(SettingSection::Accessibility),
        unit,
    )?;
    sides(canvas, view, &layout, unit)?;
    footer(
        canvas,
        [layout.version[0] * unit, layout.version[1] * unit],
        size,
        unit,
    )?;
    button_toggle(canvas, view, px(layout.toggle), unit)?;
    canvas.end_focus_region(screen);
    let stage = px(layout.character);
    Ok(
        (stage[2] - stage[0] > 8.0 * unit).then_some(CharacterPreview {
            control: stage,
            clip: stage,
        }),
    )
}

/// The Bedrock-only buttons in the margins: friends, account and inbox on the left, and the skin
/// presets under the character on the right. A label with a count is used when it fits the button, else the bare label.
fn sides(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    layout: &Layout,
    unit: f32,
) -> Result<(), PaintError> {
    let width = layout.left[0][2] - layout.left[0][0] - 6.0;
    let friends = fit(
        &[
            format!("Friends ({})", view.friends.len()),
            "Friends".to_owned(),
        ],
        view.friends.is_empty(),
        width,
    );
    let unread = view.feeds.home.inbox_unread;
    let inbox = fit(
        &[
            format!(
                "Inbox ({}{})",
                unread.min(99),
                if unread > 99 { "+" } else { "" }
            ),
            "Inbox".to_owned(),
        ],
        unread == 0,
        width,
    );
    let (account, account_action) = match view.auth_state {
        AuthState::SignedOut | AuthState::Failed(_) => ("Sign in", MenuAction::StartSignIn),
        _ => ("Account", MenuAction::OpenAccounts),
    };
    let screen = |screen| Some(MenuAction::Navigate(screen));
    for (rect, label, action) in [
        (layout.left[0], friends, screen(MenuScreen::Friends)),
        (layout.left[1], account.to_owned(), Some(account_action)),
        (layout.left[2], inbox, screen(MenuScreen::Inbox)),
        (
            layout.skins,
            fit(
                &["Skin Presets".to_owned(), "Skins".to_owned()],
                false,
                layout.skins[2] - layout.skins[0] - 6.0,
            ),
            screen(MenuScreen::DressingRoom),
        ),
    ] {
        button::draw(canvas, view, Layout::px(rect, unit), &label, action, unit)?;
    }
    Ok(())
}

/// The first label of `candidates` that is no wider than `max` Java pixels in Java's font; the
/// last when none is, and the last at once when `bare` says the first adds nothing.
pub(super) fn fit(candidates: &[String], bare: bool, max: f32) -> String {
    let last = candidates.last().cloned().unwrap_or_default();
    if bare {
        return last;
    }
    let Some(java) = crate::images::java::font() else {
        return candidates.first().cloned().unwrap_or(last);
    };
    candidates
        .iter()
        .find(|label| font::width_with(&java, label).is_some_and(|width| width <= max))
        .cloned()
        .unwrap_or(last)
}

/// The Vantage logo over `bounds`, or its name in text when the artwork is missing.
fn logo(canvas: &mut Canvas<'_>, bounds: Rect, unit: f32) -> Result<(), PaintError> {
    match canvas.title_artwork {
        Some(icon) => canvas.icon_ref(icon, bounds),
        None => {
            let style = button::label_style(canvas, unit * 3.0);
            canvas.text_centred(launcher::PRODUCT_NAME, bounds, style, TEXT, true)
        }
    }
}

fn icon_ratio(icon: IconRef) -> f32 {
    f32::from(icon.uv[2] - icon.uv[0]) / f32::from((icon.uv[3] - icon.uv[1]).max(1))
}

/// The product version with its top left corner at `at`.
fn footer(
    canvas: &mut Canvas<'_>,
    at: [f32; 2],
    size: [f32; 2],
    unit: f32,
) -> Result<(), PaintError> {
    let app = format!("{} {}", launcher::PRODUCT_NAME, launcher::PRODUCT_VERSION);
    if font::draw(canvas, &app, at, unit, TEXT, true)?.is_some() {
        return Ok(());
    }
    let style = button::label_style(canvas, unit);
    canvas.text_line(&app, [at[0], at[1] - unit], size[0] * 0.5, style, TEXT)?;
    Ok(())
}

/// The corner button that switches back to the Bedrock style, drawn as a Java button.
pub(super) fn button_toggle(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    bounds: Rect,
    unit: f32,
) -> Result<(), PaintError> {
    let options = &view.settings_options;
    let (index, value) = options.menu_style_switch();
    let label = format!("Menu style: {}", options.menu_style().label());
    button::draw(
        canvas,
        view,
        bounds,
        &label,
        Some(MenuAction::SettingsOption(index, value)),
        unit,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_column_sits_where_java_puts_it() {
        // At 320 by 240 Java pixels: j = 240 / 4 + 48 = 108, the middle is 160.
        let layout = Layout::new(320.0, 240.0, 4.0);
        assert_eq!(layout.wide[0], [60.0, 108.0, 260.0, 128.0]);
        assert_eq!(layout.wide[1], [60.0, 132.0, 260.0, 152.0]);
        assert_eq!(layout.wide[2], [60.0, 156.0, 260.0, 176.0]);
        assert_eq!(layout.wide[3], [60.0, 180.0, 260.0, 200.0]);
        assert_eq!(layout.options, [60.0, 204.0, 158.0, 224.0]);
        assert_eq!(layout.quit, [162.0, 204.0, 260.0, 224.0]);
        assert_eq!(layout.language, [36.0, 204.0, 56.0, 224.0]);
        assert_eq!(layout.accessibility, [264.0, 204.0, 284.0, 224.0]);
        assert_eq!(layout.version, [2.0, 230.0]);
        assert_eq!(layout.splash, [283.0, 69.0]);
    }

    #[test]
    fn an_odd_width_puts_the_middle_at_its_floor() {
        let layout = Layout::new(427.0, 240.0, 4.0);
        assert_eq!(layout.wide[0][0], 213.0 - 100.0);
    }

    #[test]
    fn the_logo_starts_at_thirty_and_never_reaches_the_first_button() {
        for (width, height) in [
            (320.0, 240.0),
            (427.0, 240.0),
            (640.0, 360.0),
            (320.0, 400.0),
        ] {
            for ratio in [2.0, 3.6, 6.0] {
                let layout = Layout::new(width, height, ratio);
                assert_eq!(layout.logo[1], 30.0);
                assert!(
                    layout.logo[3] < layout.wide[0][1],
                    "{width}x{height} at {ratio}"
                );
                assert!(layout.logo[2] - layout.logo[0] <= LOGO_WIDTH);
            }
        }
    }

    #[test]
    fn the_side_buttons_fit_in_the_margins_even_at_the_narrowest_window() {
        let layout = Layout::new(320.0, 240.0, 4.0);
        assert!(layout.left[0][2] <= layout.wide[0][0] - EDGE);
        assert!(layout.character[0] >= layout.wide[0][2] + EDGE);
        assert!(layout.left[0][0] >= 0.0 && layout.skins[2] <= 320.0);
        assert!(layout.skins[0] >= layout.wide[0][2] + EDGE);
        // They stay clear of the language and accessibility buttons below them too.
        assert!(layout.left[2][3] < layout.language[1]);
        // The character and its button stay above the accessibility button.
        assert!(layout.skins[3] < layout.accessibility[1]);
    }
}
