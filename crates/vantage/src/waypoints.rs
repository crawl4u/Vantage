//! Waypoints: spots the player marks, shown by the Waypoints element with their distance and the
//! compass direction to walk in. Pure data and arithmetic; `guest.rs` reads the keys and the position.

use serde::{Deserialize, Serialize};

/// The most waypoints kept in all, the most the element lists, and the most read for the compass and the world.
pub const MAX_WAYPOINTS: usize = 16;
pub const SHOWN: usize = 5;
pub const NEAREST: usize = 8;
/// How long a removed waypoint can be brought back, in seconds.
pub const UNDO_SECONDS: f64 = 15.0;

/// The colours a waypoint can have: (name, red, green, blue) as the player sees them.
pub const COLORS: [(&str, [f32; 3]); 8] = [
    ("Blue", [0.35, 0.65, 1.0]),
    ("Red", [1.0, 0.3, 0.3]),
    ("Green", [0.4, 0.9, 0.4]),
    ("Yellow", [1.0, 0.9, 0.3]),
    ("Orange", [1.0, 0.6, 0.2]),
    ("Purple", [0.7, 0.45, 1.0]),
    ("Cyan", [0.3, 0.95, 0.95]),
    ("Pink", [1.0, 0.55, 0.8]),
];
pub const COLOR_NAMES: [&str; 8] = ["Blue", "Red", "Green", "Yellow", "Orange", "Purple", "Cyan", "Pink"];
/// The colour dropdowns' rows: the eight colours, then "Custom" (opens the hex picker).
pub const COLOR_CHOICES: [&str; 9] = ["Blue", "Red", "Green", "Yellow", "Orange", "Purple", "Cyan", "Pink", "Custom"];

/// A stored colour made valid: one of the eight, or a custom `CUSTOM_BASE + 0xRRGGBB` (always opaque).
pub fn clamp_color(color: usize) -> usize {
    if color >= crate::config::CUSTOM_BASE { crate::config::opaque(crate::config::CUSTOM_BASE + crate::config::custom_rgb(color).unwrap_or(0xFF_FFFF) as usize) } else { color.min(COLORS.len() - 1) }
}

/// A stored colour as red, green and blue from 0 to 1.
pub fn rgb(color: usize) -> [f32; 3] {
    match crate::config::custom_rgb(color) {
        Some(v) => [(v >> 16 & 255) as f32 / 255.0, (v >> 8 & 255) as f32 / 255.0, (v & 255) as f32 / 255.0],
        None => COLORS[color.min(COLORS.len() - 1)].1,
    }
}

/// A stored colour as 24 bits (for the hex editor).
pub fn rgb24(color: usize) -> u32 {
    let [r, g, b] = rgb(color);
    (((r * 255.0).round() as u32) << 16) | (((g * 255.0).round() as u32) << 8) | (b * 255.0).round() as u32
}

fn yes() -> bool {
    true
}

/// One marked spot: where (block coordinates), in which dimension of which world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Waypoint {
    pub name: String,
    /// The host's id of the world or server the spot is in (empty when the host gave none).
    pub world: String,
    pub dimension: i32,
    pub pos: [i32; 3],
    /// Index into `COLORS`.
    #[serde(default)]
    pub color: usize,
    /// Shown on the compass and in the world; a hidden one stays in the list.
    #[serde(default = "yes")]
    pub visible: bool,
    /// The group it is filed under (empty: none).
    #[serde(default)]
    pub group: String,
}

/// One line of the element: a waypoint's name, how far it is (blocks, rounded) and which way it lies.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub distance: u32,
    pub direction: &'static str,
    /// The yaw that faces it (Bedrock: 0 south, 90 west, 180 north, 270 east) in half degrees, 0 to 719.
    pub heading: u16,
    pub color: usize,
}

/// The compass point of the way from here to a spot `dx` east and `dz` south of it (Minecraft: +X is east,
/// +Z is south). Eight points; "here" when both are tiny.
pub fn direction(dx: f64, dz: f64) -> &'static str {
    if dx.abs() < 0.5 && dz.abs() < 0.5 {
        return "here";
    }
    const POINTS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    // 0 degrees is north (-Z), 90 east (+X).
    let degrees = dx.atan2(-dz).to_degrees().rem_euclid(360.0);
    POINTS[(((degrees + 22.5) / 45.0) as usize) % 8]
}

/// The heading to a spot `dx` east and `dz` south, in half degrees of Bedrock yaw.
pub fn heading(dx: f64, dz: f64) -> u16 {
    let degrees = (-dx).atan2(dz).to_degrees().rem_euclid(360.0);
    ((degrees * 2.0).round() as u16) % 720
}

fn distance_between(a: [f64; 3], b: [i32; 3]) -> f64 {
    // A block's middle, so standing on a waypoint reads 0.
    let (dx, dy, dz) = (f64::from(b[0]) + 0.5 - a[0], f64::from(b[1]) - a[1], f64::from(b[2]) + 0.5 - a[2]);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn here<'a>(list: &'a [Waypoint], world: &'a str, dimension: i32) -> impl Iterator<Item = (usize, &'a Waypoint)> + 'a {
    list.iter().enumerate().filter(move |(_, w)| w.world == world && w.dimension == dimension)
}

