//! Java Edition block models as Bedrock block geometry.
//!
//! A Java block model is JSON: a `parent` to inherit from, texture variables and a list of
//! `elements`, boxes whose faces name a texture variable. [`resolve`] follows the parents, with the
//! handful of standard shapes Java ships as parents ([`builtin`]) written here as code, and
//! [`to_geometry`] turns the result into a Bedrock `minecraft:geometry` document whose faces name
//! `material_instance`s after the texture variables, so the client's own block geometry
//! compiler can draw it.

use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashSet};

/// The deepest parent chain followed; Java's own chains are a handful long.
const MAX_DEPTH: usize = 16;

/// The face names in Bedrock's and Java's shared order.
const FACES: [&str; 6] = ["north", "east", "south", "west", "up", "down"];

/// One box of a model, in Java pixels (0 to 16 across the block).
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub from: [f64; 3],
    pub to: [f64; 3],
    /// The rotation, as its origin, axis and angle in degrees.
    pub rotation: Option<([f64; 3], char, f64)>,
    /// Each present face: its texture variable, UV rectangle and optional cull face.
    pub faces: BTreeMap<&'static str, Face>,
}

/// One face of an element.
#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    /// The texture variable, without its `#`.
    pub texture: String,
    /// `[u1, v1, u2, v2]` in Java texture pixels, or `None` for the face's own box.
    pub uv: Option<[f64; 4]>,
    pub tint: bool,
    /// Quarter turns the texture is turned clockwise seen from outside the face (Java `rotation`).
    pub rotation: u8,
}

/// A model with its parents followed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    /// Texture variable to a texture name or another variable (`#name`).
    pub textures: BTreeMap<String, String>,
    pub elements: Vec<Element>,
}

