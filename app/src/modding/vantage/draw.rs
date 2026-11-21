//! Native Vantage's world drawing: its geometry and instances live in a retained store of its
//! own, its entity highlights in a mask store, and the committed result is handed to the scene
//! renderer beside every other drawing source.

use bevy::prelude::Resource;
use mod_api::drawing::{DrawingHandle, DrawingPermissions, DrawingSession, MaskDepth, MaskStyle};
use mod_render::{
    RenderOutput,
    masks::{MaskError, MaskStore},
    retained::{self as domain, Store},
};
use std::sync::Arc;
use vantage::host::{drawing as wit, masks as wit_masks};

/// The committed drawing, shared with the scene publisher.
#[derive(Clone, Default, Resource)]
pub(crate) struct NativeRenderOutput {
    pub(crate) output: RenderOutput,
    pub(crate) generation: u64,
}

/// Retained geometry, instances and masks owned by native Vantage.
pub(super) struct NativeDrawing {
    store: Store,
    masks: MaskStore,
    session: DrawingSession,
    output: RenderOutput,
    generation: u64,
    changed: bool,
    /// Calls this frame: geometry, create, update, group, remove.
    calls: [u32; 5],
    errors_logged: u32,
}

impl Default for NativeDrawing {
    fn default() -> Self {
        Self {
            store: Store::default(),
            masks: MaskStore::default(),
            session: DrawingSession::default(),
            output: RenderOutput::default(),
            generation: 0,
            changed: false,
            calls: [0; 5],
            errors_logged: 0,
        }
    }
}

impl NativeDrawing {
    /// What this connection and dimension permit; nothing outside a world.
    pub(super) fn permissions(&self) -> DrawingPermissions {
        self.session.permissions
    }

    /// Follows the connected world; leaving it releases every handle and drawing.
    pub(super) fn bind(&mut self, session: DrawingSession) {
        if self.session == session {
            return;
        }
        self.session = session;
        self.store.revoke();
        self.store
            .bind_session(session.connection, session.dimension);
        self.masks.revoke();
        self.masks.bind(session);
        self.output = RenderOutput::default();
        self.generation = mod_render::next_owner();
        self.changed = true;
    }

    /// Supplies the nearby actors drawings may anchor to, and retires those that left.
    pub(super) fn reconcile_entities(
        &mut self,
        candidates: Vec<domain::EntityKey>,
        gone: impl Fn(&domain::EntityKey) -> bool,
    ) {
        let removed: Vec<_> = self
            .store
            .entity_keys()
            .into_iter()
            .filter(|key| gone(key))
            .collect();
        let candidates = if self.session.permissions.entity_observe {
            candidates
        } else {
            Vec::new()
        };
        let _ = self.store.reconcile_entities(candidates, &removed);
    }

    /// Supplies the actors masks may select, with their render cost.
    pub(super) fn publish_mask_targets(&mut self, targets: &[(mod_api::drawing::MaskEntity, u64)]) {
        self.masks.publish_targets(targets);
    }

    /// Starts one frame's calls.
    pub(super) fn begin(&mut self) {
        self.calls = [0; 5];
        self.store.begin();
        self.masks.begin();
    }

    /// Publishes what the frame drew, bumping the generation only when something changed.
    pub(super) fn finish(&mut self) -> Option<NativeRenderOutput> {
        self.store.commit();
        let masks = self.masks.commit();
        let retained = if self.store.snapshot().instances.is_empty() {
            Vec::new()
        } else {
            vec![Arc::clone(self.store.snapshot())]
        };
        let same = retained.len() == self.output.retained.len()
            && retained
                .iter()
                .zip(&self.output.retained)
                .all(|(a, b)| Arc::ptr_eq(a, b))
            && Arc::ptr_eq(&masks, &self.output.masks);
        if same && !self.changed {
            return None;
        }
        self.changed = false;
        self.output.retained = retained;
        self.output.masks = masks;
        self.generation = mod_render::next_owner();
        Some(NativeRenderOutput {
            output: self.output.clone(),
            generation: self.generation,
        })
    }

    /// Counts one call and logs the first few refusals, which would otherwise be silent.
    fn note<T>(
        &mut self,
        which: usize,
        what: &str,
        result: Result<T, wit::DrawError>,
    ) -> Result<T, wit::DrawError> {
        self.calls[which] += 1;
        if let Err(error) = &result
            && self.errors_logged < 16
        {
            self.errors_logged += 1;
            bevy::log::info!(
                "VANTAGE_DRAW {what} refused: {:?} {}",
                error.code as u8,
                error.detail
            );
        }
        result
    }

