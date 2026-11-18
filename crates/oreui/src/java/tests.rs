use std::{collections::HashMap, sync::Arc};

use launcher::menu::{
    MenuAction, MenuScreen, MenuView,
    settings_options::{SettingId, menu_style::MenuStyle},
    settings_sections::SettingSection,
};
use ui::UiVisual;
use ui_engine::{test_support::fixture_font, text_metrics::TextMetrics};

use super::*;
use crate::{
    images::{OreUiImages, OreUiSprite, java::key},
    paint::Originals,
};

/// Texture pages of the rest, highlighted and disabled button sprites, so a test can tell which
/// one a button drew.
const REST_PAGE: u16 = 1;
const HOVER_PAGE: u16 = 2;
const DISABLED_PAGE: u16 = 3;

fn originals() -> Originals {
    let sprite = |page| OreUiSprite {
        page,
        bounds: [0, 0, 200, 20],
    };
    let sprites = Arc::new(HashMap::from([
        (key("sprites/widget/button.png"), sprite(REST_PAGE)),
        (
            key("sprites/widget/button_highlighted.png"),
            sprite(HOVER_PAGE),
        ),
        (
            key("sprites/widget/button_disabled.png"),
            sprite(DISABLED_PAGE),
        ),
        (
            key("sprites/icon/language.png"),
            OreUiSprite {
                page: 9,
                bounds: [0, 0, 15, 15],
            },
        ),
        (
            key("sprites/icon/accessibility.png"),
            OreUiSprite {
                page: 9,
                bounds: [16, 0, 31, 15],
            },
        ),
    ]));
    Originals {
        page: 0,
        images: OreUiImages {
            pages: Vec::new(),
            sprites: sprites.clone(),
            loading_frames: Default::default(),
            animations: Default::default(),
        },
        masks: HashMap::new(),
        sprites,
        loading_frames: Default::default(),
        animations: Default::default(),
    }
}

fn java_view() -> MenuView {
    let mut view = MenuView::new(true, "Player".into());
    view.screen = MenuScreen::Home;
    let options = Arc::make_mut(&mut view.settings_options);
    assert!(options.set(SettingId::MenuStyle.index(), MenuStyle::Java.value()));
    view
}

