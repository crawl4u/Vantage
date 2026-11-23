//! A small flex-style layout: boxes that stack their children in a row or a column, with fixed or growing
//! sizes, gaps, padding and alignment, plus a wrapping grid. Sizes are in design units.

use crate::geom::{Insets, Rect};

/// The axis children are placed along.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Row,
    Column,
}

/// How a box takes space along an axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    /// Exactly this much.
    Fixed(f32),
    /// What its children need (or its intrinsic size for a leaf).
    Fit,
    /// A share of the space the other children leave, in proportion to the weight.
    Grow(f32),
}

/// How children line up across the axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cross {
    Start,
    Centre,
    End,
    Stretch,
}

/// A layout box.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub axis: Axis,
    pub width: Size,
    pub height: Size,
    pub padding: Insets,
    pub gap: f32,
    pub cross: Cross,
    /// A leaf's own size when its width or height is [`Size::Fit`].
    pub intrinsic: [f32; 2],
    pub children: Vec<Node>,
    /// Where it ended up, set by [`solve`].
    pub rect: Rect,
}

impl Node {
    /// An empty box that fits its content.
    #[must_use]
    pub fn new(axis: Axis) -> Self {
        Self {
            axis,
            width: Size::Fit,
            height: Size::Fit,
            padding: Insets::default(),
            gap: 0.0,
            cross: Cross::Start,
            intrinsic: [0.0, 0.0],
            children: Vec::new(),
            rect: Rect::default(),
        }
    }

    /// A leaf of a fixed size.
    #[must_use]
    pub fn leaf(w: f32, h: f32) -> Self {
        Self { width: Size::Fixed(w), height: Size::Fixed(h), ..Self::new(Axis::Row) }
    }

    #[must_use]
    pub fn size(mut self, width: Size, height: Size) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    #[must_use]
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    #[must_use]
    pub fn cross(mut self, cross: Cross) -> Self {
        self.cross = cross;
        self
    }

    #[must_use]
    pub fn child(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }

    /// The size the box needs along the main axis and across it.
    fn fit(&self) -> [f32; 2] {
        let along = |n: &Node| if self.axis == Axis::Row { n.fit_size()[0] } else { n.fit_size()[1] };
        let across = |n: &Node| if self.axis == Axis::Row { n.fit_size()[1] } else { n.fit_size()[0] };
        let main: f32 = self.children.iter().map(along).sum::<f32>() + self.gap * self.children.len().saturating_sub(1) as f32;
        let side = self.children.iter().map(across).fold(0.0, f32::max);
        let (w, h) = if self.axis == Axis::Row { (main, side) } else { (side, main) };
        if self.children.is_empty() {
            [self.intrinsic[0] + self.padding.left + self.padding.right, self.intrinsic[1] + self.padding.top + self.padding.bottom]
        } else {
            [w + self.padding.left + self.padding.right, h + self.padding.top + self.padding.bottom]
        }
    }

    /// The size the box asks for: fixed sizes as given, others as they fit (a growing box asks for nothing).
    fn fit_size(&self) -> [f32; 2] {
        let fit = self.fit();
        let pick = |s: Size, fit: f32| match s {
            Size::Fixed(v) => v,
            Size::Fit => fit,
            Size::Grow(_) => 0.0,
        };
        [pick(self.width, fit[0]), pick(self.height, fit[1])]
    }
}

/// Places `node` and its descendants in `area`, setting every `rect`.
pub fn solve(node: &mut Node, area: Rect) {
    let own = node.fit_size();
    let w = match node.width {
        Size::Grow(_) => area.w,
        _ => own[0],
    };
    let h = match node.height {
        Size::Grow(_) => area.h,
        _ => own[1],
    };
    node.rect = Rect::new(area.x, area.y, w, h);
    place_children(node);
}

fn place_children(node: &mut Node) {
    let inner = node.rect.inset(node.padding);
    let row = node.axis == Axis::Row;
    let main_len = if row { inner.w } else { inner.h };
    let cross_len = if row { inner.h } else { inner.w };
    let count = node.children.len();
    if count == 0 {
        return;
    }
    let gaps = node.gap * (count - 1) as f32;
    let asked: Vec<f32> = node.children.iter().map(|c| if row { c.fit_size()[0] } else { c.fit_size()[1] }).collect();
    let weights: Vec<f32> = node
        .children
        .iter()
        .map(|c| match if row { c.width } else { c.height } {
            Size::Grow(w) => w.max(0.0),
            _ => 0.0,
        })
        .collect();
    let fixed: f32 = asked.iter().sum();
    let spare = (main_len - gaps - fixed).max(0.0);
    let total_weight: f32 = weights.iter().sum();
    let mut at = if row { inner.x } else { inner.y };
    for (i, child) in node.children.iter_mut().enumerate() {
        let main = asked[i] + if total_weight > 0.0 { spare * weights[i] / total_weight } else { 0.0 };
        let want_cross = if row { child.fit_size()[1] } else { child.fit_size()[0] };
        let grows_cross = matches!(if row { child.height } else { child.width }, Size::Grow(_));
        let cross = if grows_cross || node.cross == Cross::Stretch { cross_len } else { want_cross.min(cross_len.max(want_cross)) };
        let offset = match node.cross {
            Cross::Start | Cross::Stretch => 0.0,
            Cross::Centre => (cross_len - cross) / 2.0,
            Cross::End => cross_len - cross,
        };
        let (x, y, w, h) = if row {
            (at, inner.y + offset, main, cross)
        } else {
            (inner.x + offset, at, cross, main)
        };
        child.rect = Rect::new(x, y, w, h);
        place_children(child);
        at += main + node.gap;
    }
}

