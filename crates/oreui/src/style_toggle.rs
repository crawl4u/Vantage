//! The corner button that switches the menus between the Bedrock and Java styles, on the home
//! and pause screens so it works from the start menu and in game.

use launcher::menu::{MenuAction, MenuView};
use ui_engine::engine::screen::PaintError;

use super::{
    paint::Canvas,
    widgets::{self, Variant},
};

/// Draws the toggle in the top right corner of a `size` screen.
pub(super) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<(), PaintError> {
    let options = &view.settings_options;
    let (index, value) = options.menu_style_switch();
    let label = format!("Menu style: {}", options.menu_style().label());
    let margin = canvas.r(1.2);
    let width = canvas.r(19.0).min(size[0] * 0.5);
    let height = canvas.r(3.6);
    let bounds = [
        size[0] - margin - width,
        margin,
        size[0] - margin,
        margin + height,
    ];
    widgets::button(
        canvas,
        view,
        bounds,
        Variant::Neutral,
        &label,
        Some(MenuAction::SettingsOption(index, value)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review_tests::paint;
    use launcher::menu::settings_options::{SettingId, menu_style::MenuStyle};
    use std::sync::Arc;

    fn toggle_hit(view: &MenuView, size: [f32; 2]) -> (MenuAction, [f32; 4]) {
        let (_, hits, _) = paint(Default::default(), |c| draw(c, view, size).unwrap());
        let (action, bounds) = hits
            .into_iter()
            .find(|(action, _)| matches!(action, MenuAction::SettingsOption(..)))
            .expect("the style toggle is drawn");
        (
            action,
            [
                bounds.min().x(),
                bounds.min().y(),
                bounds.max().x(),
                bounds.max().y(),
            ],
        )
    }

    #[test]
    fn the_toggle_switches_to_the_other_style_and_stays_on_screen() {
        let size = [1280.0, 720.0];
        let mut view = MenuView::new(true, "Fixture".into());
        let index = SettingId::MenuStyle.index() as u16;
        let (action, bounds) = toggle_hit(&view, size);
        assert_eq!(
            action,
            MenuAction::SettingsOption(index, MenuStyle::Java.value())
        );
        assert!(bounds[0] >= 0.0 && bounds[2] <= size[0] && bounds[1] >= 0.0 && bounds[3] <= size[1]);

        let options = Arc::make_mut(&mut view.settings_options);
        assert!(options.set(index.into(), MenuStyle::Java.value()));
        let (action, _) = toggle_hit(&view, size);
        assert_eq!(
            action,
            MenuAction::SettingsOption(index, MenuStyle::Bedrock.value())
        );
    }
}
