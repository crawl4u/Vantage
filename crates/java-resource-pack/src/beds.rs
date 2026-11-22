//! Java Edition 1.21.5+ draws a bed from block models whose faces are 16 by 16 textures
//! (`<colour>_bed_head_up`, `bed_down` and the rest). Bedrock draws it from one 64 by 64 entity
//! texture, so each Bedrock face is composed from the Java face that shows the same surface,
//! turned to Bedrock's orientation. The mapping below was derived by matching vanilla Java 26.3's
//! faces against Bedrock's `textures/entity/bed/red.png`; every listed face matches exactly.

use crate::texture::Budget;
use image::{Rgba, RgbaImage};

/// Java dye stems in Bedrock's dye order, with the Bedrock texture stem of each.
pub(crate) const COLORS: [(&str, &str); 16] = [
    ("white", "white"),
    ("orange", "orange"),
    ("magenta", "magenta"),
    ("light_blue", "light_blue"),
    ("yellow", "yellow"),
    ("lime", "lime"),
    ("pink", "pink"),
    ("gray", "gray"),
    ("light_gray", "silver"),
    ("cyan", "cyan"),
    ("purple", "purple"),
    ("blue", "blue"),
    ("brown", "brown"),
    ("green", "green"),
    ("red", "red"),
    ("black", "black"),
];

/// How a source rectangle is turned to land on its destination.
#[derive(Clone, Copy)]
enum Turn {
    Same,
    FlipX,
    FlipY,
    Half,
    /// A quarter revolution counter-clockwise.
    Left,
    /// A quarter revolution clockwise.
    Right,
    Transpose,
    Transverse,
}

impl Turn {
    /// Where output texel `(x, y)` comes from in a `width` by `height` source rectangle.
    const fn source(self, x: u32, y: u32, width: u32, height: u32) -> (u32, u32) {
        match self {
            Self::Same => (x, y),
            Self::FlipX => (width - 1 - x, y),
            Self::FlipY => (x, height - 1 - y),
            Self::Half => (width - 1 - x, height - 1 - y),
            Self::Left => (width - 1 - y, x),
            Self::Right => (y, height - 1 - x),
            Self::Transpose => (y, x),
            Self::Transverse => (width - 1 - y, height - 1 - x),
        }
    }
}

/// The Java texture a face reads: `<colour>_bed_head_up`, and so on, or a shared one.
#[derive(Clone, Copy)]
enum Face {
    HeadUp,
    FootUp,
    HeadEast,
    HeadWest,
    FootEast,
    FootSouth,
    FootWest,
    /// `bed_head_north`, shared by every colour.
    HeadNorth,
    /// `bed_down`, shared by every colour.
    Down,
}

/// One Bedrock rectangle `[x, y, width, height]` on the 64 by 64 grid, and the Java rectangle
/// `[x, y, width, height]` of its source on the 16 by 16 grid, before any turn.
type Piece = ([u32; 4], Face, [u32; 4], Turn);

const FULL: [u32; 4] = [0, 0, 16, 16];
/// The six rows of a side that show the mattress.
const SIDE: [u32; 4] = [0, 7, 16, 6];

/// The mattress surfaces: top, underside, both sides and both ends.
const BODY: [Piece; 10] = [
    ([6, 6, 16, 16], Face::HeadUp, FULL, Turn::Same),
    ([6, 22, 16, 16], Face::FootUp, FULL, Turn::Same),
    ([28, 6, 16, 16], Face::Down, FULL, Turn::Half),
    ([28, 22, 16, 16], Face::Down, FULL, Turn::Half),
    ([0, 6, 6, 16], Face::HeadWest, SIDE, Turn::Right),
    ([0, 22, 6, 16], Face::FootWest, SIDE, Turn::Right),
    ([22, 6, 6, 16], Face::HeadEast, SIDE, Turn::Left),
    ([22, 22, 6, 16], Face::FootEast, SIDE, Turn::Left),
    ([6, 0, 16, 6], Face::HeadNorth, SIDE, Turn::Half),
    ([22, 0, 16, 6], Face::FootSouth, SIDE, Turn::FlipY),
];

