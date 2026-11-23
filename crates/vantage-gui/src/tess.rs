//! Tessellation: turns scene shapes into coloured triangles. Edges are feathered over one unit so they are
//! smooth at any scale; gradients are vertex colours; a shadow is a ring that fades out. The output is plain
//! data (positions in design units, colours as bytes), which the painter in the client turns into engine
//! geometry.

use std::f32::consts::{FRAC_PI_2, PI};

use crate::geom::Rect;
use crate::scene::Primitive;
use crate::style::{Border, Color, Fill, Radii, Shadow};

/// One corner point with its colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub color: [u8; 4],
}

/// Triangles to draw, back to front across calls.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

/// The feather on every edge, in design units.
pub const FEATHER: f32 = 1.0;
/// Segments along a corner arc of radius `r` (more for bigger corners).
fn arc_steps(r: f32) -> usize {
    ((r * 0.9).ceil() as usize).clamp(2, 24)
}

fn bytes(c: Color) -> [u8; 4] {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [q(c.r), q(c.g), q(c.b), q(c.a)]
}

impl Mesh {
    fn push(&mut self, pos: [f32; 2], color: [u8; 4]) -> u32 {
        self.vertices.push(Vertex { pos, color });
        (self.vertices.len() - 1) as u32
    }

    /// Appends another mesh.
    pub fn append(&mut self, other: Mesh) {
        let base = self.vertices.len() as u32;
        self.vertices.extend(other.vertices);
        self.indices.extend(other.indices.into_iter().map(|i| i + base));
    }

    /// The bounding box of the vertices, or `None` for an empty mesh.
    #[must_use]
    pub fn bounds(&self) -> Option<Rect> {
        let first = self.vertices.first()?;
        let (mut x0, mut y0, mut x1, mut y1) = (first.pos[0], first.pos[1], first.pos[0], first.pos[1]);
        for v in &self.vertices {
            x0 = x0.min(v.pos[0]);
            y0 = y0.min(v.pos[1]);
            x1 = x1.max(v.pos[0]);
            y1 = y1.max(v.pos[1]);
        }
        Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
    }
}

/// The outline of `rect` with `radii`, grown by `grow` (negative shrinks), clockwise from the top left.
fn outline(rect: Rect, radii: Radii, grow: f32) -> Vec<[f32; 2]> {
    let r = Rect::new(rect.x - grow, rect.y - grow, rect.w + grow * 2.0, rect.h + grow * 2.0);
    let radii = radii.fitted(rect.w, rect.h);
    let g = |v: f32| (v + grow).max(0.0);
    let (tl, tr, br, bl) = (g(radii.tl), g(radii.tr), g(radii.br), g(radii.bl));
    let mut out = Vec::new();
    // Every outline of a rectangle has the same number of points, so rings can join any two of them: the arc
    // steps come from the largest authored radius, and a square corner repeats its point.
    let steps = arc_steps(radii.tl.max(radii.tr).max(radii.br).max(radii.bl));
    let mut corner = |cx: f32, cy: f32, radius: f32, from: f32| {
        if radius <= 0.01 {
            out.extend(std::iter::repeat_n([cx, cy], steps + 1));
            return;
        }
        for i in 0..=steps {
            let a = from + FRAC_PI_2 * i as f32 / steps as f32;
            out.push([cx + radius * a.cos(), cy + radius * a.sin()]);
        }
    };
    corner(r.x + tl, r.y + tl, tl, PI);
    corner(r.right() - tr, r.y + tr, tr, PI * 1.5);
    corner(r.right() - br, r.bottom() - br, br, 0.0);
    corner(r.x + bl, r.bottom() - bl, bl, FRAC_PI_2);
    out
}

/// A closed ring of triangles between two outlines with the same number of points.
fn ring(mesh: &mut Mesh, outer: &[[f32; 2]], outer_color: [u8; 4], inner: &[[f32; 2]], inner_color: [u8; 4]) {
    debug_assert_eq!(outer.len(), inner.len());
    let n = outer.len();
    let o: Vec<u32> = outer.iter().map(|p| mesh.push(*p, outer_color)).collect();
    let i: Vec<u32> = inner.iter().map(|p| mesh.push(*p, inner_color)).collect();
    for k in 0..n {
        let k2 = (k + 1) % n;
        mesh.indices.extend([o[k], o[k2], i[k], o[k2], i[k2], i[k]]);
    }
}