    fn allowed(&self) -> Result<DrawingPermissions, wit::DrawError> {
        let permissions = self.session.permissions;
        if permissions.world_draw {
            Ok(permissions)
        } else {
            Err(error(domain::DrawError {
                code: domain::ErrorCode::Denied,
                detail: "world drawing",
            }))
        }
    }
}

fn handle(handle: wit::Handle) -> DrawingHandle {
    DrawingHandle {
        owner: handle.owner,
        generation: handle.generation,
        slot: handle.slot,
    }
}

fn token(handle: DrawingHandle) -> wit::Handle {
    wit::Handle {
        owner: handle.owner,
        generation: handle.generation,
        slot: handle.slot,
    }
}

fn error(error: domain::DrawError) -> wit::DrawError {
    wit::DrawError {
        detail: error.detail.into(),
        code: match error.code {
            domain::ErrorCode::Denied => wit::ErrorCode::Denied,
            domain::ErrorCode::Invalid => wit::ErrorCode::Invalid,
            domain::ErrorCode::BudgetExceeded => wit::ErrorCode::BudgetExceeded,
            domain::ErrorCode::StaleHandle => wit::ErrorCode::StaleHandle,
            domain::ErrorCode::Unsupported => wit::ErrorCode::Unsupported,
            domain::ErrorCode::InUse => wit::ErrorCode::InUse,
        },
    }
}

fn mask_error(value: MaskError) -> wit::DrawError {
    let (code, detail) = match value {
        MaskError::Invalid => (wit::ErrorCode::Invalid, "invalid mask appearance"),
        MaskError::Denied => (wit::ErrorCode::Denied, "entity mask authority denied"),
        MaskError::StaleHandle => (
            wit::ErrorCode::StaleHandle,
            "unknown owner or actor incarnation",
        ),
        MaskError::BudgetExceeded => (
            wit::ErrorCode::BudgetExceeded,
            "entity mask selection budget exceeded",
        ),
    };
    wit::DrawError {
        code,
        detail: detail.into(),
    }
}

fn vertex(v: &wit::Vertex) -> domain::Vertex {
    domain::Vertex {
        position: v.position.into(),
        color: v.color.into(),
        uv: v.uv.into(),
        data: v.data.into(),
    }
}

fn instance(spec: &wit::Instance) -> domain::Instance {
    domain::Instance {
        geometry: handle(spec.geometry),
        material: spec.material.map(handle),
        clip: spec.clip.map(handle),
        params: std::array::from_fn(|index| spec.params.get(index).copied().unwrap_or(0.)),
        anchor: match &spec.anchor {
            wit::Anchor::World(anchor) => domain::Anchor::World {
                dimension: anchor.dimension,
                origin: anchor.origin.into(),
            },
            wit::Anchor::Block(anchor) => domain::Anchor::Block {
                dimension: anchor.dimension,
                position: anchor.position.into(),
            },
            wit::Anchor::Camera => domain::Anchor::Camera,
            wit::Anchor::Entity(anchor) => domain::Anchor::Entity(domain::EntityAnchor {
                entity: handle(anchor.entity),
                body_rotation: anchor.body_rotation,
            }),
        },
        transform: domain::Transform {
            translation: spec.transform.translation.into(),
            rotation: spec.transform.rotation.into(),
            scale: spec.transform.scale.into(),
        },
        tint: spec.tint.into(),
        depth: match spec.depth {
            wit::Depth::Test => domain::Depth::Test,
            wit::Depth::Always => domain::Depth::Always,
        },
        blend: match spec.blend {
            wit::Blend::Premultiplied => domain::Blend::Premultiplied,
            wit::Blend::Additive => domain::Blend::Additive,
        },
        width: spec.width,
        width_mode: match spec.width_mode {
            wit::WidthMode::World => domain::WidthMode::World,
            wit::WidthMode::Pixels => domain::WidthMode::Pixels,
        },
        join: match spec.join {
            wit::Join::Bevel => domain::Join::Bevel,
            wit::Join::Miter => domain::Join::Miter,
            wit::Join::Round => domain::Join::Round,
        },
        cap: match spec.cap {
            wit::Cap::Butt => domain::Cap::Butt,
            wit::Cap::Round => domain::Cap::Round,
        },
    }
}

impl wit::Api for NativeDrawing {
    fn capabilities(&mut self) -> wit::Permissions {
        let p = self.session.permissions;
        let mut result = wit::Permissions::empty();
        for (enabled, permission) in [
            (p.world_draw, wit::Permissions::WORLD_DRAW),
            (p.entity_observe, wit::Permissions::ENTITY_OBSERVE),
            (p.entity_highlight, wit::Permissions::ENTITY_HIGHLIGHT),
            (
                p.render_through_walls,
                wit::Permissions::RENDER_THROUGH_WALLS,
            ),
            (p.render_material, wit::Permissions::RENDER_MATERIAL),
            (p.render_post, wit::Permissions::RENDER_POST),
        ] {
            if enabled {
                result |= permission;
            }
        }
        result
    }

