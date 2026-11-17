//! The Waypoints page of the menu (modelled on Cinnabar Client's): a list of the waypoints of the world and
//! dimension you are in, with show/hide, edit and delete on each row, and an editor beside it with the name, X, Y, Z and
//! colour, a button that fills in your position, Save and Cancel, and Undo for the last delete.
//!
//! The page is built by `tools/menukit.py` (`wprow` rows); this module keeps its state and answers its events. The
//! rows are at most `crate::waypoints::MAX_WAYPOINTS` (16).

use crate::config::Settings;
use crate::menu::Value;
use crate::waypoints::{self, Waypoint, MAX_WAYPOINTS, UNDO_SECONDS};

/// The rows the page has room for.
pub const ROWS: usize = MAX_WAYPOINTS;

/// The dimensions' names (the game's ids 0, 1 and 2).
pub fn dimension_name(dimension: i32) -> String {
    match dimension {
        0 => "Overworld".to_owned(),
        1 => "Nether".to_owned(),
        2 => "End".to_owned(),
        n => format!("Dimension {n}"),
    }
}

/// Which dimensions the list shows, in the order the button cycles.
pub const DIM_VIEWS: [&str; 5] = ["Here", "Overworld", "Nether", "End", "All"];
/// How the list is sorted.
pub const SORTS: [&str; 4] = ["Nearest", "Name", "Group", "Oldest first"];

/// One row of the list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RowView {
    pub name: String,
    /// "x, y, z".
    pub pos: String,
    pub visible: bool,
    pub color: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WpUi {
    /// Where the player is: the world's id, the dimension, the block, and whether there is a world at all.
    pub world: String,
    pub dimension: i32,
    pub pos: [i32; 3],
    pub in_world: bool,
    /// The rows of this world and dimension (storage order).
    pub rows: Vec<RowView>,
    /// The editor's fields, and the name of the waypoint being edited (none: a new one is being added).
    pub name: String,
    pub x: String,
    pub y: String,
    pub z: String,
    pub editing: Option<String>,
    /// The coordinates were typed or loaded for an edit: they stop following the player.
    pub touched: bool,
    /// What is wrong with the draft, said under the buttons.
    pub error: String,
    /// The last deleted waypoint and the seconds left to bring it back.
    pub undo: Option<(Waypoint, f64)>,
    /// The heading of the editor, the line under the list, the undo button's label and the used count.
    pub mode: String,
    pub info: String,
    pub undo_label: String,
    /// The row whose delete button was pressed once (with "Confirm Waypoint Deletion" on).
    pub confirm: Option<usize>,
    /// The list's view: which dimensions (`DIM_VIEWS`), the sort (`SORTS`), the group (0 all, then the world's groups).
    pub dim_view: usize,
    pub sort: usize,
    pub group_view: usize,
    /// The labels of the three buttons that change the view.
    pub dim_label: String,
    pub sort_label: String,
    pub group_label: String,
    /// The group field of the editor, and the dimension of the waypoint being edited.
    pub group: String,
    pub edit_dim: i32,
}

/// What the caller should do after an event.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outcome {
    /// The waypoints changed: save the settings.
    pub save: bool,
    pub republish: bool,
}

fn finite_bounded(value: f64, limit: f64) -> bool {
    value.is_finite() && value.abs() <= limit
}

/// A coordinate typed in a field: a whole or decimal number, rounded down to its block.
fn parse_coord(text: &str, limit: f64) -> Option<i32> {
    let value: f64 = text.trim().parse().ok()?;
    finite_bounded(value, limit).then(|| value.floor() as i32)
}

impl WpUi {
    /// The groups of this world's waypoints, sorted.
    fn groups(&self, settings: &Settings) -> Vec<String> {
        let mut groups: Vec<String> = settings.gameplay.waypoints.iter().filter(|w| w.world == self.world && !w.group.is_empty()).map(|w| w.group.clone()).collect();
        groups.sort_by_key(|g| g.to_lowercase());
        groups.dedup();
        groups
    }

