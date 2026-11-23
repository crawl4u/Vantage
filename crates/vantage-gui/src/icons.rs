//! Vector icons drawn as stroked polylines on a 24 by 24 grid. They are original line drawings in one weight, so
//! they scale crisply and take any colour; mods draw their own, the menu's chrome uses these.

use std::f32::consts::PI;

/// The side of the grid every icon is drawn in.
pub const GRID: f32 = 24.0;

/// An icon of the menu's chrome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Layers,
    EditHud,
    Sliders,
    User,
    Keyboard,
    Camera,
    Sparkles,
    Gear,
    Refresh,
    Star,
    MapPin,
    Cloud,
    Bolt,
    Wrench,
    Grid,
    Bell,
    Heart,
    Eye,
    Search,
    Close,
    Back,
    Forward,
    Mouse,
    Gauge,
    ZoomIn,
    Shield,
    Flask,
    Signal,
    Crosshair,
    Sun,
    Compass,
    Clock,
    Speed,
    Timer,
    Hourglass,
    Calendar,
    Angle,
    Ruler,
    Swords,
    Totem,
    Cube,
    Backpack,
    List,
    Chunk,
    Droplet,
    Diamond,
    Scoreboard,
    Target,
    ArrowTop,
    Captions,
    Brackets,
    Bulb,
    Flame,
    Box,
    Tag,
    Rings,
    Sprint,
    Crouch,
    Pointer,
    Mute,
    Funnel,
    Flag,
    Bomb,
    TextT,
    Hex,
    Perspective,
    Glass,
    Hash,
    Moon,
    Dash,
}

/// What the menu can draw as an icon: a line icon drawn by hand, or an icon converted from artwork.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Line(Icon),
    Art(crate::icon_art::Art),
    Pixel(&'static crate::pixel_art::PixelArt),
}

impl From<Icon> for Glyph {
    fn from(icon: Icon) -> Self {
        Glyph::Line(icon)
    }
}

impl From<crate::icon_art::Art> for Glyph {
    fn from(art: crate::icon_art::Art) -> Self {
        Glyph::Art(art)
    }
}

type Contour = Vec<[f32; 2]>;

/// Points of a circular arc from `from` to `to` degrees (y points down, 0 is east, 90 south).
fn arc(cx: f32, cy: f32, r: f32, from: f32, to: f32) -> Contour {
    let steps = (((to - from).abs() / 6.0).ceil() as usize).max(2);
    (0..=steps)
        .map(|i| {
            let a = (from + (to - from) * i as f32 / steps as f32) * PI / 180.0;
            [cx + r * a.cos(), cy + r * a.sin()]
        })
        .collect()
}

fn circle(cx: f32, cy: f32, r: f32) -> Contour {
    arc(cx, cy, r, 0.0, 360.0)
}

/// A rounded rectangle, closed.
fn rrect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Contour {
    let mut c = arc(x + w - r, y + r, r, 270.0, 360.0);
    c.extend(arc(x + w - r, y + h - r, r, 0.0, 90.0));
    c.extend(arc(x + r, y + h - r, r, 90.0, 180.0));
    c.extend(arc(x + r, y + r, r, 180.0, 270.0));
    c.push(c[0]);
    c
}

fn closed(mut points: Contour) -> Contour {
    points.push(points[0]);
    points
}

fn line(a: [f32; 2], b: [f32; 2]) -> Contour {
    vec![a, b]
}

/// A star of `n` points with outer and inner radii, closed.
fn star(cx: f32, cy: f32, outer: f32, inner: f32, n: usize) -> Contour {
    closed(
        (0..n * 2)
            .map(|i| {
                let r = if i % 2 == 0 { outer } else { inner };
                let a = -PI / 2.0 + PI * i as f32 / n as f32;
                [cx + r * a.cos(), cy + r * a.sin()]
            })
            .collect(),
    )
}

/// A four-pointed sparkle centred at `(cx, cy)` reaching `r` along each axis.
fn sparkle(cx: f32, cy: f32, r: f32) -> Contour {
    let k = r * 0.28;
    closed(vec![[cx, cy - r], [cx + k, cy - k], [cx + r, cy], [cx + k, cy + k], [cx, cy + r], [cx - k, cy + k], [cx - r, cy], [cx - k, cy - k]])
}

