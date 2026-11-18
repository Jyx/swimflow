//! The manifest diagram parser: source text -> [`ast::Document`].
//!
//! The tokens are the treemap's (same words, strings and comments), so the
//! scanner is shared; the grammar is its own.
//!
//! ```text
//! document := manifest*
//! manifest   := 'manifest' [name] '{' statement* '}'
//! statement:= 'class' name [attrs] ';'
//!           | word '=' value ';'
//!           | 'group' name [attrs] '{' column* '}'
//!           | column
//!           | 'rowgroup' name [attrs] '{' row* '}'
//!           | row
//! column   := 'column' name [attrs] ( ';' | '{' entry* '}' )
//! entry    := [ 'remove' | 'override' ] name ( ',' name )* [attrs] ';'
//! row      := 'row' name [attrs] [ ':' name ( ',' name )* ] ';'
//! attrs    := '[' [ attr ( ',' attr )* ] ']'
//! ```
//!
//! Inside a `rowgroup` the `row` keyword is optional.

use super::ast::{
    Attr, ClassDef, Column, Document, Entry, EntryKind, Group, Item, Manifest, Row, RowGroup,
};
use crate::plugin::treemap::parser::{ParseError, Spanned, Token, error, scan};

struct Parser {
    tokens: Vec<Spanned>,
    index: usize,
}

impl Parser {
    fn current(&self) -> &Spanned {
        &self.tokens[self.index]
    }

    fn token(&self) -> &Token {
        &self.current().token
    }

