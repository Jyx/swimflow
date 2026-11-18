//! Integer-grid swimlane layout: a line-by-line port of
//! the reference `DiagramLayoutManager` (and `ScreenNodeBuilder.build`).
//!
//! This stage is deliberately font-independent: every extent is a count of
//! character cells. Text measurement only later turns cells into pixels,
//! so `grid::run` is pure integer logic and fully deterministic.
//!
//! Several reference quirks are reproduced deliberately, because they change the
//! resulting geometry:
//!
//! * `adjust_node_order` mutates `diagram.nodes` **while iterating it**, so some
//!   entries are skipped/visited twice;
//! * the `detect_circulars` de-duplication removes entries while iterating;
//! * `set_node_height`'s comparator compares `x.xy.x` with `y.xy.y` (an upstream
//!   typo), not `x` with `x`.
//!
//! Do not "fix" these before parity is established.

use std::collections::HashSet;

use crate::build::model::{
    Diagram, Direction, DocumentSet, Edge, GridPoint, Node, NodeId, Orientation,
};

/// Run the full grid layout pipeline over a diagram.
///
/// Mirrors `ScreenNodeBuilder.build`: layout, lane fixation, then the portrait
/// rotation. Lane assignment is already performed while building the IR.
pub fn run(diagram: &mut Diagram) {
    let mut manager = LayoutManager::new(diagram);
    manager.do_layout();
    manager.fixiate_lanes();
    compute_diagram_grid(manager.diagram);
    if manager.diagram.orientation == Orientation::Portrait {
        rotate_diagram(manager.diagram);
    }
}

/// A snapshot of one edge held by the layout manager.
///
/// Python reads edges from the global `DiagramEdge` registry; the arena keeps
/// them in `Diagram::edges`, so we copy the endpoints once.
struct EdgeInfo {
    from: NodeId,
    to: NodeId,
    folded: bool,
}

struct LayoutManager<'a> {
    diagram: &'a mut Diagram,
    edges: Vec<EdgeInfo>,
    /// Current `diagram.nodes` order (a permutation of the arena indices).
    nodes: Vec<NodeId>,
    /// `NodeId` -> last-committed `.order` (stale during `adjust_node_order`).
    order: Vec<u32>,
    /// `NodeId` -> lane index.
    lane_of: Vec<usize>,
    circulars: Vec<Vec<NodeId>>,
    height_refs: HashSet<usize>,
    /// Occupied cells per lane, keyed by lane index.
    coordinates: Vec<Vec<GridPoint>>,
}

impl<'a> LayoutManager<'a> {
    fn new(diagram: &'a mut Diagram) -> Self {
        let count = diagram.nodes.len();
        let nodes: Vec<NodeId> = (0..count as u32).map(NodeId).collect();
        let order: Vec<u32> = (0..count as u32).collect();
        let lane_of: Vec<usize> = diagram
            .nodes
            .iter()
            .map(|node| node.lane.unwrap_or(0))
            .collect();
        let edges = diagram
            .edges
            .iter()
            .map(|edge| EdgeInfo {
                from: edge.from,
                to: edge.to,
                folded: edge_is_folded(edge),
            })
            .collect();
        let coordinates = vec![Vec::new(); diagram.lanes.len()];

        LayoutManager {
            diagram,
            edges,
            nodes,
            order,
            lane_of,
            circulars: Vec::new(),
            height_refs: HashSet::new(),
            coordinates,
        }
    }

    // -- small accessors -----------------------------------------------------

    fn node(&self, id: NodeId) -> &Node {
        self.diagram.node(id)
    }

    fn x(&self, id: NodeId) -> i32 {
        self.node(id).grid.x
    }

    fn y(&self, id: NodeId) -> i32 {
        self.node(id).grid.y
    }

    fn colwidth(&self, id: NodeId) -> i32 {
        self.node(id).grid.colwidth
    }

    fn colheight(&self, id: NodeId) -> i32 {
        self.node(id).grid.colheight
    }

    fn set_xy(&mut self, id: NodeId, x: i32, y: i32) {
        let mut grid = self.diagram.node(id).grid;
        grid.x = x;
        grid.y = y;
        self.diagram.node_mut(id).grid = grid;
    }

