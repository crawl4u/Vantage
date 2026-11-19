//! The Java-style profile screen, laid out as Java Edition lays out its Statistics and Advancements
//! screens: a title, tabs for the overview and the statistics, a scrolling list of lines, and the
//! footer's buttons. The overview holds the player, the social counts, the achievements' totals and
//! the suggested and recent achievements; the statistics tab holds the returned statistics. A
//! signed-out or unavailable profile says so with the button that fixes it.

use std::collections::HashMap;

use launcher::menu::{MenuAction, MenuScreen, MenuView, ProfileTab, auth::AuthState};
use ui::IconRef;
use ui_engine::engine::screen::PaintError;

use super::frame::{self, Btn, Frame, Spec, put};
use super::skins::contain;
use super::{button, text_width};
use crate::paint::{Bounds, Canvas};
use crate::theme::TEXT;

/// Sizes in Java pixels.
const ROW_WIDTH: f32 = 305.0;
const BAR_OFFSET: f32 = 164.0;
const HEADER: f32 = 50.0;
const TABS_TOP: f32 = 24.0;
const TAB_WIDTH: f32 = 100.0;
const TAB_GAP: f32 = 4.0;
const BUTTON_HEIGHT: f32 = 20.0;
const PADDING: f32 = 4.0;
const ICON: f32 = 32.0;
/// The height of a line with a value, and of a line with a picture.
const STAT_STEP: f32 = 24.0;
const PICTURE_STEP: f32 = 36.0;
const HEADING_STEP: f32 = 18.0;
const GREY: [u8; 4] = [128, 128, 128, 255];
const GOLD: [u8; 4] = [255, 215, 85, 255];
const ICON_FILL: [u8; 4] = [38, 38, 38, 255];

/// One line of the list.
enum Line {
    /// A line over what follows.
    Heading(String),
    /// A label with its value beside it, pressable when it leads somewhere.
    Stat {
        label: String,
        value: String,
        action: Option<MenuAction>,
    },
    /// A picture with a title and a line under it, and a score on the right.
    Picture {
        picture: Option<IconRef>,
        title: String,
        about: String,
        score: Option<String>,
    },
    /// Plain grey text.
    Note(String),
}

impl Line {
    fn step(&self) -> f32 {
        match self {
            Self::Heading(_) => HEADING_STEP,
            Self::Stat { .. } | Self::Note(_) => STAT_STEP,
            Self::Picture { .. } => PICTURE_STEP,
        }
    }
}

/// The lines of the open tab.
fn lines(
    view: &MenuView,
    portrait: Option<IconRef>,
    artwork: &HashMap<String, IconRef>,
) -> Vec<Line> {
    let profile = &view.feeds.profile;
    match view.profile_tab {
        ProfileTab::Overview => {
            let name = if profile.gamertag.is_empty() {
                view.display_name.clone()
            } else {
                profile.gamertag.clone()
            };
            let about = if profile.real_name.is_empty() {
                profile.presence.clone()
            } else {
                profile.real_name.clone()
            };
            let count = |value: Option<u32>| {
                value.map_or_else(|| "-".to_owned(), |value| value.to_string())
            };
            let mut found = vec![
                Line::Picture {
                    picture: portrait,
                    title: name,
                    about,
                    score: None,
                },
                Line::Stat {
                    label: "Friends".to_owned(),
                    value: count(profile.friends),
                    action: profile
                        .friends
                        .filter(|friends| *friends > 0)
                        .map(|_| MenuAction::Navigate(MenuScreen::Friends)),
                },
                Line::Stat {
                    label: "Followers".to_owned(),
                    value: count(profile.followers),
                    action: None,
                },
            ];
            let achievements = profile.achievements.as_ref();
            found.push(Line::Stat {
                label: "Achievements".to_owned(),
                value: achievements.map_or_else(
                    || "-".to_owned(),
                    |data| {
                        let score = data
                            .current_gamerscore
                            .zip(data.max_gamerscore)
                            .map(|(now, max)| format!("  ({now} / {max} G)"))
                            .unwrap_or_default();
                        format!("{} / {}{score}", data.unlocked, data.total)
                    },
                ),
                action: None,
            });
            if let Some(data) = achievements {
                let [suggested, completed] =
                    launcher::menu::profile_achievements::visible_achievements(&data.entries);
                for (title, entries) in [
                    ("Suggested next achievements", suggested),
                    ("Recently completed achievements", completed),
                ] {
                    if entries.is_empty() {
                        continue;
                    }
                    found.push(Line::Heading(title.to_owned()));
                    found.extend(entries.into_iter().map(|entry| {
                        Line::Picture {
                            picture: artwork.get(&entry.image.path).copied(),
                            title: entry.name.clone(),
                            about: entry.description.clone(),
                            score: entry
                                .gamerscore
                                .filter(|score| *score > 0)
                                .map(|score| format!("{score} G")),
                        }
                    }));
                }
            } else if !profile.achievements_loaded {
                found.push(Line::Note("Loading achievements...".to_owned()));
            }
            found
        }
        ProfileTab::Stats => {
            if !profile.statistics_loaded {
                return vec![Line::Note("Loading statistics...".to_owned())];
            }
            let Some(stats) = profile
                .statistics
                .as_ref()
                .filter(|_| !profile.statistics_error)
            else {
                return vec![Line::Note("Statistics are not available.".to_owned())];
            };
            let values = [
                ("Time played", stats.minutes_played.as_deref()),
                ("Blocks broken", stats.blocks_broken.as_deref()),
                ("Mobs defeated", stats.mobs_defeated.as_deref()),
                ("Distance traveled", stats.distance_travelled.as_deref()),
            ];
            let found: Vec<Line> = values
                .into_iter()
                .enumerate()
                .filter_map(|(index, (label, raw))| {
                    let raw = raw?;
                    let value = if index == 0 {
                        launcher::menu::profile_minutes_display(raw)
                    } else {
                        launcher::menu::profile_count_display(raw)
                    }?;
                    Some(Line::Stat {
                        label: label.to_owned(),
                        value,
                        action: None,
                    })
                })
                .collect();
            if found.is_empty() {
                vec![Line::Note("No statistics yet.".to_owned())]
            } else {
                found
            }
        }
    }
}

