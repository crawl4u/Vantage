//! Vantage's menu design: the tokens every screen reads, small builders for the JSON-UI nodes the
//! host draws, and the shell (sidebar, header and content frame) pages are placed in.
//!
//! The crate has no engine dependency: it produces documents as JSON values, so every layout is
//! testable as data.

pub mod icons;
pub mod mods;
pub mod node;
pub mod options;
pub mod picker;
pub mod shell;
pub mod tokens;