impl Icon {
    /// Every icon, for tests and previews.
    pub const ALL: [Icon; 70] = [
        Icon::Layers,
        Icon::EditHud,
        Icon::Sliders,
        Icon::User,
        Icon::Keyboard,
        Icon::Camera,
        Icon::Sparkles,
        Icon::Gear,
        Icon::Refresh,
        Icon::Star,
        Icon::MapPin,
        Icon::Cloud,
        Icon::Bolt,
        Icon::Wrench,
        Icon::Grid,
        Icon::Bell,
        Icon::Heart,
        Icon::Eye,
        Icon::Search,
        Icon::Close,
        Icon::Back,
        Icon::Forward,
        Icon::Mouse,
        Icon::Gauge,
        Icon::ZoomIn,
        Icon::Shield,
        Icon::Flask,
        Icon::Signal,
        Icon::Crosshair,
        Icon::Sun,
        Icon::Compass,
        Icon::Clock,
        Icon::Speed,
        Icon::Timer,
        Icon::Hourglass,
        Icon::Calendar,
        Icon::Angle,
        Icon::Ruler,
        Icon::Swords,
        Icon::Totem,
        Icon::Cube,
        Icon::Backpack,
        Icon::List,
        Icon::Chunk,
        Icon::Droplet,
        Icon::Diamond,
        Icon::Scoreboard,
        Icon::Target,
        Icon::ArrowTop,
        Icon::Captions,
        Icon::Brackets,
        Icon::Bulb,
        Icon::Flame,
        Icon::Box,
        Icon::Tag,
        Icon::Rings,
        Icon::Sprint,
        Icon::Crouch,
        Icon::Pointer,
        Icon::Mute,
        Icon::Funnel,
        Icon::Flag,
        Icon::Bomb,
        Icon::TextT,
        Icon::Hex,
        Icon::Perspective,
        Icon::Glass,
        Icon::Hash,
        Icon::Moon,
        Icon::Dash,
    ];

    /// How wide the icon's strokes are, in grid units.
    #[must_use]
    pub fn weight(self) -> f32 {
        1.9
    }

