//! Intermediate representation (IR) for a built diagram document.
//!
//! This is the forward-compatible model: link targets, sequence numbers,
//! legends and multi-diagram documents are all first-class here, even though
//! later milestones are what actually render them.
//!
//! Compared to the reference implementation, the process-global `Element.get()` registries
//! are replaced by plain `Vec`-backed arenas addressed with typed ids
//! (`NodeId`). Layout and rendering therefore operate on an owned, pure data
//! structure with no hidden global state.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::build::link::{Link, ResolvedLink};

/// Attribute name -> raw (unquoted) value. A `None` value corresponds to a
/// bare attribute such as `[stacked]`.
pub type AttrMap = BTreeMap<String, Option<String>>;

/// A document-level diagram name (or `"default"` for an unnamed block).
pub type DiagramId = String;

/// Integer grid point (character-cell coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GridPoint {
    pub x: i32,
    pub y: i32,
}

/// Integer grid rectangle in character cells.
///
/// `x`/`y` are the top-left cell, `colwidth`/`colheight` the extent in cells.
/// The default is a single 1x1 cell, matching `Element.__init__` in blockdiag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridRect {
    pub x: i32,
    pub y: i32,
    pub colwidth: i32,
    pub colheight: i32,
}

impl Default for GridRect {
    fn default() -> Self {
        GridRect {
            x: 0,
            y: 0,
            colwidth: 1,
            colheight: 1,
        }
    }
}

/// Index of a node within a [`Diagram`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u32);

impl NodeId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Edge direction, mirroring the DSL operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// `->`
    Forward,
    /// `<-`
    Reverse,
    /// `--`
    Undirected,
    /// `<->`
    Bidirectional,
}

/// Diagram orientation (defaults to portrait).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

