//! Tokenizer for the activity DSL.
//!
//! Ported to match the reference tokenizer exactly, including its quirks:
//!
//!   * `Name` matches first and is greedy, and `Name` may contain `-` and `.`.
//!     As a result `A->B` is a **lex error** (`A-` is consumed as a name), while
//!     `A -> B` is fine. This is faithful to the reference.
//!   * `Number` is only reachable when the token does not start with a name
//!     character, i.e. for forms like `-1`, `-.5`, `.5`.
//!   * Strings support `"…"`, `'…'`, `"""…"""`, `'''…'''`, and a closing
//!     delimiter must not be immediately preceded by a backslash.

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Name,
    Number,
    String,
    Op,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub value: String,
    pub line: u32,
    pub column: u32,
}

impl Token {
    fn new(kind: TokenKind, value: String, line: u32, column: u32) -> Self {
        Token {
            kind,
            value,
            line,
            column,
        }
    }
}

/// Tokenize `input`, skipping whitespace and comments.
pub fn tokenize(input: &str) -> Result<Vec<Token>> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    let mut line = 1u32;
    let mut col = 1u32;

    while i < chars.len() {
        let c = chars[i];

        // --- comments -------------------------------------------------------
        if c == '/' && starts_with(&chars, i, "/*") {
            i += 2;
            col += 2;
            let mut closed = false;
            while i < chars.len() {
                if chars[i] == '\n' {
                    line += 1;
                    col = 1;
                    i += 1;
                } else if chars[i] == '*' && starts_with(&chars, i, "*/") {
                    i += 2;
                    col += 2;
                    closed = true;
                    break;
                } else {
                    i += 1;
                    col += 1;
                }
            }
            if !closed {
                return Err(Error::lex(line, col, "unterminated comment"));
            }
            continue;
        }
        if c == '/' && starts_with(&chars, i, "//") {
            while i < chars.len() && chars[i] != '\n' && chars[i] != '\r' {
                i += 1;
                col += 1;
            }
            continue;
        }
        if c == '#' {
            while i < chars.len() && chars[i] != '\n' && chars[i] != '\r' {
                i += 1;
                col += 1;
            }
            continue;
        }

        // --- whitespace -----------------------------------------------------
        if c == '\n' {
            line += 1;
            col = 1;
            i += 1;
            continue;
        }
        if c == '\r' {
            i += 1;
            if i < chars.len() && chars[i] == '\n' {
                i += 1;
            }
            line += 1;
            col = 1;
            continue;
        }
        if c == ' ' || c == '\t' {
            i += 1;
            col += 1;
            continue;
        }

        let start_line = line;
        let start_col = col;

        // --- Name (tried before Number/Op, greedy) --------------------------
        if is_name_start(c) {
            let start = i;
            i += 1;
            col += 1;
            while i < chars.len() && is_name_continue(chars[i]) {
                i += 1;
                col += 1;
            }
            tokens.push(Token::new(
                TokenKind::Name,
                chars[start..i].iter().collect(),
                start_line,
                start_col,
            ));
            continue;
        }

        // --- Op -------------------------------------------------------------
        if let Some(len) = single_char_op(c) {
            tokens.push(Token::new(
                TokenKind::Op,
                c.to_string(),
                start_line,
                start_col,
            ));
            i += len;
            col += len as u32;
            continue;
        }
        if let Some(op) = multi_char_op(&chars, i) {
            tokens.push(Token::new(
                TokenKind::Op,
                op.to_string(),
                start_line,
                start_col,
            ));
            i += op.chars().count();
            col += op.chars().count() as u32;
            continue;
        }

        // --- Number ---------------------------------------------------------
        if let Some((value, len)) = match_number(&chars, i) {
            tokens.push(Token::new(TokenKind::Number, value, start_line, start_col));
            i += len;
            col += len as u32;
            continue;
        }

        // --- String ---------------------------------------------------------
        if c == '"' || c == '\'' {
            let (value, end) = scan_string(&chars, i)
                .ok_or_else(|| Error::lex(start_line, start_col, "unterminated string literal"))?;
            tokens.push(Token::new(TokenKind::String, value, start_line, start_col));
            for &ch in &chars[i..end] {
                if ch == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
            }
            i = end;
            continue;
        }

        return Err(Error::lex(
            start_line,
            start_col,
            format!("unexpected character {c:?}"),
        ));
    }

    Ok(tokens)
}

