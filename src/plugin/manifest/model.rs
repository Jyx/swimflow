//! The resolved manifest model: rows × columns with one mark per cell.
//!
//! A column is a set and a row is an element; the model decides, for every
//! pair, whether the row belongs to the column and *why*:
//!
//! * a row listed in the column (or naming the column after `:`) is a
//!   [`Mark::Direct`] member;
//! * a column with `extends = PARENT` starts from the members of its parent,
//!   which become [`Mark::Inherited`] (inheritance is transitive);
//! * `remove ROW;` drops an inherited member ([`Mark::Removed`] keeps the
//!   trace, so the cell can show it);
//! * `override ROW;` keeps an inherited member but marks it changed
//!   ([`Mark::Override`]).
//!
//! The model knows nothing about what the rows and columns stand for.

use std::collections::{BTreeMap, BTreeSet};

use super::ast::{self, Attr, EntryKind, Item, attr_value};

/// Why a row is (or is no longer) in a column. `from` is the index of the
/// column where the row was added directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    None,
    Direct,
    Inherited { from: usize },
    Override { from: usize },
    Removed { from: usize },
}

impl Mark {
    /// Whether the row is a member of the column.
    pub fn present(self) -> bool {
        matches!(
            self,
            Mark::Direct | Mark::Inherited { .. } | Mark::Override { .. }
        )
    }

    /// A short name, used by the dumps and the SVG `data-mark` attribute.
    pub fn name(self) -> &'static str {
        match self {
            Mark::None => "none",
            Mark::Direct => "direct",
            Mark::Inherited { .. } => "inherited",
            Mark::Override { .. } => "override",
            Mark::Removed { .. } => "removed",
        }
    }
}

/// One cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub mark: Mark,
    /// The `note` attribute of the entry that set the mark.
    pub note: Option<String>,
}

/// A resolved column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub label: String,
    pub attrs: Vec<Attr>,
    pub group: Option<usize>,
    /// The column this one extends.
    pub parent: Option<usize>,
    /// Left out by the view (see [`Diagram::apply_view`]).
    pub hidden: bool,
}

impl Column {
    pub fn attr(&self, key: &str) -> Option<&str> {
        attr_value(&self.attrs, key)
    }
}

/// A column group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    pub label: String,
    pub attrs: Vec<Attr>,
}

impl Group {
    pub fn attr(&self, key: &str) -> Option<&str> {
        attr_value(&self.attrs, key)
    }
}

/// A resolved row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub label: String,
    pub attrs: Vec<Attr>,
    pub group: Option<usize>,
    /// The number of (visible) columns the row is a member of.
    pub count: usize,
    /// Left out by the view (see [`Diagram::apply_view`]).
    pub hidden: bool,
}

impl Row {
    pub fn attr(&self, key: &str) -> Option<&str> {
        attr_value(&self.attrs, key)
    }
}

/// One resolved manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagram {
    pub id: Option<String>,
    pub attrs: Vec<Attr>,
    pub groups: Vec<Group>,
    pub columns: Vec<Column>,
    /// Row groups (a title and attributes, like column groups).
    pub row_groups: Vec<Group>,
    /// The rows in source order (first mention).
    pub rows: Vec<Row>,
    /// `cells[row][column]`.
    pub cells: Vec<Vec<Cell>>,
    /// The kinds of mark ([`Mark::name`]) the view does not draw.
    pub hidden_marks: BTreeSet<String>,
}

impl Diagram {
    pub fn attr(&self, key: &str) -> Option<&str> {
        attr_value(&self.attrs, key)
    }

