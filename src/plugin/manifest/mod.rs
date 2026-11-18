//! The manifest diagram plugin.
//!
//! A manifest shows which *rows* (elements) belong to which *columns* (sets),
//! like a spreadsheet with a dot in every member cell. Columns can be grouped
//! under a band, and a column can `extends` another one: it inherits its
//! members (hollow dots), and may `remove` or `override` some of them. A
//! count column shows how many columns each row is in, so the shared rows
//! stand out (they are sorted first by default).
//!
//! ```text
//! manifest "Components" {
//!   group "Products" {
//!     column base { core; ui; net; }
//!     column lite [extends = base] { remove net; }
//!     column pro [extends = base] { override ui [note = "v2"]; cloud; }
//!   }
//!   row docs: base, pro;
//! }
//! ```
//!
//! The plugin knows nothing about the meaning of the rows and the columns;
//! the data comes from the source file.

pub mod ast;
pub mod layout;
pub mod model;
pub mod parser;
pub mod render;

use crate::core::Section;
use crate::text::TextMeasurer;

use super::{DiagramPlugin, DumpKind, PluginError, RenderContext, RenderMode, Rendered};

/// The script that highlights and filters a manifest in an HTML bundle.
const RUNTIME: &str = include_str!("runtime.js");

/// The styles of the highlight, the hidden marks and the controls, and the
/// toolbar shown next to the zoom controls in a bundle. Folded rows and columns
/// are handled by the runtime, not by CSS.
const TOOLBAR: &str = "<style>\
.manifest-row[data-active] > .manifest-row-body > rect:first-child, \
.manifest-cell[data-active] > rect, .manifest-column[data-active] > rect { fill: #fff3bf; }\
.manifest-column { cursor: pointer; }\
.manifest-column[data-filter] > rect { fill: #ffe066; }\
.manifest-row-label, .manifest-cell { cursor: pointer; }\
.manifest-cell[data-filter] > rect { fill: #ffe066; }\
.manifest-row:has(.manifest-row-label[data-filter]) > .manifest-row-body > rect:first-child { fill: #ffe066; }\
.manifest-legend-entry[data-hidden] { opacity: 0.3; }\
.manifest-cell[data-mark-hidden] > ellipse, .manifest-cell[data-mark-hidden] > line { display: none; }\
.manifest-toggle, .manifest-expand, .manifest-legend-entry { cursor: pointer; }\
.manifest-toggle { opacity: 0; }\
.manifest-row:hover .manifest-toggle, .manifest-column:hover .manifest-toggle { opacity: 1; }\
</style><div class=\"swimflow-toolbar manifest-toolbar\">\
<button type=\"button\" class=\"manifest-show-all\">Show all</button></div>\n";

/// The manifest diagram type.
pub struct ManifestPlugin;

impl ManifestPlugin {
    fn parse(source: &str) -> Result<ast::Document, PluginError> {
        parser::parse(source)
            .map_err(|e| PluginError::Message(format!("failed to parse the diagram: {e}")))
    }

    fn model(source: &str) -> Result<model::Document, PluginError> {
        model::build(&Self::parse(source)?).map_err(PluginError::Message)
    }
}

