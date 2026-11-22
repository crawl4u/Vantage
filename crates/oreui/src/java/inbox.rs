//! The Java-style Inbox: the world dimmed behind a title, a row of buttons for the four message
//! categories, a scrolling list of that category's messages (unread ones marked, newest first) and
//! the buttons that act on them. Pressing a message opens it on a page of its own with the source
//! and date under the title and the text below; Done returns to the list.

use launcher::menu::{
    InboxItem, MenuAction, MenuView,
    inbox::{Action, CATEGORIES, category_index, date},
};
use ui_engine::engine::screen::PaintError;

use super::frame::{self, Btn, Frame, Spec, put, wrap_lines};
use crate::{
    inbox::empty,
    paint::{Bounds, Canvas},
    theme::TEXT,
};

/// Sizes in Java pixels.
const ENTRY_WIDTH: f32 = 305.0;
const ENTRY_STEP: f32 = 28.0;
const PADDING: f32 = 4.0;
const BAR_OFFSET: f32 = 164.0;
const DELETE_WIDTH: f32 = 20.0;
const DOT: f32 = 4.0;
const LINE: f32 = 10.0;
const GREY: [u8; 4] = [128, 128, 128, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const UNREAD: [u8; 4] = [255, 128, 133, 255];

/// Whether the Java page can draw `view`'s inbox: the list and an opened message, not the
/// Bedrock-style maintenance page.
pub(crate) fn shows_java(view: &MenuView) -> bool {
    !view.feeds.inbox_state.filters
}

/// The message the player has opened, if any.
fn opened(view: &MenuView) -> Option<&InboxItem> {
    let id = view.feeds.inbox_state.opened.as_deref()?;
    view.feeds
        .home
        .inbox
        .iter()
        .find(|item| item.instance_id == id)
}

/// The messages of the chosen category, newest first, with their place in the whole inbox.
fn messages(view: &MenuView) -> Vec<(usize, &InboxItem)> {
    let category = view.feeds.inbox_state.category;
    let mut found: Vec<_> = view
        .feeds
        .home
        .inbox
        .iter()
        .enumerate()
        .filter(|(_, item)| category_index(&item.category) == Some(category))
        .collect();
    found.sort_by(|(_, a), (_, b)| b.received.cmp(&a.received));
    found
}

/// Draws the Java Inbox, or the message the player opened.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<(), PaintError> {
    if let Some(item) = opened(view) {
        return message(canvas, view, size, item);
    }
    let category = view.feeds.inbox_state.category;
    let categories: Vec<Btn<'_>> = CATEGORIES
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let mut button = Btn::new(label, Some(MenuAction::Inbox(Action::Category(index))));
            button.lit = index == category;
            button
        })
        .collect();
    let list = messages(view);
    let any_unread = view.feeds.home.inbox.iter().any(|item| item.unread);
    let actions = [
        Btn::new(
            "Mark All Read",
            any_unread.then_some(MenuAction::Inbox(Action::MarkAllRead)),
        ),
        Btn::new(
            "Delete Read",
            view.feeds
                .home
                .inbox
                .iter()
                .any(|item| !item.unread)
                .then_some(MenuAction::Inbox(Action::DeleteAllRead)),
        ),
        Btn::new("Done", Some(MenuAction::AddBack)),
    ];
    let spec = Spec {
        title: "Inbox",
        subtitle: None,
        header: frame::HEADER,
        rows: &[&categories, &actions],
        more: None,
        title_top: None,
        footer_gap: None,
    };
    frame::draw(canvas, view, size, &spec, |canvas, frame| {
        entries(canvas, view, frame, &list, category)
    })?;
    // A pending delete asks first, in the same Java message the other confirmations use.
    crate::inbox::delete_dialog(canvas, view, size)
}

/// The scrolling list of messages, or what an empty category says.
fn entries(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    frame: &Frame,
    list: &[(usize, &InboxItem)],
    category: usize,
) -> Result<(), PaintError> {
    let unit = frame.unit;
    let content = (PADDING * 2.0 + list.len().max(2) as f32 * ENTRY_STEP) * unit;
    let left = (frame.centre - (ENTRY_WIDTH / 2.0).floor()) * unit;
    frame::scrolled(
        canvas,
        &format!("java_inbox_{category}"),
        frame.viewport,
        content,
        (frame.centre + BAR_OFFSET) * unit,
        unit,
        |canvas, offset| {
            let top = frame.viewport[1] - offset + PADDING * unit;
            if list.is_empty() {
                let (_, title, _, caption) = empty::COPY[category.min(empty::COPY.len() - 1)];
                put(canvas, title, [left + 4.0 * unit, top + 8.0 * unit], unit, TEXT, ENTRY_WIDTH * unit)?;
                for (line, text) in wrap_lines(caption, ENTRY_WIDTH - 8.0).iter().enumerate() {
                    put(
                        canvas,
                        text,
                        [left + 4.0 * unit, top + (8.0 + LINE * (line as f32 + 1.5)) * unit],
                        unit,
                        GREY,
                        ENTRY_WIDTH * unit,
                    )?;
                }
            }
            for (place, (index, item)) in list.iter().enumerate() {
                let y = top + place as f32 * ENTRY_STEP * unit;
                let bounds = [
                    left,
                    y,
                    left + ENTRY_WIDTH * unit,
                    y + (ENTRY_STEP - 4.0) * unit,
                ];
                entry(canvas, view, item, *index, bounds, unit)?;
            }
            Ok(())
        },
    )
}