    fn set_y(&mut self, id: NodeId, y: i32) {
        let x = self.x(id);
        self.set_xy(id, x, y);
    }

    // -- graph queries -------------------------------------------------------

    /// Port of `get_related_nodes`: unique adjacent nodes ordered by `.order`.
    fn get_related(&self, node: NodeId, want_parents: bool) -> Vec<NodeId> {
        let mut seen: HashSet<usize> = HashSet::new();
        let mut related: Vec<NodeId> = Vec::new();
        for edge in &self.edges {
            if edge.folded {
                continue;
            }
            let other = if want_parents {
                if edge.to == node {
                    Some(edge.from)
                } else {
                    None
                }
            } else if edge.from == node {
                Some(edge.to)
            } else {
                None
            };

            if let Some(other) = other
                && other != node
                && seen.insert(other.index())
            {
                related.push(other);
            }
        }
        related.sort_by_key(|id| self.order[id.index()]);
        related
    }

    fn get_parent_nodes(&self, node: NodeId) -> Vec<NodeId> {
        self.get_related(node, true)
    }

    fn get_child_nodes(&self, node: NodeId) -> Vec<NodeId> {
        self.get_related(node, false)
    }

    // -- circular detection --------------------------------------------------

    fn detect_circulars(&mut self) {
        let snapshot = self.nodes.clone();
        for node in snapshot {
            if self.circulars.iter().any(|c| c.contains(&node)) {
                continue;
            }
            self.detect_circulars_sub(node, vec![node]);
        }

        // "remove part of other circular" — reproduces upstream's mutation
        // during iteration (including its skipped elements) exactly.
        let mut i = 0;
        while i < self.circulars.len() {
            let c1 = self.circulars[i].clone();
            let c1_set: HashSet<usize> = c1.iter().map(|n| n.index()).collect();
            let mut remove = false;
            for c2 in &self.circulars {
                if *c2 == c1 {
                    continue;
                }
                let c2_set: HashSet<usize> = c2.iter().map(|n| n.index()).collect();
                let intersect: HashSet<usize> = c1_set.intersection(&c2_set).copied().collect();
                if c1_set == intersect {
                    remove = true;
                    break;
                }
            }
            if remove && let Some(pos) = self.circulars.iter().position(|c| *c == c1) {
                self.circulars.remove(pos);
            }
            i += 1;
        }
    }

    fn detect_circulars_sub(&mut self, node: NodeId, parents: Vec<NodeId>) {
        for child in self.get_child_nodes(node) {
            if let Some(i) = parents.iter().position(|&p| p == child) {
                self.circulars.push(parents[i..].to_vec());
            } else {
                let mut next = parents.clone();
                next.push(child);
                self.detect_circulars_sub(child, next);
            }
        }
    }

    fn is_circular_ref(&self, node1: NodeId, node2: NodeId) -> bool {
        for circular in &self.circulars {
            if !(circular.contains(&node1) && circular.contains(&node2)) {
                continue;
            }

            let mut parents: Vec<NodeId> = Vec::new();
            for &node in circular {
                for parent in self.get_parent_nodes(node) {
                    if !circular.contains(&parent) {
                        parents.push(parent);
                    }
                }
            }
            parents.sort_by_key(|id| self.order[id.index()]);

            for parent in parents {
                let children = self.get_child_nodes(parent);
                if children.contains(&node1) && children.contains(&node2) {
                    if index_of(circular, node1) > index_of(circular, node2) {
                        return true;
                    }
                } else if children.contains(&node2) {
                    return true;
                } else if children.contains(&node1) {
                    return false;
                }
            }

            // `for ... else` in upstream: runs when no `return` fired.
            if index_of(circular, node1) > index_of(circular, node2) {
                return true;
            }
        }
        false
    }

    // -- width pass ----------------------------------------------------------

    fn set_node_width(&mut self, depth: i32) {
        let snapshot = self.nodes.clone();
        for node in snapshot {
            if self.x(node) != depth {
                continue;
            }
            for child in self.get_child_nodes(node) {
                let unchanged = self.is_circular_ref(node, child)
                    || node == child
                    || self.x(child) > self.x(node) + self.colwidth(node);
                if !unchanged {
                    let x = self.x(node) + self.colwidth(node);
                    self.set_xy(child, x, 0);
                }
            }
        }

        if self.nodes.iter().any(|&node| self.x(node) > depth) {
            self.set_node_width(depth + 1);
        }
    }

