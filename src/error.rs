//! Error types with source positions.

/// A 1-based line/column position in the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// Errors produced while tokenizing or parsing a diagram.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("lex error at {pos}: {message}")]
    Lex { pos: Position, message: String },

    #[error("parse error at {pos}: {message}")]
    Parse { pos: Position, message: String },
}

impl Error {
    pub fn lex(line: u32, column: u32, message: impl Into<String>) -> Self {
        Error::Lex {
            pos: Position { line, column },
            message: message.into(),
        }
    }

    pub fn parse(line: u32, column: u32, message: impl Into<String>) -> Self {
        Error::Parse {
            pos: Position { line, column },
            message: message.into(),
        }
    }

    /// The position associated with the error.
    pub fn position(&self) -> Position {
        match self {
            Error::Lex { pos, .. } | Error::Parse { pos, .. } => *pos,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
