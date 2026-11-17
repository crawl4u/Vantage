//! The Crosshair: the shapes it is drawn with, and how a nearby actor becomes "on target".
//!
//! Plain Rust with no host calls and no settings types, so the rules (where every bar sits, how a
//! ring is built from pixels, which actor counts as the target) are unit-tested on the host. The
//! settings live in `crosshair_config.rs`, the drawing in `hud.rs`, the reading in `guest.rs`.
//!
//! Every length is in GUI pixels, and every [`Bar`] sits relative to the middle of the screen.

/// The shapes the menu offers, in the order of `CrosshairSettings::shape`.
///
/// * Gapped: four arms around a gap, like Lunar's Simple crosshair and the game's own.
/// * Plus: two solid lines through the middle (the gap is ignored).
/// * Dot: one round dot, `size` across.
/// * Circle: a smooth stroked ring as far out as the gapped arms would reach (a vector path).
pub const SHAPES: [&str; 4] = ["Gapped", "Plus", "Dot", "Circle"];

const GAPPED: usize = 0;
const PLUS: usize = 1;
const DOT: usize = 2;
const CIRCLE: usize = 3;

use std::cell::Cell;

thread_local! {
    /// Device pixels in one GUI pixel (the GUI scale): the crosshair's lines sit on the device
    /// pixel grid, so a line can be thinner than a GUI pixel.
    static PIXEL: Cell<f32> = const { Cell::new(1.0) };
}

/// Sets the GUI scale the crosshair is drawn at (1 when it is not known).
pub fn set_pixel(scale: f32) {
    let scale = if scale.is_finite() { scale.clamp(1.0, 8.0) } else { 1.0 };
    PIXEL.with(|p| p.set(scale));
}

/// Device pixels in one GUI pixel.
pub fn pixel() -> f32 {
    PIXEL.with(Cell::get)
}

/// One device pixel in GUI pixels: the smallest step of a length.
pub fn step() -> f32 {
    1.0 / pixel()
}

/// `v` rounded to a whole number of device pixels, at least `least` of them.
pub fn snap(v: f32, least: i32) -> f32 {
    let s = pixel();
    ((v.max(0.0) * s + 0.5).floor().max(least as f32)) / s
}

/// How a crosshair looks in one state (at rest or on target, with the bow or without).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// Index into [`SHAPES`].
    pub shape: usize,
    /// How long each arm is.
    pub size: f32,
    /// The empty space between the middle and an arm.
    pub gap: f32,
    pub thickness: f32,
    /// A small square in the middle (Lunar's "Crosshair Dot").
    pub dot: bool,
    /// The centre dot is round instead of square.
    pub round_dot: bool,
    /// The arms of Gapped and Plus have round ends.
    pub round_lines: bool,
    /// Everything above is multiplied by this (the "Target size").
    pub scale: f32,
}

/// One filled rectangle, its top-left corner measured from the middle of the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Corner radius; half the width on a square makes a round dot.
    pub radius: f32,
}

/// A length in whole pixels (at least `least`): bars sit on the pixel grid so that both arms of a
/// line start on a pixel edge and no edge is blurred.
fn whole(v: f32, least: i32) -> i32 {
    (v.max(0.0) + 0.5).floor().max(least as f32) as i32
}