    /// The icon's strokes as polylines on the 24 by 24 grid; a stroke that returns to its first point is closed.
    #[must_use]
    pub fn contours(self) -> Vec<Contour> {
        match self {
            Icon::Layers => vec![
                closed(vec![[12.0, 3.0], [21.0, 8.0], [12.0, 13.0], [3.0, 8.0]]),
                vec![[3.0, 12.5], [12.0, 17.5], [21.0, 12.5]],
                vec![[3.0, 17.0], [12.0, 22.0], [21.0, 17.0]],
            ],
            Icon::EditHud => vec![rrect(3.0, 3.0, 18.0, 18.0, 3.5), line([9.5, 3.0], [9.5, 21.0])],
            Icon::Sliders => vec![
                line([3.0, 8.0], [5.6, 8.0]),
                circle(8.0, 8.0, 2.4),
                line([10.4, 8.0], [21.0, 8.0]),
                line([3.0, 16.0], [13.6, 16.0]),
                circle(16.0, 16.0, 2.4),
                line([18.4, 16.0], [21.0, 16.0]),
            ],
            Icon::User => vec![circle(12.0, 8.0, 3.8), arc(12.0, 21.0, 8.0, 180.0, 360.0)],
            Icon::Keyboard => vec![
                rrect(2.5, 6.0, 19.0, 12.0, 3.0),
                line([6.5, 10.0], [7.2, 10.0]),
                line([10.0, 10.0], [10.7, 10.0]),
                line([13.5, 10.0], [14.2, 10.0]),
                line([17.0, 10.0], [17.7, 10.0]),
                line([7.5, 14.0], [16.5, 14.0]),
            ],
            Icon::Camera => vec![
                rrect(2.5, 7.0, 19.0, 13.0, 3.0),
                vec![[8.0, 7.0], [9.6, 4.2], [14.4, 4.2], [16.0, 7.0]],
                circle(12.0, 13.5, 3.6),
            ],
            Icon::Sparkles => vec![sparkle(10.0, 13.0, 8.0), sparkle(19.0, 5.0, 3.2)],
            Icon::Gear => {
                let mut teeth = Vec::new();
                for k in 0..8 {
                    let base = k as f32 * 45.0;
                    for (r, d) in [(7.2, -15.0), (9.4, -9.0), (9.4, 9.0), (7.2, 15.0)] {
                        let a = (base + d) * PI / 180.0;
                        teeth.push([12.0 + r * a.cos(), 12.0 + r * a.sin()]);
                    }
                }
                vec![closed(teeth), circle(12.0, 12.0, 3.0)]
            }
            Icon::Refresh => {
                // Two arcs chasing each other round a circle, each ending in an arrowhead.
                let half = |from: f32, to: f32| -> Vec<Contour> {
                    let ring = arc(12.0, 12.0, 8.0, from, to);
                    let end = *ring.last().unwrap_or(&[12.0, 4.0]);
                    let a = to * PI / 180.0;
                    let dir = [-a.sin(), a.cos()];
                    let turn = |angle: f32| {
                        let (c, s) = (angle.cos(), angle.sin());
                        [dir[0] * c - dir[1] * s, dir[0] * s + dir[1] * c]
                    };
                    let (l, r) = (turn(150.0 * PI / 180.0), turn(-150.0 * PI / 180.0));
                    vec![ring, vec![[end[0] + l[0] * 4.4, end[1] + l[1] * 4.4], end, [end[0] + r[0] * 4.4, end[1] + r[1] * 4.4]]]
                };
                let mut v = half(195.0, 325.0);
                v.extend(half(15.0, 145.0));
                v
            }
            Icon::Star => vec![star(12.0, 12.8, 9.6, 4.3, 5)],
            Icon::MapPin => {
                let mut pin = arc(12.0, 10.0, 7.0, 160.0, 380.0);
                pin.push([12.0, 21.5]);
                vec![closed(pin), circle(12.0, 10.0, 2.6)]
            }
            Icon::Cloud => {
                let mut c = arc(6.5, 15.5, 3.5, 90.0, 270.0);
                c.extend(arc(12.0, 11.5, 4.8, 180.0, 360.0));
                c.extend(arc(17.5, 15.5, 3.5, 270.0, 450.0));
                vec![closed(c)]
            }
            Icon::Bolt => vec![closed(vec![[13.0, 2.5], [5.0, 13.5], [11.5, 13.5], [10.5, 21.5], [19.0, 10.0], [12.5, 10.0]])],
            Icon::Wrench => vec![arc(16.5, 7.5, 4.8, 345.0, 645.0), line([13.2, 10.8], [4.5, 19.5])],
            Icon::Grid => vec![rrect(3.0, 3.0, 18.0, 18.0, 3.5), line([3.0, 9.5], [21.0, 9.5]), line([9.5, 9.5], [9.5, 21.0])],
            Icon::Bell => {
                let mut body = arc(12.0, 8.5, 6.0, 180.0, 360.0);
                body.extend([[18.5, 13.0], [21.0, 17.0], [3.0, 17.0], [5.5, 13.0]]);
                vec![closed(body), arc(12.0, 19.5, 1.9, 0.0, 180.0)]
            }
            Icon::Heart => {
                let mut h = arc(8.0, 8.5, 4.6, 150.0, 360.0);
                h.extend(arc(16.0, 8.5, 4.6, 180.0, 390.0));
                h.push([12.0, 20.5]);
                vec![closed(h)]
            }
            Icon::Eye => {
                let top: Contour = (0..=16).map(|i| {
                    let t = i as f32 / 16.0;
                    [2.0 + 20.0 * t, 12.0 - 6.5 * (PI * t).sin()]
                }).collect();
                let bottom: Contour = (0..=16).rev().map(|i| {
                    let t = i as f32 / 16.0;
                    [2.0 + 20.0 * t, 12.0 + 6.5 * (PI * t).sin()]
                }).collect();
                let mut lens = top;
                lens.extend(bottom);
                vec![closed(lens), circle(12.0, 12.0, 3.0)]
            }
            Icon::Search => vec![circle(10.5, 10.5, 6.5), line([15.4, 15.4], [20.5, 20.5])],
            Icon::Close => vec![line([6.0, 6.0], [18.0, 18.0]), line([18.0, 6.0], [6.0, 18.0])],
            Icon::Back => vec![line([19.5, 12.0], [5.0, 12.0]), vec![[11.0, 6.0], [5.0, 12.0], [11.0, 18.0]]],
            Icon::Forward => vec![line([4.5, 12.0], [19.0, 12.0]), vec![[13.0, 6.0], [19.0, 12.0], [13.0, 18.0]]],
            Icon::Mouse => vec![rrect(6.5, 2.5, 11.0, 19.0, 5.5), line([12.0, 2.5], [12.0, 9.0]), line([12.0, 5.5], [12.0, 7.5])],
            Icon::Gauge => vec![arc(12.0, 14.0, 9.0, 180.0, 360.0), line([3.0, 14.0], [5.0, 14.0]), line([19.0, 14.0], [21.0, 14.0]), line([12.0, 14.0], [16.5, 9.5]), circle(12.0, 14.0, 1.2)],
            Icon::ZoomIn => vec![circle(10.5, 10.5, 6.5), line([15.4, 15.4], [20.5, 20.5]), line([10.5, 7.5], [10.5, 13.5]), line([7.5, 10.5], [13.5, 10.5])],
            Icon::Shield => vec![closed(vec![[12.0, 2.5], [19.5, 5.5], [19.5, 12.0], [12.0, 21.5], [4.5, 12.0], [4.5, 5.5]])],
            Icon::Flask => vec![vec![[9.5, 3.0], [9.5, 10.0], [4.5, 19.0], [19.5, 19.0], [14.5, 10.0], [14.5, 3.0]], line([8.0, 3.0], [16.0, 3.0]), line([7.0, 15.0], [17.0, 15.0])],
            Icon::Signal => vec![line([5.0, 19.0], [5.0, 16.0]), line([10.0, 19.0], [10.0, 12.0]), line([15.0, 19.0], [15.0, 8.0]), line([20.0, 19.0], [20.0, 4.0])],
            Icon::Crosshair => vec![circle(12.0, 12.0, 6.5), line([12.0, 2.5], [12.0, 7.0]), line([12.0, 17.0], [12.0, 21.5]), line([2.5, 12.0], [7.0, 12.0]), line([17.0, 12.0], [21.5, 12.0])],
            Icon::Sun => {
                let mut v = vec![circle(12.0, 12.0, 4.2)];
                for k in 0..8 {
                    let a = k as f32 * PI / 4.0;
                    v.push(line([12.0 + 7.4 * a.cos(), 12.0 + 7.4 * a.sin()], [12.0 + 10.0 * a.cos(), 12.0 + 10.0 * a.sin()]));
                }
                v
            }
            Icon::Compass => vec![circle(12.0, 12.0, 9.5), closed(vec![[16.0, 8.0], [13.6, 13.6], [8.0, 16.0], [10.4, 10.4]])],
            Icon::Clock => vec![circle(12.0, 12.0, 9.5), vec![[12.0, 6.5], [12.0, 12.0], [16.0, 14.2]]],
            Icon::Speed => vec![vec![[5.5, 6.0], [11.5, 12.0], [5.5, 18.0]], vec![[12.5, 6.0], [18.5, 12.0], [12.5, 18.0]]],
            Icon::Timer => vec![circle(12.0, 13.5, 7.5), line([12.0, 13.5], [15.2, 10.2]), line([9.5, 2.5], [14.5, 2.5]), line([12.0, 2.5], [12.0, 6.0])],
            Icon::Hourglass => vec![closed(vec![[6.0, 3.0], [18.0, 3.0], [18.0, 7.0], [12.0, 12.0], [18.0, 17.0], [18.0, 21.0], [6.0, 21.0], [6.0, 17.0], [12.0, 12.0], [6.0, 7.0]])],
            Icon::Calendar => vec![rrect(3.5, 4.5, 17.0, 17.0, 3.0), line([3.5, 10.0], [20.5, 10.0]), line([8.0, 2.5], [8.0, 6.5]), line([16.0, 2.5], [16.0, 6.5])],
            Icon::Angle => vec![line([4.0, 20.0], [21.0, 20.0]), line([4.0, 20.0], [17.5, 6.5]), arc(4.0, 20.0, 9.0, -45.0, 0.0)],
            Icon::Ruler => vec![rrect(2.5, 8.0, 19.0, 8.0, 2.0), line([7.0, 8.0], [7.0, 11.5]), line([11.0, 8.0], [11.0, 12.5]), line([15.0, 8.0], [15.0, 11.5]), line([19.0, 8.0], [19.0, 12.5])],
            Icon::Swords => vec![
                line([4.0, 4.0], [15.5, 15.5]),
                line([11.5, 17.5], [17.5, 11.5]),
                line([15.5, 15.5], [19.5, 19.5]),
                line([20.0, 4.0], [8.5, 15.5]),
                line([6.5, 11.5], [12.5, 17.5]),
                line([8.5, 15.5], [4.5, 19.5]),
            ],
            Icon::Totem => vec![circle(12.0, 6.5, 3.2), line([12.0, 9.7], [12.0, 20.0]), line([5.0, 12.5], [19.0, 12.5]), line([8.5, 20.5], [15.5, 20.5])],
            Icon::Cube => vec![closed(vec![[12.0, 3.0], [20.0, 7.5], [12.0, 12.0], [4.0, 7.5]]), vec![[4.0, 7.5], [4.0, 16.5], [12.0, 21.0], [20.0, 16.5], [20.0, 7.5]], line([12.0, 12.0], [12.0, 21.0])],
            Icon::Backpack => vec![rrect(5.0, 9.0, 14.0, 12.0, 3.5), arc(12.0, 8.0, 3.2, 180.0, 360.0), line([5.0, 13.5], [19.0, 13.5]), rrect(8.5, 15.5, 7.0, 3.5, 1.2)],
            Icon::List => vec![line([8.0, 6.0], [21.0, 6.0]), line([8.0, 12.0], [21.0, 12.0]), line([8.0, 18.0], [21.0, 18.0]), line([3.5, 6.0], [4.4, 6.0]), line([3.5, 12.0], [4.4, 12.0]), line([3.5, 18.0], [4.4, 18.0])],
            Icon::Chunk => vec![rrect(3.0, 3.0, 18.0, 18.0, 2.5), line([12.0, 3.0], [12.0, 21.0]), line([3.0, 12.0], [21.0, 12.0])],
            Icon::Droplet => {
                let mut d = arc(12.0, 14.5, 6.5, -50.0, 230.0);
                d.push([12.0, 3.5]);
                vec![closed(d)]
            }
            Icon::Diamond => vec![closed(vec![[12.0, 3.0], [20.5, 12.0], [12.0, 21.0], [3.5, 12.0]]), closed(vec![[12.0, 8.0], [16.0, 12.0], [12.0, 16.0], [8.0, 12.0]])],
            Icon::Scoreboard => vec![rrect(4.0, 3.0, 16.0, 18.0, 2.5), line([8.0, 8.0], [16.0, 8.0]), line([8.0, 12.0], [16.0, 12.0]), line([8.0, 16.0], [13.0, 16.0])],
            Icon::Target => vec![circle(12.0, 12.0, 9.0), circle(12.0, 12.0, 4.6), circle(12.0, 12.0, 0.5)],
            Icon::ArrowTop => vec![line([6.0, 3.5], [18.0, 3.5]), line([12.0, 21.0], [12.0, 8.0]), vec![[7.0, 13.0], [12.0, 8.0], [17.0, 13.0]]],
            Icon::Captions => vec![rrect(2.5, 5.0, 19.0, 14.0, 3.0), line([6.5, 11.0], [10.0, 11.0]), line([12.5, 11.0], [17.5, 11.0]), line([6.5, 15.0], [8.5, 15.0]), line([11.0, 15.0], [17.5, 15.0])],
            Icon::Brackets => vec![
                vec![[3.5, 8.5], [3.5, 3.5], [8.5, 3.5]],
                vec![[15.5, 3.5], [20.5, 3.5], [20.5, 8.5]],
                vec![[20.5, 15.5], [20.5, 20.5], [15.5, 20.5]],
                vec![[8.5, 20.5], [3.5, 20.5], [3.5, 15.5]],
            ],
            Icon::Bulb => {
                let mut b = arc(12.0, 9.5, 6.2, 140.0, 400.0);
                b.extend([[14.6, 17.5], [9.4, 17.5]]);
                vec![b, line([10.0, 20.5], [14.0, 20.5])]
            }
            Icon::Flame => vec![closed(vec![[12.0, 2.5], [17.0, 9.0], [18.5, 14.0], [16.0, 19.0], [12.0, 21.0], [8.0, 19.0], [5.5, 14.0], [7.5, 10.0], [9.5, 12.5], [10.5, 7.0]])],
            Icon::Box => vec![rrect(4.5, 4.5, 15.0, 15.0, 2.5), rrect(9.0, 9.0, 6.0, 6.0, 1.0)],
            Icon::Tag => vec![closed(vec![[4.0, 12.5], [12.5, 4.0], [20.0, 4.0], [20.0, 11.5], [11.5, 20.0]]), circle(15.5, 8.5, 1.4)],
            Icon::Rings => vec![circle(12.0, 12.0, 2.8), circle(12.0, 12.0, 6.4), circle(12.0, 12.0, 10.0)],
            Icon::Sprint => vec![
                circle(15.0, 4.5, 2.0),
                line([14.0, 8.0], [11.0, 14.0]),
                line([14.0, 8.0], [18.0, 11.5]),
                line([13.0, 9.2], [8.5, 10.5]),
                vec![[11.0, 14.0], [14.5, 17.5], [13.5, 21.5]],
                vec![[11.0, 14.0], [7.0, 17.0], [4.5, 16.5]],
            ],
            Icon::Crouch => vec![
                circle(9.0, 6.0, 2.0),
                line([9.0, 9.0], [10.5, 15.0]),
                line([9.5, 10.5], [14.5, 12.5]),
                vec![[10.5, 15.0], [15.0, 15.5], [15.0, 20.5]],
                vec![[10.5, 15.0], [8.0, 19.5], [10.5, 21.0]],
            ],
            Icon::Pointer => vec![closed(vec![[5.0, 3.0], [18.5, 11.5], [12.0, 13.0], [8.5, 20.5]])],
            Icon::Mute => vec![closed(vec![[3.5, 9.5], [7.5, 9.5], [12.0, 5.5], [12.0, 18.5], [7.5, 14.5], [3.5, 14.5]]), line([15.5, 9.5], [20.5, 14.5]), line([20.5, 9.5], [15.5, 14.5])],
            Icon::Funnel => vec![closed(vec![[3.5, 4.0], [20.5, 4.0], [14.0, 12.5], [14.0, 19.5], [10.0, 17.5], [10.0, 12.5]])],
            Icon::Flag => vec![line([5.0, 21.5], [5.0, 3.5]), closed(vec![[5.0, 4.0], [19.0, 4.0], [15.5, 8.5], [19.0, 13.0], [5.0, 13.0]])],
            Icon::Bomb => vec![circle(10.5, 14.0, 7.0), vec![[15.5, 9.0], [17.6, 6.6], [20.0, 6.6]], sparkle(20.4, 3.8, 2.4)],
            Icon::TextT => vec![line([5.0, 5.5], [19.0, 5.5]), line([12.0, 5.5], [12.0, 19.0]), line([8.5, 19.0], [15.5, 19.0])],
            Icon::Hex => vec![closed((0..6).map(|i| {
                let a = (i as f32 * 60.0 - 90.0) * PI / 180.0;
                [12.0 + 9.5 * a.cos(), 12.0 + 9.5 * a.sin()]
            }).collect()), circle(12.0, 12.0, 3.0)],
            Icon::Perspective => {
                let a = 28.0_f32 * PI / 180.0;
                let orbit: Contour = (0..=24)
                    .map(|i| {
                        let t = i as f32 / 24.0 * 2.0 * PI;
                        let (x, y) = (9.8 * t.cos(), 4.4 * t.sin());
                        [12.0 + x * a.cos() - y * a.sin(), 12.0 + x * a.sin() + y * a.cos()]
                    })
                    .collect();
                vec![orbit, circle(12.0, 12.0, 2.8)]
            }
            Icon::Glass => vec![rrect(4.0, 4.0, 16.0, 16.0, 2.5), line([9.0, 16.5], [15.5, 9.5]), line([7.0, 12.0], [11.0, 8.0])],
            Icon::Hash => vec![line([9.0, 4.0], [7.5, 20.0]), line([16.5, 4.0], [15.0, 20.0]), line([4.0, 9.0], [20.0, 9.0]), line([4.5, 15.0], [19.5, 15.0])],
            Icon::Moon => {
                let mut c = arc(12.0, 12.0, 9.0, 270.0, 0.0);
                c.extend(arc(21.0, 3.0, 9.0, 90.0, 180.0));
                vec![closed(c)]
            }
            Icon::Dash => {
                let mut figure = Icon::Sprint.contours();
                figure.extend([line([2.0, 9.0], [6.0, 9.0]), line([2.0, 12.5], [5.0, 12.5]), line([3.5, 19.5], [7.0, 19.5])]);
                figure
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The extent of an icon's strokes as `(width, height)`.
    fn extent(icon: Icon) -> (f32, f32) {
        let points: Vec<[f32; 2]> = icon.contours().into_iter().flatten().collect();
        let range = |axis: usize| {
            let lo = points.iter().map(|p| p[axis]).fold(f32::MAX, f32::min);
            let hi = points.iter().map(|p| p[axis]).fold(f32::MIN, f32::max);
            hi - lo
        };
        (range(0), range(1))
    }

    #[test]
    fn square_icons_are_square() {
        for icon in [Icon::EditHud, Icon::Grid, Icon::Chunk, Icon::Box, Icon::Glass, Icon::Brackets] {
            let (w, h) = extent(icon);
            assert!((w - h).abs() < 0.01, "{icon:?} is {w} wide and {h} high");
        }
    }

    /// The area of a polygon (positive).
    fn polygon_area(points: &[[f32; 2]]) -> f32 {
        let n = points.len();
        (0..n).map(|i| points[i][0] * points[(i + 1) % n][1] - points[(i + 1) % n][0] * points[i][1]).sum::<f32>().abs() / 2.0
    }

    #[test]
    fn converted_icons_fill_exactly_the_area_they_describe() {
        use crate::icon_art::Art;
        use crate::style::Color;
        for art in Art::ALL {
            for shape in art.shapes() {
                let contours: Vec<Vec<[f32; 2]>> = shape.iter().map(|ring| ring.to_vec()).collect();
                let expected = polygon_area(&contours[0]) - contours[1..].iter().map(|h| polygon_area(h)).sum::<f32>();
                let mesh = crate::tess::path_fill(&contours, Color::hex(0xFFFFFF, 1.0));
                let filled: f32 = mesh
                    .indices
                    .chunks(3)
                    .map(|t| {
                        let (a, b, c) = (mesh.vertices[t[0] as usize].pos, mesh.vertices[t[1] as usize].pos, mesh.vertices[t[2] as usize].pos);
                        ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() / 2.0
                    })
                    .sum();
                assert!((filled - expected).abs() <= expected * 0.03 + 0.02, "{art:?} fills {filled} of {expected}");
            }
        }
    }

    #[test]
    fn every_pixel_run_sits_inside_its_grid() {
        use crate::pixel_art::HONEYCOMB;
        for art in [&HONEYCOMB] {
            assert!(!art.runs.is_empty());
            for [x, y, w] in art.runs {
                assert!(*w > 0 && u16::from(*x) + u16::from(*w) <= u16::from(art.size) && *y < art.size, "run {x},{y},{w} leaves the {} grid", art.size);
            }
        }
    }

    #[test]
    fn every_icon_has_strokes_inside_the_grid() {
        for icon in Icon::ALL {
            let contours = icon.contours();
            assert!(!contours.is_empty(), "{icon:?} draws nothing");
            for contour in contours {
                assert!(contour.len() >= 2, "{icon:?} has a stroke of one point");
                for p in contour {
                    assert!((0.0..=GRID).contains(&p[0]) && (0.0..=GRID).contains(&p[1]), "{icon:?} leaves the grid at {p:?}");
                }
            }
        }
    }
}
