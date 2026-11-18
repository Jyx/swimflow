//! Layout: the tree -> pixel geometry.
//!
//! A tidy-tree layout in two passes. The first pass measures every node and
//! the *breadth* (the extent across the tree) of every subtree; the second
//! one places the leaves side by side, centers every parent over its
//! children and aligns the nodes of a level. In the default (portrait)
//! orientation a level is a row and the root is on top; in landscape the
//! axes are swapped and the root is on the left.
//!
//! Text is measured with the shared text measurer, so the tree diagram uses
//! the same font handling as every other diagram type.

use std::collections::BTreeSet;

use crate::core::geom::PixelBox;
use crate::text::TextMeasurer;

use super::ast::{Attr, Node, Tree, attr_value};

/// Space around the drawing.
pub const MARGIN: i32 = 24;
/// Horizontal padding inside a node.
pub const PADDING_X: i32 = 14;
/// Vertical padding inside a node.
pub const PADDING_Y: i32 = 8;
/// Minimum width of a node.
pub const MIN_WIDTH: i32 = 80;
/// Minimum height of a node.
pub const MIN_HEIGHT: i32 = 34;
/// The widest a node may get; longer labels wrap.
pub const MAX_WIDTH: i32 = 220;
/// Space between two siblings.
pub const SIBLING_GAP: i32 = 20;
/// Space between two levels (before edge labels make it larger).
pub const LEVEL_GAP: i32 = 48;
/// Padding around an edge label.
pub const LABEL_PADDING: i32 = 4;

/// The direction the tree grows in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// The root is on top, the children are below it.
    Portrait,
    /// The root is on the left, the children are to its right.
    Landscape,
}

impl Orientation {
    /// The orientation named by the `orientation` attribute of a tree.
    pub fn of(tree: &Tree) -> Orientation {
        match attr_value(&tree.attrs, "orientation") {
            Some(value) if value.eq_ignore_ascii_case("landscape") => Orientation::Landscape,
            _ => Orientation::Portrait,
        }
    }
}

/// Radius of the circle that collapses or expands a branch.
pub const TOGGLE_RADIUS: i32 = 7;

/// The complete geometry of a tree diagram.
#[derive(Debug, Clone)]
pub struct Layout {
    pub width: i32,
    pub height: i32,
    /// The nodes; index 0 is the root. Parents come before their children.
    pub nodes: Vec<NodeLayout>,
    /// The trunk of every parent: the stem and the bus its children hang from.
    pub trunks: Vec<TrunkLayout>,
    /// The drops from the bus into the children, one per non-root node.
    pub edges: Vec<EdgeLayout>,
}

/// A placed node.
#[derive(Debug, Clone)]
pub struct NodeLayout {
    pub label: String,
    pub attrs: Vec<Attr>,
    /// The box of the node. A hidden node keeps its size and sits on the box
    /// of its parent, so it has somewhere to expand from.
    pub box_: PixelBox,
    /// The distance from the root.
    pub depth: usize,
    /// Index of the parent (`None` for the root).
    pub parent: Option<usize>,
    /// Whether the node is drawn (it is not when an ancestor is collapsed).
    pub visible: bool,
    /// Whether the children of the node are hidden.
    pub collapsed: bool,
    /// Whether the node has children at all.
    pub has_children: bool,
    /// The center of the collapse/expand circle (nodes with children only):
    /// the middle of the bottom edge, or of the right edge in landscape.
    pub toggle: Option<(i32, i32)>,
}

impl NodeLayout {
    pub fn attr(&self, key: &str) -> Option<&str> {
        attr_value(&self.attrs, key)
    }
}

/// The part of the connectors that belongs to a parent.
#[derive(Debug, Clone)]
pub struct TrunkLayout {
    /// Index of the parent.
    pub parent: usize,
    /// The stem from the parent to the bus (empty when nothing is shown).
    pub stem: Vec<(i32, i32)>,
    /// Whether the stem is dashed: every child hangs from it with an
    /// `edgestyle = dashed`, so no solid connector shares it.
    pub stem_dashed: bool,
    /// Whether any child hangs from the bus with a dashed connector (then the
    /// trunk has a dashed bus path, even while it is folded away).
    pub has_dashed: bool,
    /// The solid stretches of the bus across the children (empty when it is
    /// not needed).
    pub bus: Vec<Vec<(i32, i32)>>,
    /// The dashed stretches of the bus. A stretch is dashed only where every
    /// child that uses it has `edgestyle = dashed`; where a solid connector
    /// merges in, the solid line wins.
    pub bus_dashed: Vec<Vec<(i32, i32)>>,
}

