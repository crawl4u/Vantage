//! The Java-style pause menu, laid out as Java Edition lays its game menu out: the title 40 Java
//! pixels down and the buttons in 24-pixel rows from a quarter of the height plus 8, the first and
//! last 204 pixels wide and the rows between in pairs 98 pixels wide or four 48 wide. Java's rows
//! for advancements, statistics and the icon links hold what this client has instead: achievements,
//! friends and inbox, then account, mods, Discord and updates, with options beside world options. The
//! wardrobe, which Java has no place for, sits in the left margin as on the title. The world shows
//! dimmed behind it.

use launcher::menu::{MenuAction, MenuScreen, MenuView, settings_sections::SettingSection};
use ui_engine::engine::screen::PaintError;

use super::{button, home, unit};
use crate::{focus, paint::Canvas, theme::TEXT};

/// Sizes in Java pixels.
const FULL: f32 = 204.0;
const HALF: f32 = 98.0;
const HEIGHT: f32 = 20.0;
const STEP: f32 = 24.0;
/// Space between the two buttons of a pair, and between those of the row of four.
const PAIR_GAP: f32 = 8.0;
const FOUR_GAP: f32 = 4.0;
const QUARTER: f32 = 48.0;
/// The wardrobe button's width and its distance from the window's edge.
const SIDE_WIDTH: f32 = 56.0;
const EDGE: f32 = 2.0;
/// Where the title's text starts, and how tall Java's text is.
const TITLE_TOP: f32 = 40.0;
const TEXT_HEIGHT: f32 = 8.0;
/// The darkening of the world behind the menu, as Java's pause background.
const DIM: [u8; 4] = [16, 16, 16, 200];

type Rect = [f32; 4];

/// Where every control sits for a window `width` by `height` Java pixels.
struct Layout {
    title: Rect,
    resume: Rect,
    achievements: Rect,
    friends: Rect,
    inbox: Rect,
    links: [Rect; 4],
    options: Rect,
    world: Rect,
    quit: Rect,
    wardrobe: Rect,
    toggle: Rect,
}

impl Layout {
    fn new(width: f32, height: f32) -> Self {
        let centre = (width / 2.0).floor();
        let top = (height / 4.0).floor() + 8.0;
        let rect = |x: f32, row: f32, w: f32| {
            let y = top + STEP * row;
            [x, y, x + w, y + HEIGHT]
        };
        let left = centre - FULL / 2.0;
        let pair = |row: f32| {
            (
                rect(centre - PAIR_GAP / 2.0 - HALF, row, HALF),
                rect(centre + PAIR_GAP / 2.0, row, HALF),
            )
        };
        let (friends, inbox) = pair(2.0);
        let (options, world) = pair(4.0);
        let side = SIDE_WIDTH.min(left - 2.0 * EDGE);
        Self {
            title: [0.0, TITLE_TOP, width, TITLE_TOP + TEXT_HEIGHT],
            resume: rect(left, 0.0, FULL),
            achievements: rect(left, 1.0, FULL),
            friends,
            inbox,
            links: [0.0, 1.0, 2.0, 3.0]
                .map(|k| rect(left + k * (QUARTER + FOUR_GAP), 3.0, QUARTER)),
            options,
            world,
            quit: rect(left, 5.0, FULL),
            wardrobe: rect(EDGE, 1.0, side),
            toggle: [width - 2.0 - 120.0, 2.0, width - 2.0, 2.0 + HEIGHT],
        }
    }
}

/// Draws the Java pause menu.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<(), PaintError> {
    let unit = unit(view, canvas, size);
    let layout = Layout::new((size[0] / unit).ceil(), (size[1] / unit).ceil());
    let px = |rect: Rect| rect.map(|value| value * unit);
    canvas.fill([0.0, 0.0, size[0], size[1]], DIM)?;
    let screen =
        canvas.begin_focus_region(focus::SCREEN, [0.0, 0.0, size[0], size[1]], None, true)?;
    canvas.focus_delegate(Some(MenuAction::PauseResume), None);
    button::label_centred(canvas, "Game Menu", px(layout.title), unit, TEXT)?;
    let go = MenuAction::Navigate;
    let section =
        |section: SettingSection| MenuAction::OpenSettingsSection(section.definition().index);
    // Accounts cannot be switched during a session, so the button opens the Account settings.
    // The world's own settings exist for the world this client hosts; any other world has the
    // connected server's.
    let (world_label, world) = if view.settings_section_shown(SettingSection::Game) {
        ("World Options...", SettingSection::Game)
    } else {
        ("Server Options...", SettingSection::Server)
    };
    for (rect, label, action) in [
        (layout.resume, "Back to Game", MenuAction::PauseResume),
        (layout.achievements, "Achievements", go(MenuScreen::Profile)),
        (layout.friends, "Friends", go(MenuScreen::Friends)),
        (layout.inbox, "Inbox", go(MenuScreen::Inbox)),
        (layout.links[0], "Account", section(SettingSection::Account)),
        (layout.links[1], "Mods", section(SettingSection::Mods)),
        (layout.links[2], "Discord", section(SettingSection::Discord)),
        (layout.links[3], "Updates", section(SettingSection::Updates)),
        (layout.options, "Options...", MenuAction::PauseSettings),
        (layout.world, world_label, section(world)),
        (
            layout.quit,
            "Save and Quit to Title",
            MenuAction::PauseDisconnect,
        ),
        (layout.wardrobe, "Wardrobe", go(MenuScreen::DressingRoom)),
    ] {
        button::draw(canvas, view, px(rect), label, Some(action), unit)?;
    }
    home::button_toggle(canvas, view, px(layout.toggle), unit)?;
    canvas.end_focus_region(screen);
    Ok(())
}
