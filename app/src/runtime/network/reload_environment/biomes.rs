use super::{decode_pack_texture, parse_pack_json, parse_rgb};
use assets::{BlockOverlay, RuntimeAssets, TintMapId, TintSource};
use resource_pack::LayeredPackView;
use serde_json::Value;

/// Applies pack colormaps and client-biome colour components to the worker's block overlay.
pub(in crate::runtime::network) fn apply_biome_overlay(
    view: &LayeredPackView,
    base: &RuntimeAssets,
    overlay: &mut BlockOverlay,
) {
    let mut biomes = base.biome_assets().clone();
    let mut changed = false;
    let map_pixels = (assets::TINT_MAP_SIZE * assets::TINT_MAP_SIZE) as usize;
    for map in TintMapId::ALL {
        let path = format!("textures/colormap/{}.png", map.source_name());
        let Some(texture) = decode_pack_texture(view, &path) else {
            continue;
        };
        if texture.width != assets::TINT_MAP_SIZE || texture.height != assets::TINT_MAP_SIZE {
            continue;
        }
        let target = &mut biomes.tint_maps_rgb8[map as usize * map_pixels * 3..][..map_pixels * 3];
        for (output, rgba) in target
            .as_chunks_mut::<3>()
            .0
            .iter_mut()
            .zip(texture.rgba8.as_chunks::<4>().0.iter())
        {
            output.copy_from_slice(&rgba[..3]);
        }
        changed = true;
    }
    changed |= apply_water_file(view, &mut biomes);
    for (_, bytes) in super::layered_json(view, "biomes/") {
        let Some(root) = parse_pack_json(&bytes) else {
            continue;
        };
        let biome = &root["minecraft:client_biome"];
        let Some(identifier) = biome["description"]["identifier"].as_str() else {
            continue;
        };
        let Some(rule) = biomes
            .rules
            .iter_mut()
            .find(|rule| rule.name.as_ref() == identifier)
        else {
            continue;
        };
        let components = &biome["components"];
        for (component, field, target) in [
            ("minecraft:grass_appearance", "color", &mut rule.grass),
            ("minecraft:foliage_appearance", "color", &mut rule.foliage),
            (
                "minecraft:dry_foliage_color",
                "color",
                &mut rule.dry_foliage,
            ),
            (
                "minecraft:water_appearance",
                "surface_color",
                &mut rule.water,
            ),
        ] {
            if let Some(source) = tint(&components[component][field]) {
                *target = source;
                changed = true;
            }
        }
        if let Some(shaded) = components["minecraft:grass_appearance"]["grass_is_shaded"].as_bool()
        {
            rule.flags = (rule.flags & !assets::BIOME_RULE_FLAG_GRASS_SHADED)
                | if shaded {
                    assets::BIOME_RULE_FLAG_GRASS_SHADED
                } else {
                    0
                };
            changed = true;
        }
    }
    if changed {
        overlay.biomes = Some(biomes);
    }
}

/// The pack file recolouring water for every biome at once.
const WATER_FILE: &str = "cinnabar/water.json";

/// Applies `cinnabar/water.json`: a `default` colour for every biome, per-biome `biomes`
/// colours, and an `opacity` replacing the biome's water opacity. Returns whether any rule
/// changed. Client-biome files applied afterwards still win for the biomes they name.
pub(super) fn apply_water_file(
    view: &LayeredPackView,
    biomes: &mut assets::CompiledBiomeAssets,
) -> bool {
    let mut changed = false;
    for (_, bytes) in super::layered_json(view, WATER_FILE) {
        let Some(root) = parse_pack_json(&bytes) else {
            continue;
        };
        let default = parse_rgb(&root["default"]);
        let opacity = root["opacity"]
            .as_f64()
            .filter(|opacity| (0.0..=1.0).contains(opacity));
        for rule in biomes.rules.iter_mut() {
            if let Some(rgb) = parse_rgb(&root["biomes"][rule.name.as_ref()]).or(default) {
                rule.water = TintSource::direct(rgb);
                changed = true;
            }
            if let Some(opacity) = opacity
                && rule.set_water_opacity(opacity as f32).is_ok()
            {
                changed = true;
            }
        }
    }
    changed
}

/// Resolves supported named colormaps and direct RGB colours.
fn tint(value: &Value) -> Option<TintSource> {
    value["color_map"]
        .as_str()
        .and_then(TintMapId::from_source_name)
        .map(TintSource::map)
        .or_else(|| parse_rgb(value).map(TintSource::direct))
}
