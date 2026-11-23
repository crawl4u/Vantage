//! The scene: a flat, ordered list of drawing primitives and the regions that take input, built from a
//! layout. The painter draws the primitives back to front; input uses the regions.

use crate::geom::{Rect, Vec2};
use crate::style::{Border, Color, Fill, Radii, Shadow};

/// How text sits in its box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Centre,
    End,
}

/// One thing to draw.
#[derive(Clone, Debug, PartialEq)]
pub enum Primitive {
    /// A rounded rectangle with a fill, an optional border and an optional shadow.
    Rect { rect: Rect, fill: Fill, radii: Radii, border: Option<Border>, shadow: Option<Shadow> },
    /// A line of text. `size` is in design units; `font` names a face the painter knows.
    Text { rect: Rect, text: String, size: f32, color: Color, align: Align, font: &'static str },
    /// One glyph of the icon font centred in `rect`.
    Icon { rect: Rect, glyph: char, size: f32, color: Color },
    /// A filled or stroked polygon set, in the box `rect` (points are in `view` units).
    Path { rect: Rect, view: [f32; 2], contours: Vec<Vec<[f32; 2]>>, color: Color, stroke: Option<f32> },
    /// Open polylines stroked `width` view units wide with round joins and ends, in the box `rect`.
    Line { rect: Rect, view: [f32; 2], contours: Vec<Vec<[f32; 2]>>, color: Color, width: f32 },
    /// Everything until the matching [`Primitive::PopClip`] is cut to `rect`.
    PushClip { rect: Rect, radii: Radii },
    PopClip,
    /// Everything until the matching [`Primitive::PopOpacity`] is drawn at this opacity.
    PushOpacity(f32),
    PopOpacity,
}

/// A rectangle that takes input, by the id of the widget that owns it.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub id: u64,
    pub rect: Rect,
    /// The clip the region sits in: input outside it does not reach the region.
    pub clip: Rect,
}

/// The primitives and regions of one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub primitives: Vec<Primitive>,
    pub regions: Vec<Region>,
    clips: Vec<Rect>,
}

impl Scene {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, primitive: Primitive) {
        match &primitive {
            Primitive::PushClip { rect, .. } => {
                let inner = self.clip().map_or(*rect, |outer| outer.intersect(*rect));
                self.clips.push(inner);
            }
            Primitive::PopClip => {
                self.clips.pop();
            }
            _ => {}
        }
        self.primitives.push(primitive);
    }

    /// The clip in force now, if any.
    #[must_use]
    pub fn clip(&self) -> Option<Rect> {
        self.clips.last().copied()
    }

    /// Adds an input region for widget `id`, limited to the clip in force.
    pub fn region(&mut self, id: u64, rect: Rect) {
        let clip = self.clip().unwrap_or(rect);
        self.regions.push(Region { id, rect, clip });
    }

    /// The topmost region under `p`: the last one added that holds the point inside its clip.
    #[must_use]
    pub fn hit(&self, p: Vec2) -> Option<u64> {
        self.regions.iter().rev().find(|r| r.rect.contains(p) && r.clip.contains(p)).map(|r| r.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_topmost_region_wins_and_a_clip_hides_what_it_cuts_off() {
        let mut scene = Scene::new();
        scene.region(1, Rect::new(0.0, 0.0, 100.0, 100.0));
        scene.region(2, Rect::new(10.0, 10.0, 20.0, 20.0));
        assert_eq!(scene.hit(Vec2::new(15.0, 15.0)), Some(2));
        assert_eq!(scene.hit(Vec2::new(50.0, 50.0)), Some(1));
        scene.push(Primitive::PushClip { rect: Rect::new(0.0, 60.0, 100.0, 40.0), radii: Radii::default() });
        scene.region(3, Rect::new(0.0, 0.0, 100.0, 100.0));
        // Inside the clip the clipped region is on top; above it, it is cut off.
        assert_eq!(scene.hit(Vec2::new(50.0, 70.0)), Some(3));
        assert_eq!(scene.hit(Vec2::new(50.0, 50.0)), Some(1));
        scene.push(Primitive::PopClip);
        assert_eq!(scene.clip(), None);
    }

    #[test]
    fn nested_clips_intersect() {
        let mut scene = Scene::new();
        scene.push(Primitive::PushClip { rect: Rect::new(0.0, 0.0, 50.0, 50.0), radii: Radii::default() });
        scene.push(Primitive::PushClip { rect: Rect::new(25.0, 25.0, 50.0, 50.0), radii: Radii::default() });
        assert_eq!(scene.clip(), Some(Rect::new(25.0, 25.0, 25.0, 25.0)));
    }
}
