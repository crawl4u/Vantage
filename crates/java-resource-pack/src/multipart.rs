//! Java `multipart` blockstates as parts the client assembles for each block state.
//!
//! A multipart block draws every part whose `when` condition holds for the block's state: a
//! fence or pane adds an arm for each neighbour it connects to, a wall a post and arms of two
//! heights. [`parts`] reads each part's model with its turn applied and keeps its condition;
//! the client evaluates [`holds`] against the Java state it maps each Bedrock state to, so the
//! same parts serve every block that is built this way.

use crate::java_model::{self, Model};
use serde_json::Value;

/// One part of a multipart block: the model it draws, turned as the blockstate says, and the
/// condition under which it draws (`None` for always).
#[derive(Debug)]
pub(crate) struct Part {
    pub when: Option<Value>,
    pub model: Model,
}

/// Whether a part's `when` holds for a state whose properties `property` reads. Conditions
/// are Java's: property names mapped to `|`-separated values, combined with `OR` and `AND`
/// lists. `None` when a condition is malformed.
pub fn holds<'a>(when: &Value, property: &impl Fn(&str) -> Option<&'a str>) -> Option<bool> {
    let object = when.as_object()?;
    let mut result = true;
    for (key, value) in object {
        let satisfied = match key.as_str() {
            "OR" => value
                .as_array()?
                .iter()
                .map(|clause| holds(clause, property))
                .collect::<Option<Vec<_>>>()?
                .contains(&true),
            "AND" => value
                .as_array()?
                .iter()
                .map(|clause| holds(clause, property))
                .collect::<Option<Vec<_>>>()?
                .iter()
                .all(|held| *held),
            name => {
                let text = match value {
                    Value::Bool(flag) => flag.to_string(),
                    Value::String(text) => text.clone(),
                    _ => return None,
                };
                // A property the state lacks matches nothing.
                text.split('|').any(|option| property(name) == Some(option))
            }
        };
        result &= satisfied;
    }
    Some(result)
}

/// The parts of a block's `multipart`, each model resolved, turned and flattened. `None` when a
/// model is missing, a turn is not a multiple of 90 degrees, or a condition is malformed.
pub(crate) fn parts(
    multipart: &[Value],
    load: &dyn Fn(&str) -> Option<Value>,
) -> Option<Vec<Part>> {
    multipart
        .iter()
        .map(|part| {
            let apply = match part.get("apply")? {
                Value::Array(list) => list.first()?,
                other => other,
            };
            let turn = |axis: &str| apply.get(axis).and_then(Value::as_i64).unwrap_or(0);
            let model = java_model::resolve(apply.get("model")?.as_str()?, load)?
                .turned(
                    turn("x"),
                    turn("y"),
                    apply
                        .get("uvlock")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                )?
                .flattened()?;
            let when = part.get("when").cloned();
            // A condition must be readable now, so the client never meets a malformed one.
            if let Some(when) = &when {
                holds(when, &|_| None)?;
            }
            Some(Part { when, model })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pack(name: &str) -> Option<Value> {
        match name {
            "block/post" => Some(json!({ "parent": "block/fence_post",
                "textures": { "texture": "block/planks" } })),
            "block/side" => Some(json!({ "parent": "block/fence_side",
                "textures": { "texture": "block/planks" } })),
            _ => None,
        }
    }

    fn fence() -> Vec<Value> {
        let mut parts = vec![json!({ "apply": { "model": "block/post" } })];
        for (turn, direction) in ["north", "east", "south", "west"].iter().enumerate() {
            parts.push(json!({ "apply": { "model": "block/side", "uvlock": true,
                "y": turn * 90 }, "when": { *direction: "true" } }));
        }
        parts
    }

    #[test]
    fn a_fence_has_an_unconditional_post_and_one_turned_arm_for_each_direction() {
        let parts = parts(&fence(), &pack).unwrap();
        assert_eq!(parts.len(), 5);
        assert!(parts[0].when.is_none());
        assert_eq!(parts[0].model.elements.len(), 1);
        // Two bars per arm: the east arm reaches x = 16, the south arm z = 16.
        assert_eq!(parts[2].model.elements.len(), 2);
        assert_eq!(parts[2].model.elements[0].to[0], 16.0);
        assert_eq!(parts[3].model.elements[0].to[2], 16.0);
        assert!(parts[1].model.textures.contains_key("block_planks"));
    }

    #[test]
    fn conditions_support_alternatives_or_and_and_but_nothing_else() {
        let state = |name: &str| match name {
            "north" => Some("true"),
            "east" => Some("low"),
            _ => None,
        };
        let held = |when: Value| holds(&when, &state);
        assert_eq!(held(json!({ "north": "true" })), Some(true));
        assert_eq!(held(json!({ "north": true })), Some(true));
        assert_eq!(held(json!({ "north": "false" })), Some(false));
        assert_eq!(held(json!({ "east": "low|tall" })), Some(true));
        assert_eq!(held(json!({ "east": "tall" })), Some(false));
        assert_eq!(
            held(json!({ "south": "false" })),
            Some(false),
            "an absent property matches nothing"
        );
        assert_eq!(
            held(json!({ "OR": [{ "east": "tall" }, { "north": "true" }] })),
            Some(true)
        );
        assert_eq!(
            held(json!({ "AND": [{ "east": "low" }, { "north": "false" }] })),
            Some(false)
        );
        assert_eq!(held(json!({ "north": 3 })), None);
        assert_eq!(held(json!("north")), None);
    }

    #[test]
    fn a_missing_model_or_a_malformed_condition_leaves_the_block_vanilla() {
        let missing = vec![json!({ "apply": { "model": "block/absent" } })];
        assert!(parts(&missing, &pack).is_none());
        let malformed = vec![json!({ "apply": { "model": "block/post" }, "when": { "north": 3 } })];
        assert!(parts(&malformed, &pack).is_none());
    }
}
