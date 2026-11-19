//! A Java-style message over the screen, as Java Edition's confirm and progress screens show one:
//! the screen dimmed, a title and the message in lines under it, and the answers in a row of 150
//! pixel buttons (stacked 200 pixels wide when there are more than two). A message with no
//! answers, such as "Importing world...", stands alone in the middle.

use ui_engine::engine::screen::PaintError;

use super::{
    button,
    frame::{Btn, wrap_lines},
    unit,
};
use crate::{focus, modal::Modal, paint::Canvas, theme::TEXT};

/// Sizes in Java pixels.
const TITLE_TOP: f32 = 70.0;
const LINE: f32 = 9.0;
const TEXT_WIDTH: f32 = 300.0;
const PAIR_WIDTH: f32 = 150.0;
const STACK_WIDTH: f32 = 200.0;
const BUTTON_HEIGHT: f32 = 20.0;
const STACK_STEP: f32 = 24.0;
const GAP: f32 = 5.0;
const DIM: [u8; 4] = [0, 0, 0, 200];
const MESSAGE: [u8; 4] = [200, 200, 200, 255];

/// Where each of `count` buttons sits when the message ends at `bottom`, in Java pixels from the
/// centre: left, top and width.
fn button_places(count: usize, bottom: f32) -> Vec<[f32; 3]> {
    let top = bottom + 12.0;
    match count {
        0 => Vec::new(),
        1 => vec![[-STACK_WIDTH / 2.0, top, STACK_WIDTH]],
        2 => vec![
            [-PAIR_WIDTH - GAP / 2.0, top, PAIR_WIDTH],
            [GAP / 2.0, top, PAIR_WIDTH],
        ],
        _ => (0..count)
            .map(|index| {
                [
                    -STACK_WIDTH / 2.0,
                    top + index as f32 * STACK_STEP,
                    STACK_WIDTH,
                ]
            })
            .collect(),
    }
}

/// Draws `modal` in the Java style over what is on the canvas. Only its own buttons take presses.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &launcher::menu::MenuView,
    size: [f32; 2],
    modal: &Modal<'_>,
) -> Result<(), PaintError> {
    let unit = unit(view, canvas, size);
    let java_width = (size[0] / unit).ceil();
    let java_height = size[1] / unit;
    let centre = (java_width / 2.0).floor();
    canvas.hits.clear();
    canvas.clear_focus_geometry();
    canvas.fill([0.0, 0.0, size[0], size[1]], DIM)?;
    let region =
        canvas.begin_focus_region(focus::PICKER, [0.0, 0.0, size[0], size[1]], None, true)?;
    canvas.focus_trap();
    canvas.focus_delegate(modal.close, None);
    let lines = wrap_lines(&modal.body, TEXT_WIDTH.min(java_width - 20.0));
    // A message with nothing to answer sits in the middle of the screen.
    let top = if modal.buttons.is_empty() {
        java_height / 2.0 - 4.0
    } else {
        TITLE_TOP
    };
    let line = |canvas: &mut Canvas<'_>, text: &str, y: f32, colour: [u8; 4]| {
        button::label_centred(
            canvas,
            text,
            [0.0, y * unit, size[0], (y + 8.0) * unit],
            unit,
            colour,
        )
    };
    line(canvas, modal.title, top, TEXT)?;
    let mut bottom = top + LINE;
    for text in &lines {
        bottom += LINE;
        line(canvas, text, bottom - 2.0, MESSAGE)?;
    }
    let buttons: Vec<Btn<'_>> = modal
        .buttons
        .iter()
        .map(|(label, _, action)| Btn::new(label, *action))
        .collect();
    for (item, [left, y, width]) in buttons.iter().zip(button_places(buttons.len(), bottom)) {
        let bounds = [
            (centre + left) * unit,
            y * unit,
            (centre + left + width) * unit,
            (y + BUTTON_HEIGHT) * unit,
        ];
        button::draw(canvas, view, bounds, item.label, item.action, unit)?;
    }
    canvas.end_focus_region(region);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_answers_sit_side_by_side_and_more_stack_under_the_message() {
        assert!(button_places(0, 100.0).is_empty());
        let pair = button_places(2, 100.0);
        assert_eq!(pair[0][1], pair[1][1]);
        assert!(pair[0][0] < 0.0 && pair[1][0] > 0.0);
        let stack = button_places(3, 100.0);
        assert!(stack[1][1] > stack[0][1] && stack[2][1] > stack[1][1]);
        assert!(stack.iter().all(|place| place[2] == STACK_WIDTH));
    }
}
