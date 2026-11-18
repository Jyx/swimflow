//! The syntax tree of the manifest diagram DSL.

/// An attribute: `key = value` or a bare `key`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub key: String,
    pub value: Option<String>,
}

/// The value of the attribute `key` in `attrs`, if it has one. The last
/// occurrence wins.
pub fn attr_value<'a>(attrs: &'a [Attr], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .rev()
        .find(|attr| attr.key == key)
        .and_then(|attr| attr.value.as_deref())
}

/// What an entry inside a column block does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// `name;` — the row is a member of the column.
    Member,
    /// `remove name;` — drop a row the column inherits.
    Remove,
    /// `override name;` — keep an inherited row, but changed.
    Override,
}

/// One entry of a column block: one or more rows sharing the attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub kind: EntryKind,
    pub rows: Vec<String>,
    /// Attributes of the cells (`note`).
    pub attrs: Vec<Attr>,
    pub line: usize,
}

/// `column NAME [attrs] { entries }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub attrs: Vec<Attr>,
    pub entries: Vec<Entry>,
    pub line: usize,
}

/// `group NAME [attrs] { columns }`: a band over several columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    pub attrs: Vec<Attr>,
    pub columns: Vec<Column>,
    pub line: usize,
}

/// `row NAME [attrs] [: column, column] ;`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub attrs: Vec<Attr>,
    /// The columns the row is a member of.
    pub columns: Vec<String>,
    pub line: usize,
}

/// `rowgroup NAME [attrs] { rows }`: a titled block of rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowGroup {
    pub name: String,
    pub attrs: Vec<Attr>,
    pub rows: Vec<Row>,
    pub line: usize,
}

/// A reusable `class NAME [attrs];` definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassDef {
    pub name: String,
    pub attrs: Vec<Attr>,
    pub line: usize,
}

/// A statement of a manifest block, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Group(Group),
    Column(Column),
    RowGroup(RowGroup),
    Row(Row),
}

/// One `manifest { ... }` block.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Manifest {
    pub id: Option<String>,
    /// Diagram-level attributes (`title = "…";`).
    pub attrs: Vec<Attr>,
    pub classes: Vec<ClassDef>,
    pub items: Vec<Item>,
}

/// A parsed file: one or more manifest diagrams.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document {
    pub manifests: Vec<Manifest>,
}