    /// Leave out what the user hid in an interactive view, so an export
    /// looks like the screen. The tokens are those of the bundle runtime:
    ///
    /// * `rN` / `cN` — hide row / column `N` (indices into `rows`/`columns`);
    /// * `m:KIND` — do not draw marks of a kind (`direct`, `inherited`, …);
    /// * `only:cN` — hide every row that is not a member of column `N`;
    /// * `only:rN` — hide every column that row `N` is not a member of.
    ///
    /// Unknown or out-of-range tokens are ignored. The counts are recomputed
    /// over the visible columns, and a row that was a member of some column
    /// but of no visible one is hidden too (it would be an empty line).
    pub fn apply_view<'a>(&mut self, tokens: impl IntoIterator<Item = &'a str>) {
        let index = |token: &str, prefix: &str, len: usize| {
            token
                .strip_prefix(prefix)
                .and_then(|n| n.parse::<usize>().ok())
                .filter(|&n| n < len)
        };
        for token in tokens {
            if let Some(kind) = token.strip_prefix("m:") {
                self.hidden_marks.insert(kind.to_string());
            } else if let Some(c) = index(token, "only:c", self.columns.len()) {
                for (r, row) in self.rows.iter_mut().enumerate() {
                    if !self.cells[r][c].mark.present() {
                        row.hidden = true;
                    }
                }
            } else if let Some(r) = index(token, "only:r", self.rows.len()) {
                for (c, column) in self.columns.iter_mut().enumerate() {
                    if !self.cells[r][c].mark.present() {
                        column.hidden = true;
                    }
                }
            } else if let Some(r) = index(token, "r", self.rows.len()) {
                self.rows[r].hidden = true;
            } else if let Some(c) = index(token, "c", self.columns.len()) {
                self.columns[c].hidden = true;
            }
        }
        for (r, row) in self.rows.iter_mut().enumerate() {
            let had = row.count > 0;
            row.count = (0..self.columns.len())
                .filter(|&c| !self.columns[c].hidden && self.cells[r][c].mark.present())
                .count();
            if had && row.count == 0 {
                row.hidden = true;
            }
        }
    }

    /// The tokens hidden when the user has not changed the view: the mark
    /// kinds of the `hidemarks` attribute (comma separated, default
    /// `removed`; `none` shows every mark).
    pub fn default_hidden(&self) -> Vec<String> {
        let kinds = self.attr("hidemarks").unwrap_or("removed");
        if kinds.eq_ignore_ascii_case("none") {
            return Vec::new();
        }
        kinds
            .split(',')
            .map(str::trim)
            .filter(|kind| !kind.is_empty())
            .map(|kind| format!("m:{kind}"))
            .collect()
    }

    /// The indices of the columns that are not hidden, in order.
    pub fn visible_columns(&self) -> Vec<usize> {
        (0..self.columns.len())
            .filter(|&c| !self.columns[c].hidden)
            .collect()
    }

    /// The row indices in display order: row groups in source order (rows
    /// without a group first), and inside each block the order of `sort`:
    /// `count` (default; most shared first, ties in source order), `name` or
    /// `source`.
    pub fn row_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.rows.len())
            .filter(|&r| !self.rows[r].hidden)
            .collect();
        let sort = self.attr("sort").unwrap_or("count");
        order.sort_by(|&a, &b| {
            let (ra, rb) = (&self.rows[a], &self.rows[b]);
            let group = |row: &Row| row.group.map_or(0, |g| g + 1);
            group(ra).cmp(&group(rb)).then_with(|| match sort {
                "name" => ra.label.to_lowercase().cmp(&rb.label.to_lowercase()),
                "source" => std::cmp::Ordering::Equal,
                _ => rb.count.cmp(&ra.count),
            })
        });
        order
    }
}

/// A resolved file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document {
    pub diagrams: Vec<Diagram>,
}

/// Build the resolved document. Unknown columns, unknown parents, an
/// `extends` cycle, and `remove`/`override` of a row the column does not
/// inherit are errors with a line number.
pub fn build(ast: &ast::Document) -> Result<Document, String> {
    let diagrams = ast
        .manifests
        .iter()
        .map(build_manifest)
        .collect::<Result<_, _>>()?;
    Ok(Document { diagrams })
}

