//! The Java-style world list, as Java Edition's Select World screen: a title, a scrolling list of
//! 36-pixel entries (icon, name, a line about the world and a line about its size), and two rows
//! of buttons. Pressing an entry selects it; Play then joins it. Friends' worlds come first, then
//! the player's own, which can also be edited and deleted. Creating, importing and editing open the
//! existing screens, which a world's own settings still live on.

use std::collections::HashMap;

use launcher::menu::{LocalWorldAction, MenuAction, MenuView};
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
        Btn::new("Play World", chosen.map(|row| row.play)),
        Btn::new(
            "New World",
            Some(MenuAction::LocalWorld(LocalWorldAction::BeginCreate)),
        ),
        Btn::new(
            "From Template",
            Some(MenuAction::LocalWorld(LocalWorldAction::OpenTemplates)),
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
        header: frame::HEADER,
        rows: &[&first, &second],
        more: None,
    };
    frame::draw(canvas, view, size, &spec, |canvas, frame| {
        list(canvas, view, frame, &rows, chosen, images)
    })
}

/// The scrolling list of worlds.
fn list(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    frame: &Frame,
    rows: &[Row],
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
                put(
                    canvas,
                    "No worlds yet. Use New World to make one.",
                    [left + 4.0 * unit, top + 8.0 * unit],
                    unit,
                    GREY,
                    ENTRY_WIDTH * unit,
                )?;
            }
            for (index, row) in rows.iter().enumerate() {
                let y = top + index as f32 * ENTRY_STEP * unit;
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
    if let Some(art) = row
        .thumbnail
        .as_ref()
        .and_then(|key| images.get(key).or_else(|| canvas.artwork?.get(key)))
        .copied()
    {
        // The picture keeps its shape inside the square.
        let source = [
            f32::from(art.uv[2] - art.uv[0]).max(1.0),
            f32::from(art.uv[3] - art.uv[1]).max(1.0),
        ];
        let scale = (ICON * unit / source[0]).min(ICON * unit / source[1]);
        let size = [source[0] * scale, source[1] * scale];
        let at = [
            icon[0] + (ICON * unit - size[0]) * 0.5,
            icon[1] + (ICON * unit - size[1]) * 0.5,
        ];
        canvas.icon_ref(art, [at[0], at[1], at[0] + size[0], at[1] + size[1]])?;
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
    canvas.hit(select, bounds)
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
        assert!(labelled(&nothing, "Play World").is_none());
        assert!(labelled(&nothing, "Edit").is_none() && labelled(&nothing, "Delete").is_none());
        assert!(labelled(&nothing, "New World").is_some());
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
            labelled(&chosen, "Play World"),
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
            labelled(&chosen, "Play World"),
            Some(MenuAction::PlayFriend(0))
        );
        assert!(labelled(&chosen, "Edit").is_none() && labelled(&chosen, "Delete").is_none());
    }
}
