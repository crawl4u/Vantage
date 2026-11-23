//! Hands the latest scene from the menu code to the painter when both run in one process (the native client),
//! with no serialisation. Where they do not share a process (the wasm build) nothing is published and the
//! painter draws nothing for a scene node.

use std::sync::{Arc, Mutex};

use crate::scene::Scene;

static LATEST: Mutex<Option<Arc<Scene>>> = Mutex::new(None);

/// Makes `scene` the one the painter draws from now on.
pub fn publish(scene: Scene) {
    if let Ok(mut latest) = LATEST.lock() {
        *latest = Some(Arc::new(scene));
    }
}

/// The scene most recently published, if any.
#[must_use]
pub fn latest() -> Option<Arc<Scene>> {
    LATEST.lock().ok().and_then(|latest| latest.clone())
}

/// Forgets the published scene.
pub fn clear() {
    if let Ok(mut latest) = LATEST.lock() {
        *latest = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Primitive;

    #[test]
    fn the_latest_scene_replaces_the_one_before_and_clearing_forgets_it() {
        clear();
        assert!(latest().is_none());
        let mut first = Scene::new();
        first.push(Primitive::PopOpacity);
        publish(first);
        assert_eq!(latest().unwrap().primitives.len(), 1);
        publish(Scene::new());
        assert!(latest().unwrap().primitives.is_empty());
        clear();
        assert!(latest().is_none());
    }
}
