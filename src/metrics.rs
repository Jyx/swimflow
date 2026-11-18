//! Grid -> pixel geometry: a port of the subset of the reference metrics code
//! that is actually used.
//!
//! The reference implementation places nodes on an integer cell grid and then derives pixel
//! boxes from it. The node/lane/frame geometry is **font-independent**: text is
//! centred inside a box at draw time, it does not resize the box. Consequently
//! these boxes can be compared against the Python reference with tolerance 0
//! (see `tests/metrics_compat.rs`).
//!
//! Ported pieces:
//! * `DiagramMetrics.{cell,textsize,frame,lane_textbox,lane_headerbox,pagesize}`
//! * `SpreadSheetMetrics.{node,_node_topleft,_node_bottomright,pagesize}`
//! * `NodeMetrics.box/corebox/marginbox`
//!
//! Constants follow `blockdiag.metrics.DiagramMetrics`.

use crate::build::model::{Diagram, Direction, DocumentSet, Edge, Lane, Node, Orientation};

/// `cellsize` in blockdiag.
pub const CELLSIZE: i32 = 8;
/// Padding between a node box and its text (`NodeMetrics.corebox`).
pub const NODE_PADDING: i32 = 4;
/// Spacing between wrapped text lines.
pub const LINE_SPACING: i32 = 2;
/// Default node box size (`node_width = cellsize * 16`, `node_height = cellsize * 5`).
pub const DEFAULT_NODE_WIDTH: i32 = CELLSIZE * 16;
pub const DEFAULT_NODE_HEIGHT: i32 = CELLSIZE * 5;
/// Default gap between grid cells (`span_width = cellsize * 8`, `span_height = cellsize * 5`).
pub const DEFAULT_SPAN_WIDTH: i32 = CELLSIZE * 8;
pub const DEFAULT_SPAN_HEIGHT: i32 = CELLSIZE * 5;

/// An axis-aligned pixel rectangle, matching blockdiag's `Box` `(x1, y1, x2, y2)`.
///
/// The type itself lives in the core (it is shared by every diagram type);
/// the activity-specific helpers below stay here.
pub use crate::core::geom::PixelBox;

impl PixelBox {
    /// `NodeMetrics.corebox`: the node box shrunk by `node_padding`.
    pub fn corebox(&self) -> PixelBox {
        PixelBox {
            x1: self.x1 + NODE_PADDING,
            y1: self.y1 + NODE_PADDING,
            x2: self.x2 - NODE_PADDING * 2,
            y2: self.y2 - NODE_PADDING * 2,
        }
    }

    /// `NodeMetrics.marginbox`: the node box grown by the span fractions.
    pub fn marginbox(&self, span_width: i32, span_height: i32) -> PixelBox {
        PixelBox {
            x1: self.x1 - span_width / 8,
            y1: self.y1 - span_height / 4,
            x2: self.x2 + span_width / 8,
            y2: self.y2 + span_height / 4,
        }
    }
}

/// A generic grid cell, so the same math works for real nodes, lanes and the
/// synthetic node `frame` uses.
#[derive(Debug, Clone, Copy, Default)]
struct Cell {
    x: i32,
    y: i32,
    colwidth: i32,
    colheight: i32,
    width: Option<i32>,
    height: Option<i32>,
}

impl Cell {
    fn from_node(node: &Node) -> Cell {
        Cell {
            x: node.grid.x,
            y: node.grid.y,
            colwidth: node.grid.colwidth,
            colheight: node.grid.colheight,
            width: node.width,
            height: node.height,
        }
    }

    fn from_lane(lane: &Lane) -> Cell {
        Cell {
            x: lane.grid.x,
            y: lane.grid.y,
            colwidth: lane.grid.colwidth,
            colheight: lane.grid.colheight,
            width: None,
            height: None,
        }
    }
}

/// The three boxes and separators of the swimlane frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub headerbox: PixelBox,
    pub outline: PixelBox,
    /// Each separator is a line segment given by its two endpoints.
    pub separators: Vec<((i32, i32), (i32, i32))>,
}

/// Per-diagram pixel metrics.
#[derive(Debug, Clone)]
pub struct Metrics {
    /// Diagram grid width/height, as set by the reference `pagesize`.
    pub colwidth: i32,
    pub colheight: i32,
    pub node_width: i32,
    pub node_height: i32,
    pub span_width: i32,
    pub span_height: i32,
    /// CSS-like `[top, right, bottom, left]`, plus `margin` always zero.
    pub page_padding: [i32; 4],
    node_width_cells: Vec<Option<i32>>,
    node_height_cells: Vec<Option<i32>>,
    span_width_cells: Vec<Option<i32>>,
    span_height_cells: Vec<Option<i32>>,
}

impl Metrics {
    /// Build metrics for a diagram, mirroring the reference `DiagramMetrics`.
    pub fn new(diagram: &Diagram) -> Metrics {
        let node_width = diagram.node_width.unwrap_or(DEFAULT_NODE_WIDTH);
        let node_height = diagram.node_height.unwrap_or(DEFAULT_NODE_HEIGHT);
        let span_width = diagram.span_width.unwrap_or(DEFAULT_SPAN_WIDTH);
        let span_height = diagram.span_height.unwrap_or(DEFAULT_SPAN_HEIGHT);

        let page_padding = match diagram.orientation {
            crate::build::model::Orientation::Landscape => [0, 0, 0, node_width + span_width],
            crate::build::model::Orientation::Portrait => [node_height + span_height, 0, 0, 0],
        };

        let mut metrics = Metrics {
            colwidth: diagram.grid.colwidth,
            colheight: diagram.grid.colheight,
            node_width,
            node_height,
            span_width,
            span_height,
            page_padding,
            node_width_cells: vec![None; diagram.grid.colwidth.max(0) as usize + 2],
            node_height_cells: vec![None; diagram.grid.colheight.max(0) as usize + 2],
            span_width_cells: vec![None; diagram.grid.colwidth.max(0) as usize + 2],
            span_height_cells: vec![None; diagram.grid.colheight.max(0) as usize + 2],
        };

        for x in 0..diagram.grid.colwidth {
            let widths = diagram
                .nodes
                .iter()
                .filter(|node| node.grid.x == x)
                .map(|node| node.width.unwrap_or(node_width));
            metrics.set_node_width(x, widths.max());
        }
        for y in 0..diagram.grid.colheight {
            let heights = diagram
                .nodes
                .iter()
                .filter(|node| node.grid.y == y)
                .map(|node| node.height.unwrap_or(node_height));
            metrics.set_node_height(y, heights.max());
        }

        metrics
    }

