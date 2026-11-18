//! Recursive-descent parser for the activity DSL.
//!
//! Ported from the reference parser (which itself derives from blockdiag). The grammar is small and irregular (lane blocks, edge chains,
//! extension statements), so a hand-written parser is both clearer and gives
//! better error messages than a combinator library here.
//!
//! Notable semantics preserved from the reference:
//!   * `A, B [attrs]` creates two nodes sharing the attributes.
//!   * `A -> B -> C [attrs]` creates two edges, both sharing the attributes.
//!   * `lane A [attrs]` stores the attributes as `Stmt::Attr` inside the lane.
//!   * Attribute and extension statements are moved ahead of other statements
//!     (the `sort_tree` step) so classes/attributes are visible before use.
//!   * `legend { … }` is a swimflow extension; `legend` remains usable as an
//!     ordinary node name.

use std::path::Path;

use crate::ast::*;
use crate::error::{Error, Result};
use crate::lexer::{Token, TokenKind, tokenize};

/// Parse a document (one or more diagram blocks).
pub fn parse(input: &str) -> Result<Document> {
    let tokens = tokenize(input)?;
    let mut parser = Parser { tokens, pos: 0 };
    let doc = parser.document()?;
    Ok(doc)
}

/// Parse a UTF-8 file, stripping a leading BOM (matching `utf-8-sig`).
pub fn parse_file(path: impl AsRef<Path>) -> std::io::Result<Result<Document>> {
    let bytes = std::fs::read(path)?;
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    let text = String::from_utf8_lossy(bytes);
    Ok(parse(&text))
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    // -- primitives ---------------------------------------------------------

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        self.pos += 1;
        token
    }

    fn error<T>(&self, message: impl Into<String>) -> Result<T> {
        let (line, column) = self.peek().map(|t| (t.line, t.column)).unwrap_or((0, 0));
        Err(Error::parse(line, column, message))
    }

    fn at_op(&self, op: &str) -> bool {
        matches!(self.peek(), Some(t) if t.kind == TokenKind::Op && t.value == op)
    }

    fn eat_op(&mut self, op: &str) -> bool {
        if self.at_op(op) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect_op(&mut self, op: &str) -> Result<()> {
        if self.eat_op(op) {
            Ok(())
        } else {
            self.error(format!("expected {op:?}"))
        }
    }

    fn at_kw(&self, kw: &str) -> bool {
        matches!(self.peek(), Some(t) if t.kind == TokenKind::Name && t.value == kw)
    }

    fn eat_kw(&mut self, kw: &str) -> bool {
        if self.at_kw(kw) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn is_id(&self) -> bool {
        matches!(
            self.peek(),
            Some(t) if matches!(t.kind, TokenKind::Name | TokenKind::Number | TokenKind::String)
        )
    }

    /// `_id` — a Name, Number or String. Returns the raw token value (strings
    /// keep their delimiters, as upstream does).
    fn id(&mut self) -> Result<String> {
        if self.is_id() {
            Ok(self.advance().value)
        } else {
            self.error("expected identifier")
        }
    }

    fn peek_relation(&self) -> Option<EdgeType> {
        match self.peek() {
            Some(t) if t.kind == TokenKind::Op => match t.value.as_str() {
                "->" => Some(EdgeType::Arrow),
                "<-" => Some(EdgeType::ReverseArrow),
                "--" => Some(EdgeType::Line),
                "<->" => Some(EdgeType::DoubleArrow),
                _ => None,
            },
            _ => None,
        }
    }

    fn edge_relation(&mut self) -> Result<EdgeType> {
        match self.peek_relation() {
            Some(rel) => {
                self.pos += 1;
                Ok(rel)
            }
            None => self.error("expected edge relation"),
        }
    }

    // -- grammar ------------------------------------------------------------

    fn document(&mut self) -> Result<Document> {
        if self.peek().is_none() {
            return self.error("expected diagram");
        }
        let mut diagrams = Vec::new();
        while self.peek().is_some() {
            diagrams.push(self.diagram()?);
        }
        Ok(Document { diagrams })
    }

    fn diagram(&mut self) -> Result<Diagram> {
        let id = self.diagram_id()?;
        self.expect_op("{")?;

        let mut stmts = Vec::new();
        while !self.at_op("}") {
            if self.peek().is_none() {
                return self.error("expected '}'");
            }
            stmts.extend(self.statement()?);
            self.eat_op(";");
        }
        self.expect_op("}")?;

        sort_stmts(&mut stmts);
        Ok(Diagram { id, stmts })
    }

    fn diagram_id(&mut self) -> Result<Option<String>> {
        if self.at_kw("diagram") || self.at_kw("actdiag") {
            self.pos += 1;
            if self.is_id() {
                Ok(Some(self.id()?))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn statement(&mut self) -> Result<Vec<Stmt>> {
        // Ordered choice with backtracking (as upstream): `class`/`plugin` are
        // only extensions when followed by a name; otherwise they are ordinary
        // node/attribute names.
        if self.at_kw("class") || self.at_kw("plugin") {
            let save = self.pos;
            match self.extension_stmt() {
                Ok(stmt) => return Ok(vec![stmt]),
                Err(_) => self.pos = save,
            }
        }

        if self.at_kw("lane") {
            let save = self.pos;
            match self.lane_stmt() {
                Ok(stmt) => return Ok(vec![stmt]),
                Err(_) => self.pos = save,
            }
        }

        if self.at_kw("legend") {
            let save = self.pos;
            match self.legend_stmt() {
                Ok(stmt) => return Ok(vec![stmt]),
                Err(_) => self.pos = save,
            }
        }

        self.node_or_edge_stmt()
    }

    fn node_or_edge_stmt(&mut self) -> Result<Vec<Stmt>> {
        let node_list = self.node_list()?;

        if self.peek_relation().is_some() {
            return self.edge_tail(node_list);
        }

        if node_list.len() == 1 && self.at_op("=") {
            self.pos += 1;
            let value = self.id()?;
            let name = node_list.into_iter().next().unwrap();
            return Ok(vec![Stmt::Attr(Attr {
                name,
                value: Some(value),
            })]);
        }

        let attrs = self.option_list()?;
        Ok(node_list
            .into_iter()
            .map(|id| {
                Stmt::Node(NodeStmt {
                    id,
                    attrs: attrs.clone(),
                })
            })
            .collect())
    }

    fn edge_tail(&mut self, from_nodes: Vec<String>) -> Result<Vec<Stmt>> {
        let rel = self.edge_relation()?;
        let to_nodes = self.node_list()?;

        let mut edges = vec![EdgeStmt {
            from_nodes: from_nodes.clone(),
            edge_type: rel,
            to_nodes: to_nodes.clone(),
            attrs: Vec::new(),
        }];

        let mut from = to_nodes;
        while let Some(rel) = self.peek_relation() {
            self.pos += 1;
            let to = self.node_list()?;
            edges.push(EdgeStmt {
                from_nodes: from.clone(),
                edge_type: rel,
                to_nodes: to.clone(),
                attrs: Vec::new(),
            });
            from = to;
        }

        let attrs = self.option_list()?;
        for edge in &mut edges {
            edge.attrs = attrs.clone();
        }
        Ok(edges.into_iter().map(Stmt::Edge).collect())
    }

    fn node_list(&mut self) -> Result<Vec<String>> {
        let mut list = vec![self.id()?];
        while self.eat_op(",") {
            list.push(self.id()?);
        }
        Ok(list)
    }

    fn option_stmt(&mut self) -> Result<Attr> {
        let name = self.id()?;
        let value = if self.eat_op("=") {
            Some(self.id()?)
        } else {
            None
        };
        Ok(Attr { name, value })
    }

    fn option_list(&mut self) -> Result<Vec<Attr>> {
        let mut attrs = Vec::new();
        if self.eat_op("[") {
            attrs.push(self.option_stmt()?);
            while self.eat_op(",") {
                attrs.push(self.option_stmt()?);
            }
            self.expect_op("]")?;
        }
        Ok(attrs)
    }

    fn extension_stmt(&mut self) -> Result<Stmt> {
        let kind = if self.eat_kw("class") {
            ExtensionKind::Class
        } else if self.eat_kw("plugin") {
            ExtensionKind::Plugin
        } else {
            return self.error("expected 'class' or 'plugin'");
        };
        let name = self.id()?;
        let attrs = self.option_list()?;
        Ok(Stmt::Extension(Extension { kind, name, attrs }))
    }

    fn lane_stmt(&mut self) -> Result<Stmt> {
        if !self.eat_kw("lane") {
            return self.error("expected 'lane'");
        }

        // Block form: `lane [id] [attrs] { ... }`. Inline attrs are a
        // swimflow extension (upstream rejects `lane foo [attrs] { ... }`);
        // they are stored as `Attr` statements on the lane, exactly like the
        // separate `lane foo [attrs];` declaration.
        let save = self.pos;
        let id = if self.is_id() { Some(self.id()?) } else { None };
        let attrs = self.option_list()?;
        if self.eat_op("{") {
            let mut stmts: Vec<Stmt> = attrs.into_iter().map(Stmt::Attr).collect();
            while !self.at_op("}") {
                if self.peek().is_none() {
                    return self.error("expected '}'");
                }
                stmts.extend(self.lane_inline_stmt()?);
                self.eat_op(";");
            }
            self.expect_op("}")?;
            sort_stmts(&mut stmts);
            return Ok(Stmt::Lane(LaneStmt { id, stmts }));
        }

        // Declaration form: `lane id [attrs];`
        self.pos = save;
        let id = self.id()?;
        let attrs = self.option_list()?;
        let stmts = attrs.into_iter().map(Stmt::Attr).collect();
        Ok(Stmt::Lane(LaneStmt {
            id: Some(id),
            stmts,
        }))
    }

    fn lane_inline_stmt(&mut self) -> Result<Vec<Stmt>> {
        let node_list = self.node_list()?;

        if self.peek_relation().is_some() {
            return self.edge_tail(node_list);
        }

        if node_list.len() == 1 && self.at_op("=") {
            self.pos += 1;
            let value = self.id()?;
            let name = node_list.into_iter().next().unwrap();
            return Ok(vec![Stmt::Attr(Attr {
                name,
                value: Some(value),
            })]);
        }

        let attrs = self.option_list()?;
        Ok(node_list
            .into_iter()
            .map(|id| {
                Stmt::Node(NodeStmt {
                    id,
                    attrs: attrs.clone(),
                })
            })
            .collect())
    }

    fn legend_stmt(&mut self) -> Result<Stmt> {
        if !self.eat_kw("legend") {
            return self.error("expected 'legend'");
        }
        let attrs = self.option_list()?;
        self.expect_op("{")?;

        let mut items = Vec::new();
        while !self.at_op("}") {
            if self.peek().is_none() {
                return self.error("expected '}'");
            }
            if self.eat_kw("class") {
                let name = self.id()?;
                let attrs = self.option_list()?;
                items.push(LegendItem::Class { name, attrs });
            } else if self.eat_kw("entry") {
                let label = self.id()?;
                let attrs = self.option_list()?;
                items.push(LegendItem::Entry { label, attrs });
            } else if self.eat_kw("numbered") {
                items.push(LegendItem::Numbered);
            } else {
                return self.error("expected 'class', 'entry' or 'numbered' in legend");
            }
            self.eat_op(";");
        }
        self.expect_op("}")?;

        Ok(Stmt::Legend(LegendStmt { attrs, items }))
    }
}

/// Stable reordering of `Attr`/`Extension`/`Legend` before other statements,
/// mirroring `blockdiag.parser.sort_tree`.
fn sort_stmts(stmts: &mut [Stmt]) {
    stmts.sort_by_key(stmt_weight);
    for stmt in stmts.iter_mut() {
        if let Stmt::Lane(lane) = stmt {
            sort_stmts(&mut lane.stmts);
        }
    }
}

fn stmt_weight(stmt: &Stmt) -> u8 {
    match stmt {
        Stmt::Attr(_) | Stmt::Extension(_) | Stmt::Legend(_) => 0,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(s: &str) -> Document {
        parse(s).unwrap_or_else(|e| panic!("parse failed for {s:?}: {e}"))
    }

    #[test]
    fn simple_lane_diagram() {
        let doc = parse_ok(include_str!("../tests/fixtures/simple.diag"));
        assert_eq!(doc.diagrams.len(), 1);
        let d = &doc.diagrams[0];
        assert_eq!(d.id, None);

        let lanes: Vec<_> = d
            .stmts
            .iter()
            .filter_map(|s| match s {
                Stmt::Lane(l) => Some(l),
                _ => None,
            })
            .collect();
        assert_eq!(lanes.len(), 2);

        // lane { A; C; } and lane { B; }
        let node_ids = |lane: &LaneStmt| {
            lane.stmts
                .iter()
                .filter_map(|s| match s {
                    Stmt::Node(n) => Some(n.id.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(node_ids(lanes[0]), vec!["A", "C"]);
        assert_eq!(node_ids(lanes[1]), vec!["B"]);
    }

    #[test]
    fn edge_chain_shares_attrs() {
        let doc = parse_ok("{ A -> B -> C [label = x]; }");
        let edges: Vec<_> = doc.diagrams[0]
            .stmts
            .iter()
            .filter_map(|s| match s {
                Stmt::Edge(e) => Some(e),
                _ => None,
            })
            .collect();
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].from_nodes, vec!["A"]);
        assert_eq!(edges[0].to_nodes, vec!["B"]);
        assert_eq!(edges[1].from_nodes, vec!["B"]);
        assert_eq!(edges[1].to_nodes, vec!["C"]);
        for e in edges {
            assert_eq!(e.attrs.len(), 1);
            assert_eq!(e.attrs[0].name, "label");
            assert_eq!(e.attrs[0].value.as_deref(), Some("x"));
        }
    }

    #[test]
    fn node_list_shares_attrs() {
        let doc = parse_ok("{ A, B [color = red]; }");
        let nodes: Vec<_> = doc.diagrams[0]
            .stmts
            .iter()
            .filter_map(|s| match s {
                Stmt::Node(n) => Some(n),
                _ => None,
            })
            .collect();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].id, "A");
        assert_eq!(nodes[1].id, "B");
        assert_eq!(nodes[0].attrs[0].value.as_deref(), Some("red"));
    }

    #[test]
    fn diagram_attributes_sort_first() {
        let doc = parse_ok("{ A; orientation = landscape; }");
        // `orientation` (Attr) must be moved before the node.
        assert!(matches!(doc.diagrams[0].stmts[0], Stmt::Attr(_)));
        assert!(matches!(doc.diagrams[0].stmts[1], Stmt::Node(_)));
    }

    #[test]
    fn class_extension_and_lane_declaration() {
        let doc =
            parse_ok("{ class red [color = red]; lane foo [color = blue]; A [class = red]; }");
        let stmts = &doc.diagrams[0].stmts;
        assert!(matches!(stmts[0], Stmt::Extension(_)));
        let lane = stmts.iter().find_map(|s| match s {
            Stmt::Lane(l) => Some(l),
            _ => None,
        });
        let lane = lane.expect("lane");
        assert_eq!(lane.id.as_deref(), Some("foo"));
        assert!(matches!(lane.stmts[0], Stmt::Attr(_)));
    }

    #[test]
    fn lane_block_inline_attributes() {
        let doc = parse_ok("{ lane foo [color = \"#ff0000\"] { A; } }");
        let lane = doc.diagrams[0]
            .stmts
            .iter()
            .find_map(|s| match s {
                Stmt::Lane(l) => Some(l),
                _ => None,
            })
            .expect("lane");
        assert_eq!(lane.id.as_deref(), Some("foo"));
        // The inline attr is sorted before the node statement.
        assert!(matches!(lane.stmts[0], Stmt::Attr(_)));
        assert!(matches!(lane.stmts[1], Stmt::Node(_)));
    }

    #[test]
    fn named_and_multiple_diagrams() {
        let doc = parse_ok("diagram a { X; } diagram b { Y; }");
        assert_eq!(doc.diagrams.len(), 2);
        assert_eq!(doc.diagrams[0].id.as_deref(), Some("a"));
        assert_eq!(doc.diagrams[1].id.as_deref(), Some("b"));
    }

    #[test]
    fn legend_extension() {
        let doc = parse_ok(
            "{ legend [title = \"Legend\"] { class red; entry \"Auto\" [color = green]; numbered; } A; }",
        );
        let legend = doc.diagrams[0]
            .stmts
            .iter()
            .find_map(|s| match s {
                Stmt::Legend(l) => Some(l),
                _ => None,
            })
            .expect("legend");
        assert_eq!(legend.items.len(), 3);
        assert!(matches!(legend.items[0], LegendItem::Class { .. }));
        assert!(matches!(legend.items[1], LegendItem::Entry { .. }));
        assert!(matches!(legend.items[2], LegendItem::Numbered));
    }

    #[test]
    fn legend_can_still_be_a_node_name() {
        let doc = parse_ok("{ legend; }");
        assert!(matches!(doc.diagrams[0].stmts[0], Stmt::Node(_)));
    }

    #[test]
    fn lane_can_still_be_a_node_name() {
        let doc = parse_ok("{ lane; }");
        assert!(matches!(doc.diagrams[0].stmts[0], Stmt::Node(_)));
    }

    #[test]
    fn keyword_like_names_fall_back_to_nodes() {
        // Upstream ordered choice: these are ordinary nodes/attributes.
        assert!(matches!(
            parse_ok("{ class; }").diagrams[0].stmts[0],
            Stmt::Node(_)
        ));
        assert!(matches!(
            parse_ok("{ plugin; }").diagrams[0].stmts[0],
            Stmt::Node(_)
        ));
        assert!(matches!(
            parse_ok("{ class = red; }").diagrams[0].stmts[0],
            Stmt::Attr(_)
        ));
        assert!(matches!(
            parse_ok("{ class -> B; }").diagrams[0].stmts[0],
            Stmt::Edge(_)
        ));
    }

    #[test]
    fn edge_without_spaces_is_rejected() {
        assert!(parse("{ A->B }").is_err());
    }

    #[test]
    fn unterminated_block_errors() {
        let err = parse("{ A; ").unwrap_err();
        assert!(matches!(err, Error::Parse { .. }));
    }
}
