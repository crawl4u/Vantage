//! The Vantage logo drawn with the host's vector path renderer, from [`crate::brand`]'s layers.

use serde_json::{Value, json};

use crate::brand::{LAYERS, SIZE};
use crate::tokens::Rgba;

/// The logo's nodes in a box `width` wide (GUI pixels) at `offset`; the height follows the logo's shape.
#[must_use]
pub fn logo(offset: [f32; 2], width: f32) -> Vec<Value> {
    let size = [width, width * SIZE[1] / SIZE[0]];
    LAYERS
        .iter()
        .enumerate()
        .map(|(n, layer)| {
            let color: Rgba = [
                f32::from((layer.rgb >> 16 & 255) as u8) / 255.0,
                f32::from((layer.rgb >> 8 & 255) as u8) / 255.0,
                f32::from((layer.rgb & 255) as u8) / 255.0,
                1.0,
            ];
            let mut node = json!({
                "type": "custom",
                "renderer": "cinnabar_vector_path",
                "size": size,
                "offset": offset,
                "anchor_from": "top_left",
                "anchor_to": "top_left",
                "view_box": [0.0, 0.0, SIZE[0], SIZE[1]],
                "paths": layer.contours,
                "color": color,
            });
            if let Some(stroke) = layer.stroke {
                node["stroke_width"] = json!(stroke);
                node["closed"] = json!(true);
            }
            json!({ format!("logo_{n}"): node })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_layer_is_a_node_inside_the_renderers_limits() {
        let nodes = logo([0.0, 0.0], 100.0);
        assert_eq!(nodes.len(), LAYERS.len());
        for layer in LAYERS {
            assert!(layer.contours.len() <= 64, "too many contours");
            assert!(layer.contours.iter().map(|c| c.len()).sum::<usize>() <= 2048, "too many points");
        }
    }
}
