//! Template rebinding for base models and cubes redrawn with overlay geometry.

use crate::{
    AssetError, BlockFlags, BlockVisual, MODEL_TEMPLATE_FLAG_POTTED_PLANT, ModelQuad,
    ModelTemplate, VisualKind,
};

/// Points base visuals at overlay templates. A model with special meshing behaviour (a flag on
/// its template) must be replaced by a copy of its complete selectable span (topology group or
/// compound chain) with the same flags and quad counts, and keep a pot-content template directly
/// before it exactly when the base has one, so neighbour-selected offsets and potted contents stay
/// inside the copy. A flag-free model, and a plain cube, may instead be redrawn as any plain
/// model; a cube then loses its occlusion.
pub(super) fn apply(
    replacements: &[(u32, u32)],
    visuals: &mut [BlockVisual],
    base_templates: &[ModelTemplate],
    overlay_templates: &[ModelTemplate],
    overlay_quads: &[ModelQuad],
    template_base: u32,
) -> Result<(), AssetError> {
    let mut seen = std::collections::HashSet::new();
    for &(id, replacement) in replacements {
        if !seen.insert(id) {
            return Err(invalid("base template override is duplicated"));
        }
        let visual = visuals
            .get_mut(id as usize)
            .ok_or_else(|| invalid("base template override visual is out of bounds"))?;
        if visual.kind == VisualKind::Cube
            && visual.model_template == crate::NO_MODEL_TEMPLATE
            && visual.variant == 0
            && !visual.flags.contains(BlockFlags::LEAF_MODEL)
        {
            if !is_plain_chain(overlay_templates, overlay_quads, replacement) {
                return Err(invalid("cube template override is not a plain model"));
            }
            visual.kind = VisualKind::Model;
            visual
                .flags
                .remove(BlockFlags::CUBE_GEOMETRY | BlockFlags::OCCLUDES_FULL_FACE);
            visual.model_template = template_base + replacement;
            continue;
        }
        if !matches!(visual.kind, VisualKind::Model | VisualKind::Cross)
            || visual.model_template == crate::NO_MODEL_TEMPLATE
        {
            return Err(invalid("base template override targets no model visual"));
        }
        let original = visual.model_template;
        // A model with no special meshing behaviour may be redrawn as any plain model.
        if base_templates
            .get(original as usize)
            .is_some_and(|template| template.flags == 0)
            && is_plain_chain(overlay_templates, overlay_quads, replacement)
        {
            visual.model_template = template_base + replacement;
            continue;
        }
        // A fence gate's chain splits its quads across as many parts as they need, so a redrawn
        // gate may run to another length: its first part keeps the gate's axis, and what follows
        // is checked as any compound chain is.
        let gate_axes = crate::model::MODEL_TEMPLATE_FLAG_GATE_AXIS_X
            | crate::model::MODEL_TEMPLATE_FLAG_GATE_AXIS_Z;
        let original_flags = base_templates[original as usize].flags;
        if original_flags & gate_axes != 0 {
            let chain = crate::model_template_parts(overlay_templates, replacement);
            if chain.is_none_or(|chain| {
                chain[0].flags != original_flags
                    || chain.iter().any(|part| part.quad_count == 0)
                    || chain.len() < 2
            }) {
                return Err(invalid("base template override is not a gate chain"));
            }
            visual.model_template = template_base + replacement;
            continue;
        }
        let span = crate::model_template_span(base_templates, original)
            .ok_or_else(|| invalid("base template override targets an unbounded template"))?;
        let copied = overlay_templates
            .get(replacement as usize..)
            .and_then(|tail| tail.get(..span as usize));
        let source = &base_templates[original as usize..][..span as usize];
        let potted = |templates: &[ModelTemplate], index: u32| {
            index
                .checked_sub(1)
                .and_then(|previous| templates.get(previous as usize))
                .is_some_and(|template| template.flags == MODEL_TEMPLATE_FLAG_POTTED_PLANT)
        };
        // Connection groups and wall shapes are picked by neighbours or state, never by quad
        // position, so a redrawn pane, fence or wall may draw any number of quads in each.
        let authored_shapes = crate::model::MODEL_TEMPLATE_FLAG_PANE
            | crate::model::MODEL_TEMPLATE_FLAG_FENCE_WOOD
            | crate::model::MODEL_TEMPLATE_FLAG_FENCE_NETHER
            | crate::model::MODEL_TEMPLATE_FLAG_WALL;
        if copied.is_none_or(|copied| {
            copied.iter().zip(source).any(|(copy, base)| {
                copy.flags != base.flags
                    || (copy.quad_count != base.quad_count && base.flags & authored_shapes == 0)
            })
        }) || potted(base_templates, original) != potted(overlay_templates, replacement)
        {
            return Err(invalid(
                "base template override does not copy its template span",
            ));
        }
        visual.model_template = template_base + replacement;
    }
    Ok(())
}

