//! Dragging packs between the lists of the Java-style Resource Packs screen. A press on a pack
//! captures the pointer; past a few pixels it becomes a drag, and letting go over the selected
//! list puts the pack there, over the list of unused packs takes it out, and anywhere else does
//! nothing. A press released without moving selects the pack, as a click does.

use launcher::{
    global_resources::Action,
    menu::{MenuAction, MenuScreen},
};
use ui::UiPoint;

use crate::paint::Bounds;

/// Pixels the pointer must travel before a press becomes a drag.
const THRESHOLD: f32 = 6.0;

/// One pack drawn in a list, where the pointer can grab it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DragEntry {
    pub active: bool,
    pub index: usize,
    pub bounds: Bounds,
    /// What releasing without a drag does.
    pub select: MenuAction,
    /// Areas of the pack that act on a press of their own, so a press there does not grab it.
    pub hot: [Option<Bounds>; 3],
}

#[derive(Clone, Copy)]
struct Capture {
    entry: DragEntry,
    anchor: UiPoint,
    pointer: UiPoint,
    dragging: bool,
}

/// The lists and packs of the last frame, and the pointer capture over them.
#[derive(Default)]
pub(crate) struct PackDrag {
    /// The visible area of the list of unused packs, then of the selected list.
    lists: [Option<Bounds>; 2],
    entries: Vec<DragEntry>,
    /// The folder rows a pack can be dropped on, with the folder each means: a place in the
    /// open folder's subfolders, or none for the folder above.
    targets: Vec<(Bounds, Option<usize>)>,
    /// How many packs the selected list holds, shown or not.
    active_len: usize,
    capture: Option<Capture>,
    touched: bool,
}

fn contains(bounds: Bounds, point: UiPoint) -> bool {
    point.x() >= bounds[0]
        && point.x() <= bounds[2]
        && point.y() >= bounds[1]
        && point.y() <= bounds[3]
}

impl PackDrag {
    /// Whether a pack is held, so the screen keeps repainting under the pointer.
    pub(crate) fn captured(&self) -> bool {
        self.capture.is_some()
    }

    /// Starts a frame's layout: the screen reports its lists and packs after this.
    pub(crate) fn begin_layout(&mut self, active_len: usize) {
        self.lists = [None; 2];
        self.entries.clear();
        self.targets.clear();
        self.active_len = active_len;
        self.touched = true;
    }

    /// Reports the visible area of a list.
    pub(crate) fn list(&mut self, active: bool, area: Bounds) {
        self.lists[usize::from(active)] = Some(area);
    }

    /// Reports a folder row a pack can be dropped on.
    pub(crate) fn target(&mut self, bounds: Bounds, folder: Option<usize>) {
        self.targets.push((bounds, folder));
    }

    /// The folder the held pack is over, when it came from the list of unused packs.
    pub(crate) fn over_target(&self) -> Option<Option<usize>> {
        let (entry, pointer, _) = self.held()?;
        if entry.active {
            return None;
        }
        self.targets
            .iter()
            .find(|(bounds, _)| contains(*bounds, pointer))
            .map(|(_, folder)| *folder)
    }

    /// Reports a pack the pointer can grab.
    pub(crate) fn entry(&mut self, entry: DragEntry) {
        self.entries.push(entry);
    }

    /// Forgets the layout, and any capture, when the screen was not drawn this frame.
    pub(crate) fn end_frame(&mut self) {
        if !std::mem::take(&mut self.touched) {
            self.cancel();
            self.lists = [None; 2];
            self.entries.clear();
            self.targets.clear();
        }
    }

    pub(crate) fn cancel(&mut self) {
        self.capture = None;
    }

    /// The held pack, where the pointer is, and how far into the pack it was grabbed, while it is
    /// being dragged.
    pub(crate) fn held(&self) -> Option<(DragEntry, UiPoint, [f32; 2])> {
        let capture = self.capture.filter(|capture| capture.dragging)?;
        let grab = [
            capture.anchor.x() - capture.entry.bounds[0],
            capture.anchor.y() - capture.entry.bounds[1],
        ];
        Some((capture.entry, capture.pointer, grab))
    }

