//! The Java-style world list, as Java Edition's Select World screen: a title, a scrolling list of
//! 36-pixel entries (icon, name, a line about the world and a line about its size), and two rows
//! of buttons. Pressing an entry selects it; Play then joins it. Friends' worlds come first, then
//! the player's own, which can also be edited and deleted. Creating, importing and editing open the
//! existing screens, which a world's own settings still live on.

use std::collections::HashMap;

use launcher::menu::{LocalWorldAction, MenuAction, MenuField, MenuView};
use ui::IconRef;
use ui_engine::engine::screen::PaintError;

use super::frame::{self, Btn, Frame, Spec, put};
use crate::{
    paint::{Bounds, Canvas},
    theme::TEXT,
};

/// Sizes in Java pixels.
const ENTRY_WIDTH: f32 = 305.0;
const ENTRY_STEP: f32 = 36.0;
const ICON: f32 = 32.0;
const PADDING: f32 = 4.0;
const BAR_OFFSET: f32 = 164.0;
const GREY: [u8; 4] = [128, 128, 128, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const ICON_FILL: [u8; 4] = [38, 38, 38, 255];
/// Java's header of the world list: the title from 8, the search field from 21, ending at 48.
const HEADER: f32 = 48.0;
const TITLE_TOP: f32 = 8.0;
const SEARCH_TOP: f32 = 21.0;
/// Java's footer rows are 308 wide with 8 between buttons: two of 150, four of 71.
const FOOTER_GAP: f32 = 8.0;
const SEARCH_HEIGHT: f32 = 20.0;
const SEARCH_WIDTH: f32 = 200.0;
/// Java's play arrow over a hovered world's icon, at rest and under the pointer.
const JOIN: [&str; 2] = [
    "sprites/world_list/join.png",
    "sprites/world_list/join_highlighted.png",
];

/// One world of the list.
struct Row {
    title: String,
    /// What the world is: its mode, type and server, or whose it is.
    about: String,
    /// Its size and date, or who is in it.
    detail: String,
    thumbnail: Option<String>,
    /// What playing it does.
    play: MenuAction,
    /// The index of an owned world, which only owned worlds have.
    local: Option<usize>,
}

impl Row {
    /// Whether the world's name has every whitespace-separated term of `query`, in any case.
    fn matches(&self, query: &str) -> bool {
        let name = self.title.to_lowercase();
        query
            .split_whitespace()
            .all(|term| name.contains(&term.to_lowercase()))
    }
}

/// The rows: friends' worlds, then the player's own.
fn rows(view: &MenuView) -> Vec<Row> {
    let friends = view.friends.iter().enumerate().map(|(index, friend)| Row {
        title: friend.world_name.clone(),
        about: format!("{}'s world", friend.gamertag),
        detail: friend.members.clone(),
        thumbnail: None,
        play: MenuAction::PlayFriend(index),
        local: None,
    });
    let own = view
        .local_worlds
        .iter()
        .enumerate()
        .map(|(index, world)| Row {
            title: world.name.clone(),
            about: [
                world.game_mode.as_str(),
                world.world_type.as_str(),
                world.server.as_str(),
            ]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · "),
            detail: [world.size.as_str(), world.date.as_str()]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" · "),
            thumbnail: world.thumbnail.as_ref().map(|image| image.key.clone()),
            play: MenuAction::PlayLocalWorld(index),
            local: Some(index),
        });
    friends.chain(own).collect()
}

/// The footer's buttons for the chosen world, if any.
fn footer(chosen: Option<&Row>) -> (Vec<Btn<'static>>, Vec<Btn<'static>>) {
    let local = chosen.and_then(|row| row.local);
    let first = vec![
        Btn::new("Play Selected World", chosen.map(|row| row.play)),
        Btn::new(
            "Create New World",
            Some(MenuAction::LocalWorld(LocalWorldAction::BeginCreate)),
        ),
    ];
    let second = vec![
        Btn::new(
            "Edit",
            local.map(|index| MenuAction::LocalWorld(LocalWorldAction::Edit(index))),
        ),
        Btn::new("Delete", local.map(MenuAction::DeleteLocalWorld)),
        Btn::new(
            "Import",
            Some(MenuAction::LocalWorld(LocalWorldAction::Import)),
        ),
        Btn::new("Back", Some(MenuAction::AddBack)),
    ];
    (first, second)
}

