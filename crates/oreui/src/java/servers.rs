//! The Java-style Multiplayer screen, laid out as Java Edition lays its server list out: a title
//! 10 Java pixels down, three tabs under it, a scrolling list of 305-pixel entries 36 pixels apart
//! (icon, name, the server's message, its player count and ping bars), and two rows of buttons
//! under that. The tabs choose the list: the player's own servers, featured servers or creator
//! servers. The selected entry is boxed in grey.

use launcher::menu::{MenuAction, MenuView, PingInfo, pingable, server_list::ServerGroup};
use ui_engine::engine::screen::PaintError;

use super::{button, font, unit};
use crate::{
    focus,
    paint::{Bounds, Canvas, SpriteScrollbar},
    play_servers::{Selection, group_entries, selection},
    theme::TEXT,
};

/// Sizes in Java pixels.
const ENTRY_WIDTH: f32 = 305.0;
const ENTRY_STEP: f32 = 36.0;
const ICON: f32 = 32.0;
const HEADER: f32 = 50.0;
const FOOTER: f32 = 64.0;
const TITLE_TOP: f32 = 10.0;
/// The tabs' top edge, their width and the gap between them.
const TABS_TOP: f32 = 24.0;
const TAB_WIDTH: f32 = 98.0;
const TAB_GAP: f32 = 5.0;
const TEXT_HEIGHT: f32 = 8.0;
const LIST_PADDING: f32 = 4.0;
const SEPARATOR: f32 = 2.0;
const BUTTON_HEIGHT: f32 = 20.0;
const WIDE: f32 = 150.0;
const NARROW: f32 = 74.0;
/// Where the rows of buttons start below the footer's top edge.
const FIRST_ROW: f32 = 12.0;
const SECOND_ROW: f32 = 36.0;
/// The ping bars' size, and the gap before them from the entry's right edge.
const PING: [f32; 2] = [10.0, 8.0];
const SCROLLBAR_WIDTH: f32 = 6.0;
const SCROLLBAR_OFFSET: f32 = 164.0;
const SCROLLER_MIN: f32 = 32.0;
const DIM: [u8; 4] = [16, 16, 16, 170];
const LIST_SHADE: [u8; 4] = [0, 0, 0, 90];
const GREY: [u8; 4] = [128, 128, 128, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const ICON_FILL: [u8; 4] = [38, 38, 38, 255];
/// Java's ping sprites from worst to best, then the sprite of a server that did not answer and
/// the one shown while the answer is awaited.
const PING_SPRITES: [&str; 5] = [
    "sprites/server_list/ping_1.png",
    "sprites/server_list/ping_2.png",
    "sprites/server_list/ping_3.png",
    "sprites/server_list/ping_4.png",
    "sprites/server_list/ping_5.png",
];
const UNREACHABLE: &str = "sprites/server_list/ping_unreachable.png";
const PINGING: &str = "sprites/server_list/pinging_1.png";

/// One server of the list.
struct Row {
    name: String,
    message: String,
    ping: Option<PingInfo>,
    pinged: bool,
    select: MenuAction,
    join: MenuAction,
    /// The saved server's index, which only saved servers have, so they alone can be edited.
    saved: Option<usize>,
    selected: bool,
}

/// What a tab is called, and what its list says while it is empty.
fn tab_text(group: ServerGroup) -> (&'static str, &'static str) {
    match group {
        ServerGroup::Saved => ("My Servers", "No servers yet. Add one to start."),
        ServerGroup::Featured => ("Featured", "No featured servers right now."),
        ServerGroup::Creator => ("Creator", "No creator servers right now."),
    }
}

/// The rows of the `group` list, with the row the buttons act on marked: the picked server when
/// it is in this list, else the first.
fn rows(view: &MenuView, group: ServerGroup) -> Vec<Row> {
    let picked = selection(view, view.featured.len());
    let ping = |address: &str| {
        view.feeds
            .pings
            .get(address)
            .cloned()
            .filter(|_| pingable(address))
    };
    let mut rows: Vec<Row> = Vec::new();
    if group == ServerGroup::Saved {
        rows.extend(view.servers.iter().enumerate().map(|(index, server)| {
            let ping = ping(&server.address);
            Row {
                name: server.name.clone(),
                message: ping
                    .as_ref()
                    .map(|ping| ping.motd.clone())
                    .unwrap_or_default(),
                ping,
                pinged: pingable(&server.address),
                select: MenuAction::SelectSaved(index),
                join: MenuAction::PlaySaved(index),
                saved: Some(index),
                selected: picked == Some(Selection::Saved(index)),
            }
        }));
    } else {
        let tab = if group == ServerGroup::Creator {
            "creator"
        } else {
            "featured"
        };
        rows.extend(group_entries(view, tab).into_iter().map(|index| {
            let server = &view.featured[index];
            let ping = ping(&server.address);
            Row {
                name: server.name.clone(),
                message: ping
                    .as_ref()
                    .map(|ping| ping.motd.clone())
                    .filter(|motd| !motd.is_empty())
                    .unwrap_or_else(|| server.caption.clone()),
                ping,
                pinged: pingable(&server.address),
                select: MenuAction::SelectFeatured(index),
                join: MenuAction::PlayFeatured(index),
                saved: None,
                selected: picked == Some(Selection::Featured(index)),
            }
        }));
    }
    if !rows.iter().any(|row| row.selected)
        && let Some(first) = rows.first_mut()
    {
        first.selected = true;
    }
    rows
}

/// `text` without colour codes and line breaks, as one line.
fn plain(text: &str) -> String {
    let mut chars = text.chars();
    let mut clean = String::with_capacity(text.len());
    while let Some(c) = chars.next() {
        match c {
            '§' => {
                chars.next();
            }
            '\n' | '\r' => clean.push(' '),
            _ => clean.push(c),
        }
    }
    clean
}

/// `text` broken at spaces into at most two lines of `max` Java pixels, the second cut short with
/// an ellipsis when more remains, as Java wraps a server's message.
fn wrap(text: &str, max: f32) -> Vec<String> {
    let width = |line: &str| -> f32 {
        crate::images::java::font()
            .and_then(|font| font::width_with(&font, line))
            .unwrap_or(line.chars().count() as f32 * 6.0)
    };
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in plain(text).split_whitespace() {
        let joined = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };
        if width(&joined) <= max || current.is_empty() {
            current = joined;
        } else {
            lines.push(std::mem::take(&mut current));
            current = word.to_owned();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.len() > 2 {
        lines.truncate(2);
        if let Some(last) = lines.last_mut() {
            while !last.is_empty() && width(&format!("{last}...")) > max {
                last.pop();
            }
            last.push_str("...");
        }
    }
    lines
}

/// Java's ping sprite for `ping`, by its round trip: under 150 ms the best, over a second the
/// worst.
fn ping_sprite(ping: Option<&PingInfo>, pinged: bool) -> &'static str {
    match ping {
        None if pinged => PINGING,
        None => UNREACHABLE,
        Some(ping) if !ping.online => UNREACHABLE,
        Some(ping) => {
            let tier = match ping.ping_ms {
                0..150 => 4,
                150..300 => 3,
                300..600 => 2,
                600..1000 => 1,
                _ => 0,
            };
            PING_SPRITES[tier]
        }
    }
}

/// Draws `text` with its top left at `at`, in Java's font or else the client's.
fn put(
    canvas: &mut Canvas<'_>,
    text: &str,
    at: [f32; 2],
    unit: f32,
    colour: [u8; 4],
    room: f32,
) -> Result<(), PaintError> {
    if font::draw(canvas, text, at, unit, colour, true)?.is_some() {
        return Ok(());
    }
    let style = button::label_style(canvas, unit);
    canvas.text_line(text, [at[0], at[1] - unit], room, style, colour)?;
    Ok(())
}

/// Draws the Java Multiplayer screen.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<(), PaintError> {
    let unit = unit(view, canvas, size);
    let java_width = (size[0] / unit).ceil();
    let java_height = size[1] / unit;
    let centre = (java_width / 2.0).floor();
    canvas.fill([0.0, 0.0, size[0], size[1]], DIM)?;
    let screen =
        canvas.begin_focus_region(focus::SCREEN, [0.0, 0.0, size[0], size[1]], None, true)?;
    canvas.focus_delegate(Some(MenuAction::AddBack), None);
    let viewport = [0.0, HEADER * unit, size[0], size[1] - FOOTER * unit];
    canvas.fill(viewport, LIST_SHADE)?;
    let group = view.feeds.java_server_group;
    let rows = rows(view, group);
    list(canvas, view, viewport, centre, &rows, group, unit)?;
    separators(canvas, size, unit)?;
    button::label_centred(
        canvas,
        "Play Multiplayer",
        [
            0.0,
            TITLE_TOP * unit,
            size[0],
            (TITLE_TOP + TEXT_HEIGHT) * unit,
        ],
        unit,
        TEXT,
    )?;
    let chosen = rows.iter().find(|row| row.selected);
    let join = chosen.map(|row| row.join);
    let saved = chosen.and_then(|row| row.saved);
    tabs(canvas, view, centre, group, unit)?;
    let top = java_height - FOOTER;
    let place = |x: f32, row: f32, width: f32| {
        [x, top + row, x + width, top + row + BUTTON_HEIGHT].map(|value| value * unit)
    };
    for (bounds, label, action) in [
        (place(centre - 155.0, FIRST_ROW, WIDE), "Join Server", join),
        (
            place(centre + 5.0, FIRST_ROW, WIDE),
            "Add Server",
            Some(MenuAction::PlayAddServer),
        ),
        (
            place(centre - 155.0, SECOND_ROW, NARROW),
            "Edit",
            saved.map(MenuAction::EditSaved),
        ),
        (
            place(centre - 77.0, SECOND_ROW, NARROW),
            "Delete",
            saved.map(MenuAction::RemoveSavedDialog),
        ),
        (
            place(centre + 5.0, SECOND_ROW, WIDE),
            "Back",
            Some(MenuAction::AddBack),
        ),
    ] {
        button::draw(canvas, view, bounds, label, action, unit)?;
    }
    canvas.end_focus_region(screen);
    Ok(())
}