    fn slot(v: &mut Vec<Option<i32>>, index: i32, width: i32) {
        if width <= 0 || index < 0 {
            return;
        }
        let index = index as usize;
        if index >= v.len() {
            v.resize(index + 1, None);
        }
        if v[index].map(|current| current < width).unwrap_or(true) {
            v[index] = Some(width);
        }
    }

    fn set_node_width(&mut self, x: i32, width: Option<i32>) {
        if let Some(width) = width {
            Self::slot(&mut self.node_width_cells, x, width);
        }
    }

    fn set_node_height(&mut self, y: i32, height: Option<i32>) {
        if let Some(height) = height {
            Self::slot(&mut self.node_height_cells, y, height);
        }
    }

    /// `SpreadSheetMetrics.node_width[x]` with its per-column default.
    pub fn node_width_at(&self, x: i32) -> i32 {
        if x < 0 {
            return self.node_width;
        }
        self.node_width_cells
            .get(x as usize)
            .copied()
            .flatten()
            .unwrap_or(self.node_width)
    }

    pub fn node_height_at(&self, y: i32) -> i32 {
        if y < 0 {
            return self.node_height;
        }
        self.node_height_cells
            .get(y as usize)
            .copied()
            .flatten()
            .unwrap_or(self.node_height)
    }

    pub fn span_width_at(&self, x: i32) -> i32 {
        if x < 0 {
            return self.span_width;
        }
        self.span_width_cells
            .get(x as usize)
            .copied()
            .flatten()
            .unwrap_or(self.span_width)
    }

    pub fn span_height_at(&self, y: i32) -> i32 {
        if y < 0 {
            return self.span_height;
        }
        self.span_height_cells
            .get(y as usize)
            .copied()
            .flatten()
            .unwrap_or(self.span_height)
    }

    fn sum_range<F: Fn(i32) -> i32>(n: i32, f: F) -> i32 {
        (0..n.max(0)).map(f).sum()
    }

    fn diffs(&self, x: i32, y: i32, cell: &Cell, use_padding: bool) -> (i32, i32) {
        if !use_padding {
            return (0, 0);
        }
        let width = cell.width.unwrap_or(self.node_width);
        let mut xdiff = (self.node_width_at(x) - width) / 2;
        if xdiff < 0 {
            xdiff = 0;
        }
        let height = cell.height.unwrap_or(self.node_height);
        let mut ydiff = (self.node_height_at(y) - height) / 2;
        if ydiff < 0 {
            ydiff = 0;
        }
        (xdiff, ydiff)
    }

    fn node_topleft(&self, cell: &Cell, use_padding: bool) -> (i32, i32) {
        let node_width = Self::sum_range(cell.x, |i| self.node_width_at(i));
        let node_height = Self::sum_range(cell.y, |i| self.node_height_at(i));
        let span_width = Self::sum_range(cell.x + 1, |i| self.span_width_at(i));
        let span_height = Self::sum_range(cell.y + 1, |i| self.span_height_at(i));
        let (xdiff, ydiff) = self.diffs(cell.x, cell.y, cell, use_padding);

        (
            self.page_padding[3] + node_width + span_width + xdiff,
            self.page_padding[0] + node_height + span_height + ydiff,
        )
    }

    fn node_bottomright(&self, cell: &Cell, use_padding: bool) -> (i32, i32) {
        let x = cell.x + cell.colwidth - 1;
        let y = cell.y + cell.colheight - 1;
        let node_width = Self::sum_range(x + 1, |i| self.node_width_at(i));
        let node_height = Self::sum_range(y + 1, |i| self.node_height_at(i));
        let span_width = Self::sum_range(x + 1, |i| self.span_width_at(i));
        let span_height = Self::sum_range(y + 1, |i| self.span_height_at(i));
        let (xdiff, ydiff) = self.diffs(x, y, cell, use_padding);

        (
            self.page_padding[3] + node_width + span_width - xdiff,
            self.page_padding[0] + node_height + span_height - ydiff,
        )
    }

    fn cell_of(&self, cell: &Cell, use_padding: bool) -> PixelBox {
        let (x1, y1) = self.node_topleft(cell, use_padding);
        let (x2, y2) = self.node_bottomright(cell, use_padding);
        PixelBox { x1, y1, x2, y2 }
    }

    /// `DiagramMetrics.cell(node)`: the node's pixel box.
    pub fn cell(&self, node: &Node) -> PixelBox {
        self.cell_of(&Cell::from_node(node), true)
    }

    /// `DiagramMetrics.group(group)` / `frame`'s dummy cell (`use_padding=False`).
    pub fn cell_without_padding(&self, node: &Node) -> PixelBox {
        self.cell_of(&Cell::from_node(node), false)
    }