/// The standard parents Java ships, by name without the `minecraft:` namespace, as the JSON the
/// game would hold for them. Only shapes that are simple geometry are written out.
fn builtin(name: &str) -> Option<Value> {
    let all_faces = |texture: &dyn Fn(&str) -> String| -> Value {
        let faces: Map<String, Value> = FACES
            .iter()
            .map(|face| {
                (
                    (*face).to_owned(),
                    json!({ "texture": texture(face), "cullface": face }),
                )
            })
            .collect();
        json!({ "elements": [{ "from": [0, 0, 0], "to": [16, 16, 16], "faces": faces }] })
    };
    Some(match name {
        "block/block" => json!({}),
        "block/cube" => all_faces(&|face| format!("#{face}")),
        "block/cube_all" => json!({ "parent": "block/cube", "textures": {
            "particle": "#all", "down": "#all", "up": "#all", "north": "#all",
            "south": "#all", "east": "#all", "west": "#all" } }),
        "block/cube_column" => json!({ "parent": "block/cube", "textures": {
            "particle": "#side", "down": "#end", "up": "#end", "north": "#side",
            "south": "#side", "east": "#side", "west": "#side" } }),
        "block/cube_bottom_top" => json!({ "parent": "block/cube", "textures": {
            "particle": "#side", "down": "#bottom", "up": "#top", "north": "#side",
            "south": "#side", "east": "#side", "west": "#side" } }),
        "block/cross" | "block/tinted_cross" => {
            let tint = name.ends_with("tinted_cross");
            let plane = |axis_from: [i32; 3], axis_to: [i32; 3], angle: i32| {
                let face = |dir: &str| {
                    let mut face = json!({ "uv": [0, 0, 16, 16], "texture": "#cross" });
                    if tint {
                        face["tintindex"] = json!(0);
                    }
                    (dir.to_owned(), face)
                };
                json!({ "from": axis_from, "to": axis_to, "shade": false,
                    "rotation": { "origin": [8, 8, 8], "axis": "y", "angle": angle, "rescale": true },
                    "faces": Map::from_iter([face("north"), face("south")]) })
            };
            json!({ "textures": { "particle": "#cross" }, "elements": [
                plane([0, 0, 8], [16, 16, 8], 45), plane([8, 0, 0], [8, 16, 16], 45)] })
        }
        "block/fence_post" => json!({ "textures": { "particle": "#texture" }, "elements": [
            part([6, 0, 6], [10, 16, 10], &[
                ("down", [6, 6, 10, 10], "texture"), ("up", [6, 6, 10, 10], "texture"),
                ("north", [6, 0, 10, 16], "texture"), ("south", [6, 0, 10, 16], "texture"),
                ("west", [6, 0, 10, 16], "texture"), ("east", [6, 0, 10, 16], "texture")])] }),
        "block/fence_side" => json!({ "textures": { "particle": "#texture" }, "elements": [
            part([7, 12, 0], [9, 15, 9], &[
                ("down", [7, 0, 9, 9], "texture"), ("up", [7, 0, 9, 9], "texture"),
                ("north", [7, 1, 9, 4], "texture"), ("west", [0, 1, 9, 4], "texture"),
                ("east", [0, 1, 9, 4], "texture")]),
            part([7, 6, 0], [9, 9, 9], &[
                ("down", [7, 0, 9, 9], "texture"), ("up", [7, 0, 9, 9], "texture"),
                ("north", [7, 7, 9, 10], "texture"), ("west", [0, 7, 9, 10], "texture"),
                ("east", [0, 7, 9, 10], "texture")])] }),
        "block/template_glass_pane_post" => {
            json!({ "textures": { "particle": "#pane" }, "elements": [
            part([7, 0, 7], [9, 16, 9], &[
                ("down", [7, 7, 9, 9], "edge"), ("up", [7, 7, 9, 9], "edge")])] })
        }
        "block/template_glass_pane_side" => {
            json!({ "textures": { "particle": "#pane" }, "elements": [
            part([7, 0, 0], [9, 16, 7], &[
                ("down", [7, 0, 9, 7], "edge"), ("up", [7, 0, 9, 7], "edge"),
                ("north", [7, 0, 9, 16], "edge"), ("west", [16, 0, 9, 16], "pane"),
                ("east", [9, 0, 16, 16], "pane")])] })
        }
        "block/template_glass_pane_side_alt" => {
            json!({ "textures": { "particle": "#pane" }, "elements": [
            part([7, 0, 9], [9, 16, 16], &[
                ("down", [7, 0, 9, 7], "edge"), ("up", [7, 0, 9, 7], "edge"),
                ("south", [7, 0, 9, 16], "edge"), ("west", [7, 0, 0, 16], "pane"),
                ("east", [0, 0, 7, 16], "pane")])] })
        }
        "block/template_glass_pane_noside" => {
            json!({ "textures": { "particle": "#pane" }, "elements": [
            part([7, 0, 7], [9, 16, 9], &[("north", [9, 0, 7, 16], "pane")])] })
        }
        "block/template_glass_pane_noside_alt" => {
            json!({ "textures": { "particle": "#pane" }, "elements": [
            part([7, 0, 7], [9, 16, 9], &[("east", [7, 0, 9, 16], "pane")])] })
        }
        _ => return None,
    })
}

/// One element of a built-in parent: a box and the faces it shows, each as a face name, a UV
/// rectangle and the texture variable it draws.
fn part(from: [i32; 3], to: [i32; 3], faces: &[(&str, [i32; 4], &str)]) -> Value {
    let faces: Map<String, Value> = faces
        .iter()
        .map(|(face, uv, texture)| {
            (
                (*face).to_owned(),
                json!({ "uv": uv, "texture": format!("#{texture}") }),
            )
        })
        .collect();
    json!({ "from": from, "to": to, "faces": faces })
}

/// Strips a `minecraft:` namespace and any `.json`.
fn bare(name: &str) -> &str {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    name.strip_suffix(".json").unwrap_or(name)
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64().filter(|value| value.is_finite())
}

fn triple(value: &Value) -> Option<[f64; 3]> {
    let list = value.as_array()?;
    Some([
        number(list.first()?)?,
        number(list.get(1)?)?,
        number(list.get(2)?)?,
    ])
}

