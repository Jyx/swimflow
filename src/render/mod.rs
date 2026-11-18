//! Rendering: geometry -> output format.
//!
//! The SVG writer is canonical; PNG (`resvg`) and PDF (`svg2pdf`) are derived
//! from the same SVG tree so all formats share one layout and one font.

pub mod bundle;
pub mod labels;
pub mod legend;
pub mod pdf;
pub mod png;
pub mod svg;

use std::io::Write;

use crate::build::model::DocumentSet;

/// Supported output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Svg,
    Png,
    Pdf,
    Html,
}

impl Format {
    /// Infer a format from a file extension (case-insensitive).
    pub fn from_extension(extension: &str) -> Option<Format> {
        match extension
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "svg" => Some(Format::Svg),
            "png" => Some(Format::Png),
            "pdf" => Some(Format::Pdf),
            "html" | "htm" => Some(Format::Html),
            _ => None,
        }
    }

    /// Parse a format name (the CLI `--format` value).
    pub fn from_name(name: &str) -> Option<Format> {
        match name.to_ascii_lowercase().as_str() {
            "svg" => Some(Format::Svg),
            "png" => Some(Format::Png),
            "pdf" => Some(Format::Pdf),
            "html" => Some(Format::Html),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Format::Svg => "svg",
            Format::Png => "png",
            Format::Pdf => "pdf",
            Format::Html => "html",
        }
    }

    /// MIME type used when writing this format to an HTTP response.
    pub fn content_type(self) -> &'static str {
        match self {
            Format::Svg => "image/svg+xml",
            Format::Png => "image/png",
            Format::Pdf => "application/pdf",
            Format::Html => "text/html; charset=utf-8",
        }
    }
}

/// Options shared by the renderers.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Embed the default font as a base64 `@font-face` so the SVG is fully
    /// self-contained and renders identically everywhere.
    pub embed_font: bool,
    /// Outline glyphs into `<path>` elements, removing every font dependency
    /// (and all `<text>` elements).
    pub text_as_paths: bool,
    /// Omit the XML doctype declaration.
    pub nodoctype: bool,
    /// Emit `width`/`height` instead of a `viewBox`.
    pub noviewbox: bool,
    /// Do not paint an opaque background.
    pub transparent: bool,
    /// Raster scale for PNG (1.0 = 96 DPI).
    pub scale: f32,
    /// Entry diagram for an HTML bundle. `None` uses the first diagram of
    /// the root document.
    pub bundle_root: Option<String>,
    /// Omit external `href` links. Internal `ref` links are
    /// unaffected because they never leave the bundle.
    pub no_external: bool,
    /// Font used for text measurement, the SVG `@font-face`/`font-family` and
    /// the `usvg` database behind PNG/PDF.
    pub font: crate::text::FontSpec,
    /// The state of an interactive view an export should reproduce (folded
    /// branches, the open diagram). Empty for a plain render.
    pub view: crate::core::view::ViewState,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            embed_font: false,
            text_as_paths: false,
            nodoctype: false,
            noviewbox: false,
            transparent: false,
            scale: 1.0,
            bundle_root: None,
            no_external: false,
            font: crate::text::FontSpec::default(),
            view: crate::core::view::ViewState::default(),
        }
    }
}

/// Errors produced while rendering.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("output format not implemented yet: {0}")]
    Unsupported(&'static str),
    #[error("no diagrams to render")]
    Empty,
    #[error("bundle error: {0}")]
    Bundle(String),
    #[error(transparent)]
    Format(#[from] crate::core::formats::FormatError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// A renderer turns a [`DocumentSet`] into bytes for one output format.
pub trait Renderer {
    fn render(
        &self,
        set: &DocumentSet,
        out: &mut dyn Write,
        options: &RenderOptions,
    ) -> Result<(), RenderError>;
}

/// Render the root document with the requested format.
pub fn render(
    format: Format,
    set: &DocumentSet,
    out: &mut dyn Write,
    options: &RenderOptions,
) -> Result<(), RenderError> {
    match format {
        Format::Svg => svg::SvgRenderer.render(set, out, options),
        Format::Png => png::PngRenderer.render(set, out, options),
        Format::Pdf => pdf::PdfRenderer.render(set, out, options),
        Format::Html => bundle::BundleRenderer.render(set, out, options),
    }
}

/// The first (root) diagram of a document set, or [`RenderError::Empty`].
pub(crate) fn first_diagram(
    set: &DocumentSet,
) -> Result<&crate::build::model::Diagram, RenderError> {
    set.root().diagrams.first().ok_or(RenderError::Empty)
}