/// The nearest `limit` shown (not hidden) waypoints of this world and dimension as lines, nearest first.
pub fn rows(list: &[Waypoint], world: &str, dimension: i32, at: [f64; 3], limit: usize) -> Vec<Row> {
    let mut found: Vec<(f64, Row)> = here(list, world, dimension)
        .filter(|(_, w)| w.visible)
        .map(|(_, w)| {
            let distance = distance_between(at, w.pos);
            let (dx, dz) = (f64::from(w.pos[0]) + 0.5 - at[0], f64::from(w.pos[2]) + 0.5 - at[2]);
            let row = Row {
                name: w.name.clone(),
                distance: distance.round().min(9_999_999.0) as u32,
                direction: direction(dx, dz),
                heading: heading(dx, dz),
                color: clamp_color(w.color),
            };
            (distance, row)
        })
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().take(limit).map(|(_, row)| row).collect()
}

/// Marks `pos`; the new waypoint is called "Waypoint N" with the lowest N not taken in this world.
/// None when `MAX_WAYPOINTS` are kept already.
pub fn add(list: &mut Vec<Waypoint>, world: &str, dimension: i32, pos: [i32; 3], color: usize) -> Option<String> {
    add_named(list, world, dimension, pos, color, "Waypoint", true)
}

/// Marks `pos` as `base` (then "base 2", "base 3"... while taken); with `numbered` it starts at "base 1".
pub fn add_named(list: &mut Vec<Waypoint>, world: &str, dimension: i32, pos: [i32; 3], color: usize, base: &str, numbered: bool) -> Option<String> {
    if list.len() >= MAX_WAYPOINTS {
        return None;
    }
    let base: String = base.trim().chars().filter(|c| !c.is_control()).take(24).collect();
    let base = if base.is_empty() { "Waypoint".to_owned() } else { base };
    let name = (1..)
        .map(|n| if n == 1 && !numbered { base.clone() } else { format!("{base} {n}") })
        .find(|name| !here(list, world, dimension).any(|(_, w)| &w.name == name))?;
    list.push(Waypoint { name: name.clone(), world: world.to_owned(), dimension, pos, color: clamp_color(color), visible: true, group: String::new() });
    Some(name)
}

/// Removes the nearest waypoint of this world and dimension (hidden ones too) and gives it back, or None when there is none.
pub fn remove_nearest(list: &mut Vec<Waypoint>, world: &str, dimension: i32, at: [f64; 3]) -> Option<Waypoint> {
    let index = here(list, world, dimension)
        .min_by(|a, b| distance_between(at, a.1.pos).total_cmp(&distance_between(at, b.1.pos)))
        .map(|(i, _)| i)?;
    Some(list.remove(index))
}

/// Puts a removed waypoint back; false when the list is full or its name is taken again in that world.
pub fn restore(list: &mut Vec<Waypoint>, waypoint: Waypoint) -> bool {
    if list.len() >= MAX_WAYPOINTS || here(list, &waypoint.world, waypoint.dimension).any(|(_, w)| w.name == waypoint.name) {
        return false;
    }
    list.push(waypoint);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directions_follow_the_compass() {
        assert_eq!(direction(0.0, -10.0), "N");
        assert_eq!(direction(10.0, 0.0), "E");
        assert_eq!(direction(0.0, 10.0), "S");
        assert_eq!(direction(-10.0, 0.0), "W");
        assert_eq!(direction(10.0, -10.0), "NE");
        assert_eq!(direction(-10.0, 10.0), "SW");
        assert_eq!(direction(0.1, 0.1), "here");
    }

    #[test]
    fn waypoints_are_named_listed_nearest_first_and_removed() {
        let mut list = Vec::new();
        assert_eq!(add(&mut list, "w", 0, [100, 64, 0], 0).as_deref(), Some("Waypoint 1"));
        assert_eq!(add(&mut list, "w", 0, [10, 64, 0], 1).as_deref(), Some("Waypoint 2"));
        assert_eq!(add(&mut list, "other", 0, [0, 64, 0], 0).as_deref(), Some("Waypoint 1"));
        let shown = rows(&list, "w", 0, [0.5, 64.0, 0.5], 5);
        assert_eq!(shown.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Waypoint 2", "Waypoint 1"]);
        assert_eq!((shown[0].distance, shown[0].direction), (10, "E"));
        let removed = remove_nearest(&mut list, "w", 0, [0.5, 64.0, 0.5]).unwrap();
        assert_eq!(removed.name, "Waypoint 2");
        assert!(restore(&mut list, removed.clone()), "an undo puts it back");
        assert!(!restore(&mut list, removed.clone()), "but not twice");
        remove_nearest(&mut list, "w", 0, [0.5, 64.0, 0.5]);
        assert_eq!(add(&mut list, "w", 0, [1, 64, 1], 2).as_deref(), Some("Waypoint 2"), "the freed name is reused");
        assert_eq!(rows(&list, "w", 1, [0.0; 3], 5), Vec::new());
    }

    #[test]
    fn headings_use_bedrock_yaw_and_hidden_waypoints_are_not_listed() {
        assert_eq!([heading(0.0, 10.0), heading(-10.0, 0.0), heading(0.0, -10.0), heading(10.0, 0.0)], [0, 180, 360, 540]);
        let mut list = Vec::new();
        add(&mut list, "w", 0, [10, 64, 0], 3);
        list[0].visible = false;
        assert!(rows(&list, "w", 0, [0.0; 3], 5).is_empty());
    }

    #[test]
    fn the_list_is_capped() {
        let mut list = Vec::new();
        for n in 0..MAX_WAYPOINTS {
            assert!(add(&mut list, "w", 0, [n as i32, 64, 0], 0).is_some());
        }
        assert_eq!(add(&mut list, "w", 0, [0, 0, 0], 0), None);
    }
}
