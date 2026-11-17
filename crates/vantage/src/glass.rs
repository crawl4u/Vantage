//! Glass styles: the surface rules that make glass frameless, connected or invisible.
//!
//! The game draws a glass face as a three-by-three partition (corners, edges, centre). A rule says which
//! of the nine tiles a face keeps, so hiding the outer ring leaves frameless glass, and keeping an edge
//! only where no glass of the same kind touches it joins neighbouring blocks into one sheet. The
//! textures, collision and breaking are not touched. The rules are pure data here; `guest.rs` hands them
//! to `terrain::set_surface_rules`.

/// The styles on the Glass page, in the order of the dropdown.
/// The numbers are saved in settings and share codes: only ever add to the end.
pub const GLASS_NAMES: [&str; 4] = ["Normal", "Frameless", "Connected", "Clear"];
pub const NORMAL: usize = 0;
pub const FRAMELESS: usize = 1;
pub const CONNECTED: usize = 2;
/// Invisible: no tile of any face is drawn, so no texture pack can show it.
pub const CLEAR: usize = 3;

/// Width of the frame in 1/256 block: one texel (1/16 block), the frame of the game's glass texture.
pub const BORDER: u8 = 16;

/// The centre tile: bit 4 of the partition (bits run along the face's U axis, then its V axis).
const CENTRE: u16 = 1 << 4;

/// The plain glass block the first section applies to. Panes are not full cells and keep their own look.
pub const PLAIN: [&str; 1] = ["minecraft:glass"];

/// The coloured glass blocks the second section applies to: the 16 stained glass colours and tinted glass.
pub const COLOURED: [&str; 17] = [
    "minecraft:white_stained_glass",
    "minecraft:orange_stained_glass",
    "minecraft:magenta_stained_glass",
    "minecraft:light_blue_stained_glass",
    "minecraft:yellow_stained_glass",
    "minecraft:lime_stained_glass",
    "minecraft:pink_stained_glass",
    "minecraft:gray_stained_glass",
    "minecraft:light_gray_stained_glass",
    "minecraft:cyan_stained_glass",
    "minecraft:purple_stained_glass",
    "minecraft:blue_stained_glass",
    "minecraft:brown_stained_glass",
    "minecraft:green_stained_glass",
    "minecraft:red_stained_glass",
    "minecraft:black_stained_glass",
    "minecraft:tinted_glass",
];

/// One neighbour test, face-local: along U, along V and out of the face.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Condition {
    pub offset: (i8, i8, i8),
    pub present: bool,
}

/// Tiles added when every condition holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clause {
    pub conditions: Vec<Condition>,
    pub tiles: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub groups: Vec<Vec<String>>,
    pub border: u8,
    pub clauses: Vec<Clause>,
}

/// The rules for the two sections: plain glass in `plain` style, coloured glass in `coloured` style.
/// Normal gives no rule, which gives the game's own glass back.
pub fn rules(plain: usize, coloured: usize) -> Vec<Rule> {
    let mut all = style_rules(&PLAIN, plain);
    all.extend(style_rules(&COLOURED, coloured));
    all
}

fn style_rules(blocks: &[&str], style: usize) -> Vec<Rule> {
    let names = || blocks.iter().map(|name| (*name).to_owned()).collect::<Vec<_>>();
    match style {
        FRAMELESS => vec![Rule {
            // Every kind in one group: with no conditions the group makes no difference.
            groups: vec![names()],
            border: BORDER,
            clauses: vec![Clause { conditions: Vec::new(), tiles: CENTRE }],
        }],
        CONNECTED => vec![Rule {
            // One group per colour, so a block joins only glass of its own kind.
            groups: names().into_iter().map(|name| vec![name]).collect(),
            border: BORDER,
            clauses: connected_clauses(),
        }],
        // No clause adds a tile, so every face of these blocks is left out. Coverage is the mesh's, not the
        // texture's, so a texture pack cannot bring the glass back.
        CLEAR => vec![Rule { groups: vec![names()], border: BORDER, clauses: Vec::new() }],
        _ => Vec::new(),
    }
}

/// The centre always, a side strip (with the two corners it ends in) where nothing joins that side, and a
/// corner where the diagonal block is missing. A corner stays hidden only when both of its sides and the
/// diagonal are glass, so a flat wall has no lines and an outside corner keeps its frame.
fn connected_clauses() -> Vec<Clause> {
    // Tile bits: 0 1 2 / 3 4 5 / 6 7 8, U across and V down the rows.
    let missing = |offset: (i8, i8, i8), tiles: u16| Clause { conditions: vec![Condition { offset, present: false }], tiles };
    vec![
        Clause { conditions: Vec::new(), tiles: CENTRE },
        missing((-1, 0, 0), 1 | 1 << 3 | 1 << 6),
        missing((1, 0, 0), 1 << 2 | 1 << 5 | 1 << 8),
        missing((0, -1, 0), 1 | 1 << 1 | 1 << 2),
        missing((0, 1, 0), 1 << 6 | 1 << 7 | 1 << 8),
        missing((-1, -1, 0), 1),
        missing((1, -1, 0), 1 << 2),
        missing((-1, 1, 0), 1 << 6),
        missing((1, 1, 0), 1 << 8),
    ]
}

