//! WAILA ("what am I looking at"): the name of the block under the crosshair for a HUD element.
//!
//! Pure text logic, no host calls: `guest.rs` reads the target block and its identifier and hands them here.

/// A kind of tool, as Jade shows it: the icon is the iron one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Pickaxe,
    Axe,
    Shovel,
    Hoe,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Pickaxe => "Pickaxe",
            Kind::Axe => "Axe",
            Kind::Shovel => "Shovel",
            Kind::Hoe => "Hoe",
        }
    }

    /// The item drawn for the tool.
    pub fn icon(self) -> &'static str {
        match self {
            Kind::Pickaxe => "minecraft:iron_pickaxe",
            Kind::Axe => "minecraft:iron_axe",
            Kind::Shovel => "minecraft:iron_shovel",
            Kind::Hoe => "minecraft:iron_hoe",
        }
    }
}

/// How a block is mined: with which tool, how good a one (0 any, 1 stone, 2 iron, 3 diamond), and whether the block
/// gives nothing without the right one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mining {
    pub kind: Kind,
    pub tier: u8,
    pub required: bool,
}

impl Mining {
    /// The item drawn for the tool: always the weakest one that mines the block (a wooden, stone, iron or diamond tool).
    pub fn icon(&self) -> String {
        let material = match self.tier {
            0 => "wooden",
            1 => "stone",
            2 => "iron",
            _ => "diamond",
        };
        let tool = self.kind.icon().rsplit('_').next().unwrap_or("pickaxe");
        format!("minecraft:{material}_{tool}")
    }

    /// "Pickaxe" or "Pickaxe (Iron)".
    pub fn label(&self) -> String {
        match (self.kind, self.tier) {
            (Kind::Pickaxe, 1) => "Pickaxe (Stone)".to_owned(),
            (Kind::Pickaxe, 2) => "Pickaxe (Iron)".to_owned(),
            (Kind::Pickaxe, 3) => "Pickaxe (Diamond)".to_owned(),
            (kind, _) => kind.name().to_owned(),
        }
    }

    /// Whether the held item (an identifier, "minecraft:iron_pickaxe") gets the block's drops. A block that needs no
    /// particular tool is always fine.
    pub fn can_harvest(&self, held: &str) -> bool {
        if !self.required {
            return true;
        }
        held_tool(held).is_some_and(|(kind, tier)| kind == self.kind && tier >= self.tier)
    }
}

/// The kind and tier of a tool item, "minecraft:stone_axe" -> (Axe, 1).
pub fn held_tool(item: &str) -> Option<(Kind, u8)> {
    let path = item.rsplit(':').next().unwrap_or(item);
    let (material, kind) = path.rsplit_once('_')?;
    let kind = match kind {
        "pickaxe" => Kind::Pickaxe,
        "axe" => Kind::Axe,
        "shovel" => Kind::Shovel,
        "hoe" => Kind::Hoe,
        _ => return None,
    };
    let tier = match material {
        "wooden" | "wood" | "golden" | "gold" => 0,
        "stone" => 1,
        "iron" => 2,
        "diamond" | "netherite" => 3,
        _ => return None,
    };
    Some((kind, tier))
}

/// What the element shows: a block's (or a mob's) display name, its identifier and where it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WailaView {
    /// "Oak Log".
    pub name: String,
    /// "minecraft:oak_log".
    pub id: String,
    /// The block's position.
    pub pos: [i32; 3],
    /// "Minecraft": the mod the block comes from.
    pub mod_name: String,
    /// A crop's growth in percent, for a block that grows.
    pub growth: Option<u32>,
    /// How the block is mined.
    pub mining: Option<Mining>,
    /// A redstone block's signal strength.
    pub power: Option<u32>,
    /// A mob's health and most health, in tenths of a heart-point (so the view stays comparable).
    pub health: Option<(u32, u32)>,
    /// How far the block is, in tenths of a block (only filled while the option is on).
    pub distance: Option<u32>,
    /// The item drawn as the icon ("minecraft:air" draws nothing).
    pub icon: String,
    /// Whether the held item gets the block's drops (set each frame from what is in hand; none when not asked).
    pub harvest: Option<bool>,
}

/// The longest name or identifier kept (the HUD box would otherwise stretch for a very long one).
const MAX_CHARS: usize = 40;