/// Reads an element's JSON; `None` when it has no usable box.
fn element(value: &Value) -> Option<Element> {
    let from = triple(value.get("from")?)?;
    let to = triple(value.get("to")?)?;
    let rotation = value.get("rotation").and_then(|rotation| {
        let axis = rotation.get("axis")?.as_str()?.chars().next()?;
        ("xyz".contains(axis)).then_some(())?;
        Some((
            triple(rotation.get("origin")?)?,
            axis,
            number(rotation.get("angle")?)?,
        ))
    });
    let mut faces = BTreeMap::new();
    for name in FACES {
        let Some(face) = value.get("faces").and_then(|faces| faces.get(name)) else {
            continue;
        };
        let Some(texture) = face.get("texture").and_then(Value::as_str) else {
            continue;
        };
        let uv = face.get("uv").and_then(Value::as_array).and_then(|uv| {
            Some([
                number(uv.first()?)?,
                number(uv.get(1)?)?,
                number(uv.get(2)?)?,
                number(uv.get(3)?)?,
            ])
        });
        faces.insert(
            name,
            Face {
                texture: texture.trim_start_matches('#').to_owned(),
                uv,
                tint: face.get("tintindex").is_some(),
                rotation: face
                    .get("rotation")
                    .and_then(Value::as_i64)
                    .filter(|degrees| degrees % 90 == 0)
                    .map_or(0, |degrees| (degrees / 90).rem_euclid(4) as u8),
            },
        );
    }
    Some(Element {
        from,
        to,
        rotation,
        faces,
    })
}

/// The model named `name` with its parents followed. `load` reads a pack's own model JSON by
/// name (without namespace or extension, such as `block/oak_fence_post`); names it does not hold
/// fall back to [`builtin`]. `None` when a parent is missing or the chain loops or runs too deep.
pub fn resolve(name: &str, load: &dyn Fn(&str) -> Option<Value>) -> Option<Model> {
    let mut chain: Vec<Value> = Vec::new();
    let mut seen = HashSet::new();
    let mut next = Some(bare(name).to_owned());
    while let Some(current) = next.take() {
        if chain.len() >= MAX_DEPTH || !seen.insert(current.clone()) {
            return None;
        }
        let document = load(&current).or_else(|| builtin(&current))?;
        next = document
            .get("parent")
            .and_then(Value::as_str)
            .map(|parent| bare(parent).to_owned());
        chain.push(document);
    }
    // Children override their parents' textures; the first ancestor that lists elements wins.
    let mut model = Model::default();
    for document in chain.iter().rev() {
        if let Some(textures) = document.get("textures").and_then(Value::as_object) {
            for (key, value) in textures {
                // Current Java wraps a texture in an object naming its `sprite`.
                if let Some(value) = value
                    .as_str()
                    .or_else(|| value.get("sprite").and_then(Value::as_str))
                {
                    model.textures.insert(key.clone(), value.to_owned());
                }
            }
        }
        if let Some(elements) = document.get("elements").and_then(Value::as_array) {
            model.elements = elements.iter().filter_map(element).collect();
        }
    }
    Some(model)
}

impl Model {
    /// The texture a variable names after following `#` references, or `None` when it names
    /// nothing or loops.
    pub fn texture(&self, variable: &str) -> Option<&str> {
        let mut name = variable;
        for _ in 0..MAX_DEPTH {
            let value = self.textures.get(name)?;
            match value.strip_prefix('#') {
                Some(next) => name = next,
                None => return Some(value.strip_prefix("minecraft:").unwrap_or(value)),
            }
        }
        None
    }
}

impl Model {
    /// The model after Java's blockstate turns: `x` then `y` degrees, each a multiple of 90.
    /// `None` for other angles and for models with element rotations, whose rescaling is not
    /// translated. Each face's texture turns with it, unless `uvlock` keeps it as it was.
    pub fn turned(&self, x: i64, y: i64, uvlock: bool) -> Option<Model> {
        if x % 90 != 0 || y % 90 != 0 || self.elements.iter().any(|e| e.rotation.is_some()) {
            return None;
        }
        let mut model = self.clone();
        // A face's own box decides its default UVs, so they are fixed before the box moves.
        for element in &mut model.elements {
            let (from, to) = (element.from, element.to);
            for (name, face) in &mut element.faces {
                face.uv.get_or_insert_with(|| default_uv(name, from, to));
            }
        }
        let quarter_turns = [(x, false), (y, true)];
        for (degrees, about_y) in quarter_turns {
            for _ in 0..(degrees.rem_euclid(360) / 90) {
                for element in &mut model.elements {
                    turn_element(element, about_y, uvlock);
                }
            }
        }
        Some(model)
    }

