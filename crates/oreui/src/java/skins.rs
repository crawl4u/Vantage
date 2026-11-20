//! The Java-style Skin Presets screen: tabs for skins, capes and cosmetics over a list of 36-pixel
//! entries, as Java Edition lists worlds. Each entry shows the look's picture, its name and what it
//! is, and pressing it equips it; the looks the player imported also have an Edit button. Importing
//! and leaving sit in the footer. The equipped look stands beside the list when the window is wide
//! enough. Editing a look opens a page of its own, drawn over this one.

use launcher::dressing_room::{
    Action, CosmeticCategory, DressingRoomSection, SkinEditor, SkinEditorMode, SkinEditorTarget,
    SkinModel, cape_thumbnail_key, thumbnail_key,
};
use launcher::menu::{MenuAction, MenuField, MenuView};
use ui_engine::engine::screen::PaintError;

use super::button;
use super::frame::{self, Btn, Frame, Spec, put};
use super::options::{Entry, list};
use crate::dressing_room::PreviewArea;
use crate::paint::{Bounds, Canvas};
use crate::theme::TEXT;

/// Sizes in Java pixels.
const ENTRY_WIDTH: f32 = 305.0;
const ENTRY_STEP: f32 = 36.0;
const ICON: f32 = 32.0;
const PADDING: f32 = 4.0;
const BAR_OFFSET: f32 = 164.0;
const HEADER: f32 = 50.0;
const TABS_TOP: f32 = 24.0;
const TAB_WIDTH: f32 = 100.0;
const TAB_GAP: f32 = 4.0;
const BUTTON_HEIGHT: f32 = 20.0;
const EDIT_WIDTH: f32 = 44.0;
const EDIT_HEIGHT: f32 = 16.0;
/// The room the equipped character needs beside the list, and its distance from the list.
const STAGE_WIDTH: f32 = 64.0;
const STAGE_GAP: f32 = 10.0;
const STAGE_MIN: f32 = 36.0;
const GREY: [u8; 4] = [128, 128, 128, 255];
const GREEN: [u8; 4] = [85, 255, 85, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const ICON_FILL: [u8; 4] = [38, 38, 38, 255];

fn command(action: Action) -> MenuAction {
    MenuAction::DressingRoom(action)
}

/// What an entry stands for, so the screen can tell the adapter which pictures it needs.
#[derive(Clone, Copy)]
enum Kind {
    Skin(usize),
    Cape(Option<usize>),
    Cosmetic(usize),
}

/// One entry of the list.
struct Row {
    kind: Kind,
    title: String,
    about: String,
    /// Where the look is from, or that it is equipped.
    status: String,
    /// The key of its picture in the artwork.
    picture: Option<String>,
    equipped: bool,
    /// What pressing the entry does; none for a look the player does not own.
    select: Option<MenuAction>,
    /// Opens the page that edits an imported look.
    edit: Option<MenuAction>,
}

/// The rows of the open tab: the game's own looks first, then the player's.
fn rows(view: &MenuView) -> Vec<Row> {
    let room = &view.dressing_room;
    let equipped = |is: bool, from: &str| {
        if is {
            "Equipped".to_owned()
        } else {
            from.to_owned()
        }
    };
    match room.section {
        DressingRoomSection::Skins => [false, true]
            .into_iter()
            .flat_map(|imported| {
                room.skins
                    .iter()
                    .enumerate()
                    .filter(move |(_, skin)| skin.imported == imported)
            })
            .map(|(index, skin)| Row {
                kind: Kind::Skin(index),
                title: skin.name.clone(),
                about: match skin.model {
                    SkinModel::Classic => "Classic arms",
                    SkinModel::Slim => "Slim arms",
                    SkinModel::Custom => "Custom model",
                }
                .to_owned(),
                status: equipped(
                    room.selected == Some(index),
                    if skin.imported {
                        "Custom skin"
                    } else {
                        "Minecraft skin"
                    },
                ),
                picture: Some(thumbnail_key(&skin.id)),
                equipped: room.selected == Some(index),
                select: Some(command(Action::Select(index))),
                edit: skin.imported.then(|| command(Action::BeginRename(index))),
            })
            .collect(),
        DressingRoomSection::Capes => {
            let none = Row {
                kind: Kind::Cape(None),
                title: "No cape".to_owned(),
                about: "Keep your look simple".to_owned(),
                status: equipped(room.selected_cape.is_none(), ""),
                picture: None,
                equipped: room.selected_cape.is_none(),
                select: Some(command(Action::SelectCape(None))),
                edit: None,
            };
            let capes = [false, true].into_iter().flat_map(|imported| {
                room.capes
                    .iter()
                    .enumerate()
                    .filter(move |(_, cape)| cape.imported == imported)
            });
            std::iter::once(none)
                .chain(capes.map(|(index, cape)| {
                    Row {
                        kind: Kind::Cape(Some(index)),
                        title: cape.name.clone(),
                        about: if cape.imported {
                            "Custom cape"
                        } else {
                            "Minecraft cape"
                        }
                        .to_owned(),
                        status: equipped(room.selected_cape == Some(index), ""),
                        picture: Some(cape_thumbnail_key(&cape.id)),
                        equipped: room.selected_cape == Some(index),
                        select: Some(command(Action::SelectCape(Some(index)))),
                        edit: cape
                            .imported
                            .then(|| command(Action::BeginRenameCape(index))),
                    }
                }))
                .collect()
        }
        DressingRoomSection::Cosmetics => cosmetics::Item::all()
            .into_iter()
            .enumerate()
            .filter(|(_, item)| {
                CosmeticCategory::All.includes_slot(&item.descriptor().slot)
                    && room
                        .cosmetic_category
                        .includes_slot(&item.descriptor().slot)
            })
            .map(|(index, item)| {
                let owned = item.is_owned();
                let worn = owned
                    && room
                        .selected_cosmetics
                        .get(&item.descriptor().slot)
                        .and_then(|value| value.as_deref())
                        == Some(item.id());
                Row {
                    kind: Kind::Cosmetic(index),
                    title: item.name().to_owned(),
                    about: item.descriptor().slot.clone(),
                    status: if worn {
                        "Equipped".to_owned()
                    } else if owned {
                        String::new()
                    } else {
                        "Locked".to_owned()
                    },
                    picture: Some(item.thumbnail_key()),
                    equipped: worn,
                    select: owned.then(|| command(Action::SelectCosmetic(Some(item)))),
                    edit: None,
                }
            })
            .collect(),
    }
}

/// Draws the Java Skin Presets screen and returns where the equipped character stands and which
/// pictures the list shows.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<PreviewArea, PaintError> {
    let room = &view.dressing_room;
    let rows = rows(view);
    let (import, import_action) = match room.section {
        DressingRoomSection::Skins => ("Import Skin", Some(command(Action::Import))),
        DressingRoomSection::Capes => ("Import Cape", Some(command(Action::ImportCape))),
        DressingRoomSection::Cosmetics => {
            ("Remove All", Some(command(Action::SelectCosmetic(None))))
        }
    };
    let busy = room.busy;
    let gallery = view.skin_gallery;
    let first = [
        Btn::new(import, import_action.filter(|_| !busy)),
        Btn::new(
            if gallery {
                "View: Gallery"
            } else {
                "View: List"
            },
            Some(MenuAction::SetSkinGallery(!gallery)),
        ),
    ];
    let second = [Btn::new("Done", Some(MenuAction::AddBack))];
    let spec = Spec {
        title: "Skin Presets",
        subtitle: None,
        header: HEADER,
        rows: &[&first, &second],
        more: None,
        title_top: None,
        footer_gap: None,
    };
    let mut preview = PreviewArea {
        control: [-4.0, -4.0, -2.0, -2.0],
        clip: [-4.0, -4.0, -2.0, -2.0],
        visible_skins: Vec::new(),
        visible_capes: Vec::new(),
        visible_cosmetics: Vec::new(),
        filter_anchor: None,
    };
    frame::draw(canvas, view, size, &spec, |canvas, frame| {
        tabs(canvas, view, frame.centre, frame.unit)?;
        if gallery {
            // Cards carry the looks' pictures themselves, so no character stands beside them.
            let new = import_action
                .filter(|_| !busy && room.section != DressingRoomSection::Cosmetics)
                .map(|action| (import, action));
            return cards(canvas, view, frame, &rows, new, &mut preview);
        }
        entries(canvas, view, frame, &rows, &mut preview)?;
        let unit = frame.unit;
        let left = frame.centre + (ENTRY_WIDTH / 2.0).floor() + STAGE_GAP;
        let width = size[0] / unit - left - 4.0;
        if width >= STAGE_MIN && room.section != DressingRoomSection::Cosmetics {
            let stage = [
                left * unit,
                frame.viewport[1] + 8.0 * unit,
                (left + width.min(STAGE_WIDTH)) * unit,
                frame.viewport[3] - 8.0 * unit,
            ];
            preview.control = stage;
            preview.clip = stage;
        }
        Ok(())
    })?;
    Ok(preview)
}