/// The display name of a block identifier: the namespace is dropped and each word capitalised,
/// "minecraft:oak_log" -> "Oak Log". An identifier with nothing readable in it gives "?".
pub fn display_name(identifier: &str) -> String {
    let path = identifier.rsplit(':').next().unwrap_or(identifier);
    let mut out = String::new();
    for word in path.split(['_', ' ', '/']).filter(|w| !w.is_empty()) {
        if !out.is_empty() {
            out.push(' ');
        }
        let mut chars = word.chars().filter(|c| !c.is_control());
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.extend(chars);
        }
        if out.chars().count() >= MAX_CHARS {
            break;
        }
    }
    let out: String = out.chars().take(MAX_CHARS).collect();
    if out.is_empty() { "?".to_owned() } else { out }
}

/// The identifier as shown on the second line: control characters removed, cut to the maximum.
pub fn clean_id(identifier: &str) -> String {
    identifier.chars().filter(|c| !c.is_control()).take(MAX_CHARS + 16).collect()
}

/// The mod of a block identifier: its namespace, written like a name ("minecraft" -> "Minecraft").
/// A bare identifier belongs to Minecraft.
pub fn mod_name(identifier: &str) -> String {
    match identifier.split_once(':') {
        Some((namespace, _)) if !namespace.is_empty() => display_name(namespace),
        _ => "Minecraft".to_owned(),
    }
}

/// The number stored under `key` in a canonical state, whatever its notation: JSON (`{"growth":7}`,
/// `{"name":"minecraft:wheat","states":{"growth":7}}`) or the bracket form (`minecraft:wheat[age=7]`).
pub fn state_value(canonical: &str, key: &str) -> Option<u32> {
    let bytes = canonical.as_bytes();
    let mut from = 0;
    while let Some(found) = canonical[from..].find(key) {
        let start = from + found;
        let end = start + key.len();
        from = end;
        // The key is a whole word: not part of a longer name.
        let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        if start > 0 && word(bytes[start - 1]) || bytes.get(end).is_some_and(|b| word(*b)) {
            continue;
        }
        let rest = canonical[end..].trim_start_matches(['"', '\'', ' ']);
        let Some(rest) = rest.strip_prefix([':', '=']) else { continue };
        let digits: String = rest.trim_start_matches([' ', '"', '\'']).chars().take_while(char::is_ascii_digit).collect();
        if let Ok(value) = digits.parse() {
            return Some(value);
        }
    }
    None
}

/// The last growth stage of a crop block; None for a block that does not grow.
fn max_growth(identifier: &str) -> Option<u32> {
    let path = identifier.rsplit(':').next().unwrap_or(identifier);
    match path {
        "wheat" | "carrots" | "potatoes" | "beetroot" | "melon_stem" | "pumpkin_stem" | "torchflower_crop" => Some(7),
        "beetroots" | "nether_wart" | "sweet_berry_bush" => Some(3),
        "cocoa" => Some(2),
        "pitcher_crop" => Some(4),
        _ => None,
    }
}

/// A crop's growth in percent (0 to 100) from its identifier and state, "growth" or "age" as the game names it.
pub fn growth(identifier: &str, canonical: Option<&str>) -> Option<u32> {
    let max = max_growth(identifier)?;
    let state = canonical?;
    let stage = state_value(state, "growth").or_else(|| state_value(state, "age"))?;
    Some((stage.min(max) * 100 + max / 2) / max)
}

/// The tool of a block from its tags ("minecraft:mineable/pickaxe", "minecraft:is_pickaxe_item_destructible",
/// "iron_pick_diggable" and so on); the tier is read from the "needs" / "diggable" tags.
fn mining_from_tags(tags: &[String]) -> Option<Mining> {
    let words: Vec<String> = tags
        .iter()
        .flat_map(|t| t.split(|c: char| !c.is_ascii_alphanumeric()).map(str::to_ascii_lowercase).collect::<Vec<_>>())
        .collect();
    let has = |w: &str| words.iter().any(|x| x == w);
    let kind = if has("pickaxe") || has("pick") {
        Kind::Pickaxe
    } else if has("shovel") {
        Kind::Shovel
    } else if has("hoe") {
        Kind::Hoe
    } else if has("axe") {
        Kind::Axe
    } else {
        return None;
    };
    let tiered: Vec<&String> = tags.iter().filter(|t| t.contains("needs") || t.contains("diggable")).collect();
    let tier = [("diamond", 3), ("iron", 2), ("stone", 1)]
        .into_iter()
        .find(|(name, _)| tiered.iter().any(|t| t.to_ascii_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == *name)))
        .map_or(0, |(_, tier)| tier);
    Some(Mining { kind, tier, required: kind == Kind::Pickaxe || !tiered.is_empty() })
}

