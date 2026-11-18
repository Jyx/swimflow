//! The treemap diagram parser: source text -> [`ast::Document`].
//!
//! The grammar is small, so the source is scanned into a token list first and
//! then read by a recursive-descent parser. Every problem is reported with a
//! line and a column.
//!
//! ```text
//! document := treemap*
//! treemap  := 'treemap' [name] '{' statement* '}'
//! statement:= 'root' [name] [attrs] ( ';' | '{' node* '}' )
//!           | word '=' value ';'
//!           | node
//! node     := name [ ':' number ] [attrs] ( ';' | '{' node* '}' )
//! attrs    := '[' [ attr ( ',' attr )* ] ']'
//! attr     := word [ '=' value ]
//! ```

use std::fmt;

use super::ast::{Attr, ClassDef, Document, Node, RootDecl, Treemap};

/// How deeply nodes may nest.
const MAX_DEPTH: usize = 200;

/// A parse error with a position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "line {}, column {}: {}",
            self.line, self.column, self.message
        )
    }
}

impl std::error::Error for ParseError {}

/// A token of the treemap DSL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token {
    /// A bare word.
    Word(String),
    /// A quoted string.
    Str(String),
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Semi,
    Comma,
    Eq,
    Colon,
    Eof,
}

impl Token {
    pub(crate) fn describe(&self) -> String {
        match self {
            Token::Word(word) => format!("{word:?}"),
            Token::Str(_) => "a string".to_string(),
            Token::LBrace => "'{'".to_string(),
            Token::RBrace => "'}'".to_string(),
            Token::LBracket => "'['".to_string(),
            Token::RBracket => "']'".to_string(),
            Token::Semi => "';'".to_string(),
            Token::Comma => "','".to_string(),
            Token::Eq => "'='".to_string(),
            Token::Colon => "':'".to_string(),
            Token::Eof => "the end of the file".to_string(),
        }
    }
}

/// A token and where it starts.
#[derive(Debug, Clone)]
pub(crate) struct Spanned {
    pub(crate) token: Token,
    pub(crate) line: usize,
    pub(crate) column: usize,
}

pub(crate) fn error<T>(
    message: impl Into<String>,
    line: usize,
    column: usize,
) -> Result<T, ParseError> {
    Err(ParseError {
        message: message.into(),
        line,
        column,
    })
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.') || byte >= 0x80
}

/// Scan the whole source into tokens.
///
/// The manifest diagram (`crate::plugin::manifest`) shares this scanner: both
/// languages have the same words, strings, punctuation and comments.
pub(crate) fn scan(source: &str) -> Result<Vec<Spanned>, ParseError> {
    let src = source.as_bytes();
    let mut tokens = Vec::new();
    let (mut pos, mut line, mut column) = (0usize, 1usize, 1usize);

    // Advance one byte, keeping the position.
    macro_rules! bump {
        () => {{
            let byte = src[pos];
            pos += 1;
            if byte == b'\n' {
                line += 1;
                column = 1;
            } else if byte & 0xC0 != 0x80 {
                column += 1;
            }
            byte
        }};
    }

    while pos < src.len() {
        let byte = src[pos];
        let (start_line, start_column) = (line, column);
        let mut push = |token: Token| {
            tokens.push(Spanned {
                token,
                line: start_line,
                column: start_column,
            })
        };

        match byte {
            b' ' | b'\t' | b'\r' | b'\n' => {
                bump!();
            }
            b'#' => {
                while pos < src.len() && src[pos] != b'\n' {
                    bump!();
                }
            }
            b'/' if src.get(pos + 1) == Some(&b'/') => {
                while pos < src.len() && src[pos] != b'\n' {
                    bump!();
                }
            }
            b'/' if src.get(pos + 1) == Some(&b'*') => {
                bump!();
                bump!();
                loop {
                    if pos >= src.len() {
                        return error("unterminated comment", start_line, start_column);
                    }
                    if src[pos] == b'*' && src.get(pos + 1) == Some(&b'/') {
                        bump!();
                        bump!();
                        break;
                    }
                    bump!();
                }
            }
            b'{' => {
                bump!();
                push(Token::LBrace);
            }
            b'}' => {
                bump!();
                push(Token::RBrace);
            }
            b'[' => {
                bump!();
                push(Token::LBracket);
            }
            b']' => {
                bump!();
                push(Token::RBracket);
            }
            b';' => {
                bump!();
                push(Token::Semi);
            }
            b',' => {
                bump!();
                push(Token::Comma);
            }
            b'=' => {
                bump!();
                push(Token::Eq);
            }
            b':' => {
                bump!();
                push(Token::Colon);
            }
            b'"' | b'\'' => {
                let quote = bump!();
                let mut bytes = Vec::new();
                loop {
                    if pos >= src.len() {
                        return error("unterminated string", start_line, start_column);
                    }
                    let byte = bump!();
                    if byte == quote {
                        break;
                    }
                    if byte == b'\\' {
                        if pos >= src.len() {
                            return error("unterminated escape sequence", line, column);
                        }
                        bytes.push(match bump!() {
                            b'n' => b'\n',
                            b't' => b'\t',
                            other => other,
                        });
                        continue;
                    }
                    bytes.push(byte);
                }
                push(Token::Str(String::from_utf8_lossy(&bytes).into_owned()));
            }
            _ if is_word_byte(byte) => {
                let start = pos;
                while pos < src.len() && is_word_byte(src[pos]) {
                    bump!();
                }
                push(Token::Word(
                    String::from_utf8_lossy(&src[start..pos]).into_owned(),
                ));
            }
            _ => {
                let ch = source[pos..].chars().next().unwrap_or('?');
                return error(
                    format!("unexpected character {ch:?}"),
                    start_line,
                    start_column,
                );
            }
        }
    }

    tokens.push(Spanned {
        token: Token::Eof,
        line,
        column,
    });
    Ok(tokens)
}

