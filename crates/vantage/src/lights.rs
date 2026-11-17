//! Light spots for flat ground: a diagonal grid whose neighbours sit 5 blocks apart on both
//! X and Z (and 10 apart in a straight line), so no block is far from a light.

/// Grid step on each axis.
pub const STEP: i32 = 5;

/// Whether a block column gets a light: X and Z on the 5-block grid, and an even number of
/// steps between them, which gives the diagonal pattern.
pub fn is_spot(x: i32, z: i32) -> bool {
    x.rem_euclid(STEP) == 0 && z.rem_euclid(STEP) == 0 && (x.div_euclid(STEP) + z.div_euclid(STEP)).rem_euclid(2) == 0
}

/// Every spot within `reach` blocks (a circle) of `center` (X, Z).
pub fn spots(center: [i32; 2], reach: i32) -> Vec<[i32; 2]> {
    let mut out = Vec::new();
    let low = [(center[0] - reach).div_euclid(STEP) * STEP, (center[1] - reach).div_euclid(STEP) * STEP];
    let mut x = low[0];
    while x <= center[0] + reach {
        let mut z = low[1];
        while z <= center[1] + reach {
            let (dx, dz) = (x - center[0], z - center[1]);
            if dx * dx + dz * dz <= reach * reach && is_spot(x, z) {
                out.push([x, z]);
            }
            z += STEP;
        }
        x += STEP;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pattern_is_diagonal_five_and_five() {
        assert!(is_spot(0, 0) && is_spot(5, 5) && is_spot(10, 0) && is_spot(-5, 5) && is_spot(-10, -20));
        assert!(!is_spot(5, 0) && !is_spot(0, 5) && !is_spot(1, 1) && !is_spot(-5, 0));
        let near = spots([2, 2], 64);
        assert!(near.iter().all(|[x, z]| is_spot(*x, *z)));
        assert!(near.len() > 200 && near.len() < 1000, "{}", near.len());
    }
}