/// Sequence-number policy for a diagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Numbering {
    #[default]
    Off,
    Auto,
    Topological,
    Depth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LegendPosition {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegendEntry {
    /// Reference to a `class`, with an optional label override.
    Class {
        name: String,
        label: Option<String>,
        attrs: AttrMap,
    },
    /// A standalone swatch row.
    Free { label: String, attrs: AttrMap },
    /// Explanation of the sequence-number badge.
    SequenceKey,
    /// A description-list row (port of the RST `:desctable:` idea).
    Description {
        number: Option<u32>,
        label: String,
        description: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Legend {
    pub title: Option<String>,
    pub position: LegendPosition,
    pub entries: Vec<LegendEntry>,
}

/// A node in a diagram.
#[derive(Debug, Clone)]
pub struct Node {
    pub name: String,
    pub label: String,
    pub class: Option<String>,
    /// Index into [`Diagram::lanes`].
    pub lane: Option<usize>,
    /// Explicit sequence number (automatic numbering is assigned by
    /// `build::numbering`).
    pub number: Option<u32>,
    /// `numbered = none`: opt out of automatic numbering.
    pub number_disabled: bool,
    pub description: Option<String>,
    pub link: Option<Link>,
    /// Filled by the link-resolution pass.
    pub resolved: Option<ResolvedLink>,
    pub shape: Option<String>,
    pub color: Option<String>,
    pub textcolor: Option<String>,
    pub linecolor: Option<String>,
    /// Explicit pixel size (`width`/`height` attributes). `None` uses the
    /// diagram defaults; see `metrics.rs`.
    pub width: Option<i32>,
    pub height: Option<i32>,
    /// Integer grid placement, filled by `grid::run`.
    pub grid: GridRect,
    /// Layout order, a permutation of the creation index.
    pub order: u32,
    /// All attributes, preserved verbatim (forward compatibility).
    pub attrs: AttrMap,
}

impl Node {
    fn new(name: String) -> Self {
        Node {
            label: name.clone(),
            name,
            class: None,
            lane: None,
            number: None,
            number_disabled: false,
            description: None,
            link: None,
            resolved: None,
            shape: None,
            color: None,
            textcolor: None,
            linecolor: None,
            width: None,
            height: None,
            grid: GridRect::default(),
            order: 0,
            attrs: AttrMap::new(),
        }
    }
}

/// A directed (or undirected) edge between two nodes.
#[derive(Debug, Clone)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub dir: Direction,
    pub label: Option<String>,
    pub color: Option<String>,
    pub style: Option<String>,
    pub attrs: AttrMap,
}

/// A swimlane (activity lane).
#[derive(Debug, Clone)]
pub struct Lane {
    /// `None` for anonymous lanes. Distinct anonymous lanes are distinguished
    /// by their position in [`Diagram::lanes`].
    pub id: Option<String>,
    pub label: Option<String>,
    pub color: Option<String>,
    pub href: Option<String>,
    pub nodes: Vec<NodeId>,
    /// Integer grid placement, filled by `grid::run`.
    pub grid: GridRect,
    pub attrs: AttrMap,
}

impl Lane {
    pub(crate) fn new(id: Option<String>) -> Self {
        Lane {
            id,
            label: None,
            color: None,
            href: None,
            nodes: Vec::new(),
            grid: GridRect::default(),
            attrs: AttrMap::new(),
        }
    }
}

/// A single diagram block.
#[derive(Debug, Clone)]
pub struct Diagram {
    pub id: DiagramId,
    pub nodes: Vec<Node>,
    /// Author-visible (unquoted) node name -> index.
    pub name_index: BTreeMap<String, NodeId>,
    pub edges: Vec<Edge>,
    pub lanes: Vec<Lane>,
    pub classes: BTreeMap<String, AttrMap>,
    pub plugins: Vec<(String, AttrMap)>,
    pub attrs: AttrMap,
    pub orientation: Orientation,
    pub numbering: Numbering,
    pub legend: Option<Legend>,
    /// Pixel-metrics overrides from diagram attributes (`node_width`,
    /// `node_height`, `span_width`, `span_height`); `None` uses defaults.
    pub node_width: Option<i32>,
    pub node_height: Option<i32>,
    pub span_width: Option<i32>,
    pub span_height: Option<i32>,
    /// Overall grid extent (`colwidth` x `colheight`), filled by `grid::run`.
    pub grid: GridRect,
}

impl Diagram {
    pub fn new(id: impl Into<String>) -> Self {
        Diagram {
            id: id.into(),
            nodes: Vec::new(),
            name_index: BTreeMap::new(),
            edges: Vec::new(),
            lanes: Vec::new(),
            classes: BTreeMap::new(),
            plugins: Vec::new(),
            attrs: AttrMap::new(),
            orientation: Orientation::Portrait,
            numbering: Numbering::Off,
            legend: None,
            node_width: None,
            node_height: None,
            span_width: None,
            span_height: None,
            grid: GridRect::default(),
        }
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }

    pub fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id.index()]
    }

    /// Intern a node by its unquoted name, creating it on first use.
    pub fn intern(&mut self, name: &str) -> NodeId {
        if let Some(&id) = self.name_index.get(name) {
            return id;
        }
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(Node::new(name.to_string()));
        self.name_index.insert(name.to_string(), id);
        id
    }

    /// Resolve an author-visible node name.
    pub fn find_node(&self, name: &str) -> Option<NodeId> {
        self.name_index.get(name).copied()
    }
}

/// A parsed file: one or more diagram blocks with a shared source path.
#[derive(Debug, Clone, Default)]
pub struct Document {
    pub source: Option<PathBuf>,
    pub diagrams: Vec<Diagram>,
    /// Diagram id -> index in `diagrams`.
    pub index: BTreeMap<DiagramId, usize>,
    /// Non-fatal warnings collected while building (e.g. duplicate numbers).
    pub warnings: Vec<String>,
}

impl Document {
    pub fn diagram(&self, id: &str) -> Option<&Diagram> {
        self.index.get(id).map(|&i| &self.diagrams[i])
    }
}

/// The result of loading a root file plus any referenced files.
#[derive(Debug, Clone, Default)]
pub struct DocumentSet {
    pub documents: Vec<Document>,
    /// Index of the entry document in `documents`.
    pub root: usize,
    pub issues: Vec<LinkIssue>,
}

impl DocumentSet {
    pub fn root(&self) -> &Document {
        &self.documents[self.root]
    }
}

/// A non-fatal problem found while resolving links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkIssue {
    pub file: String,
    pub node: String,
    pub message: String,
}