/// Draws the Java world list.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
    images: &HashMap<String, IconRef>,
) -> Result<(), PaintError> {
    let rows = rows(view);
    let chosen = view
        .feeds
        .selected_play_world
        .filter(|index| *index < rows.len());
    let (first, second) = footer(chosen.map(|index| &rows[index]));
    let spec = Spec {
        title: "Select World",
        subtitle: None,
        header: HEADER,
        rows: &[&first, &second],
        more: None,
        title_top: Some(TITLE_TOP),
        footer_gap: Some(FOOTER_GAP),
    };
    frame::draw(canvas, view, size, &spec, |canvas, frame| {
        let unit = frame.unit;
        let width = SEARCH_WIDTH.min(frame.centre * 2.0 - 24.0);
        let left = frame.centre - width / 2.0;
        crate::widgets::java_field(
            canvas,
            view,
            [left, SEARCH_TOP, left + width, SEARCH_TOP + SEARCH_HEIGHT].map(|v| v * unit),
            &view.world_query,
            "Search...",
            view.field == Some(MenuField::WorldQuery),
            Some(MenuAction::SearchWorlds),
            unit,
        )?;
        let shown: Vec<(usize, &Row)> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.matches(&view.world_query))
            .collect();
        list(canvas, view, frame, &shown, rows.is_empty(), chosen, images)
    })
}