fn is_name_start(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || ('\u{80}'..='\u{ffff}').contains(&c)
}

fn is_name_continue(c: char) -> bool {
    is_name_start(c) || c == '-' || c == '.'
}

/// Single-character operators: `{};,=[]`
fn single_char_op(c: char) -> Option<usize> {
    if matches!(c, '{' | '}' | ';' | ',' | '=' | '[' | ']') {
        Some(1)
    } else {
        None
    }
}

/// Multi-character operators, in the same priority order as upstream:
/// `<->`, `<-`, `--`, `->`.
fn multi_char_op(chars: &[char], i: usize) -> Option<&'static str> {
    if starts_with(chars, i, "<->") {
        Some("<->")
    } else if starts_with(chars, i, "<-") {
        Some("<-")
    } else if starts_with(chars, i, "--") {
        Some("--")
    } else if starts_with(chars, i, "->") {
        Some("->")
    } else {
        None
    }
}

/// Matches the reachable part of `-?(\.[0-9]+)|([0-9]+(\.[0-9]*)?)`.
///
/// `Name` is tried first and accepts digits, dots and dashes, so the
/// `[0-9]+...` branch is never reached; the effective form is `-?\.[0-9]+`.
/// Consequently `-1` and `-1.5` are **lex errors** upstream, while `.5` and
/// `-.5` are numbers.
fn match_number(chars: &[char], i: usize) -> Option<(String, usize)> {
    let mut j = i;
    if chars.get(j) == Some(&'-') {
        j += 1;
    }

    if chars.get(j) != Some(&'.') {
        return None;
    }
    j += 1;
    let digits_start = j;
    while j < chars.len() && chars[j].is_ascii_digit() {
        j += 1;
    }
    if j == digits_start {
        return None;
    }
    Some((chars[i..j].iter().collect(), j - i))
}

/// Scans a string literal. Returns the raw token value (including delimiters)
/// and the end index (exclusive).
fn scan_string(chars: &[char], i: usize) -> Option<(String, usize)> {
    let delim: &str = if starts_with(chars, i, "\"\"\"") {
        "\"\"\""
    } else if starts_with(chars, i, "'''") {
        "'''"
    } else if chars[i] == '"' {
        "\""
    } else {
        "'"
    };
    let delim_chars: Vec<char> = delim.chars().collect();
    let dlen = delim_chars.len();

    let mut j = i + dlen;
    let mut close_at = None;
    loop {
        if j + dlen <= chars.len()
            && chars[j..j + dlen] == delim_chars[..]
            && (j == i + dlen || chars[j - 1] != '\\')
        {
            close_at = Some(j);
            break;
        }
        if j >= chars.len() {
            break;
        }
        j += 1;
    }

    let close = close_at?;
    let end = close + dlen;
    Some((chars[i..end].iter().collect(), end))
}

fn starts_with(chars: &[char], i: usize, pat: &str) -> bool {
    let pat: Vec<char> = pat.chars().collect();
    i + pat.len() <= chars.len() && chars[i..i + pat.len()] == pat[..]
}

