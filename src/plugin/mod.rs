//! The diagram-type plugin architecture.
//!
//! A *plugin* implements one diagram type. It owns the front end (how the
//! source is read) and the drawing (how the diagram looks), and it produces
//! SVG. Everything else — fonts, PNG/PDF conversion, the interactive HTML
//! bundle, the CLI — belongs to the core and is shared by every plugin.
//!
//! The activity/swimlane diagram is implemented by [`activity`], the sequence
//! diagram by [`sequence`], the tree by [`tree`], the treemap by [`treemap`] and the manifest by
//! [`manifest`].
//! All are built-in, but the trait is deliberately small so that another type
//! (a state machine, a class diagram, …) is a self-contained module plus one
//! line in [`Registry::builtin`].
//!
//! ```text
//!   source text ──▶ plugin (parse + lay out + draw) ──▶ Section (SVG)
//!                                                            │
//!                       core: PNG / PDF / HTML bundle ◀───────┘
//! ```
//!
//! Note the distinction from [`crate::plugins`]: those are activity *feature*
//! plugins (`autolane`), which extend the activity diagram itself. The
//! modules here are *diagram-type* plugins.

pub mod activity;
pub mod manifest;
pub mod sequence;
pub mod tree;
pub mod treemap;

use std::path::Path;

use crate::build::LoadOptions;
use crate::core::Section;
use crate::core::overlay::Overlay;
use crate::render::{Format, RenderError, RenderOptions};

/// How the rendered diagrams are going to be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// One standalone document (SVG, PNG or PDF).
    Standalone,
    /// Several diagrams in one interactive HTML bundle.
    Bundle,
}

/// The kind of debug dump requested on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DumpKind {
    /// The syntax tree.
    Ast,
    /// The resolved model.
    Model,
    /// The computed geometry.
    Geometry,
}

/// Errors raised by a plugin.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// A human-readable problem in the source file.
    #[error("{0}")]
    Message(String),

    /// The requested diagram type does not exist.
    #[error("unknown diagram type {0:?} (available: {1})")]
    UnknownType(String, String),

    /// The diagram type could not be determined from the file or its content.
    #[error(
        "cannot determine the diagram type of {path}; use --type to pick one \
            (available: {available})"
    )]
    UnknownDiagramType { path: String, available: String },
}

/// What a plugin needs in order to render.
pub struct RenderContext<'a> {
    /// The source text of the diagram.
    pub source: &'a str,
    /// Where the source came from, if it came from a file.
    pub path: Option<&'a Path>,
    /// Rendering options (font, `--embed-font`, …).
    pub options: &'a RenderOptions,
    /// Whether the output is a standalone document or an HTML bundle.
    pub mode: RenderMode,
    /// How referenced files are loaded.
    pub load: &'a LoadOptions,
    /// Whether `--dump-geometry --pixels` was requested.
    pub pixels: bool,
    /// Files that exist in memory only (the browser session). A plugin that
    /// resolves references to other files reads them from here.
    pub overlay: &'a Overlay,
}

impl<'a> RenderContext<'a> {
    /// A standalone context with the given overlay and load options.
    pub fn new(
        source: &'a str,
        options: &'a RenderOptions,
        load: &'a LoadOptions,
        overlay: &'a Overlay,
    ) -> RenderContext<'a> {
        RenderContext {
            source,
            path: None,
            options,
            mode: RenderMode::Standalone,
            load,
            pixels: false,
            overlay,
        }
    }
}

/// A diagram type as the outside world sees it: a name, a description and the
/// file extensions it claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeInfo {
    /// The stable name (`--type`, `--list-types`).
    pub name: &'static str,
    /// A one-line description.
    pub description: &'static str,
    /// The file extensions the type claims, without the dot. The first one is
    /// the *source* extension: the format the type is written in.
    pub extensions: &'static [&'static str],
}

