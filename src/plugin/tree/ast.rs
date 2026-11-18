//! The syntax tree of the tree diagram DSL.

/// An attribute of a node: `key = value` or a bare `key`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub key: String,
    pub value: Option<String>,
}

/// A node of the tree: a leaf or a branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<Attr>,
    pub children: Vec<Node>,
    /// The line the node starts on (for diagnostics).
    pub line: usize,
}

impl Node {
    /// The value of the attribute `key`, if it has one.
    pub fn attr(&self, key: &str) -> Option<&str> {
        attr_value(&self.attrs, key)
    }
}

/// The value of the attribute `key` in `attrs`, if it has one.
pub fn attr_value<'a>(attrs: &'a [Attr], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .rev()
        .find(|attr| attr.key == key)
        .and_then(|attr| attr.value.as_deref())
}

/// The `root` statement of a tree: the label and look of the root node.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RootDecl {
    pub name: Option<String>,
    pub attrs: Vec<Attr>,
}

/// One `tree { ... }` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    /// The block name, if it has one.
    pub id: Option<String>,
    /// The `root` statement (defaults when absent).
    pub root: RootDecl,
    /// Diagram-level attributes (`orientation = landscape;`).
    pub attrs: Vec<Attr>,
    /// The children of the root.
    pub children: Vec<Node>,
}

/// A parsed file: one or more trees.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document {
    pub trees: Vec<Tree>,
}
