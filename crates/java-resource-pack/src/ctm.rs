//! OptiFine connected-texture properties that pick a texture by block position.
//!
//! A `random` rule lists several textures for a block texture and OptiFine chooses one per block
//! position, with optional weights. Bedrock's terrain atlas does the same for a tile whose
//! `terrain_texture.json` entry lists weighted textures, so such a rule is written there. A
//! `repeat` rule lays a width-by-height pattern of textures over the block grid, which the entry
//! declares through the Cinnabar `cinnabar_repeat` extension. Rules that depend on neighbours,
//! biome, height or block state are not translated.

use std::collections::BTreeMap;

/// One texture a rule may pick.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Choice {
    /// The block texture the rule replaces, as the pack ships it.
    Default,
    /// A pack file, as a full path from the pack root.
    File(String),
}

/// A `random` or `repeat` rule for one block texture.
#[derive(Debug, PartialEq)]
pub(crate) struct Rule {
    /// The block texture name the rule matches (`dirt`, not `block/dirt`).
    pub(crate) texture: String,
    pub(crate) choices: Vec<(Choice, f32)>,
    /// The pattern's width and height for a `repeat` rule, whose choices fill it in row order.
    pub(crate) repeat: Option<(u8, u8)>,
}

/// Properties that make a rule depend on more than the block position.
const CONDITIONS: [&str; 9] = [
    "biomes",
    "heights",
    "metadata",
    "name",
    "nbt",
    "matchBlocks",
    "connect",
    "connectTiles",
    "weatherEffects",
];

/// Parses a `.properties` file's key/value lines; later duplicates win.
fn properties(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('!'))
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
        .collect()
}

/// A tile list's entries: `<default>`, `a-b` ranges and names, as they appear in a rule.
fn tile_tokens(list: &str) -> Vec<String> {
    list.split_whitespace()
        .flat_map(|token| {
            let range = token
                .split_once('-')
                .and_then(|(from, to)| Some((from.parse::<u32>().ok()?, to.parse::<u32>().ok()?)));
            match range {
                Some((from, to)) if from <= to && to - from < 1024 => {
                    (from..=to).map(|number| number.to_string()).collect()
                }
                _ => vec![token.to_owned()],
            }
        })
        .collect()
}

/// The pack path of a tile token for a rule in `folder` (a pack path ending in `/`).
fn tile_path(folder: &str, token: &str) -> String {
    let token = token.strip_prefix("minecraft:").unwrap_or(token);
    let file = if token.ends_with(".png") {
        token.to_owned()
    } else {
        format!("{token}.png")
    };
    if let Some(rest) = file.strip_prefix("./") {
        format!("{folder}{rest}")
    } else if let Some(rest) = file.strip_prefix("~/") {
        format!("assets/minecraft/{rest}")
    } else if file.contains('/') {
        format!("assets/minecraft/{file}")
    } else {
        format!("{folder}{file}")
    }
}