    // -- ordering ------------------------------------------------------------

    fn adjust_node_order(&mut self) {
        // Upstream iterates the list it mutates; an index-based walk with a
        // fresh read each step reproduces that behaviour.
        let mut k = 0;
        while k < self.nodes.len() {
            let node = self.nodes[k];
            let parents = self.get_parent_nodes(node);
            if parents.len() > 1 {
                for i in 1..parents.len() {
                    let prev = parents[i - 1];
                    let cur = parents[i];
                    let idx1 = self.nodes.iter().position(|&n| n == prev).unwrap();
                    let idx2 = self.nodes.iter().position(|&n| n == cur).unwrap();
                    if idx1 < idx2 {
                        self.nodes.remove(idx2);
                        self.nodes.insert(idx1 + 1, cur);
                    } else {
                        self.nodes.remove(idx1);
                        self.nodes.insert(idx2 + 1, prev);
                    }
                }
            }
            k += 1;
        }
        self.update_order();
    }

    fn update_order(&mut self) {
        for (i, &node) in self.nodes.iter().enumerate() {
            self.diagram.node_mut(node).order = i as u32;
            self.order[node.index()] = i as u32;
        }
    }

    // -- height pass ---------------------------------------------------------

    fn initialize_markers(&mut self) {
        self.coordinates = vec![Vec::new(); self.diagram.lanes.len()];
    }

    fn mark_xy(&mut self, node: NodeId) {
        let (x, y, w, h) = (
            self.x(node),
            self.y(node),
            self.colwidth(node),
            self.colheight(node),
        );
        let lane = self.lane_of[node.index()];
        for dx in 0..w {
            for dy in 0..h {
                self.coordinates[lane].push(GridPoint {
                    x: x + dx,
                    y: y + dy,
                });
            }
        }
    }

    fn is_marked(&self, lane: usize, xy: GridPoint) -> bool {
        self.coordinates[lane].contains(&xy)
    }

    fn set_node_height(&mut self, node: NodeId, mut height: i32) -> bool {
        let x = self.x(node);
        let lane = self.lane_of[node.index()];
        if self.is_marked(lane, GridPoint { x, y: height }) {
            return false;
        }
        self.set_xy(node, x, height);
        self.mark_xy(node);

        let mut count = 0;
        // `children.sort(key=cmp_to_key(cmp))` with upstream's typo: compare the
        // first node's `x` with the second node's `y`.
        let mut keyed: Vec<(NodeId, i32, i32)> = self
            .get_child_nodes(node)
            .into_iter()
            .map(|child| (child, self.x(child), self.y(child)))
            .collect();
        keyed.sort_by(|a, b| a.1.cmp(&b.2));

        for (child, _, _) in keyed {
            let child_index = child.index();
            if self.height_refs.contains(&child_index) {
                continue;
            }
            if self.x(node) >= self.x(child) {
                continue;
            }

            let same_lane = lane == self.lane_of[child_index];
            let mut h = if same_lane { height } else { 0 };

            loop {
                if self.set_node_height(child, h) {
                    self.set_y(child, h);
                    self.mark_xy(child);
                    self.height_refs.insert(child_index);
                    count += 1;
                    break;
                } else if !same_lane {
                    h += 1;
                } else if count == 0 {
                    return false;
                } else {
                    h += 1;
                }
            }

            if same_lane {
                height = h + 1;
            }
        }

        true
    }

    // -- top level -----------------------------------------------------------

    #[allow(unused_assignments)]
    fn do_layout(&mut self) {
        self.detect_circulars();
        self.set_node_width(0);
        self.adjust_node_order();

        let mut height = 0;
        self.initialize_markers();
        let snapshot = self.nodes.clone();
        for node in snapshot {
            if self.x(node) == 0 {
                let lane = self.lane_of[node.index()];
                if !self.coordinates[lane].is_empty() {
                    height = self.coordinates[lane]
                        .iter()
                        .map(|point| point.y)
                        .max()
                        .unwrap()
                        + 1;
                } else {
                    height = 0;
                }
                self.set_node_height(node, height);
            }
        }
    }

