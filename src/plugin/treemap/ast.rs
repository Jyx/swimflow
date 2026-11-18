//! The syntax tree of the treemap diagram DSL.

/// An attribute of a node: `key = value` or a bare `key`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub key: String,
    pub value: Option<String>,
}

/// A node of the treemap: a rectangle with a name, an optional value and
/// optional children.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub name: String,
    /// The numbered parameter after the name (`Rent: 1200;`).
    pub value: Option<f64>,
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

/// The `root` statement of a treemap: the label and look of the root node.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RootDecl {
    pub name: Option<String>,
    pub attrs: Vec<Attr>,
}

/// A reusable `class NAME [attrs];` definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassDef {
    pub name: String,
    pub attrs: Vec<Attr>,
    /// The line the class starts on (for diagnostics).
    pub line: usize,
}

/// One `treemap { ... }` block.
#[derive(Debug, Clone, PartialEq)]
pub struct Treemap {
    /// The block name, if it has one.
    pub id: Option<String>,
    /// The `root` statement (defaults when absent).
    pub root: RootDecl,
    /// Diagram-level attributes (`width = 800;`).
    pub attrs: Vec<Attr>,
    /// Reusable classes, in source order.
    pub classes: Vec<ClassDef>,
    /// The children of the root.
    pub children: Vec<Node>,
}

/// A parsed file: one or more treemaps.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Document {
    pub treemaps: Vec<Treemap>,
}
