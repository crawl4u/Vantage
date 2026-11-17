//! Spawners in the Nether. Blazes come from spawners in Nether fortresses; a spawner works
//! while a player is within 16 blocks of it, and puts mobs in a 9 × 3 × 9 box around it
//! (4 blocks to each side, from one block below to one above). The block alone does not say
//! which mob it spawns, so every spawner in the Nether is shown (bastions have magma cube
//! ones).

pub const BLOCK: &str = "minecraft:mob_spawner";
/// How far around you spawners are looked for, in blocks.
pub const SEARCH_RANGE: f32 = 96.0;
/// Spawners drawn at most (three drawings each).
pub const MAX_SHOWN: usize = 24;
/// The player must be this close for a spawner to work.
pub const ACTIVATION_RADIUS: f32 = 16.0;

/// The 9 × 3 × 9 spawn box's edges, from its lowest corner.
pub fn area() -> Vec<[f32; 3]> {
    crate::hitbox::cube().into_iter().map(|[x, y, z]| [x * 9.0, y * 3.0, z * 9.0]).collect()
}

/// The activation range around its centre: a sphere of `ACTIVATION_RADIUS` drawn as three
/// circles (flat, and two upright), as one path.
pub fn circle() -> Vec<[f32; 3]> {
    const SIDES: usize = 48;
    let r = ACTIVATION_RADIUS;
    let point = |i: usize| {
        let a = i as f32 / SIDES as f32 * std::f32::consts::TAU;
        (a.cos() * r, a.sin() * r)
    };
    let mut path = Vec::with_capacity(SIDES * 3 + 3);
    // Flat circle, from (r, 0, 0) back to it.
    for i in 0..=SIDES {
        let (c, s) = point(i);
        path.push([c, 0.0, s]);
    }
    // Upright circle in X/Y, starting and ending at (r, 0, 0).
    for i in 1..=SIDES {
        let (c, s) = point(i);
        path.push([c, s, 0.0]);
    }
    // Over to (0, 0, r) along the flat circle, then the upright circle in Z/Y.
    for i in 1..=SIDES / 4 {
        let (c, s) = point(i);
        path.push([c, 0.0, s]);
    }
    for i in 1..=SIDES {
        let (c, s) = point(i);
        path.push([0.0, s, c]);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_fit_the_host_extent() {
        assert!(area().iter().all(|p| p.iter().all(|v| (0.0..=9.0).contains(v))));
        let c = circle();
        assert!(c.iter().all(|p| ((p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt() - 16.0).abs() < 1e-3));
    }
}
