//! How each mod's card icon is drawn. This table is the one place that says which mods get a rendered preview,
//! which get a converted icon and which keep their glyph of the icon font; `docs/mod-icons.md` explains the rule
//! a new mod follows and a test fails until every mod has an entry here.

use vantage_gui::icon_art::Art;

/// How a mod's card icon is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A small render of the mod's own output, drawn by `gui::previews`.
    Preview,
    /// An icon converted from the OneClient-style set.
    Art(Art),
    /// The mod's glyph of the icon font.
    Glyph,
}

/// Every mod, in `MODS` order, with how its icon is drawn.
pub const KINDS: [(&str, Kind); 64] = [
    ("hud_keystrokes", Kind::Preview),
    ("hud_cps", Kind::Preview),
    ("hud_fps", Kind::Preview),
    ("zoom_on", Kind::Glyph),
    ("hud_armor", Kind::Preview),
    ("hud_coords", Kind::Preview),
    ("hud_potion", Kind::Preview),
    ("hud_ping", Kind::Preview),
    ("ch_on", Kind::Preview),
    ("fullbright", Kind::Art(Art::Sun)),
    ("hud_facing", Kind::Preview),
    ("gp_time_on", Kind::Art(Art::Moon)),
    ("gp_weather_on", Kind::Art(Art::Cloud)),
    ("hud_food", Kind::Preview),
    ("hud_speed", Kind::Preview),
    ("hud_stopwatch", Kind::Preview),
    ("hud_playtime", Kind::Preview),
    ("hud_clock", Kind::Preview),
    ("hud_day", Kind::Preview),
    ("hud_viewangles", Kind::Preview),
    ("hud_reach", Kind::Preview),
    ("hud_combo", Kind::Preview),
    ("hud_potioncount", Kind::Preview),
    ("hud_totems", Kind::Preview),
    ("hud_itemcounter", Kind::Preview),
    ("hud_inventory", Kind::Preview),
    ("hud_arraylist", Kind::Preview),
    ("hud_chunk", Kind::Preview),
    ("hud_slime", Kind::Preview),
    ("hud_monument", Kind::Preview),
    ("hud_scoreboard", Kind::Preview),
    ("hud_target", Kind::Preview),
    ("hud_waila", Kind::Preview),
    ("hud_height", Kind::Preview),
    ("hud_subtitles", Kind::Preview),
    ("hud_pvp", Kind::Preview),
    ("borders", Kind::Glyph),
    ("subchunks", Kind::Art(Art::Layers)),
    ("slime_chunks", Kind::Glyph),
    ("guardians", Kind::Glyph),
    ("lights", Kind::Glyph),
    ("blaze", Kind::Glyph),
    ("hitboxes", Kind::Art(Art::SelectAll)),
    ("gp_nametags", Kind::Glyph),
    ("gp_hit_on", Kind::Glyph),
    ("gp_blur_on", Kind::Glyph),
    ("gp_dmg_on", Kind::Art(Art::Lightning01)),
    ("gp_wp_on", Kind::Glyph),
    ("gp_no_hurt_cam", Kind::Glyph),
    ("gp_toggle_sprint", Kind::Glyph),
    ("gp_toggle_sneak", Kind::Glyph),
    ("gp_always_sprint", Kind::Glyph),
    ("gp_cursor_center", Kind::Glyph),
    ("item_delay_fix", Kind::Art(Art::Qol)),
    ("gp_nocam_on", Kind::Art(Art::Refresh)),
    ("gp_noswing_on", Kind::Glyph),
    ("gp_cpslim_on", Kind::Glyph),
    ("gp_bo_on", Kind::Glyph),
    ("gp_mk_on", Kind::Glyph),
    ("gp_tnt_on", Kind::Glyph),
    ("gp_th_on", Kind::Art(Art::TextInput)),
    ("gp_hive_on", Kind::Art(Art::Honeycomb)),
    ("gp_persp_on", Kind::Glyph),
    ("gp_glass_on", Kind::Glyph),
];

/// How mod `id`'s card icon is drawn: the glyph of the icon font for a mod with no entry.
#[must_use]
pub fn kind(id: &str) -> Kind {
    KINDS.iter().find(|(k, _)| *k == id).map_or(Kind::Glyph, |(_, kind)| *kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu_controls::MODS;

    #[test]
    fn every_mod_has_exactly_one_entry_in_mod_order() {
        let ids: Vec<&str> = MODS.iter().map(|m| m.0).collect();
        let listed: Vec<&str> = KINDS.iter().map(|k| k.0).collect();
        assert_eq!(listed, ids, "add the new mod to mod_icons::KINDS (see docs/mod-icons.md)");
    }

    #[test]
    fn hud_mods_have_a_preview_and_the_other_mods_do_not() {
        // A mod in the HUD category shows its own output on the card, so it needs a preview.
        for (id, name, category) in MODS {
            let preview = kind(id) == Kind::Preview;
            let hud = category == 0;
            let crosshair = id == "ch_on";
            assert_eq!(preview, hud || crosshair, "{name}: previews are for HUD mods and the crosshair (docs/mod-icons.md)");
        }
    }
}
