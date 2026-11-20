//! The Java-style Shader Packs screen, laid out as the shader screens of Java mods are: a title
//! with "Select" under it, a list of rows (Off, then every pack) in which the chosen one is boxed,
//! a hint about dropping packs in, and two rows of buttons: add, open the folder and the pack's
//! settings, then delete, reload and done. Choosing a row loads that pack at once.

use launcher::{
    menu::{
        JavaSettingsAction, MenuAction, MenuView, RendererChoice,
        settings_sections::SettingSection,
        settings_storage::{StorageAction, StorageFolder},
        shader_pack_status::{PackIssue, pack_label},
    },
    shader_options::Action as ShaderAction,
};
use ui_engine::engine::screen::PaintError;

use super::frame::{self, Btn, Frame, Spec, put};
use crate::{
    paint::{Bounds, Canvas},
    theme::TEXT,
};

/// Sizes in Java pixels.
const ROW_WIDTH: f32 = 305.0;
const ROW_HEIGHT: f32 = 20.0;
const ROW_STEP: f32 = 24.0;
const PADDING: f32 = 6.0;
const BAR_OFFSET: f32 = 164.0;
const GREY: [u8; 4] = [128, 128, 128, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const FAILED: [u8; 4] = [255, 85, 85, 255];
const WARNING: [u8; 4] = [255, 170, 0, 255];

/// One row of the list.
struct Row<'a> {
    title: &'a str,
    /// What the row says about its pack on the right, with its colour.
    status: Option<(&'static str, [u8; 4])>,
    selected: bool,
    select: Option<MenuAction>,
}

/// Off, then every pack, with the chosen one marked.
fn rows(view: &MenuView) -> Vec<Row<'_>> {
    let active = view.render_mode == ui::RenderMode::ShaderPack;
    let removing = view.shader_pack_removing.as_deref();
    let status = match &view.renderer_issue {
        Some(PackIssue::Failure(_)) => ("Failed", FAILED),
        Some(PackIssue::Warning(_)) => ("Warnings", WARNING),
        None if view.shader_pack_loading => ("Loading...", GREY),
        None => ("Active", GREY),
    };
    let mut rows = vec![Row {
        title: "Off",
        status: None,
        selected: !active,
        select: Some(MenuAction::SelectRenderer(RendererChoice::Vanilla)),
    }];
    for (index, name) in view.shader_packs.iter().enumerate() {
        let Ok(index) = u16::try_from(index) else {
            continue;
        };
        let choice = RendererChoice::ShaderPack(index, view.shader_packs_revision);
        let selected = active && view.renderer == Some(choice);
        let busy = removing == Some(name.as_str());
        rows.push(Row {
            title: pack_label(name),
            status: if busy {
                Some(("Removing...", GREY))
            } else {
                selected.then_some(status)
            },
            selected,
            select: (!busy).then_some(MenuAction::SelectRenderer(choice)),
        });
    }
    rows
}

/// The footer's buttons for the chosen pack and what the host can do.
fn footer(view: &MenuView) -> (Vec<Btn<'static>>, Vec<Btn<'static>>) {
    let pack = view.render_mode == ui::RenderMode::ShaderPack && view.renderer.is_some();
    let locked = view.shader_pack_loading || view.shader_pack_removing.is_some();
    let add = Btn::new(
        if view.shader_pack_importing {
            "Adding..."
        } else {
            "Add Pack"
        },
        (!view.shader_pack_importing).then_some(MenuAction::ImportShaderPacks),
    );
    let folder = Btn::new(
        "Open Folder",
        view.storage
            .opens_folders
            .then_some(MenuAction::SettingsStorage(StorageAction::OpenFolder(
                StorageFolder::ShaderPacks,
            ))),
    );
    let settings = Btn::new(
        "Pack Settings",
        (pack && !locked).then_some(MenuAction::ShaderOptions(ShaderAction::Open)),
    );
    let remove = match view.renderer {
        Some(RendererChoice::ShaderPack(index, revision)) if pack => {
            Some(MenuAction::RemoveShaderPackDialog(index, revision))
        }
        _ => None,
    };
    let delete = Btn::new("Delete", remove.filter(|_| !locked));
    let reload = Btn::new(
        "Reload",
        (pack && !locked).then_some(MenuAction::ReloadShaders),
    );
    let done = Btn::new("Done", Some(MenuAction::AddBack));
    (vec![add, folder, settings], vec![delete, reload, done])
}

/// What the list says under its rows about adding packs.
fn hint(view: &MenuView) -> &'static str {
    if view.storage.accepts_file_drops {
        "+ Drag and Drop Shader Packs to add"
    } else {
        "+ Use Add Pack to add a .zip"
    }
}

/// Draws the Java Shader Packs screen.
pub(crate) fn draw(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    size: [f32; 2],
) -> Result<(), PaintError> {
    let (first, second) = footer(view);
    let more = MenuAction::JavaSettings(JavaSettingsAction::Bedrock(
        SettingSection::Shaders.definition().index,
    ));
    let spec = Spec {
        title: "Shader Packs",
        subtitle: Some("Select"),
        header: frame::HEADER,
        rows: &[&first, &second],
        more: Some(more),
        title_top: None,
    };
    frame::draw(canvas, view, size, &spec, |canvas, frame| {
        list(canvas, view, frame)
    })
}

