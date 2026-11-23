//! Paints a first-party scene (see `vantage-gui`) directly: shapes as tessellated, vertex-coloured triangles,
//! text and icons as ordinary glyph runs. The document only places one `vantage_scene` node; the scene is
//! not a document, so the mod-panel size, node and binding limits do not apply to it.

use std::sync::Arc;

use ui::{UiBlendMode, UiMesh, UiMeshBatch, UiMeshVertex, UiVisual};
use ui_engine::engine::{
    Canvas,
    screen::{CustomDraw, CustomRenderer, PaintError},
    text_paint::TextPaint,
};
use vantage_gui::{
    geom::Rect,
    scene::{Align, Primitive, Scene},
    style::Color,
    tess::{self, Mesh},
};

/// The renderer name of the node a scene is painted in.
pub(crate) const RENDERER: &str = ui::mod_panel::SCENE_RENDERER;

/// How many font pixels one unit of a text size is (the engine's natural glyph height is nine).
const NATURAL_GLYPH: f32 = 9.0;

pub(super) struct SceneRenderer<'a> {
    pub scene: Option<&'a Scene>,
}

fn bytes(c: Color) -> [u8; 4] {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [q(c.r), q(c.g), q(c.b), q(c.a)]
}

/// The engine geometry of a tessellated mesh, placed in `origin` (the node's top left) and normalised to the
/// mesh's own bounds, as the engine expects of a node's mesh. `None` for an empty or oversized mesh.
fn engine_mesh(mesh: &Mesh, origin: [f32; 2], scale: f32, fade: f32, solid_page: u16) -> Option<(Arc<UiMesh>, [f32; 4])> {
    let bounds = mesh.bounds()?;
    let bounds = Rect::new(bounds.x * scale, bounds.y * scale, bounds.w * scale, bounds.h * scale);
    if bounds.w <= 0.0 || bounds.h <= 0.0 || mesh.indices.is_empty() {
        return None;
    }
    let vertices: Vec<UiMeshVertex> = mesh
        .vertices
        .iter()
        .map(|v| UiMeshVertex {
            position: [(v.pos[0] * scale - bounds.x) / bounds.w, (v.pos[1] * scale - bounds.y) / bounds.h],
            clip_z: 0.0,
            clip_w: 1.0,
            uv: [0.5, 0.5],
            color: [v.color[0], v.color[1], v.color[2], (f32::from(v.color[3]) * fade).round() as u8],
            model_light: 1.0,
            overlay_color: [0.0; 4],
            style_flags: 0,
            terrain_mips: false,
            alpha_test: false,
        })
        .collect();
    let batch = UiMeshBatch {
        texture_page: solid_page,
        index_range: 0..mesh.indices.len() as u32,
        blend: UiBlendMode::Alpha,
        depth_test: false,
        depth_write: false,
        alpha_cutoff: None,
    };
    let built = UiMesh::new(vertices.into(), mesh.indices.clone().into(), vec![batch].into()).ok()?;
    Some((Arc::new(built), [origin[0] + bounds.x, origin[1] + bounds.y, origin[0] + bounds.right(), origin[1] + bounds.bottom()]))
}

fn text_options(font: &str) -> json_ui::TextOptions {
    json_ui::TextOptions { font_type: Some(font.to_owned()), hide_hyphen: true, ..Default::default() }
}

impl CustomRenderer for SceneRenderer<'_> {
    fn paint(&mut self, node: &CustomDraw<'_>, canvas: &mut Canvas<'_>) -> Result<bool, PaintError> {
        if node.renderer != RENDERER {
            return Ok(false);
        }
        let Some(scene) = self.scene else {
            return Ok(true);
        };
        let origin = [node.dest[0], node.dest[1]];
        // The scene is in the panel's units; one unit is this many logical pixels.
        let scale = canvas.px();
        // Edges fade over about one screen pixel, so shapes stay crisp at any scale.
        tess::set_feather(1.0 / scale.max(1e-3));
        let base_clip = canvas.clip().unwrap_or(canvas.screen());
        let mut clips = vec![base_clip];
        let mut fades = vec![1.0_f32];
        canvas.group(base_clip)?;
        for primitive in &scene.primitives {
            let fade = *fades.last().unwrap_or(&1.0);
            match primitive {
                Primitive::Rect { .. } | Primitive::Path { .. } => {
                    if let Some(mesh) = tess::primitive(primitive)
                        && let Some((mesh, bounds)) = engine_mesh(&mesh, origin, scale, fade, canvas.solid_page())
                    {
                        canvas.push(UiVisual::Mesh(mesh), bounds)?;
                    }
                }
                Primitive::Text { rect, text, size, color, align, font } => {
                    let dest = [origin[0] + rect.x * scale, origin[1] + rect.y * scale, origin[0] + rect.right() * scale, origin[1] + rect.bottom() * scale];
                    let options = text_options(font);
                    let align = match align {
                        Align::Start => json_ui::TextAlign::Left,
                        Align::Centre => json_ui::TextAlign::Center,
                        Align::End => json_ui::TextAlign::Right,
                    };
                    let mut c = bytes(*color);
                    c[3] = (f32::from(c[3]) * fade).round() as u8;
                    canvas.text(
                        text,
                        dest,
                        *clips.last().unwrap_or(&base_clip),
                        TextPaint {
                            color: c,
                            edit: None,
                            shadow: ui::TextShadow::None,
                            align,
                            scale: size / NATURAL_GLYPH,
                            localize: false,
                            options: &options,
                        },
                    )?;
                }
                Primitive::Icon { rect, glyph, size, color } => {
                    let dest = [origin[0] + rect.x * scale, origin[1] + rect.y * scale, origin[0] + rect.right() * scale, origin[1] + rect.bottom() * scale];
                    let options = text_options(ui::mod_panel::FONT_NAME);
                    let mut c = bytes(*color);
                    c[3] = (f32::from(c[3]) * fade).round() as u8;
                    canvas.text(
                        &glyph.to_string(),
                        dest,
                        *clips.last().unwrap_or(&base_clip),
                        TextPaint {
                            color: c,
                            edit: None,
                            shadow: ui::TextShadow::None,
                            align: json_ui::TextAlign::Center,
                            scale: size / NATURAL_GLYPH,
                            localize: false,
                            options: &options,
                        },
                    )?;
                }
                Primitive::PushClip { rect, .. } => {
                    let outer = *clips.last().unwrap_or(&base_clip);
                    let inner = Rect::new(origin[0] + rect.x * scale, origin[1] + rect.y * scale, rect.w * scale, rect.h * scale);
                    let clip = [
                        inner.x.max(outer[0]),
                        inner.y.max(outer[1]),
                        inner.right().min(outer[2]),
                        inner.bottom().min(outer[3]),
                    ];
                    clips.push(clip);
                    if clip[0] < clip[2] && clip[1] < clip[3] {
                        canvas.group(clip)?;
                    }
                }
                Primitive::PopClip => {
                    if clips.len() > 1 {
                        clips.pop();
                    }
                    canvas.group(*clips.last().unwrap_or(&base_clip))?;
                }
                Primitive::PushOpacity(a) => fades.push(fade * a.clamp(0.0, 1.0)),
                Primitive::PopOpacity => {
                    if fades.len() > 1 {
                        fades.pop();
                    }
                }
            }
        }
        canvas.group(base_clip)?;
        Ok(true)
    }
}