/// The bars that make up `look`, none of them overlapping (a see-through crosshair would show a
/// darker patch where two bars lie on each other).
///
/// Every edge is a whole number of pixels from the *middle pixel*: the pixel at the centre of the
/// screen, whose top-left corner is the origin. Opposite arms are mirror images about that pixel, so
/// the lines always line up. (Lengths are rounded to whole pixels, thickness included.)
pub fn bars(look: &Look) -> Vec<Bar> {
    let px = pixel();
    let k = if look.scale.is_finite() && look.scale > 0.0 { look.scale } else { 1.0 };
    // Everything below is counted in device pixels, then turned back into GUI pixels.
    let size = whole(look.size * k * px, 0) as f32;
    let gap = whole(look.gap * k * px, 0) as f32;
    let t = whole(look.thickness * k * px, 1) as f32;
    // The middle line covers [lo, hi) on both axes: the middle pixel, plus one more for an even width.
    let lo = -((t as i32 - 1) / 2) as f32;
    let hi = lo + t;
    let reach = gap + size;
    let mut out = Vec::with_capacity(5);
    let line = |x: f32, y: f32, w: f32, h: f32| Bar { x, y, w, h, radius: if look.round_lines { w.min(h) / 2.0 } else { 0.0 } };
    match look.shape {
        PLUS => {
            // The vertical line whole, the horizontal one in two halves either side of it.
            out.push(line(lo, lo - reach, t, t + reach * 2.0));
            out.push(line(lo - reach, lo, reach, t));
            out.push(line(hi, lo, reach, t));
        }
        DOT => {
            let side = size.max(t);
            let from = -((side as i32 - 1) / 2) as f32;
            out.push(Bar { x: from, y: from, w: side, h: side, radius: side / 2.0 });
        }
        CIRCLE => {
            // The ring itself is a vector path (`ring_of`); only the centre dot is a bar.
            if look.dot {
                out.push(dot_bar(lo, t, look.round_dot));
            }
        }
        _ => {
            if size > 0.0 {
                out.push(line(lo, lo - reach, t, size));
                out.push(line(lo, hi + gap, t, size));
                out.push(line(lo - reach, lo, size, t));
                out.push(line(hi + gap, lo, size, t));
            }
            if look.dot {
                out.push(dot_bar(lo, t, look.round_dot));
            }
        }
    }
    for b in &mut out {
        b.x /= px;
        b.y /= px;
        b.w /= px;
        b.h /= px;
        b.radius /= px;
    }
    out
}

/// The small dot in the middle: a square, or a round one (a corner radius of half its width).
fn dot_bar(lo: f32, t: f32, round: bool) -> Bar {
    Bar { x: lo, y: lo, w: t, h: t, radius: if round { t / 2.0 } else { 0.0 } }
}

/// A stroked circle around the middle pixel (its points are measured from the pixel's centre, which
/// is half a pixel from the bars' origin).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ring {
    /// Distance from the middle to the middle of the stroke.
    pub radius: f32,
    pub width: f32,
}

impl Ring {
    /// The same ring with a stroke `by` wider on each side (its outline).
    pub fn grown(self, by: f32) -> Ring {
        Ring { radius: self.radius, width: self.width + by * 2.0 }
    }
}

/// The ring of a look: only the Circle shape has one.
pub fn ring_of(look: &Look) -> Option<Ring> {
    if look.shape != CIRCLE {
        return None;
    }
    let k = if look.scale.is_finite() && look.scale > 0.0 { look.scale } else { 1.0 };
    Some(Ring { radius: ((look.size.max(0.0) + look.gap.max(0.0)) * k).max(0.5), width: look.thickness.max(0.1) * k })
}

/// Points on the unit circle, evenly spaced: with round joins 48 of them are smooth to well under a
/// hundredth of a pixel at any radius the menu allows. A table, because sines are costly in wasm.
pub const RING_POINTS: usize = 48;
const UNIT: [[f32; 2]; RING_POINTS] = [[1.00000, 0.00000], [0.99144, 0.13053], [0.96593, 0.25882], [0.92388, 0.38268], [0.86603, 0.50000], [0.79335, 0.60876], [0.70711, 0.70711], [0.60876, 0.79335], [0.50000, 0.86603], [0.38268, 0.92388], [0.25882, 0.96593], [0.13053, 0.99144], [0.00000, 1.00000], [-0.13053, 0.99144], [-0.25882, 0.96593], [-0.38268, 0.92388], [-0.50000, 0.86603], [-0.60876, 0.79335], [-0.70711, 0.70711], [-0.79335, 0.60876], [-0.86603, 0.50000], [-0.92388, 0.38268], [-0.96593, 0.25882], [-0.99144, 0.13053], [-1.00000, 0.00000], [-0.99144, -0.13053], [-0.96593, -0.25882], [-0.92388, -0.38268], [-0.86603, -0.50000], [-0.79335, -0.60876], [-0.70711, -0.70711], [-0.60876, -0.79335], [-0.50000, -0.86603], [-0.38268, -0.92388], [-0.25882, -0.96593], [-0.13053, -0.99144], [-0.00000, -1.00000], [0.13053, -0.99144], [0.25882, -0.96593], [0.38268, -0.92388], [0.50000, -0.86603], [0.60876, -0.79335], [0.70711, -0.70711], [0.79335, -0.60876], [0.86603, -0.50000], [0.92388, -0.38268], [0.96593, -0.25882], [0.99144, -0.13053]];

