//! Vanilla blocks redrawn with the Java block models a translated pack carries.

use std::{collections::HashSet, sync::Arc};

use protocol::{CustomMaterialInstance, CustomTransformation, CustomVisualComponents};
use protocol_ids::BlockRenderMethod;
use resource_pack::LayeredPackView;
use serde_json::Value;

use pack_compiler::block_geometry::Geometry;

use super::{
    Builder,
    geometry_inputs::geometry_catalog,
    java_multipart::{self, Multipart},
};

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
fn entries(view: &LayeredPackView) -> (Vec<Entry>, Vec<Multipart>) {
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
    let multiparts = java_multipart::read(list);
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
    (variants, multiparts)
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
        let (entries, multiparts) = entries(view);
        let Some(base) = self
            .base_assets
            .filter(|_| !entries.is_empty() || !multiparts.is_empty())
        else {
            return;
        };
        let wanted: HashSet<&str> = entries
            .iter()
            .map(|entry| &*entry.geometry)
            .chain(multiparts.iter().flat_map(Multipart::geometries))
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
        // Fences, panes and walls of one family and pack entry share their templates.
        let mut shapes: std::collections::HashMap<(u32, usize), Option<u32>> =
            std::collections::HashMap::new();
        for (visual, record) in records.iter().enumerate() {
            // Every Bedrock state takes the Java state the game's mapping gives it, so Java names
            // and properties need no table of their own. The last matching entry wins, as the
            // pack's own order decides.
            let Some(java) = assets::facts::java_mapping::java_block_state(record.sequential_id)
            else {
                continue;
            };
            if let Some(group) = multiparts
                .iter()
                .rposition(|entry| entry.block == java.name())
            {
                self.bind_multipart(
                    base,
                    visual as u32,
                    java,
                    (group, &multiparts[group]),
                    &geometries,
                    &mut shapes,
                );
                continue;
            }
            let Some(index) = entries.iter().rposition(|entry| matches(entry, java)) else {
                continue;
            };
            let entry = &entries[index];
            let block = base.resolve(assets::NetworkIdMode::Sequential, visual as u32);
            // A fence gate's chain of parts takes any model, keeping its axis; any other block
            // with special meshing behaviour keeps its vanilla shape, as an override it cannot
            // take would fail the whole overlay.
            let gate_head = block
                .model_template()
                .and_then(|template| base.model_template_flags(template))
                .filter(|flags| {
                    flags
                        & (assets::MODEL_TEMPLATE_FLAG_GATE_AXIS_X
                            | assets::MODEL_TEMPLATE_FLAG_GATE_AXIS_Z)
                        != 0
                });
            if gate_head.is_none() && !base.accepts_plain_model(block) {
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
                gate_head.unwrap_or(0) as usize,
            );
            let template = *templates.entry(key).or_insert_with(|| {
                let method = render_method(
                    base,
                    base.resolve(assets::NetworkIdMode::Sequential, visual as u32),
                );
                let components =
                    components(&entry.geometry, &entry.materials, entry.rotation, method);
                let first = self.overlay.model_templates.len();
                let quads = self.overlay.model_quads.len();
                let built = self.model(&components, geometry, &[]);
                if built.kind != assets::VisualKind::Model {
                    return None;
                }
                match gate_head {
                    Some(head) if !self.as_gate_chain(first, head) => {
                        self.overlay.model_templates.truncate(first);
                        self.overlay.model_quads.truncate(quads);
                        None
                    }
                    _ => Some(built.model_template),
                }
            });
            if let Some(template) = template {
                self.overlay
                    .template_overrides
                    .push((visual as u32, template));
            }
        }
    }
}

/// How a redrawn block's faces blend: with what is behind them where the vanilla block already
/// does (stained glass), and as cut-out art otherwise.
pub(super) fn render_method(
    base: &assets::RuntimeAssets,
    block: assets::ResolvedBlock,
) -> BlockRenderMethod {
    let blends = assets::BlockFace::ALL.into_iter().any(|face| {
        base.material(block.face(face).material_id()).flags & assets::MATERIAL_FLAG_ALPHA_BLEND != 0
    });
    if blends {
        BlockRenderMethod::Blend
    } else {
        BlockRenderMethod::AlphaTest
    }
}

/// Java's blockstate turns in degrees about each axis as Bedrock quarter turns. Java turns
/// clockwise seen from above (`y`: north to east) and downward about the east-west axis
/// (`x`: north to down); a Bedrock quarter turn about an axis is the opposite way round.
fn bedrock_turns(java_degrees: [i32; 3]) -> [i32; 3] {
    java_degrees.map(|degrees| (-degrees / 90).rem_euclid(4))
}

/// The components drawing `geometry` with each variable bound to its terrain key and the model
/// turned about the block centre; `rotation` is Java's turns in degrees.
pub(super) fn components(
    geometry: &Arc<str>,
    materials: &[(Arc<str>, Arc<str>)],
    rotation: [i32; 3],
    method: BlockRenderMethod,
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
                    material.render_method = method;
                    material
                })
                .collect(),
        ),
        transformation: Some(CustomTransformation {
            rotation: bedrock_turns(rotation),
            scale: [1.0; 3],
            translation: [0.0; 3],
        }),
        ..CustomVisualComponents::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where the block transformation takes a point a Java turn takes it, for both axes Java
    /// turns about, so a bound model faces the way the pack's blockstate says.
    #[test]
    fn java_turns_move_points_the_way_java_moves_them() {
        let turn = |degrees: [i32; 3], point: [f32; 3]| {
            CustomTransformation {
                rotation: bedrock_turns(degrees),
                scale: [1.0; 3],
                translation: [0.0; 3],
            }
            .apply(point)
        };
        let (north, east, south, west) = (
            [0.0, 0.0, -8.0],
            [8.0, 0.0, 0.0],
            [0.0, 0.0, 8.0],
            [-8.0, 0.0, 0.0],
        );
        let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4);
        // y turns clockwise seen from above.
        assert!(close(turn([0, 90, 0], north), east));
        assert!(close(turn([0, 180, 0], north), south));
        assert!(close(turn([0, 270, 0], north), west));
        assert!(close(turn([0, 360, 0], north), north));
        // x tips the north face down.
        assert!(close(turn([90, 0, 0], north), [0.0, -8.0, 0.0]));
        assert!(close(turn([270, 0, 0], north), [0.0, 8.0, 0.0]));
    }

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
