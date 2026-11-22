//! Bounded numbered sky textures from the admitted resource stack.

use assets::{
    MAX_SKY_LAYERS, PackSkybox, SkyBlend, SkyDays, SkyFade, SkyLayer, SkyRotation, SkySchedule,
    SkyWeather, SkyboxDimension, SkyboxFace,
};
use image::ImageFormat;
use resource_pack::LayeredPackView;

const IMAGE_EXTENSIONS: [(&str, Option<ImageFormat>); 6] = [
    ("tga", Some(ImageFormat::Tga)),
    ("png", Some(ImageFormat::Png)),
    ("jpg", Some(ImageFormat::Jpeg)),
    ("jpeg", Some(ImageFormat::Jpeg)),
    ("hdr", None),
    ("ktx", None),
];

/// Resolves each face independently so upper packs can replace individual images.
pub fn prepare_pack_skyboxes(view: &LayeredPackView) -> [PackSkybox; SkyboxDimension::ALL.len()] {
    SkyboxDimension::ALL.map(|dimension| {
        let mut skybox = PackSkybox {
            faces: std::array::from_fn(|face| {
                let path = format!("{}{face}", dimension.texture_prefix());
                let texture = decode_face(view, &path)?;
                Some(SkyboxFace {
                    width: texture.width,
                    height: texture.height,
                    rgba8: texture.rgba8,
                })
            }),
            ..PackSkybox::default()
        };
        if dimension == SkyboxDimension::Overworld {
            prepare_overworld_layers(view, &mut skybox);
        }
        skybox
    })
}

/// Where the translator puts an overworld pack's Java sky layers, beside the numbered faces.
const OVERWORLD_LAYERS: &str = "textures/environment/overworld_cubemap/layers";

/// The ticks, in 24-hour clock hours, a Java sky fade names.
fn schedule_from_json(layer: &serde_json::Value) -> Option<SkySchedule> {
    let hours = |value: &serde_json::Value| value.as_f64().map(|hours| hours as f32);
    let fade = layer
        .get("fade")
        .and_then(serde_json::Value::as_array)
        .filter(|edges| edges.len() == 4)
        .and_then(|edges| {
            Some(SkyFade {
                start_in: assets::sky_ticks_from_hours(hours(&edges[0])?),
                end_in: assets::sky_ticks_from_hours(hours(&edges[1])?),
                start_out: assets::sky_ticks_from_hours(hours(&edges[2])?),
                end_out: assets::sky_ticks_from_hours(hours(&edges[3])?),
            })
        });
    let rotation = layer.get("rotation").filter(|value| value.is_object()).and_then(|value| {
        let axis = value.get("axis")?.as_array()?;
        let axis: [f32; 3] = [
            axis.first()?.as_f64()? as f32,
            axis.get(1)?.as_f64()? as f32,
            axis.get(2)?.as_f64()? as f32,
        ];
        let speed = value.get("speed")?.as_f64()? as f32;
        (speed.is_finite() && axis.iter().all(|c| c.is_finite()))
            .then_some(SkyRotation { speed, axis })
    });
    let weather = layer
        .get("weather")
        .and_then(serde_json::Value::as_u64)
        .map_or(SkyWeather::ALL, |bits| SkyWeather((bits & 7) as u8));
    let days = layer.get("days").filter(|value| value.is_object()).map(|days| SkyDays {
        ranges: days
            .get("ranges")
            .and_then(serde_json::Value::as_array)
            .map(|ranges| {
                ranges
                    .iter()
                    .filter_map(|range| {
                        let range = range.as_array()?;
                        Some((
                            u32::try_from(range.first()?.as_u64()?).ok()?,
                            u32::try_from(range.get(1)?.as_u64()?).ok()?,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        repeat: days
            .get("repeat")
            .and_then(serde_json::Value::as_u64)
            .and_then(|repeat| u32::try_from(repeat).ok()),
    });
    Some(SkySchedule {
        blend: layer
            .get("blend")
            .and_then(serde_json::Value::as_str)
            .and_then(SkyBlend::from_name)
            .unwrap_or(SkyBlend::Add),
        fade,
        rotation,
        weather,
        days,
    })
}

/// Reads the overworld's Java sky layers: the schedule file, then each distinct source's six
/// faces. Layers beyond [`MAX_SKY_LAYERS`] and layers whose source has no face are dropped.
fn prepare_overworld_layers(view: &LayeredPackView, skybox: &mut PackSkybox) {
    let path = format!("{OVERWORLD_LAYERS}.json");
    let limit = resource_pack::MAX_PACK_TEXTURE_BYTES;
    if let Some(dependencies) = view.dependencies() {
        dependencies.extend([resource_pack::PackDependency::File {
            path: path.clone(),
            limit,
        }]);
    }
    let Some(bytes) = view
        .layers()
        .rev()
        .find_map(|layer| layer.read_file_with_limit(&path, limit).ok().flatten())
    else {
        return;
    };
    let Ok(document) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return;
    };
    let Some(entries) = document.get("layers").and_then(serde_json::Value::as_array) else {
        return;
    };
    let mut source_index = std::collections::BTreeMap::new();
    for entry in entries.iter().take(MAX_SKY_LAYERS) {
        let (Some(source), Some(schedule)) = (
            entry.get("source").and_then(serde_json::Value::as_u64),
            schedule_from_json(entry),
        ) else {
            continue;
        };
        let index = match source_index.get(&source) {
            Some(index) => *index,
            None => {
                let faces = std::array::from_fn(|face| {
                    let texture = decode_face(view, &format!("{OVERWORLD_LAYERS}/{source}/cubemap_{face}"))?;
                    Some(SkyboxFace {
                        width: texture.width,
                        height: texture.height,
                        rgba8: texture.rgba8,
                    })
                });
                if faces.iter().all(Option::is_none) {
                    continue;
                }
                skybox.sources.push(faces);
                source_index.insert(source, skybox.sources.len() - 1);
                skybox.sources.len() - 1
            }
        };
        skybox.layers.push(SkyLayer {
            source: index,
            schedule,
        });
    }
}

/// Chooses the highest pack, then its first image extension; an invalid winner stays absent.
fn decode_face(view: &LayeredPackView, path: &str) -> Option<crate::pack_textures::DecodedTexture> {
    let limit = resource_pack::MAX_PACK_TEXTURE_BYTES;
    let candidates =
        IMAGE_EXTENSIONS.map(|(extension, format)| (format!("{path}.{extension}"), format));
    if let Some(dependencies) = view.dependencies() {
        dependencies.extend(candidates.iter().map(|(path, _)| {
            resource_pack::PackDependency::File {
                path: path.clone(),
                limit,
            }
        }));
    }
    for layer in view.layers().rev() {
        for (path, format) in &candidates {
            if let Some(bytes) = layer.read_file_with_limit(path, limit).ok()? {
                // The bounded raster decoder does not admit HDR or compressed KTX images.
                return crate::pack_textures::decode_image(&bytes, (*format)?);
            }
        }
    }
    None
}
