//! Runs the whole Vantage guest natively against a recording host.

use crate::guest::Mod;
use crate::host::{self, environment, hud, player_state, settings};
use mod_api::handled_player_mod::Guest;

#[derive(Default)]
struct Recorder {
    surfaces: Vec<String>,
    patches: usize,
    saved: Vec<String>,
    fullbright: Vec<bool>,
    load: Option<String>,
}

impl hud::Api for Recorder {
    fn set_surface(&mut self, json: &str) -> Result<(), String> {
        self.surfaces.push(json.to_owned());
        Ok(())
    }
    fn update_surface(&mut self, _json: &str) -> Result<(), String> {
        self.patches += 1;
        Ok(())
    }
    fn read_viewport(&mut self) -> Option<hud::Viewport> {
        Some(hud::Viewport { width: 427.0, height: 240.0, scale: 3.0 })
    }
    fn measure_text(&mut self, text: &str, _font: hud::Font) -> Result<f32, String> {
        Ok(text.chars().count() as f32 * 6.0)
    }
}

impl environment::Api for Recorder {
    fn set_fullbright(&mut self, enabled: bool) -> Result<(), String> {
        self.fullbright.push(enabled);
        Ok(())
    }
}

impl settings::Api for Recorder {
    fn load(&mut self) -> Result<String, String> {
        self.load.clone().ok_or_else(|| "none".to_owned())
    }

    fn save(&mut self, json: &str) -> Result<(), String> {
        self.saved.push(json.to_owned());
        Ok(())
    }
}

impl player_state::Api for Recorder {
    fn read_fps(&mut self) -> Result<Option<f32>, String> {
        Ok(Some(144.0))
    }
}

macro_rules! refuse {
    ($($module:ident),*) => {$(impl host::$module::Api for Recorder {})*};
}
refuse!(action_filter, camera, clock, events, facts, gameplay, input, item_use, panel, presentation, render, screen, sound_handling, teleport_handling, terrain, touch, budget, drawing, masks);

#[test]
fn the_whole_guest_runs_natively_and_draws_its_default_hud() {
    let mut recorder = Recorder::default();
    host::scope(&mut recorder, || {
        Mod::init();
        for _ in 0..8 {
            Mod::frame();
        }
    });
    let first = recorder.surfaces.first().expect("the guest published a HUD surface");
    let value: serde_json::Value = serde_json::from_str(first).expect("the surface is JSON");
    assert!(value.get("screen").is_some() && value.get("document").is_some());
}

#[test]
fn fullbright_in_the_settings_reaches_the_host() {
    let mut recorder = Recorder { load: Some("{\"fullbright\":true}".to_owned()), ..Default::default() };
    host::scope(&mut recorder, || {
        Mod::init();
        for _ in 0..8 {
            Mod::frame();
        }
    });
    assert_eq!(recorder.fullbright.first(), Some(&true), "calls: {:?}", recorder.fullbright);
}
