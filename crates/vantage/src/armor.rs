//! Worn armour for the Armor status element.
//!
//! This is the only place that asks the SDK about armour: `player-state.read-equipment` gives the
//! worn slots, each with its item, the damage it has taken and (when known) its durability
//! maximum; `facts.read-item` fills in a maximum the item does not carry, once per item.

use crate::hud::Worn;
use crate::host::{facts, player_state};
use std::collections::HashMap;

/// Pieces in the order the element shows them: helmet, chestplate, leggings, boots, held item.
pub const SLOTS: usize = crate::hud::ARMOR_SLOTS;

/// Remembers each item's most durability from the facts, so the host is asked once per item.
#[derive(Default)]
pub struct Reader {
    most: HashMap<String, Option<u32>>,
    /// Whether the selected hotbar item is a bow or crossbow, for the Crosshair's weapon preset.
    pub ranged: bool,
    /// The main inventory (27 slots, top row first; the hotbar is the first nine slots of the
    /// host's inventory), for the Inventory HUD. Without durabilities: icons and counts only.
    pub inventory: Vec<Worn>,
    /// The identifier the Item counter counts; empty while it is off.
    pub count_item: &'static str,
    /// How many of `count_item` are in the inventory and the off hand at the last read.
    pub counted: u32,
    /// Whether to count the healing potions and mushroom stews (the Potion counter is on), and how many there were.
    pub count_potions: bool,
    pub potions: u32,
}

impl Reader {
    /// The four worn pieces and the held item, or none when the host cannot say (not in a world,
    /// or no permission).
    pub fn read(&mut self) -> Option<[Worn; SLOTS]> {
        let equipment = crate::hive_host::counted(|| player_state::read_equipment()).ok().flatten()?;
        let wanted = self.count_item;
        self.counted = if wanted.is_empty() {
            0
        } else {
            equipment
                .inventory
                .iter()
                .chain(std::iter::once(&equipment.offhand))
                .filter_map(|slot| slot.item.as_ref())
                .filter(|item| item.identifier.as_deref() == Some(wanted))
                .map(|item| u32::from(item.count))
                .sum()
        };
        self.potions = if self.count_potions {
            equipment
                .inventory
                .iter()
                .chain(std::iter::once(&equipment.offhand))
                .filter_map(|slot| slot.item.as_ref())
                .filter(|item| is_healing(item.identifier.as_deref(), u16::try_from(item.metadata).unwrap_or(0)))
                .map(|item| u32::from(item.count))
                .sum()
        } else {
            0
        };
        self.inventory = equipment
            .inventory
            .iter()
            .skip(HOTBAR)
            .take(crate::hud::INV_SLOTS)
            .map(|slot| slot.item.as_ref().map_or_else(Worn::default, plain_piece))
            .collect();
        let mut out: [Worn; SLOTS] = Default::default();
        for (slot, worn) in equipment.armor.iter().take(4).enumerate() {
            if let Some(item) = worn.item.as_ref() {
                out[slot] = self.worn(item);
            }
        }
        // The item in your hand: the selected hotbar slot.
        let held = equipment
            .selected_slot
            .and_then(|slot| equipment.inventory.get(usize::from(slot)))
            .and_then(|slot| slot.item.as_ref());
        self.ranged = held.and_then(|item| item.identifier.as_deref()).is_some_and(crate::crosshair::is_ranged);
        if let Some(item) = held {
            out[4] = self.worn(item);
        }
        Some(out)
    }

    fn worn(&mut self, item: &player_state::Item) -> Worn {
        let Some(identifier) = item.identifier.as_deref().and_then(clean) else { return Worn::default() };
        let most = match item.max_durability.filter(|most| *most > 0) {
            Some(most) => Some(most),
            None => *self.most.entry(identifier.clone()).or_insert_with(|| facts_most(&identifier)),
        };
        let damage = item.damage.unwrap_or(0);
        Worn {
            durability: most.map(|most| (most.saturating_sub(damage), most)),
            metadata: u16::try_from(item.metadata).unwrap_or(0),
            count: item.count,
            item: identifier,
        }
    }

    /// Forgets what was learned about items.
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.most.clear();
    }
}

/// A splash potion of Healing (data 21) or Healing II (22), or a mushroom stew: what a Pot PvP player counts.
fn is_healing(identifier: Option<&str>, metadata: u16) -> bool {
    match identifier {
        Some("minecraft:mushroom_stew") => true,
        Some("minecraft:splash_potion") => matches!(metadata, 21 | 22),
        _ => false,
    }
}

/// The slots of the host's inventory before the main inventory starts: the hotbar.
const HOTBAR: usize = 9;

/// An item for the Inventory HUD: its icon and its count, without asking the facts for durability.
fn plain_piece(item: &player_state::Item) -> Worn {
    let Some(identifier) = item.identifier.as_deref().and_then(clean) else { return Worn::default() };
    Worn { item: identifier, metadata: u16::try_from(item.metadata).unwrap_or(0), durability: None, count: item.count }
}

/// An identifier that is safe to put in the HUD document: letters, digits and `_ : . / -`.
fn clean(identifier: &str) -> Option<String> {
    let ok = !identifier.is_empty()
        && identifier.len() <= 96
        && identifier.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '.' | '/' | '-'));
    ok.then(|| identifier.to_owned())
}

/// The most durability the facts know for an item; none for items that do not wear out.
fn facts_most(identifier: &str) -> Option<u32> {
    let item = facts::read_item(identifier, 0).ok().flatten()?;
    item.max_durability.filter(|most| *most > 0)
}