/// The closed polyline of `ring`, around the middle of the screen.
pub fn ring_points(ring: &Ring) -> [[f32; 2]; RING_POINTS] {
    UNIT.map(|[x, y]| [x * ring.radius, y * ring.radius])
}

/// `bars` grown by `width` on every side: drawn first, they are the crosshair's outline. Straight
/// bars get square corners, or round ones with `round`; a dot stays round either way.
pub fn outline(bars: &[Bar], width: f32, round: bool) -> Vec<Bar> {
    bars.iter()
        .map(|b| Bar {
            x: b.x - width,
            y: b.y - width,
            w: b.w + width * 2.0,
            h: b.h + width * 2.0,
            radius: if b.radius > 0.0 { b.radius + width } else if round { width } else { 0.0 },
        })
        .collect()
}

/// The preview in the menu: the look drawn bigger in a square box. Plain rectangles, a ring and a
/// disc, because the menu can only bind numbers (a position, a size, a colour), not shapes.
#[derive(Clone, Debug, PartialEq)]
pub struct Preview {
    /// Pixels of the box per GUI pixel.
    pub k: f32,
    /// The bars as `[x, y, width, height]` in the box.
    pub fill: Vec<[f32; 4]>,
    /// The same bars grown by the outline.
    pub edges: Vec<[f32; 4]>,
    /// The corner radius of each of `fill` and `edges`, in box pixels.
    pub fill_radius: Vec<f32>,
    pub edge_radius: Vec<f32>,
    /// The Circle's ring and its outline (radius and stroke), around the middle of the box.
    pub ring: Option<Ring>,
    pub ring_edge: Option<Ring>,
    /// A round dot (the Dot shape, or a round centre dot): a ring with no hole, and its outline.
    pub disc: Option<Ring>,
    pub disc_edge: Option<Ring>,
    /// Where the disc's middle is in the box.
    pub disc_at: [f32; 2],
}

/// The most a round shape of the preview reaches from its middle, in box pixels. The host refuses to
/// draw a stroked path that covers too large an area, so rings and discs are kept small.
const MAX_ROUND_REACH: f32 = 14.0;
/// The usual zoom: the same for every size, so that changing a slider only grows or shrinks the
/// drawing. It drops (in whole steps for bars) only when the drawing no longer fits.
const PREVIEW_ZOOM: f32 = 4.0;

/// The preview of `look` in a box `half * 2` across. `outline` is the width of the outline in GUI
/// pixels (0 for none).
pub fn preview(look: &Look, outline: f32, half: f32) -> Preview {
    preview_with(look, outline, false, half)
}

/// A bar that is a round dot: as wide as it is tall, and round all the way.
fn is_dot(b: &Bar) -> bool {
    b.radius > 0.0 && (b.w - b.h).abs() < 0.01 && b.radius * 2.0 >= b.w - 0.01
}