/// The parser.
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

    /// A name: a bare word or a quoted string.
    fn name(&mut self, what: &str) -> Result<String, ParseError> {
        match self.token().clone() {
            Token::Word(text) | Token::Str(text) => {
                self.advance();
                Ok(text)
            }
            other => self.fail(format!("expected {what}, found {}", other.describe())),
        }
    }

    /// The value after a colon. The tokenizer treats numbers as words, so the
    /// word is parsed as a number here. Negative, non-finite and malformed
    /// values are rejected with their position.
    fn number(&mut self) -> Result<f64, ParseError> {
        let at = self.current().clone();
        let text = match self.token().clone() {
            Token::Word(text) => {
                self.advance();
                text
            }
            other => {
                return self.fail(format!("expected a number, found {}", other.describe()));
            }
        };
        let value = text.parse::<f64>().map_err(|_| ParseError {
            message: format!("expected a number, found {text:?}"),
            line: at.line,
            column: at.column,
        })?;
        if !value.is_finite() {
            return error("a value must be finite", at.line, at.column);
        }
        if value < 0.0 {
            return error("a value must not be negative", at.line, at.column);
        }
        Ok(value)
    }

    fn document(&mut self) -> Result<Document, ParseError> {
        let mut document = Document::default();
        while *self.token() != Token::Eof {
            if !self.is_word("treemap") {
                return self.fail(format!(
                    "expected \"treemap\", found {}",
                    self.token().describe()
                ));
            }
            self.advance();
            document.treemaps.push(self.treemap()?);
        }
        Ok(document)
    }

    fn treemap(&mut self) -> Result<Treemap, ParseError> {
        let id = if *self.token() == Token::LBrace {
            None
        } else {
            Some(self.name("a treemap name or '{'")?)
        };
        self.expect(&Token::LBrace)?;

        let mut treemap = Treemap {
            id,
            root: RootDecl::default(),
            attrs: Vec::new(),
            classes: Vec::new(),
            children: Vec::new(),
        };
        let mut seen_root = false;

        loop {
            match self.token() {
                Token::RBrace => {
                    self.advance();
                    return Ok(treemap);
                }
                Token::Eof => return self.fail("expected '}', found the end of the file"),
                _ => {}
            }

            // `class NAME [attrs] ;` is a reusable class definition. A lone
            // `class;` or `class [..];` is an ordinary node called "class".
            if self.is_word("class") && matches!(self.ahead(), Token::Word(_) | Token::Str(_)) {
                let line = self.current().line;
                self.advance();
                let name = self.name("a class name")?;
                let attrs = self.attrs()?;
                self.expect(&Token::Semi)?;
                treemap.classes.push(ClassDef { name, attrs, line });
                continue;
            }

            // `root [name] [attrs] ;` — but a lone `root;`, `root [..]` or
            // `root: 42;` is an ordinary node that happens to be called "root".
            if self.is_word("root")
                && matches!(self.ahead(), Token::Word(_) | Token::Str(_) | Token::LBrace)
            {
                if seen_root {
                    return self.fail("a treemap has only one root statement");
                }
                seen_root = true;
                self.advance();
                if matches!(self.token(), Token::Word(_) | Token::Str(_)) {
                    treemap.root.name = Some(self.name("a root name")?);
                }
                treemap.root.attrs = self.attrs()?;
                match self.token() {
                    Token::Semi => {
                        self.advance();
                    }
                    Token::LBrace => {
                        self.advance();
                        let children = self.children(1)?;
                        treemap.children.extend(children);
                    }
                    other => {
                        let found = other.describe();
                        return self.fail(format!("expected ';' or '{{', found {found}"));
                    }
                }
                continue;
            }

            // `word = value ;` is an attribute of the diagram.
            if matches!(self.token(), Token::Word(_)) && *self.ahead() == Token::Eq {
                let key = self.name("an attribute name")?;
                self.advance();
                let value = self.name("a value")?;
                self.expect(&Token::Semi)?;
                treemap.attrs.push(Attr {
                    key,
                    value: Some(value),
                });
                continue;
            }

            let node = self.node(1)?;
            treemap.children.push(node);
        }
    }

    /// The nodes up to the closing brace (which is consumed).
    fn children(&mut self, depth: usize) -> Result<Vec<Node>, ParseError> {
        let mut nodes = Vec::new();
        loop {
            match self.token() {
                Token::RBrace => {
                    self.advance();
                    return Ok(nodes);
                }
                Token::Eof => return self.fail("expected '}', found the end of the file"),
                _ => nodes.push(self.node(depth)?),
            }
        }
    }

    fn node(&mut self, depth: usize) -> Result<Node, ParseError> {
        if depth > MAX_DEPTH {
            return self.fail(format!(
                "nodes are nested more than {MAX_DEPTH} levels deep"
            ));
        }
        let line = self.current().line;
        let name = self.name("a node name")?;
        let value = if *self.token() == Token::Colon {
            self.advance();
            Some(self.number()?)
        } else {
            None
        };
        let attrs = self.attrs()?;
        let children = match self.token() {
            Token::Semi => {
                self.advance();
                Vec::new()
            }
            Token::LBrace => {
                self.advance();
                self.children(depth + 1)?
            }
            other => {
                let found = other.describe();
                return self.fail(format!("expected ';' or '{{', found {found}"));
            }
        };
        Ok(Node {
            name,
            value,
            attrs,
            children,
            line,
        })
    }

    /// An optional `[key = value, flag, ...]` list.
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