/// A connector from a parent to a child.
#[derive(Debug, Clone)]
pub struct EdgeLayout {
    /// Index of the child it leads to.
    pub child: usize,
    /// The drop from the bus (empty when the child is hidden).
    pub polyline: Vec<(i32, i32)>,
    /// Whether the drop is dashed (`edgestyle = dashed`).
    pub dashed: bool,
    /// The label of the connector and the box it is drawn in.
    pub label: Option<(String, PixelBox)>,
}

/// A node while the layout is computed.
struct Flat {
    label: String,
    attrs: Vec<Attr>,
    children: Vec<usize>,
    parent: Option<usize>,
    depth: usize,
    width: i32,
    height: i32,
    /// The size of the connector label of this node, if it has one.
    edge_label: Option<(String, i32, i32)>,
    collapsed: bool,
    visible: bool,
}

/// The label a node is drawn with.
fn label_of(node: &Node) -> String {
    node.attr("label").unwrap_or(&node.name).to_string()
}

/// Compute the geometry of a tree in the state its source asks for.
pub fn layout(tree: &Tree, text: &mut TextMeasurer) -> Layout {
    layout_with(tree, text, None)
}

/// Compute the geometry of a tree.
///
/// `folded` is the complete set of nodes (by index) whose children are folded
/// away; `None` keeps the `collapsed` attributes of the source.
pub fn layout_with(
    tree: &Tree,
    text: &mut TextMeasurer,
    folded: Option<&BTreeSet<usize>>,
) -> Layout {
    let orientation = Orientation::of(tree);

    // 1. Flatten the tree (parents before children) and measure the nodes.
    let mut flat: Vec<Flat> = Vec::new();
    let root_label = tree
        .root
        .name
        .clone()
        .or_else(|| tree.id.clone())
        .unwrap_or_else(|| "root".to_string());
    let root_label = attr_value(&tree.root.attrs, "label")
        .map(str::to_string)
        .unwrap_or(root_label);
    flat.push(measured(root_label, tree.root.attrs.clone(), None, 0, text));

    // An explicit stack keeps the order stable without recursion.
    let mut stack: Vec<(usize, &[Node])> = vec![(0, &tree.children)];
    let mut queue_index = 0;
    while queue_index < stack.len() {
        let (parent, nodes) = stack[queue_index];
        queue_index += 1;
        for node in nodes {
            let index = flat.len();
            let depth = flat[parent].depth + 1;
            flat.push(measured(
                label_of(node),
                node.attrs.clone(),
                Some(parent),
                depth,
                text,
            ));
            flat[parent].children.push(index);
            stack.push((index, &node.children));
        }
    }

    if let Some(folded) = folded {
        for (index, node) in flat.iter_mut().enumerate() {
            node.collapsed = folded.contains(&index);
        }
    }

    // Which nodes are drawn, and which children are laid out.
    for index in 1..flat.len() {
        let parent = flat[index].parent.expect("a non-root node has a parent");
        flat[index].visible = flat[parent].visible && !flat[parent].collapsed;
    }
    let shown: Vec<Vec<usize>> = flat
        .iter()
        .map(|node| {
            if node.visible && !node.collapsed {
                node.children.clone()
            } else {
                Vec::new()
            }
        })
        .collect();

    // 2. The breadth of every subtree. Children have larger indexes than their
    //    parents, so walking backwards visits children first.
    let breadth_of = |node: &Flat| match orientation {
        Orientation::Portrait => node.width,
        Orientation::Landscape => node.height,
    };
    let depth_of = |node: &Flat| match orientation {
        Orientation::Portrait => node.height,
        Orientation::Landscape => node.width,
    };

    let mut subtree = vec![0i32; flat.len()];
    for index in (0..flat.len()).rev() {
        if !flat[index].visible {
            continue;
        }
        let total = children_extent(&shown[index], &subtree);
        subtree[index] = breadth_of(&flat[index]).max(total);
    }

    // 3. The position of every node across the tree (the center).
    let mut center = vec![0i32; flat.len()];
    let mut start = vec![0i32; flat.len()];
    start[0] = MARGIN;
    for index in 0..flat.len() {
        if !flat[index].visible {
            continue;
        }
        let total = children_extent(&shown[index], &subtree);
        let mut cursor = start[index] + (subtree[index] - total) / 2;
        for &child in &shown[index] {
            start[child] = cursor;
            cursor += subtree[child] + SIBLING_GAP;
        }
        center[index] = start[index] + subtree[index] / 2;
    }
    // A parent is centered over the span of its first and last child.
    for index in (0..flat.len()).rev() {
        if let (Some(&first), Some(&last)) = (shown[index].first(), shown[index].last()) {
            center[index] = (center[first] + center[last]) / 2;
        }
    }

    // 4. The levels: their thickness and the gaps between them.
    let levels = flat
        .iter()
        .filter(|node| node.visible)
        .map(|node| node.depth)
        .max()
        .unwrap_or(0)
        + 1;
    let mut thickness = vec![0i32; levels];
    let mut gap = vec![LEVEL_GAP; levels];
    for node in flat.iter().filter(|node| node.visible) {
        thickness[node.depth] = thickness[node.depth].max(depth_of(node));
        if let Some((_, label_width, label_height)) = &node.edge_label {
            // The label sits on the last half of the connector into the node.
            let needed = match orientation {
                Orientation::Portrait => *label_height,
                Orientation::Landscape => *label_width,
            };
            let level = node.depth.saturating_sub(1);
            gap[level] = gap[level].max(2 * (needed + 8));
        }
    }
    let mut offset = vec![0i32; levels];
    let mut cursor = MARGIN;
    for level in 0..levels {
        offset[level] = cursor;
        cursor += thickness[level] + gap[level];
    }
    let depth_total = offset[levels - 1] + thickness[levels - 1] + MARGIN;
    let breadth_total = subtree[0] + 2 * MARGIN;

    // 5. Boxes. The nodes of a level start on the same line, so the bus of a
    //    parent touches all of its children. A hidden node sits on its parent.
    let mut nodes: Vec<NodeLayout> = Vec::with_capacity(flat.len());
    for (index, node) in flat.iter().enumerate() {
        let box_ = if node.visible {
            let along = offset[node.depth];
            match orientation {
                Orientation::Portrait => {
                    let x1 = center[index] - node.width / 2;
                    PixelBox {
                        x1,
                        y1: along,
                        x2: x1 + node.width,
                        y2: along + node.height,
                    }
                }
                Orientation::Landscape => {
                    let y1 = center[index] - node.height / 2;
                    PixelBox {
                        x1: along,
                        y1,
                        x2: along + node.width,
                        y2: y1 + node.height,
                    }
                }
            }
        } else {
            let parent = nodes[node.parent.expect("a hidden node has a parent")].box_;
            PixelBox {
                x1: parent.x1,
                y1: parent.y1,
                x2: parent.x1 + node.width,
                y2: parent.y1 + node.height,
            }
        };
        let has_children = !node.children.is_empty();
        nodes.push(NodeLayout {
            label: node.label.clone(),
            attrs: node.attrs.clone(),
            box_,
            depth: node.depth,
            parent: node.parent,
            visible: node.visible,
            collapsed: node.collapsed,
            has_children,
            toggle: has_children.then(|| match orientation {
                // The circle sits where the connector leaves the node.
                Orientation::Portrait => box_.bottom(),
                Orientation::Landscape => box_.right(),
            }),
        });
    }

    // 6. Connectors. A parent has one trunk (the stem down to the bus and the
    //    bus across its children) and every child has a drop from the bus; this
    //    way no segment is drawn twice.
    let mut trunks = Vec::new();
    for (index, node) in flat.iter().enumerate() {
        if node.children.is_empty() {
            continue;
        }
        let mut trunk = TrunkLayout {
            parent: index,
            stem: Vec::new(),
            stem_dashed: false,
            has_dashed: false,
            bus: Vec::new(),
            bus_dashed: Vec::new(),
        };
        if let (Some(&first), Some(&last)) = (shown[index].first(), shown[index].last()) {
            let from = nodes[index].box_;
            let level_gap = gap[node.depth];
            match orientation {
                Orientation::Portrait => {
                    let bus = nodes[first].box_.y1 - level_gap / 2;
                    let (px, py) = from.bottom();
                    let (lo, hi) = (nodes[first].box_.top().0, nodes[last].box_.top().0);
                    trunk.stem = vec![(px, py), (px, bus)];
                    if lo != hi {
                        let stops = stops_of(&shown[index], &nodes, |b| b.top().0);
                        let (solid, dashed) = bus_stretches(px, &stops);
                        let at = |&(a, b): &(i32, i32)| vec![(a, bus), (b, bus)];
                        trunk.bus = solid.iter().map(at).collect();
                        trunk.bus_dashed = dashed.iter().map(at).collect();
                    }
                }
                Orientation::Landscape => {
                    let bus = nodes[first].box_.x1 - level_gap / 2;
                    let (px, py) = from.right();
                    let (lo, hi) = (nodes[first].box_.left().1, nodes[last].box_.left().1);
                    trunk.stem = vec![(px, py), (bus, py)];
                    if lo != hi {
                        let stops = stops_of(&shown[index], &nodes, |b| b.left().1);
                        let (solid, dashed) = bus_stretches(py, &stops);
                        let at = |&(a, b): &(i32, i32)| vec![(bus, a), (bus, b)];
                        trunk.bus = solid.iter().map(at).collect();
                        trunk.bus_dashed = dashed.iter().map(at).collect();
                    }
                }
            }
        }
        // Folding must not change the look of a connector, so this looks at
        // every child, shown or not.
        trunk.stem_dashed = node
            .children
            .iter()
            .all(|&child| has_dashed_edge(&nodes[child]));
        trunk.has_dashed = node
            .children
            .iter()
            .any(|&child| has_dashed_edge(&nodes[child]));
        trunks.push(trunk);
    }

    let mut edges = Vec::new();
    for (index, node) in flat.iter().enumerate() {
        if node.parent.is_none() {
            continue;
        }
        let to = nodes[index].box_;
        let (polyline, anchor) = if node.visible {
            let level_gap = gap[node.depth - 1];
            match orientation {
                Orientation::Portrait => {
                    let bus = to.y1 - level_gap / 2;
                    let (cx, cy) = to.top();
                    (vec![(cx, bus), (cx, cy)], (cx, (bus + cy) / 2))
                }
                Orientation::Landscape => {
                    let bus = to.x1 - level_gap / 2;
                    let (cx, cy) = to.left();
                    (vec![(bus, cy), (cx, cy)], ((bus + cx) / 2, cy))
                }
            }
        } else {
            (Vec::new(), (to.x1, to.y1))
        };
        let label = node.edge_label.as_ref().map(|(text, width, height)| {
            (
                text.clone(),
                PixelBox {
                    x1: anchor.0 - width / 2,
                    y1: anchor.1 - height / 2,
                    x2: anchor.0 - width / 2 + width,
                    y2: anchor.1 - height / 2 + height,
                },
            )
        });
        edges.push(EdgeLayout {
            child: index,
            polyline,
            dashed: has_dashed_edge(&nodes[index]),
            label,
        });
    }

    let (width, height) = match orientation {
        Orientation::Portrait => (breadth_total, depth_total),
        Orientation::Landscape => (depth_total, breadth_total),
    };

    Layout {
        width,
        height,
        nodes,
        trunks,
        edges,
    }
}

