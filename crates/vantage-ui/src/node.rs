//! Builders for the JSON-UI nodes the host draws. Each returns one control as `{name: node}`, the
//! form a `controls` list holds, with absolute offsets from its parent's top left corner.

use serde_json::{Map, Value, json};

use crate::tokens::Rgba;

/// The label font Cinnabar draws authored panels with.
pub const FONT: &str = "mod_panel_sans";

/// How a label sits in its box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

impl Align {
    fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
}

fn named(name: &str, node: Value) -> Value {
    let mut control = Map::new();
    control.insert(name.to_owned(), node);
    Value::Object(control)
}

fn anchored(mut node: Map<String, Value>) -> Value {
    node.insert("anchor_from".into(), json!("top_left"));
    node.insert("anchor_to".into(), json!("top_left"));
    Value::Object(node)
}

fn base(size: [f32; 2], offset: [f32; 2]) -> Map<String, Value> {
    let mut node = Map::new();
    node.insert("size".into(), json!(size));
    node.insert("offset".into(), json!(offset));
    node
}

/// A filled rounded rectangle.
#[must_use]
pub fn rounded(name: &str, offset: [f32; 2], size: [f32; 2], radius: f32, color: Rgba) -> Value {
    let mut node = base(size, offset);
    node.insert("type".into(), json!("custom"));
    node.insert("renderer".into(), json!("cinnabar_rounded_rectangle"));
    node.insert("radius".into(), json!(radius));
    node.insert("color".into(), json!(color));
    named(name, anchored(node))
}

/// A rounded rectangle outline over nothing: only its border shows.
#[must_use]
pub fn outline(
    name: &str,
    offset: [f32; 2],
    size: [f32; 2],
    radius: f32,
    width: f32,
    color: Rgba,
) -> Value {
    let mut node = base(size, offset);
    node.insert("type".into(), json!("custom"));
    node.insert("renderer".into(), json!("cinnabar_rounded_rectangle"));
    node.insert("radius".into(), json!(radius));
    node.insert("color".into(), json!([0.0, 0.0, 0.0, 0.0]));
    node.insert("border_width".into(), json!(width));
    node.insert("border_color".into(), json!(color));
    named(name, anchored(node))
}

/// A text label. `scale` is the font scale factor.
#[must_use]
pub fn label(
    name: &str,
    text: &str,
    offset: [f32; 2],
    size: [f32; 2],
    color: Rgba,
    scale: f32,
    align: Align,
) -> Value {
    let mut node = base(size, offset);
    node.insert("type".into(), json!("label"));
    node.insert("text".into(), json!(text));
    node.insert("localize".into(), json!(false));
    node.insert("font_type".into(), json!(FONT));
    node.insert("font_scale_factor".into(), json!(scale));
    node.insert("color".into(), json!(color));
    node.insert("text_alignment".into(), json!(align.name()));
    named(name, anchored(node))
}

/// A plain container holding `controls`.
#[must_use]
pub fn panel(name: &str, offset: [f32; 2], size: [f32; 2], controls: Vec<Value>) -> Value {
    let mut node = base(size, offset);
    node.insert("type".into(), json!("panel"));
    node.insert("controls".into(), Value::Array(controls));
    named(name, anchored(node))
}

/// A container that cuts what it holds to its box.
#[must_use]
pub fn clip(name: &str, offset: [f32; 2], size: [f32; 2], controls: Vec<Value>) -> Value {
    let mut node = base(size, offset);
    node.insert("type".into(), json!("panel"));
    node.insert("clip_children".into(), json!(true));
    node.insert("controls".into(), Value::Array(controls));
    named(name, anchored(node))
}

/// A button over `controls` that sends `press` (for example `mod.close`) when pressed.
#[must_use]
pub fn button(
    name: &str,
    offset: [f32; 2],
    size: [f32; 2],
    press: &str,
    controls: Vec<Value>,
) -> Value {
    let mut node = base(size, offset);
    node.insert("type".into(), json!("button"));
    node.insert("controls".into(), Value::Array(controls));
    node.insert(
        "button_mappings".into(),
        json!([{
            "from_button_id": "button.menu_select",
            "to_button_id": press,
            "mapping_type": "pressed",
        }]),
    );
    named(name, anchored(node))
}

/// A horizontal colour fade from `from` to `to`.
#[must_use]
pub fn fade(name: &str, offset: [f32; 2], size: [f32; 2], from: Rgba, to: Rgba) -> Value {
    let mut node = base(size, offset);
    node.insert("type".into(), json!("custom"));
    node.insert("renderer".into(), json!("gradient_renderer"));
    node.insert("color1".into(), json!(from));
    node.insert("color2".into(), json!(to));
    node.insert("gradient_direction".into(), json!("horizontal"));
    named(name, anchored(node))
}

