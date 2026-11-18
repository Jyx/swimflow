//! Syntactic tree produced by the parser.
//!
//! This mirrors the parse tree of the reference parser:
//!   * `Node`, `Edge`, `Attr`, `Extension`, `Lane`, `Diagram`.
//!
//! The original wraps node/edge statements in a `Statements` node; here those
//! expand directly into a `Vec<Stmt>` because the grouping carries no meaning
//! for downstream stages. The `Attr`/`Extension` reordering performed by the
//! original `sort_tree` is preserved (see `crate::parser`).

/// One or more diagram blocks in a document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document {
    pub diagrams: Vec<Diagram>,
}

/// A `[diagram|actdiag] [id] { ... }` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagram {
    /// Optional diagram name. `None` for an unnamed (anonymous) block.
    pub id: Option<String>,
    pub stmts: Vec<Stmt>,
}

/// A single statement inside a diagram or lane body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Node(NodeStmt),
    Attr(Attr),
    Edge(EdgeStmt),
    Extension(Extension),
    Lane(LaneStmt),
    Legend(LegendStmt),
}

/// `A;`, `A, B [attrs];`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeStmt {
    pub id: String,
    pub attrs: Vec<Attr>,
}

/// `name = value` or, inside an option list, just `name`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub name: String,
    pub value: Option<String>,
}

/// `A -> B`, `A, B -> C`, `A -> B -> C [attrs]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeStmt {
    pub from_nodes: Vec<String>,
    pub edge_type: EdgeType,
    pub to_nodes: Vec<String>,
    pub attrs: Vec<Attr>,
}

/// Edge relation operators supported by the reference grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    /// `->`
    Arrow,
    /// `<-`
    ReverseArrow,
    /// `--`
    Line,
    /// `<->`
    DoubleArrow,
}

impl EdgeType {
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeType::Arrow => "->",
            EdgeType::ReverseArrow => "<-",
            EdgeType::Line => "--",
            EdgeType::DoubleArrow => "<->",
        }
    }
}

/// `class NAME [attrs];` or `plugin NAME [attrs];`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extension {
    pub kind: ExtensionKind,
    pub name: String,
    pub attrs: Vec<Attr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionKind {
    Class,
    Plugin,
}

/// `lane [id] { ... }` or `lane id [attrs];`
///
/// For the declaration form the attributes are stored as `Stmt::Attr` items in
/// `stmts`, matching the original parser's representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneStmt {
    pub id: Option<String>,
    pub stmts: Vec<Stmt>,
}

/// `legend [attrs] { ... }` — a swimflow extension (not in the reference grammar).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendStmt {
    pub attrs: Vec<Attr>,
    pub items: Vec<LegendItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegendItem {
    /// `class NAME [attrs];`
    Class { name: String, attrs: Vec<Attr> },
    /// `entry "LABEL" [attrs];`
    Entry { label: String, attrs: Vec<Attr> },
    /// `numbered;`
    Numbered,
}
