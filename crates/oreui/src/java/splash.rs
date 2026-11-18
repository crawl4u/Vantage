//! The title's splash, drawn as Java draws it: one line of Java's own splash list in yellow, turned
//! 20 degrees, pulsing once a second, its size following its length. The line is picked once per
//! run from the lines Java's font can draw.

use std::sync::OnceLock;

use ui_engine::engine::screen::PaintError;

use super::font;
use crate::{images::java, paint::Canvas};

/// The splash's yellow.
const YELLOW: [u8; 4] = [255, 255, 0, 255];
/// The splash is turned 20 degrees, rising to the right.
const ANGLE: f32 = -20.0 * std::f32::consts::PI / 180.0;

/// The line shown this run: a random one among those Java's font can draw, none without the list.
fn pick() -> Option<&'static str> {
    static PICK: OnceLock<Option<String>> = OnceLock::new();
    PICK.get_or_init(|| {
        let font = java::font()?;
        let lines: Vec<&String> = java::splashes()
            .iter()
            .filter(|line| font::width_with(font, line).is_some())
            .collect();
        if lines.is_empty() {
            return None;
        }
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.subsec_nanos() as usize ^ elapsed.as_secs() as usize);
        Some(lines[nanos % lines.len()].clone())
    })
    .as_deref()
}

/// The scale of the splash `seconds` into the menu: the pulse, then shrunk so a long line is no
/// larger than a short one, as the game does it. `width` is the line's width in Java pixels.
pub(super) fn scale(seconds: f64, width: f32) -> f32 {
    let phase = (seconds.rem_euclid(1.0) as f32) * std::f32::consts::TAU;
    (1.8 - phase.sin().abs() * 0.1) * 100.0 / (width + 32.0)
}

/// Draws the splash about `pivot` (logical pixels) at `unit` logical pixels per Java pixel.
pub(super) fn draw(canvas: &mut Canvas<'_>, pivot: [f32; 2], unit: f32) -> Result<(), PaintError> {
    let (Some(line), Some(java_font)) = (pick(), java::font()) else {
        return Ok(());
    };
    let Some(width) = font::width_with(java_font, line) else {
        return Ok(());
    };
    let scale = scale(canvas.seconds, width) * unit;
    font::draw_turned(canvas, line, pivot, scale, ANGLE, YELLOW)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pulse_runs_between_one_point_seven_and_one_point_eight_before_shrinking() {
        let width = 68.0;
        let shrink = 100.0 / (width + 32.0);
        // At the start of each second the sine is zero, so the splash is at its full size.
        assert!((scale(0.0, width) - 1.8 * shrink).abs() < 1e-6);
        assert!((scale(3.0, width) - 1.8 * shrink).abs() < 1e-6);
        // A quarter of the way round the sine is at its peak: the smallest size.
        assert!((scale(0.25, width) - 1.7 * shrink).abs() < 1e-5);
        for tick in 0..200 {
            let value = scale(f64::from(tick) / 37.0, width) / shrink;
            assert!((1.7 - 1e-5..=1.8 + 1e-5).contains(&value), "{value}");
        }
    }

    #[test]
    fn a_longer_line_is_drawn_smaller() {
        assert!(scale(0.0, 200.0) < scale(0.0, 40.0));
    }
}