/// A column statement with the group it sits in.
struct PendingColumn<'a> {
    column: &'a ast::Column,
    group: Option<usize>,
}

fn build_manifest(manifest: &ast::Manifest) -> Result<Diagram, String> {
    let classes: BTreeMap<&str, &[Attr]> = manifest
        .classes
        .iter()
        .map(|class| (class.name.as_str(), class.attrs.as_slice()))
        .collect();
    let resolve = |attrs: &[Attr], line: usize| resolve_attrs(attrs, &classes, line);

    // Columns and groups first, so a row may name a column defined later.
    let mut groups = Vec::new();
    let mut pending = Vec::new();
    let mut row_groups = Vec::new();
    for item in &manifest.items {
        match item {
            Item::Group(group) => {
                let index = groups.len();
                let attrs = resolve(&group.attrs, group.line)?;
                groups.push(Group {
                    label: attr_value(&attrs, "label")
                        .unwrap_or(&group.name)
                        .to_string(),
                    name: group.name.clone(),
                    attrs,
                });
                for column in &group.columns {
                    pending.push(PendingColumn {
                        column,
                        group: Some(index),
                    });
                }
            }
            Item::Column(column) => pending.push(PendingColumn {
                column,
                group: None,
            }),
            Item::RowGroup(group) => {
                let attrs = resolve(&group.attrs, group.line)?;
                row_groups.push(Group {
                    label: attr_value(&attrs, "label")
                        .unwrap_or(&group.name)
                        .to_string(),
                    name: group.name.clone(),
                    attrs,
                });
            }
            Item::Row(_) => {}
        }
    }

    let mut index: BTreeMap<&str, usize> = BTreeMap::new();
    let mut columns = Vec::new();
    for p in &pending {
        if index
            .insert(p.column.name.as_str(), columns.len())
            .is_some()
        {
            return Err(format!(
                "line {}: the column {:?} is defined twice",
                p.column.line, p.column.name
            ));
        }
        let attrs = resolve(&p.column.attrs, p.column.line)?;
        columns.push(Column {
            label: attr_value(&attrs, "label")
                .unwrap_or(&p.column.name)
                .to_string(),
            name: p.column.name.clone(),
            attrs,
            group: p.group,
            parent: None,
            hidden: false,
        });
    }
    for (i, p) in pending.iter().enumerate() {
        if let Some(parent) = columns[i].attr("extends") {
            let parent = *index.get(parent).ok_or_else(|| {
                format!(
                    "line {}: the column {:?} extends the unknown column {parent:?}",
                    p.column.line, p.column.name
                )
            })?;
            columns[i].parent = Some(parent);
        }
    }

    // Rows in order of first mention, and the edits of every column.
    let mut rows = RowTable::default();
    let mut edits: Edits = vec![Vec::new(); columns.len()];
    let mut row_group = 0;
    for item in &manifest.items {
        let entries = |column: &ast::Column, rows: &mut RowTable, edits: &mut Edits| {
            let c = index[column.name.as_str()];
            for entry in &column.entries {
                let note = attr_value(&entry.attrs, "note").map(str::to_string);
                for name in &entry.rows {
                    let r = rows.intern(name);
                    edits[c].push((r, entry.kind, note.clone(), entry.line));
                }
            }
        };
        let declare = |row: &ast::Row,
                       group: Option<usize>,
                       rows: &mut RowTable,
                       edits: &mut Edits|
         -> Result<(), String> {
            let r = rows.intern(&row.name);
            let attrs = resolve(&row.attrs, row.line)?;
            let target = &mut rows.rows[r];
            target.attrs.extend(attrs);
            if let Some(label) = target.attr("label") {
                target.label = label.to_string();
            }
            if group.is_some() {
                target.group = group;
            }
            for column in &row.columns {
                let c = index
                    .get(column.as_str())
                    .copied()
                    .ok_or_else(|| format!("line {}: unknown column {column:?}", row.line))?;
                edits[c].push((r, EntryKind::Member, None, row.line));
            }
            Ok(())
        };
        match item {
            Item::Group(group) => {
                for column in &group.columns {
                    entries(column, &mut rows, &mut edits);
                }
            }
            Item::Column(column) => entries(column, &mut rows, &mut edits),
            Item::Row(row) => declare(row, None, &mut rows, &mut edits)?,
            Item::RowGroup(group) => {
                for row in &group.rows {
                    declare(row, Some(row_group), &mut rows, &mut edits)?;
                }
                row_group += 1;
            }
        }
    }
    let mut rows = rows.rows;

    // Resolve the columns parents first.
    let order = topological(&columns)?;
    let empty = Cell {
        mark: Mark::None,
        note: None,
    };
    let mut cells = vec![vec![empty; columns.len()]; rows.len()];
    for &c in &order {
        if let Some(parent) = columns[c].parent {
            for row in cells.iter_mut() {
                row[c].mark = match row[parent].mark {
                    Mark::Direct => Mark::Inherited { from: parent },
                    Mark::Inherited { from } | Mark::Override { from } => Mark::Inherited { from },
                    _ => Mark::None,
                };
            }
        }
        for (r, kind, note, line) in &edits[c] {
            let cell = &mut cells[*r][c];
            let inherited = match cell.mark {
                Mark::Inherited { from } => Some(from),
                _ => None,
            };
            cell.mark = match (kind, inherited) {
                (EntryKind::Member, _) => Mark::Direct,
                (EntryKind::Remove, Some(from)) => Mark::Removed { from },
                (EntryKind::Override, Some(from)) => Mark::Override { from },
                (kind, None) => {
                    let verb = if *kind == EntryKind::Remove {
                        "remove"
                    } else {
                        "override"
                    };
                    return Err(format!(
                        "line {line}: the column {:?} cannot {verb} {:?}: it does not inherit it",
                        columns[c].name, rows[*r].name
                    ));
                }
            };
            if note.is_some() {
                cell.note = note.clone();
            }
        }
    }
    for (r, row) in rows.iter_mut().enumerate() {
        row.count = cells[r].iter().filter(|cell| cell.mark.present()).count();
    }

    Ok(Diagram {
        id: manifest.id.clone(),
        attrs: manifest.attrs.clone(),
        groups,
        columns,
        row_groups,
        rows,
        cells,
        hidden_marks: BTreeSet::new(),
    })
}

