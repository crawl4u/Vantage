//! Sharing part of a setup: which mods a share code carries.
//!
//! A person can leave any mod out of the codes they make (the Sharing page). A left-out mod's settings are
//! written as the defaults, which costs next to nothing, and the code names the mods it leaves out, so a load
//! keeps the loader's own settings for those. Everything a mod owns is listed here once, and both sides use
//! the same `copy`: making a code copies the defaults over the left-out mods, loading one copies the
//! loader's own over them.
//!
//! What is not a mod's travels as before: the HUD editor's guides. The menu key, the saved setups, the
//! starred cards, the colour picker's lists and the waypoints never travel in a code.

use crate::config::{Element, Settings, element_keys, page_keys};

/// Every mod, by the id of its card's switch. A code names a left-out mod by its place in this list, and the
/// settings file keeps the same numbers (`Settings::share_skip`), so only ever add to the end.
pub const MODS: [&str; 64] = [
    "hud_keystrokes",
    "hud_cps",
    "hud_fps",
    "zoom_on",
    "hud_armor",
    "hud_coords",
    "hud_potion",
    "hud_ping",
    "ch_on",
    "fullbright",
    "hud_facing",
    "gp_time_on",
    "gp_weather_on",
    "hud_food",
    "hud_speed",
    "hud_stopwatch",
    "hud_playtime",
    "hud_clock",
    "hud_day",
    "hud_viewangles",
    "hud_reach",
    "hud_combo",
    "hud_potioncount",
    "hud_totems",
    "hud_itemcounter",
    "hud_inventory",
    "hud_arraylist",
    "hud_chunk",
    "hud_slime",
    "hud_monument",
    "hud_scoreboard",
    "hud_target",
    "hud_waila",
    "hud_height",
    "hud_subtitles",
    "hud_pvp",
    "borders",
    "subchunks",
    "slime_chunks",
    "guardians",
    "lights",
    "blaze",
    "hitboxes",
    "gp_nametags",
    "gp_hit_on",
    "gp_blur_on",
    "gp_dmg_on",
    "gp_wp_on",
    "gp_no_hurt_cam",
    "gp_toggle_sprint",
    "gp_toggle_sneak",
    "gp_always_sprint",
    "gp_cursor_center",
    "item_delay_fix",
    "gp_nocam_on",
    "gp_noswing_on",
    "gp_cpslim_on",
    "gp_bo_on",
    "gp_mk_on",
    "gp_tnt_on",
    "gp_th_on",
    "gp_hive_on",
    "gp_persp_on",
    "gp_glass_on",
];

/// A mod's number: its place in `MODS`.
pub fn position(id: &str) -> Option<usize> {
    MODS.iter().position(|m| *m == id)
}

/// The page that holds a mod's own options (`config::page_keys` lists them), for the mods that have one.
/// Zoom and the Crosshair keep theirs in one value each, which `copy` moves whole.
fn page(id: &str) -> Option<&'static str> {
    Some(match id {
        "borders" => "Chunk borders",
        "hitboxes" => "Hitboxes",
        "gp_hit_on" => "Hit color",
        "gp_time_on" => "Time changer",
        "gp_weather_on" => "Weather changer",
        "gp_nametags" => "Nametags",
        "gp_no_hurt_cam" => "Hurt camera",
        "gp_blur_on" => "Menu blur",
        "gp_wp_on" => "Waypoint settings",
        "gp_dmg_on" => "Damage indicator",
        "gp_cpslim_on" => "CPS limiter",
        "gp_bo_on" => "Block outline",
        "gp_mk_on" => "Markers",
        "gp_th_on" => "Text hotkey",
        "gp_hive_on" => "Hive",
        "gp_persp_on" => "Perspective",
        "gp_glass_on" => "Glass",
        _ => return None,
    })
}

/// The HUD element a mod draws: its look, its chroma and its own options go with the mod.
fn element(id: &str) -> Option<Element> {
    if id == "gp_wp_on" { Some(Element::Waypoints) } else { Element::from_toggle_id(id).filter(|e| *e != Element::Waypoints && *e != Element::Arrows) }
}

/// The key slots a mod owns besides the one of its own switch (which has the mod's id).
fn keys(id: &str) -> &'static [&'static str] {
    match id {
        "hud_stopwatch" => &["sw_pause", "sw_reset"],
        "hud_scoreboard" => &["hide_scoreboard"],
        "hud_waila" => &["waila_details"],
        "gp_wp_on" => &["hud_waypoints", "wp_add", "wp_remove", "wp_undo", "gp_wp_hidden", "wp_quick", "wp_menu"],
        "gp_mk_on" => &["mk_place", "mk_clear", "mk_danger"],
        "gp_th_on" => &["th_key0", "th_key1", "th_key2", "th_key3"],
        "gp_hive_on" => &["hive_rq"],
        "gp_persp_on" => &["persp_cycle", "persp_behind", "persp_free", "persp_snap", "persp_snap_fwd"],
        _ => &[],
    }
}