/// A leg face: its 3 by 3 Bedrock rectangle, and where its texels sit in the Java row of legs.
const fn leg(x: u32, y: u32, face: Face, ox: u32, turn: Turn) -> Piece {
    ([x, y, 3, 3], face, [ox, 13, 3, 3], turn)
}

/// The four legs' faces, each 3 by 3 texels from the legs' strip of the Java foot textures.
const LEGS: [Piece; 24] = [
    leg(12, 41, Face::FootEast, 3, Turn::Right),
    leg(15, 41, Face::FootEast, 6, Turn::FlipX),
    leg(18, 41, Face::FootSouth, 13, Turn::Left),
    leg(21, 41, Face::FootEast, 6, Turn::FlipX),
    leg(15, 38, Face::FootEast, 0, Turn::FlipX),
    leg(18, 38, Face::FootEast, 3, Turn::Half),
    leg(0, 41, Face::FootSouth, 13, Turn::Left),
    leg(3, 41, Face::FootEast, 6, Turn::FlipX),
    leg(6, 41, Face::FootEast, 3, Turn::Transpose),
    leg(9, 41, Face::FootEast, 6, Turn::FlipX),
    leg(3, 38, Face::FootEast, 0, Turn::FlipY),
    leg(6, 38, Face::FootEast, 3, Turn::FlipY),
    leg(12, 47, Face::FootEast, 3, Turn::Transverse),
    leg(15, 47, Face::FootEast, 6, Turn::FlipX),
    leg(18, 47, Face::FootSouth, 13, Turn::Right),
    leg(21, 47, Face::FootEast, 6, Turn::FlipX),
    leg(15, 44, Face::FootEast, 3, Turn::Half),
    leg(18, 44, Face::FootEast, 0, Turn::FlipX),
    leg(0, 47, Face::FootEast, 0, Turn::Left),
    leg(3, 47, Face::FootEast, 6, Turn::FlipX),
    leg(6, 47, Face::FootEast, 3, Turn::Left),
    leg(9, 47, Face::FootEast, 6, Turn::FlipX),
    leg(3, 44, Face::FootEast, 3, Turn::FlipY),
    leg(6, 44, Face::FootSouth, 13, Turn::FlipX),
];

// Bedrock's bed also has frame rails between the legs, which Java's block models do not. Their
// regions of the texture stay transparent, and the alpha-tested bed pass leaves them undrawn.

/// The texture names a bed colour reads, without folders or extension, in [`Face`] order.
fn name(color: &str, face: Face) -> String {
    match face {
        Face::HeadUp => format!("{color}_bed_head_up"),
        Face::FootUp => format!("{color}_bed_foot_up"),
        Face::HeadEast => format!("{color}_bed_head_east"),
        Face::HeadWest => format!("{color}_bed_head_west"),
        Face::FootEast => format!("{color}_bed_foot_east"),
        Face::FootSouth => format!("{color}_bed_foot_south"),
        Face::FootWest => format!("{color}_bed_foot_west"),
        Face::HeadNorth => "bed_head_north".to_owned(),
        Face::Down => "bed_down".to_owned(),
    }
}

