//! Colours, fills, corner radii, borders and shadows.

/// A colour as red, green, blue and alpha, each 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    #[must_use]
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// From `0xRRGGBB` and an alpha.
    #[must_use]
    pub fn hex(rgb: u32, a: f32) -> Self {
        let c = |shift: u32| ((rgb >> shift) & 255) as f32 / 255.0;
        Self { r: c(16), g: c(8), b: c(0), a }
    }

    /// The colour `t` of the way to `other` (0 is this one, 1 is `other`), in every channel.
    #[must_use]
    pub fn mix(self, other: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let l = |a: f32, b: f32| a + (b - a) * t;
        Color { r: l(self.r, other.r), g: l(self.g, other.g), b: l(self.b, other.b), a: l(self.a, other.a) }
    }

    /// The same colour with its alpha multiplied by `factor`.
    #[must_use]
    pub fn faded(self, factor: f32) -> Color {
        Color { a: self.a * factor.clamp(0.0, 1.0), ..self }
    }
}

/// How a shape is filled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fill {
    None,
    Solid(Color),
    /// From `from` to `to` along `angle` degrees clockwise from straight down.
    Gradient { from: Color, to: Color, angle: f32 },
}

/// Corner radii, clockwise from the top left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Radii {
    pub tl: f32,
    pub tr: f32,
    pub br: f32,
    pub bl: f32,
}

impl Radii {
    #[must_use]
    pub const fn all(r: f32) -> Self {
        Self { tl: r, tr: r, br: r, bl: r }
    }

    /// Round only the top corners.
    #[must_use]
    pub const fn top(r: f32) -> Self {
        Self { tl: r, tr: r, br: 0.0, bl: 0.0 }
    }

    /// Round only the bottom corners.
    #[must_use]
    pub const fn bottom(r: f32) -> Self {
        Self { tl: 0.0, tr: 0.0, br: r, bl: r }
    }

    /// Whether all four corners have the same radius.
    #[must_use]
    pub fn is_uniform(&self) -> bool {
        self.tl == self.tr && self.tr == self.br && self.br == self.bl
    }

    /// Each radius limited to half the shorter side, so adjacent corners never overlap.
    #[must_use]
    pub fn fitted(self, w: f32, h: f32) -> Radii {
        let cap = (w.min(h) / 2.0).max(0.0);
        Radii { tl: self.tl.min(cap), tr: self.tr.min(cap), br: self.br.min(cap), bl: self.bl.min(cap) }
    }
}

/// An outline of `width` inside the shape's edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Border {
    pub width: f32,
    pub color: Color,
}

/// A soft shadow under a shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub offset: [f32; 2],
    pub blur: f32,
    pub color: Color,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixing_runs_from_one_colour_to_the_other_and_clamps() {
        let (a, b) = (Color::hex(0x000000, 1.0), Color::hex(0xFFFFFF, 0.0));
        assert_eq!(a.mix(b, 0.0), a);
        assert_eq!(a.mix(b, 1.0), b);
        assert!((a.mix(b, 0.5).r - 0.5).abs() < 1e-6 && (a.mix(b, 0.5).a - 0.5).abs() < 1e-6);
        assert_eq!(a.mix(b, 7.0), b);
    }

    #[test]
    fn radii_are_fitted_to_half_the_shorter_side() {
        let r = Radii::all(40.0).fitted(100.0, 20.0);
        assert_eq!(r, Radii::all(10.0));
        assert!(Radii::top(8.0).fitted(100.0, 100.0) != Radii::all(8.0));
        assert!(Radii::all(3.0).is_uniform() && !Radii::bottom(3.0).is_uniform());
    }
}