/// Parse a treemap diagram source.
pub fn parse(source: &str) -> Result<Document, ParseError> {
    let tokens = scan(source)?;
    Parser { tokens, index: 0 }.document()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_treemap_with_values_and_nested_children() {
        let document = parse(
            "treemap \"Budget\" {\n  root \"Monthly budget\";\n  Housing: 1500 {\n    Rent: 1000;\n    Utilities: 500;\n  }\n  Food: 600;\n  Savings;\n}\n",
        )
        .unwrap();
        assert_eq!(document.treemaps.len(), 1);
        let treemap = &document.treemaps[0];
        assert_eq!(treemap.id.as_deref(), Some("Budget"));
        assert_eq!(treemap.root.name.as_deref(), Some("Monthly budget"));
        assert_eq!(treemap.children.len(), 3);
        let housing = &treemap.children[0];
        assert_eq!(housing.name, "Housing");
        assert_eq!(housing.value, Some(1500.0));
        assert_eq!(housing.children[0].value, Some(1000.0));
        assert_eq!(treemap.children[2].value, None);
    }

    #[test]
    fn numeric_names_and_values_are_unambiguous() {
        let document = parse("treemap { 2024; 2024: 12; \"Q4: Europe\": 3.5; }").unwrap();
        let children = &document.treemaps[0].children;
        assert_eq!(
            (children[0].name.as_str(), children[0].value),
            ("2024", None)
        );
        assert_eq!(
            (children[1].name.as_str(), children[1].value),
            ("2024", Some(12.0))
        );
        assert_eq!(
            (children[2].name.as_str(), children[2].value),
            ("Q4: Europe", Some(3.5))
        );
    }

    #[test]
    fn class_definitions_and_class_attributes() {
        let document = parse(
            "treemap { class warm [color = \"#fff3bf\"]; class cool [color = blue]; A: 1 [class = warm]; }",
        )
        .unwrap();
        let treemap = &document.treemaps[0];
        assert_eq!(treemap.classes.len(), 2);
        assert_eq!(treemap.classes[0].name, "warm");
        assert_eq!(treemap.classes[0].attrs[0].key, "color");
        assert_eq!(treemap.children[0].attr("class"), Some("warm"));
    }

    #[test]
    fn class_can_be_a_node_name() {
        let document = parse("treemap { class; class [color = red]; class: 2; }").unwrap();
        assert_eq!(document.treemaps[0].classes.len(), 0);
        assert_eq!(document.treemaps[0].children.len(), 3);
    }

    #[test]
    fn attributes_are_kept_in_order() {
        let document =
            parse("treemap { A: 2 [color = \"#fff\", showvalue = false, href = x]; }").unwrap();
        let node = &document.treemaps[0].children[0];
        assert_eq!(node.attr("color"), Some("#fff"));
        assert_eq!(node.attr("showvalue"), Some("false"));
        assert_eq!(node.attr("href"), Some("x"));
        assert_eq!(node.attrs[1].key, "showvalue");
    }

    #[test]
    fn diagram_attributes_and_comments() {
        let document =
            parse("# a comment\ntreemap {\n  width = 400; // trailing\n  /* block */ A: 2;\n}\n")
                .unwrap();
        let treemap = &document.treemaps[0];
        assert_eq!(treemap.attrs[0].key, "width");
        assert_eq!(treemap.attrs[0].value.as_deref(), Some("400"));
        assert_eq!(treemap.children.len(), 1);
    }

    #[test]
    fn root_can_be_a_node_name() {
        let document = parse("treemap { root; root [color = red]; root: 4; \"root\"; }").unwrap();
        assert_eq!(document.treemaps[0].children.len(), 4);
        assert!(document.treemaps[0].root.name.is_none());
    }

    #[test]
    fn root_with_braces_adds_children() {
        let document = parse("treemap { root R { A: 1; B: 2; } }").unwrap();
        assert_eq!(document.treemaps[0].root.name.as_deref(), Some("R"));
        assert_eq!(document.treemaps[0].children.len(), 2);
    }

    #[test]
    fn several_treemaps_in_one_file() {
        let document = parse("treemap a { X: 1; } treemap b { Y: 2; }").unwrap();
        assert_eq!(document.treemaps.len(), 2);
        assert_eq!(document.treemaps[1].id.as_deref(), Some("b"));
    }

    #[test]
    fn errors_have_positions() {
        let err = parse("treemap {\n  A\n}\n").unwrap_err();
        assert_eq!((err.line, err.column), (3, 1));
        assert!(err.message.contains("expected ';'"), "{err}");

        let err = parse("treemap { A: -1; }").unwrap_err();
        assert!(err.message.contains("negative"), "{err}");

        let err = parse("treemap { A: nope; }").unwrap_err();
        assert!(err.message.contains("number"), "{err}");

        let err = parse("graph { }").unwrap_err();
        assert!(err.message.contains("\"treemap\""), "{err}");

        let err = parse("treemap { \"abc; }").unwrap_err();
        assert!(err.message.contains("unterminated string"), "{err}");
    }

    #[test]
    fn a_second_root_statement_is_an_error() {
        let err = parse("treemap { root A; root B; }").unwrap_err();
        assert!(err.message.contains("only one root"), "{err}");
    }

    #[test]
    fn excessive_nesting_is_an_error_not_a_crash() {
        let mut source = String::from("treemap { ");
        for _ in 0..300 {
            source.push_str("A { ");
        }
        let err = parse(&source).unwrap_err();
        assert!(err.message.contains("nested"), "{err}");
    }
}