/// [`preview`], with `round_outline` for the outline's corners on straight bars.
pub fn preview_with(look: &Look, outline: f32, round_outline: bool, half: f32) -> Preview {
    let bars = bars(look);
    let ring_px = ring_of(look);
    // The farthest edge from the middle, outline included, decides how far the box can zoom in.
    let mut reach = 1.0f32;
    let mut round = 0.0f32;
    for b in &bars {
        if is_dot(b) {
            round = round.max(b.w / 2.0 + outline);
        }
        reach = reach.max(b.x.abs().max((b.x + b.w).abs()).max(b.y.abs()).max((b.y + b.h).abs()));
    }
    if let Some(r) = ring_px {
        round = round.max(r.radius + r.width / 2.0 + outline);
        reach = reach.max(r.radius + r.width / 2.0);
    }
    let mut zoom = ((half - 6.0) / (reach + outline + 1.0)).floor().clamp(1.0, PREVIEW_ZOOM);
    if round > 0.0 {
        zoom = zoom.min(MAX_ROUND_REACH / round).max(0.5);
    }
    // GUI pixels to the box: the middle pixel's middle is the middle of the box.
    let at = |v: f32| half + (v - 0.5 * step()) * zoom;
    let rect = |b: &Bar, grow: f32| [at(b.x - grow), at(b.y - grow), (b.w + grow * 2.0) * zoom, (b.h + grow * 2.0) * zoom];
    let mut out = Preview { k: zoom, fill: Vec::new(), edges: Vec::new(), fill_radius: Vec::new(), edge_radius: Vec::new(), ring: None, ring_edge: None, disc: None, disc_edge: None, disc_at: [half, half] };
    for b in &bars {
        if is_dot(b) {
            // A round dot: a ring with no hole, as wide as the dot is across.
            let disc = Ring { radius: b.w * zoom / 4.0, width: b.w * zoom / 2.0 };
            out.disc = Some(disc);
            out.disc_at = [at(b.x + b.w / 2.0), at(b.y + b.h / 2.0)];
            if outline > 0.0 {
                out.disc_edge = Some(Ring { radius: disc.radius, width: disc.width + outline * zoom * 2.0 });
            }
            continue;
        }
        out.fill.push(rect(b, 0.0));
        out.fill_radius.push(b.radius * zoom);
        if outline > 0.0 {
            out.edges.push(rect(b, outline));
            out.edge_radius.push(if b.radius > 0.0 { (b.radius + outline) * zoom } else if round_outline { outline * zoom } else { 0.0 });
        }
    }
    if let Some(r) = ring_px {
        let ring = Ring { radius: r.radius * zoom, width: r.width * zoom };
        out.ring = Some(ring);
        if outline > 0.0 {
            out.ring_edge = Some(ring.grown(outline * zoom));
        }
    }
    out
}

/// The distinct looks among the four states, and which one each state uses. Two states that look
/// the same share their bars, so the document holds each drawing once.
///
/// The states are numbered `state(weapon, target)`.
pub fn groups(looks: [Look; 4]) -> (Vec<Look>, [usize; 4]) {
    let mut unique: Vec<Look> = Vec::with_capacity(4);
    let mut which = [0usize; 4];
    for (state, look) in looks.iter().enumerate() {
        which[state] = match unique.iter().position(|known| known == look) {
            Some(found) => found,
            None => {
                unique.push(*look);
                unique.len() - 1
            }
        };
    }
    (unique, which)
}

/// The number of a state: 0 at rest, 1 on target, 2 with the bow at rest, 3 with the bow on target.
pub fn state(weapon: bool, targeted: bool) -> usize {
    usize::from(weapon) * 2 + usize::from(targeted)
}

/// What is under the crosshair.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Target {
    #[default]
    None,
    Player,
    Mob,
}

/// What the host says about one nearby actor, as far as the crosshair cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seen {
    pub runtime_id: u64,
    pub player: bool,
    /// The host's own answer to "would an attack from here reach it": `Some(true)` for the actor
    /// the crosshair is on (within reach, nothing in the way), `None` when it has no ray to give.
    pub hittable: Option<bool>,
}

/// The actor the crosshair is on: the first one the host calls hittable, other than yourself.
pub fn target_of(actors: impl IntoIterator<Item = Seen>, own: u64) -> Target {
    for actor in actors {
        if actor.runtime_id != own && actor.hittable == Some(true) {
            return if actor.player { Target::Player } else { Target::Mob };
        }
    }
    Target::None
}