    /// The same model with every face naming a variable derived from the texture it shows, so
    /// models whose variables differ can be merged. `None` when a face shows no texture.
    pub fn flattened(&self) -> Option<Model> {
        let mut model = Model::default();
        let mut elements = self.elements.clone();
        for element in &mut elements {
            for face in element.faces.values_mut() {
                let texture = self.texture(&face.texture)?;
                let variable = texture.replace('/', "_");
                model.textures.insert(variable.clone(), texture.to_owned());
                face.texture = variable;
            }
        }
        model.elements = elements;
        Some(model)
    }
}

/// A face's outward normal.
fn normal(face: &str) -> [i32; 3] {
    match face {
        "north" => [0, 0, -1],
        "east" => [1, 0, 0],
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        _ => [0, -1, 0],
    }
}

/// The face with outward normal `normal`.
fn face_named(normal: [i32; 3]) -> &'static str {
    FACES
        .into_iter()
        .find(|face| self::normal(face) == normal)
        .unwrap_or("north")
}

/// The direction the top of an unturned texture points on a face: up for the sides, north on the
/// up face (its `v` runs south) and south on the down face (its `v` runs north).
fn texture_top(face: &str) -> [i32; 3] {
    match face {
        "up" => [0, 0, -1],
        "down" => [0, 0, 1],
        _ => [0, 1, 0],
    }
}

/// One quarter turn of a direction: about the vertical axis clockwise seen from above (north to
/// east), or about the east-west axis tipping north down.
fn turn_vector([x, y, z]: [i32; 3], about_y: bool) -> [i32; 3] {
    if about_y { [-z, y, x] } else { [x, z, -y] }
}

/// Turns one element a quarter turn about the block centre, renaming its faces and turning each
/// face's texture with it: by the quarter turns between where the texture's top pointed and
/// where an unturned texture's top points on the face it lands on, unless `uvlock`.
fn turn_element(element: &mut Element, about_y: bool, uvlock: bool) {
    let point = |[x, y, z]: [f64; 3]| {
        if about_y {
            [16.0 - z, y, x]
        } else {
            [x, z, 16.0 - y]
        }
    };
    let (a, b) = (point(element.from), point(element.to));
    element.from = std::array::from_fn(|axis| a[axis].min(b[axis]));
    element.to = std::array::from_fn(|axis| a[axis].max(b[axis]));
    let mut turned = BTreeMap::new();
    for (name, mut face) in std::mem::take(&mut element.faces) {
        let landed_normal = turn_vector(normal(name), about_y);
        let landed = face_named(landed_normal);
        if !uvlock {
            // Clockwise seen from outside takes a direction `v` to `v x normal`.
            let cross = |v: [i32; 3], n: [i32; 3]| {
                [
                    v[1] * n[2] - v[2] * n[1],
                    v[2] * n[0] - v[0] * n[2],
                    v[0] * n[1] - v[1] * n[0],
                ]
            };
            let wanted = turn_vector(texture_top(name), about_y);
            let mut top = texture_top(landed);
            let mut quarters = 0;
            while top != wanted && quarters < 4 {
                top = cross(top, landed_normal);
                quarters += 1;
            }
            face.rotation = (face.rotation + quarters % 4) % 4;
        }
        turned.insert(landed, face);
    }
    element.faces = turned;
}

/// The UV rectangle a face with no `uv` of its own shows: its box, as Java derives it.
fn default_uv(face: &str, from: [f64; 3], to: [f64; 3]) -> [f64; 4] {
    let [fx, fy, fz] = from;
    let [tx, ty, tz] = to;
    match face {
        "down" => [fx, 16.0 - tz, tx, 16.0 - fz],
        "up" => [fx, fz, tx, tz],
        "north" => [16.0 - tx, 16.0 - ty, 16.0 - fx, 16.0 - fy],
        "south" => [fx, 16.0 - ty, tx, 16.0 - fy],
        "west" => [fz, 16.0 - ty, tz, 16.0 - fy],
        _ => [16.0 - tz, 16.0 - ty, 16.0 - fz, 16.0 - fy],
    }
}

