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
        (key("sprites/widget/button_highlighted.png"), sprite(HOVER_PAGE)),
        (key("sprites/widget/button_disabled.png"), sprite(DISABLED_PAGE)),
        (key("sprites/icon/language.png"), OreUiSprite { page: 9, bounds: [0, 0, 15, 15] }),
        (key("sprites/icon/accessibility.png"), OreUiSprite { page: 9, bounds: [16, 0, 31, 15] }),
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
        let (hits, _) = paint(true, size, |canvas| home::draw(canvas, &view, size).unwrap());
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
    let (_, nodes) = paint(true, size, |canvas| home::draw(canvas, &view, size).unwrap());
    let pages: Vec<u16> = nodes
        .iter()
        .filter_map(|node| match node.visual() {
            UiVisual::Sprite { texture_page, .. } => Some(*texture_page),
            _ => None,
        })
        .collect();
    // Nine pieces of each button's nine-slice: one hovered, the rest at rest, none disabled.
    assert_eq!(pages.iter().filter(|page| **page == HOVER_PAGE).count(), 9);
    assert!(pages.iter().filter(|page| **page == REST_PAGE).count() >= 9 * 9);
    assert!(!pages.contains(&DISABLED_PAGE));
}

#[test]
fn the_size_starts_at_javas_automatic_scale_and_the_gui_scale_lowers_it() {
    let physical = |size: [f32; 2]| size.map(|side| side as u32);
    for size in [[1280.0, 720.0], [1920.0, 1080.0], [640.0, 360.0], [2560.0, 1440.0], [3840.0, 2160.0]] {
        let auto = ui::gui_scale(physical(size), None) as f32;
        let java = (size[0] as u32 / 320).min(size[1] as u32 / 240).max(1) as f32;
        // The client's own automatic scale gives Java's automatic scale: the size of the game.
        assert_eq!(unit_for(size, auto, 1.0), java, "{size:?}");
        // Each step the GUI scale is lowered lowers the menu one step, down to one.
        for steps in 1..=4 {
            let lowered = (auto - steps as f32).max(1.0);
            assert_eq!(unit_for(size, lowered, 1.0), (java - (auto - lowered)).max(1.0), "{size:?}");
        }
        assert_eq!(unit_for(size, 1.0, 1.0).fract(), 0.0);
    }
    // A window smaller than 320 by 240 still draws at one.
    assert_eq!(unit_for([300.0, 200.0], 1.0, 1.0), 1.0);
}

#[test]
fn the_scale_counts_physical_pixels_at_other_display_scales() {
    // 2560 by 1440 physical at a display scale of 2 is a 1280 by 720 logical window; Java's
    // automatic scale is 6 physical pixels per Java pixel, which is 3 logical pixels.
    let auto = ui::gui_scale([2560, 1440], None) as f32;
    assert_eq!(unit_for([1280.0, 720.0], auto, 2.0), 3.0);
    assert_eq!((unit_for([1280.0, 720.0], auto - 1.0, 2.0) * 2.0).fract(), 0.0);
}

#[test]
fn the_java_title_follows_the_gui_scale_of_the_canvas() {
    // The test canvas draws at the client's GUI scale 2, its automatic scale at this window, which
    // is Java's automatic scale 3: a Java pixel is three logical pixels.
    let view = java_view();
    let size = [1280.0, 720.0];
    let (hits, _) = paint(true, size, |canvas| home::draw(canvas, &view, size).unwrap());
    let play = hits
        .iter()
        .find(|(action, _)| *action == MenuAction::Navigate(MenuScreen::Play))
        .unwrap()
        .1;
    assert_eq!(play.max().x() - play.min().x(), 200.0 * 3.0);
    assert_eq!(play.max().y() - play.min().y(), 20.0 * 3.0);
}