/// Whether a visual with these facts may be redrawn as any plain model: a plain cube, or a model
/// whose template carries no special meshing behaviour.
pub(super) fn accepts_plain_model(
    kind: VisualKind,
    flags: BlockFlags,
    variant: u32,
    template: Option<u32>,
    base_templates: &[ModelTemplate],
) -> bool {
    match template {
        None => kind == VisualKind::Cube && variant == 0 && !flags.contains(BlockFlags::LEAF_MODEL),
        Some(template) => {
            matches!(kind, VisualKind::Model | VisualKind::Cross)
                && base_templates
                    .get(template as usize)
                    .is_some_and(|template| template.flags == 0)
        }
    }
}

/// Whether the overlay holds a complete chain of ordinary templates from `first`: none carries a
/// vanilla meshing behaviour, and each owns quads the overlay stores.
fn is_plain_chain(templates: &[ModelTemplate], quads: &[ModelQuad], first: u32) -> bool {
    crate::model_template_parts(templates, first).is_some_and(|parts| {
        parts.iter().all(|part| {
            part.flags & !crate::MODEL_TEMPLATE_FLAG_COMPOUND_NEXT == 0
                && part.quad_count != 0
                && quads
                    .get(part.quad_start as usize..)
                    .is_some_and(|rest| rest.len() >= part.quad_count as usize)
        })
    })
}

