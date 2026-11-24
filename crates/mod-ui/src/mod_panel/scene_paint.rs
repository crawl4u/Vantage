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
    /// The game's icon of an item by identifier and metadata, from the loaded pack.
    pub icon: &'a dyn Fn(&str, u32) -> Option<ui::IconRef>,
}

/// The clips a scene is drawn inside, innermost last. A clip that nothing shows through is kept (so the pushes and
/// pops stay paired) but never made a group, since the engine refuses an inverted rectangle.
struct ClipStack {
    stack: Vec<[f32; 4]>,
}

impl ClipStack {
    fn new(base: [f32; 4]) -> Self {
        Self { stack: vec![base] }
    }

    /// The clip in force: the innermost one.
    fn current(&self) -> [f32; 4] {
        self.stack.last().copied().unwrap_or_default()
    }

    /// Whether something shows through `clip`.
    fn shows(clip: [f32; 4]) -> bool {
        clip[0] < clip[2] && clip[1] < clip[3]
    }

    /// Whether nothing shows through the clip in force.
    fn hidden(&self) -> bool {
        !Self::shows(self.current())
    }

    /// Enters `inner` (left, top, right, bottom) inside the clip in force. Returns the clip to make a group of, when
    /// something shows through it.
    fn push(&mut self, inner: [f32; 4]) -> Option<[f32; 4]> {
        let outer = self.current();
        let clip = [inner[0].max(outer[0]), inner[1].max(outer[1]), inner[2].min(outer[2]), inner[3].min(outer[3])];
        self.stack.push(clip);
        Self::shows(clip).then_some(clip)
    }