impl TypeInfo {
    /// The extension the type is written in (the first one it claims).
    pub fn source_extension(&self) -> &'static str {
        self.extensions.first().copied().unwrap_or("txt")
    }

    /// Whether the type claims `extension` (without the dot).
    pub fn claims(&self, extension: &str) -> bool {
        self.extensions
            .iter()
            .any(|claimed| claimed.eq_ignore_ascii_case(extension))
    }
}

/// The result of rendering a document with a plugin.
#[derive(Debug, Clone, Default)]
pub struct Rendered {
    /// The rendered diagrams, in definition order.
    pub sections: Vec<Section>,
    /// Non-fatal problems found while rendering.
    pub warnings: Vec<String>,
    /// The key of the diagram an HTML bundle opens with. `None` means "the
    /// first section".
    pub root: Option<String>,
}

impl Rendered {
    /// The first section, i.e. the diagram a standalone output shows.
    pub fn first(&self) -> Option<&Section> {
        self.sections.first()
    }

    /// Make the section `key` the one that is shown: the one a standalone
    /// output contains and the one an HTML bundle opens with. An unknown key
    /// changes nothing.
    pub fn show_first(&mut self, key: &str) {
        if let Some(index) = self.sections.iter().position(|section| section.key == key) {
            let section = self.sections.remove(index);
            self.sections.insert(0, section);
            self.root = Some(key.to_string());
        }
    }
}

/// One diagram type.
///
/// The trait is intentionally small: a plugin reads a source file, draws it
/// and hands the SVG to the core. It does not know about output formats.
pub trait DiagramPlugin: Send + Sync {
    /// Stable name, used by `--type` and by `--list-types`.
    fn name(&self) -> &'static str;

    /// One-line description, shown by `--list-types`.
    fn description(&self) -> &'static str;

    /// File extensions this plugin claims (without the leading dot).
    fn extensions(&self) -> &'static [&'static str];

    /// Whether the source looks like this diagram type. Used to pick a
    /// plugin for a file with an unknown extension (or for standard input).
    fn detect(&self, source: &str) -> bool;

    /// The order the types are shown in (lower comes first).
    ///
    /// The order the types are *detected* in is the registration order; this
    /// one is only about how the user interface lists them.
    fn priority(&self) -> u32 {
        100
    }

    /// A starter template for a new file of this type.
    ///
    /// The live preview offers "New" in the editor; the template is what it
    /// starts from.
    fn template(&self) -> &'static str {
        ""
    }

    /// Parse, lay out and draw the source.
    fn render(&self, context: &RenderContext<'_>) -> Result<Rendered, PluginError>;

    /// Produce a debug dump (`--dump-ast`, `--dump-model`, `--dump-geometry`).
    ///
    /// Diagram types that do not support a given dump report an error.
    fn dump(&self, kind: DumpKind, context: &RenderContext<'_>) -> Result<String, PluginError> {
        let _ = (kind, context);
        Err(PluginError::Message(format!(
            "the {} diagram type does not support debug dumps",
            self.name()
        )))
    }
}

/// What the caller knows about the input before a plugin is chosen.
#[derive(Debug, Clone)]
pub struct PluginRequest<'a> {
    /// The type explicitly requested with `--type`.
    pub type_name: Option<&'a str>,
    /// The path of the input file, if any.
    pub path: Option<&'a Path>,
    /// The source text (used for content sniffing).
    pub source: &'a str,
}

/// The set of known diagram types.
pub struct Registry {
    plugins: Vec<Box<dyn DiagramPlugin>>,
}

impl Registry {
    /// A registry with the built-in diagram types.
    pub fn builtin() -> Registry {
        Registry {
            plugins: vec![
                Box::new(sequence::SequencePlugin),
                Box::new(tree::TreePlugin),
                Box::new(treemap::TreemapPlugin),
                Box::new(manifest::ManifestPlugin),
                Box::new(activity::ActivityPlugin),
            ],
        }
    }

    /// A registry with a custom set of plugins.
    pub fn new(plugins: Vec<Box<dyn DiagramPlugin>>) -> Registry {
        Registry { plugins }
    }

    /// All registered plugins, in resolution order.
    pub fn plugins(&self) -> &[Box<dyn DiagramPlugin>] {
        &self.plugins
    }