/// Names rejected overrides without exposing unchecked runtime references.
fn invalid(detail: &str) -> AssetError {
    AssetError::InvalidCompiledAssets {
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template(flags: u32) -> ModelTemplate {
        ModelTemplate {
            quad_start: 0,
            quad_count: 1,
            flags,
        }
    }

    #[test]
    fn only_plain_cubes_and_flag_free_models_accept_any_plain_model() {
        let templates = [template(0), template(crate::MODEL_TEMPLATE_FLAG_TORCH)];
        let accepts = |kind, flags, variant, index| {
            accepts_plain_model(kind, flags, variant, index, &templates)
        };
        let cube = BlockFlags::CUBE_GEOMETRY;
        assert!(accepts(VisualKind::Cube, cube, 0, None));
        assert!(!accepts(
            VisualKind::Cube,
            cube | BlockFlags::LEAF_MODEL,
            0,
            None
        ));
        assert!(!accepts(VisualKind::Cube, cube, 1, None));
        assert!(accepts(VisualKind::Model, BlockFlags::empty(), 0, Some(0)));
        assert!(!accepts(VisualKind::Model, BlockFlags::empty(), 0, Some(1)));
        assert!(!accepts(VisualKind::Model, BlockFlags::empty(), 0, Some(9)));
    }

    /// A pane or fence group redrawn from pack art may draw other quad counts than vanilla's,
    /// while a group of another family still has to match quad for quad.
    #[test]
    fn connection_groups_may_change_their_quad_counts_but_not_their_flags() {
        let group = |flags: u32, quads: u32| -> Vec<ModelTemplate> {
            (0..16)
                .map(|index| ModelTemplate {
                    quad_start: index * quads,
                    quad_count: quads,
                    flags,
                })
                .collect()
        };
        let visual = || {
            let mut visual =
                BlockVisual::diagnostic(BlockFlags::empty(), crate::ContributorRole::default());
            visual.kind = VisualKind::Model;
            visual.model_template = 0;
            visual
        };
        let apply_with = |base: &[ModelTemplate], overlay: &[ModelTemplate]| {
            let quad = ModelQuad {
                positions: [[0; 3]; 4],
                uvs: [[0; 2]; 4],
                material: 0,
                flags: 0,
            };
            let mut visuals = [visual()];
            let result = apply(
                &[(0, 0)],
                &mut visuals,
                base,
                overlay,
                &vec![quad; 16 * 12],
                base.len() as u32,
            );
            (result, visuals[0].model_template)
        };
        let pane = crate::MODEL_TEMPLATE_FLAG_PANE;
        let (result, template) = apply_with(&group(pane, 6), &group(pane, 3));
        assert!(result.is_ok());
        assert_eq!(template, 16);
        let wall = crate::MODEL_TEMPLATE_FLAG_WALL;
        assert!(apply_with(&group(pane, 6), &group(wall, 6)).0.is_err());
        // Only an overlay's own groups may leave vanilla's counts.
        assert!(crate::blob::compiled_connected_bases(&group(pane, 3), true).is_ok());
        assert!(crate::blob::compiled_connected_bases(&group(pane, 3), false).is_err());
    }

    /// A gate may be redrawn as a chain of another length, and a wall as a shape of another
    /// quad count, while the head keeps the gate's axis.
    #[test]
    fn gates_and_walls_may_change_their_shapes_but_not_their_flags() {
        let part = |quad_start: u32, quad_count: u32, flags: u32| ModelTemplate {
            quad_start,
            quad_count,
            flags,
        };
        let axis =
            crate::MODEL_TEMPLATE_FLAG_COMPOUND_NEXT | crate::MODEL_TEMPLATE_FLAG_GATE_AXIS_X;
        let visual = || {
            let mut visual =
                BlockVisual::diagnostic(BlockFlags::empty(), crate::ContributorRole::default());
            visual.kind = VisualKind::Model;
            visual.model_template = 0;
            visual
        };
        let quad = ModelQuad {
            positions: [[0; 3]; 4],
            uvs: [[0; 2]; 4],
            material: 0,
            flags: 0,
        };
        let redraw = |base: &[ModelTemplate], overlay: &[ModelTemplate]| {
            let mut visuals = [visual()];
            apply(
                &[(0, 0)],
                &mut visuals,
                base,
                overlay,
                &vec![quad; 64],
                base.len() as u32,
            )
        };
        let gate = [part(0, 32, axis), part(32, 4, 0)];
        assert!(
            redraw(
                &gate,
                &[
                    part(0, 3, axis),
                    part(3, 2, crate::MODEL_TEMPLATE_FLAG_COMPOUND_NEXT),
                    part(5, 2, 0)
                ]
            )
            .is_ok()
        );
        assert!(redraw(&gate, &[part(0, 3, axis), part(3, 2, 0)]).is_ok());
        let other_axis =
            crate::MODEL_TEMPLATE_FLAG_COMPOUND_NEXT | crate::MODEL_TEMPLATE_FLAG_GATE_AXIS_Z;
        assert!(redraw(&gate, &[part(0, 3, other_axis), part(3, 2, 0)]).is_err());
        let wall = crate::MODEL_TEMPLATE_FLAG_WALL;
        assert!(redraw(&[part(0, 6, wall)], &[part(0, 30, wall)]).is_ok());
        assert!(redraw(&[part(0, 6, wall)], &[part(0, 30, 0)]).is_err());
    }
}
