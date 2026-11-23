//! Points, sizes and rectangles in design units.

/// A point or offset.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A rectangle by its top left and size.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    #[must_use]
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    #[must_use]
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    #[must_use]
    pub fn centre(&self) -> Vec2 {
        Vec2::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// Whether `p` is inside: the left and top edges count, the right and bottom do not.
    #[must_use]
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }

    /// The rectangle shrunk by `insets`, never below zero size.
    #[must_use]
    pub fn inset(&self, insets: Insets) -> Rect {
        Rect::new(
            self.x + insets.left,
            self.y + insets.top,
            (self.w - insets.left - insets.right).max(0.0),
            (self.h - insets.top - insets.bottom).max(0.0),
        )
    }

    /// The overlap with `other`, or an empty rectangle at this one's corner.
    #[must_use]
    pub fn intersect(&self, other: Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let r = self.right().min(other.right());
        let b = self.bottom().min(other.bottom());
        if r > x && b > y { Rect::new(x, y, r - x, b - y) } else { Rect::new(self.x, self.y, 0.0, 0.0) }
    }
}

/// Space inside the edges of a box.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Insets {
    #[must_use]
    pub const fn all(v: f32) -> Self {
        Self { left: v, top: v, right: v, bottom: v }
    }

    #[must_use]
    pub const fn xy(x: f32, y: f32) -> Self {
        Self { left: x, top: y, right: x, bottom: y }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rectangle_contains_its_top_left_but_not_its_bottom_right_edge() {
        let r = Rect::new(10.0, 10.0, 20.0, 10.0);
        assert!(r.contains(Vec2::new(10.0, 10.0)));
        assert!(!r.contains(Vec2::new(30.0, 15.0)));
        assert!(!r.contains(Vec2::new(15.0, 20.0)));
    }

    #[test]
    fn insetting_past_the_size_leaves_an_empty_rectangle_and_intersections_are_clipped() {
        assert_eq!(Rect::new(0.0, 0.0, 10.0, 10.0).inset(Insets::all(8.0)).w, 0.0);
        let overlap = Rect::new(0.0, 0.0, 10.0, 10.0).intersect(Rect::new(6.0, 4.0, 10.0, 10.0));
        assert_eq!(overlap, Rect::new(6.0, 4.0, 4.0, 6.0));
        assert_eq!(Rect::new(0.0, 0.0, 4.0, 4.0).intersect(Rect::new(9.0, 9.0, 1.0, 1.0)).w, 0.0);
    }
}
