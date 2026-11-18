//! The treemap diagram plugin.
//!
//! A treemap draws a hierarchy as nested rectangles whose areas follow a
//! numbered parameter. It uses the same font handling, palette and output
//! formats as the other diagram types, all of which come from the core.
//!
//! ```text
//! treemap "Budget" {
//!   root "Monthly budget";
//!
//!   Housing: 1500 {
//!     Rent: 1000;
//!     Utilities: 500;
//!   }
//!   Food: 600;
//!   Savings: 400;
//! }
//! ```
//!
//! The whole treemap is drawn as one SVG. In an HTML bundle a small script
//! (`runtime.js`) hides and shows the values.

pub mod ast;
pub mod layout;
pub mod model;
pub mod parser;
pub mod render;

use crate::core::Section;

use super::{DiagramPlugin, DumpKind, PluginError, RenderContext, RenderMode, Rendered};

/// The script that hides and shows the values in an HTML bundle.
const RUNTIME: &str = include_str!("runtime.js");

/// The button in the page bar of an HTML bundle that toggles the values. The
/// runtime gives it its behavior; a file or an image has no button.
const VALUE_TOGGLE: &str = "<div class=\"swimflow-toolbar\">\
<button type=\"button\" class=\"treemap-values-toggle\" aria-pressed=\"true\">\
Hide values</button></div>\n";

/// The treemap diagram type.
pub struct TreemapPlugin;

impl TreemapPlugin {
    fn parse(source: &str) -> Result<ast::Document, PluginError> {
        parser::parse(source)
            .map_err(|e| PluginError::Message(format!("failed to parse the diagram: {e}")))
    }

    fn model(source: &str) -> Result<model::Document, PluginError> {
        model::build(&Self::parse(source)?).map_err(PluginError::Message)
    }
}