/// The tool that mines a block, from its tags: "Pickaxe", "Pickaxe (Iron)", "Axe"...
pub fn tool(tags: &[String]) -> Option<String> {
    mining_from_tags(tags).map(|m| m.label())
}

/// The tool a block is mined with, guessed from its name when the tags say nothing (the tag names a host reports
/// vary): soft ground by the shovel, wood by the axe, stone and ores by the pickaxe.
fn mining_from_name(identifier: &str) -> Option<Mining> {
    let path = identifier.rsplit(':').next().unwrap_or(identifier);
    let any = |words: &[&str]| words.iter().any(|w| path.contains(w));
    let plain = |kind| Some(Mining { kind, tier: 0, required: false });
    let pick = |tier| Some(Mining { kind: Kind::Pickaxe, tier, required: true });
    if any(&["leaves", "hay_block", "sponge", "moss", "wart_block", "shroomlight", "sculk"]) && !path.contains("stone") {
        return plain(Kind::Hoe);
    }
    if any(&["dirt", "grass_block", "podzol", "mycelium", "sand", "gravel", "clay", "snow", "mud", "soul_soil", "farmland", "concrete_powder", "path"]) && !path.contains("sandstone") && !path.contains("mud_brick") {
        return plain(Kind::Shovel);
    }
    if any(&["log", "planks", "wood", "stem", "hyphae", "crafting_table", "chest", "bookshelf", "barrel", "fence", "door", "sign", "ladder", "bamboo", "pumpkin", "melon", "cactus", "note_block", "jukebox", "lectern", "loom", "beehive"]) && !path.contains("stone") {
        return plain(Kind::Axe);
    }
    if path.contains("nether_gold_ore") {
        return pick(0);
    }
    if any(&["obsidian", "ancient_debris", "netherite", "respawn_anchor"]) {
        return pick(3);
    }
    if any(&["diamond_ore", "emerald_ore", "gold_ore", "redstone_ore", "diamond_block", "emerald_block", "gold_block", "redstone_block"]) {
        return pick(2);
    }
    if any(&["iron_ore", "lapis", "copper", "iron_block", "raw_iron"]) {
        return pick(1);
    }
    if any(&["stone", "ore", "brick", "deepslate", "cobble", "basalt", "blackstone", "netherrack", "terracotta", "concrete", "furnace", "anvil", "prismarine", "end_", "purpur", "quartz", "glazed", "iron_", "rail", "hopper", "cauldron", "lantern", "chain", "bell", "magma", "ice", "tuff", "calcite", "dripstone", "andesite", "diorite", "granite", "coal", "amethyst"]) {
        return pick(0);
    }
    None
}

/// `mining_from_name` as the text it shows.
pub fn tool_by_name(identifier: &str) -> Option<String> {
    mining_from_name(identifier).map(|m| m.label())
}

/// How a block is mined: what its tags say, and the name's guess for the rest (the better tier of the two).
pub fn mining(identifier: &str, tags: &[String]) -> Option<Mining> {
    match (mining_from_tags(tags), mining_from_name(identifier)) {
        (Some(a), Some(b)) if a.kind == b.kind => Some(Mining { tier: a.tier.max(b.tier), required: a.required || b.required, ..a }),
        (Some(a), _) => Some(a),
        (None, b) => b,
    }
}

/// The distance from the eye to the middle of the block, in tenths of a block.
pub fn distance_tenths(eye: [f32; 3], block: [i32; 3]) -> Option<u32> {
    let d = [eye[0] - (block[0] as f32 + 0.5), eye[1] - (block[1] as f32 + 0.5), eye[2] - (block[2] as f32 + 0.5)];
    let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    length.is_finite().then(|| (length * 10.0).round().clamp(0.0, 9999.0) as u32)
}

/// A redstone block's signal strength from its state ("redstone_signal" or "power").
pub fn power(canonical: Option<&str>) -> Option<u32> {
    let state = canonical?;
    state_value(state, "redstone_signal").or_else(|| state_value(state, "power")).map(|p| p.min(15))
}