/// The three tabs under the title, the open one highlighted.
fn tabs(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    centre: f32,
    open: ServerGroup,
    unit: f32,
) -> Result<(), PaintError> {
    let left = centre - (3.0 * TAB_WIDTH + 2.0 * TAB_GAP) / 2.0;
    for (index, group) in ServerGroup::ALL.into_iter().enumerate() {
        let x = left + index as f32 * (TAB_WIDTH + TAB_GAP);
        let bounds =
            [x, TABS_TOP, x + TAB_WIDTH, TABS_TOP + BUTTON_HEIGHT].map(|value| value * unit);
        button::draw_lit(
            canvas,
            view,
            bounds,
            tab_text(group).0,
            Some(MenuAction::JavaServerGroup(group)),
            group == open,
            unit,
        )?;
    }
    Ok(())
}

/// The header and footer's inner edges, from Java's separator sprites.
fn separators(canvas: &mut Canvas<'_>, size: [f32; 2], unit: f32) -> Result<(), PaintError> {
    for (name, top) in [
        ("header_separator.png", (HEADER - SEPARATOR) * unit),
        ("footer_separator.png", size[1] - FOOTER * unit),
    ] {
        let key = crate::images::java::key(name);
        let Some([width, _]) = canvas.java_sprite_size(&key) else {
            continue;
        };
        let tile = (width * unit).max(1.0);
        let mut x = 0.0;
        while x < size[0] {
            canvas.sprite(
                &key,
                [x, top, (x + tile).min(size[0]), top + SEPARATOR * unit],
                [255; 4],
            )?;
            x += tile;
        }
    }
    Ok(())
}

