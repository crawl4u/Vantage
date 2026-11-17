//! Block outline: the box drawn around the block you point at. Plain Rust (colour and width choices, the rainbow
//! and pulse modes), unit-tested on the host; `guest.rs` reads the target block and draws it.

/// How the colour behaves: fixed, cycling through the rainbow, or breathing in and out.
pub const MODE_NAMES: [&str; 3] = ["Static", "Rainbow", "Pulse"];
/// Opacities in percent, and their names.
pub const OPACITIES: [f32; 6] = [20.0, 40.0, 60.0, 80.0, 100.0, 0.0];
pub const OPACITY_NAMES: [&str; 6] = ["20%", "40%", "60%", "80%", "100%", "0%"];

/// A fully saturated colour for a hue turn in 0..1.
pub fn rainbow(turn: f64) -> [f32; 3] {
    let h = turn.rem_euclid(1.0) * 6.0;
    let x = (1.0 - (h % 2.0 - 1.0).abs()) as f32;
    match h as u32 {
        0 => [1.0, x, 0.0],
        1 => [x, 1.0, 0.0],
        2 => [0.0, 1.0, x],
        3 => [0.0, x, 1.0],
        4 => [x, 0.0, 1.0],
        _ => [1.0, 0.0, x],
    }
}

/// Lunar's Chroma for text: the colour at `frame` (30ths of a second) for a speed (1 to 100; the hue
/// turns about 1.43 degrees a second per unit). `step` offsets Wave's colour for each line or key.
pub fn chroma_color(frame: u32, speed: f64, wave: bool, step: usize) -> [f32; 3] {
    let t = f64::from(frame) / 30.0;
    let shift = if wave { step as f64 * 0.04 } else { 0.0 };
    rainbow(t * speed * 1.43 / 360.0 + shift)
}

/// The line colour `[r, g, b, a]` for a mode at time `t` (seconds): `base` is the chosen colour, `opacity` in 0..1.
pub fn tint(mode: usize, base: [f32; 3], opacity: f32, t: f64) -> [f32; 4] {
    match mode {
        1 => {
            let [r, g, b] = rainbow(t * 0.25);
            [r, g, b, opacity]
        }
        2 => {
            // Between a third and all of the opacity, once every two seconds.
            let wave = 0.5 + 0.5 * (t * std::f64::consts::PI).sin() as f32;
            [base[0], base[1], base[2], opacity * (0.33 + 0.67 * wave)]
        }
        _ => [base[0], base[1], base[2], opacity],
    }
}


/// Fill opacities in percent (index 0 is off) and the game-highlight fix strengths in percent (0 is off).
pub const FILL_PERCENTS: [u8; 6] = [0, 10, 20, 30, 40, 50];
pub const FILL_NAMES: [&str; 6] = ["Off", "10%", "20%", "30%", "40%", "50%"];
pub const FIX_PERCENTS: [u8; 5] = [0, 25, 50, 75, 100];
pub const FIX_NAMES: [&str; 5] = ["Off", "25%", "50%", "75%", "100%"];

/// How much the game brightens the faces of the block it selects (when its own outline lines are off).
pub const GAME_HIGHLIGHT_GAIN: f32 = 1.3;

/// The black overlay opacity that darkens a face by `1 / gain`, which cancels the game's brightening at 100%.
pub fn highlight_cancel_alpha(percent: u8) -> f32 {
    let gain = 1.0 + (GAME_HIGHLIGHT_GAIN - 1.0) * f32::from(percent.min(100)) / 100.0;
    1.0 - 1.0 / gain
}

/// Grows every box by `by` on all sides, so lines and faces do not fight the block's own faces for depth.
pub fn inflate(boxes: &[[f64; 6]], by: f64) -> Vec<[f64; 6]> {
    boxes.iter().map(|b| [b[0] - by, b[1] - by, b[2] - by, b[3] + by, b[4] + by, b[5] + by]).collect()
}

// ---- Shapes: ported from boldy-mods (MIT, (c) 2026 Vainius Kadys; see LICENSE-BOLDY), trimmed to what Vantage uses.

/// One straight piece of a shape's outline, along `axis` from `start`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub start: [f64; 3],
    pub axis: usize,
    pub length: f64,
    /// Which of the four quarters around the line are inside the shape: bit `a + 2 * b`, where
    /// `a` and `b` are 1 on the high side of the first and second other axis.
    quarters: u8,
}

impl Segment {
    pub fn end(&self) -> [f64; 3] {
        let mut end = self.start;
        end[self.axis] += self.length;
        end
    }

    /// Whether a face meeting at this line faces `camera`, so the line is on the visible side
    /// of the shape.
    pub fn visible(&self, camera: [f64; 3]) -> bool {
        let [u, v] = Edge {
            axis: self.axis,
            sides: [false, false],
        }
        .others();
        let inside = |a: u8, b: u8| self.quarters >> (a + 2 * b) & 1 == 1;
        let ahead = |axis: usize, high: bool| {
            let offset = camera[axis] - self.start[axis];
            if high { offset > 1e-9 } else { offset < -1e-9 }
        };
        // A face between two quarters faces away from the inside one.
        (0..2).any(|b| inside(0, b) != inside(1, b) && ahead(u, inside(0, b)))
            || (0..2).any(|a| inside(a, 0) != inside(a, 1) && ahead(v, inside(a, 0)))
    }
}