/// One message: an unread mark, its title, its source and date, and a button to delete it.
fn entry(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    item: &InboxItem,
    index: usize,
    bounds: Bounds,
    unit: f32,
) -> Result<(), PaintError> {
    let open = MenuAction::Inbox(Action::Open(index));
    let state = canvas.interaction(view, Some(open));
    let face = [
        bounds[0],
        bounds[1],
        bounds[2] - (DELETE_WIDTH + 2.0) * unit,
        bounds[3],
    ];
    if state.hovered || state.focused {
        let outer = [
            face[0] - 2.0 * unit,
            face[1] - 2.0 * unit,
            face[2] + 2.0 * unit,
            face[3] + 2.0 * unit,
        ];
        canvas.fill(outer, GREY)?;
        canvas.fill(
            [
                outer[0] + unit,
                outer[1] + unit,
                outer[2] - unit,
                outer[3] - unit,
            ],
            BLACK,
        )?;
    }
    if item.unread {
        canvas.fill(
            [
                face[0] + 2.0 * unit,
                face[1] + 10.0 * unit,
                face[0] + (2.0 + DOT) * unit,
                face[1] + (10.0 + DOT) * unit,
            ],
            UNREAD,
        )?;
    }
    let text_left = face[0] + 10.0 * unit;
    let room = face[2] - text_left;
    put(
        canvas,
        &item.header,
        [text_left, face[1] + unit],
        unit,
        TEXT,
        room,
    )?;
    let source = if item.source.is_empty() {
        "Minecraft"
    } else {
        &item.source
    };
    put(
        canvas,
        &format!("{source} · {}", date(&item.received)),
        [text_left, face[1] + 12.0 * unit],
        unit,
        GREY,
        room,
    )?;
    canvas.hit(open, face)?;
    let delete = [
        bounds[2] - DELETE_WIDTH * unit,
        bounds[1],
        bounds[2],
        bounds[1] + 20.0 * unit,
    ];
    super::button::draw(
        canvas,
        view,
        delete,
        "X",
        Some(MenuAction::Inbox(Action::Delete(index))),
        unit,
    )
}

/// An opened message: its title, then its source and date, then its text, in a scrolling page.
fn message(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
    item: &InboxItem,
) -> Result<(), PaintError> {
    let done = [Btn::new("Done", Some(MenuAction::Inbox(Action::Cancel)))];
    let source = if item.source.is_empty() {
        "Minecraft"
    } else {
        &item.source
    };
    let spec = Spec {
        title: &item.header,
        subtitle: Some(&format!("{source} · {}", date(&item.received))),
        header: frame::HEADER,
        rows: &[&done],
        more: None,
        title_top: None,
        footer_gap: None,
    };
    frame::draw(canvas, view, size, &spec, |canvas, frame| {
        let unit = frame.unit;
        let width = ENTRY_WIDTH.min(frame.centre * 2.0 - 24.0);
        let lines = wrap_lines(&item.body, width - 8.0);
        let content = (PADDING * 2.0 + lines.len().max(1) as f32 * LINE) * unit;
        let left = (frame.centre - (width / 2.0).floor()) * unit;
        frame::scrolled(
            canvas,
            "java_inbox_message",
            frame.viewport,
            content,
            (frame.centre + BAR_OFFSET) * unit,
            unit,
            |canvas, offset| {
                let top = frame.viewport[1] - offset + PADDING * unit;
                for (line, text) in lines.iter().enumerate() {
                    put(
                        canvas,
                        text,
                        [left + 4.0 * unit, top + line as f32 * LINE * unit],
                        unit,
                        TEXT,
                        width * unit,
                    )?;
                }
                Ok(())
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, category: &str, received: &str) -> InboxItem {
        InboxItem {
            instance_id: id.to_owned(),
            category: category.to_owned(),
            received: received.to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn the_list_is_the_chosen_categorys_messages_newest_first_with_their_places_kept() {
        let mut view = MenuView::new(true, "Player".into());
        let first = CATEGORIES[0];
        let other = CATEGORIES[1];
        let mut inbox = view.feeds.home.inbox.clone();
        inbox.push(item("old", first, "2026-01-01T00:00:00Z"));
        inbox.push(item("elsewhere", other, "2026-03-01T00:00:00Z"));
        inbox.push(item("new", first, "2026-02-01T00:00:00Z"));
        view.feeds.home.inbox = inbox;
        view.feeds.inbox_state.category = 0;
        let listed = messages(&view);
        let ids: Vec<_> = listed.iter().map(|(_, item)| item.instance_id.as_str()).collect();
        assert_eq!(ids, ["new", "old"]);
        assert_eq!(listed[0].0, 2, "the place in the whole inbox is the action's index");
    }

    #[test]
    fn the_maintenance_page_stays_bedrock_and_an_opened_message_is_found_by_its_id() {
        let mut view = MenuView::new(true, "Player".into());
        assert!(shows_java(&view));
        view.feeds.inbox_state.filters = true;
        assert!(!shows_java(&view));
        view.feeds.inbox_state.filters = false;
        view.feeds.home.inbox.push(item("a", CATEGORIES[0], ""));
        assert!(opened(&view).is_none());
        view.feeds.inbox_state.opened = Some("a".to_owned());
        assert!(opened(&view).is_some());
    }
}