/// The model as a Bedrock `minecraft:geometry` document named `identifier`. Every face names a
/// `material_instance` after its texture variable, so the block binds each variable to a texture.
pub fn to_geometry(identifier: &str, model: &Model) -> Value {
    let cubes: Vec<Value> = model
        .elements
        .iter()
        .map(|element| {
            let mut uv = Map::new();
            for (face, data) in &element.faces {
                let [u1, v1, u2, v2] = data
                    .uv
                    .unwrap_or_else(|| default_uv(face, element.from, element.to));
                // Bedrock starts the up and down textures from the opposite corner to Java.
                let turns =
                    (data.rotation + if matches!(*face, "up" | "down") { 2 } else { 0 }) % 4;
                let mut entry = json!({
                    "uv": [u1, v1],
                    "uv_size": [u2 - u1, v2 - v1],
                    "material_instance": data.texture,
                });
                if turns != 0 {
                    entry["uv_rotation"] = json!(u32::from(turns) * 90);
                }
                uv.insert((*face).to_owned(), entry);
            }
            // Geometry X runs the opposite way to the block's: the cube starts at its far edge.
            let origin = [8.0 - element.to[0], element.from[1], element.from[2] - 8.0];
            let size = [
                element.to[0] - element.from[0],
                element.to[1] - element.from[1],
                element.to[2] - element.from[2],
            ];
            let mut cube = json!({ "origin": origin, "size": size, "uv": uv });
            if let Some((pivot, axis, angle)) = element.rotation {
                let mut rotation = [0.0; 3];
                rotation["xyz".find(axis).unwrap_or(0)] = angle;
                cube["pivot"] = json!([pivot[0] - 8.0, pivot[1], pivot[2] - 8.0]);
                cube["rotation"] = json!(rotation);
            }
            cube
        })
        .collect();
    json!({
        "format_version": "1.21.0",
        "minecraft:geometry": [{
            "description": {
                "identifier": identifier,
                "texture_width": 16,
                "texture_height": 16,
            },
            "bones": [{ "name": "root", "pivot": [0, 0, 0], "cubes": cubes }],
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str) -> Option<Value> {
        None
    }

    #[test]
    fn cube_all_is_a_full_cube_whose_faces_follow_the_all_variable() {
        let mut model = resolve("minecraft:block/cube_all", &none).unwrap();
        model.textures.insert("all".into(), "block/stone".into());
        assert_eq!(model.elements.len(), 1);
        assert_eq!(model.elements[0].faces.len(), 6);
        assert_eq!(model.texture("north"), Some("block/stone"));
        assert_eq!(model.texture("particle"), Some("block/stone"));
    }

    #[test]
    fn a_pack_model_overrides_textures_and_inherits_the_parents_elements() {
        let pack = |name: &str| match name {
            "block/oak_log" => Some(json!({
                "parent": "block/cube_column",
                "textures": { "end": "block/oak_log_top", "side": "block/oak_log" },
            })),
            _ => None,
        };
        let model = resolve("block/oak_log", &pack).unwrap();
        assert_eq!(model.elements.len(), 1);
        assert_eq!(model.texture("up"), Some("block/oak_log_top"));
        assert_eq!(model.texture("east"), Some("block/oak_log"));
    }

    #[test]
    fn a_missing_a_looping_or_too_deep_parent_chain_resolves_to_nothing() {
        assert!(resolve("block/unknown_parent", &none).is_none());
        let looping = |name: &str| match name {
            "a" => Some(json!({"parent": "b"})),
            "b" => Some(json!({"parent": "a"})),
            _ => None,
        };
        assert!(resolve("a", &looping).is_none());
        let deep = |name: &str| {
            let number: usize = name.strip_prefix('m')?.parse().ok()?;
            Some(json!({ "parent": format!("m{}", number + 1) }))
        };
        assert!(resolve("m0", &deep).is_none());
    }

    #[test]
    fn a_face_without_a_uv_shows_its_own_box_as_java_derives_it() {
        assert_eq!(
            default_uv("up", [4.0, 0.0, 4.0], [12.0, 8.0, 12.0]),
            [4.0, 4.0, 12.0, 12.0]
        );
        assert_eq!(
            default_uv("south", [4.0, 0.0, 4.0], [12.0, 8.0, 12.0]),
            [4.0, 8.0, 12.0, 16.0]
        );
        assert_eq!(
            default_uv("north", [0.0; 3], [16.0; 3]),
            [0.0, 0.0, 16.0, 16.0]
        );
    }

    #[test]
    fn geometry_places_boxes_about_the_block_centre_with_per_face_uvs_and_materials() {
        let pack = |name: &str| match name {
            "block/post" => Some(json!({ "textures": {"side": "block/stone"}, "elements": [{
                "from": [6, 0, 6], "to": [10, 16, 10],
                "rotation": {"origin": [8, 8, 8], "axis": "y", "angle": 45},
                "faces": { "north": { "texture": "#side", "uv": [6, 0, 10, 16] } } }] })),
            _ => None,
        };
        let model = resolve("block/post", &pack).unwrap();
        let geometry = to_geometry("geometry.post", &model);
        let cube = &geometry["minecraft:geometry"][0]["bones"][0]["cubes"][0];
        assert_eq!(cube["origin"], json!([-2.0, 0.0, -2.0]));
        assert_eq!(cube["size"], json!([4.0, 16.0, 4.0]));
        assert_eq!(cube["pivot"], json!([0.0, 8.0, 0.0]));
        assert_eq!(cube["rotation"], json!([0.0, 45.0, 0.0]));
        assert_eq!(cube["uv"]["north"]["uv"], json!([6.0, 0.0]));
        assert_eq!(cube["uv"]["north"]["uv_size"], json!([4.0, 16.0]));
        assert_eq!(cube["uv"]["north"]["material_instance"], "side");
        assert!(cube["uv"].get("south").is_none());
    }

    #[test]
    fn the_cross_template_is_two_planes_turned_forty_five_degrees() {
        let model = resolve("block/cross", &none).unwrap();
        assert_eq!(model.elements.len(), 2);
        assert!(
            model
                .elements
                .iter()
                .all(|e| e.rotation.is_some() && e.faces.len() == 2)
        );
        let tinted = resolve("block/tinted_cross", &none).unwrap();
        assert!(tinted.elements[0].faces["north"].tint);
    }

    #[test]
    fn quarter_turns_move_boxes_and_rename_faces_keeping_their_uvs() {
        let side = resolve("block/fence_side", &none).unwrap();
        let east = side.turned(0, 90, false).unwrap();
        let bar = &east.elements[0];
        // A bar reaching north reaches east after a clockwise quarter turn.
        assert_eq!((bar.from, bar.to), ([7.0, 12.0, 7.0], [16.0, 15.0, 9.0]));
        assert_eq!(
            bar.faces.keys().copied().collect::<Vec<_>>(),
            ["down", "east", "north", "south", "up"]
        );
        assert_eq!(bar.faces["east"].uv, Some([7.0, 1.0, 9.0, 4.0]));
        assert_eq!(
            side.turned(0, 360, false).unwrap(),
            side.turned(0, 0, false).unwrap()
        );
        assert!(side.turned(0, 45, false).is_none());
        let down = side.turned(90, 0, false).unwrap();
        assert!(down.elements[0].faces.contains_key("down"));
    }

    #[test]
    fn texture_objects_and_flattening_name_variables_after_their_textures() {
        let pack = |name: &str| match name {
            "block/pane" => Some(json!({ "parent": "block/template_glass_pane_noside",
                "textures": { "pane": { "force_translucent": true, "sprite": "minecraft:block/glass" },
                    "edge": "block/glass_pane_top" } })),
            _ => None,
        };
        let model = resolve("block/pane", &pack).unwrap();
        assert_eq!(model.texture("pane"), Some("block/glass"));
        let flat = model.flattened().unwrap();
        assert_eq!(flat.elements[0].faces["north"].texture, "block_glass");
        assert_eq!(flat.texture("block_glass"), Some("block/glass"));
    }

    #[test]
    fn a_turned_faces_texture_turns_with_it_unless_the_model_locks_its_uvs() {
        let side = resolve("block/template_glass_pane_side", &none).unwrap();
        let top = |model: &Model| model.elements[0].faces["up"].rotation;
        // The up face's texture follows a clockwise quarter turn of the block.
        assert_eq!(top(&side.turned(0, 90, false).unwrap()), 1);
        assert_eq!(top(&side.turned(0, 270, false).unwrap()), 3);
        assert_eq!(top(&side.turned(0, 90, true).unwrap()), 0);
        // The down face is seen from the other side, so it turns the other way.
        let bottom = |model: &Model| model.elements[0].faces["down"].rotation;
        assert_eq!(bottom(&side.turned(0, 90, false).unwrap()), 3);
        // A side face tipped onto the down face has its top pointing north: half a turn off.
        let tipped = side.turned(90, 0, false).unwrap();
        assert_eq!(tipped.elements[0].faces["down"].rotation, 2);
    }
}
