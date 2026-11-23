//! The menu's design tokens: palette, shapes, type scale, spacing and the contrast rule.
//!
//! Every measurement is in "design units", the scale the menu was drawn at; [`px`] turns one into
//! the GUI pixels the panel document uses. Nothing else in the menu hard-codes a colour, radius or
//! gap, so a theme is one [`Palette`] value.

/// A colour as red, green, blue and alpha, each in `0..=1`.
pub type Rgba = [f32; 4];

/// One design unit in GUI pixels.
pub const UNIT: f32 = 0.5;

/// A design measurement in GUI pixels.
#[must_use]
pub fn px(units: f32) -> f32 {
    units * UNIT
}

/// An opaque or translucent colour from `0xRRGGBB` and an alpha in `0..=1`.
#[must_use]
pub const fn rgb(hex: u32, alpha: f32) -> Rgba {
    [
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
        alpha,
    ]
}

/// The colours of one menu theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// The window and sidebar fill.
    pub panel: Rgba,
    /// A surface set on the panel, such as the content pane.
    pub surface: Rgba,
    /// A card or control raised above a surface.
    pub raised: Rgba,
    /// A quiet fill: a disabled control or an empty slot.
    pub dim: Rgba,
    /// The inside of a text field or a progress track.
    pub field: Rgba,
    /// Hairlines between regions and around controls.
    pub border: Rgba,
    /// Ordinary text.
    pub text: Rgba,
    /// Secondary text and captions.
    pub muted: Rgba,
    /// Text that must stand out: titles and active entries.
    pub highlight: Rgba,
    /// The accent: switched-on bars, the selected entry, focus rings.
    pub accent: Rgba,
    /// Text drawn on the accent.
    pub on_accent: Rgba,
    /// The favourite star.
    pub star: Rgba,
    /// A destructive action.
    pub danger: Rgba,
}

impl Palette {
    /// The default dark theme: translucent black glass with a purple accent.
    #[must_use]
    pub const fn dark() -> Self {
        Self {
            panel: rgb(0x020203, 0.88),
            surface: rgb(0x0C0C12, 0.0),
            raised: rgb(0x2A2A36, 0.50),
            dim: rgb(0x3A3A4A, 0.60),
            field: rgb(0x000000, 0.30),
            border: rgb(0xFFFFFF, 0.10),
            text: rgb(0xE3E8FA, 1.0),
            muted: rgb(0x8E96AC, 1.0),
            highlight: rgb(0xFFFFFF, 1.0),
            accent: rgb(0x7C3AED, 1.0),
            on_accent: rgb(0xFFFFFF, 1.0),
            star: rgb(0xFFD700, 1.0),
            danger: rgb(0xE5484D, 1.0),
        }
    }

    /// A light theme with the same roles.
    #[must_use]
    pub const fn light() -> Self {
        Self {
            panel: rgb(0xF1F1F6, 0.90),
            surface: rgb(0xFFFFFF, 0.0),
            raised: rgb(0xFFFFFF, 0.62),
            dim: rgb(0xD9D9E4, 0.80),
            field: rgb(0xFFFFFF, 0.70),
            border: rgb(0x000000, 0.12),
            text: rgb(0x1B1B26, 1.0),
            muted: rgb(0x4B4B5E, 1.0),
            highlight: rgb(0x000000, 1.0),
            accent: rgb(0x7C3AED, 1.0),
            on_accent: rgb(0xFFFFFF, 1.0),
            star: rgb(0xB57D00, 1.0),
            danger: rgb(0xC62A2F, 1.0),
        }
    }
}

/// Corner radii, in design units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Radii {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
}

/// The menu's corner radii.
pub const RADII: Radii = Radii {
    xs: 8.0,
    sm: 10.0,
    md: 16.0,
    lg: 28.0,
};

impl Radii {
    /// The radius of a shape inset by `inset` inside one with radius `outer`, so nested corners stay
    /// concentric. Never below `least`.
    #[must_use]
    pub fn concentric(outer: f32, inset: f32, least: f32) -> f32 {
        (outer - inset).max(least)
    }
}

/// A named text style: its size in design units, whether it is semi-bold and its letter spacing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    pub bold: bool,
    pub spacing: f32,
}

/// The type scale, largest first.
pub mod text {
    use super::TextStyle;

    pub const DISPLAY: TextStyle = TextStyle { size: 24.0, bold: true, spacing: 0.0 };
    pub const TITLE: TextStyle = TextStyle { size: 18.0, bold: true, spacing: 0.0 };
    pub const HEADING: TextStyle = TextStyle { size: 15.0, bold: true, spacing: 0.0 };
    pub const BODY: TextStyle = TextStyle { size: 14.0, bold: false, spacing: 0.0 };
    pub const LABEL: TextStyle = TextStyle { size: 13.0, bold: false, spacing: 0.0 };
    pub const SMALL: TextStyle = TextStyle { size: 12.0, bold: false, spacing: 0.0 };
    pub const CAPTION: TextStyle = TextStyle { size: 11.0, bold: false, spacing: 0.0 };
    /// A small all-caps section heading.
    pub const EYEBROW: TextStyle = TextStyle { size: 11.0, bold: false, spacing: 1.5 };
}

