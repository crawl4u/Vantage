//! The Java-style Friends screen, in the manner of Java Edition's Social Interactions list: a title
//! and a search field above a scrolling list of the friends playing now, each entry a face tile, the
//! friend's name and the world and players they are in, and Done below. Pressing an entry joins
//! that friend's world, as the rows of the Bedrock-style drawer do.

use launcher::menu::{MenuAction, MenuField, MenuView};
use ui_engine::engine::screen::PaintError;

use super::frame::{self, Btn, Frame, Spec, put};
use crate::{
    paint::{Bounds, Canvas},
    theme::TEXT,
};

/// Sizes in Java pixels, from the world list's layout.
const ENTRY_WIDTH: f32 = 305.0;
const ENTRY_STEP: f32 = 36.0;
const ICON: f32 = 32.0;
const PADDING: f32 = 4.0;
const BAR_OFFSET: f32 = 164.0;
const HEADER: f32 = 48.0;
const TITLE_TOP: f32 = 8.0;
const SEARCH_TOP: f32 = 21.0;
const SEARCH_HEIGHT: f32 = 20.0;
const SEARCH_WIDTH: f32 = 200.0;
const GREY: [u8; 4] = [128, 128, 128, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const ICON_FILL: [u8; 4] = [38, 38, 38, 255];

/// The friends whose name has every whitespace-separated term of `query`, in any case, with their
/// place in the unfiltered list.
fn matching<'a>(
    friends: &'a [launcher::menu::MenuFriendCard],
    query: &str,
) -> Vec<(usize, &'a launcher::menu::MenuFriendCard)> {
    friends
        .iter()
        .enumerate()
        .filter(|(_, friend)| {
            let name = friend.gamertag.to_lowercase();
            query
                .split_whitespace()
                .all(|term| name.contains(&term.to_lowercase()))
        })
        .collect()
}

/// Draws the Java Friends screen.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<(), PaintError> {
    let done = [Btn::new("Done", Some(MenuAction::AddBack))];
    let spec = Spec {
        title: "Friends",
        subtitle: None,
        header: HEADER,
        rows: &[&done],
        more: None,
        title_top: Some(TITLE_TOP),
        footer_gap: None,
    };
    frame::draw(canvas, view, size, &spec, |canvas, frame| {
        let unit = frame.unit;
        let width = SEARCH_WIDTH.min(frame.centre * 2.0 - 24.0);
        let left = frame.centre - width / 2.0;
        crate::widgets::java_field(
            canvas,
            view,
            [left, SEARCH_TOP, left + width, SEARCH_TOP + SEARCH_HEIGHT].map(|v| v * unit),
            &view.friend_query,
            "Search for people...",
            view.field == Some(MenuField::FriendQuery),
            Some(MenuAction::SearchFriends),
            unit,
        )?;
        let shown = matching(&view.friends, &view.friend_query);
        list(canvas, view, frame, &shown)
    })
}

/// The scrolling list of friends playing now.
fn list(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    frame: &Frame,
    shown: &[(usize, &launcher::menu::MenuFriendCard)],
) -> Result<(), PaintError> {
    let unit = frame.unit;
    let content = (PADDING * 2.0 + shown.len().max(1) as f32 * ENTRY_STEP) * unit;
    let left = (frame.centre - (ENTRY_WIDTH / 2.0).floor()) * unit;
    frame::scrolled(
        canvas,
        "java_friends",
        frame.viewport,
        content,
        (frame.centre + BAR_OFFSET) * unit,
        unit,
        |canvas, offset| {
            let top = frame.viewport[1] - offset + PADDING * unit;
            if shown.is_empty() {
                let text = if view.friend_query.trim().is_empty() {
                    "None of your friends are playing right now."
                } else {
                    "No matching friends are playing right now."
                };
                put(
                    canvas,
                    text,
                    [left + 4.0 * unit, top + 8.0 * unit],
                    unit,
                    GREY,
                    ENTRY_WIDTH * unit,
                )?;
            }
            for (place, (index, friend)) in shown.iter().enumerate() {
                let y = top + place as f32 * ENTRY_STEP * unit;
                let bounds = [
                    left,
                    y,
                    left + ENTRY_WIDTH * unit,
                    y + (ENTRY_STEP - 4.0) * unit,
                ];
                entry(canvas, view, friend, *index, bounds, unit)?;
            }
            Ok(())
        },
    )
}

/// One friend's entry: a tile with their initial, their name, then their world and who is in it.
fn entry(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    friend: &launcher::menu::MenuFriendCard,
    index: usize,
    bounds: Bounds,
    unit: f32,
) -> Result<(), PaintError> {
    let join = MenuAction::PlayFriend(index);
    let state = canvas.interaction(view, Some(join));
    if state.hovered || state.focused {
        let outer = [
            bounds[0] - 2.0 * unit,
            bounds[1] - 2.0 * unit,
            bounds[2] + 2.0 * unit,
            bounds[3] + 2.0 * unit,
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
    let tile = [
        bounds[0],
        bounds[1],
        bounds[0] + ICON * unit,
        bounds[1] + ICON * unit,
    ];
    canvas.fill(tile, ICON_FILL)?;
    if let Some(initial) = friend.gamertag.chars().next() {
        put(
            canvas,
            &initial.to_uppercase().to_string(),
            [tile[0] + 13.0 * unit, tile[1] + 12.0 * unit],
            unit,
            TEXT,
            ICON * unit,
        )?;
    }
    let text_left = bounds[0] + (ICON + 3.0) * unit;
    let room = bounds[2] - text_left;
    put(
        canvas,
        &friend.gamertag,
        [text_left, bounds[1] + unit],
        unit,
        TEXT,
        room,
    )?;
    put(
        canvas,
        &friend.world_name,
        [text_left, bounds[1] + 12.0 * unit],
        unit,
        GREY,
        room,
    )?;
    put(
        canvas,
        &friend.members,
        [text_left, bounds[1] + 21.0 * unit],
        unit,
        GREY,
        room,
    )?;
    canvas.hit(join, bounds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher::menu::MenuFriendCard;

    fn friend(name: &str) -> MenuFriendCard {
        MenuFriendCard {
            gamertag: name.to_owned(),
            world_name: format!("{name}'s world"),
            members: "2 players".to_owned(),
            xuid: String::new(),
            max_members: 0,
        }
    }

    #[test]
    fn the_search_needs_every_term_in_the_name_in_any_case_and_keeps_each_friends_place() {
        let friends = vec![friend("Alex Smith"), friend("Steve"), friend("alexander")];
        let all = matching(&friends, "");
        assert_eq!(all.len(), 3);
        let found = matching(&friends, "ALEX");
        assert_eq!(
            found.iter().map(|(index, _)| *index).collect::<Vec<_>>(),
            [0, 2]
        );
        assert_eq!(matching(&friends, "alex smith").len(), 1);
        assert!(matching(&friends, "nobody").is_empty());
    }
}