/// Whether the connector into a node is dashed: `edgestyle = dashed`.
///
/// (`style` is taken: it dashes the outline of the node itself.)
pub fn has_dashed_edge(node: &NodeLayout) -> bool {
    node.attr("edgestyle")
        .is_some_and(|value| value.eq_ignore_ascii_case("dashed"))
}

/// The positions along the bus of the shown children, with whether the
/// connector into each one is dashed.
fn stops_of(
    shown: &[usize],
    nodes: &[NodeLayout],
    along: impl Fn(&PixelBox) -> i32,
) -> Vec<(i32, bool)> {
    shown
        .iter()
        .map(|&child| (along(&nodes[child].box_), has_dashed_edge(&nodes[child])))
        .collect()
}

/// Split the bus of a trunk into solid and dashed stretches.
///
/// `origin` is where the stem meets the bus and `stops` the children along it.
/// A stretch of the bus carries every child beyond it (seen from the stem), so
/// it is dashed only when all of those children are dashed; a solid line that
/// merges in takes precedence. Neighbouring stretches of one kind are joined.
/// Stretches of a bus as `(from, to)` positions along it.
type Stretches = Vec<(i32, i32)>;

fn bus_stretches(origin: i32, stops: &[(i32, bool)]) -> (Stretches, Stretches) {
    let mut points: Vec<i32> = stops.iter().map(|&(at, _)| at).collect();
    points.push(origin);
    points.sort_unstable();
    points.dedup();

    let mut solid: Vec<(i32, i32)> = Vec::new();
    let mut dashed: Vec<(i32, i32)> = Vec::new();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let beyond = |&&(at, _): &&(i32, bool)| if a >= origin { at >= b } else { at <= a };
        let dash = stops.iter().filter(beyond).all(|&(_, dash)| dash);
        let target = if dash { &mut dashed } else { &mut solid };
        match target.last_mut() {
            Some(last) if last.1 == a => last.1 = b,
            _ => target.push((a, b)),
        }
    }
    (solid, dashed)
}