/// The edits of every column: (row, kind, note, line).
type Edits = Vec<Vec<(usize, EntryKind, Option<String>, usize)>>;

/// The rows in order of first mention.
#[derive(Default)]
struct RowTable {
    rows: Vec<Row>,
    index: BTreeMap<String, usize>,
}

impl RowTable {
    fn intern(&mut self, name: &str) -> usize {
        if let Some(&r) = self.index.get(name) {
            return r;
        }
        self.rows.push(Row {
            name: name.to_string(),
            label: name.to_string(),
            attrs: Vec::new(),
            group: None,
            count: 0,
            hidden: false,
        });
        self.index.insert(name.to_string(), self.rows.len() - 1);
        self.rows.len() - 1
    }
}

/// The columns ordered so that every parent comes before its children.
fn topological(columns: &[Column]) -> Result<Vec<usize>, String> {
    let mut state = vec![0u8; columns.len()]; // 0 new, 1 visiting, 2 done
    let mut order = Vec::new();
    for start in 0..columns.len() {
        let mut chain = Vec::new();
        let mut c = Some(start);
        while let Some(i) = c {
            match state[i] {
                2 => break,
                1 => {
                    return Err(format!(
                        "the column {:?} extends itself (through extends)",
                        columns[i].name
                    ));
                }
                _ => {}
            }
            state[i] = 1;
            chain.push(i);
            c = columns[i].parent;
        }
        for &i in chain.iter().rev() {
            state[i] = 2;
            order.push(i);
        }
    }
    Ok(order)
}

