//! The activity (swimlane) diagram plugin.
//!
//! This is the diagram type swimflow has always supported: it reuses the
//! reference-compatible parser, the arena IR, the integer-grid layout, the
//! pixel metrics and the SVG renderer. The plugin is a thin adapter that
//! presents the existing pipeline through the [`DiagramPlugin`] interface, so
//! the activity diagram and every other diagram type are handled the same way
//! by the CLI and by the core.

use crate::build::{self, model::DocumentSet};
use crate::core::Section;
use crate::render::bundle;
use crate::render::svg::render_diagram;

use super::{DiagramPlugin, DumpKind, PluginError, RenderContext, RenderMode, Rendered};

/// The activity/swimlane diagram type.
pub struct ActivityPlugin;

impl DiagramPlugin for ActivityPlugin {
    fn name(&self) -> &'static str {
        "activity"
    }

    fn description(&self) -> &'static str {
        "activity/swimlane diagrams (reference compatible)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["diag", "actdiag"]
    }

    /// The activity grammar is the fallback: any file that does not look like
    /// a sequence diagram is offered to this plugin.
    fn detect(&self, _source: &str) -> bool {
        true
    }

    /// The activity diagram is the primary type; it is listed first.
    fn priority(&self) -> u32 {
        10
    }

    /// A starter diagram for the live preview.
    fn template(&self) -> &'static str {
        "diagram \"new\" {\n  lane \"Start\" {\n    Request;\n  }\n  \
         lane \"Work\" {\n    Review -> Approve;\n  }\n  Request -> Review;\n}\n"
    }

    fn render(&self, context: &RenderContext<'_>) -> Result<Rendered, PluginError> {
        let set = load(context)?;
        let mut warnings = Vec::new();
        for document in &set.documents {
            warnings.extend(document.warnings.iter().cloned());
        }
        // Broken links are tolerated with `--allow-broken-links`; report each
        // one so the node that was drawn disabled is not a silent surprise.
        for issue in &set.issues {
            warnings.push(format!(
                "link in {}: node {:?}: {}",
                issue.file, issue.node, issue.message
            ));
        }

        let (sections, root) = match context.mode {
            RenderMode::Bundle => {
                let (sections, root) = bundle::sections(&set, context.options)
                    .map_err(|e| PluginError::Message(e.to_string()))?;
                (sections, Some(root))
            }
            RenderMode::Standalone => {
                let sections = standalone_sections(&set, context.options);
                let root = sections.first().map(|section| section.key.clone());
                (sections, root)
            }
        };

        Ok(Rendered {
            sections,
            warnings,
            root,
        })
    }

    fn dump(&self, kind: DumpKind, context: &RenderContext<'_>) -> Result<String, PluginError> {
        match kind {
            DumpKind::Ast => {
                let document = crate::parser::parse(context.source)
                    .map_err(|e| PluginError::Message(e.to_string()))?;
                Ok(format!("{document:#?}"))
            }
            DumpKind::Model => {
                let set = load(context)?;
                Ok(build::dump(&set))
            }
            DumpKind::Geometry => {
                let set = load(context)?;
                if context.pixels {
                    Ok(crate::metrics::dump_pixels(&set))
                } else {
                    Ok(build::dump_geometry(&set))
                }
            }
        }
    }
}

/// Render the diagrams of the root document as standalone sections.
///
/// The first section is the diagram a single-file output (SVG/PNG/PDF)
/// shows, exactly like the previous, non-plugin code path did.
fn standalone_sections(set: &DocumentSet, options: &crate::render::RenderOptions) -> Vec<Section> {
    set.root()
        .diagrams
        .iter()
        .map(|diagram| {
            Section::new(
                diagram.id.clone(),
                diagram.id.clone(),
                render_diagram(diagram, options),
            )
        })
        .collect()
}

/// Load the document set of the input file.
///
/// A file on disk is loaded like before (so `ref = "other.diag"` still
/// resolves); standard input is parsed on its own. Files that only exist in
/// the browser session are read from the overlay.
fn load(context: &RenderContext<'_>) -> Result<DocumentSet, PluginError> {
    match context.path {
        Some(path) => {
            build::load_str_with_overlay(context.source, path, context.overlay, context.load)
                .map_err(|e| PluginError::Message(e.to_string()))
        }
        None => build::build_str_with_options(context.source, context.load)
            .map_err(|e| PluginError::Message(e.to_string())),
    }
}