/// The mobs that have a spawn egg, whose item is the icon Jade-style displays beside a mob's name.
const EGGS: &[&str] = &[
    "allay", "armadillo", "axolotl", "bat", "bee", "blaze", "bogged", "breeze", "camel", "cat", "cave_spider", "chicken", "cod",
    "cow", "creaking", "creeper", "dolphin", "donkey", "drowned", "elder_guardian", "enderman", "endermite", "evoker", "fox",
    "frog", "ghast", "glow_squid", "goat", "guardian", "hoglin", "horse", "husk", "llama", "magma_cube", "mooshroom", "mule",
    "ocelot", "panda", "parrot", "phantom", "pig", "piglin", "piglin_brute", "pillager", "polar_bear", "pufferfish", "rabbit",
    "ravager", "salmon", "sheep", "shulker", "silverfish", "skeleton", "skeleton_horse", "slime", "sniffer", "spider", "squid",
    "stray", "strider", "tadpole", "trader_llama", "tropical_fish", "turtle", "vex", "villager", "vindicator", "wandering_trader",
    "warden", "witch", "wither_skeleton",
];
const EGGS_MORE: &[&str] = &["wolf", "zoglin", "zombie", "zombie_horse", "zombie_villager"];

/// The spawn egg item of a mob ("minecraft:zombie" -> "minecraft:zombie_spawn_egg"), air for one without an egg
/// (players, items, arrows...). Bedrock's second-generation ids ("villager_v2") are named without the suffix.
pub fn spawn_egg(identifier: &str) -> String {
    let (namespace, path) = identifier.split_once(':').unwrap_or(("minecraft", identifier));
    let path = path.strip_suffix("_v2").unwrap_or(path);
    if namespace == "minecraft" && (EGGS.contains(&path) || EGGS_MORE.contains(&path)) {
        format!("minecraft:{path}_spawn_egg")
    } else {
        "minecraft:air".to_owned()
    }
}

/// A mob's view: its name, identifier and health (in tenths).
pub fn entity(name: &str, identifier: &str, health: Option<f32>, max_health: Option<f32>) -> WailaView {
    let tenths = |v: f32| (v.max(0.0) * 10.0).round().min(99_999.0) as u32;
    WailaView {
        name: name.chars().filter(|c| !c.is_control()).take(MAX_CHARS).collect::<String>(),
        id: clean_id(identifier),
        pos: [0; 3],
        mod_name: mod_name(identifier),
        health: health.filter(|h| h.is_finite()).zip(max_health.filter(|h| h.is_finite())).map(|(h, m)| (tenths(h), tenths(m))),
        icon: spawn_egg(identifier),
        ..WailaView::default()
    }
}

/// The view of the block with `identifier` at `pos`.
pub fn view(identifier: &str, pos: [i32; 3]) -> WailaView {
    describe(identifier, None, &[], pos)
}

/// The view of a block from everything the terrain read says about it.
pub fn describe(identifier: &str, canonical: Option<&str>, tags: &[String], pos: [i32; 3]) -> WailaView {
    WailaView {
        name: display_name(identifier),
        id: clean_id(identifier),
        pos,
        mod_name: mod_name(identifier),
        growth: growth(identifier, canonical),
        mining: mining(identifier, tags),
        power: power(canonical),
        health: None,
        distance: None,
        icon: clean_id(identifier),
        harvest: None,
    }
}

/// The health line of a mob, "Health: 14 / 20" (whole numbers plain, otherwise one decimal).
pub fn health_text(health: (u32, u32)) -> String {
    let show = |tenths: u32| if tenths % 10 == 0 { (tenths / 10).to_string() } else { format!("{}.{}", tenths / 10, tenths % 10) };
    format!("Health: {} / {}", show(health.0), show(health.1))
}

/// The distance line, "Distance: 4.5".
pub fn distance_text(tenths: u32) -> String {
    format!("Distance: {}.{}", tenths / 10, tenths % 10)
}

/// The text of the position line, "X 12  Y 64  Z -3".
pub fn position_text(pos: [i32; 3]) -> String {
    format!("X {}  Y {}  Z {}", pos[0], pos[1], pos[2])
}

/// What the element shows while it is being placed and nothing is aimed at.
pub fn sample() -> WailaView {
    let mut sample = describe("minecraft:iron_ore", None, &["minecraft:mineable/pickaxe".to_owned(), "minecraft:needs_stone_tool".to_owned()], [12, 64, -3]);
    sample.growth = Some(57);
    sample.harvest = Some(true);
    sample
}

