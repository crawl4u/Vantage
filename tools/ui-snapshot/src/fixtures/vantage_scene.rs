//! A demonstration of the first-party scene painter: gradients, per-corner radii, borders, shadows and text.

use std::sync::Arc;

use ui::mod_panel::Panel;
use vantage_gui::{
    geom::Rect,
    scene::{Align, Primitive, Scene},
    style::{Border, Color, Fill, Radii, Shadow},
};

use super::Fixture;

fn rect(rect: Rect, fill: Fill, radii: Radii) -> Primitive {
    Primitive::Rect { rect, fill, radii, border: None, shadow: None }
}

/// A scene of the shapes the menu needs.
fn scene() -> Scene {
    let mut scene = Scene::new();
    let ink = Color::hex(0xE3E8FA, 1.0);
    scene.push(Primitive::Rect {
        rect: Rect::new(40.0, 40.0, 560.0, 280.0),
        fill: Fill::Gradient { from: Color::hex(0x1B2340, 0.95), to: Color::hex(0x0F1424, 0.95), angle: 0.0 },
        radii: Radii::all(28.0),
        border: Some(Border { width: 1.0, color: Color::hex(0xFFFFFF, 0.10) }),
        shadow: Some(Shadow { offset: [0.0, 8.0], blur: 24.0, color: Color::hex(0x000000, 0.45) }),
    });
    // A card whose name bar has square top corners and round bottom ones.
    scene.push(Primitive::Rect {
        rect: Rect::new(70.0, 70.0, 150.0, 120.0),
        fill: Fill::Solid(Color::hex(0x1E2740, 1.0)),
        radii: Radii::all(16.0),
        border: None,
        shadow: Some(Shadow { offset: [0.0, 4.0], blur: 10.0, color: Color::hex(0x000000, 0.4) }),
    });
    scene.push(rect(Rect::new(70.0, 160.0, 150.0, 30.0), Fill::Solid(Color::hex(0x2459F5, 1.0)), Radii::bottom(16.0)));
    scene.push(Primitive::Text {
        rect: Rect::new(70.0, 166.0, 150.0, 20.0),
        text: "Keystrokes".to_owned(),
        size: 11.0,
        color: Color::hex(0xFFFFFF, 1.0),
        align: Align::Centre,
        font: "mod_panel_display",
    });
    scene.push(rect(Rect::new(236.0, 70.0, 150.0, 120.0), Fill::Solid(Color::hex(0x1E2740, 1.0)), Radii::all(16.0)));
    scene.push(rect(Rect::new(236.0, 160.0, 150.0, 30.0), Fill::Solid(Color::hex(0x303851, 1.0)), Radii::bottom(16.0)));
    // A gradient chip, a pill switch and a status dot.
    scene.push(rect(
        Rect::new(70.0, 215.0, 110.0, 30.0),
        Fill::Gradient { from: Color::hex(0x2B66FF, 1.0), to: Color::hex(0x1E3FD8, 1.0), angle: 90.0 },
        Radii::all(10.0),
    ));
    scene.push(rect(Rect::new(460.0, 215.0, 52.0, 28.0), Fill::Solid(Color::hex(0x2459F5, 1.0)), Radii::all(14.0)));
    scene.push(rect(Rect::new(488.0, 219.0, 20.0, 20.0), Fill::Solid(Color::hex(0xFFFFFF, 1.0)), Radii::all(10.0)));
    scene.push(rect(Rect::new(358.0, 78.0, 9.0, 9.0), Fill::Solid(Color::hex(0x2459F5, 1.0)), Radii::all(4.5)));
    scene.push(Primitive::Text {
        rect: Rect::new(70.0, 262.0, 400.0, 24.0),
        text: "Poppins text drawn by the scene painter".to_owned(),
        size: 13.0,
        color: ink,
        align: Align::Start,
        font: "mod_panel_display",
    });
    scene
}

/// The demonstration panel.
pub(super) fn demo() -> Fixture {
    let json = serde_json::json!({
        "reference_size": [640.0, 360.0],
        "fill": true,
        "controls": [],
        "surface": {
            "screen": "vantage.scene",
            "document": serde_json::json!({
                "namespace": "vantage",
                "scene": {"type": "panel", "size": ["100%", "100%"], "controls": [
                    {"draw": {"type": "custom", "renderer": "vantage_scene", "size": ["100%", "100%"]}}
                ]},
            }).to_string(),
        },
    });
    let mut panel: Panel = serde_json::from_value(json).expect("the demo panel parses");
    panel.scene = Some(Arc::new(scene()));
    Fixture::ModPanel(Box::new(panel), None, None)
}