impl DiagramPlugin for ManifestPlugin {
    fn name(&self) -> &'static str {
        "manifest"
    }

    fn description(&self) -> &'static str {
        "manifest diagrams (which rows belong to which columns, with inheritance)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["mfst"]
    }

    /// A manifest is recognized by a `manifest` block header.
    fn detect(&self, source: &str) -> bool {
        source.lines().any(|line| {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("manifest") else {
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

    /// The manifest is listed after the treemap.
    fn priority(&self) -> u32 {
        50
    }

    fn template(&self) -> &'static str {
        "manifest \"new\" {\n  group \"Products\" {\n    column base { core; ui; }\n    \
         column extended [extends = base] { remove ui; extra; }\n  }\n  \
         column other { core; }\n}\n"
    }

    fn render(&self, context: &RenderContext<'_>) -> Result<Rendered, PluginError> {
        let document = Self::model(context.source)?;
        if document.diagrams.is_empty() {
            return Err(PluginError::Message(
                "the file does not contain a manifest diagram".to_string(),
            ));
        }

        let mut sections = Vec::new();
        for (index, diagram) in document.diagrams.iter().enumerate() {
            let id = diagram.id.clone().unwrap_or_else(|| {
                if index == 0 {
                    "default".to_string()
                } else {
                    format!("manifest{index}")
                }
            });
            let key = if index == 0 {
                id.clone()
            } else {
                format!("{id}-n{index}")
            };
            let mut options = context.options.clone();
            if context.mode == RenderMode::Bundle {
                options.embed_font = false;
                options.nodoctype = true;
            }
            // An export of the live preview leaves out what the user hid. A
            // file without a view hides the default marks (`hidemarks`); a
            // bundle draws everything and starts with those marks hidden.
            let interactive = context.mode == RenderMode::Bundle;
            let mut diagram = diagram.clone();
            let initial = match context.options.view.hidden.get(&key) {
                Some(tokens) => {
                    diagram.apply_view(tokens.iter().map(String::as_str));
                    tokens.iter().cloned().collect()
                }
                None if interactive => diagram.default_hidden(),
                None => {
                    let defaults = diagram.default_hidden();
                    diagram.apply_view(defaults.iter().map(String::as_str));
                    defaults
                }
            };
            let svg = render::render(&diagram, &options, interactive, &initial);
            let mut section = Section::new(key, id, svg);
            if context.mode == RenderMode::Bundle && !diagram.columns.is_empty() {
                section = section.with_script(RUNTIME).with_toolbar(TOOLBAR);
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
        let mut out = String::new();
        match kind {
            DumpKind::Ast => out = format!("{:#?}", Self::parse(context.source)?),
            DumpKind::Model => {
                for diagram in &document.diagrams {
                    out.push_str(&format!(
                        "manifest {}\n",
                        diagram.id.as_deref().unwrap_or("-")
                    ));
                    for column in &diagram.columns {
                        out.push_str(&format!("  column {:?}", column.name));
                        if let Some(g) = column.group {
                            out.push_str(&format!(" group {:?}", diagram.groups[g].name));
                        }
                        if let Some(p) = column.parent {
                            out.push_str(&format!(" extends {:?}", diagram.columns[p].name));
                        }
                        out.push('\n');
                    }
                    for r in diagram.row_order() {
                        let row = &diagram.rows[r];
                        let marks: Vec<&str> = diagram.cells[r]
                            .iter()
                            .map(|cell| cell.mark.name())
                            .collect();
                        out.push_str(&format!(
                            "  row {:?} count {} [{}]\n",
                            row.name,
                            row.count,
                            marks.join(" ")
                        ));
                    }
                }
            }
            DumpKind::Geometry => {
                let mut text = TextMeasurer::from_spec(&context.options.font);
                for diagram in &document.diagrams {
                    let layout = layout::layout(diagram, &mut text, false);
                    out.push_str(&format!(
                        "manifest {}\n  size {}x{}\n",
                        diagram.id.as_deref().unwrap_or("-"),
                        layout.width,
                        layout.height
                    ));
                    for (c, b) in layout.columns.iter().enumerate() {
                        out.push_str(&format!(
                            "  column c{c} {:?} at {},{} {}x{}\n",
                            diagram.columns[c].label,
                            b.x1,
                            b.y1,
                            b.width(),
                            b.height()
                        ));
                    }
                    for row in &layout.rows {
                        out.push_str(&format!(
                            "  row r{} {:?} at {},{} {}x{}\n",
                            row.row,
                            diagram.rows[row.row].label,
                            row.box_.x1,
                            row.box_.y1,
                            row.box_.width(),
                            row.box_.height()
                        ));
                    }
                }
            }
        }
        Ok(out)
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
        let context = RenderContext::new(source, &options, &load, &overlay);
        let context = RenderContext { mode, ..context };
        ManifestPlugin.render(&context).expect("render")
    }

    #[test]
    fn detection_needs_a_block_header() {
        let plugin = ManifestPlugin;
        assert!(plugin.detect("manifest \"x\" {\n}\n"));
        assert!(plugin.detect("manifest {\n}\n"));
        assert!(!plugin.detect("manifestish {\n}\n"));
        assert!(!plugin.detect("treemap { manifest: 1; }\n"));
        assert!(!plugin.detect("A -> B;\n"));
    }

    #[test]
    fn a_bundle_gets_the_runtime_and_a_file_does_not() {
        let source = "manifest { column a { x; } }";
        let bundle = render(source, RenderMode::Bundle);
        assert!(!bundle.sections[0].scripts.is_empty());
        assert!(bundle.sections[0].toolbar.contains("manifest-show-all"));
        let standalone = render(source, RenderMode::Standalone);
        assert!(standalone.sections[0].scripts.is_empty());
        assert!(standalone.sections[0].toolbar.is_empty());
    }

    #[test]
    fn several_matrices_get_stable_keys() {
        let rendered = render(
            "manifest one { column a; } manifest two { column b; }",
            RenderMode::Standalone,
        );
        assert_eq!(rendered.sections[0].key, "one");
        assert_eq!(rendered.sections[1].key, "two-n1");
    }

    #[test]
    fn the_model_dump_lists_the_marks() {
        let options = RenderOptions::default();
        let load = LoadOptions::default();
        let overlay = Overlay::new();
        let context = RenderContext::new(
            "manifest { column a { x; } column b [extends = a] { remove x; } }",
            &options,
            &load,
            &overlay,
        );
        let dump = ManifestPlugin.dump(DumpKind::Model, &context).unwrap();
        assert!(dump.contains("column \"b\" extends \"a\""), "{dump}");
        assert!(
            dump.contains("row \"x\" count 1 [direct removed]"),
            "{dump}"
        );
    }
}