    /// The list as shown: storage indexes of this world's waypoints in the chosen dimensions and group, sorted.
    fn order(&self, settings: &Settings) -> Vec<usize> {
        let dimension = match self.dim_view {
            0 => Some(self.dimension),
            1 => Some(0),
            2 => Some(1),
            3 => Some(2),
            _ => None,
        };
        let group = self.group_view.checked_sub(1).and_then(|n| self.groups(settings).get(n).cloned());
        let list = &settings.gameplay.waypoints;
        let mut found: Vec<usize> = (0..list.len())
            .filter(|&i| list[i].world == self.world && dimension.map_or(true, |d| list[i].dimension == d) && (self.group_view == 0 || group.as_ref().is_some_and(|g| &list[i].group == g)))
            .collect();
        let apart = |i: usize| {
            let w = &list[i];
            let d = (0..3).map(|k| f64::from(w.pos[k] - self.pos[k]).powi(2)).sum::<f64>();
            (w.dimension != self.dimension, w.dimension, d as u64)
        };
        match self.sort {
            0 => found.sort_by_key(|&i| apart(i)),
            1 => found.sort_by_key(|&i| list[i].name.to_lowercase()),
            2 => found.sort_by_key(|&i| (list[i].group.is_empty(), list[i].group.to_lowercase(), list[i].name.to_lowercase())),
            _ => {}
        }
        found
    }

    /// Where the player is now (the guest calls this every frame); true when the page's texts changed.
    pub fn sync(&mut self, settings: &Settings, world: &str, dimension: i32, pos: [i32; 3], in_world: bool) -> bool {
        let before = self.clone();
        self.world.clear();
        self.world.push_str(world);
        self.dimension = dimension;
        self.pos = pos;
        self.in_world = in_world;
        let list = &settings.gameplay.waypoints;
        let groups = self.groups(settings);
        if self.group_view > groups.len() {
            self.group_view = 0;
        }
        let many = self.dim_view == 4;
        self.rows = self
            .order(settings)
            .into_iter()
            .take(ROWS)
            .map(|i| {
                let w = &list[i];
                let mut pos = format!("{}, {}, {}", w.pos[0], w.pos[1], w.pos[2]);
                if many || w.dimension != self.dimension {
                    pos.push_str(" - ");
                    pos.push_str(&dimension_name(w.dimension));
                }
                if !w.group.is_empty() {
                    pos.push_str(" - ");
                    pos.push_str(&w.group);
                }
                RowView { name: w.name.clone(), pos, visible: w.visible, color: waypoints::clamp_color(w.color) }
            })
            .collect();
        self.dim_label = format!("Show: {}", DIM_VIEWS[self.dim_view.min(DIM_VIEWS.len() - 1)]);
        self.sort_label = format!("Sort: {}", SORTS[self.sort.min(SORTS.len() - 1)]);
        self.group_label = match self.group_view.checked_sub(1).and_then(|n| groups.get(n)) {
            Some(g) => format!("Group: {g}"),
            None => "Group: All".to_owned(),
        };
        // An edit whose waypoint is gone (deleted by a key) is over.
        if let Some(name) = &self.editing {
            if !list.iter().any(|w| w.world == self.world && w.dimension == self.edit_dim && &w.name == name) {
                self.editing = None;
            }
        }
        // A new waypoint starts at the player's block: the boxes follow it until they are typed in.
        if self.editing.is_none() && !self.touched && in_world {
            self.x = pos[0].to_string();
            self.y = pos[1].to_string();
            self.z = pos[2].to_string();
        }
        self.mode = match &self.editing {
            Some(name) => format!("Edit {name}"),
            None => "Add waypoint".to_owned(),
        };
        self.info = if !in_world {
            "Join a world to add waypoints.".to_owned()
        } else if list.len() >= MAX_WAYPOINTS {
            format!("{} of {} used. Delete one first.", list.len(), MAX_WAYPOINTS)
        } else if self.rows.is_empty() {
            format!("None here yet. {} of {} used.", list.len(), MAX_WAYPOINTS)
        } else {
            format!("{} of {} used across worlds.", list.len(), MAX_WAYPOINTS)
        };
        self.undo_label = match &self.undo {
            Some((w, left)) => format!("Undo delete of {} ({}s)", w.name.chars().take(14).collect::<String>(), left.ceil() as u32),
            None => "Nothing to undo".to_owned(),
        };
        *self != before
    }

