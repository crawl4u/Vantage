//! OptiFine/MCPatcher custom skies and the Bedrock overworld cube map built from them.

use crate::{
    meta::ASSETS,
    texture::{Budget, crop},
};
use image::RgbaImage;

/// Bedrock cube-map face written for each 3x2 cell of an OptiFine sky, in reading order.
const CUBEMAP_FACES: [u32; 6] = [5, 4, 2, 3, 0, 1];

/// The shallowest folder named `world0` (OptiFine's overworld sky folder), ending in `/`.
pub(crate) fn sky_folder<'a>(paths: impl IntoIterator<Item = &'a str>) -> Option<String> {
    paths
        .into_iter()
        .filter_map(|path| {
            let end = path
                .find("world0/")
                .filter(|start| *start == 0 || path.as_bytes()[start - 1] == b'/')?;
            Some(&path[..end + "world0/".len()])
        })
        .min_by_key(|folder| (folder.matches('/').count(), *folder))
        .map(str::to_owned)
}

/// Resolves a sky `source` relative to the sky folder `folder` (ending in `/`), accepting
/// `./x.png`, `x.png`, `optifine/sky/world0/x.png` and namespaced `minecraft:optifine/...`.
pub(crate) fn source_path(folder: &str, source: &str) -> String {
    // `namespace:path` names `assets/<namespace>/<path>`.
    if let Some((namespace, path)) = source.split_once(':')
        && namespace != "minecraft"
        && !namespace.is_empty()
        && !namespace.contains('/')
    {
        return format!("assets/{namespace}/{path}");
    }
    let source = source.strip_prefix("minecraft:").unwrap_or(source);
    if let Some(relative) = source.strip_prefix("./") {
        return format!("{folder}{relative}");
    }
    if source.contains('/') {
        return match folder.find(ASSETS) {
            Some(start) => format!("{}{source}", &folder[..start + ASSETS.len()]),
            None => source.to_owned(),
        };
    }
    format!("{folder}{source}")
}

/// Splits a 3x2 OptiFine sky into the six Bedrock `cubemap_<n>.png` faces, indexed by `n`.
pub(crate) fn cubemap_faces(sky: &RgbaImage, budget: &Budget) -> Option<[RgbaImage; 6]> {
    let (width, height) = (sky.width() / 3, sky.height() / 2);
    if width == 0 || height == 0 {
        return None;
    }
    let mut faces: [RgbaImage; 6] = Default::default();
    for (cell, face) in CUBEMAP_FACES.into_iter().enumerate() {
        let cell = cell as u32;
        faces[face as usize] = crop(
            sky,
            (cell % 3) * width,
            (cell / 3) * height,
            width,
            height,
            budget,
        )?;
    }
    Some(faces)
}

/// One OptiFine sky layer read from a `.properties` file; times are Java clock hours.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayerSpec {
    pub(crate) source: String,
    pub(crate) blend: String,
    /// `startFadeIn`, `endFadeIn`, `startFadeOut`, `endFadeOut`; absent when none is given.
    pub(crate) fade: Option<[f32; 4]>,
    /// `speed` and `axis`; absent when `rotate=false`.
    pub(crate) rotation: Option<(f32, [f32; 3])>,
    /// Clear, rain and thunder bits; all when the layer names none.
    pub(crate) weather: u8,
    /// Inclusive day ranges, and `daysLoop`.
    pub(crate) days: Option<LayerDays>,
}

/// A layer's inclusive day ranges and the `daysLoop` period.
pub(crate) type LayerDays = (Vec<(u32, u32)>, Option<u32>);

/// Parses a Java clock time such as `19:20`, `4:20:30` or `6` into hours.
fn parse_hours(value: &str) -> Option<f32> {
    let mut parts = value.trim().split(':');
    let hours: f32 = parts.next()?.trim().parse().ok()?;
    let minutes: f32 = parts.next().map_or(Some(0.0), |part| part.trim().parse().ok())?;
    let seconds: f32 = parts.next().map_or(Some(0.0), |part| part.trim().parse().ok())?;
    let total = hours + minutes / 60.0 + seconds / 3600.0;
    total.is_finite().then_some(total)
}