    fn fixiate_lanes(&mut self) {
        let mut height = 0;
        for lane in 0..self.diagram.lanes.len() {
            if !self.coordinates[lane].is_empty() {
                let lane_height = self.coordinates[lane]
                    .iter()
                    .map(|point| point.y)
                    .max()
                    .unwrap()
                    + 1;
                for node in &mut self.diagram.nodes {
                    if node.lane == Some(lane) {
                        node.grid.y += height;
                    }
                }
                height += lane_height;
            }

            let mut min_x = i32::MAX;
            let mut min_y = i32::MAX;
            let mut max_x = i32::MIN;
            let mut max_y = i32::MIN;
            for node in &self.diagram.nodes {
                if node.lane == Some(lane) {
                    min_x = min_x.min(node.grid.x);
                    min_y = min_y.min(node.grid.y);
                    max_x = max_x.max(node.grid.x + node.grid.colwidth);
                    max_y = max_y.max(node.grid.y + node.grid.colheight);
                }
            }
            if min_x != i32::MAX {
                let grid = &mut self.diagram.lanes[lane].grid;
                grid.x = min_x;
                grid.y = min_y;
                grid.colwidth = max_x - min_x;
                grid.colheight = max_y - min_y;
            }
        }
    }
}

fn index_of(nodes: &[NodeId], node: NodeId) -> usize {
    nodes.iter().position(|&n| n == node).unwrap()
}

fn edge_is_folded(edge: &Edge) -> bool {
    if edge.attrs.contains_key("nofolded") {
        return false;
    }
    edge.attrs.contains_key("folded")
}

fn compute_diagram_grid(diagram: &mut Diagram) {
    if diagram.nodes.is_empty() {
        return;
    }
    let mut width = 0;
    let mut height = 0;
    for node in &diagram.nodes {
        width = width.max(node.grid.x + node.grid.colwidth);
        height = height.max(node.grid.y + node.grid.colheight);
    }
    diagram.grid.x = 0;
    diagram.grid.y = 0;
    diagram.grid.colwidth = width;
    diagram.grid.colheight = height;
}

/// Portrait rotation: swap each element's axes (`ScreenNodeBuilder.rotate_diagram`).
fn rotate_diagram(diagram: &mut Diagram) {
    for node in &mut diagram.nodes {
        std::mem::swap(&mut node.grid.x, &mut node.grid.y);
        std::mem::swap(&mut node.grid.colwidth, &mut node.grid.colheight);
    }
    for lane in &mut diagram.lanes {
        std::mem::swap(&mut lane.grid.x, &mut lane.grid.y);
        std::mem::swap(&mut lane.grid.colwidth, &mut lane.grid.colheight);
    }
    std::mem::swap(&mut diagram.grid.colwidth, &mut diagram.grid.colheight);
}

// ---------------------------------------------------------------------------
// Geometry dump
// ---------------------------------------------------------------------------

