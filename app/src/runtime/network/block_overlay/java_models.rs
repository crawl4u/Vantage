//! Vanilla blocks redrawn with the Java block models a translated pack carries.

use std::{collections::HashSet, sync::Arc};

use protocol::{CustomMaterialInstance, CustomTransformation, CustomVisualComponents};
use protocol_ids::BlockRenderMethod;
use resource_pack::LayeredPackView;
use serde_json::Value;

use pack_compiler::block_geometry::Geometry;

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

/// A fence or pane: the geometry that always draws and the one each connection mask adds.
struct Connected {
    block: String,
    always: Arc<str>,
    masks: Vec<Arc<str>>,
    materials: Vec<(Arc<str>, Arc<str>)>,
}

/// Reads the translated pack's model bindings; the highest layer that holds them wins.
fn entries(view: &LayeredPackView) -> (Vec<Entry>, Vec<Connected>) {
    let Some(bytes) = view
        .read_layers_capped(
            java_resource_pack::block_models::BINDINGS,
            MAX_BINDINGS_BYTES,
        )
        .next_back()
    else {
        return Default::default();
    };
    let Ok(document) = serde_json::from_slice::<Value>(&bytes) else {
        return Default::default();
    };
    let Some(list) = document.get("blocks").and_then(Value::as_array) else {
        return Default::default();
    };
    let materials = |entry: &Value| -> Option<Vec<(Arc<str>, Arc<str>)>> {
        Some(
            entry
                .get("materials")?
                .as_object()?
                .iter()
                .filter_map(|(name, key)| Some((name.as_str().into(), key.as_str()?.into())))
                .collect(),
        )
    };
    let connected = list
        .iter()
        .filter_map(|entry| {
            let connections = entry.get("connections")?;
            let masks: Vec<Arc<str>> = connections
                .get("masks")?
                .as_array()?
                .iter()
                .filter_map(|mask| Some(mask.as_str()?.into()))
                .collect();
            Some(Connected {
                block: entry.get("block")?.as_str()?.to_owned(),
                always: connections.get("always")?.as_str()?.into(),
                masks: (masks.len() == 16).then_some(masks)?,
                materials: materials(entry)?,
            })
        })
        .collect();
    let variants = list
        .iter()
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
                materials: materials(entry)?,
            })
        })
        .collect();
    (variants, connected)
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
        let (entries, connected) = entries(view);
        let Some(base) = self
            .base_assets
            .filter(|_| !entries.is_empty() || !connected.is_empty())
        else {
            return;
        };
        let wanted: HashSet<&str> = entries
            .iter()
            .map(|entry| &*entry.geometry)
            .chain(
                connected
                    .iter()
                    .flat_map(|entry| std::iter::once(&entry.always).chain(&entry.masks))
                    .map(|geometry| &**geometry),
            )
            .collect();
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
        // Fences and panes of one family and pack entry share one template group.
        let mut groups: std::collections::HashMap<(u32, usize), Option<u32>> =
            std::collections::HashMap::new();
        for (visual, record) in records.iter().enumerate() {
            // Every Bedrock state takes the Java state the game's mapping gives it, so Java names
            // and properties need no table of their own. The last matching entry wins, as the
            // pack's own order decides.
            let Some(java) = assets::facts::java_mapping::java_block_state(record.sequential_id)
            else {
                continue;
            };
            if let Some(group) = connected
                .iter()
                .rposition(|entry| entry.block == java.name())
            {
                self.bind_connected(
                    base,
                    visual as u32,
                    (group, &connected[group]),
                    &geometries,
                    &mut groups,
                );
                continue;
            }
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
                let components = components(&entry.geometry, &entry.materials, entry.rotation);
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

/// The components drawing `geometry` with each variable bound to its terrain key and the model
/// turned about the block centre.
fn components(
    geometry: &Arc<str>,
    materials: &[(Arc<str>, Arc<str>)],
    rotation: [i32; 3],
) -> CustomVisualComponents {
    CustomVisualComponents {
        geometry: Some(geometry.clone()),
        materials: Some(
            materials
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
            rotation,
            scale: [1.0; 3],
            translation: [0.0; 3],
        }),
        ..CustomVisualComponents::default()
    }
}