/// The `key=value` pairs of a properties file, the last of a repeated key winning.
fn properties(text: &str) -> std::collections::BTreeMap<String, String> {
    text.lines()
        .map(str::trim_start)
        .filter(|line| !line.starts_with('#') && !line.starts_with('!'))
        .filter_map(|line| {
            let split = line.find(['=', ':'])?;
            Some((line[..split].trim().to_owned(), line[split + 1..].trim().to_owned()))
        })
        .collect()
}

fn parse_weather(value: &str) -> u8 {
    value
        .split(|c: char| c == ',' || c.is_whitespace())
        .fold(0, |bits, word| {
            bits | match word.trim().to_ascii_lowercase().as_str() {
                "clear" => 1,
                "rain" => 2,
                "thunder" => 4,
                _ => 0,
            }
        })
}

fn parse_days(value: &str) -> Vec<(u32, u32)> {
    value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(|word| {
            let word = word.trim();
            match word.split_once('-') {
                Some((first, last)) => Some((first.trim().parse().ok()?, last.trim().parse().ok()?)),
                None => word.parse().ok().map(|day| (day, day)),
            }
        })
        .collect()
}

fn parse_axis(value: &str) -> Option<[f32; 3]> {
    let parts: Vec<f32> = value
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let axis: [f32; 3] = parts.try_into().ok()?;
    let length = axis.iter().map(|c| c * c).sum::<f32>().sqrt();
    (length.is_finite() && length > 1e-6).then(|| axis.map(|c| c / length))
}

/// Reads an OptiFine/MCPatcher sky layer. `None` when it names no source.
pub(crate) fn parse_layer(bytes: &[u8]) -> Option<LayerSpec> {
    let text = String::from_utf8_lossy(bytes);
    let map = properties(&text);
    let source = map.get("source").filter(|value| !value.is_empty())?.clone();
    let time = |key: &str| map.get(key).and_then(|value| parse_hours(value));
    let (start_in, end_in) = (time("startFadeIn"), time("endFadeIn"));
    let (start_out, end_out) = (time("startFadeOut"), time("endFadeOut"));
    // A layer gives its fade-in or its fade-out, or both; a missing edge takes its partner.
    let fade = match (start_in.or(end_in), start_out.or(end_out)) {
        (Some(fade_in), Some(fade_out)) => Some([
            start_in.unwrap_or(fade_in),
            end_in.unwrap_or(fade_in),
            start_out.unwrap_or(fade_out),
            end_out.unwrap_or(fade_out),
        ]),
        _ => None,
    };
    let rotate = map
        .get("rotate")
        .is_none_or(|value| !value.eq_ignore_ascii_case("false"));
    let rotation = rotate.then(|| {
        let speed = map
            .get("speed")
            .and_then(|value| value.parse().ok())
            .filter(|speed: &f32| speed.is_finite())
            .unwrap_or(1.0);
        let axis = map
            .get("axis")
            .and_then(|value| parse_axis(value))
            .unwrap_or([0.0, 0.0, 1.0]);
        (speed, axis)
    });
    let weather = match map.get("weather").map(|value| parse_weather(value)) {
        Some(bits) if bits != 0 => bits,
        _ => 7,
    };
    let days = map
        .get("days")
        .map(|value| parse_days(value))
        .filter(|ranges| !ranges.is_empty())
        .map(|ranges| {
            let repeat = map
                .get("daysLoop")
                .and_then(|value| value.parse().ok())
                .filter(|repeat: &u32| *repeat > 0);
            (ranges, repeat)
        });
    Some(LayerSpec {
        source,
        blend: map.get("blend").cloned().unwrap_or_else(|| "add".into()),
        fade,
        rotation,
        weather,
        days,
    })
}

#[cfg(test)]
mod layer_tests {
    use super::*;

