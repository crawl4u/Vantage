//! What a block does to light and to mob spawning, from the client's own block light tables.

use crate::blockdata::{EMIT_HASH, EMIT_STATE, FILTER_HASH, VARIABLE_HASH};
#[cfg(test)]
use crate::blockdata::{EMIT, FILTER};
use mod_api::handled_player_mod::cinnabar::extension::terrain::{BlockState, Region, ResolvedShape, ShapeBinding};

/// Light a cell holds back, in the low nibble: 0 lets light through losing one level per
/// block, 15 stops it.
pub const FILTER_MASK: u8 = 0x0f;
/// A mob can stand on top of it: a full, solid, opaque top face.
pub const FLOOR: u8 = 0x10;
/// Nothing to collide with and no liquid: a mob's body fits here.
pub const CLEAR: u8 = 0x20;
pub const LIQUID: u8 = 0x40;
/// The terrain reader had this cell loaded.
pub const KNOWN: u8 = 0x80;

pub fn short(identifier: &str) -> &str {
    identifier.strip_prefix("minecraft:").unwrap_or(identifier)
}

/// FNV-1a of a short identifier. Lookups go by this hash because comparing strings costs
/// wasm fuel per byte; the generator checks every known identifier hashes uniquely.
pub const fn name_hash(name: &str) -> u32 {
    let bytes = name.as_bytes();
    let mut hash = 0x811c_9dc5u32;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        index += 1;
    }
    hash
}

fn by_hash(table: &[(u32, u8)], hash: u32) -> Option<u8> {
    table.binary_search_by(|(candidate, _)| candidate.cmp(&hash)).ok().map(|index| table[index].1)
}

/// How much light a block holds back; unknown blocks are treated as opaque.
pub fn filter(identifier: &str) -> u8 {
    by_hash(FILTER_HASH, name_hash(short(identifier))).unwrap_or(15)
}

/// The light a block gives off in this state.
pub fn emission(identifier: &str, state: Option<&str>) -> u8 {
    let name = short(identifier);
    let hash = name_hash(name);
    if let Some(light) = by_hash(EMIT_HASH, hash) {
        return light;
    }
    if VARIABLE_HASH.binary_search(&hash).is_err() {
        return 0;
    }
    // Candles, campfires, copper bulbs and the like: rare enough for string work.
    let Some(key) = state.map(state_key) else {
        return 0;
    };
    let start = EMIT_STATE.partition_point(|(candidate, _, _)| *candidate < name);
    EMIT_STATE[start..]
        .iter()
        .take_while(|(candidate, _, _)| *candidate == name)
        .find(|(_, candidate, _)| *candidate == key)
        .map_or(0, |(_, _, light)| *light)
}

/// A block state as sorted `name=value` pairs, whichever JSON shape the host sends it in:
/// `{"lit":{"type":"byte","value":1}}`, `{"lit":1}` or `{"lit":true}`.
pub fn state_key(state: &str) -> String {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(state) else {
        return String::new();
    };
    let mut pairs: Vec<String> = map
        .iter()
        .map(|(name, value)| {
            let value = value.get("value").unwrap_or(value);
            let text = match value {
                serde_json::Value::Bool(flag) => u8::from(*flag).to_string(),
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            format!("{name}={text}")
        })
        .collect();
    pairs.sort();
    pairs.join(",")
}

const MONUMENT: [u32; 4] = [
    name_hash("prismarine"),
    name_hash("prismarine_bricks"),
    name_hash("dark_prismarine"),
    name_hash("sea_lantern"),
];

const LIQUIDS: [u32; 4] =
    [name_hash("water"), name_hash("flowing_water"), name_hash("lava"), name_hash("flowing_lava")];

/// Blocks an ocean monument is built from.
pub fn is_monument_block(identifier: &str) -> bool {
    MONUMENT.contains(&name_hash(short(identifier)))
}

/// One palette entry of a region, reduced to what the light and spawn checks need.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Info {
    /// `FILTER_MASK` bits plus `FLOOR`, `CLEAR` and `LIQUID`.
    pub cell: u8,
    pub emission: u8,
    pub monument: bool,
}

/// Classifies a palette entry; `boxes` is its collision shape, `None` when unknown.
pub fn classify(state: &BlockState, boxes: Option<&[f64]>) -> Info {
    let identifier = state.identifier.as_deref().unwrap_or("");
    let flags = state.physics.as_ref().map(|physics| physics.flags);
    let liquid = LIQUIDS.contains(&name_hash(short(identifier))) || flags.is_some_and(|f| {
            f.contains(mod_api::handled_player_mod::cinnabar::extension::terrain::PhysicsFlags::WATER)
                || f.contains(mod_api::handled_player_mod::cinnabar::extension::terrain::PhysicsFlags::LAVA)
        });
    let filter = if identifier.is_empty() { 15 } else { filter(identifier) };
    let mut cell = filter & FILTER_MASK;
    if liquid {
        cell |= LIQUID;
    }
    match boxes {
        Some([]) if !liquid => cell |= CLEAR,
        Some(boxes) if filter == 15 && !liquid && full_top(boxes) => cell |= FLOOR,
        _ => {}
    }
    Info {
        cell,
        emission: if identifier.is_empty() { 0 } else { emission(identifier, state.canonical_state.as_deref()) },
        monument: is_monument_block(identifier),
    }
}