/// The tabs under the title, the open one lit.
fn tabs(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    centre: f32,
    unit: f32,
) -> Result<(), PaintError> {
    let count = DressingRoomSection::ALL.len() as f32;
    let total = count * TAB_WIDTH + (count - 1.0) * TAB_GAP;
    for (place, section) in DressingRoomSection::ALL.into_iter().enumerate() {
        let x = centre - total / 2.0 + place as f32 * (TAB_WIDTH + TAB_GAP);
        let bounds = [x, TABS_TOP, x + TAB_WIDTH, TABS_TOP + BUTTON_HEIGHT].map(|v| v * unit);
        button::draw_lit(
            canvas,
            view,
            bounds,
            section.label(),
            Some(command(Action::SetSection(section))),
            section == view.dressing_room.section,
            unit,
        )?;
    }
    Ok(())
}

/// The scrolling list of looks.
fn entries(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    frame: &Frame,
    rows: &[Row],
    preview: &mut PreviewArea,
) -> Result<(), PaintError> {
    let unit = frame.unit;
    let content = (PADDING * 2.0 + rows.len().max(1) as f32 * ENTRY_STEP) * unit;
    let left = (frame.centre - (ENTRY_WIDTH / 2.0).floor()) * unit;
    let key = format!("java_skins_{:?}", view.dressing_room.section);
    frame::scrolled(
        canvas,
        &key,
        frame.viewport,
        content,
        (frame.centre + BAR_OFFSET) * unit,
        unit,
        |canvas, offset| {
            let top = frame.viewport[1] - offset + PADDING * unit;
            for (place, row) in rows.iter().enumerate() {
                let y = top + place as f32 * ENTRY_STEP * unit;
                let bounds = [
                    left,
                    y,
                    left + ENTRY_WIDTH * unit,
                    y + (ENTRY_STEP - 4.0) * unit,
                ];
                if bounds[3] < frame.viewport[1] || bounds[1] > frame.viewport[3] {
                    continue;
                }
                match row.kind {
                    Kind::Skin(index) => preview.visible_skins.push(index),
                    Kind::Cape(Some(index)) => preview.visible_capes.push(index),
                    Kind::Cape(None) => {}
                    Kind::Cosmetic(index) => preview.visible_cosmetics.push(index),
                }
                entry(canvas, view, row, bounds, unit)?;
            }
            Ok(())
        },
    )
}

