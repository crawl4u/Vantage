//! Inventory icons of entity-drawn block items (chests, shulker boxes, banners and the like),
//! drawn from the pack's own entity textures so a pack's art reaches the slot instead of the
//! vanilla icon baked from Bedrock's.

use super::{BlockIcons, MAX_SESSION_ICONS};
use assets::texture::policy::device_policy;
use client_ui::ui_runtime::presentation::SessionIcon;
use pack_runtime::pack_textures::decode_pack_texture_with_policy;
use resource_pack::LayeredPackView;
use std::sync::{Arc, LazyLock};

/// The largest texture side the model bake reads.
const MAX_TEXTURE_SIDE: u32 = 512;

/// Adds an icon for each block item whose entity model the pack can texture; items whose
/// textures the pack lacks keep their vanilla icon.
pub(in crate::runtime::network) fn append(view: &LayeredPackView, icons: &mut BlockIcons) {
    for (identifier, metadata) in super::vanilla_blocks::block_item_keys().chain(coloured_beds()) {
        if icons.icons.len() >= MAX_SESSION_ICONS {
            break;
        }
        let sprite = pack_entity_compiler::block_entity_icon(identifier, metadata, |path| {
            let texture = decode_pack_texture_with_policy(view, path, device_policy(), true)?;
            (texture.width.max(texture.height) <= MAX_TEXTURE_SIDE).then(|| assets::IconSprite {
                width: texture.width as u16,
                height: texture.height as u16,
                rgba8: texture.rgba8.into_vec().into(),
            })
        });
        let Some(sprite) = sprite else { continue };
        icons.icons.push(SessionIcon {
            identifier: std::sync::Arc::clone(identifier),
            metadata,
            width: u32::from(sprite.width),
            height: u32::from(sprite.height),
            rgba8: sprite.rgba8.to_vec().into_boxed_slice(),
        });
    }
}

/// Bedrock's per-colour bed items (`minecraft:red_bed`), which are items of their own rather than
/// the dyed `minecraft:bed` block item.
fn coloured_beds() -> impl Iterator<Item = (&'static Arc<str>, u32)> {
    static BEDS: LazyLock<Vec<Arc<str>>> = LazyLock::new(|| {
        assets::block_entity_geometry::SHULKER_COLORS[1..]
            .iter()
            .map(|color| {
                let stem = if *color == "silver" { "light_gray" } else { color };
                Arc::from(format!("minecraft:{stem}_bed"))
            })
            .collect()
    });
    BEDS.iter().map(|identifier| (identifier, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_coloured_bed_item_gets_a_model_icon() {
        let beds: Vec<_> = coloured_beds().collect();
        assert_eq!(beds.len(), 16);
        for (identifier, metadata) in beds {
            let icon = pack_entity_compiler::block_entity_icon(identifier, metadata, |_| {
                Some(assets::IconSprite {
                    width: 64,
                    height: 64,
                    rgba8: [120, 40, 40, 255].repeat(64 * 64).into(),
                })
            });
            assert!(icon.is_some(), "{identifier}");
        }
    }
}