/// A convex polygon as a fan from its middle, coloured by `color_at`.
fn fan(mesh: &mut Mesh, poly: &[[f32; 2]], color_at: &dyn Fn([f32; 2]) -> [u8; 4]) {
    let n = poly.len();
    let mid = [poly.iter().map(|p| p[0]).sum::<f32>() / n as f32, poly.iter().map(|p| p[1]).sum::<f32>() / n as f32];
    let c = mesh.push(mid, color_at(mid));
    let ids: Vec<u32> = poly.iter().map(|p| mesh.push(*p, color_at(*p))).collect();
    for k in 0..n {
        mesh.indices.extend([c, ids[k], ids[(k + 1) % n]]);
    }
}

/// A fill over `rect`: solid or a gradient along `angle` degrees clockwise from straight down.
#[must_use]
pub fn filled(rect: Rect, radii: Radii, fill: Fill) -> Mesh {
    let mut mesh = Mesh::default();
    if rect.w <= 0.0 || rect.h <= 0.0 {
        return mesh;
    }
    let colour_at: Box<dyn Fn([f32; 2]) -> [u8; 4]> = match fill {
        Fill::None => return mesh,
        Fill::Solid(c) => Box::new(move |_| bytes(c)),
        Fill::Gradient { from, to, angle } => {
            let a = angle.to_radians();
            let dir = [-a.sin(), a.cos()];
            let extent = (rect.w * dir[0].abs() + rect.h * dir[1].abs()).max(1e-3);
            let centre = rect.centre();
            Box::new(move |p| {
                let t = ((p[0] - centre.x) * dir[0] + (p[1] - centre.y) * dir[1]) / extent + 0.5;
                bytes(from.mix(to, t))
            })
        }
    };
    // The body is inset by half the feather, the fringe fades from there to half past the edge.
    let half = FEATHER / 2.0;
    let inner = outline(rect, radii, -half);
    fan(&mut mesh, &inner, &*colour_at);
    let outer = outline(rect, radii, half);
    let inner_colors: Vec<[u8; 4]> = inner.iter().map(|p| colour_at(*p)).collect();
    // The fringe takes its colour from the body, with alpha running to zero.
    let o: Vec<u32> = outer.iter().zip(&inner_colors).map(|(p, c)| mesh.push(*p, [c[0], c[1], c[2], 0])).collect();
    let i: Vec<u32> = inner.iter().zip(&inner_colors).map(|(p, c)| mesh.push(*p, *c)).collect();
    let n = outer.len();
    for k in 0..n {
        let k2 = (k + 1) % n;
        mesh.indices.extend([o[k], o[k2], i[k], o[k2], i[k2], i[k]]);
    }
    mesh
}

/// An outline `border.width` thick inside the edge of `rect`.
#[must_use]
pub fn bordered(rect: Rect, radii: Radii, border: Border) -> Mesh {
    let mut mesh = Mesh::default();
    if border.width <= 0.0 || border.color.a <= 0.0 {
        return mesh;
    }
    let c = bytes(border.color);
    let fade = [c[0], c[1], c[2], 0];
    let half = FEATHER / 2.0;
    let outer_edge = outline(rect, radii, half);
    let outer_body = outline(rect, radii, -half);
    let inner_body = outline(rect, radii, -(border.width - half).max(half));
    let inner_edge = outline(rect, radii, -(border.width + half));
    ring(&mut mesh, &outer_edge, fade, &outer_body, c);
    ring(&mut mesh, &outer_body, c, &inner_body, c);
    ring(&mut mesh, &inner_body, c, &inner_edge, fade);
    mesh
}

/// A soft shadow of `rect`: full strength under the shape, fading to nothing over `blur` units, in rings.
#[must_use]
pub fn shadowed(rect: Rect, radii: Radii, shadow: Shadow) -> Mesh {
    let mut mesh = Mesh::default();
    if shadow.color.a <= 0.0 {
        return mesh;
    }
    let moved = Rect::new(rect.x + shadow.offset[0], rect.y + shadow.offset[1], rect.w, rect.h);
    let base = bytes(shadow.color);
    let blur = shadow.blur.max(FEATHER);
    // Four rings approximate a smooth falloff: alpha at each distance from the edge.
    let stops = [(0.0, 1.0), (0.33, 0.7), (0.66, 0.27), (1.0, 0.0)];
    let body = outline(moved, radii, -blur * 0.0);
    fan(&mut mesh, &body, &|_| base);
    for pair in stops.windows(2) {
        let (d0, a0) = pair[0];
        let (d1, a1) = pair[1];
        let o = outline(moved, radii, blur * d1);
        let i = outline(moved, radii, blur * d0);
        let alpha = |a: f32| [base[0], base[1], base[2], (f32::from(base[3]) * a).round() as u8];
        ring(&mut mesh, &o, alpha(a1), &i, alpha(a0));
    }
    mesh
}

