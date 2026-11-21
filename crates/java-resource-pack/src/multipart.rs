//! Java `multipart` blockstates of connected blocks as geometry for each connection mask.
//!
//! Fences and panes are drawn from parts that depend only on which of the four horizontal
//! neighbours they connect to. [`connected`] evaluates a block's parts for every one of those
//! sixteen masks and merges the models, with each part's turn applied, into one model per mask.
//! Blocks whose parts depend on anything else, such as walls, are left to vanilla.

use crate::java_model::{self, Model};
use serde_json::Value;

/// The horizontal neighbours in mask bit order: north is bit 0, then east, south and west.
pub(crate) const DIRECTIONS: [&str; 4] = ["north", "east", "south", "west"];

/// How many masks four neighbours give.
pub(crate) const MASKS: usize = 1 << DIRECTIONS.len();

/// A connected block's geometry: the parts that always draw and the parts each mask adds.
#[derive(Debug)]
pub(crate) struct Connected {
    pub always: Model,
    pub masks: Vec<Model>,
}

/// Whether a part's `when` holds for `mask`; `None` when it names anything but the four
/// connections or values other than `true` and `false`.
fn holds(when: &Value, mask: usize) -> Option<bool> {
    let object = when.as_object()?;
    let mut result = true;
    for (key, value) in object {
        let satisfied = match key.as_str() {
            "OR" => value
                .as_array()?
                .iter()
                .map(|clause| holds(clause, mask))
                .collect::<Option<Vec<_>>>()?
                .contains(&true),
            "AND" => value
                .as_array()?
                .iter()
                .map(|clause| holds(clause, mask))
                .collect::<Option<Vec<_>>>()?
                .iter()
                .all(|held| *held),
            direction => {
                let bit = DIRECTIONS.iter().position(|name| *name == direction)?;
                let connected = mask >> bit & 1 == 1;
                let text = match value {
                    Value::Bool(flag) => flag.to_string(),
                    Value::String(text) => text.clone(),
                    _ => return None,
                };
                text.split('|')
                    .try_fold(false, |any, option| match option {
                        "true" => Some(any || connected),
                        "false" => Some(any || !connected),
                        _ => None,
                    })?
            }
        };
        result &= satisfied;
    }
    Some(result)
}

/// Merges models whose variables already name their textures.
fn merge(models: &[&Model]) -> Model {
    let mut merged = Model::default();
    for model in models {
        merged.textures.extend(model.textures.clone());
        merged.elements.extend(model.elements.clone());
    }
    merged
}

/// Evaluates a block's `multipart` for every connection mask. `None` when a part's condition
/// is not a connection, a model is missing or cannot be turned, or no part is conditional.
pub(crate) fn connected(
    multipart: &[Value],
    load: &dyn Fn(&str) -> Option<Value>,
) -> Option<Connected> {
    let mut always = Vec::new();
    let mut conditional = Vec::new();
    for part in multipart {
        let apply = match part.get("apply")? {
            Value::Array(list) => list.first()?,
            other => other,
        };
        let turn = |axis: &str| apply.get(axis).and_then(Value::as_i64).unwrap_or(0);
        let model = java_model::resolve(apply.get("model")?.as_str()?, load)?
            .turned(turn("x"), turn("y"))?
            .flattened()?;
        match part.get("when") {
            None => always.push(model),
            Some(when) => {
                let held = (0..MASKS)
                    .map(|mask| holds(when, mask))
                    .collect::<Option<Vec<_>>>()?;
                conditional.push((held, model));
            }
        }
    }
    if conditional.is_empty() {
        return None;
    }
    let masks = (0..MASKS)
        .map(|mask| {
            let shown: Vec<&Model> = conditional
                .iter()
                .filter(|(held, _)| held[mask])
                .map(|(_, model)| model)
                .collect();
            merge(&shown)
        })
        .collect();
    Some(Connected {
        always: merge(&always.iter().collect::<Vec<_>>()),
        masks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pack(name: &str) -> Option<Value> {
        let texture = |key: &str| json!({ "textures": { "texture": key } });
        match name {
            "block/post" => Some(json!({ "parent": "block/fence_post",
                "textures": { "texture": "block/planks" } })),
            "block/side" => Some(texture("block/planks")).map(|mut model| {
                model["parent"] = json!("block/fence_side");
                model
            }),
            _ => None,
        }
    }

    fn fence() -> Vec<Value> {
        let mut parts = vec![json!({ "apply": { "model": "block/post" } })];
        for (turn, direction) in DIRECTIONS.iter().enumerate() {
            parts.push(json!({ "apply": { "model": "block/side", "uvlock": true,
                "y": turn * 90 }, "when": { *direction: "true" } }));
        }
        parts
    }

    #[test]
    fn a_fence_draws_its_post_always_and_one_turned_arm_for_each_connection() {
        let fence = connected(&fence(), &pack).unwrap();
        assert_eq!(fence.always.elements.len(), 1);
        assert_eq!(fence.masks.len(), MASKS);
        assert!(fence.masks[0].elements.is_empty());
        // Two bars per arm: one arm east (bit 1) reaches x = 16, south (bit 2) reaches z = 16.
        assert_eq!(fence.masks[0b0010].elements.len(), 2);
        assert_eq!(fence.masks[0b0010].elements[0].to[0], 16.0);
        assert_eq!(fence.masks[0b0100].elements[0].to[2], 16.0);
        assert_eq!(fence.masks[0b1111].elements.len(), 8);
        assert!(fence.masks[1].textures.contains_key("block_planks"));
    }

    #[test]
    fn conditions_support_false_or_and_alternatives_but_nothing_else() {
        let held = |when: Value, mask| holds(&when, mask);
        assert_eq!(held(json!({ "north": "false" }), 0), Some(true));
        assert_eq!(held(json!({ "north": "false" }), 1), Some(false));
        assert_eq!(
            held(json!({ "OR": [{ "east": true }, { "west": true }] }), 2),
            Some(true)
        );
        assert_eq!(
            held(json!({ "AND": [{ "east": true }, { "west": true }] }), 2),
            Some(false)
        );
        assert_eq!(held(json!({ "north": "true|false" }), 0), Some(true));
        assert_eq!(held(json!({ "up": "true" }), 0), None);
        assert_eq!(held(json!({ "north": "low" }), 0), None);
    }

    #[test]
    fn blocks_that_do_not_depend_on_connections_are_not_connected_blocks() {
        let parts = vec![json!({ "apply": { "model": "block/post" } })];
        assert!(connected(&parts, &pack).is_none());
        let wall = vec![json!({ "apply": { "model": "block/post" }, "when": { "up": "true" } })];
        assert!(connected(&wall, &pack).is_none());
    }
}
