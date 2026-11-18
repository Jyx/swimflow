//! Interactive HTML bundle: every loaded diagram inlined into a single,
//! self-contained file with a small dependency-free navigation runtime.
//!
//! Design:
//!
//! * all diagrams are rendered once, each into its own `<section>` with an
//!   `id` equal to its bundle key;
//! * `ref` nodes become `<a href="#key" data-target="key">` and `href` nodes
//!   stay external links (optionally gated by `--no-external`);
//! * the runtime shows one section at a time, animates the transition, keeps a
//!   breadcrumb and history stack, and routes deep links (`out.html#detail`)
//!   through the URL hash;
//! * without JavaScript the CSS `:target` rules still reveal the linked
//!   diagram, so the bundle degrades to plain anchor links.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

use crate::build::link::Link;
use crate::build::model::{Diagram, DocumentSet};
use crate::core::html::{self, Section};
use crate::render::svg::{NodeHref, render_diagram_with_text};
use crate::render::{RenderError, RenderOptions, Renderer};
use crate::text::TextMeasurer;

/// Renders a [`DocumentSet`] as one self-contained HTML page.
pub struct BundleRenderer;

impl Renderer for BundleRenderer {
    fn render(
        &self,
        set: &DocumentSet,
        out: &mut dyn Write,
        options: &RenderOptions,
    ) -> Result<(), RenderError> {
        let html = render(set, options)?;
        out.write_all(html.as_bytes())?;
        Ok(())
    }
}

/// Build the bundle HTML for `set`. The root diagram is taken from
/// `options.bundle_root`, falling back to the first diagram of the root
/// document.
pub fn render(set: &DocumentSet, options: &RenderOptions) -> Result<String, RenderError> {
    render_with_font_url(set, options, None)
}

/// Like [`render`], but when the font is not embedded, reference `font_url`
/// in an `@font-face` rule instead of inlining it.
///
/// The live-preview server uses this to serve the (multi-megabyte) font once
/// as a static asset rather than embedding it in every re-render.
pub fn render_with_font_url(
    set: &DocumentSet,
    options: &RenderOptions,
    font_url: Option<&str>,
) -> Result<String, RenderError> {
    let (sections, root) = sections(set, options)?;
    let font_face = html::font_face_css(&options.font, options.embed_font, font_url);
    Ok(html::bundle(&sections, &root, &font_face))
}

/// Render every diagram of the document set into a bundle section.
///
/// The sections are the reusable unit of the HTML bundle: the key is the
/// deep-link fragment, the label is shown in the breadcrumb and the SVG is
/// the diagram itself (with its drill-down links already resolved).
pub fn sections(
    set: &DocumentSet,
    options: &RenderOptions,
) -> Result<(Vec<Section>, String), RenderError> {
    if set.documents.iter().all(|doc| doc.diagrams.is_empty()) {
        return Err(RenderError::Empty);
    }

    let keys = DiagramKeys::new(set);
    let root = resolve_root(set, &keys, options.bundle_root.as_deref())?;

    let mut sections = Vec::new();
    // One measurer for every diagram: it owns the (embedded) font database.
    let mut measurer = TextMeasurer::from_spec(&options.font);
    for (doc_index, document) in set.documents.iter().enumerate() {
        for diagram in &document.diagrams {
            let Some(key) = keys.get(doc_index, &diagram.id) else {
                continue;
            };
            let links = build_links(&keys, diagram, options);
            let mut svg_options = options.clone();
            // The doctype is XML-only and the font is embedded once, in the
            // page CSS, not per diagram.
            svg_options.nodoctype = true;
            svg_options.embed_font = false;
            let svg = render_diagram_with_text(diagram, &svg_options, &links, &mut measurer);
            sections.push(Section::new(key, diagram.id.clone(), svg));
        }
    }

    Ok((sections, root))
}

/// A mapping from `(document index, diagram id)` to a page-unique HTML key.
///
/// Diagrams in the root document keep their plain id so deep links like
/// `out.html#detail` work; other documents are prefixed with their file stem
/// only when a collision would occur.
struct DiagramKeys {
    map: BTreeMap<(usize, String), String>,
}

impl DiagramKeys {
    fn new(set: &DocumentSet) -> Self {
        let mut map = BTreeMap::new();
        let mut used: BTreeSet<String> = BTreeSet::new();

        if let Some(root) = set.documents.get(set.root) {
            for diagram in &root.diagrams {
                used.insert(diagram.id.clone());
                map.insert((set.root, diagram.id.clone()), diagram.id.clone());
            }
        }

        for (doc_index, document) in set.documents.iter().enumerate() {
            if doc_index == set.root {
                continue;
            }
            let stem = document
                .source
                .as_ref()
                .and_then(|path| path.file_stem())
                .and_then(|stem| stem.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("doc{doc_index}"));
            for diagram in &document.diagrams {
                let mut key = diagram.id.clone();
                if used.contains(&key) {
                    key = format!("{stem}-{}", diagram.id);
                }
                let mut suffix = 2;
                while used.contains(&key) {
                    key = format!("{stem}-{}-{suffix}", diagram.id);
                    suffix += 1;
                }
                used.insert(key.clone());
                map.insert((doc_index, diagram.id.clone()), key);
            }
        }

        DiagramKeys { map }
    }