    /// The reference `DiagramMetrics.frame(lanes)`.
    pub fn frame(&self, lanes: &[Lane]) -> Frame {
        let dummy = Cell {
            x: 0,
            y: 0,
            colwidth: self.colwidth,
            colheight: self.colheight,
            width: None,
            height: None,
        };
        let cell = self.cell_of(&dummy, false);

        let half_span_width = self.span_width / 2;
        let half_span_height = self.span_height / 2;
        let headerbox = PixelBox {
            x1: cell.x1 - half_span_width,
            y1: cell.y1 - self.node_height - self.span_height - 2,
            x2: cell.x2 + half_span_width,
            y2: cell.y1 - half_span_height,
        };
        let outline = PixelBox {
            x1: headerbox.x1,
            y1: headerbox.y1,
            x2: headerbox.x2,
            y2: cell.y2 + half_span_height,
        };

        let mut separators = vec![((headerbox.x1, headerbox.y2), (headerbox.x2, headerbox.y2))];

        for lane in &lanes[..lanes.len().saturating_sub(1)] {
            let x = lane.grid.x + lane.grid.colwidth + 1;
            let m = self.cell_of(&Cell::from_lane(lane), false);
            let span_width = self.span_width_at(x) / 2;
            let x1 = m.right().0 + span_width;
            separators.push(((x1, outline.y1), (x1, outline.y2)));
        }

        Frame {
            headerbox,
            outline,
            separators,
        }
    }

    /// The reference `DiagramMetrics.lane_textbox(lane)`.
    pub fn lane_textbox(&self, lane: &Lane) -> PixelBox {
        let headerbox = self.frame(&[]).headerbox;
        let m = self.cell_of(&Cell::from_lane(lane), false);
        PixelBox {
            x1: m.x1,
            y1: headerbox.y1,
            x2: m.x2,
            y2: headerbox.y2,
        }
    }

    /// The reference `DiagramMetrics.lane_headerbox(lane)`.
    pub fn lane_headerbox(&self, lane: &Lane) -> PixelBox {
        let headerbox = self.frame(&[]).headerbox;
        let m = self.cell_of(&Cell::from_lane(lane), true);
        PixelBox {
            x1: m.x1 - self.span_width_at(lane.grid.x) / 2,
            y1: headerbox.y1,
            x2: m.x2 + self.span_width_at(lane.grid.x + 1) / 2,
            y2: headerbox.y2,
        }
    }

    /// `DiagramMetrics.pagesize`: overall canvas size including padding.
    pub fn pagesize(&self) -> (i32, i32) {
        let dummy = Cell {
            x: self.colwidth - 1,
            y: self.colheight - 1,
            colwidth: 1,
            colheight: 1,
            width: None,
            height: None,
        };
        let (x, y) = self.node_bottomright(&dummy, false);
        let x_span = self.span_width_at(self.colwidth);
        let y_span = self.span_height_at(self.colheight);
        (
            x + self.page_padding[1] + x_span,
            y + self.page_padding[2] + y_span,
        )
    }

    /// `DiagramMetrics.edge`: route an edge into shaft polylines and arrowhead
    /// polygons.
    ///
    /// This is the port of `blockdiag.metrics.PortraitEdgeMetrics` /
    /// `LandscapeEdgeMetrics` including the `EdgeLayoutManager` "skipped"
    /// detection. Ports of `EdgeMetrics._shaft`, `_head` and `headshapes`.
    pub fn edge_route(&self, diagram: &Diagram, edge: &Edge) -> EdgeRoute {
        let from = diagram.node(edge.from);
        let to = diagram.node(edge.to);
        let rel = relative_direction(from, to);
        let skipped = edge_is_skipped(diagram, from, to, rel);
        let hstyle = edge.attrs.get("hstyle").and_then(|v| v.as_deref());

        let (heads, polylines) = match diagram.orientation {
            Orientation::Portrait => (
                self.portrait_headshapes(edge.dir, rel, skipped, hstyle),
                self.portrait_shaft(from, to, rel, skipped),
            ),
            Orientation::Landscape => (
                self.landscape_headshapes(edge.dir, rel, skipped, hstyle),
                self.landscape_shaft(from, to, rel, skipped),
            ),
        };

        let mut head_polygons = Vec::new();
        if let Some(head) = heads[0] {
            head_polygons.push(self.head_polygon(from, head, hstyle));
        }
        if let Some(head) = heads[1] {
            head_polygons.push(self.head_polygon(to, head, hstyle));
        }

        EdgeRoute {
            polylines,
            heads: head_polygons,
        }
    }

