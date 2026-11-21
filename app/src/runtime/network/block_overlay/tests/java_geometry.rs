//! A Java model's geometry, compiled by the block geometry compiler, puts every texture corner
//! where Java puts it: the translator and the compiler agree on mirrored X, on the up and down
//! faces' opposite start corner and on face rotations.

use java_resource_pack::java_model::{Element, Face, Model, to_geometry};
use pack_compiler::block_geometry::{FACE_NAMES, parse_geometry_file};

type Vector = [f32; 3];

/// A face's right and up directions seen from outside it, and so its outward normal.
fn view(face: &str) -> (Vector, Vector) {
    match face {
        "north" => ([-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        "south" => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        "east" => ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        "west" => ([0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        "up" => ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        _ => ([-1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
    }
}

/// Where Java puts the texture rectangle `uv` at `corner` of an unturned face.
fn java_uv(face: &str, corner: Vector, from: Vector, to: Vector, uv: [f32; 4]) -> [f32; 2] {
    let [u1, v1, u2, v2] = uv;
    let (at_from, at_to) = (
        |axis: usize| corner[axis] == from[axis],
        |axis: usize| corner[axis] == to[axis],
    );
    let pick = |first: bool, a: f32, b: f32| if first { a } else { b };
    match face {
        "north" => [pick(at_to(0), u1, u2), pick(at_to(1), v1, v2)],
        "south" => [pick(at_from(0), u1, u2), pick(at_to(1), v1, v2)],
        "west" => [pick(at_from(2), u1, u2), pick(at_to(1), v1, v2)],
        "east" => [pick(at_to(2), u1, u2), pick(at_to(1), v1, v2)],
        "up" => [pick(at_from(0), u1, u2), pick(at_from(2), v1, v2)],
        _ => [pick(at_from(0), u1, u2), pick(at_to(2), v1, v2)],
    }
}

fn dot(a: Vector, b: Vector) -> f32 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}

#[test]
fn texture_corners_land_where_java_puts_them() {
    let (from, to): (Vector, Vector) = ([2.0, 3.0, 5.0], [11.0, 9.0, 14.0]);
    let mut faces = std::collections::BTreeMap::new();
    let names = ["north", "east", "south", "west", "up", "down"];
    for (index, name) in names.into_iter().enumerate() {
        // Distinct rectangles, and every face turned a different number of quarters.
        let base = 1.0 + index as f64;
        faces.insert(
            name,
            Face {
                texture: "t".to_owned(),
                uv: Some([base, base + 2.0, base + 7.0, base + 12.0]),
                tint: false,
                rotation: index as u8 % 4,
            },
        );
    }
    let model = Model {
        textures: [("t".to_owned(), "block/t".to_owned())].into(),
        elements: vec![Element {
            from: from.map(f64::from),
            to: to.map(f64::from),
            rotation: None,
            faces,
        }],
    };
    let document = to_geometry("geometry.test", &model).to_string();
    let geometry = parse_geometry_file(document.as_bytes())
        .into_iter()
        .next()
        .expect("the geometry compiles")
        .1;
    let quads = geometry.cubes[0].quads();
    assert_eq!(quads.len(), 6);
    for (quad, _) in quads {
        let name = FACE_NAMES[quad.face];
        let index = names.iter().position(|face| *face == name).unwrap();
        let base = 1.0 + index as f32;
        let uv = [base, base + 2.0, base + 7.0, base + 12.0];
        let rotation = index % 4;
        // Corners counter-clockwise seen from outside, by their angle about the face centre.
        let (right, up) = view(name);
        let centre = quad.positions.iter().fold([0.0; 3], |sum, corner| {
            std::array::from_fn(|axis| sum[axis] + corner[axis] / 4.0)
        });
        let mut order: Vec<usize> = (0..4).collect();
        let angle = |corner: usize| {
            let offset: Vector =
                std::array::from_fn(|axis| quad.positions[corner][axis] - centre[axis]);
            dot(offset, up).atan2(dot(offset, right))
        };
        order.sort_by(|&a, &b| angle(a).total_cmp(&angle(b)));
        for (slot, &corner) in order.iter().enumerate() {
            // A quarter turn hands each corner the texture of the next one counter-clockwise.
            let source = quad.positions[order[(slot + rotation) % 4]];
            let expected = java_uv(name, source, from, to, uv);
            let actual = quad.uvs[corner];
            assert!(
                (expected[0] - actual[0]).abs() < 1e-3 && (expected[1] - actual[1]).abs() < 1e-3,
                "{name} corner {:?}: expected {expected:?}, got {actual:?}",
                quad.positions[corner],
            );
        }
        // The face sits where the Java box puts it.
        let sits = quad
            .positions
            .iter()
            .all(|corner| (0..3).all(|axis| (from[axis]..=to[axis]).contains(&corner[axis])));
        assert!(sits, "{name} lies inside the Java box");
    }
}