/// The mesh of one drawable primitive (text, icons, clips and opacity are the painter's own business).
#[must_use]
pub fn primitive(p: &Primitive) -> Option<Mesh> {
    match p {
        Primitive::Rect { rect, fill, radii, border, shadow } => {
            let mut mesh = Mesh::default();
            if let Some(shadow) = shadow {
                mesh.append(shadowed(*rect, *radii, *shadow));
            }
            mesh.append(filled(*rect, *radii, *fill));
            if let Some(border) = border {
                mesh.append(bordered(*rect, *radii, *border));
            }
            Some(mesh)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn white() -> Fill {
        Fill::Solid(Color::hex(0xFFFFFF, 1.0))
    }

    #[test]
    fn every_index_is_in_range_and_triangles_come_in_threes() {
        for radii in [Radii::all(0.0), Radii::all(12.0), Radii::top(10.0), Radii::all(500.0)] {
            let mesh = filled(Rect::new(0.0, 0.0, 80.0, 40.0), radii, white());
            assert_eq!(mesh.indices.len() % 3, 0);
            assert!(mesh.indices.iter().all(|i| (*i as usize) < mesh.vertices.len()));
            assert!(!mesh.indices.is_empty());
        }
    }

    #[test]
    fn a_fill_stays_within_half_a_feather_of_its_rectangle() {
        let rect = Rect::new(10.0, 20.0, 80.0, 40.0);
        let bounds = filled(rect, Radii::all(8.0), white()).bounds().unwrap();
        let slack = FEATHER / 2.0 + 1e-3;
        assert!(bounds.x >= rect.x - slack && bounds.y >= rect.y - slack);
        assert!(bounds.right() <= rect.right() + slack && bounds.bottom() <= rect.bottom() + slack);
        assert!(bounds.w >= rect.w - 1.0, "the fill covers the rectangle");
    }

    #[test]
    fn a_vertical_gradient_runs_from_the_first_colour_at_the_top_to_the_second_at_the_bottom() {
        let fill = Fill::Gradient { from: Color::hex(0xFF0000, 1.0), to: Color::hex(0x0000FF, 1.0), angle: 0.0 };
        let mesh = filled(Rect::new(0.0, 0.0, 40.0, 100.0), Radii::default(), fill);
        let top = mesh.vertices.iter().filter(|v| v.pos[1] < 2.0 && v.color[3] == 255).next().unwrap();
        let bottom = mesh.vertices.iter().filter(|v| v.pos[1] > 98.0 && v.color[3] == 255).next().unwrap();
        assert!(top.color[0] > 240 && top.color[2] < 15, "{:?}", top.color);
        assert!(bottom.color[2] > 240 && bottom.color[0] < 15, "{:?}", bottom.color);
    }

    #[test]
    fn a_border_is_a_ring_and_a_zero_width_border_draws_nothing() {
        let rect = Rect::new(0.0, 0.0, 60.0, 30.0);
        let ring_mesh = bordered(rect, Radii::all(6.0), Border { width: 2.0, color: Color::hex(0xFFFFFF, 1.0) });
        assert!(!ring_mesh.indices.is_empty());
        assert!(bordered(rect, Radii::all(6.0), Border { width: 0.0, color: Color::hex(0xFFFFFF, 1.0) }).indices.is_empty());
    }

    #[test]
    fn a_shadow_fades_to_nothing_at_its_far_edge_and_reaches_past_the_shape() {
        let rect = Rect::new(50.0, 50.0, 40.0, 40.0);
        let mesh = shadowed(rect, Radii::all(8.0), Shadow { offset: [0.0, 4.0], blur: 12.0, color: Color::hex(0x000000, 0.5) });
        let bounds = mesh.bounds().unwrap();
        assert!(bounds.right() > rect.right() + 10.0 && bounds.bottom() > rect.bottom() + 10.0);
        let faintest = mesh.vertices.iter().map(|v| v.color[3]).min().unwrap();
        assert_eq!(faintest, 0);
    }

    #[test]
    fn an_empty_rectangle_has_no_mesh() {
        assert!(filled(Rect::new(0.0, 0.0, 0.0, 10.0), Radii::default(), white()).indices.is_empty());
        assert!(filled(Rect::new(0.0, 0.0, 10.0, 10.0), Radii::default(), Fill::None).indices.is_empty());
    }
}
