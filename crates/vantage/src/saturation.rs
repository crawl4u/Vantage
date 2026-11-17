//! The Saturation overlay: Lunar's Saturation HUD (AppleSkin) drawn over the game's hunger bar.
//!
//! Three layers of ten drumstick outlines sit on the ten hunger icons: your saturation (steady),
//! the hunger the food in your hand would restore (flashing), and the saturation it would add
//! (flashing). All the arithmetic is here and plain Rust; `hud` writes the document and
//! `guest` feeds in the held item.

/// Hunger icons on the bar.
pub const SHANKS: usize = 10;
/// Saturation, potential hunger, potential saturation.
pub const LAYERS: usize = 3;
/// Distance between two icons, and an icon's size, in GUI pixels.
pub const STEP: f32 = 8.0;
pub const ICON: f32 = 9.0;
/// Flash levels, 0 to this.
pub const PULSE_MAX: u8 = 7;

/// How the game lays out the bars above the hotbar. The icons sit on the right half of the hotbar
/// in both; they differ in how high the rows are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    /// Java's: the hunger row's top edge is 39 GUI pixels above the bottom of the screen.
    #[default]
    Java,
    /// Bedrock's hotbar: the rows sit higher than Java's, 41 GUI pixels above the bottom of the
    /// screen (measured in game: 38 put the overlay 3 pixels too low).
    Bedrock,
}

impl Layout {
    /// The choices in the menu, in the order of `from_index`.
    pub const NAMES: [&'static str; 2] = ["Java", "Bedrock"];

    pub fn from_index(index: usize) -> Self {
        if index == 1 { Self::Bedrock } else { Self::Java }
    }

    /// How far the right-most icon's left edge is right of the screen's middle.
    pub fn first_x(self) -> f32 {
        82.0
    }

    /// How far the icons' top edge is above the bottom of the screen.
    pub fn top_from_bottom(self) -> f32 {
        match self {
            Self::Java => 39.0,
            Self::Bedrock => 41.0,
        }
    }
}

/// The hunger icon's pixels, `#` where the icon has one (the meat top left, the bone bottom
/// right): the shape of the game's own icon, measured from it at its size.
pub const SPRITE: [&str; 9] = [
    "..##.....",
    ".####....",
    "######...",
    "#######..",
    ".######..",
    "..######.",
    "...######",
    "......###",
    "......##.",
];

/// The pixels of the sprite's edge: those with a neighbour (left, right, above or below) that is
/// outside the icon. The overlay colours only these, so it follows the icon's outline like
/// AppleSkin's golden border instead of covering the icon.
pub fn outline() -> [[bool; 9]; 9] {
    let inside = |x: i32, y: i32| (0..9).contains(&x) && (0..9).contains(&y) && SPRITE[y as usize].as_bytes()[x as usize] == b'#';
    let mut out = [[false; 9]; 9];
    for y in 0..9i32 {
        for x in 0..9i32 {
            out[y as usize][x as usize] = inside(x, y) && ![(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|(dx, dy)| inside(x + dx, y + dy));
        }
    }
    out
}

/// The outline as the fewest rectangles that cover exactly its pixels (x, y, width, height):
/// every row's runs, with a run that repeats the one above it grown downwards instead.
pub fn rects() -> Vec<[f32; 4]> {
    let edge = outline();
    let mut out: Vec<[f32; 4]> = Vec::new();
    // The runs open in the row above: (start, end) and the index of their rectangle.
    let mut open: Vec<(usize, usize, usize)> = Vec::new();
    for (y, row) in edge.iter().enumerate() {
        let mut runs = Vec::new();
        let mut x = 0;
        while x < row.len() {
            if row[x] {
                let start = x;
                while x < row.len() && row[x] {
                    x += 1;
                }
                runs.push((start, x));
            } else {
                x += 1;
            }
        }
        let mut next = Vec::new();
        for (start, end) in runs {
            if let Some(&(_, _, at)) = open.iter().find(|(s, e, _)| *s == start && *e == end) {
                out[at][3] += 1.0;
                next.push((start, end, at));
            } else {
                out.push([start as f32, y as f32, (end - start) as f32, 1.0]);
                next.push((start, end, out.len() - 1));
            }
        }
        open = next;
    }
    out
}

/// What eating `identifier` gives: hunger points and saturation in tenths. Items the game does
/// not let you eat are none.
pub fn food_value(identifier: &str) -> Option<(u32, u32)> {
    let name = identifier.strip_prefix("minecraft:").unwrap_or(identifier);
    Some(match name {
        "apple" => (4, 24),
        "baked_potato" => (5, 60),
        "beef" => (3, 18),
        "beetroot" => (1, 12),
        "beetroot_soup" => (6, 72),
        "bread" => (5, 60),
        "carrot" => (3, 36),
        "chicken" => (2, 12),
        "chorus_fruit" => (4, 24),
        "cod" => (2, 4),
        "cooked_beef" => (8, 128),
        "cooked_chicken" => (6, 72),
        "cooked_cod" => (5, 60),
        "cooked_mutton" => (6, 96),
        "cooked_porkchop" => (8, 128),
        "cooked_rabbit" => (5, 60),
        "cooked_salmon" => (6, 96),
        "cookie" => (2, 4),
        "dried_kelp" => (1, 6),
        "enchanted_golden_apple" | "golden_apple" => (4, 96),
        "glow_berries" | "sweet_berries" => (2, 4),
        "golden_carrot" => (6, 144),
        "honey_bottle" => (6, 12),
        "melon_slice" => (2, 12),
        "mushroom_stew" | "suspicious_stew" => (6, 72),
        "mutton" => (2, 12),
        "poisonous_potato" => (2, 12),
        "porkchop" => (3, 18),
        "potato" => (1, 6),
        "pufferfish" | "tropical_fish" => (1, 2),
        "pumpkin_pie" => (8, 48),
        "rabbit" => (3, 18),
        "rabbit_stew" => (10, 120),
        "rotten_flesh" => (4, 8),
        "salmon" => (2, 4),
        "spider_eye" => (2, 32),
        _ => return None,
    })
}

/// Which parts of the overlay are wanted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Parts {
    pub saturation: bool,
    pub hunger: bool,
    pub potential: bool,
}

impl Parts {
    /// The layers that need a document node: saturation, potential hunger, potential saturation.
    pub fn layers(self) -> [bool; LAYERS] {
        [self.saturation, self.hunger, self.potential]
    }