/// Whether an item is one the weapon preset is for.
pub fn is_ranged(identifier: &str) -> bool {
    matches!(identifier, "minecraft:bow" | "minecraft:crossbow")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(shape: usize) -> Look {
        Look { shape, size: 5.0, gap: 2.0, thickness: 1.0, dot: false, round_dot: false, round_lines: false, scale: 1.0 }
    }

    /// Whether the point (a pixel's middle) lies inside any bar.
    fn covered(bars: &[Bar], x: f32, y: f32) -> bool {
        bars.iter().any(|b| x >= b.x && x <= b.x + b.w && y >= b.y && y <= b.y + b.h)
    }

    fn area(bars: &[Bar]) -> f32 {
        bars.iter().map(|b| b.w * b.h).sum()
    }

    /// No two bars share any area.
    fn disjoint(bars: &[Bar]) -> bool {
        for (i, a) in bars.iter().enumerate() {
            for b in &bars[i + 1..] {
                let (dx, dy) = ((a.x + a.w).min(b.x + b.w) - a.x.max(b.x), (a.y + a.h).min(b.y + b.h) - a.y.max(b.y));
                if dx > 1e-4 && dy > 1e-4 {
                    return false;
                }
            }
        }
        true
    }

    #[test]
    fn the_gapped_crosshair_is_four_arms_around_a_gap() {
        let b = bars(&look(GAPPED));
        assert_eq!(b.len(), 4);
        // The middle pixel is [0,1). Arms of 5 start 2 pixels from it: [-7,-2) and [3,8).
        assert_eq!(b[0], Bar { x: 0.0, y: -7.0, w: 1.0, h: 5.0, radius: 0.0 });
        assert_eq!(b[1], Bar { x: 0.0, y: 3.0, w: 1.0, h: 5.0, radius: 0.0 });
        assert_eq!(b[2], Bar { x: -7.0, y: 0.0, w: 5.0, h: 1.0, radius: 0.0 });
        assert_eq!(b[3], Bar { x: 3.0, y: 0.0, w: 5.0, h: 1.0, radius: 0.0 });
        assert!(disjoint(&b));
        assert_eq!(area(&b), 20.0);
    }

    #[test]
    fn opposite_arms_line_up_and_mirror_each_other() {
        for t in [1.0, 2.0, 3.0, 1.5, 4.0] {
            for gap in [0.0, 1.0, 2.0, 5.0] {
                let l = Look { thickness: t, gap, ..look(GAPPED) };
                let b = bars(&l);
                // Top and bottom share their column and width; left and right share their row.
                assert_eq!((b[0].x, b[0].w), (b[1].x, b[1].w), "{t} {gap}");
                assert_eq!((b[2].y, b[2].h), (b[3].y, b[3].h));
                // Mirror images about the middle line, which spans [lo, lo + t).
                assert_eq!(b[1].y - (b[0].y + b[0].h), 2.0 * gap + t.round().max(1.0), "{t} {gap}");
                assert_eq!(b[0].y, b[2].x);
                // Every edge is a whole pixel.
                for bar in &b {
                    assert!([bar.x, bar.y, bar.w, bar.h].iter().all(|v| v.fract() == 0.0));
                }
            }
        }
    }

    #[test]
    fn the_dot_option_adds_a_square_in_the_middle() {
        let mut l = look(GAPPED);
        l.dot = true;
        let b = bars(&l);
        assert_eq!(b.len(), 5);
        assert!(covered(&b, 0.5, 0.5) && disjoint(&b));
        l.thickness = 3.0;
        assert_eq!(bars(&l).last().map(|d| (d.x, d.w, d.h)), Some((-1.0, 3.0, 3.0)));
    }

    #[test]
    fn plus_is_solid_through_the_middle_and_ignores_the_gap() {
        let b = bars(&look(PLUS));
        assert_eq!(b.len(), 3);
        assert!(disjoint(&b));
        // The whole line from -7 to 8 on both axes, middle included: 15 + 14 (the horizontal
        // line's middle pixel belongs to the vertical one).
        assert_eq!(area(&b), 15.0 + 14.0);
        assert!(covered(&b, 0.5, 0.5) && covered(&b, 0.5, 7.5) && covered(&b, -6.5, 0.5));
        let mut wide = look(PLUS);
        wide.gap = 6.0;
        assert_eq!(area(&bars(&wide)), (1.0 + 22.0) + 22.0);
    }

    #[test]
    fn the_dot_shape_is_round_and_never_thinner_than_the_line() {
        let b = bars(&look(DOT));
        assert_eq!(b, vec![Bar { x: -2.0, y: -2.0, w: 5.0, h: 5.0, radius: 2.5 }]);
        let mut thick = look(DOT);
        thick.size = 1.0;
        thick.thickness = 3.0;
        assert_eq!(bars(&thick)[0].w, 3.0);
    }

    #[test]
    fn the_circle_is_one_stroked_ring_and_the_other_shapes_have_none() {
        let l = Look { shape: CIRCLE, size: 5.0, gap: 2.0, thickness: 1.5, dot: false, round_dot: false, round_lines: false, scale: 1.0 };
        assert_eq!(ring_of(&l), Some(Ring { radius: 7.0, width: 1.5 }));
        assert!(bars(&l).is_empty(), "the ring is not made of bars");
        assert_eq!(ring_of(&Look { scale: 2.0, ..l }), Some(Ring { radius: 14.0, width: 3.0 }));
        for shape in [GAPPED, PLUS, DOT] {
            assert_eq!(ring_of(&look(shape)), None);
        }
        // The dot option is still a square in the middle.
        let b = bars(&Look { dot: true, ..l });
        assert_eq!(b.len(), 1);
        assert!(covered(&b, 0.5, 0.5));
        assert_eq!(ring_of(&l).unwrap().grown(1.0), Ring { radius: 7.0, width: 3.5 });
    }

    #[test]
    fn the_ring_points_lie_on_the_circle_and_close_up_evenly() {
        let ring = Ring { radius: 9.0, width: 1.0 };
        let p = ring_points(&ring);
        assert_eq!(p.len(), RING_POINTS);
        for [x, y] in p {
            assert!(((x * x + y * y).sqrt() - 9.0).abs() < 0.001, "{x},{y}");
        }
        // Even spacing: every step is the same length, and the circle is symmetric.
        let step = |a: [f32; 2], b: [f32; 2]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
        let first = step(p[0], p[1]);
        for i in 0..RING_POINTS {
            assert!((step(p[i], p[(i + 1) % RING_POINTS]) - first).abs() < 0.001);
        }
        assert!((p[0][0] - 9.0).abs() < 0.001 && p[0][1].abs() < 0.001);
        assert!((p[RING_POINTS / 2][0] + 9.0).abs() < 0.001);
        // The error of a 48-gon against the real circle at the biggest radius is far below a pixel.
        let worst = 56.0 * (1.0 - (std::f32::consts::PI / RING_POINTS as f32).cos());
        assert!(worst < 0.13, "{worst}");
    }

    #[test]
    fn scale_grows_every_length() {
        let mut l = look(GAPPED);
        l.scale = 2.0;
        let b = bars(&l);
        // Arms of 10 starting 4 away, 2 thick: the middle line is [-0, 2) .. so the top arm is [-14,-4).
        assert_eq!((b[0].y, b[0].h, b[0].w), (-14.0, 10.0, 2.0));
        assert_eq!(area(&b), 4.0 * 10.0 * 2.0);
        // A broken scale falls back to 1.
        l.scale = f32::NAN;
        assert_eq!(bars(&l), bars(&look(GAPPED)));
    }

    #[test]
    fn the_outline_surrounds_every_bar() {
        let b = bars(&look(GAPPED));
        let o = outline(&b, 1.0, false);
        assert_eq!(o.len(), b.len());
        for (inner, outer) in b.iter().zip(&o) {
            assert_eq!((outer.x, outer.y, outer.w, outer.h), (inner.x - 1.0, inner.y - 1.0, inner.w + 2.0, inner.h + 2.0));
            assert_eq!(outer.radius, 0.0);
        }
        // Rounded on request, and a dot is round either way.
        assert!(outline(&b, 1.0, true).iter().all(|o| o.radius == 1.0));
        assert_eq!(outline(&bars(&look(DOT)), 1.0, false)[0].radius, 3.5);
    }

    #[test]
    fn states_that_look_alike_share_a_drawing() {
        let rest = look(GAPPED);
        let mut target = rest;
        target.scale = 1.4;
        let (unique, which) = groups([rest, target, rest, target]);
        assert_eq!((unique.len(), which), (2, [0, 1, 0, 1]));
        let (unique, which) = groups([rest; 4]);
        assert_eq!((unique.len(), which), (1, [0; 4]));
        let mut bow = rest;
        bow.size = 3.0;
        let (unique, which) = groups([rest, rest, bow, bow]);
        assert_eq!((unique.len(), which), (2, [0, 0, 1, 1]));
        assert_eq!([state(false, false), state(false, true), state(true, false), state(true, true)], [0, 1, 2, 3]);
    }

    #[test]
    fn the_target_is_the_hittable_actor_that_is_not_you() {
        let seen = |runtime_id, player, hittable| Seen { runtime_id, player, hittable };
        assert_eq!(target_of([], 1), Target::None);
        assert_eq!(target_of([seen(2, false, None), seen(3, true, Some(false))], 1), Target::None);
        assert_eq!(target_of([seen(2, false, Some(true))], 1), Target::Mob);
        assert_eq!(target_of([seen(2, false, Some(false)), seen(3, true, Some(true))], 1), Target::Player);
        // You are never your own target.
        assert_eq!(target_of([seen(1, true, Some(true))], 1), Target::None);
    }

    #[test]
    fn only_bows_get_the_weapon_preset() {
        assert!(is_ranged("minecraft:bow") && is_ranged("minecraft:crossbow"));
        assert!(!is_ranged("minecraft:diamond_sword") && !is_ranged("minecraft:arrow") && !is_ranged(""));
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;

    fn look(shape: usize) -> Look {
        Look { shape, size: 5.0, gap: 2.0, thickness: 1.0, dot: false, round_dot: false, round_lines: false, scale: 1.0 }
    }

    #[test]
    fn the_preview_keeps_one_zoom_and_centres_the_middle_pixel() {
        let p = preview(&look(GAPPED), 0.0, 62.0);
        assert_eq!(p.k, 4.0);
        assert_eq!(p.fill.len(), 4);
        assert!(p.edges.is_empty() && p.ring.is_none() && p.disc.is_none());
        // The top arm: 1 pixel wide at the middle, from 7 pixels up to 2 up.
        assert_eq!(p.fill[0], [60.0, 62.0 - 7.5 * 4.0, 4.0, 20.0]);
        let (top, bottom) = (p.fill[0], p.fill[1]);
        assert_eq!(top[0], bottom[0]);
        assert_eq!(62.0 - top[1], bottom[1] + bottom[3] - 62.0);
        // Changing a slider does not change the zoom, until the drawing no longer fits.
        let big = Look { size: 9.0, ..look(GAPPED) };
        assert_eq!(preview(&big, 0.0, 62.0).k, 4.0);
        assert!(preview(&Look { size: 16.0, gap: 12.0, ..look(GAPPED) }, 0.0, 62.0).k < 4.0);
    }

    #[test]
    fn outlines_rings_and_discs_come_with_the_preview() {
        let p = preview(&look(GAPPED), 1.0, 62.0);
        assert_eq!((p.fill.len(), p.edges.len()), (4, 4));
        assert_eq!(p.edges[0][0], p.fill[0][0] - p.k);
        assert_eq!(p.edges[0][2], p.fill[0][2] + 2.0 * p.k);
        let c = preview(&look(CIRCLE), 1.0, 62.0);
        let (ring, edge) = (c.ring.unwrap(), c.ring_edge.unwrap());
        assert_eq!(ring.radius, 7.0 * c.k);
        assert_eq!(edge.width, ring.width + 2.0 * c.k);
        assert!(ring.radius + ring.width / 2.0 + c.k <= MAX_ROUND_REACH + 0.01);
        // The round dot shape is a disc: a ring whose stroke fills it.
        let d = preview(&look(DOT), 0.0, 62.0);
        let r = d.disc.unwrap();
        assert_eq!(r.radius * 2.0, r.width);
        assert!(d.fill.is_empty() && d.ring.is_none());
        // A round centre dot is a disc too, next to the arms (or the ring).
        let g = preview(&Look { dot: true, round_dot: true, thickness: 3.0, ..look(GAPPED) }, 0.0, 62.0);
        assert_eq!((g.fill.len(), g.disc.is_some()), (4, true));
        let sq = preview(&Look { dot: true, thickness: 3.0, ..look(GAPPED) }, 0.0, 62.0);
        assert_eq!((sq.fill.len(), sq.disc.is_some()), (5, false));
        let cr = preview(&Look { dot: true, round_dot: true, ..look(CIRCLE) }, 0.0, 62.0);
        assert!(cr.ring.is_some() && cr.disc.is_some());
        // Everything stays inside the box, whatever the look.
        for shape in 0..4 {
            let big = Look { size: 16.0, gap: 12.0, thickness: 8.0, dot: true, round_dot: true, ..look(shape) };
            let p = preview(&big, 1.0, 62.0);
            for [x, y, w, h] in p.fill.iter().chain(&p.edges) {
                assert!(*x >= 0.0 && *y >= 0.0 && x + w <= 124.0 && y + h <= 124.0, "{shape}");
            }
            for e in [p.ring_edge.or(p.ring), p.disc_edge.or(p.disc)].into_iter().flatten() {
                assert!(e.radius + e.width / 2.0 <= 62.0, "{shape}");
            }
        }
    }

    #[test]
    fn thickness_keeps_growing_to_the_limit() {
        for shape in [PLUS, GAPPED, DOT] {
            for k in [1.0f32, 2.0, 3.0] {
                let mut last = 0.0f32;
                for step in 1..=32 {
                    let t = step as f32 * 0.5;
                    let l = Look { thickness: t, scale: k, size: 20.0, ..look(shape) };
                    let w = bars(&l).iter().map(|b| b.w.min(b.h)).fold(0.0f32, f32::max);
                    assert!(w >= last, "{shape} k={k} t={t}");
                    last = w;
                }
                assert!(last >= 16.0 * k - 1.0, "{shape} k={k}: {last}");
            }
        }
    }

    #[test]
    fn lines_can_be_thinner_than_a_gui_pixel() {
        set_pixel(3.0);
        let l = Look { thickness: 1.0 / 3.0, size: 5.0, gap: 2.0, ..look(GAPPED) };
        let b = bars(&l);
        assert!((b[0].w - 1.0 / 3.0).abs() < 1e-4, "{:?}", b[0]);
        let wide = bars(&Look { thickness: 2.0 / 3.0, ..l });
        assert!(wide[0].w > b[0].w);
        set_pixel(1.0);
    }

    #[test]
    fn round_lines_round_the_arms_not_the_dot_rule() {
        let l = Look { round_lines: true, thickness: 4.0, ..look(GAPPED) };
        for b in bars(&l) {
            assert!((b.radius - b.w.min(b.h) / 2.0).abs() < 1e-4, "{b:?}");
        }
        let square = Look { round_lines: false, thickness: 4.0, ..look(GAPPED) };
        assert!(bars(&square).iter().all(|b| b.radius == 0.0));
        // The outline of a round arm follows the curve.
        let o = outline(&bars(&l), 1.0, false);
        assert!(o.iter().all(|b| b.radius > 0.0));
        // Round arms are rectangles in the preview, not discs.
        let p = preview(&l, 0.0, 62.0);
        assert_eq!(p.fill.len(), 4);
        assert!(p.fill_radius.iter().all(|r| *r > 0.0));
        assert!(p.disc.is_none());
    }

    #[test]
    fn a_round_centre_dot_is_a_round_bar() {
        let l = Look { dot: true, round_dot: true, thickness: 3.0, ..look(GAPPED) };
        assert_eq!(bars(&l).last().map(|b| b.radius), Some(1.5));
        let l = Look { round_dot: false, ..l };
        assert_eq!(bars(&l).last().map(|b| b.radius), Some(0.0));
        // The Dot shape ignores the option: it is always round.
        assert_eq!(bars(&Look { dot: true, ..look(DOT) }).len(), 1);
    }
}