/// Whether one of the boxes covers the whole top face of the block.
fn full_top(boxes: &[f64]) -> bool {
    boxes.chunks_exact(6).any(|b| b[0] <= 0.001 && b[2] <= 0.001 && b[3] >= 0.999 && b[4] >= 0.999 && b[5] >= 0.999)
}

/// No layer in this slot.
pub const EMPTY_SLOT: u16 = 65535;

/// The collision boxes of a palette entry with a fixed shape: `Some(&[])` for no collision,
/// `None` when the shape is unknown or depends on the position.
pub fn fixed_boxes<'a>(region: &'a Region, state: &BlockState) -> Option<&'a [f64]> {
    let ShapeBinding::Fixed(ResolvedShape::Known(shape)) = state.collision else {
        return None;
    };
    if shape == 0 {
        return Some(&[]);
    }
    let start = *region.shape_offsets.get(shape as usize)? as usize;
    let end = *region.shape_offsets.get(shape as usize + 1)? as usize;
    region.shape_boxes.get(start * 6..end * 6)
}

/// Combines the waterlogging layer with the block in the same cell.
pub fn combine(base: u8, extra: u8) -> u8 {
    let filter = (base & FILTER_MASK).max(extra & FILTER_MASK);
    let mut cell = (base & !FILTER_MASK) | filter;
    if extra & LIQUID != 0 {
        cell = (cell | LIQUID) & !CLEAR & !FLOOR;
    }
    if extra & CLEAR == 0 && extra & LIQUID == 0 {
        // A second solid layer: nothing fits in this cell any more.
        cell &= !CLEAR;
    }
    cell
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashed_tables_agree_with_the_named_ones() {
        assert!(EMIT_HASH.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(FILTER_HASH.windows(2).all(|w| w[0].0 < w[1].0));
        for (name, light) in EMIT {
            assert_eq!(by_hash(EMIT_HASH, name_hash(name)), Some(*light), "{name}");
        }
        for (name, value) in FILTER {
            assert_eq!(filter(name), *value, "{name}");
        }
    }

    #[test]
    fn the_tables_are_sorted_for_binary_search() {
        assert!(EMIT.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(FILTER.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(EMIT_STATE.windows(2).all(|w| (w[0].0, w[0].1) < (w[1].0, w[1].1)));
    }

    #[test]
    fn light_sources_give_their_vanilla_light() {
        assert_eq!(emission("minecraft:torch", None), 14);
        assert_eq!(emission("minecraft:glowstone", Some("{}")), 15);
        assert_eq!(emission("minecraft:redstone_torch", None), 7);
        assert_eq!(emission("minecraft:soul_torch", None), 10);
        assert_eq!(emission("minecraft:stone", None), 0);
        let four_lit = r#"{"candles":{"type":"int","value":3},"lit":{"type":"byte","value":1}}"#;
        assert_eq!(emission("minecraft:candle", Some(four_lit)), 12);
        assert_eq!(emission("minecraft:candle", Some(r#"{"candles":3,"lit":true}"#)), 12);
        assert_eq!(emission("minecraft:candle", Some(r#"{"candles":3,"lit":false}"#)), 0);
        let lit = r#"{"extinguished":0,"minecraft:cardinal_direction":"north"}"#;
        assert_eq!(emission("minecraft:campfire", Some(lit)), 15);
        let out = r#"{"extinguished":1,"minecraft:cardinal_direction":"north"}"#;
        assert_eq!(emission("minecraft:campfire", Some(out)), 0);
    }

    #[test]
    fn filters_match_the_client() {
        assert_eq!(filter("minecraft:air"), 0);
        assert_eq!(filter("minecraft:glass"), 0);
        assert_eq!(filter("minecraft:water"), 1);
        assert_eq!(filter("minecraft:stone"), 15);
        assert_eq!(filter("minecraft:something_new"), 15);
    }

    #[test]
    fn waterlogging_fills_the_cell() {
        let air = CLEAR;
        let water = LIQUID | 1;
        let cell = combine(air, water);
        assert_eq!(cell & CLEAR, 0);
        assert_ne!(cell & LIQUID, 0);
        assert_eq!(cell & FILTER_MASK, 1);
        assert!(is_monument_block("minecraft:dark_prismarine"));
        assert!(is_monument_block("minecraft:sea_lantern"));
        assert!(!is_monument_block("minecraft:prismarine_slab"));
    }
}
