//! The tree diagram plugin.
//!
//! A tree diagram draws a hierarchy: a root, its children, their children and
//! so on, as boxes joined by connectors. It uses the same font handling, the
//! same palette and the same output formats as the other diagram types, all
//! of which come from the core.
//!
//! ```text
//! tree "Product" {
//!   root "Swimflow";
//!
//!   Core {
//!     Parser;
//!     Renderer { SVG; PNG; PDF; }
//!   }
//!   Docs [color = "#cfe8ff"];
//! }
//! ```
//!
//! The whole tree is drawn as one SVG. In an HTML bundle a small script
//! (`runtime.js`) folds and unfolds the branches in place.

pub mod ast;
pub mod layout;
pub mod parser;
pub mod render;

use std::collections::BTreeMap;

use crate::core::Section;

use super::{DiagramPlugin, DumpKind, PluginError, RenderContext, RenderMode, Rendered};

/// The script that folds and unfolds the branches of a tree in an HTML bundle.
const RUNTIME: &str = include_str!("runtime.js");

/// The buttons in the page bar of an HTML bundle that fold or unfold every
/// branch. The runtime gives them their behavior; a file or an image has no
/// buttons.
const FOLD_ALL: &str = "<div class=\"swimflow-toolbar\">\
<button type=\"button\" class=\"tree-collapse-all\">Collapse all</button>\
<button type=\"button\" class=\"tree-expand-all\">Expand all</button>\
</div>\n";

/// The tree diagram type.
pub struct TreePlugin;

impl TreePlugin {
    fn parse(source: &str) -> Result<ast::Document, PluginError> {
        parser::parse(source)
            .map_err(|e| PluginError::Message(format!("failed to parse the diagram: {e}")))
    }
}

impl DiagramPlugin for TreePlugin {
    fn name(&self) -> &'static str {
        "tree"
    }

    fn description(&self) -> &'static str {
        "tree diagrams (a root and its nested children)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["tree", "treediag"]
    }

    /// A tree diagram is recognized by a `tree` block header: a line that
    /// starts with `tree` and ends with `{` (or names the tree in quotes).
    fn detect(&self, source: &str) -> bool {
        source.lines().any(|line| {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("tree") else {
                return false;
            };
            let boundary = rest
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace() || matches!(c, '{' | '"' | '\''));
            boundary
                && !line.contains("->")
                && (line.ends_with('{') || rest.trim_start().starts_with(['"', '\'']))
        })
    }

    /// The tree diagram is listed after the sequence diagram.
    fn priority(&self) -> u32 {
        30
    }

    /// A starter tree diagram for the live preview.
    fn template(&self) -> &'static str {
        "tree \"new\" {\n  root \"Root\";\n\n  \"Branch\" {\n    \"Leaf\";\n    \
         \"Another leaf\";\n  }\n  \"Other branch\";\n}\n"
    }

    fn render(&self, context: &RenderContext<'_>) -> Result<Rendered, PluginError> {
        let document = Self::parse(context.source)?;
        if document.trees.is_empty() {
            return Err(PluginError::Message(
                "the file does not contain a tree diagram".to_string(),
            ));
        }

        // The section key of every tree, so a `ref` can point at one.
        let mut ids = Vec::new();
        let mut links = render::Links {
            navigable: context.mode == RenderMode::Bundle,
            keys: BTreeMap::new(),
        };
        for (index, tree) in document.trees.iter().enumerate() {
            let id = tree.id.clone().unwrap_or_else(|| {
                if index == 0 {
                    "default".to_string()
                } else {
                    format!("tree{index}")
                }
            });
            let key = if index == 0 {
                id.clone()
            } else {
                format!("{id}-n{index}")
            };
            links.keys.entry(id.clone()).or_insert_with(|| key.clone());
            ids.push((id, key));
        }

        let mut sections = Vec::new();
        let mut warnings = Vec::new();
        for (tree, (id, key)) in document.trees.iter().zip(ids) {
            warn_about_refs(&id, tree, &links, &mut warnings);

            // In a bundle the font is embedded once, in the page CSS, and the
            // XML doctype does not belong in an HTML document.
            let mut options = context.options.clone();
            if context.mode == RenderMode::Bundle {
                options.embed_font = false;
                options.nodoctype = true;
            }
            // An export of the live preview carries the folds the user made.
            let folded = context.options.view.collapsed.get(&key);
            let svg = render::render_with_links(tree, &options, &links, folded);
            let mut section = Section::new(key, id, svg).with_script(RUNTIME);
            // A tree whose nodes have no children has nothing to fold.
            if context.mode == RenderMode::Bundle && section.svg.contains("class=\"tree-toggle\"") {
                section = section.with_toolbar(FOLD_ALL);
            }
            sections.push(section);
        }

        Ok(Rendered {
            sections,
            warnings,
            root: None,
        })
    }

    fn dump(&self, kind: DumpKind, context: &RenderContext<'_>) -> Result<String, PluginError> {
        let document = Self::parse(context.source)?;
        match kind {
            DumpKind::Ast => Ok(format!("{document:#?}")),
            DumpKind::Model => {
                let mut out = String::new();
                for tree in &document.trees {
                    out.push_str(&format!("tree {}\n", tree.id.as_deref().unwrap_or("-")));
                    out.push_str(&format!(
                        "  root {}\n",
                        tree.root.name.as_deref().unwrap_or("-")
                    ));
                    dump_nodes(&tree.children, 1, &mut out);
                }
                Ok(out)
            }
            DumpKind::Geometry => {
                let mut text = crate::text::TextMeasurer::from_spec(&context.options.font);
                let mut out = String::new();
                for tree in &document.trees {
                    let layout = layout::layout(tree, &mut text);
                    out.push_str(&format!("tree {}\n", tree.id.as_deref().unwrap_or("-")));
                    out.push_str(&format!("  size {}x{}\n", layout.width, layout.height));
                    for node in &layout.nodes {
                        out.push_str(&format!(
                            "  node {:?} depth {} at {},{} {}x{}\n",
                            node.label,
                            node.depth,
                            node.box_.x1,
                            node.box_.y1,
                            node.box_.width(),
                            node.box_.height()
                        ));
                    }
                }
                Ok(out)
            }
        }
    }
}