/// The cards' sizes in Java pixels: the card, the gap between cards, and the picture's height and
/// the height of the button at the card's foot.
const CARD_WIDTH: f32 = 76.0;
const CARD_HEIGHT: f32 = 112.0;
const CARD_GAP: f32 = 6.0;
const CARD_PICTURE: f32 = 62.0;
const CARD_BUTTON: f32 = 16.0;
const CARD_EDGE: [u8; 4] = [64, 64, 64, 255];
const CARD_LIT: [u8; 4] = [24, 24, 24, 255];

/// The looks as large cards, side by side in as many columns as fit, with a card at the end that
/// adds a new one.
fn cards(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    frame: &Frame,
    rows: &[Row],
    new: Option<(&str, MenuAction)>,
    preview: &mut PreviewArea,
) -> Result<(), PaintError> {
    let unit = frame.unit;
    let width = (frame.viewport[2] - frame.viewport[0]) / unit - 2.0 * (PADDING + 8.0);
    let columns = (((width + CARD_GAP) / (CARD_WIDTH + CARD_GAP)).floor() as usize).max(1);
    let total = rows.len() + usize::from(new.is_some());
    let lines = total.div_ceil(columns).max(1);
    let used = columns as f32 * CARD_WIDTH + (columns as f32 - 1.0) * CARD_GAP;
    let left = (frame.centre - used / 2.0).floor();
    let content = (PADDING * 2.0 + lines as f32 * (CARD_HEIGHT + CARD_GAP)) * unit;
    let key = format!("java_skin_cards_{:?}", view.dressing_room.section);
    frame::scrolled(
        canvas,
        &key,
        frame.viewport,
        content,
        (frame.centre + used / 2.0 + 4.0) * unit,
        unit,
        |canvas, offset| {
            let top = frame.viewport[1] - offset + PADDING * unit;
            for place in 0..total {
                let x = left + (place % columns) as f32 * (CARD_WIDTH + CARD_GAP);
                let y = (place / columns) as f32 * (CARD_HEIGHT + CARD_GAP);
                let bounds = [
                    x * unit,
                    top + y * unit,
                    (x + CARD_WIDTH) * unit,
                    top + (y + CARD_HEIGHT) * unit,
                ];
                if bounds[3] < frame.viewport[1] || bounds[1] > frame.viewport[3] {
                    continue;
                }
                match rows.get(place) {
                    Some(row) => {
                        match row.kind {
                            Kind::Skin(index) => preview.visible_skins.push(index),
                            Kind::Cape(Some(index)) => preview.visible_capes.push(index),
                            Kind::Cape(None) => {}
                            Kind::Cosmetic(index) => preview.visible_cosmetics.push(index),
                        }
                        card(canvas, view, row, bounds, unit)?;
                    }
                    None => {
                        if let Some((label, action)) = new {
                            new_card(canvas, view, label, action, bounds, unit)?;
                        }
                    }
                }
            }
            Ok(())
        },
    )
}