    /// Counts the undo down; true when it ran out.
    pub fn tick(&mut self, seconds: f32) -> bool {
        let Some((_, left)) = &mut self.undo else { return false };
        *left -= f64::from(seconds.max(0.0));
        if *left <= 0.0 {
            self.undo = None;
            return true;
        }
        false
    }

    /// Keeps a removed waypoint for the undo button and the undo key.
    pub fn set_undo(&mut self, waypoint: Waypoint) {
        self.undo = Some((waypoint, UNDO_SECONDS));
    }

    /// Brings the last removed waypoint back; its name, or a reason it cannot be.
    pub fn restore(&mut self, settings: &mut Settings) -> Result<String, String> {
        let Some((waypoint, _)) = self.undo.take() else { return Err("Nothing to undo.".to_owned()) };
        let name = waypoint.name.clone();
        if waypoints::restore(&mut settings.gameplay.waypoints, waypoint) {
            Ok(name)
        } else {
            Err("Undo failed: full or name taken.".to_owned())
        }
    }

    /// A field's text (typed or pasted).
    pub fn set_field(&mut self, id: &str, text: &str) {
        let clean = |limit: usize| -> String { text.chars().filter(|c| !c.is_control()).take(limit).collect() };
        match id {
            "wp_name" => self.name = clean(48),
            "wp_x" => self.x = clean(12),
            "wp_y" => self.y = clean(12),
            "wp_z" => self.z = clean(12),
            "wp_group" => self.group = clean(16),
            _ => return,
        }
        if !matches!(id, "wp_name" | "wp_group") {
            self.touched = true;
        }
        self.error.clear();
    }

    fn clear_draft(&mut self) {
        self.name.clear();
        self.x.clear();
        self.y.clear();
        self.z.clear();
        self.group.clear();
        self.editing = None;
        self.touched = false;
        self.error.clear();
    }

    /// The waypoint of row `row` in the settings' list.
    fn index_of(&self, settings: &Settings, row: usize) -> Option<usize> {
        self.order(settings).get(row).copied()
    }

    /// A press on one of the page's buttons.
    pub fn event(&mut self, settings: &mut Settings, id: &str) -> Outcome {
        let mut out = Outcome { republish: true, ..Outcome::default() };
        let row = |prefix: &str| id.strip_prefix(prefix).and_then(|n| n.parse::<usize>().ok());
        let asked = self.confirm.take();
        if let Some(n) = row("wp_name_").or_else(|| row("wp_pos_")).or_else(|| row("wp_edit_")) {
            if let Some(i) = self.index_of(settings, n) {
                let w = settings.gameplay.waypoints[i].clone();
                self.name = w.name.clone();
                self.x = w.pos[0].to_string();
                self.y = w.pos[1].to_string();
                self.z = w.pos[2].to_string();
                self.editing = Some(w.name);
                self.edit_dim = w.dimension;
                self.group = w.group.clone();
                self.touched = true;
                self.error.clear();
                settings.gameplay.wp_color = waypoints::clamp_color(w.color);
                out.save = true;
            }
        } else if let Some(n) = row("wp_vis_") {
            if let Some(i) = self.index_of(settings, n) {
                let w = &mut settings.gameplay.waypoints[i];
                w.visible = !w.visible;
                out.save = true;
            }
        } else if let Some(n) = row("wp_del_") {
            if let Some(i) = self.index_of(settings, n) {
                if settings.gameplay.wp_confirm && asked != Some(n) {
                    self.confirm = Some(n);
                    self.error = "Press delete again to confirm".to_owned();
                    return out;
                }
                self.error.clear();
                let removed = settings.gameplay.waypoints.remove(i);
                if self.editing.as_deref() == Some(removed.name.as_str()) {
                    self.clear_draft();
                }
                self.set_undo(removed);
                out.save = true;
            }
        } else {
            match id {
                "wp_cancel" => self.clear_draft(),
                "wp_fdim" => self.dim_view = (self.dim_view + 1) % DIM_VIEWS.len(),
                "wp_fsort" => self.sort = (self.sort + 1) % SORTS.len(),
                "wp_fgroup" => self.group_view = (self.group_view + 1) % (self.groups(settings).len() + 1),
                "wp_undo" => match self.restore(settings) {
                    Ok(_) => out.save = true,
                    Err(message) => self.error = message,
                },
                "wp_save" => out.save = self.save(settings),
                _ => out.republish = false,
            }
        }
        out
    }