/// The scrolling list of rows and the hint under it.
fn list(canvas: &mut Canvas<'_>, view: &MenuView, frame: &Frame) -> Result<(), PaintError> {
    let unit = frame.unit;
    let rows = rows(view);
    let content = (PADDING * 2.0 + rows.len() as f32 * ROW_STEP + ROW_HEIGHT) * unit;
    let left = (frame.centre - (ROW_WIDTH / 2.0).floor()) * unit;
    frame::scrolled(
        canvas,
        "java_shaders",
        frame.viewport,
        content,
        (frame.centre + BAR_OFFSET) * unit,
        unit,
        |canvas, offset| {
            let top = frame.viewport[1] - offset + PADDING * unit;
            for (index, row) in rows.iter().enumerate() {
                let y = top + index as f32 * ROW_STEP * unit;
                let bounds = [left, y, left + ROW_WIDTH * unit, y + ROW_HEIGHT * unit];
                entry(canvas, view, row, bounds, unit)?;
            }
            let y = top + rows.len() as f32 * ROW_STEP * unit;
            put(
                canvas,
                hint(view),
                [left + 4.0 * unit, y + 6.0 * unit],
                unit,
                GREY,
                ROW_WIDTH * unit,
            )
        },
    )
}

/// One row: boxed while chosen, lit under the pointer.
fn entry(
    canvas: &mut Canvas<'_>,
    view: &MenuView,
    row: &Row<'_>,
    bounds: Bounds,
    unit: f32,
) -> Result<(), PaintError> {
    let state = canvas.interaction(view, row.select);
    if row.selected {
        canvas.fill(bounds, GREY)?;
        canvas.fill(
            [
                bounds[0] + unit,
                bounds[1] + unit,
                bounds[2] - unit,
                bounds[3] - unit,
            ],
            BLACK,
        )?;
    } else if state.hovered || state.focused {
        canvas.fill(bounds, [255, 255, 255, 24])?;
    }
    let colour = if row.select.is_some() { TEXT } else { GREY };
    put(
        canvas,
        row.title,
        [bounds[0] + 4.0 * unit, bounds[1] + 6.0 * unit],
        unit,
        colour,
        (bounds[2] - bounds[0]) * 0.6,
    )?;
    if let Some((text, status)) = row.status {
        put(
            canvas,
            text,
            [bounds[2] - 70.0 * unit, bounds[1] + 6.0 * unit],
            unit,
            status,
            66.0 * unit,
        )?;
    }
    if let Some(action) = row.select {
        canvas.hit(action, bounds)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_is_chosen_until_a_pack_is_active() {
        let mut view = MenuView::new(true, "Player".into());
        view.shader_packs = vec!["One.zip".to_owned(), "Two".to_owned()].into();
        let listed = rows(&view);
        assert_eq!(listed.len(), 3);
        assert!(listed[0].selected && !listed[1].selected && !listed[2].selected);
        assert_eq!(
            listed[0].select,
            Some(MenuAction::SelectRenderer(RendererChoice::Vanilla))
        );
        // Settings, reload and delete wait for a pack.
        let (first, second) = footer(&view);
        let enabled = |label: &str| {
            first
                .iter()
                .chain(&second)
                .find(|button| button.label == label)
                .is_some_and(|button| button.action.is_some())
        };
        assert!(enabled("Add Pack"));
        assert!(!enabled("Pack Settings") && !enabled("Reload") && !enabled("Delete"));
    }

    #[test]
    fn the_active_pack_is_boxed_and_can_be_reloaded_deleted_and_configured() {
        let mut view = MenuView::new(true, "Player".into());
        view.shader_packs = vec!["One.zip".to_owned(), "Two".to_owned()].into();
        view.shader_packs_revision = 3;
        view.render_mode = ui::RenderMode::ShaderPack;
        view.renderer = Some(RendererChoice::ShaderPack(1, 3));
        let listed = rows(&view);
        assert!(!listed[0].selected && !listed[1].selected && listed[2].selected);
        assert_eq!(listed[2].status.map(|status| status.0), Some("Active"));
        let (first, second) = footer(&view);
        let find = |label: &str| {
            first
                .iter()
                .chain(&second)
                .find(|button| button.label == label)
                .and_then(|button| button.action)
        };
        assert_eq!(
            find("Delete"),
            Some(MenuAction::RemoveShaderPackDialog(1, 3))
        );
        assert_eq!(find("Reload"), Some(MenuAction::ReloadShaders));
        assert!(find("Pack Settings").is_some());
        // While it loads nothing that changes it is offered.
        view.shader_pack_loading = true;
        let (first, second) = footer(&view);
        assert!(
            first
                .iter()
                .chain(&second)
                .filter(|button| matches!(button.label, "Delete" | "Reload"))
                .all(|button| button.action.is_none())
        );
    }
}
