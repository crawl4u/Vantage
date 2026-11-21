//! Vanilla blocks redrawn with the Java block models a translated pack carries.

use std::{collections::HashSet, sync::Arc};

use protocol::{CustomMaterialInstance, CustomTransformation, CustomVisualComponents};
use protocol_ids::BlockRenderMethod;
use resource_pack::LayeredPackView;
use serde_json::Value;

use super::{Builder, geometry_inputs::geometry_catalog};

/// The largest bindings document read.
const MAX_BINDINGS_BYTES: u64 = 8 << 20;

/// One Java block state's model.
struct Entry {
    block: String,
    properties: Vec<(String, String)>,
    geometry: Arc<str>,
    rotation: [i32; 3],
    materials: Vec<(Arc<str>, Arc<str>)>,
}

/// Reads the translated pack's model bindings; the highest layer that holds them wins.
fn entries(view: &LayeredPackView) -> Vec<Entry> {
    let Some(bytes) = view
        .read_layers_capped(
            java_resource_pack::block_models::BINDINGS,
            MAX_BINDINGS_BYTES,
        )
        .next_back()
    else {
        return Vec::new();
    };
    let Ok(document) = serde_json::from_slice::<Value>(&bytes) else {
        return Vec::new();
    };
    let Some(list) = document.get("blocks").and_then(Value::as_array) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|entry| {
            let rotation = entry.get("rotation")?.as_array()?;
            let turn = |index: usize| i32::try_from(rotation.get(index)?.as_i64()?).ok();
            Some(Entry {
                block: entry.get("block")?.as_str()?.to_owned(),
                properties: entry
                    .get("properties")?
                    .as_object()?
                    .iter()
                    .map(|(name, value)| Some((name.clone(), value.as_str()?.to_owned())))
                    .collect::<Option<_>>()?,
                geometry: entry.get("geometry")?.as_str()?.into(),
                rotation: [turn(0)?, turn(1)?, turn(2)?],
                materials: entry
                    .get("materials")?
                    .as_object()?
                    .iter()
                    .filter_map(|(name, key)| Some((name.as_str().into(), key.as_str()?.into())))
                    .collect(),
            })
        })
        .collect()
}

/// Whether the Java state `java` is the one `entry` names; properties the entry leaves out
/// never change the model.
fn matches(entry: &Entry, java: assets::facts::java_mapping::JavaBlockState<'_>) -> bool {
    entry.block == java.name()
        && entry
            .properties
            .iter()
            .all(|(name, value)| java.property(name) == Some(value.as_str()))
}

impl Builder<'_> {
    /// Redraws every vanilla block state a pack's model bindings name with that Java model.
    pub(super) fn bind_java_models(&mut self, view: &LayeredPackView) {
        let entries = entries(view);
        let Some(base) = self.base_assets.filter(|_| !entries.is_empty()) else {
            return;
        };
        let wanted: HashSet<&str> = entries.iter().map(|entry| &*entry.geometry).collect();
        let geometries = geometry_catalog(view, &wanted);
        let Ok(records) = assets::read_registry_for_protocol(
            assets::pinned_block_registry_bytes(),
            assets::active_content_registry_protocol(),
        ) else {
            return;
        };
        // States drawing the same model with the same turn share one template.
        let mut templates: std::collections::HashMap<(usize, usize), Option<u32>> =
            std::collections::HashMap::new();
        for (visual, record) in records.iter().enumerate() {
            // Every Bedrock state takes the Java state the game's mapping gives it, so Java names
            // and properties need no table of their own. The last matching entry wins, as the
            // pack's own order decides.
            let Some(java) = assets::facts::java_mapping::java_block_state(record.sequential_id)
            else {
                continue;
            };
            let Some(index) = entries.iter().rposition(|entry| matches(entry, java)) else {
                continue;
            };
            let entry = &entries[index];
            // A block with special meshing behaviour keeps its vanilla shape; an override it
            // cannot take would fail the whole overlay.
            if !base
                .accepts_plain_model(base.resolve(assets::NetworkIdMode::Sequential, visual as u32))
            {
                continue;
            }
            let Some(geometry) = geometries.get(&*entry.geometry) else {
                continue;
            };
            let key = (
                entries
                    .iter()
                    .position(|other| {
                        other.geometry == entry.geometry && other.rotation == entry.rotation
                    })
                    .unwrap_or(index),
                0,
            );
            let template = *templates.entry(key).or_insert_with(|| {
                let components = CustomVisualComponents {
                    geometry: Some(entry.geometry.clone()),
                    materials: Some(
                        entry
                            .materials
                            .iter()
                            .map(|(name, texture)| {
                                let mut material = CustomMaterialInstance::with_authored_defaults(
                                    name.clone(),
                                    texture.clone(),
                                );
                                material.render_method = BlockRenderMethod::AlphaTest;
                                material
                            })
                            .collect(),
                    ),
                    transformation: Some(CustomTransformation {
                        rotation: entry.rotation,
                        scale: [1.0; 3],
                        translation: [0.0; 3],
                    }),
                    ..CustomVisualComponents::default()
                };
                let built = self.model(&components, geometry, &[]);
                (built.kind == assets::VisualKind::Model).then_some(built.model_template)
            });
            if let Some(template) = template {
                self.overlay
                    .template_overrides
                    .push((visual as u32, template));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A block whose Java name differs from its Bedrock name is found by the Java name and its
    /// Java properties, with no table of names kept here.
    #[test]
    fn entries_name_java_blocks_and_properties_whatever_the_bedrock_name_is() {
        let records = assets::read_registry_for_protocol(
            assets::pinned_block_registry_bytes(),
            assets::active_content_registry_protocol(),
        )
        .unwrap();
        let (record, java) = records
            .iter()
            .find_map(|record| {
                let java = assets::facts::java_mapping::java_block_state(record.sequential_id)?;
                (java.name() != record.name.as_ref() && java.properties().len() > 0)
                    .then_some((record, java))
            })
            .expect("some Bedrock block is named differently in Java");
        let (property, value) = java.properties().next().unwrap();
        let entry = |value: &str| Entry {
            block: java.name().to_owned(),
            properties: vec![(property.to_owned(), value.to_owned())],
            geometry: "geometry.java.stub".into(),
            rotation: [0; 3],
            materials: Vec::new(),
        };
        assert_ne!(record.name.as_ref(), java.name());
        assert!(matches(&entry(value), java));
        assert!(!matches(&entry("no such value"), java));
    }
}