    /// `DiagramMetrics.edge(...).labelbox`: the box an edge label is centred in.
    ///
    /// Port of `PortraitEdgeMetrics.labelbox` / `LandscapeEdgeMetrics.labelbox`.
    pub fn edge_labelbox(&self, diagram: &Diagram, edge: &Edge) -> PixelBox {
        let from = diagram.node(edge.from);
        let to = diagram.node(edge.to);
        let rel = relative_direction(from, to);
        let skipped = edge_is_skipped(diagram, from, to, rel);

        let sx = self.span_width;
        let sy = self.span_height;
        let node1 = self.cell_without_padding(from);
        let node2 = self.cell_without_padding(to);
        let box_ = if diagram.orientation == Orientation::Landscape {
            self.landscape_labelbox(from, to, rel, skipped, &node1, &node2, sx, sy)
        } else {
            self.portrait_labelbox(from, to, rel, skipped, &node1, &node2, sx, sy)
        };

        // Shrink the box horizontally by an eighth of the span on each side.
        PixelBox {
            x1: box_.x1 + sx / 8,
            y1: box_.y1,
            x2: box_.x2 - sx / 8,
            y2: box_.y2,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn portrait_labelbox(
        &self,
        from: &Node,
        to: &Node,
        rel: RelDir,
        skipped: bool,
        node1: &PixelBox,
        node2: &PixelBox,
        sx: i32,
        sy: i32,
    ) -> PixelBox {
        match rel {
            RelDir::Right if skipped => PixelBox {
                x1: node1.bottomright().0 + sx,
                y1: node1.bottomright().1,
                x2: node2.bottomleft().0 - sx,
                y2: node2.bottomleft().1 + sy / 2,
            },
            RelDir::Right => PixelBox {
                x1: node1.topright().0,
                y1: node1.topright().1 - sy / 8,
                x2: node2.left().0,
                y2: node2.left().1 - sy / 8,
            },
            RelDir::RightUp => PixelBox {
                x1: node2.left().0 - sx,
                y1: node2.left().1,
                x2: node2.bottomleft().0,
                y2: node2.bottomleft().1,
            },
            RelDir::RightDown => PixelBox {
                x1: node2.topleft().0,
                y1: node2.topleft().1 - sy / 2,
                x2: node2.top().0,
                y2: node2.top().1,
            },
            RelDir::Up | RelDir::LeftUp | RelDir::Left | RelDir::Same => {
                if to.grid.y < from.grid.y {
                    PixelBox {
                        x1: node1.topright().0 - sx / 2 + sx / 4,
                        y1: node1.topright().1 - sy / 2,
                        x2: node1.topright().0 + sx / 2 + sx / 4,
                        y2: node1.topright().1,
                    }
                } else {
                    PixelBox {
                        x1: node1.top().0 + sx / 4,
                        y1: node1.top().1 - sy,
                        x2: node1.topright().0 + sx / 4,
                        y2: node1.topright().1 - sy / 2,
                    }
                }
            }
            RelDir::Down => PixelBox {
                x1: node2.top().0 + sx / 4,
                y1: node2.top().1 - sy / 2,
                x2: node2.topright().0 + sx / 4,
                y2: node2.topright().1,
            },
            RelDir::LeftDown => PixelBox {
                x1: node1.bottomleft().0,
                y1: node1.bottomleft().1,
                x2: node1.bottom().0,
                y2: node1.bottom().1 + sy / 2,
            },
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn landscape_labelbox(
        &self,
        from: &Node,
        to: &Node,
        rel: RelDir,
        skipped: bool,
        node1: &PixelBox,
        node2: &PixelBox,
        sx: i32,
        sy: i32,
    ) -> PixelBox {
        let node_h = self.node_height;
        match rel {
            RelDir::Right if skipped => PixelBox {
                x1: node1.bottomright().0 + sx,
                y1: node1.bottomright().1,
                x2: node2.bottomleft().0 - sx,
                y2: node2.bottomleft().1 + sy / 2,
            },
            RelDir::Right => PixelBox {
                x1: node1.topright().0,
                y1: node1.topright().1 - sy / 8,
                x2: node2.left().0,
                y2: node2.left().1 - sy / 8,
            },
            RelDir::RightUp => PixelBox {
                x1: node2.left().0 - sx,
                y1: node1.top().1 - node_h / 2,
                x2: node2.bottomleft().0,
                y2: node1.top().1,
            },
            RelDir::RightDown => PixelBox {
                x1: node1.right().0,
                y1: node2.topleft().1 - sy / 8,
                x2: node1.right().0 + sx,
                y2: node2.left().1 - sy / 8,
            },
            RelDir::Up | RelDir::LeftUp | RelDir::Left | RelDir::Same => {
                if to.grid.y < from.grid.y {
                    PixelBox {
                        x1: node1.topright().0 - sx / 2 + sx / 4,
                        y1: node1.topright().1 - sy / 2,
                        x2: node1.topright().0 + sx / 2 + sx / 4,
                        y2: node1.topright().1,
                    }
                } else {
                    PixelBox {
                        x1: node1.top().0 + sx / 4,
                        y1: node1.top().1 - sy,
                        x2: node1.topright().0 + sx / 4,
                        y2: node1.topright().1 - sy / 2,
                    }
                }
            }
            RelDir::LeftDown | RelDir::Down => PixelBox {
                x1: node2.top().0 + sx / 4,
                y1: node2.top().1 - sy,
                x2: node2.topright().0 + sx / 4,
                y2: node2.topright().1 - sy / 2,
            },
        }
    }

    // --- edge shafts -------------------------------------------------------

    #[allow(clippy::too_many_lines)]
    fn portrait_shaft(
        &self,
        from: &Node,
        to: &Node,
        rel: RelDir,
        skipped: bool,
    ) -> Vec<Vec<(i32, i32)>> {
        let span_w = self.span_width;
        let span_h = self.span_height;
        let node1 = self.cell(from);
        let cell1 = self.cell_without_padding(from);
        let node2 = self.cell(to);
        let cell2 = self.cell_without_padding(to);
        let mut lines = Lines::new();

        match rel {
            RelDir::Right if !skipped => {
                lines.move_to(node1.right());
                lines.line_to(node2.left());
            }
            RelDir::Up | RelDir::RightUp | RelDir::Same | RelDir::Right => {
                lines.move_to(node1.bottom());
                lines.line_to((cell1.bottom().0, cell1.bottom().1 + span_h / 2));
                lines.line_to((cell2.right().0 + span_w / 4, cell1.bottom().1 + span_h / 2));
                let y = cell2.top().1 - span_h / 2 + span_h / 8;
                lines.line_to((cell2.right().0 + span_w / 4, y));
                lines.line_to((cell2.top().0, y));
                lines.line_to(node2.top());
            }
            RelDir::LeftUp | RelDir::Left => {
                lines.move_to(node1.right());
                lines.line_to((cell1.right().0 + span_w / 4, cell1.right().1));
                let y = cell2.top().1 - span_h / 2 + span_h / 8;
                lines.line_to((cell1.right().0 + span_w / 4, y));
                lines.line_to((cell2.top().0, y));
                lines.line_to(node2.top());
            }
            RelDir::RightDown => {
                lines.move_to(node1.bottom());
                lines.line_to((cell1.bottom().0, cell1.bottom().1 + span_h / 2));
                if skipped {
                    lines.line_to((cell2.left().0 - span_w / 2, cell1.bottom().1 + span_h / 2));
                    lines.line_to((
                        cell2.topleft().0 - span_w / 2,
                        cell2.topleft().1 - span_h / 2,
                    ));
                    lines.line_to((cell2.top().0, cell2.top().1 - span_h / 2));
                } else {
                    lines.line_to((cell2.top().0, cell1.bottom().1 + span_h / 2));
                }
                lines.line_to(node2.top());
            }
            RelDir::LeftDown => {
                lines.move_to(node1.bottom());
                if skipped {
                    lines.line_to((cell1.bottom().0, cell1.bottom().1 + span_h / 2));
                    lines.line_to((cell2.right().0 + span_w / 2, cell1.bottom().1 + span_h / 2));
                    lines.line_to((cell2.right().0 + span_w / 2, cell2.top().1 - span_h / 2));
                } else {
                    lines.line_to((cell1.bottom().0, cell2.top().1 - span_h / 2));
                }
                lines.line_to((cell2.top().0, cell2.top().1 - span_h / 2));
                lines.line_to(node2.top());
            }
            RelDir::Down => {
                lines.move_to(node1.bottom());
                if skipped {
                    lines.line_to((cell1.bottom().0, cell1.bottom().1 + span_h / 2));
                    lines.line_to((cell1.right().0 + span_w / 2, cell1.bottom().1 + span_h / 2));
                    lines.line_to((cell2.right().0 + span_w / 2, cell2.top().1 - span_h / 2));
                    lines.line_to((cell2.top().0, cell2.top().1 - span_h / 2));
                }
                lines.line_to(node2.top());
            }
        }
        lines.polylines
    }

    #[allow(clippy::too_many_lines)]
    fn landscape_shaft(
        &self,
        from: &Node,
        to: &Node,
        rel: RelDir,
        skipped: bool,
    ) -> Vec<Vec<(i32, i32)>> {
        let span_w = self.span_width;
        let span_h = self.span_height;
        let node1 = self.cell(from);
        let cell1 = self.cell_without_padding(from);
        let node2 = self.cell(to);
        let cell2 = self.cell_without_padding(to);
        let mut lines = Lines::new();

        match rel {
            RelDir::Right => {
                lines.move_to(node1.right());
                if skipped {
                    lines.line_to((cell1.right().0 + span_w / 2, cell1.right().1));
                    lines.line_to((
                        cell1.right().0 + span_w / 2,
                        cell1.bottomright().1 + span_h / 2,
                    ));
                    lines.line_to((
                        cell2.left().0 - span_w / 4,
                        cell2.bottomright().1 + span_h / 2,
                    ));
                    lines.line_to((cell2.left().0 - span_w / 4, cell2.left().1));
                }
                lines.line_to(node2.left());
            }
            RelDir::RightUp => {
                lines.move_to(node1.right());
                if skipped {
                    lines.line_to((cell1.right().0 + span_w / 2, cell1.right().1));
                    lines.line_to((
                        cell1.right().0 + span_w / 2,
                        cell2.bottomleft().1 + span_h / 2,
                    ));
                    lines.line_to((
                        cell2.left().0 - span_w / 4,
                        cell2.bottomleft().1 + span_h / 2,
                    ));
                    lines.line_to((cell2.left().0 - span_w / 4, cell2.left().1));
                } else {
                    lines.line_to((cell2.left().0 - span_w / 4, cell1.right().1));
                    lines.line_to((cell2.left().0 - span_w / 4, cell2.left().1));
                }
                lines.line_to(node2.left());
            }
            RelDir::RightDown => {
                lines.move_to(node1.right());
                lines.line_to((cell1.right().0 + span_w / 2, cell1.right().1));
                if skipped {
                    lines.line_to((cell1.right().0 + span_w / 2, cell2.topleft().1 - span_h / 2));
                    lines.line_to((cell2.left().0 - span_w / 4, cell2.topleft().1 - span_h / 2));
                    lines.line_to((cell2.left().0 - span_w / 4, cell2.left().1));
                } else {
                    lines.line_to((cell1.right().0 + span_w / 2, cell2.left().1));
                }
                lines.line_to(node2.left());
            }
            RelDir::Up => {
                if skipped {
                    lines.move_to(node1.right());
                    lines.line_to((cell1.right().0 + span_w / 4, cell1.right().1));
                    lines.line_to((cell1.right().0 + span_w / 4, cell2.bottom().1 + span_h / 2));
                    lines.line_to((cell2.bottom().0, cell2.bottom().1 + span_h / 2));
                } else {
                    lines.move_to(node1.top());
                }
                lines.line_to(node2.bottom());
            }
            RelDir::LeftUp | RelDir::Left | RelDir::Same => {
                lines.move_to(node1.right());
                lines.line_to((cell1.right().0 + span_w / 4, cell1.right().1));
                let y = cell2.top().1 - span_h / 2 + span_h / 8;
                lines.line_to((cell1.right().0 + span_w / 4, y));
                lines.line_to((cell2.top().0, y));
                lines.line_to(node2.top());
            }
            RelDir::LeftDown => {
                if skipped {
                    lines.move_to(node1.right());
                    lines.line_to((cell1.right().0 + span_w / 2, cell1.right().1));
                    lines.line_to((cell1.right().0 + span_w / 2, cell2.top().1 - span_h / 2));
                    lines.line_to((cell2.top().0, cell2.top().1 - span_h / 2));
                } else {
                    lines.move_to(node1.bottom());
                    lines.line_to((cell1.bottom().0, cell2.top().1 - span_h / 2));
                    lines.line_to((cell2.top().0, cell2.top().1 - span_h / 2));
                }
                lines.line_to(node2.top());
            }
            RelDir::Down => {
                if skipped {
                    lines.move_to(node1.right());
                    lines.line_to((cell1.right().0 + span_w / 2, cell1.right().1));
                    let y = cell2.top().1 - span_h / 2 + span_h / 8;
                    lines.line_to((cell1.right().0 + span_w / 2, y));
                    lines.line_to((cell2.top().0, y));
                } else {
                    lines.move_to(node1.bottom());
                }
                lines.line_to(node2.top());
            }
        }
        lines.polylines
    }

    // --- edge arrowheads --------------------------------------------------

    fn portrait_headshapes(
        &self,
        dir: Direction,
        rel: RelDir,
        skipped: bool,
        hstyle: Option<&str>,
    ) -> [Option<Head>; 2] {
        let mut heads = [None, None];
        if matches!(dir, Direction::Reverse | Direction::Bidirectional) {
            let head = match rel {
                RelDir::Right => {
                    if skipped {
                        Head::Up
                    } else {
                        Head::Left
                    }
                }
                RelDir::Up | RelDir::RightUp | RelDir::Same => Head::Up,
                RelDir::LeftUp | RelDir::Left => Head::Left,
                RelDir::LeftDown | RelDir::Down | RelDir::RightDown => {
                    if skipped {
                        Head::Left
                    } else {
                        Head::Up
                    }
                }
            };
            heads[0] = Some(head);
        }
        if matches!(dir, Direction::Forward | Direction::Bidirectional) {
            let head = match rel {
                RelDir::Right => {
                    if skipped {
                        Head::Down
                    } else {
                        Head::Right
                    }
                }
                RelDir::Up
                | RelDir::RightUp
                | RelDir::Same
                | RelDir::LeftUp
                | RelDir::Left
                | RelDir::LeftDown
                | RelDir::Down
                | RelDir::RightDown => Head::Down,
            };
            heads[1] = Some(head);
        }
        reverse_heads(&mut heads, hstyle);
        heads
    }

    fn landscape_headshapes(
        &self,
        dir: Direction,
        rel: RelDir,
        skipped: bool,
        hstyle: Option<&str>,
    ) -> [Option<Head>; 2] {
        let mut heads = [None, None];
        if matches!(dir, Direction::Reverse | Direction::Bidirectional) {
            let head = match rel {
                RelDir::LeftUp
                | RelDir::Left
                | RelDir::Same
                | RelDir::RightUp
                | RelDir::Right
                | RelDir::RightDown => Head::Left,
                RelDir::Up => {
                    if skipped {
                        Head::Left
                    } else {
                        Head::Down
                    }
                }
                RelDir::LeftDown | RelDir::Down => {
                    if skipped {
                        Head::Left
                    } else {
                        Head::Up
                    }
                }
            };
            heads[0] = Some(head);
        }
        if matches!(dir, Direction::Forward | Direction::Bidirectional) {
            let head = match rel {
                RelDir::RightUp | RelDir::Right | RelDir::RightDown => Head::Right,
                RelDir::Up => Head::Up,
                RelDir::LeftUp | RelDir::Left | RelDir::LeftDown | RelDir::Down | RelDir::Same => {
                    Head::Down
                }
            };
            heads[1] = Some(head);
        }
        reverse_heads(&mut heads, hstyle);
        heads
    }

    /// `EdgeMetrics._head`: the arrowhead polygon for one node endpoint.
    fn head_polygon(&self, node: &Node, head: Head, hstyle: Option<&str>) -> Vec<(i32, i32)> {
        let cell = CELLSIZE;
        let box_ = self.cell(node);
        let (x, y) = match head {
            Head::Up | Head::RUp => box_.bottom(),
            Head::Down | Head::RDown => box_.top(),
            Head::Right | Head::RRight => box_.left(),
            Head::Left | Head::RLeft => box_.right(),
        };

        let mut points: Vec<(i32, i32)> = match head {
            Head::Up => vec![
                (x, y + 1),
                (x - cell / 2, y + cell),
                (x, y + cell * 2),
                (x + cell / 2, y + cell),
                (x, y + 1),
            ],
            Head::RUp => vec![
                (x, y + cell),
                (x - cell, y + 1),
                (x, y + 2),
                (x + cell, y + 1),
                (x, y + cell),
            ],
            Head::Down => vec![
                (x, y - 1),
                (x - cell / 2, y - cell),
                (x, y - cell * 2),
                (x + cell / 2, y - cell),
                (x, y - 1),
            ],
            Head::RDown => vec![
                (x, y - cell),
                (x - cell, y - 1),
                (x, y - 2),
                (x + cell, y - 1),
                (x, y - cell),
            ],
            Head::Right => vec![
                (x - 1, y),
                (x - cell, y - cell / 2),
                (x - cell * 2, y),
                (x - cell, y + cell / 2),
                (x - 1, y),
            ],
            Head::RRight => vec![
                (x - cell, y),
                (x - 1, y - cell),
                (x - 2, y),
                (x - 1, y + cell),
                (x - cell, y),
            ],
            Head::Left => vec![
                (x + 1, y),
                (x + cell, y - cell / 2),
                (x + cell * 2, y),
                (x + cell, y + cell / 2),
                (x + 1, y),
            ],
            Head::RLeft => vec![
                (x + cell, y),
                (x + 1, y - cell),
                (x + 2, y),
                (x + 1, y + cell),
                (x + cell, y),
            ],
        };

        // `if hstyle not in ('composition', 'aggregation'): head.pop(2)`.
        if !matches!(hstyle, Some("composition" | "aggregation")) {
            points.remove(2);
        }
        points
    }
}

/// A routed edge: shaft polylines plus the arrowhead polygons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeRoute {
    pub polylines: Vec<Vec<(i32, i32)>>,
    pub heads: Vec<Vec<(i32, i32)>>,
}

/// Relative placement of an edge's endpoints on the layout grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelDir {
    Right,
    RightUp,
    RightDown,
    Up,
    LeftUp,
    Left,
    LeftDown,
    Down,
    Same,
}

/// One of the eight arrowhead shapes (`EdgeMetrics._head`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Head {
    Up,
    Down,
    Left,
    Right,
    RUp,
    RDown,
    RLeft,
    RRight,
}

/// Port of `blockdiag.metrics.EdgeLines`.
struct Lines {
    current: Option<(i32, i32)>,
    stroking: bool,
    polylines: Vec<Vec<(i32, i32)>>,
}

impl Lines {
    fn new() -> Self {
        Lines {
            current: None,
            stroking: false,
            polylines: Vec::new(),
        }
    }