/// The block the target point is in: the point is the block's centre, so it is floored on each axis.
/// None when any coordinate is not a finite number.
pub fn block_of(point: [f32; 3]) -> Option<[i32; 3]> {
    if point.iter().any(|v| !v.is_finite()) {
        return None;
    }
    Some([point[0].floor() as i32, point[1].floor() as i32, point[2].floor() as i32])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_readable() {
        assert_eq!(display_name("minecraft:oak_log"), "Oak Log");
        assert_eq!(display_name("minecraft:stone"), "Stone");
        assert_eq!(display_name("minecraft:deepslate_diamond_ore"), "Deepslate Diamond Ore");
        assert_eq!(display_name("othermod:copper_pipe"), "Copper Pipe");
        assert_eq!(display_name("tnt"), "Tnt");
    }

    #[test]
    fn odd_identifiers_never_give_an_empty_name() {
        assert_eq!(display_name(""), "?");
        assert_eq!(display_name("minecraft:"), "?");
        assert_eq!(display_name("minecraft:___"), "?");
        assert_eq!(display_name("minecraft:a\u{7}b"), "Ab");
    }

    #[test]
    fn long_names_are_cut() {
        let long = format!("minecraft:{}", "very_long_word_".repeat(20));
        assert!(display_name(&long).chars().count() <= MAX_CHARS);
        assert!(clean_id(&long).chars().count() <= MAX_CHARS + 16);
    }

    #[test]
    fn the_view_carries_name_id_and_position() {
        let v = view("minecraft:oak_log", [1, 2, 3]);
        assert_eq!((v.name.as_str(), v.id.as_str(), v.pos, v.mod_name.as_str()), ("Oak Log", "minecraft:oak_log", [1, 2, 3], "Minecraft"));
        assert_eq!(position_text([12, 64, -3]), "X 12  Y 64  Z -3");
    }

    #[test]
    fn mods_come_from_the_namespace() {
        assert_eq!(mod_name("minecraft:stone"), "Minecraft");
        assert_eq!(mod_name("create:brass_casing"), "Create");
        assert_eq!(mod_name("my_mod:thing"), "My Mod");
        assert_eq!(mod_name("stone"), "Minecraft");
    }

    #[test]
    fn crop_growth_reads_age_or_growth() {
        assert_eq!(growth("minecraft:wheat", Some("minecraft:wheat[age=7]")), Some(100));
        assert_eq!(growth("minecraft:wheat", Some("minecraft:wheat[\"growth\"=3]")), Some(43));
        assert_eq!(growth("minecraft:nether_wart", Some("minecraft:nether_wart[age=1]")), Some(33));
        assert_eq!(growth("minecraft:wheat", Some("minecraft:wheat[age=99]")), Some(100));
        assert_eq!(growth("minecraft:stone", Some("minecraft:stone[age=1]")), None);
        assert_eq!(growth("minecraft:wheat", None), None);
        assert_eq!(growth("minecraft:wheat", Some("minecraft:wheat")), None);
        assert_eq!(growth("minecraft:wheat", Some("{\"growth\":7}")), Some(100));
        assert_eq!(growth("minecraft:beetroot", Some("{\"name\":\"minecraft:beetroot\",\"states\":{\"growth\":2}}")), Some(29));
        assert_eq!(state_value("{\"age_x\":4,\"age\":2}", "age"), Some(2));
    }

    #[test]
    fn the_tool_comes_from_the_tags() {
        let t = |v: &[&str]| tool(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(t(&["minecraft:mineable/pickaxe"]).as_deref(), Some("Pickaxe"));
        assert_eq!(t(&["minecraft:mineable/pickaxe", "minecraft:needs_iron_tool"]).as_deref(), Some("Pickaxe (Iron)"));
        assert_eq!(t(&["minecraft:mineable/axe"]).as_deref(), Some("Axe"));
        assert_eq!(t(&["minecraft:is_shovel_item_destructible"]).as_deref(), Some("Shovel"));
        assert_eq!(t(&["diamond_pick_diggable"]).as_deref(), Some("Pickaxe (Diamond)"));
        assert_eq!(t(&["stone", "wood"]), None);
        assert_eq!(t(&[]), None);
    }

    #[test]
    fn tools_are_guessed_by_name() {
        assert_eq!(tool_by_name("minecraft:grass_block").as_deref(), Some("Shovel"));
        assert_eq!(tool_by_name("minecraft:oak_log").as_deref(), Some("Axe"));
        assert_eq!(tool_by_name("minecraft:stone").as_deref(), Some("Pickaxe"));
        assert_eq!(tool_by_name("minecraft:sandstone").as_deref(), Some("Pickaxe"));
        assert_eq!(tool_by_name("minecraft:diamond_ore").as_deref(), Some("Pickaxe (Iron)"));
        assert_eq!(tool_by_name("minecraft:obsidian").as_deref(), Some("Pickaxe (Diamond)"));
        assert_eq!(tool_by_name("minecraft:oak_leaves").as_deref(), Some("Hoe"));
        assert_eq!(tool_by_name("minecraft:wheat"), None);
    }

    #[test]
    fn harvesting_compares_the_held_tool() {
        let iron_ore = mining("minecraft:iron_ore", &[]).unwrap();
        assert_eq!((iron_ore.kind, iron_ore.tier, iron_ore.required), (Kind::Pickaxe, 1, true));
        assert!(iron_ore.can_harvest("minecraft:stone_pickaxe"));
        assert!(iron_ore.can_harvest("minecraft:diamond_pickaxe"));
        assert!(!iron_ore.can_harvest("minecraft:wooden_pickaxe"));
        assert!(!iron_ore.can_harvest("minecraft:iron_axe"));
        assert!(!iron_ore.can_harvest("minecraft:air"));
        let dirt = mining("minecraft:dirt", &[]).unwrap();
        assert!(!dirt.required && dirt.can_harvest("minecraft:air"));
        let diamond = mining("minecraft:diamond_ore", &["minecraft:needs_iron_tool".to_owned()]).unwrap();
        assert_eq!(diamond.tier, 2);
        assert!(!diamond.can_harvest("minecraft:stone_pickaxe") && diamond.can_harvest("minecraft:iron_pickaxe"));
        assert_eq!(held_tool("minecraft:netherite_shovel"), Some((Kind::Shovel, 3)));
        assert_eq!(held_tool("minecraft:stick"), None);
        assert_eq!(Kind::Hoe.icon(), "minecraft:iron_hoe");
        let m = |kind, tier, required| Mining { kind, tier, required };
        assert_eq!(m(Kind::Pickaxe, 3, true).icon(), "minecraft:diamond_pickaxe");
        assert_eq!(m(Kind::Pickaxe, 1, true).icon(), "minecraft:stone_pickaxe");
        assert_eq!(m(Kind::Pickaxe, 0, true).icon(), "minecraft:wooden_pickaxe");
        assert_eq!(m(Kind::Axe, 0, false).icon(), "minecraft:wooden_axe");
        assert_eq!(mining("minecraft:obsidian", &[]).unwrap().icon(), "minecraft:diamond_pickaxe");
    }

    #[test]
    fn power_distance_and_mobs() {
        assert_eq!(power(Some("{\"redstone_signal\":12}")), Some(12));
        assert_eq!(power(Some("minecraft:redstone_wire[power=3]")), Some(3));
        assert_eq!(power(None), None);
        assert_eq!(distance_tenths([0.5, 0.5, 0.5], [0, 0, 3]), Some(30));
        assert_eq!(distance_tenths([f32::NAN, 0.0, 0.0], [0, 0, 0]), None);
        let cow = entity("Cow", "minecraft:cow", Some(7.5), Some(10.0));
        assert_eq!(cow.health, Some((75, 100)));
        assert_eq!((cow.mod_name.as_str(), cow.icon.as_str()), ("Minecraft", "minecraft:cow_spawn_egg"));
        assert_eq!(spawn_egg("minecraft:villager_v2"), "minecraft:villager_spawn_egg");
        assert_eq!(spawn_egg("minecraft:player"), "minecraft:air");
        assert_eq!(spawn_egg("create:thing"), "minecraft:air");
        assert_eq!(spawn_egg("minecraft:zombie_villager_v2"), "minecraft:zombie_villager_spawn_egg");
        assert_eq!(health_text((75, 100)), "Health: 7.5 / 10");
        assert_eq!(health_text((200, 200)), "Health: 20 / 20");
        assert_eq!(distance_text(45), "Distance: 4.5");
    }

    #[test]
    fn the_block_of_a_point_is_floored() {
        assert_eq!(block_of([10.5, 64.5, -2.5]), Some([10, 64, -3]));
        assert_eq!(block_of([-0.5, 0.5, 0.5]), Some([-1, 0, 0]));
        assert_eq!(block_of([f32::NAN, 0.0, 0.0]), None);
        assert_eq!(block_of([0.0, f32::INFINITY, 0.0]), None);
    }
}
