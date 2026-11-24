//! The Java-style Add Server screen, laid out as Java Edition's Edit Server Info screen: the title
//! near the top, a labelled name field and a labelled address field in a 200-pixel column, and the
//! buttons at the bottom. The port has a field of its own because the Bedrock servers this client
//! joins carry one. Done saves the server, Save & Join also connects, Cancel goes back.

use launcher::menu::{MenuAction, MenuView};
use ui_engine::engine::screen::PaintError;

use super::frame::{self, Btn, Spec, put};
use crate::{paint::Canvas, widgets};

/// Sizes in Java pixels.
const COLUMN: f32 = 200.0;
const FIELD_HEIGHT: f32 = 20.0;
const TITLE_TOP: f32 = 17.0;
/// Where each label sits and the field under it, as Java places them.
const NAME_LABEL: f32 = 53.0;
const NAME_FIELD: f32 = 66.0;
const ADDRESS_LABEL: f32 = 94.0;
const ADDRESS_FIELD: f32 = 106.0;
const PORT_LABEL: f32 = 134.0;
const PORT_FIELD: f32 = 146.0;
const PORT_WIDTH: f32 = 60.0;
const MESSAGE_TOP: f32 = 174.0;
const GREY: [u8; 4] = [160, 160, 160, 255];
const RED: [u8; 4] = [255, 85, 85, 255];

/// Whether the draft can be saved: it has a name and an address.
fn ready(view: &MenuView) -> bool {
    !view.name.trim().is_empty() && !view.address.trim().is_empty()
}

/// The footer's buttons: Save and Save & Join wait for a name and an address.
fn footer(view: &MenuView) -> (Vec<Btn<'static>>, Vec<Btn<'static>>) {
    let save = ready(view);
    (
        vec![
            Btn::new("Done", save.then_some(MenuAction::AddSave)),
            Btn::new("Save & Join", save.then_some(MenuAction::AddSaveConnect)),
        ],
        vec![Btn::new("Cancel", Some(MenuAction::AddBack))],
    )
}

/// Draws the Java Add Server (or Edit Server) screen.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<(), PaintError> {
    let (first, second) = footer(view);
    let spec = Spec {
        title: "Edit Server Info",
        subtitle: None,
        header: frame::HEADER,
        rows: &[&first, &second],
        more: None,
        title_top: Some(TITLE_TOP),
        footer_gap: None,
    };
    frame::draw(canvas, view, size, &spec, |canvas, frame| {
        let unit = frame.unit;
        let width = COLUMN.min(frame.centre * 2.0 - 24.0);
        let left = frame.centre - width / 2.0;
        let label = |canvas: &mut Canvas<'_>, text: &str, top: f32| {
            put(
                canvas,
                text,
                [left * unit, top * unit],
                unit,
                GREY,
                width * unit,
            )
        };
        let field = |canvas: &mut Canvas<'_>,
                     top: f32,
                     span: f32,
                     value: &str,
                     placeholder: &str,
                     action: MenuAction| {
            widgets::java_field(
                canvas,
                view,
                [left, top, left + span, top + FIELD_HEIGHT].map(|v| v * unit),
                value,
                placeholder,
                view.field == action.text_field(),
                Some(action),
                unit,
            )
        };
        label(canvas, "Server Name", NAME_LABEL)?;
        field(
            canvas,
            NAME_FIELD,
            width,
            &view.name,
            "My server",
            MenuAction::AddName,
        )?;
        label(canvas, "Server Address", ADDRESS_LABEL)?;
        field(
            canvas,
            ADDRESS_FIELD,
            width,
            &view.address,
            "Hostname or IP address",
            MenuAction::AddAddress,
        )?;
        label(canvas, "Port", PORT_LABEL)?;
        field(
            canvas,
            PORT_FIELD,
            PORT_WIDTH.min(width),
            &view.port,
            "Default",
            MenuAction::AddPort,
        )?;
        if let Some(message) = view.message.as_deref() {
            put(
                canvas,
                message,
                [left * unit, MESSAGE_TOP * unit],
                unit,
                RED,
                width * unit,
            )?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labelled(buttons: &(Vec<Btn<'static>>, Vec<Btn<'static>>), label: &str) -> Option<MenuAction> {
        buttons
            .0
            .iter()
            .chain(&buttons.1)
            .find(|button| button.label == label)
            .and_then(|button| button.action)
    }

    #[test]
    fn saving_waits_for_a_name_and_an_address_and_cancel_always_goes_back() {
        let mut view = MenuView::new(true, "Player".into());
        let empty = footer(&view);
        assert!(labelled(&empty, "Done").is_none() && labelled(&empty, "Save & Join").is_none());
        assert_eq!(labelled(&empty, "Cancel"), Some(MenuAction::AddBack));
        view.name = "Mine".to_owned();
        assert!(labelled(&footer(&view), "Done").is_none());
        view.address = "play.example.test".to_owned();
        let filled = footer(&view);
        assert_eq!(labelled(&filled, "Done"), Some(MenuAction::AddSave));
        assert_eq!(labelled(&filled, "Save & Join"), Some(MenuAction::AddSaveConnect));
    }
}
