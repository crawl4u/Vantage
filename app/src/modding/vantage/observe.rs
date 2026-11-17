//! The player facts native Vantage reads, in the shapes its code expects. The client already
//! observes the player each frame (`gameplay`, `module_observations`); this copies those
//! observations over, field for field.

use mod_host::module_observations::ModuleObservations;
use vantage::host::player_state as guest;

/// One frame's observations, converted once and read by any number of host calls.
#[derive(Default)]
pub(super) struct Observed {
    pub motion: Option<guest::Motion>,
    pub vitals: Option<guest::Vitals>,
    pub world_time: Option<guest::WorldTime>,
    pub ping_ms: Option<u32>,
    pub world: Option<guest::WorldObservation>,
    pub effects: Option<guest::Effects>,
    pub equipment: Option<guest::Equipment>,
    pub sidebar: Option<guest::Sidebar>,
    /// Nearest first, as the client observed them.
    pub actors: Vec<guest::ObservedActor>,
    pub combat: Vec<guest::CombatEvent>,
    /// Newest received chat lines, oldest first.
    pub chat: Vec<guest::ChatMessage>,
}

fn item(item: &mod_host::PlayerStateItem) -> guest::Item {
    guest::Item {
        identifier: item.identifier.clone(),
        network_id: item.network_id,
        metadata: item.metadata,
        count: item.count,
        block: item.block,
        damage: item.damage,
        max_durability: item.max_durability,
    }
}

fn slot(slot: &mod_host::PlayerStateSlot) -> guest::Slot {
    guest::Slot {
        known: slot.known,
        item: slot.item.as_ref().map(item),
    }
}

fn actor(observed: &mod_host::ObservedActor) -> guest::ObservedActor {
    let a = &observed.actor;
    guest::ObservedActor {
        actor: guest::Actor {
            session: a.session,
            dimension: a.dimension,
            runtime_id: a.runtime_id,
            player: a.player,
            position: a.position,
            boxes: a.boxes.iter().map(|b| guest::Bounds { min: b.min, max: b.max }).collect(),
            name: a.name.clone(),
            identifier: a.identifier.clone(),
            yaw: a.yaw,
            pitch: a.pitch,
            pickable: a.pickable,
            hittable: a.hittable,
            look_origin: a.look_origin,
            look_direction: a.look_direction,
            invisible: a.invisible,
            sneaking: a.sneaking,
            hurt: a.hurt,
            health: a.health,
        },
        max_health: observed.max_health,
        unique_id: observed.unique_id,
    }
}

fn combat(event: &mod_host::CombatObservation) -> guest::CombatEvent {
    guest::CombatEvent {
        sequence: event.sequence,
        session: event.session,
        dimension: event.dimension,
        kind: match event.kind {
            mod_host::CombatKind::AttackAdmitted => guest::CombatKind::AttackAdmitted,
            mod_host::CombatKind::Hurt => guest::CombatKind::Hurt,
            mod_host::CombatKind::Death => guest::CombatKind::Death,
            mod_host::CombatKind::Totem => guest::CombatKind::Totem,
        },
        local: event.local,
        player: event.player,
        runtime_id: event.runtime_id,
        distance: event.distance,
        position: event.position,
    }
}

/// Converts the client's observations for this frame.
pub(super) fn convert(
    snapshot: Option<&mod_host::PlayerStateSnapshot>,
    world: Option<&mod_host::WorldObservation>,
    modules: &ModuleObservations,
    sidebar: Option<(String, Vec<(String, i32)>)>,
    chat: &[mod_host::ObservedChatMessage],
) -> Observed {
    Observed {
        motion: modules.motion.as_ref().map(|m| guest::Motion {
            session: m.session,
            dimension: m.dimension,
            yaw: m.yaw,
            pitch: m.pitch,
            velocity: m.velocity,
            on_ground: m.on_ground,
        }),
        vitals: modules.vitals.as_ref().map(|v| guest::Vitals {
            session: v.session,
            dimension: v.dimension,
            hunger: v.hunger,
            max_hunger: v.max_hunger,
            saturation: v.saturation,
            held_food: v.held_food.as_ref().map(|food| guest::Food {
                nutrition: food.nutrition,
                saturation_modifier: food.saturation_modifier,
            }),
        }),
        world_time: modules.world_time.as_ref().map(|t| guest::WorldTime {
            session: t.session,
            time: t.time,
            daylight_cycle: t.daylight_cycle,
        }),
        ping_ms: modules.ping_ms,
        world: world.map(|w| guest::WorldObservation {
            session: w.session,
            world: w.world.clone(),
            dimension: w.dimension,
            position: w.position,
            alive: w.alive,
            runtime_id: w.runtime_id,
        }),
        effects: snapshot.map(|s| guest::Effects {
            session: s.session,
            dimension: s.dimension,
            effects: s
                .effects
                .iter()
                .map(|e| guest::Effect {
                    effect_id: e.effect_id,
                    amplifier: e.amplifier,
                    remaining_ticks: e.remaining_ticks,
                    ambient: e.ambient,
                    particles: e.particles,
                })
                .collect(),
        }),
        equipment: snapshot.map(|s| guest::Equipment {
            session: s.session,
            dimension: s.dimension,
            selected_slot: s.selected_slot,
            inventory: s.inventory.iter().map(slot).collect(),
            armor: s.armor.iter().map(slot).collect(),
            offhand: slot(&s.offhand),
        }),
        chat: chat
            .iter()
            .map(|message| guest::ChatMessage { sequence: message.sequence, text: message.text.clone() })
            .collect(),
        actors: modules.actors.iter().map(actor).collect(),
        combat: modules.combat.iter().map(|recorded| combat(&recorded.event)).collect(),
        sidebar: sidebar.map(|(title, rows)| guest::Sidebar {
            title,
            rows: rows
                .into_iter()
                .map(|(label, score)| guest::SidebarRow { label, score })
                .collect(),
        }),
    }
}
