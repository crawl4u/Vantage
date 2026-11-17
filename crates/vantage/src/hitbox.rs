//! Hitboxes: a box around every nearby mob and player, and the way it faces, as F3+B shows
//! them. Plain maths, so it is tested on the host.

/// A unit cube's 12 edges as one path (three short stretches are drawn twice).
pub fn cube() -> Vec<[f32; 3]> {
    let c = |i: usize| -> [f32; 3] {
        // 0-3 bottom, 4-7 top, counter-clockwise from the origin.
        let [x, z] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]][i % 4];
        [x, if i >= 4 { 1.0 } else { 0.0 }, z]
    };
    [0, 1, 2, 3, 0, 4, 5, 1, 5, 6, 2, 6, 7, 3, 7, 4].map(c).to_vec()
}

/// A one-block line along -Z, the way an actor faces in its body frame.
pub fn ray() -> Vec<[f32; 3]> {
    vec![[0.0, 0.0, 0.0], [0.0, 0.0, -1.0]]
}

/// The rotation (x, y, z, w) turning the `ray` (along -Z) to where an actor looks, from its
/// actor YXZ angles in degrees: yaw turns left, pitch turns up, and 0, 0 looks along -Z.
pub fn look_rotation(yaw: f64, pitch: f64) -> (f32, f32, f32, f32) {
    let (sy, cy) = (yaw.to_radians() * 0.5).sin_cos();
    let (sp, cp) = (pitch.to_radians() * 0.5).sin_cos();
    ((cy * sp) as f32, (sy * cp) as f32, (-sy * sp) as f32, (cy * cp) as f32)
}

/// How far the look line reaches, in blocks.
pub const LOOK_LENGTH: f32 = 2.0;

/// One box to draw: its corner in the world and its size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Boxed {
    pub min: [f64; 3],
    pub size: [f32; 3],
}

/// The world boxes of an actor at `position`. Boxes far from the actor are taken as relative
/// to its feet, so either form the host reports draws in the right place.
pub fn boxes(position: [f32; 3], bounds: &[([f32; 3], [f32; 3])]) -> Vec<Boxed> {
    bounds
        .iter()
        .filter_map(|(min, max)| {
            let size = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
            if !size.iter().all(|s| s.is_finite() && *s > 0.0 && *s < 64.0) {
                return None;
            }
            let centre = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5, (min[2] + max[2]) * 0.5];
            let far = (0..3).any(|i| (centre[i] - position[i]).abs() > 8.0);
            let base = if far { position } else { [0.0; 3] };
            Some(Boxed { min: [0, 1, 2].map(|i| f64::from(base[i] + min[i])), size })
        })
        .collect()
}

/// How far an attack from you reaches in survival, in blocks: Highlight Players in Range uses it.
pub const HIT_RANGE: f64 = 3.0;

/// The distance in blocks from `eye` to the nearest point of any of `boxes`; none without boxes.
pub fn distance_to(eye: [f64; 3], boxes: &[Boxed]) -> Option<f64> {
    boxes
        .iter()
        .map(|b| {
            let gap = |i: usize| {
                let (low, high) = (b.min[i], b.min[i] + f64::from(b.size[i]));
                (low - eye[i]).max(eye[i] - high).max(0.0)
            };
            (gap(0).powi(2) + gap(1).powi(2) + gap(2).powi(2)).sqrt()
        })
        .min_by(f64::total_cmp)
}

/// Where a rotation sends a point, for tests.
#[cfg(test)]
fn turn(q: (f32, f32, f32, f32), v: [f32; 3]) -> [f32; 3] {
    let (x, y, z, w) = q;
    // v + 2w(u × v) + 2u × (u × v)
    let u = [x, y, z];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let t = cross(u, v).map(|c| 2.0 * c);
    let ut = cross(u, t);
    [0, 1, 2].map(|i| v[i] + w * t[i] + ut[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_look_line_points_where_you_face() {
        let tip = ray()[1];
        let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4);
        // Yaw 0 is north (-Z); yaw 180 south, the Bedrock view yaw 0.
        assert!(close(turn(look_rotation(0.0, 0.0), tip), [0.0, 0.0, -1.0]));
        assert!(close(turn(look_rotation(180.0, 0.0), tip), [0.0, 0.0, 1.0]));
        // Yaw turns left: from north, 90 looks west (-X).
        assert!(close(turn(look_rotation(90.0, 0.0), tip), [-1.0, 0.0, 0.0]));
        // Pitch up looks up.
        assert!(close(turn(look_rotation(0.0, 90.0), tip), [0.0, 1.0, 0.0]));
        let d = turn(look_rotation(90.0, 45.0), tip);
        assert!(close(d, [-0.70710677, 0.70710677, 0.0]), "{d:?}");
        assert_eq!(mod_api::bedrock_view_angles(0.0, 0.0)[0], 180.0);
    }

    #[test]
    fn the_distance_is_to_the_nearest_point_of_the_box() {
        let player = boxes([10.0, 64.0, 10.0], &[([9.7, 64.0, 9.7], [10.3, 65.8, 10.3])]);
        // Level with the box and 2 blocks off its side: 2 - 0.3 = 1.7.
        let d = distance_to([8.0, 64.5, 10.0], &player).unwrap();
        assert!((d - 1.7).abs() < 1e-4, "{d}");
        // Inside it: 0. Above it, past the head: the height only.
        assert_eq!(distance_to([10.0, 65.0, 10.0], &player), Some(0.0));
        assert!((distance_to([10.0, 66.8, 10.0], &player).unwrap() - 1.0).abs() < 1e-4);
        // A diagonal gap counts on every axis.
        let d = distance_to([13.3, 66.8, 10.0], &player).unwrap();
        assert!((d - (3.0f64 * 3.0 + 1.0).sqrt()).abs() < 1e-4, "{d}");
        assert_eq!(distance_to([0.0; 3], &[]), None);
        assert!(distance_to([13.4, 64.5, 10.0], &player).unwrap() > HIT_RANGE, "3.1 blocks from the side is out of range");
        assert!(distance_to([13.2, 64.5, 10.0], &player).unwrap() < HIT_RANGE);
    }

    #[test]
    fn the_cube_path_covers_all_twelve_edges() {
        let path = cube();
        let mut edges = std::collections::HashSet::new();
        for pair in path.windows(2) {
            let key = |p: [f32; 3]| (p[0] as i32, p[1] as i32, p[2] as i32);
            let (a, b) = (key(pair[0]), key(pair[1]));
            let differ = (a.0 != b.0) as u8 + (a.1 != b.1) as u8 + (a.2 != b.2) as u8;
            assert_eq!(differ, 1, "every step is one edge");
            edges.insert(if a < b { (a, b) } else { (b, a) });
        }
        assert_eq!(edges.len(), 12);
    }

    #[test]
    fn boxes_in_world_or_relative_land_on_the_actor() {
        let world = boxes([100.5, 64.0, -20.5], &[([100.2, 64.0, -20.8], [100.8, 65.8, -20.2])]);
        let relative = boxes([100.5, 64.0, -20.5], &[([-0.3, 0.0, -0.3], [0.3, 1.8, 0.3])]);
        assert_eq!(world.len(), 1);
        assert!((world[0].min[0] - 100.2).abs() < 1e-4 && (relative[0].min[0] - 100.2).abs() < 1e-4);
        assert!((world[0].size[1] - 1.8).abs() < 1e-4);
        assert!(boxes([0.0; 3], &[([0.0; 3], [0.0; 3])]).is_empty(), "empty boxes are skipped");
    }
}
