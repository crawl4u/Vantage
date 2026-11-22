//! Vantage for Cinnabar: keystrokes, CPS, FPS, coordinates, direction, chunk info, slime
//! chunk and guardian HUD elements (each placed, sized, coloured and switched on and off on its
//! own), plus chunk borders, slime chunks, guardian spawn columns, light levels, mob spawn spots,
//! hitboxes, fullbright and scoreboard toggles and a smooth zoom (a port of Zoomify).
//!
//! Everything except the host glue in `guest` is plain Rust, unit-tested on the host.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

pub mod binds;
pub mod combat;
pub mod blockdata;
pub mod blocks;
pub mod host;
pub mod config;
pub mod cps;
pub mod fuel;
pub mod damage;
pub mod crosshair;
pub mod crosshair_config;
pub mod gameplay;
pub mod handlers;
pub mod hive;
pub mod glass;
pub mod hitbox;
pub mod lights;
pub mod spawners;
pub mod hud;
pub mod outline;
pub mod json;
pub mod monument;
pub mod menu;
pub mod menu_v2;
pub mod menu_link;
pub mod mods_core;
pub mod menu_controls;
pub mod menu_tabset;
pub mod panel;
pub mod potion;
pub mod reset_fields;
pub mod saturation;
pub mod shapes;
pub mod code;
pub mod setups;
pub mod share;
pub mod slime;
pub mod subtitles;
pub mod timers;
pub mod transition;
pub mod waypoint_ui;
pub mod waila;
pub mod waypoints;
pub mod zoom;
pub mod zoom_config;

#[cfg(target_arch = "wasm32")]
mod arena;
/// Natively there is no fuel to save, so receiving a region needs no scratch arena.
#[cfg(not(target_arch = "wasm32"))]
mod arena {
    pub fn receiving<T>(receive: impl FnOnce() -> T) -> T {
        receive()
    }
    pub fn reset() {}
}
pub mod armor;
pub mod guest;
pub use guest::{Mod, block_outline_on, handler_config};
pub mod hive_host;

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOCATOR: arena::Arena = arena::Arena;

/// The menu with default settings, embedded in the component as its panel bundle.
pub fn menu_panel() -> String {
    panel::build(&config::Settings::default(), &panel::PanelState::default())
}

/// The menu font and icons, which the client rasterizes for the panel.
pub const UI_FONT: &[u8] = include_bytes!("../assets/vantage-ui.ttf");

/// The bundle route of the menu's surface.
pub const MENU_ROUTE: &str = menu::ROUTE;

#[cfg(test)]
mod native_tests;
