//! The standard block model parents Java ships, written as code.
//!
//! A pack's model usually names one of these as its `parent` and only sets texture variables, so
//! its geometry is a fact of the game rather than of the pack. Each parent is given here as the
//! JSON a model file would hold, for [`crate::java_model::resolve`] to follow like any pack model.
//! Shapes the client draws through meshing special cases that depend on neighbours, such as
//! stairs, doors and walls, are not here: they are rebuilt per connection elsewhere.

use serde_json::{Map, Value, json};

/// One face: its name, the UV rectangle (`None` for the face's own box), the texture variable it
/// draws and the quarter-turn angle of its texture in degrees.
type FaceSpec<'a> = (&'a str, Option<[f64; 4]>, &'a str, u32);

/// A box and the faces it shows.
fn cuboid(from: [f64; 3], to: [f64; 3], faces: &[FaceSpec]) -> Value {
    let faces: Map<String, Value> = faces
        .iter()
        .map(|(name, uv, texture, turn)| {
            let mut face = json!({ "texture": format!("#{texture}") });
            if let Some(uv) = uv {
                face["uv"] = json!(uv);
            }
            if *turn != 0 {
                face["rotation"] = json!(turn);
            }
            ((*name).to_owned(), face)
        })
        .collect();
    json!({ "from": from, "to": to, "faces": faces })
}

/// The same box turned `angle` degrees about `axis` through `origin`.
fn turned(mut element: Value, origin: [f64; 3], axis: &str, angle: f64) -> Value {
    element["rotation"] = json!({ "origin": origin, "axis": axis, "angle": angle });
    element
}

/// Six faces of a box, each drawing one texture variable with a UV rectangle.
fn sides<'a>(texture: &'a str, uvs: [[f64; 4]; 6]) -> [FaceSpec<'a>; 6] {
    let names = ["down", "up", "north", "south", "west", "east"];
    std::array::from_fn(|index| (names[index], Some(uvs[index]), texture, 0))
}

/// The six faces of a cube drawing the variables named after them.
fn cube_faces(turns: [u32; 6]) -> Vec<FaceSpec<'static>> {
    ["down", "up", "north", "south", "west", "east"]
        .into_iter()
        .zip(turns)
        .map(|(name, turn)| (name, None, name, turn))
        .collect()
}

