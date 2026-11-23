//! Vantage's menu toolkit. Widgets build a [`scene::Scene`] of plain drawing primitives from a layout; nothing
//! here depends on the game engine, so every page is testable as data. See `docs/vantage-gui.md`.

pub mod anim;
pub mod geom;
pub mod layout;
pub mod scene;
pub mod style;
pub mod tess;