/// The scrolling list of server entries.
fn list(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    viewport: Bounds,
    centre: f32,
    rows: &[Row],
    group: ServerGroup,
    unit: f32,
) -> Result<(), PaintError> {
    let key = match group {
        ServerGroup::Saved => "java_servers/saved",
        ServerGroup::Featured => "java_servers/featured",
        ServerGroup::Creator => "java_servers/creator",
    };
    let content = (LIST_PADDING * 2.0 + rows.len() as f32 * ENTRY_STEP) * unit;
    let max = (content - (viewport[3] - viewport[1])).max(0.0);
    if let Some(offset) = canvas.offsets.get_mut(key) {
        *offset = offset.min(max);
    }
    canvas.sprite_scrollbar = Some(SpriteScrollbar {
        track: crate::images::java::key("sprites/widget/scroller_background.png"),
        thumb: crate::images::java::key("sprites/widget/scroller.png"),
        x: (centre + SCROLLBAR_OFFSET) * unit,
        width: SCROLLBAR_WIDTH * unit,
        min_thumb: SCROLLER_MIN * unit,
    });
    let scroll = canvas.begin_scroll(key, viewport)?;
    let left = centre - (ENTRY_WIDTH / 2.0).floor();
    for (index, row) in rows.iter().enumerate() {
        let y = viewport[1] / unit + LIST_PADDING + index as f32 * ENTRY_STEP;
        let bounds = [left, y, left + ENTRY_WIDTH, y + ENTRY_STEP - 4.0].map(|value| value * unit);
        let bounds = [
            bounds[0],
            bounds[1] - scroll.offset,
            bounds[2],
            bounds[3] - scroll.offset,
        ];
        entry(canvas, view, row, bounds, unit)?;
    }
    if rows.is_empty() {
        put(
            canvas,
            tab_text(group).1,
            [
                (centre - 80.0) * unit,
                viewport[1] + (LIST_PADDING + 8.0) * unit,
            ],
            unit,
            GREY,
            200.0 * unit,
        )?;
    }
    canvas.end_scroll(scroll, content)
}

