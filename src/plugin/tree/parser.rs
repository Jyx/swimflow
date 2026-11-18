//! The tree diagram parser: source text -> [`ast::Document`].
//!
//! The grammar is small, so the source is scanned into a token list first and
//! then read by a recursive-descent parser. Every problem is reported with a
//! line and a column.
//!
//! ```text
//! document := tree*
//! tree     := 'tree' [name] '{' statement* '}'
//! statement:= 'root' [name] [attrs] ( ';' | '{' node* '}' )
//!           | word '=' value ';'
//!           | node
//! node     := name [attrs] ( ';' | '{' node* '}' )
//! attrs    := '[' [ attr ( ',' attr )* ] ']'
//! attr     := word [ '=' value ]
//! ```

use std::fmt;

use super::ast::{Attr, Document, Node, RootDecl, Tree};

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

/// A token of the tree DSL.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
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
    Eof,
}

impl Token {
    fn describe(&self) -> String {
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
            Token::Eof => "the end of the file".to_string(),
        }
    }
}

/// A token and where it starts.
#[derive(Debug, Clone)]
struct Spanned {
    token: Token,
    line: usize,
    column: usize,
}

fn error<T>(message: impl Into<String>, line: usize, column: usize) -> Result<T, ParseError> {
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
fn scan(source: &str) -> Result<Vec<Spanned>, ParseError> {
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

    fn document(&mut self) -> Result<Document, ParseError> {
        let mut document = Document::default();
        while *self.token() != Token::Eof {
            if !self.is_word("tree") {
                return self.fail(format!(
                    "expected \"tree\", found {}",
                    self.token().describe()
                ));
            }
            self.advance();
            document.trees.push(self.tree()?);
        }
        Ok(document)
    }

    fn tree(&mut self) -> Result<Tree, ParseError> {
        let id = if *self.token() == Token::LBrace {
            None
        } else {
            Some(self.name("a tree name or '{'")?)
        };
        self.expect(&Token::LBrace)?;

        let mut tree = Tree {
            id,
            root: RootDecl::default(),
            attrs: Vec::new(),
            children: Vec::new(),
        };
        let mut seen_root = false;

        loop {
            match self.token() {
                Token::RBrace => {
                    self.advance();
                    return Ok(tree);
                }
                Token::Eof => return self.fail("expected '}', found the end of the file"),
                _ => {}
            }

            // `root [name] [attrs] ;` — but a lone `root;` or `root [..]` is
            // an ordinary node that happens to be called "root".
            if self.is_word("root")
                && matches!(self.ahead(), Token::Word(_) | Token::Str(_) | Token::LBrace)
            {
                if seen_root {
                    return self.fail("a tree has only one root statement");
                }
                seen_root = true;
                self.advance();
                if matches!(self.token(), Token::Word(_) | Token::Str(_)) {
                    tree.root.name = Some(self.name("a root name")?);
                }
                tree.root.attrs = self.attrs()?;
                match self.token() {
                    Token::Semi => {
                        self.advance();
                    }
                    Token::LBrace => {
                        self.advance();
                        let children = self.children(1)?;
                        tree.children.extend(children);
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
                tree.attrs.push(Attr {
                    key,
                    value: Some(value),
                });
                continue;
            }

            let node = self.node(1)?;
            tree.children.push(node);
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

/// Parse a tree diagram source.
pub fn parse(source: &str) -> Result<Document, ParseError> {
    let tokens = scan(source)?;
    Parser { tokens, index: 0 }.document()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tree_with_nested_children() {
        let document = parse(
            "tree \"Product\" {\n  root \"Swimflow\";\n  Core {\n    Parser;\n    Renderer { SVG; PNG; }\n  }\n  Docs;\n}\n",
        )
        .unwrap();
        assert_eq!(document.trees.len(), 1);
        let tree = &document.trees[0];
        assert_eq!(tree.id.as_deref(), Some("Product"));
        assert_eq!(tree.root.name.as_deref(), Some("Swimflow"));
        assert_eq!(tree.children.len(), 2);
        let core = &tree.children[0];
        assert_eq!(core.name, "Core");
        assert_eq!(core.children[1].children.len(), 2);
        assert_eq!(core.children[1].children[1].name, "PNG");
    }

    #[test]
    fn attributes_are_kept_in_order() {
        let document = parse("tree { A [color = \"#fff\", collapsed, edge = yes]; }").unwrap();
        let node = &document.trees[0].children[0];
        assert_eq!(node.attr("color"), Some("#fff"));
        assert_eq!(node.attr("edge"), Some("yes"));
        assert_eq!(node.attrs[1].key, "collapsed");
        assert_eq!(node.attrs[1].value, None);
    }

    #[test]
    fn diagram_attributes_and_comments() {
        let document = parse(
            "# a comment\ntree {\n  orientation = landscape; // trailing\n  /* block */ A;\n}\n",
        )
        .unwrap();
        let tree = &document.trees[0];
        assert_eq!(tree.attrs[0].key, "orientation");
        assert_eq!(tree.attrs[0].value.as_deref(), Some("landscape"));
        assert_eq!(tree.children.len(), 1);
    }

    #[test]
    fn root_can_be_a_node_name() {
        let document = parse("tree { root; root [color = red]; \"root\"; }").unwrap();
        assert_eq!(document.trees[0].children.len(), 3);
        assert!(document.trees[0].root.name.is_none());
    }

    #[test]
    fn root_with_braces_adds_children() {
        let document = parse("tree { root R { A; B; } }").unwrap();
        assert_eq!(document.trees[0].root.name.as_deref(), Some("R"));
        assert_eq!(document.trees[0].children.len(), 2);
    }

    #[test]
    fn several_trees_in_one_file() {
        let document = parse("tree a { X; } tree b { Y; }").unwrap();
        assert_eq!(document.trees.len(), 2);
        assert_eq!(document.trees[1].id.as_deref(), Some("b"));
    }

    #[test]
    fn errors_have_positions() {
        let err = parse("tree {\n  A\n}\n").unwrap_err();
        assert_eq!((err.line, err.column), (3, 1));
        assert!(err.message.contains("expected ';'"), "{err}");

        let err = parse("tree { A; ").unwrap_err();
        assert!(err.message.contains("'}'"), "{err}");

        let err = parse("tree { A [color]; B [x = ; }").unwrap_err();
        assert!(err.message.contains("attribute value"), "{err}");

        let err = parse("graph { }").unwrap_err();
        assert!(err.message.contains("\"tree\""), "{err}");

        let err = parse("tree { \"abc; }").unwrap_err();
        assert!(err.message.contains("unterminated string"), "{err}");
    }

    #[test]
    fn a_second_root_statement_is_an_error() {
        let err = parse("tree { root A; root B; }").unwrap_err();
        assert!(err.message.contains("only one root"), "{err}");
    }

    #[test]
    fn excessive_nesting_is_an_error_not_a_crash() {
        let mut source = String::from("tree { ");
        for _ in 0..300 {
            source.push_str("A { ");
        }
        let err = parse(&source).unwrap_err();
        assert!(err.message.contains("nested"), "{err}");
    }
}
