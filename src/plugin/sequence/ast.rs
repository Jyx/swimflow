//! Syntax tree of the sequence diagram DSL.
//!
//! The sequence DSL is line-oriented and small:
//!
//! ```text
//! sequence "Checkout" {
//!   participant "Client" as client;
//!   participant "Server";
//!
//!   client -> Server: "POST /orders";
//!   Server --> client: "201 Created";
//!
//!   note over client, Server: "the request phase";
//!
//!   loop "until the order is confirmed" {
//!     Server -> client: "are you there?";
//!   }
//! }
//! ```

/// A parsed file: one or more `sequence` blocks.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document {
    pub diagrams: Vec<Diagram>,
}

/// A `sequence [id] { ... }` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagram {
    /// Optional diagram name.
    pub id: Option<String>,
    /// The statements of the block.
    pub items: Vec<Item>,
}

/// A statement inside a sequence block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// `participant NAME [as ALIAS];`
    Participant(Participant),
    /// `A -> B: text;`
    Message(Message),
    /// `note over A[, B]: text;`
    Note(Note),
    /// `loop|opt|par|alt|critical [label] { ... }`
    Block(Block),
}

/// A participant declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    /// The display name of the participant.
    pub name: String,
    /// An optional short alias usable in messages.
    pub alias: Option<String>,
}

/// A message between two participants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub kind: MessageKind,
    /// The message text (may be empty).
    pub text: String,
}

/// The arrow style of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    /// `->` — a solid line with a filled head: a synchronous call.
    Call,
    /// `->>` — a solid line with an open head: an asynchronous message.
    Async,
    /// `-->` — a dashed line with an open head: a reply.
    Reply,
    /// `--` — a dashed line without a head: a result.
    Return,
}

impl MessageKind {
    /// The operator that produced this kind.
    pub fn as_str(self) -> &'static str {
        match self {
            MessageKind::Call => "->",
            MessageKind::Async => "->>",
            MessageKind::Reply => "-->",
            MessageKind::Return => "--",
        }
    }

    /// Whether the line is dashed.
    pub fn is_dashed(self) -> bool {
        matches!(self, MessageKind::Reply | MessageKind::Return)
    }

    /// Whether the arrow has a head.
    pub fn has_head(self) -> bool {
        !matches!(self, MessageKind::Return)
    }

    /// Whether the head is filled.
    pub fn has_filled_head(self) -> bool {
        matches!(self, MessageKind::Call)
    }
}

/// A note attached to one or more participants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// The participants the note covers.
    pub over: Vec<String>,
    /// The note text.
    pub text: String,
}

/// A frame grouping a set of statements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: BlockKind,
    /// The optional label of the frame.
    pub label: Option<String>,
    pub items: Vec<Item>,
    /// The `else`/`and` branches of an `alt` or `par` frame. They are closed
    /// by the same `end` as the frame itself.
    pub branches: Vec<Block>,
}

/// The kind of a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Loop,
    Opt,
    Par,
    Alt,
    /// The `else` branch of an `alt` frame.
    Else,
    /// The `and` branch of a `par` frame.
    And,
    Critical,
    Break,
}

impl BlockKind {
    /// The keyword that introduces the frame.
    pub fn as_str(self) -> &'static str {
        match self {
            BlockKind::Loop => "loop",
            BlockKind::Opt => "opt",
            BlockKind::Par => "par",
            BlockKind::Alt => "alt",
            BlockKind::Else => "else",
            BlockKind::And => "and",
            BlockKind::Critical => "critical",
            BlockKind::Break => "break",
        }
    }

    /// The label of the frame's tab, e.g. `loop` or `alt / else`.
    pub fn tab_label(self) -> &'static str {
        match self {
            BlockKind::Else => "else",
            BlockKind::And => "and",
            other => other.as_str(),
        }
    }

    /// Parse a frame keyword.
    pub fn from_keyword(keyword: &str) -> Option<BlockKind> {
        match keyword.to_ascii_lowercase().as_str() {
            "loop" => Some(BlockKind::Loop),
            "opt" => Some(BlockKind::Opt),
            "par" => Some(BlockKind::Par),
            "alt" => Some(BlockKind::Alt),
            "else" => Some(BlockKind::Else),
            "and" => Some(BlockKind::And),
            "critical" => Some(BlockKind::Critical),
            "break" => Some(BlockKind::Break),
            _ => None,
        }
    }
}