/// The outline of the union of `boxes`: every box edge, split where other boxes start or end,
/// keeping only the pieces that are real corners of the merged shape (not lines across a flat
/// face or inside it). A single box gives its 12 edges.
pub fn shape_edges(boxes: &[[f64; 6]]) -> Vec<Segment> {
    const EPS: f64 = 1e-4;
    // One box (most blocks, every mob): its own 12 edges, each with the box in one quarter.
    if let [b] = boxes {
        return EDGES
            .iter()
            .filter(|edge| b[edge.axis + 3] - b[edge.axis] >= 0.01)
            .map(|edge| {
                let [u, v] = edge.others();
                let mut start = [0.0; 3];
                start[edge.axis] = b[edge.axis];
                start[u] = if edge.sides[0] { b[u + 3] } else { b[u] };
                start[v] = if edge.sides[1] { b[v + 3] } else { b[v] };
                // The box lies on the low side of an edge on its high side, and the other way.
                let quarter = u8::from(!edge.sides[0]) + 2 * u8::from(!edge.sides[1]);
                Segment {
                    start,
                    axis: edge.axis,
                    length: b[edge.axis + 3] - b[edge.axis],
                    quarters: 1 << quarter,
                }
            })
            .collect();
    }
    // Reused for every edge rather than allocated for each.
    let mut cuts: Vec<f64> = Vec::with_capacity(2 * boxes.len() + 2);
    let inside = |p: [f64; 3]| {
        boxes
            .iter()
            .any(|b| (0..3).all(|axis| b[axis] < p[axis] && p[axis] < b[axis + 3]))
    };
    let mut segments: Vec<Segment> = Vec::new();
    for b in boxes {
        for edge in EDGES {
            let axis = edge.axis;
            let [u, v] = edge.others();
            let mut start = [0.0; 3];
            start[u] = if edge.sides[0] { b[u + 3] } else { b[u] };
            start[v] = if edge.sides[1] { b[v + 3] } else { b[v] };
            let (lo, hi) = (b[axis], b[axis + 3]);
            // Where other boxes begin or end along this edge.
            cuts.clear();
            cuts.extend([lo, hi]);
            for other in boxes {
                for cut in [other[axis], other[axis + 3]] {
                    if cut > lo + EPS && cut < hi - EPS {
                        cuts.push(cut);
                    }
                }
            }
            cuts.sort_unstable_by(f64::total_cmp);
            cuts.dedup_by(|a, b| (*a - *b).abs() < EPS);
            for pair in cuts.windows(2) {
                let (from, to) = (pair[0], pair[1]);
                if to - from < 0.01 {
                    continue;
                }
                let mut probe = start;
                probe[axis] = (from + to) / 2.0;
                let mut quarters = 0u8;
                for (bit, (du, dv)) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                    .into_iter()
                    .enumerate()
                {
                    let mut p = probe;
                    p[u] += du * EPS * 10.0;
                    p[v] += dv * EPS * 10.0;
                    if inside(p) {
                        quarters |= 1 << bit;
                    }
                }
                // A corner of the merged shape: one or three quarters inside, or two diagonal.
                let real = matches!(quarters.count_ones(), 1 | 3)
                    || quarters == 0b1001
                    || quarters == 0b0110;
                if !real {
                    continue;
                }
                let mut from_point = start;
                from_point[axis] = from;
                let segment = Segment {
                    start: from_point,
                    axis,
                    length: to - from,
                    quarters,
                };
                let same = |s: &Segment| {
                    s.axis == axis
                        && (0..3).all(|i| (s.start[i] - segment.start[i]).abs() < EPS)
                        && (s.length - segment.length).abs() < EPS
                };
                if !segments.iter().any(same) {
                    segments.push(segment);
                }
            }
        }
    }
    segments
}

/// One box face: the axis it is perpendicular to and whether it is on the maximum side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Face {
    pub axis: usize,
    pub max_side: bool,
}

/// The six faces of a box.
pub const FACES: [Face; 6] = [
    Face {
        axis: 0,
        max_side: false,
    },
    Face {
        axis: 0,
        max_side: true,
    },
    Face {
        axis: 1,
        max_side: false,
    },
    Face {
        axis: 1,
        max_side: true,
    },
    Face {
        axis: 2,
        max_side: false,
    },
    Face {
        axis: 2,
        max_side: true,
    },
];