/// A card's frame: an edge with the dark face inside it, lit while `lit`.
fn card_face(
    canvas: &mut Canvas<'_>,
    bounds: Bounds,
    edge: [u8; 4],
    lit: bool,
    unit: f32,
) -> Result<(), PaintError> {
    canvas.fill(bounds, edge)?;
    canvas.fill(
        [
            bounds[0] + unit,
            bounds[1] + unit,
            bounds[2] - unit,
            bounds[3] - unit,
        ],
        if lit { CARD_LIT } else { BLACK },
    )
}

/// `text` centred across `bounds`, `y` Java pixels down.
fn centred(
    canvas: &mut Canvas<'_>,
    bounds: Bounds,
    text: &str,
    y: f32,
    colour: [u8; 4],
    unit: f32,
) -> Result<(), PaintError> {
    let room = bounds[2] - bounds[0] - 8.0 * unit;
    let width = super::text_width(text).map_or(room, |width| (width * unit).min(room));
    put(
        canvas,
        text,
        [(bounds[0] + bounds[2] - width) / 2.0, bounds[1] + y * unit],
        unit,
        colour,
        room,
    )
}

/// One look's card: its name over a large picture, what it is, and Edit when it can be edited.
/// Boxed in grey while equipped, lit under the pointer.
fn card(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    row: &Row,
    bounds: Bounds,
    unit: f32,
) -> Result<(), PaintError> {
    let state = canvas.interaction(view, row.select);
    let edge = if row.equipped { GREY } else { CARD_EDGE };
    card_face(canvas, bounds, edge, state.hovered || state.focused, unit)?;
    centred(canvas, bounds, &row.title, 4.0, TEXT, unit)?;
    let picture = [
        bounds[0] + 4.0 * unit,
        bounds[1] + 15.0 * unit,
        bounds[2] - 4.0 * unit,
        bounds[1] + (15.0 + CARD_PICTURE) * unit,
    ];
    canvas.fill(picture, ICON_FILL)?;
    if let Some(art) = row
        .picture
        .as_ref()
        .and_then(|key| canvas.artwork?.get(key))
        .copied()
    {
        contain(canvas, art, picture)?;
    }
    let (status, colour) = if row.equipped {
        ("Equipped", GREEN)
    } else {
        (row.about.as_str(), GREY)
    };
    centred(
        canvas,
        bounds,
        status,
        15.0 + CARD_PICTURE + 4.0,
        colour,
        unit,
    )?;
    if let Some(select) = row.select {
        canvas.hit(select, bounds)?;
    }
    if let Some(edit) = row.edit {
        let top = bounds[3] - (CARD_BUTTON + 4.0) * unit;
        button::draw(
            canvas,
            view,
            [
                bounds[0] + 4.0 * unit,
                top,
                bounds[2] - 4.0 * unit,
                top + CARD_BUTTON * unit,
            ],
            "Edit",
            Some(edit),
            unit,
        )?;
    }
    Ok(())
}

