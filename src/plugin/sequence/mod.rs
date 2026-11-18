//! The sequence diagram plugin.
//!
//! A sequence diagram is a completely different kind of drawing from a
//! swimlane diagram: it has lifelines instead of lanes and arrows instead of
//! edges. What it shares with the activity diagram is everything the *core*
//! provides — fonts, text measurement, the SVG primitives, the PNG/PDF
//! conversion and the HTML bundle.
//!
//! This module is the proof that the split works: it contains a parser, a
//! model, a layout and a renderer, and it plugs into the CLI like any other
//! diagram type.
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
//!   loop "until confirmed" {
//!     Server -> client: "still there?";
//!   }
//! }
//! ```

pub mod ast;
pub mod layout;
pub mod model;
pub mod parser;
pub mod render;

use crate::core::Section;
use crate::render::RenderOptions;

use super::{DiagramPlugin, DumpKind, PluginError, RenderContext, RenderMode, Rendered};

/// The sequence diagram type.
pub struct SequencePlugin;

impl DiagramPlugin for SequencePlugin {
    fn name(&self) -> &'static str {
        "sequence"
    }

    fn description(&self) -> &'static str {
        "sequence diagrams (participants, messages, notes and frames)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["seq", "seqdiag", "sdiag"]
    }

    /// A sequence diagram is recognized by its `sequence` block or by a
    /// `participant` declaration.
    fn detect(&self, source: &str) -> bool {
        for line in source.lines() {
            let line = line.trim();
            if line.starts_with("sequence") || line.starts_with("participant") {
                return true;
            }
        }
        false
    }

    /// The sequence diagram is listed after the activity diagram.
    fn priority(&self) -> u32 {
        20
    }

    /// A starter sequence diagram for the live preview.
    fn template(&self) -> &'static str {
        "sequence \"new\" {\n  participant \"Client\" as client;\n  \
         participant \"Server\";\n\n  client -> Server: \"hello\";\n  \
         Server --> client: \"hi\";\n}\n"
    }

    fn render(&self, context: &RenderContext<'_>) -> Result<Rendered, PluginError> {
        let ast = parser::parse(context.source)
            .map_err(|e| PluginError::Message(format!("failed to parse the diagram: {e}")))?;
        let document = model::build(&ast);

        if document.diagrams.is_empty() {
            return Err(PluginError::Message(
                "the file does not contain a sequence diagram".to_string(),
            ));
        }

        let mut sections = Vec::new();
        for (index, diagram) in document.diagrams.iter().enumerate() {
            let key = if index == 0 {
                diagram.id.clone()
            } else {
                format!("{}-n{}", diagram.id, index)
            };
            // In a bundle the font is embedded once, in the page CSS, and the
            // XML doctype does not belong in an HTML document.
            let mut options = context.options.clone();
            if context.mode == RenderMode::Bundle {
                options.embed_font = false;
                options.nodoctype = true;
            }
            let svg = render::render(diagram, &options);
            sections.push(Section::new(key.clone(), diagram.id.clone(), svg));
        }

        Ok(Rendered {
            sections,
            warnings: document.warnings,
            root: None,
        })
    }

    fn dump(&self, kind: DumpKind, context: &RenderContext<'_>) -> Result<String, PluginError> {
        match kind {
            DumpKind::Ast => {
                let ast = parser::parse(context.source).map_err(|e| {
                    PluginError::Message(format!("failed to parse the diagram: {e}"))
                })?;
                Ok(format!("{ast:#?}"))
            }
            DumpKind::Model => {
                let ast = parser::parse(context.source).map_err(|e| {
                    PluginError::Message(format!("failed to parse the diagram: {e}"))
                })?;
                let document = model::build(&ast);
                Ok(dump_model(&document))
            }
            DumpKind::Geometry => {
                let ast = parser::parse(context.source).map_err(|e| {
                    PluginError::Message(format!("failed to parse the diagram: {e}"))
                })?;
                let document = model::build(&ast);
                let mut text = crate::text::TextMeasurer::from_spec(&context.options.font);
                let mut out = String::new();
                for diagram in &document.diagrams {
                    let layout = layout::layout(diagram, &mut text);
                    out.push_str(&format!("diagram {}\n", diagram.id));
                    out.push_str(&format!("  size {}x{}\n", layout.width, layout.height));
                    for (index, participant) in layout.participants.iter().enumerate() {
                        out.push_str(&format!(
                            "  participant {} at {}..{} (lifeline {})\n",
                            index,
                            participant.box_.x1,
                            participant.box_.x2,
                            layout.lifelines[index].x
                        ));
                    }
                    for message in &layout.messages {
                        out.push_str(&format!(
                            "  message {} -> {} at y={} {}\n",
                            message.from,
                            message.to,
                            message.y,
                            message.kind.as_str()
                        ));
                    }
                }
                Ok(out)
            }
        }
    }
}

/// A deterministic, line-oriented summary of a resolved document.
fn dump_model(document: &model::Document) -> String {
    let mut out = String::new();
    for diagram in &document.diagrams {
        out.push_str(&format!("diagram {}\n", diagram.id));
        for (index, participant) in diagram.participants.iter().enumerate() {
            out.push_str(&format!("  participant {} {}\n", index, participant.name));
        }
        dump_items(&diagram.items, 1, &mut out);
    }
    out
}

fn dump_items(items: &[model::Item], depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    for item in items {
        match item {
            model::Item::Message(message) => out.push_str(&format!(
                "{indent}message {} -> {} {} {}\n",
                message.from,
                message.to,
                message.kind.as_str(),
                message.text
            )),
            model::Item::Note(note) => out.push_str(&format!(
                "{indent}note over {:?} {}\n",
                note.over, note.text
            )),
            model::Item::Block(block) => {
                out.push_str(&format!(
                    "{indent}block {} {:?}\n",
                    block.kind.as_str(),
                    block.label
                ));
                dump_items(&block.items, depth + 1, out);
            }
        }
    }
}

/// The rendering options a sequence diagram uses.
pub type Options = RenderOptions;
