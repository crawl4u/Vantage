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
    Brush,
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
}

type Contour = Vec<[f32; 2]>;

/// Points of a circular arc from `from` to `to` degrees (y points down, 0 is east, 90 south).
fn arc(cx: f32, cy: f32, r: f32, from: f32, to: f32) -> Contour {
    let steps = (((to - from).abs() / 12.0).ceil() as usize).max(2);
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
    pub const ALL: [Icon; 23] = [
        Icon::Layers,
        Icon::EditHud,
        Icon::Sliders,
        Icon::User,
        Icon::Keyboard,
        Icon::Camera,
        Icon::Brush,
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
    ];

    /// The icon's strokes as polylines on the 24 by 24 grid; a stroke that returns to its first point is closed.
    #[must_use]
    pub fn contours(self) -> Vec<Contour> {
        match self {
            Icon::Layers => vec![
                closed(vec![[12.0, 3.0], [21.0, 8.0], [12.0, 13.0], [3.0, 8.0]]),
                vec![[3.0, 12.5], [12.0, 17.5], [21.0, 12.5]],
                vec![[3.0, 17.0], [12.0, 22.0], [21.0, 17.0]],
            ],
            Icon::EditHud => vec![rrect(3.0, 4.0, 18.0, 16.0, 3.5), line([9.5, 4.0], [9.5, 20.0])],
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
            Icon::Brush => vec![line([20.5, 3.5], [11.8, 12.2]), closed(vec![[11.8, 12.2], [14.6, 15.0], [12.2, 19.2], [7.0, 20.6], [3.4, 20.6], [5.4, 16.8], [8.4, 13.4]])],
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
                let mut ring = arc(12.0, 12.0, 8.0, 30.0, 300.0);
                let end = *ring.last().unwrap_or(&[12.0, 4.0]);
                ring.shrink_to_fit();
                vec![ring, vec![[end[0] + 4.2, end[1] + 0.4], end, [end[0] + 0.4, end[1] + 4.4]]]
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
            Icon::Wrench => vec![line([4.0, 20.0], [12.4, 11.6]), arc(16.0, 8.0, 4.6, 140.0, 410.0)],
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