/// Makes everything mod `id` owns in `to` what it is in `from`: whether it is on, its element's look and
/// chroma, its options and its keys. Nothing else is touched. An id that is not a mod's does nothing.
#[inline(never)]
pub fn copy(to: &mut Settings, from: &Settings, id: &str) {
    if position(id).is_none() {
        return;
    }
    let on = from.toggle(id);
    if let Some(switch) = to.toggle_mut(id) {
        *switch = on;
    }
    if let Some(element) = element(id) {
        let at = element.index();
        if at < to.elements.len() && at < from.elements.len() {
            to.elements[at] = from.elements[at].clone();
        }
        let chroma = from.gameplay.chroma(at);
        if to.gameplay.chroma(at) != chroma {
            to.gameplay.set_chroma_on(at, chroma.on);
            to.gameplay.set_chroma_speed(at, chroma.speed);
            to.gameplay.set_chroma_type(at, usize::from(!chroma.wave));
        }
        to.reset_fields(from, element_keys(element));
    }
    match id {
        "zoom_on" => to.zoom = from.zoom.clone(),
        "ch_on" => to.crosshair = from.crosshair.clone(),
        _ => {}
    }
    if let Some(page) = page(id) {
        to.reset_fields(from, page_keys(page));
    }
    for slot in std::iter::once(id).chain(keys(id).iter().copied()) {
        let key = from.key(slot).to_owned();
        to.set_key(slot, &key);
    }
}

/// The numbers of a list of left-out mods as kept in the settings and written in a code: each once, in
/// order, and only numbers this version knows.
pub fn clean(skip: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = skip.iter().copied().filter(|n| usize::from(*n) < MODS.len()).collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The whole of `copy` for a list of left-out mods at once (the game spreads this over frames, see
/// `setups.rs`): `to` gets `from`'s settings for each of them.
#[cfg(test)]
pub fn copy_all(to: &mut Settings, from: &Settings, skip: &[u8]) {
    for n in clean(skip) {
        copy(to, from, MODS[usize::from(n)]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_the_mods_grid() {
        let mut ours: Vec<&str> = MODS.to_vec();
        let mut grid: Vec<&str> = crate::menu_controls::MODS.iter().map(|m| m.0).collect();
        ours.sort_unstable();
        grid.sort_unstable();
        assert_eq!(ours, grid, "a card without a place in share::MODS (add it to the END), or the other way round");
        ours.dedup();
        assert_eq!(ours.len(), MODS.len());
    }

    #[test]
    fn every_mod_has_a_switch_and_every_key_slot_one_owner() {
        let mut settings = Settings::default();
        for id in MODS {
            assert!(settings.toggle_mut(id).is_some(), "{id} has no switch");
        }
        for i in 0..crate::config::KEY_SLOTS {
            let slot = crate::config::key_slot_id(i).unwrap();
            let owners = MODS.iter().filter(|id| **id == slot || keys(id).contains(&slot)).count();
            // The retired Arrow counter's slot belongs to no mod.
            assert_eq!(owners, usize::from(slot != "hud_arrows"), "{slot}");
        }
    }

    #[test]
    fn a_page_or_an_element_belongs_to_one_mod() {
        let pages: Vec<&str> = MODS.iter().filter_map(|id| page(id)).collect();
        let mut once = pages.clone();
        once.sort_unstable();
        once.dedup();
        assert_eq!(once.len(), pages.len());
        assert!(pages.iter().all(|p| !page_keys(p).is_empty()), "a page name that `page_keys` does not know");
        let elements: Vec<usize> = MODS.iter().filter_map(|id| element(id)).map(Element::index).collect();
        let mut once = elements.clone();
        once.sort_unstable();
        once.dedup();
        assert_eq!(once.len(), elements.len());
        // Every element but the retired Arrow counter goes with a mod.
        assert_eq!(elements.len(), Element::COUNT - 1);
    }

    #[test]
    fn copying_one_mod_leaves_the_others_alone() {
        let mut from = Settings::default();
        from.set_key("gp_glass_on", "KeyG");
        from.set_key("mk_place", "KeyM");
        from.gameplay.glass_on = true;
        from.gameplay.glass_mode = crate::glass::CLEAR;
        from.gameplay.mk_on = true;
        from.gameplay.mk_time = 4;
        from.style_mut(Element::Fps).scale = 2.0;
        let mut to = Settings::default();
        copy(&mut to, &from, "gp_glass_on");
        assert!(to.gameplay.glass_on && to.gameplay.glass_mode == crate::glass::CLEAR && to.key("gp_glass_on") == "KeyG");
        assert!(!to.gameplay.mk_on && to.gameplay.mk_time == Settings::default().gameplay.mk_time && to.key("mk_place").is_empty());
        assert_eq!(to.style(Element::Fps).scale, Settings::default().style(Element::Fps).scale);
        copy(&mut to, &from, "hud_fps");
        copy(&mut to, &from, "gp_mk_on");
        assert_eq!(to, from);
        // And back: the defaults over one mod take all of it away again.
        copy(&mut to, &Settings::default(), "gp_mk_on");
        assert!(!to.gameplay.mk_on && to.key("mk_place").is_empty() && to.gameplay.glass_on);
    }

    #[test]
    fn a_list_of_left_out_mods_is_kept_clean() {
        assert_eq!(clean(&[5, 200, 5, 1]), vec![1, 5]);
    }
}
