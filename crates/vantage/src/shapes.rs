//! Local-space line and quad shapes, uploaded once and placed with world anchors. The host
//! rejects coordinates more than 64 blocks from a shape's origin, so tall lines are stacked.

pub type Point = [f32; 3];

/// One 16-block-tall slice of a chunk's four walls with a line every 2 blocks both ways,
/// drawn as one continuous path. Retraced stretches only overlap lines already drawn.
pub fn chunk_wall() -> Vec<Point> {
    // Integer maths only: float remainder is a slow library routine in wasm.
    let perimeter = |step: usize| -> [f32; 2] {
        let distance = (step * 2) % 64;
        let [x, z] = match distance {
            d if d < 16 => [d, 0],
            d if d < 32 => [16, d - 16],
            d if d < 48 => [48 - d, 16],
            d => [0, 64 - d],
        };
        [x as f32, z as f32]
    };
    let mut points = Vec::with_capacity(110);
    // Vertical lines, joined alternately along the bottom and the top edge.
    for step in 0..=32 {
        let [x, z] = perimeter(step);
        let (first, second) = if step % 2 == 0 { (0.0, 16.0) } else { (16.0, 0.0) };
        points.push([x, first, z]);
        points.push([x, second, z]);
    }
    // The path ends at the starting corner, at the bottom; add a ring every 2 blocks.
    for level in (0..16).step_by(2) {
        let y = level as f32;
        points.push([0.0, y, 0.0]);
        points.push([16.0, y, 0.0]);
        points.push([16.0, y, 16.0]);
        points.push([0.0, y, 16.0]);
        points.push([0.0, y, 0.0]);
    }
    points
}

/// A straight vertical line.
pub fn post(height: f32) -> Vec<Point> {
    vec![[0.0, 0.0, 0.0], [0.0, height, 0.0]]
}

/// A square outline on the ground, drawn as a closed path.
pub fn square(side: f32) -> Vec<Point> {
    vec![[0.0, 0.0, 0.0], [side, 0.0, 0.0], [side, 0.0, side], [0.0, 0.0, side]]
}

/// A square outline centred on its origin: the host limits shapes to 64 blocks from their
/// origin in a straight line, so a monument-sized square has to sit around its centre.
pub fn centred_square(side: f32) -> Vec<Point> {
    let half = side / 2.0;
    vec![[-half, 0.0, -half], [half, 0.0, -half], [half, 0.0, half], [-half, 0.0, half]]
}

/// Whether every point stays inside the host's per-shape extent.
#[cfg(test)]
pub fn fits(points: &[Point]) -> bool {
    points.iter().all(|p| p.iter().all(|v| v.abs() <= 64.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_fit_the_host_extent_and_budget() {
        let wall = chunk_wall();
        assert!(fits(&wall));
        assert!(fits(&post(64.0)));
        assert!(fits(&square(16.0)));
        // Every point within 64 blocks of the origin, the host's real limit.
        for shape in [chunk_wall(), post(64.0), square(16.0), centred_square(58.0), post(22.0)] {
            assert!(shape.iter().all(|p| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt() <= 64.0));
        }
        // Expanded vertices: 30 per segment; the wall stays a small slice of the budget.
        assert!((wall.len() - 1) * 30 < 4000, "{}", wall.len());
        // The path reaches every corner of the chunk at the bottom and the top.
        for corner in [[0.0, 0.0], [16.0, 0.0], [16.0, 16.0], [0.0, 16.0]] {
            for y in [0.0, 16.0] {
                assert!(wall.contains(&[corner[0], y, corner[1]]), "{corner:?} {y}");
            }
        }
    }
}