    /// Where the held pack would land: a place in the selected list, or none for the other list.
    /// None when it is over neither list.
    fn destination(&self, pointer: UiPoint) -> Option<Option<usize>> {
        if self.lists[0].is_some_and(|area| contains(area, pointer)) {
            return Some(None);
        }
        let area = self.lists[1].filter(|area| contains(*area, pointer))?;
        let _ = area;
        let before = self
            .entries
            .iter()
            .filter(|entry| entry.active)
            .find(|entry| pointer.y() < (entry.bounds[1] + entry.bounds[3]) * 0.5);
        Some(Some(before.map_or(self.active_len, |entry| entry.index)))
    }

    /// The line to draw where the held pack would land in the selected list: its left and right
    /// edges and height. None unless that is where it would land.
    pub(crate) fn marker(&self) -> Option<[f32; 3]> {
        let (_, pointer, _) = self.held()?;
        let place = self.destination(pointer)??;
        let area = self.lists[1]?;
        let y = self
            .entries
            .iter()
            .filter(|entry| entry.active)
            .find(|entry| entry.index == place)
            .map(|entry| entry.bounds[1])
            .or_else(|| {
                self.entries
                    .iter()
                    .filter(|entry| entry.active)
                    .map(|entry| entry.bounds[3])
                    .reduce(f32::max)
            })?;
        Some([area[0], area[2], y])
    }

    /// Follows the pointer: whether a pack is held, and the action a release brings.
    pub(crate) fn pointer(
        &mut self,
        point: Option<UiPoint>,
        held: bool,
        pressed: bool,
    ) -> (bool, Option<MenuAction>) {
        if pressed
            && self.capture.is_none()
            && let Some(point) = point
            && let Some(entry) = self.entries.iter().find(|entry| {
                contains(entry.bounds, point)
                    && !entry
                        .hot
                        .iter()
                        .flatten()
                        .any(|zone| contains(*zone, point))
                    && self.lists[usize::from(entry.active)]
                        .is_some_and(|area| contains(area, point))
            })
        {
            self.capture = Some(Capture {
                entry: *entry,
                anchor: point,
                pointer: point,
                dragging: false,
            });
        }
        let Some(mut capture) = self.capture else {
            return (false, None);
        };
        let Some(point) = point else {
            self.cancel();
            return (true, None);
        };
        capture.pointer = point;
        let travelled =
            (point.x() - capture.anchor.x()).powi(2) + (point.y() - capture.anchor.y()).powi(2);
        capture.dragging |= travelled >= THRESHOLD * THRESHOLD;
        if held {
            self.capture = Some(capture);
            return (true, None);
        }
        let into_folder = self.over_target().map(|folder| {
            MenuAction::GlobalResources(Action::PutInFolder {
                index: capture.entry.index,
                folder,
            })
        });
        let action = if capture.dragging && into_folder.is_some() {
            into_folder
        } else if capture.dragging {
            self.destination(point).and_then(|to| {
                let entry = capture.entry;
                // Putting a pack back where it already is changes nothing.
                let unchanged = match to {
                    None => !entry.active,
                    Some(place) => {
                        entry.active && (place == entry.index || place == entry.index + 1)
                    }
                };
                (!unchanged).then_some(MenuAction::GlobalResources(Action::Drop {
                    from_active: entry.active,
                    index: entry.index,
                    to,
                }))
            })
        } else {
            Some(capture.entry.select)
        };
        self.cancel();
        (true, action)
    }
}