    /// Saves the draft as a new waypoint or over the one being edited; true when the settings changed.
    fn save(&mut self, settings: &mut Settings) -> bool {
        if !self.in_world {
            self.error = "Join a world first.".to_owned();
            return false;
        }
        let name = self.name.trim().to_owned();
        if name.chars().count() > 48 || name.chars().any(char::is_control) {
            self.error = "Name: at most 48 characters.".to_owned();
            return false;
        }
        // Empty coordinates mean "where I stand".
        let coord = |text: &str, here: i32, limit: f64| if text.trim().is_empty() { Some(here) } else { parse_coord(text, limit) };
        let (Some(x), Some(y), Some(z)) = (coord(&self.x, self.pos[0], 30_000_000.0), coord(&self.y, self.pos[1], 4_096.0), coord(&self.z, self.pos[2], 30_000_000.0)) else {
            self.error = "Enter valid X, Y and Z numbers.".to_owned();
            return false;
        };
        let color = waypoints::clamp_color(settings.gameplay.wp_color);
        let group: String = self.group.trim().chars().filter(|c| !c.is_control()).take(16).collect();
        // An edit keeps the dimension its waypoint is in; a new one is made where you stand.
        let (world, dimension) = (self.world.clone(), if self.editing.is_some() { self.edit_dim } else { self.dimension });
        let taken = |list: &[Waypoint], skip: Option<usize>, name: &str| {
            list.iter().enumerate().any(|(i, w)| Some(i) != skip && w.world == world && w.dimension == dimension && w.name == name)
        };
        let list = &mut settings.gameplay.waypoints;
        match self.editing.clone() {
            Some(old) => {
                let Some(i) = list.iter().position(|w| w.world == world && w.dimension == dimension && w.name == old) else {
                    self.error = "That waypoint is gone.".to_owned();
                    self.clear_draft();
                    return false;
                };
                let name = if name.is_empty() { old } else { name };
                if taken(list, Some(i), &name) {
                    self.error = "That name is already used here.".to_owned();
                    return false;
                }
                let w = &mut list[i];
                w.name = name;
                w.pos = [x, y, z];
                w.color = color;
                w.group = group;
            }
            None => {
                if list.len() >= MAX_WAYPOINTS {
                    self.error = format!("All {MAX_WAYPOINTS} slots are in use.");
                    return false;
                }
                if !name.is_empty() && taken(list, None, &name) {
                    self.error = "That name is already used here.".to_owned();
                    return false;
                }
                match waypoints::add(list, &world, dimension, [x, y, z], color) {
                    Some(auto) => {
                        if let Some(w) = list.iter_mut().find(|w| w.world == world && w.dimension == dimension && w.name == auto) {
                            if !name.is_empty() {
                                w.name = name;
                            }
                            w.group = group;
                        }
                    }
                    None => {
                        self.error = format!("All {MAX_WAYPOINTS} slots are in use.");
                        return false;
                    }
                }
            }
        }
        self.clear_draft();
        true
    }

