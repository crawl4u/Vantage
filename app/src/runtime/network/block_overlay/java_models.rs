//! Vanilla blocks redrawn with the Java block models a translated pack carries.

use std::{collections::HashSet, sync::Arc};

use protocol::{CustomMaterialInstance, CustomTransformation, CustomVisualComponents};
use protocol_ids::BlockRenderMethod;
use resource_pack::LayeredPackView;
use serde_json::{Map, Value};

use super::{Builder, geometry_inputs::geometry_catalog};

/// The largest bindings document read.
const MAX_BINDINGS_BYTES: u64 = 8 << 20;

/// One vanilla block state's Java model.
struct Entry {
    block: String,
    states: Map<String, Value>,
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
                states: entry.get("states")?.as_object()?.clone(),
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

/// Whether the vanilla `record` is in the state `entry` names.
fn matches(entry: &Entry, record: &assets::RegistryRecord) -> bool {
    entry.states.iter().all(|(name, expected)| {
        let Some(value) = assets::facts::BlockProperty::from_name(name)
            .and_then(|property| record.property(property))
        else {
            return false;
        };
        match (value, expected) {
            (assets::facts::StateValue::String(value), Value::String(expected)) => {
                value.as_ref() == expected
            }
            (assets::facts::StateValue::Int(value), Value::Number(expected)) => {
                expected.as_i64() == Some(i64::from(*value))
            }
            (assets::facts::StateValue::Bool(value), Value::Bool(expected)) => value == expected,
            _ => false,
        }
    })
}

impl Builder<'_> {
    /// Redraws every vanilla block state a pack's model bindings name with that Java model.
    pub(super) fn bind_java_models(&mut self, view: &LayeredPackView) {
        let entries = entries(view);
        if entries.is_empty() || self.base_assets.is_none() {
            return;
        }
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
            // The last matching entry wins, as the pack's own order decides.
            let Some(index) = entries
                .iter()
                .rposition(|entry| entry.block == record.name.as_ref() && matches(entry, record))
            else {
                continue;
            };
            let entry = &entries[index];
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