/// Draws the Java profile screen.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
    portrait: Option<IconRef>,
    artwork: &HashMap<String, IconRef>,
) -> Result<(), PaintError> {
    let profile = &view.feeds.profile;
    let signed_in = view.auth_state == AuthState::Authenticated;
    let ready = signed_in && profile.loaded && !profile.unavailable;
    let avatar = artwork
        .get(&profile.avatar_path)
        .copied()
        .filter(|_| !profile.avatar_error);
    let portrait = portrait.or(avatar);
    let lines = if ready {
        lines(view, portrait, artwork)
    } else {
        Vec::new()
    };
    let wardrobe = Btn::new(
        "Skin Presets",
        Some(MenuAction::Navigate(MenuScreen::DressingRoom)),
    );
    let refresh = Btn::new(
        "Refresh",
        (signed_in && !profile.unavailable).then_some(MenuAction::RefreshProfile),
    );
    let first = [wardrobe, refresh];
    let second = [Btn::new("Done", Some(MenuAction::AddBack))];
    let spec = Spec {
        title: "Your Profile",
        subtitle: None,
        header: if ready { HEADER } else { frame::HEADER },
        rows: &[&first, &second],
        more: None,
    };
    frame::draw(canvas, view, size, &spec, |canvas, frame| {
        if !signed_in {
            return notice(
                canvas,
                view,
                frame,
                size,
                "Profile Unavailable",
                "Please sign in to a Microsoft account to view player profiles.",
                "Sign in with Microsoft",
                MenuAction::StartSignIn,
            );
        }
        if profile.unavailable {
            return notice(
                canvas,
                view,
                frame,
                size,
                "Profile couldn't load!",
                "We encountered an unknown error, please try again.",
                "Try again",
                MenuAction::RefreshProfile,
            );
        }
        if !profile.loaded {
            return centred(canvas, frame, size, "Loading profile...", frame.viewport[1]);
        }
        tabs(canvas, view, frame.centre, frame.unit)?;
        list(canvas, view, frame, &lines)
    })
}

/// The tabs under the title, the open one lit.
fn tabs(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    centre: f32,
    unit: f32,
) -> Result<(), PaintError> {
    let tabs = [
        ("Overview", ProfileTab::Overview),
        ("Stats", ProfileTab::Stats),
    ];
    let total = tabs.len() as f32 * TAB_WIDTH + (tabs.len() as f32 - 1.0) * TAB_GAP;
    for (place, (label, tab)) in tabs.into_iter().enumerate() {
        let x = centre - total / 2.0 + place as f32 * (TAB_WIDTH + TAB_GAP);
        let bounds = [x, TABS_TOP, x + TAB_WIDTH, TABS_TOP + BUTTON_HEIGHT].map(|v| v * unit);
        button::draw_lit(
            canvas,
            view,
            bounds,
            label,
            Some(MenuAction::SelectProfileTab(tab)),
            view.profile_tab == tab,
            unit,
        )?;
    }
    Ok(())
}

/// Text in the middle of the list, `top` logical pixels down, in Java's font.
fn centred(
    canvas: &mut Canvas<'_>,
    frame: &Frame,
    size: [f32; 2],
    text: &str,
    top: f32,
) -> Result<(), PaintError> {
    let unit = frame.unit;
    let width = text_width(text).unwrap_or(text.len() as f32 * 6.0);
    put(
        canvas,
        text,
        [(size[0] - width * unit) / 2.0, top + 16.0 * unit],
        unit,
        TEXT,
        size[0],
    )
}