/// Remove surrounding quotes and unescape the delimiter, mirroring
/// `blockdiag.utils.unquote`.
pub fn unquote(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let replaced = s.replace("\"\"\"", "").replace("'''", "");
    let bytes: Vec<char> = replaced.chars().collect();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == '"' || first == '\'') && first == last {
            let inner: String = bytes[1..bytes.len() - 1].iter().collect();
            let escaped = format!("\\{first}");
            return inner.replace(&escaped, &first.to_string());
        }
    }
    replaced
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<(TokenKind, String)> {
        tokenize(s)
            .unwrap()
            .into_iter()
            .map(|t| (t.kind, t.value))
            .collect()
    }

    #[test]
    fn simple_edge_with_spaces() {
        assert_eq!(
            toks("A -> B"),
            vec![
                (TokenKind::Name, "A".into()),
                (TokenKind::Op, "->".into()),
                (TokenKind::Name, "B".into()),
            ]
        );
    }

    #[test]
    fn edge_without_spaces_is_a_lex_error() {
        // Faithful to upstream: "A-" is consumed as a name.
        assert!(tokenize("A->B").is_err());
    }

    #[test]
    fn double_dash_without_spaces_is_a_name() {
        assert_eq!(toks("A--B"), vec![(TokenKind::Name, "A--B".into())]);
    }

    #[test]
    fn angle_relations_do_not_conflict() {
        assert_eq!(
            toks("A<->B"),
            vec![
                (TokenKind::Name, "A".into()),
                (TokenKind::Op, "<->".into()),
                (TokenKind::Name, "B".into()),
            ]
        );
        assert_eq!(
            toks("A<-B"),
            vec![
                (TokenKind::Name, "A".into()),
                (TokenKind::Op, "<-".into()),
                (TokenKind::Name, "B".into()),
            ]
        );
    }

    #[test]
    fn names_may_contain_dash_and_dot() {
        assert_eq!(toks("a-b.c"), vec![(TokenKind::Name, "a-b.c".into())]);
    }

    #[test]
    fn unicode_names() {
        assert_eq!(
            toks("\u{3042}\u{3044}"),
            vec![(TokenKind::Name, "\u{3042}\u{3044}".into())]
        );
    }

    #[test]
    fn comments_are_skipped() {
        assert_eq!(
            toks("A // hi\n B"),
            vec![(TokenKind::Name, "A".into()), (TokenKind::Name, "B".into()),]
        );
        assert_eq!(toks("# hi\nA"), vec![(TokenKind::Name, "A".into())]);
        assert_eq!(toks("/* hi */A"), vec![(TokenKind::Name, "A".into())]);
    }

    #[test]
    fn strings_all_quote_forms() {
        assert_eq!(toks("\"a b\""), vec![(TokenKind::String, "\"a b\"".into())]);
        assert_eq!(toks("'a b'"), vec![(TokenKind::String, "'a b'".into())]);
        assert_eq!(
            toks("\"\"\"a\nb\"\"\""),
            vec![(TokenKind::String, "\"\"\"a\nb\"\"\"".into())]
        );
        assert_eq!(toks("'''x'''"), vec![(TokenKind::String, "'''x'''".into())]);
    }

    #[test]
    fn escaped_quote_does_not_close() {
        assert_eq!(
            toks(r#""a\"b""#),
            vec![(TokenKind::String, r#""a\"b""#.into())]
        );
    }

    #[test]
    fn numbers_only_for_leading_dot_forms() {
        // Faithful to upstream: only `-?\.digits` is a Number token.
        assert_eq!(toks(".5"), vec![(TokenKind::Number, ".5".into())]);
        assert_eq!(toks("-.5"), vec![(TokenKind::Number, "-.5".into())]);
        // bare digits and trailing dots are names (Name is tried first)
        assert_eq!(toks("42"), vec![(TokenKind::Name, "42".into())]);
        assert_eq!(toks("1.5"), vec![(TokenKind::Name, "1.5".into())]);
        assert_eq!(toks("1."), vec![(TokenKind::Name, "1.".into())]);
        // negative integers are lex errors
        assert!(tokenize("-1").is_err());
        assert!(tokenize("-1.5").is_err());
        assert!(tokenize("-").is_err());
        assert!(tokenize(".").is_err());
    }

    #[test]
    fn unquote_forms() {
        assert_eq!(unquote("\"abc\""), "abc");
        assert_eq!(unquote("'abc'"), "abc");
        assert_eq!(unquote("\"\"\"abc\"\"\""), "abc");
        assert_eq!(unquote(r#""a\"b""#), "a\"b".replace("\\", ""));
        assert_eq!(unquote("abc"), "abc");
    }

    #[test]
    fn position_tracking() {
        let t = tokenize("A\n  B").unwrap();
        assert_eq!((t[1].line, t[1].column), (2, 3));
    }
}