/// Runs `draw` on a canvas, with the Java sprites installed or not.
fn paint(
    installed: bool,
    size: [f32; 2],
    draw: impl FnOnce(&mut Canvas<'_>),
) -> (Vec<(MenuAction, ui::UiRect)>, Vec<ui::UiNode>) {
    let originals = originals();
    let (mut nodes, mut next, mut layouts) =
        (Vec::new(), 1, ui::TextLayoutCache::new(128, 1024 * 1024));
    let font = fixture_font();
    let metrics = TextMetrics::for_viewport(
        size.map(|value| value as u32),
        ui::DpiScale::new(1.0).unwrap(),
        Some(2),
    );
    let mut canvas = Canvas::new(
        &mut nodes,
        &mut next,
        &mut layouts,
        &font,
        metrics,
        0,
        installed.then_some(&originals),
    );
    draw(&mut canvas);
    let hits = canvas.hits;
    (hits, nodes)
}

#[test]
fn java_screens_draw_only_when_chosen_and_installed() {
    let size = [1280.0, 720.0];
    let java = java_view();
    paint(true, size, |canvas| assert!(selected(&java, canvas)));
    paint(false, size, |canvas| assert!(!selected(&java, canvas)));
    let mut bedrock = java_view();
    let options = Arc::make_mut(&mut bedrock.settings_options);
    assert!(options.set(SettingId::MenuStyle.index(), MenuStyle::Bedrock.value()));
    paint(true, size, |canvas| assert!(!selected(&bedrock, canvas)));
}

#[test]
fn the_java_title_offers_every_home_action_inside_the_window() {
    let view = java_view();
    let toggle = view.settings_options.menu_style_switch();
    for size in [[1280.0, 720.0], [640.0, 360.0], [360.0, 240.0]] {
        let (hits, _) = paint(true, size, |canvas| {
            home::draw(canvas, &view, size).unwrap()
        });
        for action in [
            MenuAction::Navigate(MenuScreen::Play),
            MenuAction::Navigate(MenuScreen::Servers),
            MenuAction::Navigate(MenuScreen::Social),
            MenuAction::Navigate(MenuScreen::Settings),
            MenuAction::OpenExitDialog,
            MenuAction::Navigate(MenuScreen::DressingRoom),
            MenuAction::Navigate(MenuScreen::Friends),
            MenuAction::Navigate(MenuScreen::Inbox),
            MenuAction::StartSignIn,
            MenuAction::OpenSettingsSection(SettingSection::Language.definition().index),
            MenuAction::OpenSettingsSection(SettingSection::Accessibility.definition().index),
            MenuAction::SettingsOption(toggle.0, toggle.1),
        ] {
            let found = hits
                .iter()
                .find(|(candidate, _)| *candidate == action)
                .unwrap_or_else(|| panic!("{action:?} is missing at {size:?}"));
            let bounds = found.1;
            assert!(
                bounds.min().x() >= 0.0
                    && bounds.min().y() >= 0.0
                    && bounds.max().x() <= size[0]
                    && bounds.max().y() <= size[1],
                "{action:?} leaves the window at {size:?}"
            );
        }
    }
}

#[test]
fn a_hovered_button_draws_the_highlighted_sprite_and_the_rest_draw_at_rest() {
    let mut view = java_view();
    view.hovered = Some(MenuAction::Navigate(MenuScreen::Play));
    let size = [1280.0, 720.0];
    let (_, nodes) = paint(true, size, |canvas| {
        home::draw(canvas, &view, size).unwrap()
    });
    let pages: Vec<u16> = nodes
        .iter()
        .filter_map(|node| match node.visual() {
            UiVisual::Sprite { texture_page, .. } => Some(*texture_page),
            _ => None,
        })
        .collect();
    // With no borders declared a sprite is stretched whole, one piece per button: one hovered,
    // the nine others and the two icon buttons at rest, none disabled.
    assert_eq!(pages.iter().filter(|page| **page == HOVER_PAGE).count(), 1);
    assert!(pages.iter().filter(|page| **page == REST_PAGE).count() >= 10);
    assert!(!pages.contains(&DISABLED_PAGE));
}

#[test]
fn auto_is_the_largest_whole_number_that_fits_and_steps_at_javas_thresholds() {
    for (size, expected) in [
        ([1280.0, 720.0], 3.0),
        ([1920.0, 1080.0], 4.0),
        ([2560.0, 1440.0], 6.0),
        ([3840.0, 2160.0], 9.0),
        ([640.0, 360.0], 1.0),
        ([300.0, 200.0], 1.0),
    ] {
        assert_eq!(unit_for(size, None, 1.0), expected, "{size:?}");
    }
    // A title bar's few pixels cross a threshold, as in the game: that is Auto's own behaviour.
    assert_eq!(unit_for([2560.0, 1409.0], None, 1.0), 5.0);
    assert_eq!(unit_for([2560.0, 1440.0], None, 1.0), 6.0);
}

#[test]
fn a_fixed_scale_does_not_change_between_windowed_and_fullscreen() {
    // Windowed with a title bar and fullscreen, as in the owner's recording.
    for chosen in 1..=5 {
        assert_eq!(
            unit_for([2560.0, 1409.0], Some(chosen), 1.0),
            unit_for([2560.0, 1440.0], Some(chosen), 1.0),
            "scale {chosen}"
        );
        assert_eq!(
            unit_for([2560.0, 1440.0], Some(chosen), 1.0),
            f32::from(chosen)
        );
    }
}

#[test]
fn a_fixed_scale_drops_only_when_the_window_cannot_hold_it() {
    assert_eq!(unit_for([1280.0, 720.0], Some(3), 1.0), 3.0);
    assert_eq!(unit_for([1280.0, 720.0], Some(5), 1.0), 3.0);
    assert_eq!(unit_for([640.0, 360.0], Some(4), 1.0), 1.0);
    assert_eq!(unit_for([320.0, 240.0], Some(8), 1.0), 1.0);
}

#[test]
fn the_scale_counts_physical_pixels_at_other_display_scales() {
    // 2560 by 1440 physical at a display scale of 2 is a 1280 by 720 logical window: Auto is 6
    // physical pixels per Java pixel, which is 3 logical pixels, and a fixed 4 is 2 logical.
    assert_eq!(unit_for([1280.0, 720.0], None, 2.0), 3.0);
    assert_eq!(unit_for([1280.0, 720.0], Some(4), 2.0), 2.0);
    assert_eq!((unit_for([1280.0, 720.0], Some(5), 1.5) * 1.5).fract(), 0.0);
}

#[test]
fn the_java_title_is_drawn_at_the_chosen_java_gui_scale() {
    // Auto at this window is 3: a Java pixel is three logical pixels.
    let view = java_view();
    let size = [1280.0, 720.0];
    let (hits, _) = paint(true, size, |canvas| {
        home::draw(canvas, &view, size).unwrap()
    });
    let play = hits
        .iter()
        .find(|(action, _)| *action == MenuAction::Navigate(MenuScreen::Play))
        .unwrap()
        .1;
    assert_eq!(play.max().x() - play.min().x(), 200.0 * 3.0);
    assert_eq!(play.max().y() - play.min().y(), 20.0 * 3.0);
    // Choosing 2 draws it at 2, and the Bedrock GUI scale the canvas draws at does not matter.
    let mut view = java_view();
    let options = Arc::make_mut(&mut view.settings_options);
    assert!(options.set(SettingId::JavaGuiScale.index(), 2));
    let (hits, _) = paint(true, size, |canvas| {
        home::draw(canvas, &view, size).unwrap()
    });
    let play = hits
        .iter()
        .find(|(action, _)| *action == MenuAction::Navigate(MenuScreen::Play))
        .unwrap()
        .1;
    assert_eq!(play.max().x() - play.min().x(), 200.0 * 2.0);
}

#[test]
fn the_java_pause_menu_offers_every_pause_action_inside_the_window() {
    let view = java_view();
    let toggle = view.settings_options.menu_style_switch();
    for size in [[1280.0, 720.0], [640.0, 360.0], [360.0, 240.0]] {
        let (hits, _) = paint(true, size, |canvas| {
            pause::draw(canvas, &view, size).unwrap()
        });
        for action in [
            MenuAction::PauseResume,
            MenuAction::PauseSettings,
            MenuAction::PauseDisconnect,
            MenuAction::Navigate(MenuScreen::DressingRoom),
            MenuAction::SettingsOption(toggle.0, toggle.1),
        ] {
            let bounds = hits
                .iter()
                .find(|(candidate, _)| *candidate == action)
                .unwrap_or_else(|| panic!("{action:?} is missing at {size:?}"))
                .1;
            assert!(
                bounds.min().x() >= 0.0
                    && bounds.min().y() >= 0.0
                    && bounds.max().x() <= size[0]
                    && bounds.max().y() <= size[1],
                "{action:?} leaves the window at {size:?}"
            );
        }
    }
}

#[test]
fn the_java_pause_rows_sit_where_java_puts_them() {
    let view = java_view();
    let size = [1280.0, 720.0];
    let (hits, _) = paint(true, size, |canvas| {
        pause::draw(canvas, &view, size).unwrap()
    });
    let unit = unit_for(size, None, 1.0);
    let bounds = |action| hits.iter().find(|(found, _)| *found == action).unwrap().1;
    let resume = bounds(MenuAction::PauseResume);
    let quit = bounds(MenuAction::PauseDisconnect);
    let options = bounds(MenuAction::PauseSettings);
    let wardrobe = bounds(MenuAction::Navigate(MenuScreen::DressingRoom));
    let java_height = (size[1] / unit).ceil();
    // The first row is at a quarter of the height plus 8, each row 24 below the last.
    assert_eq!(resume.min().y(), ((java_height / 4.0).floor() + 8.0) * unit);
    assert_eq!(wardrobe.min().y() - resume.min().y(), 24.0 * unit);
    assert_eq!(quit.min().y() - wardrobe.min().y(), 24.0 * unit);
    assert_eq!(resume.width(), 204.0 * unit);
    assert_eq!(quit.width(), 204.0 * unit);
    assert_eq!(wardrobe.width(), 98.0 * unit);
    assert_eq!(options.width(), 98.0 * unit);
    assert_eq!(options.min().x() - wardrobe.max().x(), 8.0 * unit);
}