    /// Leaves the innermost clip. Returns the enclosing clip to make a group of again, when something shows through it.
    fn pop(&mut self) -> Option<[f32; 4]> {
        if self.stack.len() > 1 {
            self.stack.pop();
        }
        let back = self.current();
        Self::shows(back).then_some(back)
    }
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

/// The outline of a rounded rectangle `dest` (left, top, right, bottom) with corner `radius`, grown by `grow` all
/// round, as points in order.
fn rounded_outline(dest: [f32; 4], radius: f32, grow: f32) -> Vec<[f32; 2]> {
    const STEPS: usize = 8;
    let r = (radius + grow).max(0.0).min((dest[2] - dest[0]) / 2.0).min((dest[3] - dest[1]) / 2.0);
    let [l, t, rt, b] = [dest[0] - grow, dest[1] - grow, dest[2] + grow, dest[3] + grow];
    let corners = [([rt - r, t + r], -90.0_f32), ([rt - r, b - r], 0.0), ([l + r, b - r], 90.0), ([l + r, t + r], 180.0)];
    let mut points = Vec::with_capacity(corners.len() * (STEPS + 1));
    for (centre, start) in corners {
        for step in 0..=STEPS {
            let angle = (start + 90.0 * step as f32 / STEPS as f32).to_radians();
            points.push([centre[0] + r * angle.cos(), centre[1] + r * angle.sin()]);
        }
    }
    points
}

/// A sprite laid on a rounded rectangle: the same texels over `dest`, with the corners cut to `radius` and the edge
/// faded over one pixel. `None` for a visual that is not a plain sprite.
fn rounded_sprite(visual: &UiVisual, dest: [f32; 4], radius: f32) -> Option<(Arc<UiMesh>, [f32; 4])> {
    let UiVisual::Sprite { texture_page, uv, color } = visual else {
        return None;
    };
    let feather = 0.75;
    let inner = rounded_outline(dest, radius, -feather * 0.5);
    let outer = rounded_outline(dest, radius, feather * 0.5);
    let bounds = [dest[0] - feather, dest[1] - feather, dest[2] + feather, dest[3] + feather];
    let (w, h) = (bounds[2] - bounds[0], bounds[3] - bounds[1]);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let vertex = |p: [f32; 2], alpha: u8| {
        let across = ((p[0] - dest[0]) / (dest[2] - dest[0])).clamp(0.0, 1.0);
        let down = ((p[1] - dest[1]) / (dest[3] - dest[1])).clamp(0.0, 1.0);
        UiMeshVertex {
            position: [(p[0] - bounds[0]) / w, (p[1] - bounds[1]) / h],
            clip_z: 0.0,
            clip_w: 1.0,
            uv: [f32::from(uv[0]) + across * f32::from(uv[2] - uv[0]), f32::from(uv[1]) + down * f32::from(uv[3] - uv[1])],
            color: [color[0], color[1], color[2], alpha],
            model_light: 1.0,
            overlay_color: [0.0; 4],
            style_flags: 0,
            terrain_mips: false,
            alpha_test: false,
        }
    };
    let solid = color[3];
    let mut vertices: Vec<UiMeshVertex> = Vec::with_capacity(inner.len() * 2 + 1);
    vertices.push(vertex([(dest[0] + dest[2]) / 2.0, (dest[1] + dest[3]) / 2.0], solid));
    vertices.extend(inner.iter().map(|p| vertex(*p, solid)));
    vertices.extend(outer.iter().map(|p| vertex(*p, 0)));
    let n = inner.len() as u32;
    let mut indices: Vec<u32> = Vec::new();
    for i in 0..n {
        let next = (i + 1) % n;
        // The fan over the inner outline, then the fringe between the two outlines.
        indices.extend([0, 1 + i, 1 + next]);
        indices.extend([1 + i, 1 + n + i, 1 + next]);
        indices.extend([1 + next, 1 + n + i, 1 + n + next]);
    }
    let batch = UiMeshBatch {
        texture_page: *texture_page,
        index_range: 0..indices.len() as u32,
        blend: UiBlendMode::Alpha,
        depth_test: false,
        depth_write: false,
        alpha_cutoff: None,
    };
    let mesh = UiMesh::new(vertices.into(), indices.into(), vec![batch].into()).ok()?;
    Some((Arc::new(mesh), bounds))
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
        let mut clips = ClipStack::new(base_clip);
        let mut fades = vec![1.0_f32];
        canvas.group(base_clip)?;
        for primitive in &scene.primitives {
            let fade = *fades.last().unwrap_or(&1.0);
            // Inside a clip that nothing shows through (a card scrolled out of view) there is nothing to draw, and
            // a text or shape given an inverted clip would fail the whole panel.
            let hidden = clips.hidden();
            if hidden && !matches!(primitive, Primitive::PushClip { .. } | Primitive::PopClip | Primitive::PushOpacity(_) | Primitive::PopOpacity) {
                continue;
            }
            match primitive {
                Primitive::Rect { .. } | Primitive::Path { .. } | Primitive::Pixels { .. } | Primitive::Line { .. } => {
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
                        clips.current(),
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
                        clips.current(),
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
                Primitive::Item { rect, id } => {
                    if let Some(icon) = (self.icon)(id, 0).filter(|icon| icon.uv[0] < icon.uv[2] && icon.uv[1] < icon.uv[3]) {
                        let dest = [origin[0] + rect.x * scale, origin[1] + rect.y * scale, origin[0] + rect.right() * scale, origin[1] + rect.bottom() * scale];
                        canvas.push(icon.visual([255, 255, 255, (fade * 255.0).round() as u8]), dest)?;
                    }
                }
                Primitive::PlayerHead { rect, radius } => {
                    let dest = [origin[0] + rect.x * scale, origin[1] + rect.y * scale, origin[0] + rect.right() * scale, origin[1] + rect.bottom() * scale];
                    let color = [255, 255, 255, (fade * 255.0).round() as u8];
                    if let Some(visual) = canvas.sprite(ui_engine::atlas::MENU_PLAYER_HEAD, json_ui::UvRect::full(), color, json_ui::SpriteFilter::default()) {
                        // The picture is a square; it is laid on a rounded outline so its corners follow the box's.
                        match rounded_sprite(&visual, dest, radius * scale) {
                            Some((mesh, bounds)) => canvas.push(UiVisual::Mesh(mesh), bounds)?,
                            None => canvas.push(visual, dest)?,
                        }
                    }
                }
                Primitive::Texture { rect, path } => {
                    let dest = [origin[0] + rect.x * scale, origin[1] + rect.y * scale, origin[0] + rect.right() * scale, origin[1] + rect.bottom() * scale];
                    let color = [255, 255, 255, (fade * 255.0).round() as u8];
                    if let Some(visual) = canvas.sprite(path, json_ui::UvRect::full(), color, json_ui::SpriteFilter::default()) {
                        canvas.push(visual, dest)?;
                    }
                }
                Primitive::PushClip { rect, .. } => {
                    let inner = [origin[0] + rect.x * scale, origin[1] + rect.y * scale, origin[0] + rect.right() * scale, origin[1] + rect.bottom() * scale];
                    if let Some(clip) = clips.push(inner) {
                        canvas.group(clip)?;
                    }
                }
                Primitive::PopClip => {
                    if let Some(clip) = clips.pop() {
                        canvas.group(clip)?;
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaving_a_clip_nothing_shows_through_does_not_ask_for_a_group_of_it() {
        let mut clips = ClipStack::new([0.0, 0.0, 100.0, 100.0]);
        assert_eq!(clips.push([10.0, 10.0, 60.0, 60.0]), Some([10.0, 10.0, 60.0, 60.0]));
        // A clip wholly outside the one in force shows nothing, nor does anything inside it.
        assert_eq!(clips.push([70.0, 70.0, 90.0, 90.0]), None);
        assert!(clips.hidden());
        assert_eq!(clips.push([75.0, 75.0, 80.0, 80.0]), None);
        // Back out of the innermost, still inside the hidden one: no group, not an inverted one.
        assert_eq!(clips.pop(), None);
        assert!(clips.hidden());
        // Out of the hidden one, back in the first: its group again.
        assert_eq!(clips.pop(), Some([10.0, 10.0, 60.0, 60.0]));
        assert!(!clips.hidden());
        assert_eq!(clips.pop(), Some([0.0, 0.0, 100.0, 100.0]));
        // An extra pop stays on the base.
        assert_eq!(clips.pop(), Some([0.0, 0.0, 100.0, 100.0]));
    }
}