/// Where the cells of a wrapping grid go: `count` cells of `cell` size in columns that fit `width`, with
/// `gap` between them. Returns each cell's rectangle relative to the grid's top left, and the grid's height.
#[must_use]
pub fn grid(count: usize, cell: [f32; 2], gap: f32, width: f32) -> (Vec<Rect>, f32) {
    let columns = (((width + gap) / (cell[0] + gap)).floor() as usize).max(1);
    let rects: Vec<Rect> = (0..count)
        .map(|i| Rect::new((i % columns) as f32 * (cell[0] + gap), (i / columns) as f32 * (cell[1] + gap), cell[0], cell[1]))
        .collect();
    let rows = count.div_ceil(columns);
    let height = if rows == 0 { 0.0 } else { rows as f32 * cell[1] + (rows - 1) as f32 * gap };
    (rects, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_splits_the_spare_room_between_growing_children_by_weight() {
        let mut root = Node::new(Axis::Row)
            .size(Size::Fixed(100.0), Size::Fixed(20.0))
            .gap(10.0)
            .child(Node::leaf(20.0, 10.0))
            .child(Node::new(Axis::Row).size(Size::Grow(1.0), Size::Fixed(10.0)))
            .child(Node::new(Axis::Row).size(Size::Grow(3.0), Size::Fixed(10.0)));
        solve(&mut root, Rect::new(0.0, 0.0, 100.0, 20.0));
        // 100 - 20 fixed - 20 gaps = 60 spare: 15 and 45.
        assert_eq!(root.children[0].rect, Rect::new(0.0, 0.0, 20.0, 10.0));
        assert_eq!(root.children[1].rect, Rect::new(30.0, 0.0, 15.0, 10.0));
        assert_eq!(root.children[2].rect, Rect::new(55.0, 0.0, 45.0, 10.0));
    }

    #[test]
    fn a_column_pads_centres_and_fits_its_content() {
        let mut root = Node::new(Axis::Column)
            .padding(Insets::all(5.0))
            .gap(4.0)
            .cross(Cross::Centre)
            .child(Node::leaf(30.0, 10.0))
            .child(Node::leaf(10.0, 10.0));
        solve(&mut root, Rect::new(0.0, 0.0, 999.0, 999.0));
        assert_eq!(root.rect, Rect::new(0.0, 0.0, 40.0, 34.0));
        assert_eq!(root.children[0].rect, Rect::new(5.0, 5.0, 30.0, 10.0));
        assert_eq!(root.children[1].rect, Rect::new(15.0, 19.0, 10.0, 10.0));
    }

    #[test]
    fn stretch_fills_the_cross_axis_and_end_pushes_to_the_far_edge() {
        let mut stretched = Node::new(Axis::Row).size(Size::Fixed(50.0), Size::Fixed(30.0)).cross(Cross::Stretch).child(Node::leaf(10.0, 5.0));
        solve(&mut stretched, Rect::new(0.0, 0.0, 50.0, 30.0));
        assert_eq!(stretched.children[0].rect.h, 30.0);
        let mut ended = Node::new(Axis::Row).size(Size::Fixed(50.0), Size::Fixed(30.0)).cross(Cross::End).child(Node::leaf(10.0, 5.0));
        solve(&mut ended, Rect::new(0.0, 0.0, 50.0, 30.0));
        assert_eq!(ended.children[0].rect.y, 25.0);
    }

    #[test]
    fn a_grid_wraps_to_the_width_and_reports_its_height() {
        let (cells, height) = grid(7, [100.0, 60.0], 10.0, 450.0);
        // (450 + 10) / 110 = 4 columns, 2 rows.
        assert_eq!(cells[3], Rect::new(330.0, 0.0, 100.0, 60.0));
        assert_eq!(cells[4], Rect::new(0.0, 70.0, 100.0, 60.0));
        assert_eq!(height, 130.0);
        assert_eq!(grid(0, [10.0, 10.0], 5.0, 100.0).1, 0.0);
        assert_eq!(grid(3, [200.0, 10.0], 5.0, 100.0).0.len(), 3, "a cell wider than the grid still gets a column");
    }
}
