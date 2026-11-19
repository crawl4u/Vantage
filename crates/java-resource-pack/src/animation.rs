//! Java texture animations (`.png.mcmeta`) as Bedrock flipbook patches. Java stores an animated
//! texture as a vertical strip of square frames with a `.mcmeta` giving how long each shows;
//! Bedrock animates a block texture from `textures/flipbook_textures.json`, which names the
//! terrain atlas tile the texture fills. A translated pack carries the patch so the strip plays at
//! the pack's own speed and order instead of Bedrock's.

use serde_json::{Value, json};
use std::{collections::HashMap, sync::LazyLock};

const TILES_TEXT: &str = include_str!("../data/flipbook-tiles.tsv");

/// The most frames a patch lists once frames with their own times are repeated.
const MAX_FRAMES: usize = 1024;

/// The terrain atlas tile a Bedrock block texture fills.
#[derive(Clone, Copy)]
struct Tile {
    name: &'static str,
    index: u32,
    variant: Option<u32>,
}

/// Bedrock block texture path (relative to `textures/`, no extension) to its tile.
static TILES: LazyLock<HashMap<&'static str, Tile>> = LazyLock::new(|| {
    TILES_TEXT
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut parts = line.split('\t');
            let path = parts.next().unwrap();
            let name = parts.next().unwrap();
            let index = parts.next().unwrap().parse().unwrap();
            let variant = parts.next().unwrap().parse().ok();
            (
                path,
                Tile {
                    name,
                    index,
                    variant,
                },
            )
        })
        .collect()
});

/// The terrain atlas tile name of the block texture at `path` (relative to `textures/`, no
/// extension) when it is the only texture of its tile, which is when a weighted list of textures
/// can stand in for it.
pub(crate) fn solo_tile(path: &str) -> Option<&'static str> {
    TILES
        .get(path)
        .filter(|tile| tile.index == 0 && tile.variant.is_none())
        .map(|tile| tile.name)
}

/// A Java animation: how long each frame shows by default, whether frames blend, and the frames
/// to show in order, each with its own time when it has one.
#[derive(Debug, PartialEq)]
pub(crate) struct Animation {
    frametime: u32,
    interpolate: bool,
    frames: Option<Vec<(u32, Option<u32>)>>,
}