/// The standard parent `name` (as `block/<name>`) with its texture variables and elements.
pub(crate) fn parent(name: &str) -> Option<Value> {
    const FULL: [f64; 4] = [0.0, 0.0, 16.0, 16.0];
    let slab = |from: [f64; 3], to: [f64; 3], side: [f64; 4]| {
        let faces = [
            ("down", Some(FULL), "bottom", 0),
            ("up", Some(FULL), "top", 0),
            ("north", Some(side), "side", 0),
            ("south", Some(side), "side", 0),
            ("west", Some(side), "side", 0),
            ("east", Some(side), "side", 0),
        ];
        json!({ "textures": { "particle": "#side" }, "elements": [cuboid(from, to, &faces)] })
    };
    let flat = |from: [f64; 3], to: [f64; 3], texture: &str, inset: f64, edge: [f64; 4]| {
        let top = [inset, inset, 16.0 - inset, 16.0 - inset];
        let faces = sides(
            texture,
            [
                top,
                top,
                edge,
                edge,
                [edge[0], edge[1], edge[2], edge[3]],
                edge,
            ],
        );
        json!({ "textures": { "particle": format!("#{texture}") },
            "elements": [cuboid(from, to, &faces)] })
    };
    let trapdoor = |from: [f64; 3], to: [f64; 3], uvs: [[f64; 4]; 6], turns: [u32; 6]| {
        let mut faces = sides("texture", uvs);
        for (face, turn) in faces.iter_mut().zip(turns) {
            face.3 = turn;
        }
        json!({ "textures": { "particle": "#texture" }, "elements": [cuboid(from, to, &faces)] })
    };
    let [t, u] = [[0.0, 0.0, 16.0, 16.0], [0.0, 16.0, 16.0, 0.0]];
    let (edge, open_edge) = ([0.0, 16.0, 16.0, 13.0], [0.0, 0.0, 16.0, 3.0]);
    Some(match name {
        "block/slab" => slab([0.0; 3], [16.0, 8.0, 16.0], [0.0, 8.0, 16.0, 16.0]),
        "block/slab_top" => slab([0.0, 8.0, 0.0], [16.0; 3], [0.0, 0.0, 16.0, 8.0]),
        "block/thin_block" => json!({}),
        // Flat item models carry no geometry: they draw their `layer0` texture as a sprite.
        "item/generated" | "item/handheld" | "item/handheld_rod" | "builtin/generated" => {
            json!({})
        }
        "block/carpet" => json!({ "parent": "block/thin_block",
            "textures": { "particle": "#wool" }, "elements": [cuboid(
                [0.0; 3], [16.0, 1.0, 16.0], &sides("wool", [
                    FULL, FULL, [0.0, 15.0, 16.0, 16.0], [0.0, 15.0, 16.0, 16.0],
                    [0.0, 15.0, 16.0, 16.0], [0.0, 15.0, 16.0, 16.0]]))] }),
        "block/pressure_plate_up" => flat(
            [1.0, 0.0, 1.0],
            [15.0, 1.0, 15.0],
            "texture",
            1.0,
            [1.0, 15.0, 15.0, 16.0],
        ),
        "block/pressure_plate_down" => flat(
            [1.0, 0.0, 1.0],
            [15.0, 0.5, 15.0],
            "texture",
            1.0,
            [1.0, 15.0, 15.0, 15.5],
        ),
        "block/button" | "block/button_pressed" => {
            let pressed = name.ends_with("pressed");
            let (height, edge) = if pressed { (1.02, 15.0) } else { (2.0, 16.0) };
            let (wide, narrow) = ([5.0, 14.0, 11.0, edge], [6.0, 14.0, 10.0, edge]);
            let plan = [5.0, 6.0, 11.0, 10.0];
            let faces = sides("texture", [plan, plan, wide, wide, narrow, narrow]);
            json!({ "textures": { "particle": "#texture" },
                "elements": [cuboid([5.0, 0.0, 6.0], [11.0, height, 10.0], &faces)] })
        }
        "block/template_trapdoor_bottom" => trapdoor(
            [0.0; 3],
            [16.0, 3.0, 16.0],
            [t, t, edge, edge, edge, edge],
            [0; 6],
        ),
        "block/template_trapdoor_top" => trapdoor(
            [0.0, 13.0, 0.0],
            [16.0; 3],
            [t, t, edge, edge, edge, edge],
            [0; 6],
        ),
        "block/template_trapdoor_open" => trapdoor(
            [0.0, 0.0, 13.0],
            [16.0; 3],
            [
                [0.0, 13.0, 16.0, 16.0],
                [0.0, 16.0, 16.0, 13.0],
                t,
                t,
                [16.0, 0.0, 13.0, 16.0],
                [13.0, 0.0, 16.0, 16.0],
            ],
            [0; 6],
        ),
        "block/template_orientable_trapdoor_bottom" => trapdoor(
            [0.0; 3],
            [16.0, 3.0, 16.0],
            [t, u, open_edge, open_edge, open_edge, open_edge],
            [0; 6],
        ),
        "block/template_orientable_trapdoor_top" => trapdoor(
            [0.0, 13.0, 0.0],
            [16.0; 3],
            [t, u, open_edge, open_edge, open_edge, open_edge],
            [0; 6],
        ),
        "block/template_orientable_trapdoor_open" => trapdoor(
            [0.0, 0.0, 13.0],
            [16.0; 3],
            [
                open_edge,
                [0.0, 3.0, 16.0, 0.0],
                u,
                u,
                open_edge,
                [0.0, 3.0, 16.0, 0.0],
            ],
            [0, 0, 0, 0, 90, 90],
        ),
        "block/crop" => {
            let plane = |from: [f64; 3], to: [f64; 3], first: &str, second: &str, flip: bool| {
                let (a, b) = if flip {
                    ([16.0, 0.0, 0.0, 16.0], FULL)
                } else {
                    (FULL, [16.0, 0.0, 0.0, 16.0])
                };
                cuboid(
                    from,
                    to,
                    &[(first, Some(a), "crop", 0), (second, Some(b), "crop", 0)],
                )
            };
            json!({ "textures": { "particle": "#crop" }, "elements": [
                plane([4.0, -1.0, 0.0], [4.0, 15.0, 16.0], "west", "east", false),
                plane([12.0, -1.0, 0.0], [12.0, 15.0, 16.0], "west", "east", true),
                plane([0.0, -1.0, 4.0], [16.0, 15.0, 4.0], "north", "south", false),
                plane([0.0, -1.0, 12.0], [16.0, 15.0, 12.0], "north", "south", true)] })
        }
        "block/leaves" => json!({ "textures": { "particle": "#all" }, "elements": [cuboid(
            [0.0; 3], [16.0; 3], &sides("all", [FULL; 6]))] }),
        "block/cube_mirrored" => {
            let mirrored = [16.0, 0.0, 0.0, 16.0];
            let faces: Vec<FaceSpec> = ["down", "up", "north", "south", "west", "east"]
                .into_iter()
                .map(|name| (name, Some(mirrored), name, 0))
                .collect();
            json!({ "elements": [cuboid([0.0; 3], [16.0; 3], &faces)] })
        }
        "block/cube_mirrored_all" => json!({ "parent": "block/cube_mirrored", "textures": {
            "particle": "#all", "down": "#all", "up": "#all", "north": "#all",
            "east": "#all", "south": "#all", "west": "#all" } }),
        "block/cube_column_mirrored" => json!({ "parent": "block/cube_mirrored", "textures": {
            "particle": "#side", "down": "#end", "up": "#end", "north": "#side",
            "east": "#side", "south": "#side", "west": "#side" } }),
        "block/cube_column_horizontal" => json!({ "elements": [cuboid(
            [0.0; 3], [16.0; 3], &cube_faces([0, 180, 0, 0, 0, 0]))],
            "textures": { "particle": "#side", "down": "#end", "up": "#end", "north": "#side",
                "east": "#side", "south": "#side", "west": "#side" } }),
        "block/cube_directional" => json!({ "elements": [cuboid(
            [0.0; 3], [16.0; 3], &cube_faces([180, 0, 0, 0, 270, 90]))] }),
        "block/cube_top" => json!({ "parent": "block/cube", "textures": {
            "particle": "#side", "down": "#side", "up": "#top", "north": "#side",
            "east": "#side", "south": "#side", "west": "#side" } }),
        "block/orientable_with_bottom" => json!({ "parent": "block/cube", "textures": {
            "particle": "#front", "down": "#bottom", "up": "#top", "north": "#front",
            "east": "#side", "south": "#side", "west": "#side" } }),
        "block/orientable" => json!({ "parent": "block/orientable_with_bottom",
            "textures": { "bottom": "#top" } }),
        "block/orientable_vertical" => json!({ "parent": "block/cube", "textures": {
            "particle": "#side", "down": "#side", "up": "#front", "north": "#side",
            "east": "#side", "south": "#side", "west": "#side" } }),
        "block/template_lantern" | "block/template_hanging_lantern" => {
            let hanging = name.contains("hanging");
            // The lamp, its cap, and the two crossed planes of the handle.
            let (lift, hook, reach) = if hanging {
                (1.0, 11.0, 4.0)
            } else {
                (0.0, 9.0, 2.0)
            };
            let all_sides = |uv: [f64; 4]| {
                ["north", "south", "west", "east"].map(|n| (n, Some(uv), "lantern", 0))
            };
            let lamp_ends = [0.0, 9.0, 6.0, 15.0];
            let cap_ends = [1.0, 10.0, 5.0, 14.0];
            let mut lamp = vec![
                ("down", Some(lamp_ends), "lantern", 0),
                ("up", Some(lamp_ends), "lantern", 0),
            ];
            lamp.extend(all_sides([0.0, 2.0, 6.0, 9.0]));
            let mut cap = vec![("up", Some(cap_ends), "lantern", 0)];
            if hanging {
                cap.insert(0, ("down", Some(cap_ends), "lantern", 0));
            }
            cap.extend(all_sides([1.0, 0.0, 5.0, 2.0]));
            let plane = |from: [f64; 3], to: [f64; 3], pair: [&str; 2], uv: [f64; 4]| {
                let swapped = [uv[2], uv[1], uv[0], uv[3]];
                turned(
                    cuboid(
                        from,
                        to,
                        &[
                            (pair[0], Some(uv), "lantern", 0),
                            (pair[1], Some(swapped), "lantern", 0),
                        ],
                    ),
                    [8.0; 3],
                    "y",
                    45.0,
                )
            };
            let (low, high) = if hanging { (6.0, 12.0) } else { (10.0, 12.0) };
            json!({ "textures": { "particle": "#lantern" }, "elements": [
                cuboid([5.0, lift, 5.0], [11.0, 7.0 + lift, 11.0], &lamp),
                cuboid([6.0, 7.0 + lift, 6.0], [10.0, 9.0 + lift, 10.0], &cap),
                plane([6.5, hook, 8.0], [9.5, hook + reach, 8.0], ["north", "south"],
                    [14.0, 1.0, 11.0, 1.0 + reach]),
                plane([8.0, hook - if hanging { 1.0 } else { 0.0 }, 6.5],
                    [8.0, hook - if hanging { 1.0 } else { 0.0 } + if hanging { 6.0 } else { 2.0 }, 9.5],
                    ["west", "east"], [14.0, low, 11.0, high])] })
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slab_is_half_a_block_and_its_top_variant_the_other_half() {
        let bottom = parent("block/slab").unwrap();
        let top = parent("block/slab_top").unwrap();
        assert_eq!(bottom["elements"][0]["to"], json!([16.0, 8.0, 16.0]));
        assert_eq!(top["elements"][0]["from"], json!([0.0, 8.0, 0.0]));
        assert_eq!(
            bottom["elements"][0]["faces"]["south"]["uv"],
            json!([0.0, 8.0, 16.0, 16.0])
        );
    }

    #[test]
    fn unknown_parents_are_not_invented() {
        assert!(parent("block/stairs").is_none());
        assert!(parent("block/not_a_model").is_none());
    }

    #[test]
    fn parents_resolve_through_pack_models_with_their_textures_and_shapes() {
        use crate::java_model::resolve;
        let pack = |name: &str| match name {
            "block/my_lantern" => Some(json!({ "parent": "block/template_lantern",
                "textures": { "lantern": "block/lantern" } })),
            "block/my_furnace" => Some(json!({ "parent": "block/orientable",
                "textures": { "top": "block/t", "front": "block/f", "side": "block/s" } })),
            _ => None,
        };
        let lantern = resolve("block/my_lantern", &pack).unwrap();
        assert_eq!(lantern.elements.len(), 4);
        assert_eq!(
            lantern
                .elements
                .iter()
                .filter(|e| e.rotation.is_some())
                .count(),
            2
        );
        let furnace = resolve("block/my_furnace", &pack).unwrap();
        assert_eq!(
            furnace.texture("down"),
            Some("block/t"),
            "orientable draws its bottom as the top"
        );
        assert_eq!(furnace.texture("north"), Some("block/f"));
        assert_eq!(furnace.elements.len(), 1);
        let hanging = resolve("block/template_hanging_lantern", &pack).unwrap();
        assert_eq!(hanging.elements[0].from, [5.0, 1.0, 5.0]);
    }
}