    fn move_to(&mut self, point: (i32, i32)) {
        self.stroking = false;
        self.current = Some(point);
    }

    fn line_to(&mut self, point: (i32, i32)) {
        if !self.stroking {
            self.stroking = true;
            let mut polyline = Vec::new();
            if let Some(current) = self.current {
                polyline.push(current);
            }
            self.polylines.push(polyline);
        }

        if self
            .polylines
            .last()
            .is_some_and(|line| line.last() == Some(&point))
        {
            return;
        }
        if let Some(line) = self.polylines.last_mut() {
            line.push(point);
        }
    }
}

fn relative_direction(from: &Node, to: &Node) -> RelDir {
    let (x1, y1) = (from.grid.x, from.grid.y);
    let (x2, y2) = (to.grid.x, to.grid.y);
    if x1 > x2 {
        if y1 > y2 {
            RelDir::LeftUp
        } else if y1 == y2 {
            RelDir::Left
        } else {
            RelDir::LeftDown
        }
    } else if x1 == x2 {
        if y1 > y2 {
            RelDir::Up
        } else if y1 == y2 {
            RelDir::Same
        } else {
            RelDir::Down
        }
    } else if y1 > y2 {
        RelDir::RightUp
    } else if y1 == y2 {
        RelDir::Right
    } else {
        RelDir::RightDown
    }
}

fn node_at(diagram: &Diagram, x: i32, y: i32) -> bool {
    diagram
        .nodes
        .iter()
        .any(|node| node.grid.x == x && node.grid.y == y)
}

/// Port of `blockdiag.builder.EdgeLayoutManager.run` (the non-flowchart
/// default). Only `skipped` is observable in edge routing.
fn edge_is_skipped(diagram: &Diagram, from: &Node, to: &Node, rel: RelDir) -> bool {
    let fx = from.grid.x;
    let fy = from.grid.y;
    let tx = to.grid.x;
    let ty = to.grid.y;

    if diagram.orientation == Orientation::Landscape {
        match rel {
            RelDir::Right | RelDir::RightUp => (fx + 1..tx).any(|x| node_at(diagram, x, fy)),
            RelDir::RightDown => (fx + 1..tx).any(|x| node_at(diagram, x, ty)),
            RelDir::LeftDown | RelDir::Down => (fy + 1..ty).any(|y| node_at(diagram, fx, y)),
            RelDir::Up => (ty + 1..fy).any(|y| node_at(diagram, fx, y)),
            _ => false,
        }
    } else {
        match rel {
            RelDir::Right => (fx + 1..tx).any(|x| node_at(diagram, x, fy)),
            RelDir::LeftDown | RelDir::Down => (fy + 1..ty).any(|y| node_at(diagram, fx, y)),
            RelDir::RightDown => (fy + 1..ty).any(|y| node_at(diagram, tx, y)),
            _ => false,
        }
    }
}

/// Apply the `onemany`/`manyone`/`manymany` reverse-head prefixes.
fn reverse_heads(heads: &mut [Option<Head>; 2], hstyle: Option<&str>) {
    let reverse = |head: Head| match head {
        Head::Up => Head::RUp,
        Head::Down => Head::RDown,
        Head::Left => Head::RLeft,
        Head::Right => Head::RRight,
        other => other,
    };
    if matches!(hstyle, Some("manyone" | "manymany")) {
        heads[0] = heads[0].map(reverse);
    }
    if matches!(hstyle, Some("onemany" | "manymany")) {
        heads[1] = heads[1].map(reverse);
    }
}

// ---------------------------------------------------------------------------
// Pixel geometry dump
// ---------------------------------------------------------------------------

/// Emit pixel geometry (node/lane/frame boxes and page size) as deterministic
/// JSON, for `--dump-geometry --pixels` and the metrics compatibility corpus.
pub fn dump_pixels(set: &DocumentSet) -> String {
    let mut out = String::from("{\"diagrams\":[");
    let mut first = true;
    for document in &set.documents {
        for diagram in &document.diagrams {
            if !first {
                out.push(',');
            }
            first = false;
            write_diagram(&mut out, diagram);
        }
    }
    out.push_str("]}");
    out
}

fn write_diagram(out: &mut String, diagram: &Diagram) {
    let metrics = Metrics::new(diagram);
    let (page_width, page_height) = metrics.pagesize();

    out.push_str("{\"pagesize\":[");
    out.push_str(&page_width.to_string());
    out.push(',');
    out.push_str(&page_height.to_string());
    out.push(']');

    let mut nodes: Vec<&Node> = diagram.nodes.iter().collect();
    nodes.sort_by_key(|node| node.order);
    out.push_str(",\"nodes\":[");
    for (i, node) in nodes.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"name\":");
        write_json_string(out, &node.name);
        out.push_str(",\"box\":");
        write_box(out, &metrics.cell(node));
        out.push('}');
    }
    out.push(']');

    out.push_str(",\"lanes\":[");
    for (i, lane) in diagram.lanes.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"textbox\":");
        write_box(out, &metrics.lane_textbox(lane));
        out.push_str(",\"headerbox\":");
        write_box(out, &metrics.lane_headerbox(lane));
        out.push('}');
    }
    out.push(']');

    let frame = metrics.frame(&diagram.lanes);
    out.push_str(",\"frame\":{\"headerbox\":");
    write_box(out, &frame.headerbox);
    out.push_str(",\"outline\":");
    write_box(out, &frame.outline);
    out.push_str(",\"separators\":[");
    for (i, (a, b)) in frame.separators.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('[');
        write_point(out, *a);
        out.push(',');
        write_point(out, *b);
        out.push(']');
    }
    out.push_str("]}");
    out.push('}');
}