    /// The names of all registered plugins.
    pub fn names(&self) -> Vec<&'static str> {
        self.plugins.iter().map(|plugin| plugin.name()).collect()
    }

    /// The known types, in display order.
    pub fn types(&self) -> Vec<TypeInfo> {
        let mut types: Vec<(u32, TypeInfo)> = self
            .plugins
            .iter()
            .map(|plugin| {
                (
                    plugin.priority(),
                    TypeInfo {
                        name: plugin.name(),
                        description: plugin.description(),
                        extensions: plugin.extensions(),
                    },
                )
            })
            .collect();
        types.sort_by_key(|(priority, _)| *priority);
        types.into_iter().map(|(_, info)| info).collect()
    }

    /// The starter template of a type, for the live preview.
    pub fn template(&self, name: &str) -> Option<&'static str> {
        self.find(name).map(|plugin| plugin.template())
    }

    /// The type that claims `extension` (without the dot), if any.
    pub fn type_for_extension(&self, extension: &str) -> Option<&'static str> {
        self.plugins
            .iter()
            .find(|plugin| {
                plugin
                    .extensions()
                    .iter()
                    .any(|claimed| claimed.eq_ignore_ascii_case(extension))
            })
            .map(|plugin| plugin.name())
    }

    /// The type of a file, decided by its extension.
    pub fn type_for_path(&self, path: &Path) -> Option<&'static str> {
        let extension = path.extension().and_then(|ext| ext.to_str())?;
        self.type_for_extension(extension)
    }

    /// A comma-separated list of the known type names, for error messages.
    pub fn available(&self) -> String {
        self.names().join(", ")
    }

    /// Find a plugin by name (case-insensitive).
    pub fn find(&self, name: &str) -> Option<&dyn DiagramPlugin> {
        self.plugins
            .iter()
            .find(|plugin| plugin.name().eq_ignore_ascii_case(name))
            .map(|plugin| plugin.as_ref())
    }

    /// Pick the plugin that should handle the input.
    ///
    /// The order is: an explicit `--type`, then the file extension, then the
    /// content of the file.
    pub fn resolve(&self, request: &PluginRequest<'_>) -> Result<&dyn DiagramPlugin, PluginError> {
        if let Some(name) = request.type_name {
            return self
                .find(name)
                .ok_or_else(|| PluginError::UnknownType(name.to_string(), self.available()));
        }

        if let Some(path) = request.path
            && let Some(extension) = path.extension().and_then(|e| e.to_str())
        {
            let extension = extension.trim_start_matches('.').to_ascii_lowercase();
            if let Some(plugin) = self
                .plugins
                .iter()
                .find(|plugin| plugin.extensions().iter().any(|e| *e == extension))
            {
                return Ok(plugin.as_ref());
            }
        }

        if let Some(plugin) = self
            .plugins
            .iter()
            .find(|plugin| plugin.detect(request.source))
        {
            return Ok(plugin.as_ref());
        }

        Err(PluginError::UnknownDiagramType {
            path: request
                .path
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<input>".to_string()),
            available: self.available(),
        })
    }
}

impl Default for Registry {
    fn default() -> Self {
        Registry::builtin()
    }
}

/// Turn a rendered document into the bytes of the requested format.
///
/// This is the one place where a plugin's SVG becomes an output file, so the
/// command line and the live-preview server produce the same bytes.
pub fn render_output(
    format: Format,
    rendered: &Rendered,
    options: &RenderOptions,
) -> Result<Vec<u8>, RenderError> {
    render_output_with_font_url(format, rendered, options, None)
}