    fn get(&self, document: usize, diagram: &str) -> Option<&str> {
        self.map
            .get(&(document, diagram.to_string()))
            .map(String::as_str)
    }
}

/// Resolve the entry diagram to a bundle key.
fn resolve_root(
    set: &DocumentSet,
    keys: &DiagramKeys,
    requested: Option<&str>,
) -> Result<String, RenderError> {
    if let Some(id) = requested {
        if set.documents[set.root].index.contains_key(id)
            && let Some(key) = keys.get(set.root, id)
        {
            return Ok(key.to_string());
        }
        for (doc_index, document) in set.documents.iter().enumerate() {
            if document.index.contains_key(id)
                && let Some(key) = keys.get(doc_index, id)
            {
                return Ok(key.to_string());
            }
        }
        return Err(RenderError::Bundle(format!("unknown root diagram: {id:?}")));
    }

    set.documents
        .get(set.root)
        .and_then(|document| document.diagrams.first())
        .and_then(|diagram| keys.get(set.root, &diagram.id))
        .map(str::to_string)
        .ok_or(RenderError::Empty)
}

/// Map each node to its bundle hyperlink.
fn build_links(
    keys: &DiagramKeys,
    diagram: &Diagram,
    options: &RenderOptions,
) -> Vec<Option<NodeHref>> {
    diagram
        .nodes
        .iter()
        .map(|node| match &node.link {
            None => None,
            Some(Link::External(url)) => {
                if options.no_external {
                    Some(NodeHref::disabled())
                } else {
                    Some(NodeHref {
                        href: Some(url.clone()),
                        data_target: None,
                        aria_label: Some(format!("{}: {url}", node.label)),
                    })
                }
            }
            Some(Link::Diagram(_)) => {
                let Some(resolved) = &node.resolved else {
                    return Some(NodeHref::disabled());
                };
                let Some(key) = keys.get(resolved.document, &resolved.diagram) else {
                    return Some(NodeHref::disabled());
                };
                Some(NodeHref {
                    href: Some(format!("#{key}")),
                    data_target: Some(key.to_string()),
                    aria_label: Some(format!(
                        "{}: go to diagram {}",
                        node.label, resolved.diagram
                    )),
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build;

    fn bundle(src: &str, options: &RenderOptions) -> String {
        let set = build::build_str(src).expect("build");
        render(&set, options).expect("bundle")
    }

    #[test]
    fn bundle_inlines_diagrams_and_links() {
        let html = bundle(
            "diagram main { A [ref = \"#detail\"]; } diagram detail { B -> C; }",
            &RenderOptions::default(),
        );
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("id=\"main\""));
        assert!(html.contains("id=\"detail\""));
        assert!(html.contains("data-target=\"detail\""));
        assert!(html.contains("href=\"#detail\""));
        assert!(html.contains("data-root=\"main\""));
        assert!(html.contains("<svg"));
        // No external resource references; the only `http://` occurrences are
        // XML namespace identifiers.
        assert!(!html.contains("href=\"http"));
        assert!(!html.contains("src=\"http"));
        assert!(!html.contains("<image"));
    }

    #[test]
    fn explicit_root_is_honoured() {
        let options = RenderOptions {
            bundle_root: Some("detail".to_string()),
            ..RenderOptions::default()
        };
        let html = bundle("diagram main { A; } diagram detail { B; }", &options);
        assert!(html.contains("data-root=\"detail\""));
        assert!(
            !html.contains("data-root=\"main\""),
            "only the root is marked"
        );
    }

    #[test]
    fn unknown_root_is_an_error() {
        let options = RenderOptions {
            bundle_root: Some("nope".to_string()),
            ..RenderOptions::default()
        };
        let set = build::build_str("diagram main { A; } ").expect("build");
        assert!(matches!(
            render(&set, &options),
            Err(RenderError::Bundle(_))
        ));
    }

    #[test]
    fn no_external_disables_external_links() {
        let src = "{ A [href = \"https://example.com\"]; }";
        let linked = bundle(src, &RenderOptions::default());
        assert!(linked.contains("https://example.com"));

        let gated = bundle(
            src,
            &RenderOptions {
                no_external: true,
                ..RenderOptions::default()
            },
        );
        assert!(!gated.contains("href=\"https://example.com\""));
        assert!(gated.contains("data-link-disabled=\"true\""));
    }

    #[test]
    fn broken_links_render_inert_when_allowed() {
        let options = RenderOptions::default();
        let set = {
            let source = "diagram main { A [ref = \"#missing\"]; }";
            let ast = crate::parser::parse(source).unwrap();
            let document = build::build(&ast, None).unwrap();
            crate::build::model::DocumentSet {
                documents: vec![document],
                root: 0,
                issues: vec![],
            }
        };
        let html = render(&set, &options).expect("bundle tolerates unresolved refs");
        assert!(html.contains("data-link-disabled=\"true\""));
    }

    #[test]
    fn runtime_contains_navigation_features() {
        let html = bundle(
            "diagram a { X [ref = \"#b\"]; } diagram b { Y [ref = \"#a\"]; }",
            &RenderOptions::default(),
        );
        assert!(html.contains("popstate"));
        assert!(html.contains("hashchange"));
        assert!(html.contains("Escape"));
        assert!(html.contains("aria-current"));
    }
}
