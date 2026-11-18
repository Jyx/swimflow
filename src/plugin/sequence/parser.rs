//! The sequence diagram parser: source text -> [`ast::Document`].
//!
//! The grammar is small and line-oriented, so a hand-written scanner and a
//! recursive-descent parser are enough (and keep the error messages precise:
//! every problem is reported with a line and a column).

use std::fmt;

use super::ast::{self, BlockKind, Item, MessageKind};

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

/// A token of the sequence DSL.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Ident(String),
    Str(String),
    Arrow(MessageKind),
    Colon,
    Semi,
    LBrace,
    RBrace,
    Comma,
    As,
    Text(String),
    Eof,
}

impl Token {
    /// A human-readable description, used in error messages.
    fn describe(&self) -> String {
        match self {
            Token::Ident(name) => format!("{name:?}"),
            Token::Str(_) => "a string".to_string(),
            Token::Arrow(kind) => format!("{:?}", kind.as_str()),
            Token::Colon => "':'".to_string(),
            Token::Semi => "';'".to_string(),
            Token::LBrace => "'{'".to_string(),
            Token::RBrace => "'}'".to_string(),
            Token::Comma => "','".to_string(),
            Token::As => "'as'".to_string(),
            Token::Text(_) => "a text".to_string(),
            Token::Eof => "the end of the file".to_string(),
        }
    }
}

