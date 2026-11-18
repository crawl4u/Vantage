use std::{collections::HashMap, sync::Arc};

use ui::UiVisual;
use ui_engine::{test_support::fixture_font, text_metrics::TextMetrics};

use super::font::*;
use crate::{
    images::{
        OreUiImages, OreUiSprite,
        java::key,
        java_font::{JavaFont, Sheet},
    },
    paint::{Canvas, Originals},
};

const SHEET_PAGE: u16 = 4;
const DEFINITION: &str = r#"{"providers":[
    {"type":"space","advances":{" ":4}},
    {"type":"bitmap","file":"minecraft:font/t.png","ascent":7,"chars":["ab"]}]}"#;

/// A 16 by 8 sheet of two cells: `a` is 4 texels wide and `b` is 6.
fn font() -> JavaFont {
    let mut rgba = vec![0; 16 * 8 * 4];
    for (x, y) in [(0, 2), (3, 2), (8, 2), (13, 2)] {
        rgba[(y * 16 + x) * 4 + 3] = 255;
    }
    let sheet = Sheet {
        width: 16,
        height: 8,
        rgba,
    };
    JavaFont::build(
        &[DEFINITION.as_bytes()],
        &HashMap::from([("t.png".to_owned(), sheet)]),
    )
}

fn originals(with_sheet: bool) -> Originals {
    let sprites = Arc::new(if with_sheet {
        HashMap::from([(
            key("font/t.png"),
            OreUiSprite {
                page: SHEET_PAGE,
                bounds: [0, 0, 16, 8],
            },
        )])
    } else {
        HashMap::new()
    });
    Originals {
        page: 0,
        images: OreUiImages {
            pages: Vec::new(),
            sprites: sprites.clone(),
            loading_frames: Default::default(),
            animations: Default::default(),
        },
        masks: HashMap::new(),
        sprites,
        loading_frames: Default::default(),
        animations: Default::default(),
    }
}

/// One drawn glyph: its texel rectangle, its box on screen and its colour.
type Drawn = ([u16; 4], [f32; 4], [u8; 4]);

fn paint(with_sheet: bool, draw: impl FnOnce(&mut Canvas<'_>)) -> Vec<Drawn> {
    let originals = originals(with_sheet);
    let (mut nodes, mut next, mut layouts) =
        (Vec::new(), 1, ui::TextLayoutCache::new(128, 1024 * 1024));
    let fixture = fixture_font();
    let metrics = TextMetrics::for_viewport([1280, 720], ui::DpiScale::new(1.0).unwrap(), Some(2));
    let mut canvas = Canvas::new(
        &mut nodes,
        &mut next,
        &mut layouts,
        &fixture,
        metrics,
        0,
        Some(&originals),
    );
    draw(&mut canvas);
    nodes
        .iter()
        .filter_map(|node| match node.visual() {
            UiVisual::Sprite { uv, color, .. } => {
                let b = node.bounds();
                Some((*uv, [b.min().x(), b.min().y(), b.max().x(), b.max().y()], *color))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn glyphs_advance_by_their_width_plus_one_and_sit_on_the_line() {
    let font = font();
    let drawn = paint(true, |canvas| {
        let width = draw_with(&font, canvas, "ab", [10.0, 20.0], 2.0, [255; 4], false)
            .unwrap()
            .unwrap();
        // `a` advances 5 and `b` 7, in Java pixels, drawn at two logical pixels each.
        assert_eq!(width, 24.0);
    });
    assert_eq!(
        drawn,
        [
            ([0, 0, 4, 8], [10.0, 20.0, 18.0, 36.0], [255; 4]),
            ([8, 0, 14, 8], [20.0, 20.0, 32.0, 36.0], [255; 4]),
        ]
    );
}

#[test]
fn the_shadow_is_drawn_first_one_java_pixel_down_and_right_at_a_quarter_of_the_colour() {
    let font = font();
    let drawn = paint(true, |canvas| {
        draw_with(&font, canvas, "a", [10.0, 20.0], 3.0, [255, 128, 40, 255], true).unwrap();
    });
    assert_eq!(drawn.len(), 2);
    assert_eq!(drawn[0], ([0, 0, 4, 8], [13.0, 23.0, 25.0, 47.0], [63, 32, 10, 255]));
    assert_eq!(drawn[1], ([0, 0, 4, 8], [10.0, 20.0, 22.0, 44.0], [255, 128, 40, 255]));
}

#[test]
fn a_space_only_moves_the_pen() {
    let font = font();
    let drawn = paint(true, |canvas| {
        draw_with(&font, canvas, "a b", [0.0, 0.0], 1.0, [255; 4], false).unwrap();
    });
    // `a` advances 5, the space 4, so `b` starts at 9.
    assert_eq!(drawn.len(), 2);
    assert_eq!(drawn[1].1[0], 9.0);
}

#[test]
fn text_the_font_cannot_draw_draws_nothing_so_the_caller_can_fall_back() {
    let font = font();
    let drawn = paint(true, |canvas| {
        assert_eq!(
            draw_with(&font, canvas, "aZb", [0.0, 0.0], 2.0, [255; 4], true).unwrap(),
            None
        );
        assert!(!centred_with(&font, canvas, "aZb", [0.0, 0.0, 100.0, 40.0], 2.0, [255; 4], true).unwrap());
    });
    assert!(drawn.is_empty());
    assert_eq!(width_with(&font, "aZb"), None);
}

#[test]
fn a_font_whose_sheet_is_not_installed_draws_nothing() {
    let font = font();
    let drawn = paint(false, |canvas| {
        assert_eq!(
            draw_with(&font, canvas, "ab", [0.0, 0.0], 2.0, [255; 4], true).unwrap(),
            None
        );
    });
    assert!(drawn.is_empty());
}

#[test]
fn centred_text_is_centred_across_the_button_and_on_java_s_line() {
    let font = font();
    // "ab" is 12 Java pixels wide, 24 logical at 2x; the button is 200 by 40 logical pixels.
    let drawn = paint(true, |canvas| {
        assert!(centred_with(&font, canvas, "ab", [100.0, 50.0, 300.0, 90.0], 2.0, [255; 4], false).unwrap());
    });
    // Across: (100 + 300 - 24) / 2 = 188. Down: 50 + (40 - 8 * 2) / 2 = 62.
    assert_eq!(drawn[0].1, [188.0, 62.0, 196.0, 78.0]);
}

#[test]
fn positions_snap_to_whole_logical_pixels() {
    let font = font();
    let drawn = paint(true, |canvas| {
        draw_with(&font, canvas, "a", [10.4, 20.6], 1.0, [255; 4], false).unwrap();
    });
    assert_eq!(drawn[0].1[..2], [10.0, 21.0]);
}