/// `control` with its `bindings` list set (replacing any it had).
#[must_use]
pub fn bound(mut control: Value, bindings: Value) -> Value {
    let name = control
        .as_object()
        .and_then(|map| map.keys().next().cloned())
        .expect("a control holds one node");
    control[name.as_str()]["bindings"] = bindings;
    control
}

/// A label whose text is the host binding `binding` (such as `#row_3_value`).
#[must_use]
pub fn bound_label(
    name: &str,
    binding: &str,
    offset: [f32; 2],
    size: [f32; 2],
    color: Rgba,
    scale: f32,
    align: Align,
) -> Value {
    let mut control = label(name, binding, offset, size, color, scale, align);
    control[name]["bindings"] = serde_json::json!([{"binding_name": binding}]);
    control
}

/// A vertically scrolling view of `content`, `content_height` tall, with the host's scrollbar. The
/// view is `size` wide and tall; the content is 8 narrower so the bar has room.
#[must_use]
pub fn scroll_view(
    name: &str,
    offset: [f32; 2],
    size: [f32; 2],
    content: Vec<Value>,
    content_height: f32,
) -> Value {
    let [width, height] = size;
    let mut node = base(size, offset);
    node.insert("type".into(), json!("scroll_view"));
    node.insert("clip_children".into(), json!(true));
    node.insert("scroll_speed".into(), json!(24));
    node.insert("scroll_content".into(), json!("content"));
    node.insert("scroll_view_port".into(), json!("viewport"));
    node.insert("scrollbar_track".into(), json!("track"));
    node.insert("scrollbar_box".into(), json!("thumb"));
    node.insert("scroll_box_and_track_panel".into(), json!("bar"));
    node.insert("scrollbar_track_button".into(), json!("mod.scroll_track"));
    node.insert(
        "controls".into(),
        json!([
            {"viewport": {
                "type": "panel", "size": [width - 8.0, height], "clip_children": true,
                "anchor_from": "top_left", "anchor_to": "top_left",
                "controls": [{"content": {
                    "type": "panel", "size": [width - 8.0, content_height],
                    "anchor_from": "top_left", "anchor_to": "top_left",
                    "controls": content,
                }}],
            }},
            {"bar": {
                "type": "panel", "size": [4, height],
                "anchor_from": "top_right", "anchor_to": "top_right",
                "controls": [
                    {"track": {
                        "type": "scroll_track", "size": [4, height],
                        "anchor_from": "top_left", "anchor_to": "top_left",
                        "button_mappings": [{
                            "from_button_id": "button.menu_select",
                            "mapping_type": "pressed",
                            "to_button_id": "mod.scroll_track",
                        }],
                        "controls": [],
                    }},
                    {"thumb": {
                        "type": "scrollbar_box", "size": [4, 20], "draggable": "vertical",
                        "anchor_from": "top_left", "anchor_to": "top_left",
                        "controls": [{"fill": {
                            "type": "custom", "renderer": "cinnabar_rounded_rectangle",
                            "size": ["100%", "100%"], "radius": 1,
                            "color": [0.6, 0.62, 0.6, 0.5],
                        }}],
                    }},
                ],
            }},
        ]),
    );
    named(name, anchored(node))
}

/// The one control's node, without its name.
#[must_use]
pub fn body(control: &Value) -> &Value {
    control
        .as_object()
        .and_then(|map| map.values().next())
        .expect("a control holds one node")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rounded_rectangle_carries_its_geometry_and_colour() {
        let control = rounded("face", [4.0, 6.0], [20.0, 10.0], 3.0, [1.0, 0.5, 0.0, 1.0]);
        let node = body(&control);
        assert_eq!(node["renderer"], "cinnabar_rounded_rectangle");
        assert_eq!(node["size"], json!([20.0, 10.0]));
        assert_eq!(node["offset"], json!([4.0, 6.0]));
        assert_eq!(node["radius"], json!(3.0));
        assert_eq!(node["anchor_from"], "top_left");
    }

    #[test]
    fn a_button_sends_its_press_target_and_holds_its_children() {
        let child = label("t", "x", [0.0, 0.0], [10.0, 10.0], [1.0; 4], 1.0, Align::Center);
        let control = button("close", [1.0, 2.0], [10.0, 10.0], "mod.close", vec![child]);
        let node = body(&control);
        assert_eq!(node["button_mappings"][0]["to_button_id"], "mod.close");
        assert_eq!(node["controls"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn labels_are_never_localised_so_their_text_shows_as_written() {
        let control = label("t", "Mods", [0.0, 0.0], [10.0, 10.0], [1.0; 4], 1.0, Align::Left);
        assert_eq!(body(&control)["localize"], json!(false));
        assert_eq!(body(&control)["text_alignment"], "left");
    }
}
