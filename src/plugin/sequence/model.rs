//! The sequence diagram model: the AST with participants resolved.
//!
//! Messages refer to participants by name or by alias; the model replaces
//! those names with indices into a flat participant list, which is what the
//! layout needs.

use std::collections::BTreeMap;

use super::ast::{self, BlockKind, MessageKind};

/// A resolved sequence diagram.
#[derive(Debug, Clone)]
pub struct Diagram {
    /// The diagram id (the block name, or `default`).
    pub id: String,
    /// The participants, in order of first appearance.
    pub participants: Vec<Participant>,
    /// The statements of the diagram.
    pub items: Vec<Item>,
}

/// A participant (a lifeline).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    /// The display name.
    pub name: String,
    /// The optional alias.
    pub alias: Option<String>,
}

/// A statement of a resolved diagram.
#[derive(Debug, Clone)]
pub enum Item {
    Message(Message),
    Note(Note),
    Block(Block),
}

/// A message between two participants.
#[derive(Debug, Clone)]
pub struct Message {
    /// Index of the sender.
    pub from: usize,
    /// Index of the receiver.
    pub to: usize,
    pub kind: MessageKind,
    pub text: String,
}

/// A note over one or more participants.
#[derive(Debug, Clone)]
pub struct Note {
    /// The participants the note covers.
    pub over: Vec<usize>,
    pub text: String,
}

/// A frame grouping statements.
#[derive(Debug, Clone)]
pub struct Block {
    pub kind: BlockKind,
    pub label: Option<String>,
    pub items: Vec<Item>,
    /// The `else`/`and` branches of an `alt`/`par` frame.
    pub branches: Vec<Block>,
}

/// A resolved document.
#[derive(Debug, Clone)]
pub struct Document {
    pub diagrams: Vec<Diagram>,
    /// Non-fatal problems found while resolving.
    pub warnings: Vec<String>,
}

/// Resolve an AST document into a model.
pub fn build(ast: &ast::Document) -> Document {
    let mut document = Document {
        diagrams: Vec::new(),
        warnings: Vec::new(),
    };

    for (index, diagram) in ast.diagrams.iter().enumerate() {
        let id = diagram.id.clone().unwrap_or_else(|| {
            if index == 0 {
                "default".to_string()
            } else {
                format!("diagram{index}")
            }
        });
        let mut builder = Builder {
            participants: Vec::new(),
            by_name: BTreeMap::new(),
            warnings: Vec::new(),
        };

        // Declared participants come first, in declaration order.
        for item in &diagram.items {
            if let ast::Item::Participant(declaration) = item {
                builder.declare(&declaration.name, declaration.alias.clone());
            }
        }

        let mut items = Vec::new();
        for item in &diagram.items {
            match item {
                ast::Item::Participant(_) => {}
                ast::Item::Message(message) => {
                    let from = builder.resolve(&message.from);
                    let to = builder.resolve(&message.to);
                    items.push(Item::Message(Message {
                        from,
                        to,
                        kind: message.kind,
                        text: message.text.clone(),
                    }));
                }
                ast::Item::Note(note) => {
                    let over = note.over.iter().map(|name| builder.resolve(name)).collect();
                    items.push(Item::Note(Note {
                        over,
                        text: note.text.clone(),
                    }));
                }
                ast::Item::Block(block) => {
                    items.push(Item::Block(Block {
                        kind: block.kind,
                        label: block.label.clone(),
                        items: builder.items(&block.items),
                        branches: builder.blocks(&block.branches),
                    }));
                }
            }
        }

        document.warnings.append(&mut builder.warnings);
        document.diagrams.push(Diagram {
            id,
            participants: builder.participants,
            items,
        });
    }

    document
}

/// Resolves participant names while walking the statements.
struct Builder {
    participants: Vec<Participant>,
    by_name: BTreeMap<String, usize>,
    warnings: Vec<String>,
}

impl Builder {
    /// Declare a participant (ignoring a repeated declaration).
    fn declare(&mut self, name: &str, alias: Option<String>) -> usize {
        let key = name.to_ascii_lowercase();
        if let Some(&index) = self.by_name.get(&key) {
            return index;
        }
        let index = self.participants.len();
        self.participants.push(Participant {
            name: name.to_string(),
            alias: alias.clone(),
        });
        self.by_name.insert(key, index);
        if let Some(alias) = alias {
            self.by_name.insert(alias.to_ascii_lowercase(), index);
        }
        index
    }

    /// Resolve a name, adding the participant when it is unknown.
    fn resolve(&mut self, name: &str) -> usize {
        let key = name.to_ascii_lowercase();
        if let Some(&index) = self.by_name.get(&key) {
            return index;
        }
        self.warnings.push(format!(
            "unknown participant {name:?}; it was added automatically"
        ));
        self.declare(name, None)
    }

    /// Resolve a list of statements.
    fn items(&mut self, items: &[ast::Item]) -> Vec<Item> {
        let mut out = Vec::new();
        for item in items {
            match item {
                ast::Item::Participant(declaration) => {
                    self.declare(&declaration.name, declaration.alias.clone());
                }
                ast::Item::Message(message) => {
                    let from = self.resolve(&message.from);
                    let to = self.resolve(&message.to);
                    out.push(Item::Message(Message {
                        from,
                        to,
                        kind: message.kind,
                        text: message.text.clone(),
                    }));
                }
                ast::Item::Note(note) => {
                    let over = note.over.iter().map(|name| self.resolve(name)).collect();
                    out.push(Item::Note(Note {
                        over,
                        text: note.text.clone(),
                    }));
                }
                ast::Item::Block(block) => out.push(Item::Block(Block {
                    kind: block.kind,
                    label: block.label.clone(),
                    items: self.items(&block.items),
                    branches: self.blocks(&block.branches),
                })),
            }
        }
        out
    }

    /// Resolve the branches of an `alt`/`par` frame.
    fn blocks(&mut self, blocks: &[ast::Block]) -> Vec<Block> {
        blocks
            .iter()
            .map(|block| Block {
                kind: block.kind,
                label: block.label.clone(),
                items: self.items(&block.items),
                branches: self.blocks(&block.branches),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::sequence::parser;

    fn model(source: &str) -> Document {
        let ast = parser::parse(source).expect("parse");
        build(&ast)
    }

    #[test]
    fn participants_are_resolved_by_name_and_alias() {
        let document =
            model("sequence { participant \"Client\" as c; participant Server; c -> Server: hi; }");
        let diagram = &document.diagrams[0];
        assert_eq!(diagram.participants.len(), 2);
        assert_eq!(diagram.participants[0].name, "Client");
        match &diagram.items[0] {
            Item::Message(message) => {
                assert_eq!(message.from, 0);
                assert_eq!(message.to, 1);
            }
            other => panic!("expected a message, got {other:?}"),
        }
    }

    #[test]
    fn unknown_participants_are_added_with_a_warning() {
        let document = model("sequence { A -> B: hi; }");
        assert_eq!(document.diagrams[0].participants.len(), 2);
        assert_eq!(document.warnings.len(), 2);
        assert!(document.warnings[0].contains("unknown participant"));
    }

    #[test]
    fn nested_blocks_keep_their_items() {
        let document = model("sequence { loop { A -> A: x; } }");
        match &document.diagrams[0].items[0] {
            Item::Block(block) => assert_eq!(block.items.len(), 1),
            other => panic!("expected a block, got {other:?}"),
        }
    }
}