    fn ahead(&self) -> &Token {
        let next = (self.index + 1).min(self.tokens.len() - 1);
        &self.tokens[next].token
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.index].token.clone();
        if self.index + 1 < self.tokens.len() {
            self.index += 1;
        }
        token
    }

    fn fail<T>(&self, message: impl Into<String>) -> Result<T, ParseError> {
        let at = self.current();
        error(message, at.line, at.column)
    }

    fn expect(&mut self, token: &Token) -> Result<(), ParseError> {
        if self.token() == token {
            self.advance();
            Ok(())
        } else {
            self.fail(format!(
                "expected {}, found {}",
                token.describe(),
                self.token().describe()
            ))
        }
    }

    fn is_word(&self, keyword: &str) -> bool {
        matches!(self.token(), Token::Word(word) if word.eq_ignore_ascii_case(keyword))
    }

    /// Whether the current word is `keyword` followed by a name, i.e. used as
    /// a keyword and not as a name of its own.
    fn is_keyword(&self, keyword: &str) -> bool {
        self.is_word(keyword) && matches!(self.ahead(), Token::Word(_) | Token::Str(_))
    }

    fn name(&mut self, what: &str) -> Result<String, ParseError> {
        match self.token().clone() {
            Token::Word(text) | Token::Str(text) => {
                self.advance();
                Ok(text)
            }
            other => self.fail(format!("expected {what}, found {}", other.describe())),
        }
    }

    /// `name ( ',' name )*`.
    fn names(&mut self, what: &str) -> Result<Vec<String>, ParseError> {
        let mut names = vec![self.name(what)?];
        while *self.token() == Token::Comma {
            self.advance();
            names.push(self.name(what)?);
        }
        Ok(names)
    }

    fn document(&mut self) -> Result<Document, ParseError> {
        let mut document = Document::default();
        while *self.token() != Token::Eof {
            if !self.is_word("manifest") {
                return self.fail(format!(
                    "expected \"manifest\", found {}",
                    self.token().describe()
                ));
            }
            self.advance();
            document.manifests.push(self.manifest()?);
        }
        Ok(document)
    }

    fn manifest(&mut self) -> Result<Manifest, ParseError> {
        let id = if *self.token() == Token::LBrace {
            None
        } else {
            Some(self.name("a manifest name or '{'")?)
        };
        self.expect(&Token::LBrace)?;
        let mut manifest = Manifest {
            id,
            ..Manifest::default()
        };

        loop {
            match self.token() {
                Token::RBrace => {
                    self.advance();
                    return Ok(manifest);
                }
                Token::Eof => return self.fail("expected '}', found the end of the file"),
                _ => {}
            }

            if matches!(self.token(), Token::Word(_)) && *self.ahead() == Token::Eq {
                let key = self.name("an attribute name")?;
                self.advance();
                let value = self.name("a value")?;
                self.expect(&Token::Semi)?;
                manifest.attrs.push(Attr {
                    key,
                    value: Some(value),
                });
            } else if self.is_keyword("class") {
                let line = self.current().line;
                self.advance();
                let name = self.name("a class name")?;
                let attrs = self.attrs()?;
                self.expect(&Token::Semi)?;
                manifest.classes.push(ClassDef { name, attrs, line });
            } else if self.is_keyword("group") {
                let line = self.current().line;
                self.advance();
                let name = self.name("a group name")?;
                let attrs = self.attrs()?;
                self.expect(&Token::LBrace)?;
                let mut columns = Vec::new();
                while *self.token() != Token::RBrace {
                    if !self.is_keyword("column") {
                        return self.fail(format!(
                            "expected \"column\" or '}}', found {}",
                            self.token().describe()
                        ));
                    }
                    columns.push(self.column()?);
                }
                self.advance();
                manifest.items.push(Item::Group(Group {
                    name,
                    attrs,
                    columns,
                    line,
                }));
            } else if self.is_keyword("column") {
                let column = self.column()?;
                manifest.items.push(Item::Column(column));
            } else if self.is_keyword("rowgroup") {
                let line = self.current().line;
                self.advance();
                let name = self.name("a row group name")?;
                let attrs = self.attrs()?;
                self.expect(&Token::LBrace)?;
                let mut rows = Vec::new();
                loop {
                    match self.token() {
                        Token::RBrace => {
                            self.advance();
                            break;
                        }
                        Token::Eof => return self.fail("expected '}', found the end of the file"),
                        _ => {}
                    }
                    if self.is_keyword("row") {
                        self.advance();
                    }
                    rows.push(self.row()?);
                }
                manifest.items.push(Item::RowGroup(RowGroup {
                    name,
                    attrs,
                    rows,
                    line,
                }));
            } else if self.is_keyword("row") {
                self.advance();
                let row = self.row()?;
                manifest.items.push(Item::Row(row));
            } else {
                return self.fail(format!(
                    "expected \"column\", \"group\", \"row\", \"rowgroup\", \"class\" or an \
                     attribute, found {}",
                    self.token().describe()
                ));
            }
        }
    }

    /// `column name [attrs] ( ';' | '{' entry* '}' )`, at the keyword.
    fn column(&mut self) -> Result<Column, ParseError> {
        let line = self.current().line;
        self.advance();
        let name = self.name("a column name")?;
        let attrs = self.attrs()?;
        let mut entries = Vec::new();
        match self.token() {
            Token::Semi => {
                self.advance();
            }
            Token::LBrace => {
                self.advance();
                loop {
                    match self.token() {
                        Token::RBrace => {
                            self.advance();
                            break;
                        }
                        Token::Eof => return self.fail("expected '}', found the end of the file"),
                        _ => entries.push(self.entry()?),
                    }
                }
            }
            other => {
                let found = other.describe();
                return self.fail(format!("expected ';' or '{{', found {found}"));
            }
        }
        Ok(Column {
            name,
            attrs,
            entries,
            line,
        })
    }

    fn entry(&mut self) -> Result<Entry, ParseError> {
        let line = self.current().line;
        let kind = if self.is_keyword("remove") {
            self.advance();
            EntryKind::Remove
        } else if self.is_keyword("override") {
            self.advance();
            EntryKind::Override
        } else {
            EntryKind::Member
        };
        let rows = self.names("a row name")?;
        let attrs = self.attrs()?;
        self.expect(&Token::Semi)?;
        Ok(Entry {
            kind,
            rows,
            attrs,
            line,
        })
    }

    /// `name [attrs] [ ':' names ] ';'`, after the optional keyword.
    fn row(&mut self) -> Result<Row, ParseError> {
        let line = self.current().line;
        let name = self.name("a row name")?;
        let attrs = self.attrs()?;
        let columns = if *self.token() == Token::Colon {
            self.advance();
            self.names("a column name")?
        } else {
            Vec::new()
        };
        self.expect(&Token::Semi)?;
        Ok(Row {
            name,
            attrs,
            columns,
            line,
        })
    }

    fn attrs(&mut self) -> Result<Vec<Attr>, ParseError> {
        let mut attrs = Vec::new();
        if *self.token() != Token::LBracket {
            return Ok(attrs);
        }
        self.advance();
        loop {
            if *self.token() == Token::RBracket {
                self.advance();
                return Ok(attrs);
            }
            let key = match self.token().clone() {
                Token::Word(word) => {
                    self.advance();
                    word
                }
                other => {
                    return self.fail(format!(
                        "expected an attribute name, found {}",
                        other.describe()
                    ));
                }
            };
            let value = if *self.token() == Token::Eq {
                self.advance();
                Some(self.name("an attribute value")?)
            } else {
                None
            };
            attrs.push(Attr { key, value });
            match self.token() {
                Token::Comma => {
                    self.advance();
                }
                Token::RBracket => {}
                other => {
                    let found = other.describe();
                    return self.fail(format!("expected ',' or ']', found {found}"));
                }
            }
        }
    }
}

