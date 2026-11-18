//! PDF output: convert the canonical SVG with `svg2pdf`.
//!
//! Like PNG, this uses the embedded font loaded into `usvg`'s database, and
//! keeps text as real selectable text where possible. The conversion lives in
//! the core, so every diagram type gets PDF output for free.

use std::io::Write;

use crate::build::model::DocumentSet;
use crate::core::formats;
use crate::render::{RenderError, RenderOptions, Renderer};

/// Converts the canonical SVG to PDF.
pub struct PdfRenderer;

impl Renderer for PdfRenderer {
    fn render(
        &self,
        set: &DocumentSet,
        out: &mut dyn Write,
        options: &RenderOptions,
    ) -> Result<(), RenderError> {
        let mut svg_options = options.clone();
        svg_options.embed_font = false;
        let svg = crate::render::svg::render_set(set, &svg_options)?;
        let pdf = formats::svg_to_pdf(&svg, &options.font).map_err(RenderError::Format)?;
        out.write_all(&pdf)?;
        Ok(())
    }
}