/// The `random` or `repeat` rule `text` declares for a rule file in `folder`, or `None` when it
/// is another method, depends on more than position, or lists nothing a pattern can fill.
pub(crate) fn parse(folder: &str, text: &str) -> Option<Rule> {
    let props = properties(text);
    let method = props.get("method").map(String::as_str)?;
    if !matches!(method, "random" | "repeat")
        || CONDITIONS.iter().any(|key| props.contains_key(*key))
    {
        return None;
    }
    let side = |name: &str| props.get(name)?.parse::<u8>().ok().filter(|side| *side > 0);
    let repeat = if method == "repeat" {
        Some((side("width")?, side("height")?))
    } else {
        None
    };
    let texture = props
        .get("matchTiles")?
        .split_whitespace()
        .next()?
        .trim_end_matches(".png");
    let texture = texture
        .strip_prefix("minecraft:")
        .unwrap_or(texture)
        .strip_prefix("block/")
        .unwrap_or(texture)
        .to_owned();
    let tokens = tile_tokens(props.get("tiles")?);
    let weights: Vec<f32> = props
        .get("weights")
        .map(|list| {
            list.split_whitespace()
                .filter_map(|weight| weight.parse().ok())
                .collect()
        })
        .unwrap_or_default();
    let choices: Vec<(Choice, f32)> = tokens
        .iter()
        .enumerate()
        .map(|(index, token)| {
            let choice = if token == "<default>" {
                Choice::Default
            } else {
                Choice::File(tile_path(folder, token))
            };
            let weight = weights
                .get(index)
                .copied()
                .filter(|w| *w > 0.0 && repeat.is_none())
                .unwrap_or(1.0);
            (choice, weight)
        })
        .collect();
    let fits = repeat.is_none_or(|(width, height)| {
        choices.len() == usize::from(width) * usize::from(height) && choices.len() > 1
    });
    (!choices.is_empty() && fits).then_some(Rule {
        texture,
        choices,
        repeat,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FOLDER: &str = "assets/minecraft/optifine/ctm/patrix/diorite/";

    #[test]
    fn a_random_rule_lists_default_ranges_and_names_with_weights() {
        let text = "method=random\nmatchTiles=bee_nest_side\ntiles=<default> 2-4 extra\nweights=5 1 1 1 2\n";
        let rule = parse(FOLDER, text).unwrap();
        assert_eq!(rule.texture, "bee_nest_side");
        let files: Vec<_> = rule
            .choices
            .iter()
            .map(|(choice, _)| choice.clone())
            .collect();
        assert_eq!(files[0], Choice::Default);
        assert_eq!(files[1], Choice::File(format!("{FOLDER}2.png")));
        assert_eq!(files[3], Choice::File(format!("{FOLDER}4.png")));
        assert_eq!(files[4], Choice::File(format!("{FOLDER}extra.png")));
        let weights: Vec<f32> = rule.choices.iter().map(|(_, weight)| *weight).collect();
        assert_eq!(weights, [5.0, 1.0, 1.0, 1.0, 2.0]);
    }

    #[test]
    fn a_repeat_rule_needs_a_pattern_its_tiles_fill_exactly() {
        let text =
            "method=repeat\nmatchTiles=oak_planks\nwidth=2\nheight=2\ntiles=1-4\nweights=9 9 9 9\n";
        let rule = parse(FOLDER, text).unwrap();
        assert_eq!(rule.repeat, Some((2, 2)));
        assert_eq!(rule.choices.len(), 4);
        assert!(rule.choices.iter().all(|(_, weight)| *weight == 1.0));
        let short = "method=repeat\nmatchTiles=oak_planks\nwidth=2\nheight=2\ntiles=1-3\n";
        assert!(parse(FOLDER, short).is_none());
        let no_size = "method=repeat\nmatchTiles=oak_planks\ntiles=1-4\n";
        assert!(parse(FOLDER, no_size).is_none());
    }

    #[test]
    fn missing_weights_are_one_each() {
        let rule = parse(FOLDER, "method=random\nmatchTiles=dirt\ntiles=1-3").unwrap();
        assert!(rule.choices.iter().all(|(_, weight)| *weight == 1.0));
        assert_eq!(rule.choices.len(), 3);
    }

    #[test]
    fn tile_paths_resolve_relative_to_the_rule_or_the_pack() {
        assert_eq!(tile_path(FOLDER, "./a"), format!("{FOLDER}a.png"));
        assert_eq!(
            tile_path(FOLDER, "~/ctm/x/b.png"),
            "assets/minecraft/ctm/x/b.png"
        );
        assert_eq!(
            tile_path(FOLDER, "textures/block/c"),
            "assets/minecraft/textures/block/c.png"
        );
    }

    #[test]
    fn rules_that_depend_on_more_than_the_position_are_not_translated() {
        for extra in [
            "biomes=plains",
            "heights=0-64",
            "metadata=1",
            "connect=block",
        ] {
            let text = format!("method=random\nmatchTiles=dirt\ntiles=1-2\n{extra}\n");
            assert!(parse(FOLDER, &text).is_none(), "{extra}");
        }
        assert!(parse(FOLDER, "method=ctm\nmatchTiles=dirt\ntiles=1-2").is_none());
        assert!(parse(FOLDER, "method=random\nmatchBlocks=dirt\ntiles=1-2").is_none());
        assert!(parse(FOLDER, "method=random\nmatchTiles=dirt").is_none());
    }
}