/// The scanner.
struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
    column: usize,
    /// Set after a `:` — the next token is the raw statement text.
    text_mode: bool,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Lexer<'a> {
        Lexer {
            src: src.as_bytes(),
            pos: 0,
            line: 1,
            column: 1,
            text_mode: false,
        }
    }

    fn error<T>(&self, message: impl Into<String>) -> Result<T, ParseError> {
        Err(ParseError {
            message: message.into(),
            line: self.line,
            column: self.column,
        })
    }

    fn peek_byte(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.src.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.src.get(self.pos).copied()?;
        self.pos += 1;
        if byte == b'\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(byte)
    }

    /// Skip whitespace and comments.
    fn skip_trivia(&mut self) {
        loop {
            match self.peek_byte() {
                Some(b' ') | Some(b'\t') | Some(b'\r') | Some(b'\n') => {
                    self.bump();
                }
                Some(b'#') => {
                    while let Some(byte) = self.peek_byte() {
                        if byte == b'\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                Some(b'/') if self.peek_at(1) == Some(b'/') => {
                    while let Some(byte) = self.peek_byte() {
                        if byte == b'\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                Some(b'/') if self.peek_at(1) == Some(b'*') => {
                    self.bump();
                    self.bump();
                    loop {
                        match self.peek_byte() {
                            None => break,
                            Some(b'*') if self.peek_at(1) == Some(b'/') => {
                                self.bump();
                                self.bump();
                                break;
                            }
                            _ => {
                                self.bump();
                            }
                        }
                    }
                }
                _ => break,
            }
        }
    }

    /// The next token.
    fn next_token(&mut self) -> Result<Token, ParseError> {
        if self.text_mode {
            self.text_mode = false;
            return self.read_text();
        }

        self.skip_trivia();
        let Some(byte) = self.peek_byte() else {
            return Ok(Token::Eof);
        };

        match byte {
            b'{' => {
                self.bump();
                Ok(Token::LBrace)
            }
            b'}' => {
                self.bump();
                Ok(Token::RBrace)
            }
            b';' => {
                self.bump();
                Ok(Token::Semi)
            }
            b',' => {
                self.bump();
                Ok(Token::Comma)
            }
            b':' => {
                self.bump();
                self.text_mode = true;
                Ok(Token::Colon)
            }
            b'"' | b'\'' => self.read_string(),
            b'-' => self.read_arrow(),
            _ if is_ident_start(byte) => {
                let word = self.read_word();
                if word.eq_ignore_ascii_case("as") {
                    Ok(Token::As)
                } else {
                    Ok(Token::Ident(word))
                }
            }
            _ => {
                let ch = byte as char;
                self.bump();
                self.error(format!("unexpected character {ch:?}"))
            }
        }
    }

    fn read_word(&mut self) -> String {
        let start = self.pos;
        while let Some(byte) = self.peek_byte() {
            if is_ident_continue(byte) {
                self.bump();
            } else {
                break;
            }
        }
        String::from_utf8_lossy(&self.src[start..self.pos]).into_owned()
    }

    fn read_string(&mut self) -> Result<Token, ParseError> {
        let quote = self.bump().expect("a quote");
        let mut value = String::new();
        loop {
            let Some(byte) = self.bump() else {
                return self.error("unterminated string");
            };
            if byte == quote {
                break;
            }
            if byte == b'\\' {
                let Some(escaped) = self.bump() else {
                    return self.error("unterminated escape sequence");
                };
                value.push(match escaped {
                    b'n' => '\n',
                    b't' => '\t',
                    other => other as char,
                });
                continue;
            }
            value.push(byte as char);
        }
        Ok(Token::Str(value))
    }

    fn read_arrow(&mut self) -> Result<Token, ParseError> {
        let rest = &self.src[self.pos..];
        let kind = if rest.starts_with(b"->>") {
            MessageKind::Async
        } else if rest.starts_with(b"-->") {
            MessageKind::Reply
        } else if rest.starts_with(b"->") {
            MessageKind::Call
        } else if rest.starts_with(b"--") {
            MessageKind::Return
        } else {
            return self.error("expected a message operator (->, ->>, --> or --)");
        };
        for _ in 0..kind.as_str().len() {
            self.bump();
        }
        Ok(Token::Arrow(kind))
    }

    /// Read the raw text of a statement, up to the closing `;`.
    fn read_text(&mut self) -> Result<Token, ParseError> {
        self.skip_trivia();
        let start = self.pos;
        let mut quote: Option<u8> = None;
        while let Some(byte) = self.peek_byte() {
            match quote {
                Some(q) => {
                    if byte == b'\\' {
                        self.bump();
                        self.bump();
                        continue;
                    }
                    self.bump();
                    if byte == q {
                        quote = None;
                    }
                }
                None => {
                    if byte == b'"' || byte == b'\'' {
                        quote = Some(byte);
                        self.bump();
                        continue;
                    }
                    if byte == b';' {
                        break;
                    }
                    if byte == b'\n' {
                        // A statement may not span lines; stop here and let the
                        // caller report the missing `;`.
                        break;
                    }
                    self.bump();
                }
            }
        }

        let raw = String::from_utf8_lossy(&self.src[start..self.pos])
            .trim()
            .to_string();
        if self.peek_byte() == Some(b';') {
            self.bump();
        }

        // A fully quoted text is unquoted; anything else is taken literally.
        let text = if raw.len() >= 2 {
            let first = raw.as_bytes()[0];
            let last = raw.as_bytes()[raw.len() - 1];
            if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
                raw[1..raw.len() - 1].to_string()
            } else {
                raw
            }
        } else {
            raw
        };

        Ok(Token::Text(text))
    }
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The parser.
struct Parser<'a> {
    lexer: Lexer<'a>,
    token: Token,
    /// One token of lookahead, used to tell a block label from a statement.
    ahead: Option<Token>,
    line: usize,
    column: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Result<Parser<'a>, ParseError> {
        let mut lexer = Lexer::new(source);
        let token = lexer.next_token()?;
        Ok(Parser {
            lexer,
            token,
            ahead: None,
            line: 1,
            column: 1,
        })
    }

    fn error<T>(&self, message: impl Into<String>) -> Result<T, ParseError> {
        Err(ParseError {
            message: message.into(),
            line: self.line,
            column: self.column,
        })
    }

    fn advance(&mut self) -> Result<Token, ParseError> {
        let current = self.token.clone();
        self.line = self.lexer.line;
        self.column = self.lexer.column;
        self.token = match self.ahead.take() {
            Some(token) => token,
            None => self.lexer.next_token()?,
        };
        Ok(current)
    }

    /// The token after the current one.
    fn peek_ahead(&mut self) -> Result<Token, ParseError> {
        if self.ahead.is_none() {
            self.ahead = Some(self.lexer.next_token()?);
        }
        Ok(self.ahead.clone().expect("the lookahead is filled"))
    }

    fn is_keyword(&self, keyword: &str) -> bool {
        matches!(&self.token, Token::Ident(word) if word.eq_ignore_ascii_case(keyword))
    }

    fn eat_keyword(&mut self, keyword: &str) -> Result<bool, ParseError> {
        if self.is_keyword(keyword) {
            self.advance()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<(), ParseError> {
        if self.eat_keyword(keyword)? {
            Ok(())
        } else {
            self.error(format!(
                "expected {keyword:?}, found {}",
                self.token.describe()
            ))
        }
    }

    fn expect(&mut self, token: &Token) -> Result<(), ParseError> {
        if &self.token == token {
            self.advance()?;
            Ok(())
        } else {
            self.error(format!(
                "expected {}, found {}",
                token.describe(),
                self.token.describe()
            ))
        }
    }

    /// A name: an identifier or a quoted string.
    fn name(&mut self) -> Result<String, ParseError> {
        match self.token.clone() {
            Token::Ident(name) => {
                self.advance()?;
                Ok(name)
            }
            Token::Str(name) => {
                self.advance()?;
                Ok(name)
            }
            _ => self.error(format!("expected a name, found {}", self.token.describe())),
        }
    }

    /// Parse the whole file.
    fn parse_document(&mut self) -> Result<ast::Document, ParseError> {
        let mut document = ast::Document::default();
        while !matches!(self.token, Token::Eof) {
            self.expect_keyword("sequence")?;
            let id = match &self.token {
                Token::Ident(_) | Token::Str(_) => Some(self.name()?),
                _ => None,
            };
            self.expect(&Token::LBrace)?;
            let items = self.parse_items(false)?;
            self.expect(&Token::RBrace)?;
            document.diagrams.push(ast::Diagram { id, items });
        }
        Ok(document)
    }

    /// Parse the statements of a block.
    ///
    /// `branches` tells the parser that the enclosing frame accepts `else`
    /// and `and` branches: those keywords end the statement list instead of
    /// starting a nested frame.
    fn parse_items(&mut self, branches: bool) -> Result<Vec<Item>, ParseError> {
        let mut items = Vec::new();
        loop {
            match self.token.clone() {
                Token::RBrace | Token::Eof => break,
                // A block may also be closed with the `end` keyword.
                Token::Ident(word) if word.eq_ignore_ascii_case("end") => break,
                Token::Ident(word) => {
                    let lower = word.to_ascii_lowercase();
                    if branches && (lower == "else" || lower == "and") {
                        break;
                    }
                    match lower.as_str() {
                        "participant" => items.push(Item::Participant(self.parse_participant()?)),
                        "note" => items.push(Item::Note(self.parse_note()?)),
                        _ if BlockKind::from_keyword(&lower).is_some() => {
                            items.push(Item::Block(self.parse_block()?));
                        }
                        _ => items.push(Item::Message(self.parse_message()?)),
                    }
                }
                _ => {
                    return self.error(format!(
                        "expected a statement, found {}",
                        self.token.describe()
                    ));
                }
            }
        }
        Ok(items)
    }

    fn parse_participant(&mut self) -> Result<ast::Participant, ParseError> {
        self.expect_keyword("participant")?;
        let name = self.name()?;
        let alias = if matches!(self.token, Token::As) {
            self.advance()?;
            Some(self.name()?)
        } else {
            None
        };
        self.expect(&Token::Semi)?;
        Ok(ast::Participant { name, alias })
    }

    fn parse_note(&mut self) -> Result<ast::Note, ParseError> {
        self.expect_keyword("note")?;
        if !self.eat_keyword("over")? {
            return self.error(format!(
                "only `note over PARTICIPANT[, PARTICIPANT]: text;` is supported, found {}",
                self.token.describe()
            ));
        }
        let mut over = vec![self.name()?];
        while matches!(self.token, Token::Comma) {
            self.advance()?;
            over.push(self.name()?);
        }
        self.expect(&Token::Colon)?;
        let text = self.expect_text()?;
        Ok(ast::Note { over, text })
    }

    fn parse_message(&mut self) -> Result<ast::Message, ParseError> {
        let from = self.name()?;
        let kind = match self.advance()? {
            Token::Arrow(kind) => kind,
            other => {
                return self.error(format!(
                    "expected a message operator (->, ->>, --> or --), found {}",
                    other.describe()
                ));
            }
        };
        let to = self.name()?;
        self.expect(&Token::Colon)?;
        let text = self.expect_text()?;
        Ok(ast::Message {
            from,
            to,
            kind,
            text,
        })
    }

    fn parse_block(&mut self) -> Result<ast::Block, ParseError> {
        let (kind, label, braced) = self.parse_block_header()?;
        let items = self.parse_items(matches!(kind, ast::BlockKind::Alt | ast::BlockKind::Par))?;

        // The `else`/`and` branches of an `alt`/`par` frame belong to the
        // frame and are closed by the same `end`.
        let mut branches = Vec::new();
        if matches!(kind, ast::BlockKind::Alt | ast::BlockKind::Par) {
            while self.is_keyword("else") || self.is_keyword("and") {
                let (branch_kind, branch_label, branch_braced) = self.parse_block_header()?;
                let branch_items = self.parse_items(false)?;
                if branch_braced {
                    self.close_block()?;
                }
                branches.push(ast::Block {
                    kind: branch_kind,
                    label: branch_label,
                    items: branch_items,
                    branches: Vec::new(),
                });
            }
        }

        if braced {
            self.close_block()?;
        } else {
            self.expect_keyword("end")?;
        }
        Ok(ast::Block {
            kind,
            label,
            items,
            branches,
        })
    }

    /// Read the keyword, the optional label and the opening brace of a block.
    fn parse_block_header(&mut self) -> Result<(ast::BlockKind, Option<String>, bool), ParseError> {
        let keyword = self.name()?;
        let kind = ast::BlockKind::from_keyword(&keyword).ok_or_else(|| ParseError {
            message: format!("unknown block type {keyword:?}"),
            line: self.line,
            column: self.column,
        })?;

        // A block may be written with braces (`loop { ... }`) or closed with
        // the `end` keyword (`loop ... end`). The label is optional; a name
        // right before an arrow starts a message, not a label.
        let mut label = None;
        let mut braced = false;
        match self.token.clone() {
            Token::LBrace => {
                braced = true;
                self.advance()?;
            }
            Token::Str(text) => {
                label = Some(text);
                self.advance()?;
                if matches!(self.token, Token::LBrace) {
                    braced = true;
                    self.advance()?;
                }
            }
            Token::Ident(_) => {
                let next = self.peek_ahead()?;
                if !matches!(next, Token::Arrow(_)) {
                    label = Some(self.name()?);
                    if matches!(self.token, Token::LBrace) {
                        braced = true;
                        self.advance()?;
                    }
                }
            }
            _ => {}
        }

        Ok((kind, label, braced))
    }

    /// Close a block: either with a brace or with the `end` keyword.
    fn close_block(&mut self) -> Result<(), ParseError> {
        if matches!(self.token, Token::RBrace) {
            self.advance()?;
            return Ok(());
        }
        if self.eat_keyword("end")? {
            return Ok(());
        }
        self.error(format!(
            "expected `}}` or `end` to close the block, found {}",
            self.token.describe()
        ))
    }

    fn expect_text(&mut self) -> Result<String, ParseError> {
        match self.advance()? {
            Token::Text(text) => Ok(text),
            other => self.error(format!("expected a text, found {}", other.describe())),
        }
    }
}

/// Parse a sequence diagram source file.
pub fn parse(source: &str) -> Result<ast::Document, ParseError> {
    Parser::new(source)?.parse_document()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minimal_diagram_parses() {
        let document = parse("sequence demo { participant A; A -> B: hello; }").unwrap();
        assert_eq!(document.diagrams.len(), 1);
        let diagram = &document.diagrams[0];
        assert_eq!(diagram.id.as_deref(), Some("demo"));
        assert_eq!(diagram.items.len(), 2);
    }

    #[test]
    fn aliases_and_messages_are_parsed() {
        let document = parse(
            "sequence {\n  participant \"Client\" as c;\n  participant Server;\n  \
             c -> Server: \"hi\";\n  Server --> c: \"hello\";\n}",
        )
        .unwrap();
        let items = &document.diagrams[0].items;
        match &items[0] {
            Item::Participant(p) => {
                assert_eq!(p.name, "Client");
                assert_eq!(p.alias.as_deref(), Some("c"));
            }
            other => panic!("expected a participant, got {other:?}"),
        }
        match &items[2] {
            Item::Message(m) => {
                assert_eq!(m.from, "c");
                assert_eq!(m.to, "Server");
                assert_eq!(m.kind, MessageKind::Call);
                assert_eq!(m.text, "hi");
            }
            other => panic!("expected a message, got {other:?}"),
        }
        match &items[3] {
            Item::Message(m) => assert_eq!(m.kind, MessageKind::Reply),
            other => panic!("expected a message, got {other:?}"),
        }
    }

    #[test]
    fn blocks_and_notes_are_parsed() {
        let document =
            parse("sequence { note over A, B: \"text\"; loop \"retry\" { A -> A: x; } }").unwrap();
        let items = &document.diagrams[0].items;
        match &items[0] {
            Item::Note(note) => {
                assert_eq!(note.over, vec!["A", "B"]);
                assert_eq!(note.text, "text");
            }
            other => panic!("expected a note, got {other:?}"),
        }
        match &items[1] {
            Item::Block(block) => {
                assert_eq!(block.kind, BlockKind::Loop);
                assert_eq!(block.label.as_deref(), Some("retry"));
                assert_eq!(block.items.len(), 1);
            }
            other => panic!("expected a block, got {other:?}"),
        }
    }

    #[test]
    fn comments_are_ignored() {
        let document =
            parse("# a comment\nsequence { // another\n A -> B: x; /* block */ }").unwrap();
        assert_eq!(document.diagrams[0].items.len(), 1);
    }

    #[test]
    fn errors_carry_a_position() {
        let err = parse("sequence { A -> ; }").unwrap_err();
        assert_eq!(err.line, 1);
        assert!(err.to_string().contains("line 1"));
    }

    #[test]
    fn an_unknown_keyword_is_reported() {
        let err = parse("sequence { A -> B: x; } diagram { }").unwrap_err();
        assert!(err.to_string().contains("sequence"), "{err}");
    }
}