    /// The value of one of the page's controls, or None for an id that is not the page's.
    pub fn value(&self, id: &str) -> Option<Value<'_>> {
        let row = |prefix: &str| id.strip_prefix(prefix).and_then(|n| n.parse::<usize>().ok());
        if let Some(n) = row("wp_name_") {
            return Some(Value::Button(self.rows.get(n).map(|r| r.name.as_str())));
        }
        if let Some(n) = row("wp_pos_") {
            return Some(Value::Button(self.rows.get(n).map(|r| r.pos.as_str())));
        }
        if let Some(n) = row("wp_vis_") {
            return Some(Value::Button(self.rows.get(n).map(|r| if r.visible { "Hide" } else { "Show" })));
        }
        Some(match id {
            "wp_name" => Value::Text(&self.name),
            "wp_x" => Value::Text(&self.x),
            "wp_y" => Value::Text(&self.y),
            "wp_z" => Value::Text(&self.z),
            "wp_group" => Value::Text(&self.group),
            "wp_fdim" => Value::Button(Some(&self.dim_label)),
            "wp_fsort" => Value::Button(Some(&self.sort_label)),
            "wp_fgroup" => Value::Button(Some(&self.group_label)),
            "wp_mode" => Value::Button(Some(&self.mode)),
            "wp_info" => Value::Button(Some(&self.info)),
            "wp_error" => Value::Button(Some(if self.error.is_empty() { " " } else { &self.error })),
            "wp_undo" => Value::Button(Some(&self.undo_label)),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn here(settings: &Settings) -> WpUi {
        let mut ui = WpUi::default();
        ui.sync(settings, "w", 0, [10, 64, -20], true);
        ui
    }

    #[test]
    fn a_waypoint_is_added_edited_hidden_deleted_and_brought_back() {
        let mut settings = Settings::default();
        let mut ui = here(&settings);
        // Empty fields mean "here", an empty name an automatic one.
        assert!(ui.event(&mut settings, "wp_save").save);
        ui.sync(&settings, "w", 0, [10, 64, -20], true);
        assert_eq!(ui.rows.len(), 1);
        assert_eq!((ui.rows[0].name.as_str(), ui.rows[0].pos.as_str()), ("Waypoint 1", "10, 64, -20"));
        // Edit: the fields fill in, a rename and a move are saved.
        ui.event(&mut settings, "wp_edit_0");
        assert_eq!((ui.editing.as_deref(), ui.x.as_str()), (Some("Waypoint 1"), "10"));
        ui.set_field("wp_name", "Base");
        ui.set_field("wp_x", "100.7");
        assert!(ui.event(&mut settings, "wp_save").save);
        ui.sync(&settings, "w", 0, [10, 64, -20], true);
        assert_eq!((ui.rows[0].name.as_str(), ui.rows[0].pos.as_str()), ("Base", "100, 64, -20"));
        // Hide and show.
        ui.event(&mut settings, "wp_vis_0");
        ui.sync(&settings, "w", 0, [0, 0, 0], true);
        assert!(!ui.rows[0].visible);
        // Delete, then undo.
        ui.event(&mut settings, "wp_del_0");
        ui.sync(&settings, "w", 0, [0, 0, 0], true);
        assert!(ui.rows.is_empty() && ui.undo.is_some());
        ui.event(&mut settings, "wp_undo");
        ui.sync(&settings, "w", 0, [0, 0, 0], true);
        assert_eq!(ui.rows.len(), 1);
    }

    #[test]
    fn bad_drafts_are_refused_with_a_reason() {
        let mut settings = Settings::default();
        let mut ui = here(&settings);
        ui.set_field("wp_x", "abc");
        assert!(!ui.event(&mut settings, "wp_save").save);
        assert!(ui.error.contains("coordinates"), "{}", ui.error);
        ui.set_field("wp_x", "1e99");
        assert!(!ui.event(&mut settings, "wp_save").save, "out of bounds");
        let mut away = WpUi::default();
        away.sync(&settings, "", 0, [0; 3], false);
        assert!(!away.event(&mut settings, "wp_save").save);
        assert!(away.error.contains("Join a world"));
        assert!(settings.gameplay.waypoints.is_empty());
    }

    #[test]
    fn the_undo_runs_out_and_names_stay_unique() {
        let mut settings = Settings::default();
        let mut ui = here(&settings);
        ui.set_field("wp_name", "Home");
        assert!(ui.event(&mut settings, "wp_save").save);
        ui.set_field("wp_name", "Home");
        assert!(!ui.event(&mut settings, "wp_save").save, "the name is taken");
        ui.sync(&settings, "w", 0, [0; 3], true);
        ui.event(&mut settings, "wp_del_0");
        assert!(!ui.tick(14.0));
        assert!(ui.tick(2.0));
        assert!(ui.undo.is_none());
    }
}