/// Meshing flags of the vanilla families whose template groups a pack's fence or pane redraws.
const CONNECTED_FAMILY: u32 = assets::MODEL_TEMPLATE_FLAG_PANE
    | assets::MODEL_TEMPLATE_FLAG_FENCE_WOOD
    | assets::MODEL_TEMPLATE_FLAG_FENCE_NETHER;

impl Builder<'_> {
    /// Points a vanilla fence or pane state at a template group drawn from the pack's models.
    fn bind_connected(
        &mut self,
        base: &assets::RuntimeAssets,
        visual: u32,
        (index, entry): (usize, &Connected),
        geometries: &std::collections::HashMap<String, Geometry>,
        groups: &mut std::collections::HashMap<(u32, usize), Option<u32>>,
    ) {
        let block = base.resolve(assets::NetworkIdMode::Sequential, visual);
        let Some(template) = block.model_template() else {
            return;
        };
        let Some(flags) = base
            .model_template_flags(template)
            .filter(|flags| flags & CONNECTED_FAMILY != 0)
        else {
            return;
        };
        let head = *groups
            .entry((template, index))
            .or_insert_with(|| self.connected_group(entry, flags, geometries));
        if let Some(head) = head {
            self.overlay.template_overrides.push((visual, head));
        }
    }

    /// Builds a pane's sixteen mask templates, or a fence's post and sixteen arm templates, and
    /// returns the first. Nothing stays added when a shape cannot be drawn.
    fn connected_group(
        &mut self,
        entry: &Connected,
        flags: u32,
        geometries: &std::collections::HashMap<String, Geometry>,
    ) -> Option<u32> {
        let always = geometries.get(&*entry.always)?;
        let masks = entry
            .masks
            .iter()
            .map(|id| geometries.get(&**id))
            .collect::<Option<Vec<_>>>()?;
        let shapes: Vec<Geometry> = if flags & assets::MODEL_TEMPLATE_FLAG_PANE != 0 {
            masks
                .iter()
                .map(|mask| {
                    let mut shape = always.clone();
                    shape.cubes.extend(mask.cubes.iter().cloned());
                    shape.skipped_cubes += mask.skipped_cubes;
                    shape
                })
                .collect()
        } else {
            std::iter::once(always.clone())
                .chain(masks.into_iter().cloned())
                .collect()
        };
        let family = flags & CONNECTED_FAMILY;
        let first = self.overlay.model_templates.len();
        let quads = self.overlay.model_quads.len();
        let components = components(&entry.always, &entry.materials, [0; 3]);
        for shape in &shapes {
            if !self.push_shape(&components, shape, family) {
                self.overlay.model_templates.truncate(first);
                self.overlay.model_quads.truncate(quads);
                return None;
            }
        }
        Some(first as u32)
    }

    /// Adds one group member: a single template of `family` drawing `shape`, empty when the
    /// shape has no cubes. False when the shape needs more than one template or cannot draw.
    fn push_shape(
        &mut self,
        components: &CustomVisualComponents,
        shape: &Geometry,
        family: u32,
    ) -> bool {
        if shape.cubes.is_empty() {
            self.overlay.model_templates.push(assets::ModelTemplate {
                quad_start: self.overlay.model_quads.len() as u32,
                quad_count: 0,
                flags: family,
            });
            return true;
        }
        let before = self.overlay.model_templates.len();
        let built = self.model(components, shape, &[]);
        if built.kind != assets::VisualKind::Model
            || self.overlay.model_templates.len() != before + 1
        {
            return false;
        }
        self.overlay.model_templates[before].flags = family;
        true
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