/// Emit the grid geometry of every diagram as deterministic JSON.
///
/// Keys and ordering are fixed so the output can be byte-compared in tests.
pub fn dump_geometry(set: &DocumentSet) -> String {
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
    out.push_str("{\"width\":");
    out.push_str(&diagram.grid.colwidth.to_string());
    out.push_str(",\"height\":");
    out.push_str(&diagram.grid.colheight.to_string());

    let mut nodes: Vec<&Node> = diagram.nodes.iter().collect();
    nodes.sort_by_key(|node| node.order);
    out.push_str(",\"nodes\":[");
    for (i, node) in nodes.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"name\":");
        write_json_string(out, &node.name);
        out.push_str(",\"x\":");
        out.push_str(&node.grid.x.to_string());
        out.push_str(",\"y\":");
        out.push_str(&node.grid.y.to_string());
        out.push_str(",\"colwidth\":");
        out.push_str(&node.grid.colwidth.to_string());
        out.push_str(",\"colheight\":");
        out.push_str(&node.grid.colheight.to_string());
        out.push_str(",\"lane\":");
        match node.lane {
            Some(lane) => out.push_str(&lane.to_string()),
            None => out.push_str("null"),
        }
        out.push('}');
    }
    out.push(']');

    out.push_str(",\"lanes\":[");
    for (i, lane) in diagram.lanes.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"x\":");
        out.push_str(&lane.grid.x.to_string());
        out.push_str(",\"y\":");
        out.push_str(&lane.grid.y.to_string());
        out.push_str(",\"colwidth\":");
        out.push_str(&lane.grid.colwidth.to_string());
        out.push_str(",\"colheight\":");
        out.push_str(&lane.grid.colheight.to_string());
        out.push('}');
    }
    out.push(']');

    let mut edges: Vec<&Edge> = diagram.edges.iter().collect();
    edges.sort_by(|a, b| {
        let ak = (
            diagram.node(a.from).name.as_str(),
            diagram.node(a.to).name.as_str(),
        );
        let bk = (
            diagram.node(b.from).name.as_str(),
            diagram.node(b.to).name.as_str(),
        );
        ak.cmp(&bk)
    });
    out.push_str(",\"edges\":[");
    for (i, edge) in edges.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let from = diagram.node(edge.from);
        let to = diagram.node(edge.to);
        out.push_str("{\"from\":");
        write_json_string(out, &from.name);
        out.push_str(",\"to\":");
        write_json_string(out, &to.name);
        out.push_str(",\"dir\":");
        out.push_str(dir_name(edge.dir));
        out.push_str(",\"fromxy\":[");
        out.push_str(&from.grid.x.to_string());
        out.push(',');
        out.push_str(&from.grid.y.to_string());
        out.push_str("],\"toxy\":[");
        out.push_str(&to.grid.x.to_string());
        out.push(',');
        out.push_str(&to.grid.y.to_string());
        out.push_str("]}");
    }
    out.push(']');
    out.push('}');
}

fn dir_name(dir: Direction) -> &'static str {
    match dir {
        Direction::Forward => "\"forward\"",
        Direction::Reverse => "\"back\"",
        Direction::Undirected => "\"none\"",
        Direction::Bidirectional => "\"both\"",
    }
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

    fn diagram(src: &str) -> crate::build::model::DocumentSet {
        build::build_str(src).unwrap_or_else(|e| panic!("build failed for {src:?}: {e}"))
    }

    fn node_xy(src: &str, name: &str) -> (i32, i32) {
        let set = diagram(src);
        let d = &set.root().diagrams[0];
        let id = d.find_node(name).unwrap();
        let grid = d.node(id).grid;
        (grid.x, grid.y)
    }

    #[test]
    fn portrait_rotation_swaps_axes() {
        // A fork lays out along x, then portrait rotates it onto y.
        assert_eq!(node_xy("{ A -> B; A -> C; }", "A"), (0, 0));
        assert_eq!(node_xy("{ A -> B; A -> C; }", "B"), (0, 1));
        assert_eq!(node_xy("{ A -> B; A -> C; }", "C"), (1, 1));
    }

    #[test]
    fn landscape_keeps_axes() {
        let set = diagram("{ orientation = landscape; A -> B; A -> C; }");
        let d = &set.root().diagrams[0];
        assert_eq!((d.grid.colwidth, d.grid.colheight), (2, 2));
        let b = d.node(d.find_node("B").unwrap()).grid;
        let c = d.node(d.find_node("C").unwrap()).grid;
        assert_eq!((b.x, b.y), (1, 0));
        assert_eq!((c.x, c.y), (1, 1));
    }

    #[test]
    fn cycles_collapse_to_a_single_column() {
        let set = diagram("{ A -> B; B -> C; C -> A; }");
        let d = &set.root().diagrams[0];
        assert_eq!((d.grid.colwidth, d.grid.colheight), (1, 3));
        for (i, name) in ["A", "B", "C"].iter().enumerate() {
            assert_eq!(node_xy("{ A -> B; B -> C; C -> A; }", name), (0, i as i32));
        }
    }

    #[test]
    fn layout_is_deterministic() {
        let src = "{ lane a { A; B; } lane b { C; } A -> C -> B; A -> B; }";
        assert_eq!(dump_geometry(&diagram(src)), dump_geometry(&diagram(src)));
    }
}