/// The spacing scale, in design units.
pub mod space {
    pub const XXS: f32 = 2.0;
    pub const XS: f32 = 4.0;
    pub const S: f32 = 6.0;
    pub const M: f32 = 8.0;
    pub const L: f32 = 12.0;
    pub const XL: f32 = 16.0;
    pub const XXL: f32 = 24.0;
    /// The margin between the content pane's edge and its content.
    pub const GUTTER: f32 = 25.0;
}

/// Relative luminance of an sRGB colour (the alpha is ignored).
#[must_use]
pub fn luminance(color: Rgba) -> f32 {
    let linear = |channel: f32| {
        if channel <= 0.039_28 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color[0]) + 0.7152 * linear(color[1]) + 0.0722 * linear(color[2])
}

/// `top` over an opaque `under`, as the colour that reaches the screen.
#[must_use]
pub fn over(top: Rgba, under: Rgba) -> Rgba {
    let a = top[3];
    [
        top[0] * a + under[0] * (1.0 - a),
        top[1] * a + under[1] * (1.0 - a),
        top[2] * a + under[2] * (1.0 - a),
        1.0,
    ]
}

/// The WCAG contrast ratio of `front` on `back`, compositing a translucent `front` first.
#[must_use]
pub fn contrast(front: Rgba, back: Rgba) -> f32 {
    let shown = if front[3] < 1.0 { over(front, back) } else { front };
    let a = luminance(shown) + 0.05;
    let b = luminance(back) + 0.05;
    if a > b { a / b } else { b / a }
}

/// The least contrast ordinary text must have against its surface.
pub const MIN_CONTRAST: f32 = 4.5;

/// `color`, moved toward black or white until it reads against `surface` at `least` contrast; the
/// colour itself when it already does.
#[must_use]
pub fn legible_on(color: Rgba, surface: Rgba, least: f32) -> Rgba {
    if contrast(color, surface) >= least {
        return color;
    }
    let target = if luminance(surface) > 0.5 { 0.0 } else { 1.0 };
    const STEPS: u32 = 20;
    for step in 1..=STEPS {
        let t = step as f32 / STEPS as f32;
        let candidate = [
            color[0] + (target - color[0]) * t,
            color[1] + (target - color[1]) * t,
            color[2] + (target - color[2]) * t,
            color[3],
        ];
        if contrast(candidate, surface) >= least {
            return candidate;
        }
    }
    [target, target, target, color[3]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_design_unit_is_half_a_gui_pixel() {
        assert_eq!(px(264.0), 132.0);
        assert_eq!(px(space::GUTTER), 12.5);
    }

    #[test]
    fn colours_come_from_hex_and_alpha() {
        let color = rgb(0xFF8000, 0.5);
        assert_eq!(color[0], 1.0);
        assert!((color[1] - 128.0 / 255.0).abs() < 1.0e-6);
        assert_eq!(color[2], 0.0);
        assert_eq!(color[3], 0.5);
    }

    #[test]
    fn black_on_white_is_the_greatest_contrast_and_equal_colours_the_least() {
        let white = rgb(0xFFFFFF, 1.0);
        let black = rgb(0x000000, 1.0);
        assert!((contrast(black, white) - 21.0).abs() < 0.01);
        assert!((contrast(white, white) - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn ordinary_and_secondary_text_are_legible_on_every_surface_of_both_themes() {
        for palette in [Palette::dark(), Palette::light()] {
            let ground = over(palette.panel, rgb(0x000000, 1.0));
            for surface in [palette.panel, palette.surface, palette.raised, palette.field] {
                let back = over(surface, ground);
                assert!(contrast(palette.text, back) >= MIN_CONTRAST, "text on {surface:?}");
                assert!(contrast(palette.muted, back) >= MIN_CONTRAST, "muted on {surface:?}");
                assert!(contrast(palette.highlight, back) >= MIN_CONTRAST, "highlight on {surface:?}");
            }
            assert!(
                contrast(palette.on_accent, palette.accent) >= MIN_CONTRAST,
                "text on the accent"
            );
        }
    }

    #[test]
    fn a_colour_that_cannot_be_read_is_moved_until_it_can() {
        let surface = rgb(0x151C22, 1.0);
        let faint = rgb(0x1E2830, 1.0);
        assert!(contrast(faint, surface) < MIN_CONTRAST);
        let fixed = legible_on(faint, surface, MIN_CONTRAST);
        assert!(contrast(fixed, surface) >= MIN_CONTRAST);
        // A colour that already reads is returned as it is.
        assert_eq!(legible_on(rgb(0xFFFFFF, 1.0), surface, MIN_CONTRAST), rgb(0xFFFFFF, 1.0));
    }

    #[test]
    fn nested_corners_stay_concentric_and_never_go_below_the_least() {
        assert_eq!(Radii::concentric(16.0, 4.0, 0.0), 12.0);
        assert_eq!(Radii::concentric(4.0, 8.0, 2.0), 2.0);
    }
}