fn write_point(out: &mut String, point: (i32, i32)) {
    out.push('[');
    out.push_str(&point.0.to_string());
    out.push(',');
    out.push_str(&point.1.to_string());
    out.push(']');
}

fn write_box(out: &mut String, b: &PixelBox) {
    out.push('[');
    out.push_str(&b.x1.to_string());
    out.push(',');
    out.push_str(&b.y1.to_string());
    out.push(',');
    out.push_str(&b.x2.to_string());
    out.push(',');
    out.push_str(&b.y2.to_string());
    out.push(']');
}

fn write_json_string(out: &mut String, value: &str) {
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build;

    fn node<'a>(diagram: &'a Diagram, name: &str) -> &'a Node {
        let id = diagram.find_node(name).unwrap();
        diagram.node(id)
    }

    #[test]
    fn simple_node_and_page_geometry() {
        let diagram = build::build_str("{ A -> B; }").unwrap();
        let diagram = &diagram.root().diagrams[0];
        let m = Metrics::new(diagram);

        assert_eq!(m.pagesize(), (256, 280));
        assert_eq!(
            m.cell(node(diagram, "A")),
            PixelBox {
                x1: 64,
                y1: 120,
                x2: 192,
                y2: 160
            }
        );
        assert_eq!(
            m.cell(node(diagram, "A")).corebox(),
            PixelBox {
                x1: 68,
                y1: 124,
                x2: 184,
                y2: 152
            }
        );
        assert_eq!(
            m.lane_textbox(&diagram.lanes[0]),
            PixelBox {
                x1: 64,
                y1: 38,
                x2: 192,
                y2: 100
            }
        );
        let frame = m.frame(&diagram.lanes);
        assert_eq!(
            frame.headerbox,
            PixelBox {
                x1: 32,
                y1: 38,
                x2: 224,
                y2: 100
            }
        );
        assert_eq!(
            frame.outline,
            PixelBox {
                x1: 32,
                y1: 38,
                x2: 224,
                y2: 260
            }
        );
        assert_eq!(frame.separators, vec![((32, 100), (224, 100))]);
    }

    #[test]
    fn explicit_width_is_centered_in_its_column() {
        // A is 200px wide; the column grows to 200 and B (128px) is centred in it.
        let set = build::build_str("{ A [width = 200]; A -> B; }").unwrap();
        let diagram = &set.root().diagrams[0];
        let m = Metrics::new(diagram);
        let a = m.cell(node(diagram, "A"));
        let b = m.cell(node(diagram, "B"));
        assert_eq!((a.x1, a.x2), (64, 264));
        assert_eq!((b.x1, b.x2), (64 + 36, 192 + 36));
    }

    #[test]
    fn landscape_pads_on_the_right() {
        let set = build::build_str("{ orientation = landscape; A -> B; }").unwrap();
        let diagram = &set.root().diagrams[0];
        let m = Metrics::new(diagram);
        let a = m.cell(node(diagram, "A"));
        assert_eq!((a.x1, a.y1), (256, 40));
        assert_eq!(m.pagesize(), (640, 120));
    }

    #[test]
    fn simple_edges_route_like_the_reference() {
        let set = build::build_str(include_str!("../tests/fixtures/simple.diag")).unwrap();
        let diagram = &set.root().diagrams[0];
        let m = Metrics::new(diagram);

        let a_to_b = m.edge_route(diagram, &diagram.edges[0]);
        assert_eq!(
            a_to_b.polylines,
            vec![vec![(128, 160), (128, 180), (320, 180), (320, 200)]]
        );
        assert_eq!(
            a_to_b.heads,
            vec![vec![(320, 199), (316, 192), (324, 192), (320, 199)]]
        );

        let b_to_c = m.edge_route(diagram, &diagram.edges[1]);
        assert_eq!(
            b_to_c.polylines,
            vec![vec![(320, 240), (320, 260), (128, 260), (128, 280)]]
        );
        assert_eq!(
            b_to_c.heads,
            vec![vec![(128, 279), (124, 272), (132, 272), (128, 279)]]
        );
    }

    #[test]
    fn undirected_edge_has_no_head() {
        let set = build::build_str("{ A -- B; }").unwrap();
        let diagram = &set.root().diagrams[0];
        let m = Metrics::new(diagram);
        let route = m.edge_route(diagram, &diagram.edges[0]);
        assert!(route.polylines.iter().all(|line| !line.is_empty()));
        assert!(route.heads.is_empty());
    }

    #[test]
    fn edge_labelbox_matches_the_reference() {
        let set = build::build_str(include_str!("../tests/fixtures/simple.diag")).unwrap();
        let diagram = &set.root().diagrams[0];
        let m = Metrics::new(diagram);
        // A -> B is right-down; the label sits above B's top-left corner.
        assert_eq!(
            m.edge_labelbox(diagram, &diagram.edges[0]),
            PixelBox {
                x1: 264,
                y1: 180,
                x2: 312,
                y2: 200
            }
        );
    }
}
