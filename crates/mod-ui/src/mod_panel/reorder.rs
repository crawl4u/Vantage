//! Drag-to-reorder for native collections: a tile's `reorder` control is its drag handle.
//!
//! Pressing the handle starts a drag. Moving past a small threshold lifts the tile out of its place
//! and the grid re-lays out under the pointer, the tile following the cell the pointer is over.
//! Releasing reports the tile's new position as one event of the handle's own control: the control's
//! id names the tile and the event's value is the tile's index in the new order. The mod decides what
//! the position means; nothing is saved here.

use super::*;

/// The item role whose control is a tile's drag handle (a button).
pub const HANDLE_ROLE: &str = ui::mod_panel::REORDER_ROLE;

/// Panel units the pointer must travel before a press on a handle becomes a drag, so a click is not one.
const THRESHOLD: f32 = 6.0;

/// A drag in progress.
#[derive(Debug)]
pub(super) struct Drag {
    collection: String,
    /// The tile's index in the collection's items.
    item: usize,
    /// The handle control, whose event reports the drop.
    control: usize,
    /// The collection's order when the drag began, restored if the drag is abandoned.
    original: Vec<usize>,
    start: [f32; 2],
    lifted: bool,
}

/// The drag a press on `control` starts, if `control` is some tile's handle.
pub(super) fn begin(panel: &Panel, control: usize, position: [f32; 2]) -> Option<Drag> {
    if !matches!(panel.controls.get(control), Some(Control::Button { .. })) {
        return None;
    }
    panel.surface.collections.iter().find_map(|(name, collection)| {
        let item = collection
            .definition
            .items
            .iter()
            .position(|item| item.controls.get(HANDLE_ROLE) == Some(&control))?;
        collection.order.contains(&item).then(|| Drag {
            collection: name.clone(),
            item,
            control,
            original: collection.order.clone(),
            start: position,
            lifted: false,
        })
    })
}

impl ModPanel {
    /// Moves a drag with the pointer; releasing the button ends it with the drop's event, if the tile moved.
    pub(super) fn reorder_pointer(&mut self, position: [f32; 2], point: [f64; 2], held: bool) -> Vec<Event> {
        let Some(drag) = self.reorder.as_mut() else {
            return Vec::new();
        };
        if held {
            let travelled = (position[0] - drag.start[0]).hypot(position[1] - drag.start[1]);
            drag.lifted |= travelled >= THRESHOLD;
            if !drag.lifted {
                return Vec::new();
            }
            let target = self.frame.as_ref().and_then(|frame| {
                frame
                    .hits
                    .iter()
                    .filter(|hit| hit.collection.as_deref() == Some(&drag.collection))
                    .find(|hit| {
                        let (x, y) = (point[0], point[1]);
                        x >= hit.rect.x && x < hit.rect.x + hit.rect.w && y >= hit.rect.y && y < hit.rect.y + hit.rect.h
                    })
                    .and_then(|hit| hit.collection_index)
            });
            let Some(target) = target else {
                return Vec::new();
            };
            let Some(collection) = self.panel.surface.collections.get_mut(&drag.collection) else {
                self.reorder = None;
                return Vec::new();
            };
            if move_to(&mut collection.order, drag.item, target) {
                self.refresh_data();
            }
            return Vec::new();
        }
        let Some(drag) = self.reorder.take() else {
            return Vec::new();
        };
        let Some(collection) = self.panel.surface.collections.get_mut(&drag.collection) else {
            return Vec::new();
        };
        let at = collection.order.iter().position(|item| *item == drag.item);
        let from = drag.original.iter().position(|item| *item == drag.item);
        match (at, from) {
            (Some(at), Some(from)) if drag.lifted && at != from => {
                let id = self.panel.controls.get(drag.control).map(|control| control.id().to_owned());
                self.refresh_data();
                id.map(|id| vec![Event { id, value: at as f32 }]).unwrap_or_default()
            }
            _ => {
                collection.order = drag.original;
                self.refresh_data();
                Vec::new()
            }
        }
    }

    /// Abandons a drag, putting the tiles back where they were.
    pub(super) fn cancel_reorder(&mut self) {
        if let Some(drag) = self.reorder.take()
            && let Some(collection) = self.panel.surface.collections.get_mut(&drag.collection)
        {
            collection.order = drag.original;
            self.refresh_data();
        }
    }
}

/// Moves `item` to index `to` of `order`, shifting the tiles between. Returns whether anything moved.
pub(super) fn move_to(order: &mut Vec<usize>, item: usize, to: usize) -> bool {
    let Some(from) = order.iter().position(|i| *i == item) else {
        return false;
    };
    let to = to.min(order.len() - 1);
    if from == to {
        return false;
    }
    let moved = order.remove(from);
    order.insert(to, moved);
    true
}

#[cfg(test)]
mod tests {
    use super::move_to;

    #[test]
    fn a_tile_moves_to_the_target_index_and_the_tiles_between_shift() {
        let mut order = vec![4, 7, 2, 9];
        assert!(move_to(&mut order, 4, 2));
        assert_eq!(order, [7, 2, 4, 9]);
        assert!(move_to(&mut order, 9, 0));
        assert_eq!(order, [9, 7, 2, 4]);
    }

    #[test]
    fn moving_a_tile_onto_itself_or_a_missing_tile_changes_nothing() {
        let mut order = vec![1, 2, 3];
        assert!(!move_to(&mut order, 2, 1));
        assert!(!move_to(&mut order, 8, 0));
        assert_eq!(order, [1, 2, 3]);
    }

    #[test]
    fn a_target_past_the_end_lands_last() {
        let mut order = vec![1, 2, 3];
        assert!(move_to(&mut order, 1, 40));
        assert_eq!(order, [2, 3, 1]);
    }
}