/// The breadth the children of a node need side by side.
fn children_extent(children: &[usize], subtree: &[i32]) -> i32 {
    if children.is_empty() {
        return 0;
    }
    let sum: i32 = children.iter().map(|&child| subtree[child]).sum();
    sum + SIBLING_GAP * (children.len() as i32 - 1)
}

/// Measure a node and the label of its connector.
fn measured(
    label: String,
    attrs: Vec<Attr>,
    parent: Option<usize>,
    depth: usize,
    text: &mut TextMeasurer,
) -> Flat {
    let size = text.measure(&label, Some((MAX_WIDTH - 2 * PADDING_X) as f32));
    let width = ((size.width.ceil() as i32) + 2 * PADDING_X).max(MIN_WIDTH);
    let height = ((size.height.ceil() as i32) + 2 * PADDING_Y).max(MIN_HEIGHT);

    let edge_label = attr_value(&attrs, "edge")
        .filter(|label| !label.is_empty() && parent.is_some())
        .map(|label| {
            let size = text.measure(label, None);
            (
                label.to_string(),
                size.width.ceil() as i32 + 2 * LABEL_PADDING,
                size.height.ceil() as i32 + 2 * LABEL_PADDING,
            )
        });

    let collapsed = is_collapsed(&attrs);
    Flat {
        label,
        attrs,
        children: Vec::new(),
        parent,
        depth,
        width,
        height,
        edge_label,
        collapsed,
        visible: true,
    }
}