/// A style number from a saved setting or a share code, kept in range.
pub fn clamp(style: usize) -> usize {
    style.min(GLASS_NAMES.len() - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The host's own limits (`mod_api::terrain_surface::validate`).
    fn valid(rules: &[Rule]) -> bool {
        rules.len() <= 64
            && rules.iter().map(|r| r.groups.len()).sum::<usize>() <= 64
            && rules.iter().all(|r| {
                !r.groups.is_empty()
                    && r.groups.iter().all(|g| {
                        !g.is_empty() && g.len() <= 32 && g.iter().all(|n| n.contains(':') && n.len() <= 128)
                    })
                    && (1..=127).contains(&r.border)
                    && r.clauses.len() <= 32
                    && r.clauses.iter().all(|c| {
                        c.tiles & !0x1ff == 0
                            && c.conditions.len() <= 4
                            && c.conditions.iter().all(|d| d.offset != (0, 0, 0))
                    })
            })
    }

    #[test]
    fn normal_gives_the_games_glass_back() {
        assert!(rules(NORMAL, NORMAL).is_empty());
    }

    #[test]
    fn every_style_is_within_the_hosts_limits() {
        for a in 0..GLASS_NAMES.len() {
            for b in 0..GLASS_NAMES.len() {
                assert!(valid(&rules(a, b)), "styles {a} {b}");
            }
        }
    }

    #[test]
    fn frameless_keeps_only_the_centre() {
        let rules = rules(FRAMELESS, NORMAL);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].clauses.len(), 1);
        assert!(rules[0].clauses[0].conditions.is_empty());
        assert_eq!(rules[0].clauses[0].tiles, 0x10);
    }

    /// Which tiles a face keeps, given which of its eight in-plane neighbours are glass.
    fn tiles(clauses: &[Clause], glass: impl Fn((i8, i8)) -> bool) -> u16 {
        clauses
            .iter()
            .filter(|c| c.conditions.iter().all(|d| glass((d.offset.0, d.offset.1)) == d.present))
            .fold(0, |all, c| all | c.tiles)
    }

    #[test]
    fn connected_glass_hides_the_frame_between_neighbours() {
        let clauses = &rules(CONNECTED, NORMAL)[0].clauses;
        // Alone: the whole face, frame included.
        assert_eq!(tiles(clauses, |_| false), 0x1ff);
        // Surrounded: the centre only, so a flat wall shows no lines.
        assert_eq!(tiles(clauses, |_| true), 0x10);
        // Glass to the left only: the left strip goes, the top and bottom strips still run to the corners.
        assert_eq!(tiles(clauses, |o| o == (-1, 0)), 0x1ff & !(1 << 3));
        // Glass on both sides along U but not above or below: both side strips go.
        assert_eq!(tiles(clauses, |o| o.1 == 0), 0x1ff & !(1 << 3 | 1 << 5));
        // Left and top neighbours without the diagonal: the corner between them stays.
        assert_ne!(tiles(clauses, |o| o == (-1, 0) || o == (0, -1)) & 1, 0);
    }

    #[test]
    fn clear_draws_no_tile_at_all() {
        let rules = rules(CLEAR, CLEAR);
        assert_eq!(rules.len(), 2);
        assert!(rules.iter().all(|r| r.clauses.is_empty()));
        assert_eq!(rules[0].groups[0], vec!["minecraft:glass".to_owned()]);
        assert_eq!(rules[1].groups[0].len(), COLOURED.len());
        assert!(rules[1].groups[0].contains(&"minecraft:tinted_glass".to_owned()));
    }

    #[test]
    fn connected_glass_joins_only_its_own_colour() {
        let rules = rules(NORMAL, CONNECTED);
        assert_eq!(rules[0].groups.len(), COLOURED.len());
        assert!(rules[0].groups.iter().all(|g| g.len() == 1));
    }

    #[test]
    fn the_two_sections_are_independent() {
        let rules = rules(CLEAR, NORMAL);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].groups, vec![vec!["minecraft:glass".to_owned()]]);
    }

    #[test]
    fn a_style_number_stays_in_range() {
        assert_eq!(clamp(9), CLEAR);
        assert_eq!(clamp(1), FRAMELESS);
    }
}