/// The animation a `.mcmeta` declares; `None` when it declares none or is not JSON.
pub(crate) fn parse(bytes: &[u8]) -> Option<Animation> {
    let meta: Value = serde_json::from_slice(bytes).ok()?;
    let animation = meta.get("animation")?.as_object()?;
    let whole = |value: &Value| value.as_u64().and_then(|n| u32::try_from(n).ok());
    let frametime = animation
        .get("frametime")
        .and_then(whole)
        .filter(|time| *time > 0)
        .unwrap_or(1);
    let frames = animation
        .get("frames")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|frame| match frame {
                    Value::Object(frame) => Some((
                        whole(frame.get("index")?)?,
                        frame.get("time").and_then(whole).filter(|time| *time > 0),
                    )),
                    other => Some((whole(other)?, None)),
                })
                .collect()
        });
    Some(Animation {
        frametime,
        interpolate: animation
            .get("interpolate")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        frames,
    })
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// The flipbook patch for the block texture at `path` (relative to `textures/`, no extension)
/// whose strip holds `strip_frames` frames. `None` when Bedrock's atlas has no tile for it, the
/// strip is not an animation, or the animation lists no usable frame.
pub(crate) fn patch(path: &str, animation: &Animation, strip_frames: u32) -> Option<Value> {
    let tile = TILES.get(path)?;
    if strip_frames < 2 {
        return None;
    }
    let sequence: Vec<(u32, u32)> = match &animation.frames {
        Some(frames) => frames
            .iter()
            .filter(|(index, _)| *index < strip_frames)
            .map(|(index, time)| (*index, time.unwrap_or(animation.frametime)))
            .collect(),
        None => (0..strip_frames)
            .map(|index| (index, animation.frametime))
            .collect(),
    };
    if sequence.is_empty() {
        return None;
    }
    let ticks = sequence
        .iter()
        .fold(0, |acc, (_, time)| gcd(acc, *time))
        .max(1);
    let repeated: usize = sequence
        .iter()
        .map(|(_, time)| (time / ticks) as usize)
        .sum();
    let (ticks, frames): (u32, Vec<u32>) = if repeated <= MAX_FRAMES {
        (
            ticks,
            sequence
                .iter()
                .flat_map(|(index, time)| std::iter::repeat_n(*index, (time / ticks) as usize))
                .collect(),
        )
    } else {
        (
            animation.frametime,
            sequence.iter().map(|(index, _)| *index).collect(),
        )
    };
    let mut entry = json!({
        "flipbook_texture": format!("textures/{path}"),
        "atlas_tile": tile.name,
        "atlas_index": tile.index,
        "ticks_per_frame": ticks,
        "frames": frames,
        "blend_frames": animation.interpolate,
    });
    if let Some(variant) = tile.variant {
        entry["atlas_tile_variant"] = json!(variant);
    }
    Some(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn animation(text: &str) -> Animation {
        parse(text.as_bytes()).expect("an animation")
    }

    #[test]
    fn a_mcmeta_without_an_animation_declares_none() {
        assert!(parse(br#"{"gui":{"scaling":{"type":"stretch"}}}"#).is_none());
        assert!(parse(b"not json").is_none());
    }

    #[test]
    fn defaults_play_every_frame_in_order_one_tick_each() {
        let patch = patch("blocks/lava_still", &animation(r#"{"animation":{}}"#), 4).unwrap();
        assert_eq!(patch["atlas_tile"], "still_lava");
        assert_eq!(patch["ticks_per_frame"], 1);
        assert_eq!(patch["frames"], json!([0, 1, 2, 3]));
        assert_eq!(patch["blend_frames"], false);
    }

    #[test]
    fn frametime_interpolation_and_custom_order_carry_over() {
        let text = r#"{"animation":{"frametime":3,"interpolate":true,"frames":[2,0,1]}}"#;
        let patch = patch("blocks/lava_still", &animation(text), 3).unwrap();
        assert_eq!(patch["ticks_per_frame"], 3);
        assert_eq!(patch["frames"], json!([2, 0, 1]));
        assert_eq!(patch["blend_frames"], true);
    }

    #[test]
    fn frames_with_their_own_times_repeat_on_the_common_tick() {
        let text = r#"{"animation":{"frametime":4,"frames":[{"index":0,"time":6},{"index":1}]}}"#;
        let patch = patch("blocks/lava_still", &animation(text), 2).unwrap();
        // 6 and 4 ticks share a 2-tick step: frame 0 shows three steps, frame 1 two.
        assert_eq!(patch["ticks_per_frame"], 2);
        assert_eq!(patch["frames"], json!([0, 0, 0, 1, 1]));
    }

    #[test]
    fn frames_past_the_strip_are_ignored_and_tile_variants_are_kept() {
        let text = r#"{"animation":{"frames":[0,9,1]}}"#;
        let patch = patch("blocks/bubble_column_outer_a", &animation(text), 2).unwrap();
        assert_eq!(patch["frames"], json!([0, 1]));
        assert_eq!(patch["atlas_tile_variant"], 0);
    }

    #[test]
    fn a_texture_without_a_tile_or_a_strip_gets_no_patch() {
        let plain = animation(r#"{"animation":{}}"#);
        assert!(patch("blocks/not_a_texture", &plain, 4).is_none());
        assert!(patch("blocks/lava_still", &plain, 1).is_none());
    }

    #[test]
    fn the_tile_list_matches_the_pinned_sample_pack() {
        let Some(pack) = crate::test_support::sample_pack() else {
            return;
        };
        for path in TILES.keys() {
            assert!(
                pack.join("textures").join(format!("{path}.png")).is_file()
                    || pack.join("textures").join(format!("{path}.tga")).is_file(),
                "{path} is not a sample-pack texture"
            );
        }
    }
}