impl DiagramPlugin for TreemapPlugin {
    fn name(&self) -> &'static str {
        "treemap"
    }

    fn description(&self) -> &'static str {
        "treemap diagrams (nested rectangles sized by a number)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["tmap", "treemap"]
    }

    /// A treemap is recognized by a `treemap` block header: a line that starts
    /// with `treemap` and ends with `{` (or names the block in quotes).
    fn detect(&self, source: &str) -> bool {
        source.lines().any(|line| {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("treemap") else {
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

    /// The treemap is listed after the tree diagram.
    fn priority(&self) -> u32 {
        40
    }

    /// A starter treemap for the live preview.
    fn template(&self) -> &'static str {
        "treemap \"new\" {\n  root \"Root\";\n\n  \"First\": 60;\n  \
         \"Second\": 30 {\n    \"Part A\": 20;\n    \"Part B\": 10;\n  }\n  \
         \"Third\": 10;\n}\n"
    }

    fn render(&self, context: &RenderContext<'_>) -> Result<Rendered, PluginError> {
        let document = Self::model(context.source)?;
        if document.diagrams.is_empty() {
            return Err(PluginError::Message(
                "the file does not contain a treemap diagram".to_string(),
            ));
        }

        let mut sections = Vec::new();
        for (index, diagram) in document.diagrams.iter().enumerate() {
            let id = diagram.id.clone().unwrap_or_else(|| {
                if index == 0 {
                    "default".to_string()
                } else {
                    format!("treemap{index}")
                }
            });
            let key = if index == 0 {
                id.clone()
            } else {
                format!("{id}-n{index}")
            };

            // In a bundle the font is embedded once, in the page CSS, and the
            // XML doctype does not belong in an HTML document.
            let mut options = context.options.clone();
            if context.mode == RenderMode::Bundle {
                options.embed_font = false;
                options.nodoctype = true;
            }
            let svg = render::render(diagram, &options);
            let mut section = Section::new(key, id, svg);
            if context.mode == RenderMode::Bundle && section.svg.contains("class=\"treemap-value\"")
            {
                section = section.with_script(RUNTIME).with_toolbar(VALUE_TOGGLE);
            }
            sections.push(section);
        }

        Ok(Rendered {
            sections,
            warnings: Vec::new(),
            root: None,
        })
    }

    fn dump(&self, kind: DumpKind, context: &RenderContext<'_>) -> Result<String, PluginError> {
        let document = Self::model(context.source)?;
        match kind {
            DumpKind::Ast => {
                let ast = Self::parse(context.source)?;
                Ok(format!("{ast:#?}"))
            }
            DumpKind::Model => Ok(dump_model(&document)),
            DumpKind::Geometry => {
                let mut out = String::new();
                for diagram in &document.diagrams {
                    let layout = layout::layout(diagram);
                    out.push_str(&format!(
                        "treemap {}\n",
                        diagram.id.as_deref().unwrap_or("-")
                    ));
                    out.push_str(&format!("  size {}x{}\n", layout.width, layout.height));
                    for (index, node) in layout.nodes.iter().enumerate() {
                        out.push_str(&format!(
                            "  node n{} {:?} value {} depth {} at {},{} {}x{}\n",
                            index,
                            node.label,
                            node.value,
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

fn dump_model(document: &model::Document) -> String {
    let mut out = String::new();
    for diagram in &document.diagrams {
        out.push_str(&format!(
            "treemap {}\n  root {:?} value {}\n",
            diagram.id.as_deref().unwrap_or("-"),
            diagram.label,
            diagram.weight
        ));
        dump_nodes(&diagram.children, 1, &mut out);
    }
    out
}

fn dump_nodes(nodes: &[model::Node], depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    for node in nodes {
        out.push_str(&format!(
            "{indent}node {:?} value {}\n",
            node.label, node.weight
        ));
        dump_nodes(&node.children, depth + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::LoadOptions;
    use crate::core::overlay::Overlay;
    use crate::render::RenderOptions;

    fn render(source: &str, mode: RenderMode) -> Rendered {
        let options = RenderOptions::default();
        let load = LoadOptions::default();
        let overlay = Overlay::new();
        let context = RenderContext {
            source,
            path: None,
            options: &options,
            mode,
            load: &load,
            pixels: false,
            overlay: &overlay,
        };
        TreemapPlugin.render(&context).expect("render")
    }

    #[test]
    fn detection_needs_a_block_header() {
        let plugin = TreemapPlugin;
        assert!(plugin.detect("treemap \"x\" {\n}\n"));
        assert!(plugin.detect("// c\ntreemap {\n}\n"));
        assert!(plugin.detect("treemap demo {\n}\n"));
        assert!(!plugin.detect("treemap: 1;\n"));
        assert!(!plugin.detect("treemapish {\n}\n"));
        assert!(!plugin.detect("A -> B;\n"));
    }

    #[test]
    fn a_bundle_gets_the_value_toggle_and_a_file_does_not() {
        let source = "treemap { A: 2; }";
        let bundle = render(source, RenderMode::Bundle);
        assert!(bundle.sections[0].svg.contains("treemap-value"));
        assert!(bundle.sections[0].toolbar.contains("treemap-values-toggle"));
        assert!(!bundle.sections[0].scripts.is_empty());

        let standalone = render(source, RenderMode::Standalone);
        assert!(standalone.sections[0].toolbar.is_empty());
        assert!(standalone.sections[0].scripts.is_empty());
    }

    #[test]
    fn a_treemap_without_values_needs_no_toggle() {
        let rendered = render("treemap { showvalue = false; A: 2; }", RenderMode::Bundle);
        assert!(rendered.sections[0].toolbar.is_empty());
        assert!(rendered.sections[0].scripts.is_empty());
    }

    #[test]
    fn several_treemaps_get_stable_keys() {
        let rendered = render(
            "treemap one { A: 1; } treemap two { B: 2; }",
            RenderMode::Standalone,
        );
        assert_eq!(rendered.sections.len(), 2);
        assert_eq!(rendered.sections[0].key, "one");
        assert_eq!(rendered.sections[1].key, "two-n1");
    }
}