/// Like [`render_output`], but the HTML bundle references the font at
/// `font_url` instead of embedding it. The live preview serves the font once
/// as a static asset.
pub fn render_output_with_font_url(
    format: Format,
    rendered: &Rendered,
    options: &RenderOptions,
    font_url: Option<&str>,
) -> Result<Vec<u8>, RenderError> {
    let first = rendered.first().ok_or(RenderError::Empty)?;

    Ok(match format {
        Format::Svg => first.svg.as_bytes().to_vec(),
        Format::Png => crate::core::formats::svg_to_png(&first.svg, &options.font, options.scale)
            .map_err(RenderError::Format)?,
        Format::Pdf => crate::core::formats::svg_to_pdf(&first.svg, &options.font)
            .map_err(RenderError::Format)?,
        Format::Html => {
            let root = bundle_root(rendered, options)?;
            let font_face =
                crate::core::html::font_face_css(&options.font, options.embed_font, font_url);
            crate::core::html::bundle(&rendered.sections, &root, &font_face).into_bytes()
        }
    })
}

/// Resolve the diagram an HTML bundle opens with.
///
/// Plugins that resolve their own bundle root (the activity diagram, which
/// also resolves cross-file references) set [`Rendered::root`]. For every
/// other type, `--root` is matched against the section keys first and then
/// against the diagram labels, because a second diagram of the same type gets
/// a page-unique key such as `detail-n1` while its label stays `detail`.
fn bundle_root(rendered: &Rendered, options: &RenderOptions) -> Result<String, RenderError> {
    if let Some(id) = options.bundle_root.as_deref() {
        if let Some(root) = &rendered.root {
            return Ok(root.clone());
        }
        return rendered
            .sections
            .iter()
            .find(|section| section.key == id)
            .or_else(|| rendered.sections.iter().find(|section| section.label == id))
            .map(|section| section.key.clone())
            .ok_or_else(|| RenderError::Bundle(format!("unknown root diagram: {id:?}")));
    }

    rendered
        .sections
        .first()
        .map(|section| rendered.root.clone().unwrap_or_else(|| section.key.clone()))
        .ok_or(RenderError::Empty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_knows_every_type() {
        let registry = Registry::builtin();
        assert_eq!(
            registry.names(),
            vec!["sequence", "tree", "treemap", "manifest", "activity"]
        );
        assert!(registry.find("activity").is_some());
        assert!(registry.find("treemap").is_some());
        assert!(registry.find("ACTIVITY").is_some());
        assert!(registry.find("nope").is_none());
    }

    #[test]
    fn an_explicit_type_wins() {
        let registry = Registry::builtin();
        let request = PluginRequest {
            type_name: Some("activity"),
            path: None,
            source: "sequence { }",
        };
        assert_eq!(registry.resolve(&request).unwrap().name(), "activity");
    }

    #[test]
    fn an_unknown_type_is_reported_with_the_available_ones() {
        let registry = Registry::builtin();
        let request = PluginRequest {
            type_name: Some("nope"),
            path: None,
            source: "",
        };
        let err = match registry.resolve(&request) {
            Ok(_) => panic!("an unknown type must not resolve"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("sequence"));
        assert!(err.to_string().contains("activity"));
    }

    #[test]
    fn the_extension_picks_the_plugin() {
        let registry = Registry::builtin();
        let request = PluginRequest {
            type_name: None,
            path: Some(Path::new("chat.seq")),
            source: "",
        };
        assert_eq!(registry.resolve(&request).unwrap().name(), "sequence");
    }

    #[test]
    fn the_treemap_extension_picks_the_plugin() {
        let registry = Registry::builtin();
        let request = PluginRequest {
            type_name: None,
            path: Some(Path::new("budget.tmap")),
            source: "",
        };
        assert_eq!(registry.resolve(&request).unwrap().name(), "treemap");
    }

    #[test]
    fn the_content_picks_the_plugin() {
        let registry = Registry::builtin();
        let request = PluginRequest {
            type_name: None,
            path: None,
            source: "sequence demo {\n  participant A;\n}",
        };
        assert_eq!(registry.resolve(&request).unwrap().name(), "sequence");
    }

    #[test]
    fn the_treemap_content_picks_the_plugin() {
        let registry = Registry::builtin();
        let request = PluginRequest {
            type_name: None,
            path: None,
            source: "treemap demo {\n  A: 1;\n}",
        };
        assert_eq!(registry.resolve(&request).unwrap().name(), "treemap");
    }
}