/// A title, a sentence and the button that answers it, in the middle of the screen.
#[allow(clippy::too_many_arguments)]
fn notice(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    frame: &Frame,
    size: [f32; 2],
    title: &str,
    text: &str,
    label: &str,
    action: MenuAction,
) -> Result<(), PaintError> {
    let unit = frame.unit;
    let top = frame.viewport[1];
    centred(canvas, frame, size, title, top)?;
    for (place, line) in frame::wrap_lines(text, ROW_WIDTH - 20.0)
        .into_iter()
        .enumerate()
    {
        let width = text_width(&line).unwrap_or(line.len() as f32 * 6.0);
        put(
            canvas,
            &line,
            [
                (size[0] - width * unit) / 2.0,
                top + (34.0 + place as f32 * 10.0) * unit,
            ],
            unit,
            GREY,
            size[0],
        )?;
    }
    let width = 200.0 * unit;
    let left = (size[0] - width) / 2.0;
    let y = top + 64.0 * unit;
    button::draw(
        canvas,
        view,
        [left, y, left + width, y + BUTTON_HEIGHT * unit],
        label,
        Some(action),
        unit,
    )
}

/// The scrolling list of lines.
fn list(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    frame: &Frame,
    lines: &[Line],
) -> Result<(), PaintError> {
    let unit = frame.unit;
    let content = (PADDING * 2.0 + lines.iter().map(Line::step).sum::<f32>()) * unit;
    let left = (frame.centre - (ROW_WIDTH / 2.0).floor()) * unit;
    let key = format!("java_profile_{:?}", view.profile_tab);
    frame::scrolled(
        canvas,
        &key,
        frame.viewport,
        content,
        (frame.centre + BAR_OFFSET) * unit,
        unit,
        |canvas, offset| {
            let mut y = frame.viewport[1] - offset + PADDING * unit;
            for line in lines {
                let height = line.step() * unit;
                let bounds = [left, y, left + ROW_WIDTH * unit, y + height - 4.0 * unit];
                if bounds[3] >= frame.viewport[1] && bounds[1] <= frame.viewport[3] {
                    row(canvas, view, line, bounds, unit)?;
                }
                y += height;
            }
            Ok(())
        },
    )
}

/// One line drawn inside `bounds`.
fn row(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    line: &Line,
    bounds: Bounds,
    unit: f32,
) -> Result<(), PaintError> {
    let room = bounds[2] - bounds[0];
    match line {
        Line::Heading(text) => put(
            canvas,
            text,
            [bounds[0], bounds[1] + 6.0 * unit],
            unit,
            TEXT,
            room,
        ),
        Line::Note(text) => put(
            canvas,
            text,
            [bounds[0] + 4.0 * unit, bounds[1] + 6.0 * unit],
            unit,
            GREY,
            room,
        ),
        Line::Stat {
            label,
            value,
            action,
        } => {
            let state = canvas.interaction(view, *action);
            if state.hovered || state.focused {
                canvas.fill(bounds, [255, 255, 255, 24])?;
            }
            put(
                canvas,
                label,
                [bounds[0] + 4.0 * unit, bounds[1] + 6.0 * unit],
                unit,
                TEXT,
                room * 0.5,
            )?;
            let width = text_width(value).unwrap_or(value.len() as f32 * 6.0) * unit;
            put(
                canvas,
                value,
                [
                    (bounds[2] - width - 4.0 * unit).max(bounds[0] + room * 0.5),
                    bounds[1] + 6.0 * unit,
                ],
                unit,
                GREY,
                room * 0.5,
            )?;
            if let Some(action) = action {
                canvas.hit(*action, bounds)?;
            }
            Ok(())
        }
        Line::Picture {
            picture,
            title,
            about,
            score,
        } => {
            let icon = [
                bounds[0],
                bounds[1],
                bounds[0] + ICON * unit,
                bounds[1] + ICON * unit,
            ];
            canvas.fill(icon, ICON_FILL)?;
            if let Some(art) = picture {
                contain(canvas, *art, icon)?;
            }
            let text_left = bounds[0] + (ICON + 3.0) * unit;
            let score_room = score.as_deref().map_or(0.0, |score| {
                (text_width(score).unwrap_or(score.len() as f32 * 6.0) + 8.0) * unit
            });
            let room = bounds[2] - text_left - score_room;
            put(
                canvas,
                title,
                [text_left, bounds[1] + unit],
                unit,
                TEXT,
                room,
            )?;
            put(
                canvas,
                about,
                [text_left, bounds[1] + 12.0 * unit],
                unit,
                GREY,
                room,
            )?;
            if let Some(score) = score {
                let width = text_width(score).unwrap_or(score.len() as f32 * 6.0) * unit;
                put(
                    canvas,
                    score,
                    [bounds[2] - width - 4.0 * unit, bounds[1] + 12.0 * unit],
                    unit,
                    GOLD,
                    width + unit,
                )?;
            }
            Ok(())
        }
    }
}