fn dump_nodes(nodes: &[ast::Node], depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    for node in nodes {
        out.push_str(&format!("{indent}node {}\n", node.name));
        dump_nodes(&node.children, depth + 1, out);
    }
}

/// Warn about the `ref` attributes that do not lead anywhere.
fn warn_about_refs(id: &str, tree: &ast::Tree, links: &render::Links, warnings: &mut Vec<String>) {
    fn check(
        id: &str,
        name: &str,
        attrs: &[ast::Attr],
        links: &render::Links,
        warnings: &mut Vec<String>,
    ) {
        let Some(value) = ast::attr_value(attrs, "ref") else {
            return;
        };
        match links.resolve(value) {
            render::RefTarget::Section(_) => {}
            render::RefTarget::Unknown(target) => warnings.push(format!(
                "tree {id}: `ref` on {name:?} points to the unknown tree {target:?}"
            )),
            render::RefTarget::OtherFile => warnings.push(format!(
                "tree {id}: `ref` on {name:?} points to another file, which is not supported yet"
            )),
        }
    }
    fn walk(id: &str, nodes: &[ast::Node], links: &render::Links, warnings: &mut Vec<String>) {
        for node in nodes {
            check(id, &node.name, &node.attrs, links, warnings);
            walk(id, &node.children, links, warnings);
        }
    }
    check(id, "root", &tree.root.attrs, links, warnings);
    walk(id, &tree.children, links, warnings);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_needs_a_block_header() {
        let plugin = TreePlugin;
        assert!(plugin.detect("tree \"x\" {\n}\n"));
        assert!(plugin.detect("// c\ntree {\n}\n"));
        assert!(plugin.detect("tree demo {\n}\n"));
        assert!(!plugin.detect("tree -> B;\n"));
        assert!(!plugin.detect("tree [color = red];\n"));
        assert!(!plugin.detect("treeish {\n}\n"));
        assert!(!plugin.detect("A -> B;\n"));
    }
}
