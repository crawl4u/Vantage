//! A Java-style message over the screen, as Java Edition's confirm and progress screens show one:
//! the screen dimmed, a title and the message in lines under it, and the answers in a row of 150
//! pixel buttons (stacked 200 pixels wide when there are more than two). A message with no
//! answers, such as "Importing world...", stands alone in the middle. A message that lists choices,
//! such as the saved accounts, stacks them as 200 pixel buttons between the message and the
//! answers, scrolling when they do not all fit.

use ui_engine::engine::screen::PaintError;

use super::{
    button,
    frame::{self, Btn, wrap_lines},
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
/// Java pixels kept clear below the last button.
const BOTTOM_MARGIN: f32 = 10.0;
/// A listed choice's face, and the gap before its label.
const FACE: f32 = 16.0;
const FACE_GAP: f32 = 4.0;
/// Scroll bar of a list that does not fit, in Java pixels from the choices' right edge.
const BAR_OFFSET: f32 = 4.0;
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
    let mut bottom = bottom;
    if !modal.items.is_empty() {
        bottom = choices(canvas, view, size, modal, [unit, centre], bottom, java_height)?;
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

/// How tall the listed choices are, in Java pixels: every one while they fit above `room`, else as
/// many whole rows as do.
fn choices_height(count: usize, room: f32) -> f32 {
    let slack = STACK_STEP - BUTTON_HEIGHT;
    let all = count as f32 * STACK_STEP - slack;
    if all <= room {
        return all;
    }
    let rows = ((room + slack) / STACK_STEP).floor().max(1.0);
    rows * STACK_STEP - slack
}

/// Draws the listed choices as a stack of wide buttons under the message and returns where the
/// stack ends, in Java pixels. A choice with no action is the one already chosen and is drawn
/// disabled, its face and label unchanged.
fn choices(
    canvas: &mut Canvas<'_>,
    view: &launcher::menu::MenuView,
    size: [f32; 2],
    modal: &Modal<'_>,
    [unit, centre]: [f32; 2],
    bottom: f32,
    java_height: f32,
) -> Result<f32, PaintError> {
    let top = bottom + 12.0;
    let answers = match modal.buttons.len() {
        0 => 0.0,
        1 | 2 => 12.0 + BUTTON_HEIGHT,
        count => 12.0 + (count - 1) as f32 * STACK_STEP + BUTTON_HEIGHT,
    };
    let room = (java_height - BOTTOM_MARGIN - answers - top).max(BUTTON_HEIGHT);
    let height = choices_height(modal.items.len(), room);
    let content = (modal.items.len() as f32 * STACK_STEP - (STACK_STEP - BUTTON_HEIGHT)) * unit;
    let left = (centre - STACK_WIDTH / 2.0) * unit;
    let right = (centre + STACK_WIDTH / 2.0) * unit;
    let viewport = [0.0, top * unit, size[0], (top + height) * unit];
    frame::scrolled(
        canvas,
        "java_modal_choices",
        viewport,
        content,
        right + BAR_OFFSET * unit,
        unit,
        |canvas, offset| {
            for (index, item) in modal.items.iter().enumerate() {
                let y = top * unit + index as f32 * STACK_STEP * unit - offset;
                let bounds = [left, y, right, y + BUTTON_HEIGHT * unit];
                let label = if item.selected && item.action.is_none() {
                    format!("{} (current)", item.label)
                } else {
                    item.label.to_owned()
                };
                if item.picture_slot {
                    // The account's face sits at the button's left, with the label after it.
                    let face_top = y + (BUTTON_HEIGHT - FACE) / 2.0 * unit;
                    let face = [
                        left + FACE_GAP * unit,
                        face_top,
                        left + (FACE_GAP + FACE) * unit,
                        face_top + FACE * unit,
                    ];
                    button::draw_lit(canvas, view, bounds, "", item.action, false, unit)?;
                    if let Some(art) = item.picture {
                        canvas.icon_ref_cover(art, face)?;
                    }
                    let colour = if item.action.is_some() {
                        TEXT
                    } else {
                        [160, 160, 160, 255]
                    };
                    button::label_centred(
                        canvas,
                        &label,
                        [face[2] + FACE_GAP * unit, y, right - FACE_GAP * unit, y + BUTTON_HEIGHT * unit],
                        unit,
                        colour,
                    )?;
                } else {
                    button::draw_lit(canvas, view, bounds, &label, item.action, false, unit)?;
                }
            }
            Ok(())
        },
    )?;
    Ok(top + height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listed_choices_all_show_when_they_fit_and_whole_rows_show_when_they_do_not() {
        assert_eq!(choices_height(1, 100.0), BUTTON_HEIGHT);
        assert_eq!(choices_height(3, 100.0), 2.0 * STACK_STEP + BUTTON_HEIGHT);
        // Room for two and a bit rows of five: two whole rows.
        let two = STACK_STEP + BUTTON_HEIGHT;
        assert_eq!(choices_height(5, two + 10.0), two);
        // Never less than one row, however little room there is.
        assert_eq!(choices_height(5, 3.0), BUTTON_HEIGHT);
    }

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