    #[test]
    fn a_dramatic_skys_stars_layer_reads_in_full() {
        let layer = parse_layer(
            b"source=skybox:stars.png\nblend=alpha\nstartFadeIn=19:20\nendFadeIn=19:40\n\
              startFadeOut=4:20\nendFadeOut=4:40\nrotate=true\naxis=0.0 0.0 1.0\nweather=clear\n",
        )
        .unwrap();
        assert_eq!(layer.source, "skybox:stars.png");
        assert_eq!(layer.blend, "alpha");
        let fade = layer.fade.unwrap();
        assert!((fade[0] - 19.333).abs() < 1e-2 && (fade[3] - 4.666).abs() < 1e-2);
        assert_eq!(layer.rotation, Some((1.0, [0.0, 0.0, 1.0])));
        assert_eq!(layer.weather, 1);
        assert_eq!(layer.days, None);
    }

    #[test]
    fn a_bare_source_is_an_additive_rotating_layer_shown_always() {
        let layer = parse_layer(b"source=./sky.png\n").unwrap();
        assert_eq!(layer.blend, "add");
        assert_eq!(layer.fade, None);
        assert_eq!(layer.rotation, Some((1.0, [0.0, 0.0, 1.0])));
        assert_eq!(layer.weather, 7);
    }

    #[test]
    fn rotation_speed_axis_and_the_off_switch() {
        let turning = parse_layer(b"source=a.png\nspeed=2.5\naxis=0 -2 0\n").unwrap();
        assert_eq!(turning.rotation, Some((2.5, [0.0, -1.0, 0.0])), "the axis is normalized");
        assert_eq!(parse_layer(b"source=a.png\nrotate=false\n").unwrap().rotation, None);
    }

    #[test]
    fn days_weather_and_loop() {
        let layer =
            parse_layer(b"source=a.png\ndays=0-3 7,9\ndaysLoop=10\nweather=rain thunder\n").unwrap();
        assert_eq!(layer.days, Some((vec![(0, 3), (7, 7), (9, 9)], Some(10))));
        assert_eq!(layer.weather, 6);
    }

    #[test]
    fn a_fade_with_one_edge_pair_uses_the_other_for_the_missing_edges() {
        let layer =
            parse_layer(b"source=a.png\nstartFadeIn=18:00\nendFadeOut=5:00\n").unwrap();
        assert_eq!(layer.fade, Some([18.0, 18.0, 5.0, 5.0]));
        assert_eq!(parse_layer(b"source=a.png\nstartFadeIn=18:00\n").unwrap().fade, None);
    }

    #[test]
    fn a_file_without_a_source_is_no_layer() {
        assert_eq!(parse_layer(b"blend=add\n"), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_resolve_like_optifine() {
        let folder = "assets/minecraft/optifine/sky/world0/";
        assert_eq!(
            source_path(folder, "./sky1.png"),
            format!("{folder}sky1.png")
        );
        assert_eq!(source_path(folder, "sky1.png"), format!("{folder}sky1.png"));
        assert_eq!(source_path(folder, "skybox:stars.png"), "assets/skybox/stars.png");
        assert_eq!(
            source_path(folder, "minecraft:optifine/sky/world0/a.png"),
            "assets/minecraft/optifine/sky/world0/a.png"
        );
    }

    #[test]
    fn the_shallowest_world0_folder_wins() {
        let paths = [
            "assets/minecraft/optifine/sky/world0/a.properties",
            "assets/minecraft/mcpatcher/x/sky/world0/b.properties",
            "notworld0/c.png",
        ];
        assert_eq!(
            sky_folder(paths).as_deref(),
            Some("assets/minecraft/optifine/sky/world0/")
        );
    }

    #[test]
    fn faces_follow_the_bedrock_order() {
        let sky = RgbaImage::from_fn(6, 4, |x, y| {
            image::Rgba([(x / 2 + 3 * (y / 2)) as u8, 0, 0, 255])
        });
        let faces = cubemap_faces(&sky, &crate::texture::Budget::new()).unwrap();
        for (cell, face) in CUBEMAP_FACES.into_iter().enumerate() {
            assert_eq!(faces[face as usize].get_pixel(0, 0).0[0], cell as u8);
        }
    }
}
