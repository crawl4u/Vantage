//! Line icons the converted set (`icon_art`) has none of, drawn as stroked outlines on the same 24 by 24 grid and
//! with the same stroke as it, so they sit beside the converted icons at the same size and weight.

use std::f32::consts::PI;

/// The stroke width of every icon here, in grid units.
pub const STROKE: f32 = 1.9;

/// The icons that can be asked for by name.
pub const NAMES: [&str; 3] = ["camera", "map-pin", "sparkle"];

/// Points of a circle of radius `r` about `(cx, cy)`, closed by repeating the first point.
fn circle(cx: f32, cy: f32, r: f32) -> Vec<[f32; 2]> {
    const STEPS: usize = 28;
    (0..=STEPS).map(|i| {
        let a = 2.0 * PI * i as f32 / STEPS as f32;
        [cx + r * a.cos(), cy + r * a.sin()]
    }).collect()
}

/// A closed outline through `points`.
fn closed(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let mut out = points.to_vec();
    out.push(points[0]);
    out
}

/// The strokes of icon `name`, or none for a name this set does not have.
#[must_use]
pub fn strokes(name: &str) -> Option<Vec<Vec<[f32; 2]>>> {
    Some(match name {
        "camera" => vec![
            closed(&[[3.0, 7.5], [7.0, 7.5], [9.0, 4.5], [15.0, 4.5], [17.0, 7.5], [21.0, 7.5], [21.0, 19.5], [3.0, 19.5]]),
            circle(12.0, 13.5, 3.6),
        ],
        "map-pin" => {
            // A drop: the top is an arc about (12, 10), and it narrows to the tip.
            let mut drop: Vec<[f32; 2]> = (0..=20)
                .map(|i| {
                    let a = (-30.0 + 240.0 * i as f32 / 20.0).to_radians();
                    [12.0 + 7.5 * a.cos(), 10.0 - 7.5 * a.sin()]
                })
                .collect();
            drop.push([12.0, 21.5]);
            vec![closed(&drop), circle(12.0, 10.0, 2.8)]
        }
        "sparkle" => vec![closed(&[[12.0, 2.5], [14.2, 9.8], [21.5, 12.0], [14.2, 14.2], [12.0, 21.5], [9.8, 14.2], [2.5, 12.0], [9.8, 9.8]])],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icons::GRID;

    #[test]
    fn every_named_icon_has_strokes_inside_the_grid() {
        for name in NAMES {
            let strokes = strokes(name).unwrap_or_else(|| panic!("{name} has no strokes"));
            for p in strokes.iter().flatten() {
                let margin = STROKE / 2.0;
                assert!((margin..=GRID - margin).contains(&p[0]) && (margin..=GRID - margin).contains(&p[1]), "{name} leaves the grid at {p:?}");
            }
        }
    }
}