/// Every Java texture a bed of `color` is composed from.
pub(crate) fn sources(color: &str) -> Vec<String> {
    let mut names: Vec<String> = BODY
        .iter()
        .chain(&LEGS)
        .map(|&(_, face, ..)| name(color, face))
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Copies `piece`'s Java rectangle, turned, onto the Bedrock grid scaled by `scale`.
fn paint(out: &mut RgbaImage, piece: &Piece, source: &RgbaImage, scale: u32) {
    let &([dx, dy, dw, dh], _, [sx, sy, sw, sh], turn) = piece;
    let (sw, sh) = (sw * scale, sh * scale);
    for y in 0..dh * scale {
        for x in 0..dw * scale {
            let (u, v) = turn.source(x, y, sw, sh);
            let (u, v) = (sx * scale + u, sy * scale + v);
            if u < source.width() && v < source.height() {
                out.put_pixel(dx * scale + x, dy * scale + y, *source.get_pixel(u, v));
            }
        }
    }
}

/// Composes Bedrock's `textures/entity/bed/<colour>` from the Java textures `read` returns by
/// name (as in [`sources`]). `None` when one is missing, or the faces are not all the same
/// square size, a multiple of 16.
pub(crate) fn compose(
    color: &str,
    budget: &Budget,
    mut read: impl FnMut(&str) -> Option<RgbaImage>,
) -> Option<RgbaImage> {
    let mut images = std::collections::HashMap::new();
    for name in sources(color) {
        let image = read(&name)?;
        images.insert(name, image);
    }
    let side = images.values().next()?.width();
    if side == 0
        || side % 16 != 0
        || images
            .values()
            .any(|image| image.width() != side || image.height() != side)
    {
        return None;
    }
    let scale = side / 16;
    if !budget.reserve(64 * scale, 64 * scale) {
        return None;
    }
    let mut out = RgbaImage::from_pixel(64 * scale, 64 * scale, Rgba([0; 4]));
    for piece in BODY.iter().chain(&LEGS) {
        paint(&mut out, piece, &images[&name(color, piece.1)], scale);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A texture whose every texel names its own position, so a copy shows where it came from.
    fn marked(seed: u8, side: u32) -> RgbaImage {
        RgbaImage::from_fn(side, side, |x, y| Rgba([seed, x as u8, y as u8, 255]))
    }

    fn compose_marked(side: u32) -> RgbaImage {
        let budget = Budget::new();
        let mut seeds = std::collections::HashMap::new();
        compose("red", &budget, |name| {
            let next = seeds.len() as u8 + 1;
            let seed = *seeds.entry(name.to_owned()).or_insert(next);
            Some(marked(seed, side))
        })
        .expect("all faces present")
    }

    #[test]
    fn the_head_and_foot_tops_stack_into_the_mattress_surface() {
        let out = compose_marked(16);
        assert_eq!((out.width(), out.height()), (64, 64));
        let top = out.get_pixel(6, 6).0;
        let foot = out.get_pixel(6, 22).0;
        assert_eq!([top[1], top[2]], [0, 0], "the head's top starts at its own corner");
        assert_eq!([foot[1], foot[2]], [0, 0]);
        assert_ne!(top[0], foot[0], "each half reads its own texture");
        assert_eq!(out.get_pixel(21, 37).0[0], foot[0]);
    }

    #[test]
    fn a_side_is_its_mattress_rows_turned_along_the_bed() {
        let out = compose_marked(16);
        // The right side's top-left texel is the Java west side's bottom-left mattress texel.
        let texel = out.get_pixel(0, 6).0;
        assert_eq!([texel[1], texel[2]], [0, 12]);
        // Moving down the Bedrock side runs along the Java side's width.
        assert_eq!(out.get_pixel(0, 7).0[1], 1);
    }

    #[test]
    fn faces_scale_with_the_pack_resolution() {
        let out = compose_marked(32);
        assert_eq!((out.width(), out.height()), (128, 128));
        assert_eq!(out.get_pixel(12, 12).0[1..3], [0, 0]);
        assert_eq!(out.get_pixel(12 + 31, 12 + 31).0[1..3], [31, 31]);
    }

    #[test]
    fn every_bed_texture_texel_is_covered_where_the_model_draws() {
        let out = compose_marked(16);
        for [x, y, w, h] in BODY
            .iter()
            .chain(&LEGS)
            .map(|piece| piece.0)
        {
            for py in y..y + h {
                for px in x..x + w {
                    assert_eq!(out.get_pixel(px, py).0[3], 255, "({px}, {py})");
                }
            }
        }
    }

    #[test]
    fn the_frame_rails_java_lacks_stay_transparent() {
        let out = compose_marked(16);
        for (x, y) in [(40, 3), (40, 40), (55, 20), (47, 20)] {
            assert_eq!(out.get_pixel(x, y).0[3], 0, "({x}, {y})");
        }
    }

    #[test]
    fn missing_or_mismatched_faces_compose_nothing() {
        let budget = Budget::new();
        assert!(compose("red", &budget, |_| None).is_none());
        let mut count = 0;
        assert!(
            compose("red", &budget, |_| {
                count += 1;
                Some(marked(1, if count == 1 { 16 } else { 32 }))
            })
            .is_none()
        );
        assert!(compose("red", &budget, |_| Some(marked(1, 20))).is_none());
    }
}
