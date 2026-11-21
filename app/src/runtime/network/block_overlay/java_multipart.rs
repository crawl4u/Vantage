//! Vanilla fences, panes, walls and gates redrawn from the parts and models a translated Java
//! pack carries.
//!
//! These blocks are drawn by meshing templates the world picks by neighbours or state, so a pack's
//! shapes go in as the same kinds of template: a pane's sixteen connection shapes, a fence's post
//! and sixteen arms, one shape for each wall state and a chain of parts for each gate.

use std::{collections::HashMap, sync::Arc};

use pack_compiler::block_geometry::Geometry;
use protocol::CustomVisualComponents;
use protocol_ids::BlockRenderMethod;
use serde_json::Value;

use super::{
    Builder,
    java_models::{components, render_method},
};

/// One part of a multipart block: the geometry it draws and the condition it draws under.
struct Part {
    when: Option<Value>,
    geometry: Arc<str>,
}

/// A block built from parts, as the translated pack binds it.
pub(super) struct Multipart {
    pub(super) block: String,
    parts: Vec<Part>,
    materials: Vec<(Arc<str>, Arc<str>)>,
}

/// Reads the multipart entries of a bindings document's block list.
pub(super) fn read(list: &[Value]) -> Vec<Multipart> {
    list.iter()
        .filter_map(|entry| {
            let parts = entry
                .get("parts")?
                .as_array()?
                .iter()
                .map(|part| {
                    Some(Part {
                        when: part.get("when").cloned(),
                        geometry: part.get("geometry")?.as_str()?.into(),
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            let materials = entry
                .get("materials")?
                .as_object()?
                .iter()
                .filter_map(|(name, key)| Some((name.as_str().into(), key.as_str()?.into())))
                .collect();
            Some(Multipart {
                block: entry.get("block")?.as_str()?.to_owned(),
                parts,
                materials,
            })
        })
        .collect()
}

/// Which parts a state draws.
#[derive(Clone, Copy, PartialEq)]
enum Which {
    All,
    Unconditional,
    Conditional,
}

impl Multipart {
    /// Every geometry the parts draw.
    pub(super) fn geometries(&self) -> impl Iterator<Item = &str> {
        self.parts.iter().map(|part| &*part.geometry)
    }

    /// The geometries of the parts `which` names that hold for the state `property` reads.
    fn shown<'s, 'p>(
        &'s self,
        property: &impl Fn(&str) -> Option<&'p str>,
        which: Which,
    ) -> Vec<&'s str> {
        self.parts
            .iter()
            .filter(|part| match (&part.when, which) {
                (None, Which::Conditional) | (Some(_), Which::Unconditional) => false,
                (None, _) => true,
                (Some(when), _) => {
                    java_resource_pack::multipart::holds(when, property) == Some(true)
                }
            })
            .map(|part| &*part.geometry)
            .collect()
    }
}

/// The connections of a pane or fence mask as Java properties.
fn connections(mask: usize) -> impl Fn(&str) -> Option<&'static str> {
    move |name| {
        let bit = ["north", "east", "south", "west"]
            .iter()
            .position(|direction| *direction == name)?;
        Some(if mask >> bit & 1 == 1 {
            "true"
        } else {
            "false"
        })
    }
}

/// The geometries drawn together, one shape; empty when none are drawn.
fn union(geometries: &HashMap<String, Geometry>, ids: &[&str]) -> Option<Geometry> {
    let mut shape = Geometry {
        texture_size: [16.0; 2],
        ..Geometry::default()
    };
    for (index, id) in ids.iter().enumerate() {
        let geometry = geometries.get(*id)?;
        if index == 0 {
            shape.texture_size = geometry.texture_size;
        }
        shape.cubes.extend(geometry.cubes.iter().cloned());
        shape.skipped_cubes += geometry.skipped_cubes;
    }
    Some(shape)
}

/// Meshing flags of the families whose templates a pack's multipart parts redraw: connection
/// groups and walls.
const GROUP_FAMILY: u32 = assets::MODEL_TEMPLATE_FLAG_PANE
    | assets::MODEL_TEMPLATE_FLAG_FENCE_WOOD
    | assets::MODEL_TEMPLATE_FLAG_FENCE_NETHER;

impl Builder<'_> {
    /// Points a vanilla fence, pane or wall state at templates drawn from the pack's parts.
    pub(super) fn bind_multipart(
        &mut self,
        base: &assets::RuntimeAssets,
        visual: u32,
        java: assets::facts::java_mapping::JavaBlockState<'_>,
        (index, entry): (usize, &Multipart),
        geometries: &HashMap<String, Geometry>,
        shapes: &mut HashMap<(u32, usize), Option<u32>>,
    ) {
        let block = base.resolve(assets::NetworkIdMode::Sequential, visual);
        let Some(template) = block.model_template() else {
            return;
        };
        let Some(flags) = base.model_template_flags(template) else {
            return;
        };
        let method = render_method(base, block);
        let head = if flags & GROUP_FAMILY != 0 {
            *shapes
                .entry((template, index))
                .or_insert_with(|| self.connected_group(entry, flags, method, geometries))
        } else if flags == assets::MODEL_TEMPLATE_FLAG_WALL {
            *shapes
                .entry((template, index))
                .or_insert_with(|| self.wall_shape(entry, java, method, geometries))
        } else {
            return;
        };
        if let Some(head) = head {
            self.overlay.template_overrides.push((visual, head));
        }
    }

    /// One wall state's shape: the parts that hold for its Java state, as one template.
    fn wall_shape(
        &mut self,
        entry: &Multipart,
        java: assets::facts::java_mapping::JavaBlockState<'_>,
        method: BlockRenderMethod,
        geometries: &HashMap<String, Geometry>,
    ) -> Option<u32> {
        let ids = entry.shown(&|name| java.property(name), Which::All);
        let shape = union(geometries, &ids)?;
        let components = components(&entry.parts[0].geometry, &entry.materials, [0; 3], method);
        let first = self.overlay.model_templates.len();
        let quads = self.overlay.model_quads.len();
        if self.push_shape(&components, &shape, assets::MODEL_TEMPLATE_FLAG_WALL) {
            Some(first as u32)
        } else {
            self.overlay.model_templates.truncate(first);
            self.overlay.model_quads.truncate(quads);
            None
        }
    }

    /// Builds a pane's sixteen mask templates, or a fence's post and sixteen arm templates, and
    /// returns the first. Nothing stays added when a shape cannot be drawn.
    fn connected_group(
        &mut self,
        entry: &Multipart,
        flags: u32,
        method: BlockRenderMethod,
        geometries: &HashMap<String, Geometry>,
    ) -> Option<u32> {
        let pane = flags & assets::MODEL_TEMPLATE_FLAG_PANE != 0;
        let mut shapes = Vec::with_capacity(17);
        if !pane {
            // A fence's post is whatever draws whatever its neighbours are.
            let post = entry.shown(&|_| None, Which::Unconditional);
            shapes.push(union(geometries, &post)?);
        }
        for mask in 0..16 {
            let state = connections(mask);
            let ids = entry.shown(&state, if pane { Which::All } else { Which::Conditional });
            shapes.push(union(geometries, &ids)?);
        }
        let family = flags & GROUP_FAMILY;
        let first = self.overlay.model_templates.len();
        let quads = self.overlay.model_quads.len();
        let components = components(&entry.parts[0].geometry, &entry.materials, [0; 3], method);
        for shape in &shapes {
            if !self.push_shape(&components, shape, family) {
                self.overlay.model_templates.truncate(first);
                self.overlay.model_quads.truncate(quads);
                return None;
            }
        }
        Some(first as u32)
    }

    /// Adds one template of `family` drawing `shape`, empty when the shape has no cubes. False
    /// when the shape needs more than one template or cannot draw.
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

    /// Redraws the chain of templates from `first` as a fence gate's: at least two parts, each
    /// holding some of the quads, the first carrying the gate's `head` flags. False when the
    /// chain has too few quads to split.
    pub(super) fn as_gate_chain(&mut self, first: usize, head: u32) -> bool {
        let templates = &mut self.overlay.model_templates;
        let Some(start) = templates.get(first).map(|template| template.quad_start) else {
            return false;
        };
        let total: u32 = templates[first..]
            .iter()
            .map(|template| template.quad_count)
            .sum();
        if total < 2 {
            return false;
        }
        let parts = total
            .div_ceil(assets::MAX_MODEL_TEMPLATE_QUADS as u32)
            .max(2);
        let size = total.div_ceil(parts);
        templates.truncate(first);
        let mut taken = 0;
        while taken < total {
            let count = size.min(total - taken);
            let last = taken + count == total;
            let mut flags = if last {
                0
            } else {
                assets::MODEL_TEMPLATE_FLAG_COMPOUND_NEXT
            };
            if taken == 0 {
                flags = head;
            }
            templates.push(assets::ModelTemplate {
                quad_start: start + taken,
                quad_count: count,
                flags,
            });
            taken += count;
        }
        true
    }
}
