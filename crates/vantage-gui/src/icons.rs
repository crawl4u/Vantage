//! What the menu draws as an icon: artwork converted to vector shapes, pixel art, or a glyph of the icon font for
//! the places the converted set has no icon.

/// The side of the square grid every converted icon sits on.
pub const GRID: f32 = 24.0;

/// An icon of the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    /// An icon converted from artwork.
    Art(crate::icon_art::Art),
    /// A pixel-art icon, drawn square-edged on its own grid.
    Pixel(&'static crate::pixel_art::PixelArt),
    /// A glyph of the menu's icon font.
    Font(&'static str),
    /// A line icon from `icon_custom`, by name.
    Line(&'static str),
}

impl From<crate::icon_art::Art> for Glyph {
    fn from(art: crate::icon_art::Art) -> Self {
        Glyph::Art(art)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icon_art::Art;

    /// The area of a polygon (positive).
    fn polygon_area(points: &[[f32; 2]]) -> f32 {
        let n = points.len();
        (0..n).map(|i| points[i][0] * points[(i + 1) % n][1] - points[(i + 1) % n][0] * points[i][1]).sum::<f32>().abs() / 2.0
    }

    #[test]
    fn converted_icons_fill_exactly_the_area_they_describe() {
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
    fn converted_icons_stay_inside_the_grid() {
        for art in Art::ALL {
            for ring in art.shapes().iter().flat_map(|shape| shape.iter()) {
                for p in *ring {
                    assert!((-0.1..=GRID + 0.1).contains(&p[0]) && (-0.1..=GRID + 0.1).contains(&p[1]), "{art:?} leaves the grid at {p:?}");
                }
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
}