impl super::super::OreUiScreen {
    /// Routes the pointer to the Resource Packs screen's drag; false on any other screen.
    pub(crate) fn pack_drag_pointer(
        &mut self,
        screen: Option<MenuScreen>,
        pointer: Option<UiPoint>,
        (held, pressed): (bool, bool),
    ) -> (bool, Option<MenuAction>) {
        let state = &mut self.transitions.pack_drag;
        if screen != Some(MenuScreen::Settings) {
            state.cancel();
            return (false, None);
        }
        state.pointer(pointer, held, pressed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f32, y: f32) -> UiPoint {
        UiPoint::new(x, y).unwrap()
    }

    /// Two lists side by side, three packs available and three selected, 40 tall each.
    fn layout() -> PackDrag {
        let mut drag = PackDrag::default();
        drag.begin_layout(3);
        drag.list(false, [0.0, 0.0, 100.0, 200.0]);
        drag.list(true, [110.0, 0.0, 210.0, 200.0]);
        for (active, left) in [(false, 0.0), (true, 110.0)] {
            for index in 0..3 {
                let top = index as f32 * 40.0;
                drag.entry(DragEntry {
                    active,
                    index,
                    bounds: [left, top, left + 100.0, top + 36.0],
                    select: MenuAction::GlobalResources(if active {
                        Action::SelectActive(index)
                    } else {
                        Action::SelectAvailable(index)
                    }),
                    hot: [None; 3],
                });
            }
        }
        drag
    }

    /// Presses at `from`, moves to `to` and lets go there.
    fn gesture(drag: &mut PackDrag, from: UiPoint, to: UiPoint) -> Option<MenuAction> {
        assert!(drag.pointer(Some(from), true, true).0);
        assert!(drag.pointer(Some(to), true, false).0);
        drag.pointer(Some(to), false, false).1
    }

    #[test]
    fn a_press_released_in_place_selects_the_pack() {
        let mut drag = layout();
        let action = gesture(&mut drag, point(20.0, 50.0), point(21.0, 51.0));
        assert_eq!(
            action,
            Some(MenuAction::GlobalResources(Action::SelectAvailable(1)))
        );
        assert!(!drag.captured());
    }

    #[test]
    fn a_pack_dragged_into_the_selected_list_lands_before_the_pack_under_it() {
        let mut drag = layout();
        // Released over the upper half of the second selected pack.
        let action = gesture(&mut drag, point(20.0, 10.0), point(150.0, 45.0));
        assert_eq!(
            action,
            Some(MenuAction::GlobalResources(Action::Drop {
                from_active: false,
                index: 0,
                to: Some(1),
            }))
        );
        // Below every pack it goes last.
        let action = gesture(&mut drag, point(20.0, 10.0), point(150.0, 190.0));
        assert_eq!(
            action,
            Some(MenuAction::GlobalResources(Action::Drop {
                from_active: false,
                index: 0,
                to: Some(3),
            }))
        );
    }

    #[test]
    fn a_selected_pack_dragged_out_leaves_the_selection_and_in_place_does_nothing() {
        let mut drag = layout();
        let action = gesture(&mut drag, point(150.0, 90.0), point(50.0, 120.0));
        assert_eq!(
            action,
            Some(MenuAction::GlobalResources(Action::Drop {
                from_active: true,
                index: 2,
                to: None,
            }))
        );
        // Let go back on its own place, or just under it, and nothing changes.
        assert_eq!(
            gesture(&mut drag, point(150.0, 50.0), point(150.0, 60.0)),
            None
        );
        assert_eq!(
            gesture(&mut drag, point(150.0, 50.0), point(150.0, 75.0)),
            None
        );
        // An available pack let go over the available list changes nothing either.
        assert_eq!(
            gesture(&mut drag, point(20.0, 50.0), point(50.0, 150.0)),
            None
        );
    }

    #[test]
    fn a_pack_let_go_outside_both_lists_stays_and_the_marker_follows_the_drop_place() {
        let mut drag = layout();
        assert_eq!(
            gesture(&mut drag, point(20.0, 10.0), point(400.0, 10.0)),
            None
        );
        assert!(drag.pointer(Some(point(20.0, 10.0)), true, true).0);
        assert!(drag.pointer(Some(point(150.0, 45.0)), true, false).0);
        assert_eq!(drag.marker(), Some([110.0, 210.0, 40.0]));
        drag.pointer(Some(point(150.0, 45.0)), false, false);
        assert_eq!(drag.marker(), None);
    }

    #[test]
    fn a_pack_dropped_on_a_folder_row_goes_into_that_folder() {
        let mut drag = layout();
        drag.target([0.0, 300.0, 100.0, 336.0], Some(1));
        drag.target([0.0, 340.0, 100.0, 376.0], None);
        assert_eq!(
            gesture(&mut drag, point(20.0, 50.0), point(50.0, 310.0)),
            Some(MenuAction::GlobalResources(Action::PutInFolder {
                index: 1,
                folder: Some(1),
            }))
        );
        assert_eq!(
            gesture(&mut drag, point(20.0, 50.0), point(50.0, 350.0)),
            Some(MenuAction::GlobalResources(Action::PutInFolder {
                index: 1,
                folder: None,
            }))
        );
        // A selected pack cannot be dropped on a folder.
        assert_eq!(
            gesture(&mut drag, point(150.0, 50.0), point(50.0, 310.0)),
            None
        );
    }

    #[test]
    fn nothing_is_held_on_another_screen_or_when_the_layout_was_not_drawn() {
        let mut drag = layout();
        assert!(drag.pointer(Some(point(20.0, 10.0)), true, true).0);
        drag.end_frame();
        drag.end_frame();
        assert!(!drag.captured());
    }
}