    pub fn any(self) -> bool {
        self.saturation || self.hunger || self.potential
    }

    /// Whether the held food is looked at (it is the only thing that flashes).
    pub fn eating(self) -> bool {
        self.hunger || self.potential
    }
}

/// Where the right-most icon's top-left corner is, for a screen of `viewport` GUI pixels.
pub fn origin(layout: Layout, viewport: [f32; 2]) -> [f32; 2] {
    [viewport[0] / 2.0 + layout.first_x(), viewport[1] - layout.top_from_bottom()]
}

/// Where the right-most icon's top-left corner is, read from the rectangle the host reports for the
/// game's own row of hunger shanks (`x`, `y`, width, height in GUI pixels). None when the rectangle
/// is not the size of ten icons, so a host that reports something else falls back to the layout.
pub fn bar_origin(bounds: [f32; 4]) -> Option<[f32; 2]> {
    let [x, y, width, height] = bounds;
    let row = STEP * (SHANKS as f32 - 1.0) + ICON;
    if !(width - row).abs().le(&3.0) || !(ICON - 1.0..=ICON + 2.0).contains(&height) {
        return None;
    }
    Some([x + width - ICON, y + (height - ICON) / 2.0])
}

/// How much of icon `k` (0 is the right-most) the points from `from` up to `to` cover, rounded up
/// to a half icon: 0, 0.5 or 1.
fn cover(k: usize, from: f32, to: f32) -> f32 {
    let low = from.max(2.0 * k as f32);
    let high = to.min(2.0 * k as f32 + 2.0);
    let points = (high - low).max(0.0);
    (points.ceil() / 2.0).min(1.0)
}

/// The flash level (0..=PULSE_MAX) at `seconds`: a triangle wave, about a flash every 0.7 s.
pub fn pulse(seconds: f64) -> u8 {
    let t = (seconds * 1.4).fract();
    let tri = 1.0 - (2.0 * t - 1.0).abs();
    (tri * f64::from(PULSE_MAX)).round() as u8
}

/// The opacity of every outline: `[layer][icon]`. `food` is the hunger points and the saturation
/// in tenths, `eating` what the food in your hand would give, `flash` the pulse level.
pub fn alphas(food: Option<[u32; 2]>, eating: Option<(u32, u32)>, flash: u8, parts: Parts) -> [[f32; SHANKS]; LAYERS] {
    let mut out = [[0.0; SHANKS]; LAYERS];
    let Some([hunger, tenths]) = food else { return out };
    let hunger = hunger.min(20) as f32;
    let saturation = (tenths as f32 / 10.0).min(20.0);
    let level = 0.2 + 0.6 * f32::from(flash.min(PULSE_MAX)) / f32::from(PULSE_MAX);
    // What the food in your hand would do. Hunger never goes past 20, saturation never past hunger.
    let after = eating.map(|(points, gain)| {
        let new_hunger = (hunger + points as f32).min(20.0);
        let new_saturation = (saturation + gain as f32 / 10.0).min(new_hunger);
        (new_hunger, new_saturation)
    });
    for k in 0..SHANKS {
        if parts.saturation && saturation > 0.0 {
            out[0][k] = cover(k, 0.0, saturation);
        }
        if let (true, Some((new_hunger, _))) = (parts.hunger, after) {
            out[1][k] = cover(k, hunger, new_hunger) * level;
        }
        if let (true, Some((_, new_saturation))) = (parts.potential, after) {
            out[2][k] = cover(k, saturation, new_saturation) * level;
        }
    }
    out
}

/// The colour of an outline layer: the saturation colour (steady), white for the hunger preview,
/// and the saturation colour (or its opposite with `inverse`) for the saturation preview.
pub fn layer_rgb(layer: usize, saturation_colour: [f32; 3], inverse: bool) -> [f32; 3] {
    match layer {
        1 => [1.0, 1.0, 1.0],
        2 if inverse => saturation_colour.map(|c| 1.0 - c),
        _ => saturation_colour,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: Parts = Parts { saturation: true, hunger: true, potential: true };

    #[test]
    fn food_values_match_the_game() {
        assert_eq!(food_value("minecraft:cooked_beef"), Some((8, 128)));
        assert_eq!(food_value("golden_apple"), Some((4, 96)));
        assert_eq!(food_value("minecraft:diamond"), None);
    }

    #[test]
    fn saturation_fills_icons_from_the_right() {
        // 18 hunger, saturation 4.5: two whole icons and a half-empty third (rounded up to a half).
        let a = alphas(Some([18, 45]), None, 0, ALL);
        assert_eq!(&a[0][..4], &[1.0, 1.0, 0.5, 0.0]);
        // Nothing held: no previews.
        assert!(a[1].iter().chain(a[2].iter()).all(|v| *v == 0.0));
    }

    #[test]
    fn no_saturation_draws_nothing() {
        let a = alphas(Some([20, 0]), None, 0, ALL);
        assert!(a[0].iter().all(|v| *v == 0.0));
        assert!(alphas(None, Some((8, 128)), 7, ALL).iter().flatten().all(|v| *v == 0.0));
    }

    #[test]
    fn the_hunger_preview_covers_the_points_the_food_would_restore() {
        // 14 hunger and a steak (8): hunger rises to 20, icons 7..9 (points 14..20) light up.
        let a = alphas(Some([14, 0]), Some((8, 128)), PULSE_MAX, ALL);
        assert_eq!(a[1][6], 0.0);
        assert!(a[1][7] > 0.0 && a[1][8] > 0.0 && a[1][9] > 0.0);
        assert!((a[1][9] - 0.8).abs() < 1e-5, "full flash is 0.8: {}", a[1][9]);
        // At 20 there is no hunger to restore, but the saturation still goes up.
        let full = alphas(Some([20, 0]), Some((8, 128)), 3, ALL);
        assert!(full[1].iter().all(|v| *v == 0.0));
        assert!(full[2][0] > 0.0);
    }

    #[test]
    fn saturation_never_goes_past_hunger() {
        // 10 hunger, 9 saturation, a steak: new hunger 18, new saturation min(21.8, 18) = 18.
        let a = alphas(Some([10, 90]), Some((8, 128)), PULSE_MAX, ALL);
        assert!(a[2][8] > 0.0 && a[2][9] == 0.0, "{:?}", a[2]);
    }

    #[test]
    fn options_turn_layers_off() {
        let none = Parts::default();
        assert!(alphas(Some([10, 40]), Some((8, 128)), 7, none).iter().flatten().all(|v| *v == 0.0));
        let only_hunger = Parts { hunger: true, ..Parts::default() };
        let a = alphas(Some([10, 40]), Some((8, 128)), 7, only_hunger);
        assert!(a[0].iter().chain(a[2].iter()).all(|v| *v == 0.0) && a[1].iter().any(|v| *v > 0.0));
    }

    #[test]
    fn the_pulse_runs_up_and_down() {
        let levels: Vec<u8> = (0..20).map(|i| pulse(f64::from(i) * 0.05)).collect();
        assert!(levels.iter().all(|l| *l <= PULSE_MAX));
        assert_eq!((*levels.iter().min().unwrap(), *levels.iter().max().unwrap()), (0, PULSE_MAX));
    }

    #[test]
    fn inverse_flips_the_saturation_preview_only() {
        let gold = [1.0, 0.8, 0.0];
        assert_eq!(layer_rgb(0, gold, true), gold);
        assert_eq!(layer_rgb(2, gold, false), gold);
        let flipped = layer_rgb(2, gold, true);
        assert!(flipped.iter().zip([0.0, 0.2, 1.0]).all(|(a, b)| (a - b).abs() < 1e-5), "{flipped:?}");
    }

    #[test]
    fn the_bar_sits_over_the_right_half_of_the_hotbar() {
        // 480 by 270 GUI pixels (a 1920 by 1080 screen at scale 4): icon 0 starts at x 322, y 231.
        assert_eq!(origin(Layout::Java, [480.0, 270.0]), [322.0, 231.0]);
        // The layouts agree across the screen and differ in height (Bedrock's rows are 2 pixels higher).
        let bedrock = origin(Layout::Bedrock, [480.0, 270.0]);
        assert_eq!((bedrock[0], bedrock[1]), (322.0, 229.0));
        // A 2560 pixel wide screen at scale 3 is 853.33 GUI pixels: the centre is not rounded down.
        let odd = origin(Layout::Java, [2560.0 / 3.0, 480.0]);
        assert!((odd[0] - (2560.0 / 6.0 + 82.0)).abs() < 1e-3);
        assert_eq!((Layout::from_index(0), Layout::from_index(1), Layout::from_index(9)), (Layout::Java, Layout::Bedrock, Layout::Java));
    }

    #[test]
    fn the_hosts_bar_rectangle_places_the_overlay() {
        // Ten shanks span 81 GUI pixels: the right-most icon starts 9 before the row's end.
        assert_eq!(bar_origin([100.0, 50.0, 81.0, 9.0]), Some([172.0, 50.0]));
        // A rectangle with a pixel of padding all round still lands the icon on the same spot.
        assert_eq!(bar_origin([99.0, 49.0, 83.0, 11.0]), Some([173.0, 50.0]));
        // Anything not the size of the row is ignored.
        assert_eq!(bar_origin([0.0, 0.0, 40.0, 9.0]), None);
        assert_eq!(bar_origin([0.0, 0.0, 81.0, 30.0]), None);
    }

    #[test]
    fn the_rectangles_cover_exactly_the_outline_and_none_of_the_inside() {
        let edge = outline();
        let mut painted = [[false; 9]; 9];
        for [x, y, w, h] in rects() {
            for j in 0..h as usize {
                for i in 0..w as usize {
                    let (px, py) = (x as usize + i, y as usize + j);
                    assert!(!painted[py][px], "pixel {px},{py} is covered twice");
                    painted[py][px] = true;
                }
            }
        }
        assert_eq!(painted, edge);
        // Inside the meat (4, 3) and the icon's edge (0, 2) differ; nothing is drawn outside the icon.
        assert!(!painted[3][4] && painted[2][0]);
        for (y, row) in SPRITE.iter().enumerate() {
            for (x, c) in row.bytes().enumerate() {
                assert!(c == b'#' || !painted[y][x], "pixel {x},{y} is outside the icon");
            }
        }
        assert!(rects().len() <= 13, "{} rectangles", rects().len());
    }
}