/// Parse a manifest diagram source.
pub fn parse(source: &str) -> Result<Document, ParseError> {
    let tokens = scan(source)?;
    Parser { tokens, index: 0 }.document()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_columns_entries_and_rows() {
        let document = parse(
            "manifest \"Uses\" {\n  title = \"Gits\";\n  group repo [color = yellow] {\n    \
             column base { a; b, c [note = x]; }\n    \
             column child [extends = base] { d; remove b; override c [note = \"new url\"]; }\n  }\n  \
             column alone;\n  row e [href = y]: base, alone;\n  rowgroup tools { row f: alone; g; }\n}\n",
        )
        .unwrap();
        let manifest = &document.manifests[0];
        assert_eq!(manifest.id.as_deref(), Some("Uses"));
        assert_eq!(manifest.attrs[0].key, "title");
        let Item::Group(group) = &manifest.items[0] else {
            panic!("a group")
        };
        assert_eq!(group.columns.len(), 2);
        assert_eq!(group.columns[0].entries[1].rows, vec!["b", "c"]);
        let child = &group.columns[1];
        assert_eq!(child.entries[1].kind, EntryKind::Remove);
        assert_eq!(child.entries[2].kind, EntryKind::Override);
        let Item::Row(row) = &manifest.items[2] else {
            panic!("a row")
        };
        assert_eq!(row.columns, vec!["base", "alone"]);
        let Item::RowGroup(rows) = &manifest.items[3] else {
            panic!("a row group")
        };
        assert_eq!(rows.rows.len(), 2);
        assert_eq!(rows.rows[1].name, "g");
    }

    #[test]
    fn keywords_can_be_row_names_inside_a_column() {
        let document =
            parse("manifest { column c { remove; override; row; remove remove; } }").unwrap();
        let Item::Column(column) = &document.manifests[0].items[0] else {
            panic!("a column")
        };
        let kinds: Vec<_> = column.entries.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                EntryKind::Member,
                EntryKind::Member,
                EntryKind::Member,
                EntryKind::Remove
            ]
        );
        assert_eq!(column.entries[3].rows, vec!["remove"]);
    }

    #[test]
    fn errors_have_positions() {
        let err = parse("manifest {\n  column c { a }\n}").unwrap_err();
        assert_eq!(err.line, 2);
        assert!(err.message.contains("expected ';'"), "{err}");

        let err = parse("manifest { group g { row a; } }").unwrap_err();
        assert!(err.message.contains("\"column\""), "{err}");

        let err = parse("manifest { stray; }").unwrap_err();
        assert!(err.message.contains("\"rowgroup\""), "{err}");

        let err = parse("graph { }").unwrap_err();
        assert!(err.message.contains("\"manifest\""), "{err}");
    }
}