impl Face {
    /// The face's four corners on the unit cube, in order around it.
    pub fn unit_corners(self) -> [[f32; 3]; 4] {
        let side = if self.max_side { 1.0 } else { 0.0 };
        let (u, v) = match self.axis {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]].map(|[a, b]| {
            let mut corner = [0.0; 3];
            corner[self.axis] = side;
            corner[u] = a;
            corner[v] = b;
            corner
        })
    }

    /// Whether the camera is beyond the face's plane, so the face is turned towards it.
    pub fn visible(self, min: [f64; 3], max: [f64; 3], camera: [f64; 3]) -> bool {
        if self.max_side {
            camera[self.axis] > max[self.axis]
        } else {
            camera[self.axis] < min[self.axis]
        }
    }
}

/// One box edge: the axis it runs along and, for the other two axes in increasing order,
/// whether it lies on their minimum (false) or maximum (true) side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub axis: usize,
    pub sides: [bool; 2],
}

/// The twelve edges of a box, four along each axis.
pub const EDGES: [Edge; 12] = {
    let mut edges = [Edge {
        axis: 0,
        sides: [false, false],
    }; 12];
    let mut index = 0;
    while index < 12 {
        edges[index] = Edge {
            axis: index / 4,
            sides: [index % 2 == 1, (index / 2) % 2 == 1],
        };
        index += 1;
    }
    edges
};

impl Edge {
    /// The two axes other than the edge's own, in increasing order.
    pub fn others(self) -> [usize; 2] {
        match self.axis {
            0 => [1, 2],
            1 => [0, 2],
            _ => [0, 1],
        }
    }

    /// Start and end points of this edge on the unit cube.
    pub fn unit_points(self) -> [[f32; 3]; 2] {
        let mut start = [0.0; 3];
        for (axis, side) in self.others().into_iter().zip(self.sides) {
            start[axis] = if side { 1.0 } else { 0.0 };
        }
        let mut end = start;
        end[self.axis] = 1.0;
        [start, end]
    }

    /// Whether the edge borders a face that faces `camera`: a face is visible when the camera is
    /// beyond its plane. Edges between two faces turned away from the camera are hidden behind
    /// the block, so only the outline of the faces you can see remains.
    pub fn visible(self, min: [f64; 3], max: [f64; 3], camera: [f64; 3]) -> bool {
        self.others()
            .into_iter()
            .zip(self.sides)
            .any(|(axis, side)| {
                if side {
                    camera[axis] > max[axis]
                } else {
                    camera[axis] < min[axis]
                }
            })
    }

    /// World-space start and end points on the box.
    pub fn points(self, min: [f64; 3], max: [f64; 3]) -> [[f32; 3]; 2] {
        self.unit_points().map(|unit| {
            std::array::from_fn(|axis| {
                (min[axis] + f64::from(unit[axis]) * (max[axis] - min[axis])) as f32
            })
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn chroma_cycles_and_waves() {
        // Speed 40 turns the hue 57.2 degrees a second: red at the start, and a wave offsets each line.
        assert_eq!(chroma_color(0, 40.0, false, 3), [1.0, 0.0, 0.0]);
        assert_ne!(chroma_color(0, 40.0, true, 3), chroma_color(0, 40.0, true, 0));
        assert_ne!(chroma_color(30, 40.0, false, 0), chroma_color(0, 40.0, false, 0));
    }

    use super::*;

    #[test]
    fn the_rainbow_starts_red_and_stays_saturated() {
        assert_eq!(rainbow(0.0), [1.0, 0.0, 0.0]);
        for i in 0..60 {
            let [r, g, b] = rainbow(f64::from(i) / 60.0);
            assert!((r.max(g).max(b) - 1.0).abs() < 1e-6 && r.min(g).min(b) == 0.0);
        }
    }

    #[test]
    fn pulse_never_exceeds_the_opacity() {
        for i in 0..40 {
            let a = tint(2, [1.0; 3], 0.8, f64::from(i) * 0.05)[3];
            assert!(a <= 0.8 + 1e-6 && a >= 0.8 * 0.33 - 1e-6);
        }
        assert_eq!(tint(0, [0.1, 0.2, 0.3], 0.5, 9.0), [0.1, 0.2, 0.3, 0.5]);
    }

    #[test]
    fn one_box_has_twelve_edges_and_a_stair_merges_its_lines() {
        let cube = [[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]];
        assert_eq!(shape_edges(&cube).len(), 12);
        // A stair: a slab with a half-size step on top.
        let stair = [[0.0, 0.0, 0.0, 1.0, 0.5, 1.0], [0.0, 0.5, 0.0, 1.0, 1.0, 0.5]];
        let edges = shape_edges(&stair);
        assert!(edges.len() > 12 && edges.len() < 24, "{}", edges.len());
    }

    #[test]
    fn the_cancel_overlay_matches_the_games_gain() {
        assert_eq!(highlight_cancel_alpha(0), 0.0);
        assert!((highlight_cancel_alpha(100) - (1.0 - 1.0 / 1.3)).abs() < 1e-6);
    }

    #[test]
    fn only_edges_of_faces_towards_the_camera_show() {
        let cube = [[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]];
        let shown = shape_edges(&cube).iter().filter(|s| s.visible([0.5, 3.0, 0.5])).count();
        assert_eq!(shown, 4, "looking down on a cube shows only the top ring");
    }
}