/// The scrolling list of worlds.
fn list(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    frame: &Frame,
    rows: &[(usize, &Row)],
    none_at_all: bool,
    chosen: Option<usize>,
    images: &HashMap<String, IconRef>,
) -> Result<(), PaintError> {
    let unit = frame.unit;
    let content = (PADDING * 2.0 + rows.len().max(1) as f32 * ENTRY_STEP) * unit;
    let left = (frame.centre - (ENTRY_WIDTH / 2.0).floor()) * unit;
    frame::scrolled(
        canvas,
        "java_worlds",
        frame.viewport,
        content,
        (frame.centre + BAR_OFFSET) * unit,
        unit,
        |canvas, offset| {
            let top = frame.viewport[1] - offset + PADDING * unit;
            if rows.is_empty() {
                let text = if none_at_all {
                    "No worlds yet. Use New World to make one."
                } else {
                    "No worlds match"
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
            for (place, (index, row)) in rows.iter().enumerate() {
                let index = *index;
                let y = top + place as f32 * ENTRY_STEP * unit;
                let bounds = [
                    left,
                    y,
                    left + ENTRY_WIDTH * unit,
                    y + (ENTRY_STEP - 4.0) * unit,
                ];
                entry(
                    canvas,
                    view,
                    row,
                    index,
                    chosen == Some(index),
                    bounds,
                    images,
                    unit,
                )?;
            }
            Ok(())
        },
    )
}

/// One world's entry: boxed while chosen, lit under the pointer.
#[allow(clippy::too_many_arguments)]
fn entry(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    row: &Row,
    index: usize,
    selected: bool,
    bounds: Bounds,
    images: &HashMap<String, IconRef>,
    unit: f32,
) -> Result<(), PaintError> {
    let select = MenuAction::SelectPlayWorld(index);
    let state = canvas.interaction(view, Some(select));
    if selected {
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
    } else if state.hovered || state.focused {
        canvas.fill(bounds, [255, 255, 255, 24])?;
    }
    let icon = [
        bounds[0],
        bounds[1],
        bounds[0] + ICON * unit,
        bounds[1] + ICON * unit,
    ];
    canvas.fill(icon, ICON_FILL)?;
    let captured = row
        .thumbnail
        .as_ref()
        .and_then(|key| images.get(key).or_else(|| canvas.artwork?.get(key)))
        .copied();
    if let Some(art) = captured {
        // A captured view is wider than the square Java gives a world: its middle fills it,
        // with nothing stretched and no bars.
        canvas.icon_ref_cover(art, icon)?;
    } else {
        // Java's picture for a world with no icon yet.
        canvas.sprite(
            &crate::images::java::key(java_resource_pack::menu_files::UNKNOWN_WORLD_ICON),
            icon,
            [255; 4],
        )?;
    }
    let text_left = bounds[0] + (ICON + 3.0) * unit;
    let room = bounds[2] - text_left;
    put(
        canvas,
        &row.title,
        [text_left, bounds[1] + unit],
        unit,
        TEXT,
        room,
    )?;
    put(
        canvas,
        &row.about,
        [text_left, bounds[1] + 12.0 * unit],
        unit,
        GREY,
        room,
    )?;
    put(
        canvas,
        &row.detail,
        [text_left, bounds[1] + 21.0 * unit],
        unit,
        GREY,
        room,
    )?;
    canvas.hit(select, bounds)?;
    play_arrow(
        canvas,
        view,
        MenuAction::JoinPlayWorld(index),
        state.hovered || state.focused,
        icon,
        unit,
    )
}

/// Java's play arrow over a world's icon while the pointer is on the entry; pressing it plays
/// the world. It is the game's sprite, or the resource pack's own, else a plain triangle.
pub(super) fn play_arrow(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    play: MenuAction,
    entry_hovered: bool,
    icon: Bounds,
    unit: f32,
) -> Result<(), PaintError> {
    canvas.pointer_hit(play, icon)?;
    let over_icon = canvas.interaction(view, Some(play)).hovered;
    if !over_icon && !entry_hovered {
        return Ok(());
    }
    canvas.fill(icon, [144, 144, 144, 160])?;
    let [rest, lit] = JOIN;
    let key = crate::images::java::key(if over_icon { lit } else { rest });
    if canvas.sprite(&key, icon, [255; 4])? {
        return Ok(());
    }
    // Five stripes make a triangle pointing right.
    let colour = if over_icon {
        [255, 255, 160, 255]
    } else {
        [255, 255, 255, 255]
    };
    let centre = [(icon[0] + icon[2]) * 0.5, (icon[1] + icon[3]) * 0.5];
    for step in 0..7 {
        let along = (step as f32 - 3.0) * 2.0 * unit;
        let span = (3.5 - (step as f32 - 3.0).abs()) * 2.0 * unit;
        canvas.fill(
            [
                centre[0] + along - unit,
                centre[1] - span,
                centre[0] + along + unit,
                centre[1] + span,
            ],
            colour,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher::menu::LocalWorldCard;

    fn card(name: &str) -> LocalWorldCard {
        LocalWorldCard {
            name: name.to_owned(),
            game_mode: "Survival".to_owned(),
            world_type: "Default".to_owned(),
            server: String::new(),
            date: "11/2/2026".to_owned(),
            size: "48.0 MB".to_owned(),
            thumbnail: None,
        }
    }

    #[test]
    fn an_owned_world_is_described_by_its_mode_type_size_and_date() {
        let mut view = MenuView::new(true, "Player".into());
        view.local_worlds = vec![card("Home")].into();
        let listed = rows(&view);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].about, "Survival · Default");
        assert_eq!(listed[0].detail, "48.0 MB · 11/2/2026");
        assert_eq!(listed[0].play, MenuAction::PlayLocalWorld(0));
        assert_eq!(listed[0].local, Some(0));
    }

    #[test]
    fn the_search_needs_every_term_in_the_name_in_any_case() {
        let mut view = MenuView::new(true, "Player".into());
        view.local_worlds = vec![card("Creative Plots")].into();
        let row = &rows(&view)[0];
        assert!(row.matches(""));
        assert!(row.matches("plots CREATIVE"));
        assert!(!row.matches("plots survival"));
    }

    #[test]
    fn the_footer_waits_for_a_world_and_only_edits_and_deletes_owned_ones() {
        let nothing = footer(None);
        let labelled = |footer: &(Vec<Btn<'static>>, Vec<Btn<'static>>), label: &str| {
            footer
                .0
                .iter()
                .chain(&footer.1)
                .find(|button| button.label == label)
                .and_then(|button| button.action)
        };
        assert!(labelled(&nothing, "Play Selected World").is_none());
        assert!(labelled(&nothing, "Edit").is_none() && labelled(&nothing, "Delete").is_none());
        assert!(labelled(&nothing, "Create New World").is_some());
        let owned = Row {
            title: "Home".into(),
            about: String::new(),
            detail: String::new(),
            thumbnail: None,
            play: MenuAction::PlayLocalWorld(2),
            local: Some(2),
        };
        let chosen = footer(Some(&owned));
        assert_eq!(
            labelled(&chosen, "Play Selected World"),
            Some(MenuAction::PlayLocalWorld(2))
        );
        assert_eq!(
            labelled(&chosen, "Delete"),
            Some(MenuAction::DeleteLocalWorld(2))
        );
        // A friend's world can be joined but not edited or deleted.
        let friend = Row {
            local: None,
            play: MenuAction::PlayFriend(0),
            ..owned
        };
        let chosen = footer(Some(&friend));
        assert_eq!(
            labelled(&chosen, "Play Selected World"),
            Some(MenuAction::PlayFriend(0))
        );
        assert!(labelled(&chosen, "Edit").is_none() && labelled(&chosen, "Delete").is_none());
    }
}