/// One entry: its box, icon, name, message, player count and ping.
fn entry(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    row: &Row,
    bounds: Bounds,
    unit: f32,
) -> Result<(), PaintError> {
    let state = canvas.interaction(view, Some(row.select));
    if row.selected {
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
    let text_left = bounds[0] + (ICON + 3.0) * unit;
    let room = bounds[2] - text_left - (PING[0] + 8.0) * unit;
    put(
        canvas,
        &row.name,
        [text_left, bounds[1] + unit],
        unit,
        TEXT,
        room,
    )?;
    let lines = wrap(&row.message, ENTRY_WIDTH - ICON - 3.0 - 2.0);
    for (line, text) in lines.iter().enumerate() {
        put(
            canvas,
            text,
            [text_left, bounds[1] + (12.0 + 9.0 * line as f32) * unit],
            unit,
            GREY,
            bounds[2] - text_left,
        )?;
    }
    if let Some(ping) = row.ping.as_ref().filter(|ping| ping.online) {
        let players = format!("{}/{}", ping.players, ping.max_players);
        let width = crate::images::java::font()
            .and_then(|font| font::width_with(&font, &players))
            .unwrap_or(players.len() as f32 * 6.0)
            * unit;
        put(
            canvas,
            &players,
            [bounds[2] - (PING[0] + 8.0) * unit - width, bounds[1] + unit],
            unit,
            GREY,
            width + unit,
        )?;
    }
    let sprite = crate::images::java::key(ping_sprite(row.ping.as_ref(), row.pinged));
    let at = [bounds[2] - 15.0 * unit, bounds[1]];
    canvas.sprite(
        &sprite,
        [at[0], at[1], at[0] + PING[0] * unit, at[1] + PING[1] * unit],
        [255; 4],
    )?;
    canvas.hit(row.select, bounds)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_codes_and_line_breaks_are_removed_from_a_message() {
        assert_eq!(plain("§aHello\n§lWorld"), "Hello World");
    }

    #[test]
    fn a_long_message_wraps_to_two_lines_and_ends_in_an_ellipsis() {
        let lines = wrap(&"word ".repeat(200), 100.0);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with("..."));
        assert_eq!(wrap("short", 100.0), ["short"]);
        assert!(wrap("", 100.0).is_empty());
    }

    #[test]
    fn the_ping_sprite_follows_javas_tiers() {
        let pong = |ping_ms| PingInfo {
            motd: String::new(),
            online: true,
            players: 0,
            max_players: 0,
            ping_ms,
        };
        assert_eq!(ping_sprite(Some(&pong(20)), true), PING_SPRITES[4]);
        assert_eq!(ping_sprite(Some(&pong(200)), true), PING_SPRITES[3]);
        assert_eq!(ping_sprite(Some(&pong(5000)), true), PING_SPRITES[0]);
        assert_eq!(ping_sprite(Some(&PingInfo::default()), true), UNREACHABLE);
        assert_eq!(ping_sprite(None, true), PINGING);
        assert_eq!(ping_sprite(None, false), UNREACHABLE);
    }

    #[test]
    fn each_tab_lists_its_own_servers_only() {
        let mut view = MenuView::new(true, "Player".into());
        view.servers = vec![launcher::menu::SavedServer {
            name: "Mine".to_owned(),
            address: "127.0.0.1:19132".to_owned(),
            favorite: false,
            last_joined_unix: 0,
        }]
        .into();
        view.featured = vec![launcher::menu::MenuServerCard {
            name: "Featured".into(),
            address: "example.test".into(),
            caption: "A caption".into(),
            image_path: String::new(),
            icon: None,
        }]
        .into();
        let card = |name: &str, address: &str| launcher::menu::MenuServerCard {
            name: name.into(),
            address: address.into(),
            caption: String::new(),
            image_path: String::new(),
            icon: None,
        };
        let mut featured = view.featured.to_vec();
        featured.push(card("Maker", "maker.test"));
        view.featured = featured.into();
        view.feeds.details.insert(
            "maker.test".to_owned(),
            launcher::menu::ServerDetails {
                group: "creator".to_owned(),
                ..Default::default()
            },
        );
        let saved = rows(&view, ServerGroup::Saved);
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].select, MenuAction::SelectSaved(0));
        assert_eq!(saved[0].saved, Some(0));
        let featured = rows(&view, ServerGroup::Featured);
        assert_eq!(featured.len(), 1);
        assert_eq!(featured[0].select, MenuAction::SelectFeatured(0));
        assert_eq!(featured[0].message, "A caption");
        assert_eq!(featured[0].saved, None);
        let creator = rows(&view, ServerGroup::Creator);
        assert_eq!(creator.len(), 1);
        assert_eq!(creator[0].join, MenuAction::PlayFeatured(1));
        // Every list marks exactly one row for the buttons to act on.
        for group in ServerGroup::ALL {
            assert_eq!(
                rows(&view, group).iter().filter(|row| row.selected).count(),
                1
            );
        }
        assert!(rows(&MenuView::new(true, "P".into()), ServerGroup::Creator).is_empty());
    }
}