    fn create_geometry(&mut self, spec: &wit::GeometrySpec) -> Result<wit::Handle, wit::DrawError> {
        self.allowed()?;
        let spec = match spec {
            wit::GeometrySpec::Triangles(mesh) => domain::GeometrySpec::Triangles {
                vertices: mesh.vertices.iter().map(vertex).collect(),
                indices: mesh.indices.clone(),
            },
            wit::GeometrySpec::Line(line) => domain::GeometrySpec::Polyline {
                vertices: line.vertices.iter().map(vertex).collect(),
                closed: line.closed,
            },
            wit::GeometrySpec::Blocks(_) => {
                return Err(error(domain::DrawError {
                    code: domain::ErrorCode::Unsupported,
                    detail: "block geometry",
                }));
            }
        };
        self.changed = true;
        let result = self.store.create_geometry(spec).map(token).map_err(error);
        self.note(0, "create_geometry", result)
    }

    fn create(&mut self, spec: &wit::Instance) -> Result<wit::Handle, wit::DrawError> {
        let permissions = self.allowed()?;
        self.changed = true;
        let result = self
            .store
            .create(instance(spec), permissions)
            .map(token)
            .map_err(error);
        self.note(1, "create", result)
    }

    fn update(&mut self, id: wit::Handle, spec: &wit::Instance) -> Result<(), wit::DrawError> {
        let permissions = self.allowed()?;
        self.changed = true;
        let result = self
            .store
            .update(handle(id), instance(spec), permissions)
            .map_err(error);
        self.note(2, "update", result)
    }

    fn replace_frame_group(
        &mut self,
        name: &str,
        draws: &[wit::Instance],
    ) -> Result<(), wit::DrawError> {
        let permissions = self.allowed()?;
        self.changed = true;
        let result = self
            .store
            .replace_group(
                name.to_owned(),
                draws.iter().map(instance).collect(),
                permissions,
            )
            .map_err(error);
        self.note(3, "replace_frame_group", result)
    }

    fn remove(&mut self, id: wit::Handle) -> Result<(), wit::DrawError> {
        self.allowed()?;
        self.changed = true;
        let result = self.store.remove(handle(id)).map_err(error);
        self.note(4, "remove", result)
    }

    fn observe_entities(&mut self) -> Result<Vec<wit::ObservedEntity>, wit::DrawError> {
        let permissions = self.allowed()?;
        if !permissions.entity_observe {
            return Err(error(domain::DrawError {
                code: domain::ErrorCode::Denied,
                detail: "unfiltered entity observation",
            }));
        }
        self.store
            .observe_entities()
            .map(|entities| {
                entities
                    .into_iter()
                    .map(|(entity, key)| wit::ObservedEntity {
                        entity: token(entity),
                        runtime_id: key.runtime_id,
                    })
                    .collect()
            })
            .map_err(error)
    }
}

impl wit_masks::Api for NativeDrawing {
    fn create(&mut self, depth: wit_masks::DepthPolicy) -> Result<wit::Handle, wit::DrawError> {
        let depth = match depth {
            wit_masks::DepthPolicy::Test => MaskDepth::Test,
            wit_masks::DepthPolicy::Always => MaskDepth::Always,
        };
        self.changed = true;
        self.masks.create(depth).map(token).map_err(mask_error)
    }

    fn entity(&mut self, runtime_id: u64) -> Result<wit::Handle, wit::DrawError> {
        self.masks.entity(runtime_id).map(token).map_err(mask_error)
    }

    fn select(
        &mut self,
        mask: wit::Handle,
        entities: &[wit::Handle],
    ) -> Result<(), wit::DrawError> {
        self.changed = true;
        self.masks
            .select(
                handle(mask),
                &entities.iter().copied().map(handle).collect::<Vec<_>>(),
            )
            .map_err(mask_error)
    }

    fn remove(&mut self, mask: wit::Handle) -> Result<(), wit::DrawError> {
        self.changed = true;
        self.masks.remove(handle(mask)).map_err(mask_error)
    }

    fn set_style(
        &mut self,
        mask: wit::Handle,
        appearance: Option<wit_masks::Style>,
    ) -> Result<(), wit::DrawError> {
        let rgba = |value: wit_masks::Rgba| [value.r, value.g, value.b, value.a];
        let style = appearance.map(|value| MaskStyle {
            fill: None,
            outline: rgba(value.outline),
            outline_radius: value.outline_radius,
            glow: rgba(value.glow),
            glow_radius: value.glow_radius,
        });
        self.changed = true;
        self.masks
            .set_style(handle(mask), style)
            .map_err(mask_error)
    }
}
