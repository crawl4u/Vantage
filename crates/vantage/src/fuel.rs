//! The fuel guard's rules: how much of a frame's fuel must be left before each optional stage of the frame starts.
//! The host call (`budget::remaining-fuel`) is in `hive_host`; this is plain Rust, tested natively.
//!
//! A frame callback gets `FRAME_FUEL` (1,000,000, a world-entry frame four times that). A callback that uses it all
//! traps and the mod is quarantined until it is re-instantiated, which is what the Mods grid and the Crosshair page
//! did before. A stage that is skipped only waits for the next frame, so the frame skips the optional stages it can
//! no longer pay for and always ends with the HUD.

/// Fuel of one ordinary personal-mod frame (`mod_host::FRAME_FUEL`).
pub const FRAME_FUEL: u64 = 1_000_000;

/// The optional stages of a frame, in the order they run.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    /// Chunk borders, slime chunks, lights, spawners and terrain scans.
    World,
    /// Targets, damage, TNT, waypoints, WAILA and the highlight.
    Targets,
    /// The inventory read (armor) and the potion effects.
    Inventory,
    /// Uploading and moving drawings.
    Drawings,
}

/// The fuel that must be left to start `stage`: what the stage and everything after it, down to the HUD, may need.
/// These are estimates (no frame has been measured), so they err towards skipping.
pub const fn reserve(stage: Stage) -> u64 {
    match stage {
        Stage::World => 450_000,
        Stage::Targets => 300_000,
        Stage::Inventory => 200_000,
        Stage::Drawings => 120_000,
    }
}

/// Whether a frame with `left` fuel may start `stage`. A world-entry frame (four times the fuel) always may.
pub const fn allows(left: u64, stage: Stage) -> bool {
    left >= reserve(stage)
}

/// The reserves only make sense while each later stage needs less than the one before it.
const _: () = {
    assert!(reserve(Stage::World) > reserve(Stage::Targets));
    assert!(reserve(Stage::Targets) > reserve(Stage::Inventory));
    assert!(reserve(Stage::Inventory) > reserve(Stage::Drawings));
    assert!(reserve(Stage::World) < FRAME_FUEL);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_frame_runs_every_stage() {
        for stage in [Stage::World, Stage::Targets, Stage::Inventory, Stage::Drawings] {
            assert!(allows(FRAME_FUEL, stage), "{stage:?}");
        }
    }

    #[test]
    fn a_spent_frame_skips_the_early_stages_first() {
        let left = 250_000;
        assert!(!allows(left, Stage::World));
        assert!(!allows(left, Stage::Targets));
        assert!(allows(left, Stage::Inventory));
        assert!(allows(left, Stage::Drawings));
    }

    #[test]
    fn an_empty_frame_runs_nothing_optional() {
        for stage in [Stage::World, Stage::Targets, Stage::Inventory, Stage::Drawings] {
            assert!(!allows(0, stage), "{stage:?}");
        }
    }

    #[test]
    fn a_world_entry_frame_always_runs_every_stage() {
        for stage in [Stage::World, Stage::Targets, Stage::Inventory, Stage::Drawings] {
            assert!(allows(4 * FRAME_FUEL, stage), "{stage:?}");
        }
    }
}