/// Expand `class = NAME` into the class attributes (the explicit ones win).
fn resolve_attrs(
    attrs: &[Attr],
    classes: &BTreeMap<&str, &[Attr]>,
    line: usize,
) -> Result<Vec<Attr>, String> {
    let mut out = Vec::new();
    for attr in attrs {
        if attr.key == "class" {
            let name = attr.value.as_deref().unwrap_or("");
            let class = classes
                .get(name)
                .ok_or_else(|| format!("line {line}: unknown class {name:?}"))?;
            out.splice(0..0, class.iter().cloned());
        } else {
            out.push(attr.clone());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::parser::parse;
    use super::*;

    fn build_one(source: &str) -> Result<Diagram, String> {
        build(&parse(source).unwrap()).map(|mut d| d.diagrams.remove(0))
    }

    fn mark(d: &Diagram, row: &str, column: &str) -> Mark {
        let r = d.rows.iter().position(|x| x.name == row).unwrap();
        let c = d.columns.iter().position(|x| x.name == column).unwrap();
        d.cells[r][c].mark
    }

    #[test]
    fn extends_inherits_removes_and_overrides() {
        let d = build_one(
            "manifest {\n column base { a; b; c; }\n \
             column mid [extends = base] { d; remove b; override c [note = n]; }\n \
             column leaf [extends = mid] { remove d; }\n row e: leaf; }",
        )
        .unwrap();
        assert_eq!(mark(&d, "a", "base"), Mark::Direct);
        assert_eq!(mark(&d, "a", "mid"), Mark::Inherited { from: 0 });
        assert_eq!(mark(&d, "b", "mid"), Mark::Removed { from: 0 });
        assert_eq!(mark(&d, "c", "mid"), Mark::Override { from: 0 });
        assert_eq!(mark(&d, "c", "leaf"), Mark::Inherited { from: 0 });
        assert_eq!(mark(&d, "b", "leaf"), Mark::None);
        assert_eq!(mark(&d, "d", "leaf"), Mark::Removed { from: 1 });
        assert_eq!(mark(&d, "e", "leaf"), Mark::Direct);
        let a = d.rows.iter().find(|r| r.name == "a").unwrap();
        assert_eq!(a.count, 3);
        let c = d.rows.iter().position(|r| r.name == "c").unwrap();
        assert_eq!(d.cells[c][1].note.as_deref(), Some("n"));
    }

    #[test]
    fn a_child_may_be_written_before_its_parent() {
        let d = build_one("manifest { column b [extends = a]; column a { x; } }").unwrap();
        assert_eq!(mark(&d, "x", "b"), Mark::Inherited { from: 1 });
    }

    #[test]
    fn rows_sort_by_count_by_default() {
        let d = build_one("manifest { column a { x; y; } column b { y; } row z; }").unwrap();
        let names: Vec<_> = d
            .row_order()
            .iter()
            .map(|&r| d.rows[r].name.clone())
            .collect();
        assert_eq!(names, vec!["y", "x", "z"]);
        let d = build_one("manifest { sort = name; column a { y; X; } }").unwrap();
        let names: Vec<_> = d
            .row_order()
            .iter()
            .map(|&r| d.rows[r].name.clone())
            .collect();
        assert_eq!(names, vec!["X", "y"]);
    }

    #[test]
    fn row_groups_keep_their_rows_together() {
        let d = build_one(
            "manifest { column a { x; y; w; } rowgroup tools { y; } row w [label = W]; }",
        )
        .unwrap();
        let names: Vec<_> = d
            .row_order()
            .iter()
            .map(|&r| d.rows[r].label.clone())
            .collect();
        assert_eq!(names, vec!["x", "W", "y"]);
    }

    #[test]
    fn classes_apply_to_columns() {
        let d = build_one("manifest { class hot [color = red]; column a [class = hot]; }").unwrap();
        assert_eq!(d.columns[0].attr("color"), Some("red"));
    }

    #[test]
    fn a_view_hides_rows_columns_and_marks() {
        let mut d = build_one("manifest { column a { x; y; } column b { x; z; } }").unwrap();
        d.apply_view(["c1", "r1", "m:direct", "bogus", "c9"]);
        assert!(d.columns[1].hidden && !d.columns[0].hidden);
        assert_eq!(d.visible_columns(), vec![0]);
        let shown: Vec<_> = d
            .row_order()
            .iter()
            .map(|&r| d.rows[r].name.clone())
            .collect();
        assert_eq!(shown, vec!["x"]);
        // Counts only cover the visible columns.
        assert_eq!(d.rows[0].count, 1);
        // z was only in the hidden column: it is left out, not shown empty.
        assert!(d.rows[2].hidden);
        assert!(d.hidden_marks.contains("direct"));

        let mut d = build_one("manifest { column a { x; } column b { y; } }").unwrap();
        d.apply_view(["only:c1"]);
        let shown: Vec<_> = d
            .row_order()
            .iter()
            .map(|&r| d.rows[r].name.clone())
            .collect();
        assert_eq!(shown, vec!["y"]);

        let mut d = build_one("manifest { column a { x; y; } column b { y; } }").unwrap();
        d.apply_view(["only:r0"]);
        assert_eq!(d.visible_columns(), vec![0]);
        // y keeps its column a, so it stays, with a count of one.
        assert!(!d.rows[1].hidden);
        assert_eq!(d.rows[1].count, 1);

        // A cell filter sends both: rows of column b, columns of row y.
        let mut d =
            build_one("manifest { column a { x; y; } column b { y; } column c { z; } }").unwrap();
        d.apply_view(["only:r1", "only:c1"]);
        assert_eq!(d.visible_columns(), vec![0, 1]);
        let shown: Vec<_> = d
            .row_order()
            .iter()
            .map(|&r| d.rows[r].name.clone())
            .collect();
        assert_eq!(shown, vec!["y"]);
    }

    #[test]
    fn a_row_without_columns_in_the_source_stays() {
        let mut d = build_one("manifest { column a { x; } row y; }").unwrap();
        d.apply_view(Vec::<&str>::new());
        assert!(!d.rows[1].hidden);
    }

    #[test]
    fn removed_marks_are_hidden_by_default() {
        let d = build_one("manifest { column a; }").unwrap();
        assert_eq!(d.default_hidden(), vec!["m:removed"]);
        let d = build_one("manifest { hidemarks = none; column a; }").unwrap();
        assert!(d.default_hidden().is_empty());
        let d = build_one("manifest { hidemarks = \"inherited, removed\"; column a; }").unwrap();
        assert_eq!(d.default_hidden(), vec!["m:inherited", "m:removed"]);
    }

    #[test]
    fn errors() {
        let err = build_one("manifest { row x: nope; }").unwrap_err();
        assert!(err.contains("unknown column"), "{err}");
        let err = build_one("manifest { column a [extends = b]; }").unwrap_err();
        assert!(err.contains("unknown column"), "{err}");
        let err =
            build_one("manifest { column a [extends = b]; column b [extends = a]; }").unwrap_err();
        assert!(err.contains("extends itself"), "{err}");
        let err = build_one("manifest { column a { remove x; } }").unwrap_err();
        assert!(err.contains("does not inherit"), "{err}");
        let err = build_one("manifest { column a; column a; }").unwrap_err();
        assert!(err.contains("twice"), "{err}");
        let err = build_one("manifest { column a [class = nope]; }").unwrap_err();
        assert!(err.contains("unknown class"), "{err}");
    }
}