/// Whether the attributes ask for the children to be hidden: a bare
/// `collapsed`, or `collapsed = true`.
pub fn is_collapsed(attrs: &[Attr]) -> bool {
    match attrs.iter().rev().find(|attr| attr.key == "collapsed") {
        None => false,
        Some(attr) => match attr.value.as_deref() {
            None => true,
            Some(value) => !matches!(
                value.to_ascii_lowercase().as_str(),
                "false" | "no" | "off" | "0"
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::super::parser::parse;
    use super::*;

    fn laid_out(source: &str) -> Layout {
        let document = parse(source).unwrap();
        let mut text = TextMeasurer::default_size();
        layout(&document.trees[0], &mut text)
    }

    #[test]
    fn a_parent_is_centered_over_its_children() {
        let layout = laid_out("tree { root R; A; B; C; }");
        let root = layout.nodes[0].box_;
        let first = layout.nodes[1].box_;
        let last = layout.nodes[3].box_;
        let span_center = (first.x1 + last.x2) / 2;
        assert!((root.x1 + root.x2) / 2 - span_center <= 1);
        assert!(root.y2 < first.y1, "children are below the root");
        assert_eq!(first.y1, last.y1, "siblings share a level");
    }

    #[test]
    fn siblings_never_overlap() {
        let layout = laid_out("tree { A { A1; A2; A3; } B { B1; B2; } C; }");
        let mut boxes: Vec<PixelBox> = layout.nodes.iter().map(|n| n.box_).collect();
        boxes.sort_by_key(|b| (b.y1, b.x1));
        for pair in boxes.windows(2) {
            if pair[0].y1 == pair[1].y1 {
                assert!(pair[0].x2 + SIBLING_GAP <= pair[1].x1, "{pair:?}");
            }
        }
        for node in &layout.nodes {
            assert!(node.box_.x1 >= MARGIN && node.box_.x2 <= layout.width - MARGIN);
            assert!(node.box_.y2 <= layout.height - MARGIN);
        }
    }

    #[test]
    fn landscape_grows_to_the_right() {
        let layout = laid_out("tree { orientation = landscape; A; B; }");
        assert!(layout.nodes[0].box_.x2 < layout.nodes[1].box_.x1);
        assert_eq!(layout.nodes[1].box_.x1, layout.nodes[2].box_.x1);
        assert!(layout.nodes[1].box_.y2 < layout.nodes[2].box_.y1);
    }

    #[test]
    fn the_toggle_sits_where_the_connector_leaves_the_node() {
        let portrait = laid_out("tree { A { B; } }");
        let a = &portrait.nodes[1];
        assert_eq!(a.toggle, Some(a.box_.bottom()));
        assert_eq!(portrait.nodes[2].toggle, None, "a leaf has no toggle");

        let landscape = laid_out("tree { orientation = landscape; A { B; } }");
        let root = &landscape.nodes[0];
        assert_eq!(root.toggle, Some(root.box_.right()));
        let a = &landscape.nodes[1];
        assert_eq!(a.toggle, Some(a.box_.right()));
        // The stem starts on the circle.
        assert_eq!(landscape.trunks[0].stem[0], root.box_.right());
    }

    #[test]
    fn every_node_but_the_root_has_a_connector() {
        let layout = laid_out("tree { A { B; C; } D; }");
        assert_eq!(layout.nodes.len(), 5);
        assert_eq!(layout.edges.len(), 4);
        assert!(layout.nodes[0].parent.is_none());
    }

    #[test]
    fn edge_labels_widen_the_gap() {
        let plain = laid_out("tree { A; }");
        let labelled = laid_out("tree { A [edge = yes]; }");
        assert!(labelled.height > plain.height);
        let label = labelled.edges[0].label.as_ref().expect("a label").1;
        let node = labelled.nodes[1].box_;
        assert!(label.y2 <= node.y1, "the label is above the node");
    }

    #[test]
    fn the_root_label_comes_from_root_then_the_tree_name() {
        assert_eq!(laid_out("tree { root X; }").nodes[0].label, "X");
        assert_eq!(laid_out("tree T { }").nodes[0].label, "T");
        assert_eq!(laid_out("tree { }").nodes[0].label, "root");
    }

    #[test]
    fn a_dashed_stretch_ends_where_a_solid_connector_merges() {
        // Stem at 0; stops at -10 (solid), 10 (dashed), 30 (dashed).
        let (solid, dashed) = bus_stretches(0, &[(-10, false), (10, true), (30, true)]);
        assert_eq!(solid, vec![(-10, 0)]);
        assert_eq!(dashed, vec![(0, 30)]);
        // A solid child beyond a dashed one makes the shared stretch solid.
        let (solid, dashed) = bus_stretches(0, &[(10, true), (30, false)]);
        assert_eq!(solid, vec![(0, 30)]);
        assert!(dashed.is_empty());
        // The dashed child at the far end keeps its own stretch.
        let (solid, dashed) = bus_stretches(0, &[(10, false), (30, true)]);
        assert_eq!(solid, vec![(0, 10)]);
        assert_eq!(dashed, vec![(10, 30)]);
    }

    #[test]
    fn the_stem_is_dashed_only_when_every_child_is() {
        let all = laid_out("tree { A [edgestyle = dashed]; B [edgestyle = dashed]; }");
        assert!(all.trunks[0].stem_dashed);
        let some = laid_out("tree { A [edgestyle = dashed]; B; }");
        assert!(!some.trunks[0].stem_dashed);
        assert!(some.edges[0].dashed && !some.edges[1].dashed);
    }

    #[test]
    fn the_layout_is_deterministic() {
        let source = "tree { A { B; C { D; } } E; }";
        let a = laid_out(source);
        let b = laid_out(source);
        let boxes = |l: &Layout| l.nodes.iter().map(|n| n.box_).collect::<Vec<_>>();
        assert_eq!(boxes(&a), boxes(&b));
    }
}
