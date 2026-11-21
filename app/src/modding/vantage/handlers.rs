//! Offers native Vantage the client's sound and teleport events, the way a component host would:
//! No cam reset (`handle-teleport`), No swing sounds and Audio subtitles (`handle-sound`).
use client_audio::audio::engine::SoundEvent;
use mod_api::handled_player_mod::Guest;
use semantic_input::action_filter as input;
use vantage::host::{
    action_filter as actions, sound_handling as sound, teleport_handling as teleport,
};

/// The identity the semantic router sees for native Vantage's action filter.
pub(crate) const ACTION_OWNER: u64 = 1 << 62;

/// Whether the CPS limiter is on, so the client offers it every action transition.
pub(crate) fn wants_actions() -> bool {
    vantage::handler_config().cps_limit.is_some()
}

/// One verdict per transition from native Vantage's action filter (the CPS limiter).
pub(crate) fn filter_actions(frame: &input::ActionFilterFrame) -> input::ActionVerdict {
    let held = |value: [bool; 2]| actions::Held {
        attack: value[0],
        item_use: value[1],
    };
    let verdict = vantage::Mod::filter_actions(actions::ActionFrame {
        epoch: frame.epoch,
        time_us: frame.time_us,
        physical: held(frame.physical),
        logical: held(frame.logical),
        transitions: frame
            .transitions
            .iter()
            .map(|t| actions::Transition {
                held_after: t.held_after,
                action: match t.action {
                    input::FilterAction::Attack => actions::FilteredAction::Attack,
                    input::FilterAction::ItemUse => actions::FilteredAction::ItemUse,
                },
                kind: match t.kind {
                    input::TransitionKind::Press => actions::TransitionKind::Press,
                    input::TransitionKind::Release => actions::TransitionKind::Release,
                    input::TransitionKind::Repeat => actions::TransitionKind::Repeat,
                },
            })
            .collect(),
    });
    input::ActionVerdict {
        accepted: verdict.accepted,
        release: [verdict.release.attack, verdict.release.item_use],
    }
}

/// The rotation Vantage keeps for a local MovePlayer, or none while No cam reset is off.
pub(crate) fn teleport_rotation(
    rotation: crate::runtime::world::MovePlayerRotation,
) -> Option<[f32; 2]> {
    if !vantage::handler_config().no_cam_reset {
        return None;
    }
    let degrees = |[yaw, pitch]: [f32; 2]| teleport::Rotation { yaw, pitch };
    let handled = vantage::Mod::handle_teleport(teleport::TeleportContext {
        mode: match rotation.mode {
            protocol::MovePlayerMode::Normal => teleport::TeleportMode::Normal,
            protocol::MovePlayerMode::Reset => teleport::TeleportMode::Reset,
            protocol::MovePlayerMode::Teleport => teleport::TeleportMode::Teleport,
            protocol::MovePlayerMode::Rotation => teleport::TeleportMode::Rotation,
            protocol::MovePlayerMode::Unknown(_) => teleport::TeleportMode::Other,
        },
        server: degrees(rotation.server),
        presented: degrees(rotation.presented),
        rotation: degrees(rotation.server),
    });
    Some([handled.rotation.yaw, handled.rotation.pitch])
}

/// Whether Vantage wants to see this frame's sounds (No swing sounds or Audio subtitles is on).
pub(crate) fn wants_sounds() -> bool {
    let config = vantage::handler_config();
    config.no_swing_sounds || config.subtitles
}

/// Offers bounded requests in order; excess or oversized requests play unhandled.
pub(crate) fn filter_sounds(observations: &[SoundEvent]) -> Vec<bool> {
    let mut accepted = vec![true; observations.len()];
    let selected = observations
        .iter()
        .enumerate()
        .filter(|(_, event)| {
            event.name.len() <= mod_api::MAX_SOUND_NAME_BYTES
                && event
                    .position
                    .is_none_or(|p| p.iter().all(|v| v.is_finite()))
        })
        .take(mod_api::MAX_HANDLED_SOUNDS);
    for (index, event) in selected {
        let handled = vantage::Mod::handle_sound(sound::SoundContext {
            name: event.name.to_string(),
            position: event.position.map(|p| sound::Position {
                x: p[0],
                y: p[1],
                z: p[2],
            }),
            looping: event.looping,
            cancelled: false,
        });
        accepted[index] = !handled.cancelled;
    }
    accepted
}