/// The card at the end that adds a look: a plus over its label.
fn new_card(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    label: &str,
    action: MenuAction,
    bounds: Bounds,
    unit: f32,
) -> Result<(), PaintError> {
    let state = canvas.interaction(view, Some(action));
    let lit = state.hovered || state.focused;
    card_face(canvas, bounds, CARD_EDGE, lit, unit)?;
    let colour = if lit { TEXT } else { GREY };
    let middle = [
        (bounds[0] + bounds[2]) / 2.0,
        (bounds[1] + bounds[3]) / 2.0 - 6.0 * unit,
    ];
    for bar in [[8.0, 1.0], [1.0, 8.0]] {
        canvas.fill(
            [
                middle[0] - bar[0] * unit,
                middle[1] - bar[1] * unit,
                middle[0] + bar[0] * unit,
                middle[1] + bar[1] * unit,
            ],
            colour,
        )?;
    }
    centred(canvas, bounds, label, CARD_HEIGHT - 22.0, colour, unit)?;
    canvas.hit(action, bounds)
}

/// One look's entry: boxed while equipped, lit under the pointer.
fn entry(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    row: &Row,
    bounds: Bounds,
    unit: f32,
) -> Result<(), PaintError> {
    let state = canvas.interaction(view, row.select);
    if row.equipped {
        let outer = bounds.map(|v| v);
        let grow = [-2.0, -2.0, 2.0, 2.0].map(|v| v * unit);
        let outer = [
            outer[0] + grow[0],
            outer[1] + grow[1],
            outer[2] + grow[2],
            outer[3] + grow[3],
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
        .picture
        .as_ref()
        .and_then(|key| canvas.artwork?.get(key))
        .copied()
    {
        contain(canvas, art, icon)?;
    }
    let text_left = bounds[0] + (ICON + 3.0) * unit;
    let edit_room = if row.edit.is_some() {
        (EDIT_WIDTH + 8.0) * unit
    } else {
        0.0
    };
    let room = bounds[2] - text_left - edit_room;
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
        &row.status,
        [text_left, bounds[1] + 21.0 * unit],
        unit,
        if row.equipped { GREEN } else { GREY },
        room,
    )?;
    if let Some(select) = row.select {
        canvas.hit(select, bounds)?;
    }
    if let Some(edit) = row.edit {
        let height = EDIT_HEIGHT * unit;
        let top = bounds[1] + (bounds[3] - bounds[1] - height) / 2.0;
        let button_bounds = [
            bounds[2] - (EDIT_WIDTH + 4.0) * unit,
            top,
            bounds[2] - 4.0 * unit,
            top + height,
        ];
        button::draw(canvas, view, button_bounds, "Edit", Some(edit), unit)?;
    }
    Ok(())
}

/// `icon` fitted inside `bounds` whole, centred.
pub(super) fn contain(
    canvas: &mut Canvas<'_>,
    icon: ui::IconRef,
    bounds: Bounds,
) -> Result<(), PaintError> {
    let width = f32::from(icon.uv[2] - icon.uv[0]);
    let height = f32::from(icon.uv[3] - icon.uv[1]);
    if width <= 0.0 || height <= 0.0 {
        return Ok(());
    }
    let scale = ((bounds[2] - bounds[0]) / width).min((bounds[3] - bounds[1]) / height);
    let (w, h) = (width * scale, height * scale);
    let left = (bounds[0] + bounds[2] - w) / 2.0;
    let top = (bounds[1] + bounds[3] - h) / 2.0;
    canvas.icon_ref(icon, [left, top, left + w, top + h])
}

/// The page that edits one imported look, drawn over the list: its name, for a skin its arms, a
/// button that removes it, and Done and Cancel. Removing asks first.
pub(crate) fn draw_editor(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<(), PaintError> {
    let Some(editor) = view.dressing_room.editor.as_ref() else {
        return Ok(());
    };
    let cape = editor.target == SkinEditorTarget::Cape;
    let busy = view.dressing_room.busy;
    let rename = editor.mode == SkinEditorMode::Rename;
    let done = [
        Btn::new(
            "Done",
            (rename && !busy && !editor.draft.trim().is_empty())
                .then_some(command(Action::SaveRename)),
        ),
        Btn::new("Cancel", Some(command(Action::Cancel))),
    ];
    let spec = Spec {
        title: if cape { "Edit Cape" } else { "Edit Skin" },
        subtitle: None,
        header: frame::HEADER,
        rows: &[&done],
        more: None,
        title_top: None,
        footer_gap: None,
    };
    frame::draw(canvas, view, size, &spec, |canvas, frame| {
        let unit = frame.unit;
        let java_width = (size[0] / unit).ceil();
        list(
            canvas,
            view,
            &|_| None,
            frame.viewport,
            java_width,
            &editor_entries(view, editor),
            "java_skin_edit",
            unit,
        )
    })?;
    if !rename {
        let (what, body) = if cape {
            (
                "cape",
                "This removes the imported cape from your collection. You can import it again later.",
            )
        } else {
            (
                "skin",
                "This removes the imported skin from your collection. You can import it again later.",
            )
        };
        let title = format!("Remove this {what}?");
        let modal = crate::modal::Modal {
            buttons: vec![
                (
                    "Cancel".into(),
                    crate::widgets::Variant::Secondary,
                    Some(command(Action::Cancel)),
                ),
                (
                    "Remove".into(),
                    crate::widgets::Variant::Destructive,
                    (!busy).then_some(command(Action::ConfirmDelete)),
                ),
            ],
            close: Some(command(Action::Cancel)),
            ..crate::modal::Modal::text(&title, body)
        };
        super::modal::draw(canvas, view, size, &modal)?;
    }
    Ok(())
}

/// The widgets of the edit page.
fn editor_entries(view: &MenuView, editor: &SkinEditor) -> Vec<Entry> {
    let room = &view.dressing_room;
    let mut found = vec![Entry::Field {
        label: if editor.target == SkinEditorTarget::Cape {
            "Cape Name".to_owned()
        } else {
            "Skin Name".to_owned()
        },
        value: editor.draft.clone(),
        placeholder: "Name".to_owned(),
        field: MenuField::SkinName,
        press: MenuAction::EditSkinName,
    }];
    if editor.target == SkinEditorTarget::Skin {
        let skin = room.skins.get(editor.index);
        match skin.map(|skin| skin.model) {
            Some(model @ (SkinModel::Classic | SkinModel::Slim))
                if room.selected == Some(editor.index) =>
            {
                let (label, next) = if model == SkinModel::Slim {
                    ("Arms: Slim", SkinModel::Classic)
                } else {
                    ("Arms: Classic", SkinModel::Slim)
                };
                found.push(Entry::Link(
                    label.to_owned(),
                    command(Action::SetModel(next)),
                ));
            }
            Some(SkinModel::Custom) | None => {}
            Some(_) => found.push(Entry::Label(
                "Equip this skin to change its arms".to_owned(),
            )),
        }
    }
    found.push(Entry::Link(
        if editor.target == SkinEditorTarget::Cape {
            "Remove Cape".to_owned()
        } else {
            "Remove Skin".to_owned()
        },
        command(if editor.target == SkinEditorTarget::Cape {
            Action::BeginDeleteCape(editor.index)
        } else {
            Action::BeginDelete(editor.index)
        }),
    ));
    found
}
