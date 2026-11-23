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

/// The feather on every edge when none is set, in scene units.
pub const FEATHER: f32 = 1.0;

thread_local! {
    static FEATHER_NOW: std::cell::Cell<f32> = const { std::cell::Cell::new(FEATHER) };
}

/// Sets the feather the following meshes use, in scene units. The painter sets one screen pixel's worth so
/// edges stay crisp however far the scene is scaled up.
pub fn set_feather(units: f32) {
    FEATHER_NOW.with(|f| f.set(units.max(1e-3)));
}

fn feather() -> f32 {
    FEATHER_NOW.with(std::cell::Cell::get)
}
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
    let half = feather() / 2.0;
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
    let half = feather() / 2.0;
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
    let blur = shadow.blur.max(feather());
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

fn area(poly: &[[f32; 2]]) -> f32 {
    let n = poly.len();
    (0..n).map(|i| poly[i][0] * poly[(i + 1) % n][1] - poly[(i + 1) % n][0] * poly[i][1]).sum::<f32>() / 2.0
}

fn inside(poly: &[[f32; 2]], p: [f32; 2]) -> bool {
    let mut c = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            c = !c;
        }
        j = i;
    }
    c
}

fn cross(o: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

/// Triangulates a simple polygon (counter-clockwise) by clipping ears.
fn ear_clip(poly: &[[f32; 2]]) -> Vec<[usize; 3]> {
    let mut idx: Vec<usize> = (0..poly.len()).collect();
    let mut out = Vec::new();
    let mut guard = 0;
    while idx.len() > 3 && guard < poly.len() * poly.len() + 8 {
        guard += 1;
        let n = idx.len();
        let mut clipped = false;
        for k in 0..n {
            let (a, b, c) = (idx[(k + n - 1) % n], idx[k], idx[(k + 1) % n]);
            if cross(poly[a], poly[b], poly[c]) <= 1e-6 {
                continue;
            }
            let blocked = idx.iter().any(|&v| {
                v != a && v != b && v != c && poly[v] != poly[a] && poly[v] != poly[b] && poly[v] != poly[c] && cross(poly[a], poly[b], poly[v]) >= 0.0 && cross(poly[b], poly[c], poly[v]) >= 0.0 && cross(poly[c], poly[a], poly[v]) >= 0.0
            });
            if !blocked {
                out.push([a, b, c]);
                idx.remove(k);
                clipped = true;
                break;
            }
        }
        if !clipped {
            idx.remove(0);
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}

/// An even-odd fill of `contours`: each outer outline with the holes inside it, joined to it by a bridge and
/// triangulated. Contours nested two deep fill again, as even-odd says.
#[must_use]
pub fn path_fill(contours: &[Vec<[f32; 2]>], color: Color) -> Mesh {
    let mut mesh = Mesh::default();
    let rgba = bytes(color);
    // Nesting depth of each contour decides outer (even) or hole (odd).
    let depth: Vec<usize> = contours
        .iter()
        .enumerate()
        .map(|(i, c)| contours.iter().enumerate().filter(|(j, o)| *j != i && o.len() > 2 && inside(o, c[0])).count())
        .collect();
    for (i, outer) in contours.iter().enumerate().filter(|(i, c)| depth[*i] % 2 == 0 && c.len() > 2) {
        let mut poly = outer.clone();
        if area(&poly) < 0.0 {
            poly.reverse();
        }
        let mut holes: Vec<Vec<[f32; 2]>> = contours
            .iter()
            .enumerate()
            .filter(|(j, h)| depth[*j] == depth[i] + 1 && h.len() > 2 && inside(outer, h[0]))
            .map(|(_, h)| {
                let mut h = h.clone();
                if area(&h) > 0.0 {
                    h.reverse();
                }
                h
            })
            .collect();
        // Join each hole to the outline through its rightmost point, to the nearest outline point.
        holes.sort_by(|a, b| {
            let max = |h: &Vec<[f32; 2]>| h.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
            max(b).total_cmp(&max(a))
        });
        for hole in holes {
            let at = (0..hole.len()).max_by(|&a, &b| hole[a][0].total_cmp(&hole[b][0])).unwrap_or(0);
            let hp = hole[at];
            let near = (0..poly.len())
                .min_by(|&a, &b| {
                    let d = |p: [f32; 2]| (p[0] - hp[0]).powi(2) + (p[1] - hp[1]).powi(2);
                    d(poly[a]).total_cmp(&d(poly[b]))
                })
                .unwrap_or(0);
            let mut merged = Vec::with_capacity(poly.len() + hole.len() + 2);
            merged.extend_from_slice(&poly[..=near]);
            for k in 0..=hole.len() {
                merged.push(hole[(at + k) % hole.len()]);
            }
            merged.push(poly[near]);
            merged.extend_from_slice(&poly[near + 1..]);
            poly = merged;
        }
        let base = mesh.vertices.len() as u32;
        mesh.vertices.extend(poly.iter().map(|p| Vertex { pos: *p, color: rgba }));
        for t in ear_clip(&poly) {
            mesh.indices.extend([base + t[0] as u32, base + t[1] as u32, base + t[2] as u32]);
        }
    }
    mesh
}

/// A stroke of `width` along each contour (closed contours join their ends).
#[must_use]
pub fn path_stroke(contours: &[Vec<[f32; 2]>], width: f32, closed: bool, color: Color) -> Mesh {
    let mut mesh = Mesh::default();
    let rgba = bytes(color);
    let half = width / 2.0;
    for c in contours {
        let n = c.len();
        if n < 2 {
            continue;
        }
        let segs = if closed { n } else { n - 1 };
        for i in 0..segs {
            let (a, b) = (c[i], c[(i + 1) % n]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-4 {
                continue;
            }
            let (nx, ny) = (-dy / len * half, dx / len * half);
            let base = mesh.vertices.len() as u32;
            for p in [[a[0] + nx, a[1] + ny], [a[0] - nx, a[1] - ny], [b[0] - nx, b[1] - ny], [b[0] + nx, b[1] + ny]] {
                mesh.vertices.push(Vertex { pos: p, color: rgba });
            }
            mesh.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    mesh
}

/// A disc of `inner` radius that fades out to `outer`, appended to `mesh`.
fn disc(mesh: &mut Mesh, centre: [f32; 2], inner: f32, outer: f32, color: [u8; 4]) {
    const N: usize = 12;
    let fade = [color[0], color[1], color[2], 0];
    let mid = mesh.push(centre, color);
    let ring = |mesh: &mut Mesh, r: f32, c: [u8; 4]| -> Vec<u32> {
        (0..N)
            .map(|k| {
                let a = k as f32 / N as f32 * std::f32::consts::TAU;
                mesh.push([centre[0] + r * a.cos(), centre[1] + r * a.sin()], c)
            })
            .collect()
    };
    let inside = ring(mesh, inner, color);
    let outside = ring(mesh, outer, fade);
    for k in 0..N {
        let k2 = (k + 1) % N;
        mesh.indices.extend([mid, inside[k], inside[k2], inside[k], outside[k], outside[k2], inside[k], outside[k2], inside[k2]]);
    }
}

/// Open polylines `width` wide with round joins and ends, edges fading over a feather like every other shape.
#[must_use]
pub fn polyline_stroke(contours: &[Vec<[f32; 2]>], width: f32, color: Color) -> Mesh {
    let mut mesh = Mesh::default();
    let c = bytes(color);
    let fade = [c[0], c[1], c[2], 0];
    let f = feather();
    let half = (width / 2.0).max(f / 2.0);
    let (inner, outer) = ((half - f / 2.0).max(0.0), half + f / 2.0);
    for contour in contours {
        for pair in contour.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-5 {
                continue;
            }
            let (nx, ny) = (-dy / len, dx / len);
            let base = mesh.vertices.len() as u32;
            for (offset, col) in [(-outer, fade), (-inner, c), (inner, c), (outer, fade)] {
                for p in [a, b] {
                    mesh.vertices.push(Vertex { pos: [p[0] + nx * offset, p[1] + ny * offset], color: col });
                }
            }
            for row in 0..3 {
                let i = base + 2 * row;
                mesh.indices.extend([i, i + 1, i + 3, i, i + 3, i + 2]);
            }
        }
        for p in contour {
            disc(&mut mesh, *p, inner, outer, c);
        }
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
        Primitive::Path { rect, view, contours, color, stroke } => {
            let (sx, sy) = (rect.w / view[0].max(1e-3), rect.h / view[1].max(1e-3));
            let placed: Vec<Vec<[f32; 2]>> = contours.iter().map(|c| c.iter().map(|p| [rect.x + p[0] * sx, rect.y + p[1] * sy]).collect()).collect();
            Some(match stroke {
                Some(width) => path_stroke(&placed, width * sx.min(sy), true, *color),
                None => path_fill(&placed, *color),
            })
        }
        Primitive::Line { rect, view, contours, color, width } => {
            let (sx, sy) = (rect.w / view[0].max(1e-3), rect.h / view[1].max(1e-3));
            let placed: Vec<Vec<[f32; 2]>> = contours.iter().map(|c| c.iter().map(|p| [rect.x + p[0] * sx, rect.y + p[1] * sy]).collect()).collect();
            Some(polyline_stroke(&placed, width * sx.min(sy), *color))
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

    fn mesh_area(mesh: &Mesh) -> f32 {
        mesh.indices
            .chunks(3)
            .map(|t| {
                let (a, b, c) = (mesh.vertices[t[0] as usize].pos, mesh.vertices[t[1] as usize].pos, mesh.vertices[t[2] as usize].pos);
                ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() / 2.0
            })
            .sum()
    }

    #[test]
    fn a_square_with_a_square_hole_fills_the_ring_and_a_concave_shape_is_covered_exactly() {
        let outer = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let hole = vec![[3.0, 3.0], [7.0, 3.0], [7.0, 7.0], [3.0, 7.0]];
        let ring = path_fill(&[outer, hole], Color::hex(0xFFFFFF, 1.0));
        assert!((mesh_area(&ring) - 84.0).abs() < 0.5, "ring area {}", mesh_area(&ring));
        let l = vec![[0.0, 0.0], [8.0, 0.0], [8.0, 4.0], [4.0, 4.0], [4.0, 8.0], [0.0, 8.0]];
        let shape = path_fill(&[l], Color::hex(0xFFFFFF, 1.0));
        assert!((mesh_area(&shape) - 48.0).abs() < 0.1, "L area {}", mesh_area(&shape));
    }

    #[test]
    fn a_stroke_has_a_quad_per_segment() {
        let m = path_stroke(&[vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]], 2.0, false, Color::hex(0xFFFFFF, 1.0));
        assert_eq!(m.indices.len(), 12);
        assert!((mesh_area(&m) - 40.0).abs() < 0.1);
    }

    #[test]
    fn an_empty_rectangle_has_no_mesh() {
        assert!(filled(Rect::new(0.0, 0.0, 0.0, 10.0), Radii::default(), white()).indices.is_empty());
        assert!(filled(Rect::new(0.0, 0.0, 10.0, 10.0), Radii::default(), Fill::None).indices.is_empty());
    }
}
