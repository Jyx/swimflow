//! PNG output: rasterize the canonical SVG with `resvg`.
//!
//! The SVG is generated with the embedded font loaded into `usvg`'s font
//! database (see `core::formats::usvg_options`), so text shaping matches the
//! SVG output exactly and the binary stays self-contained.
//!
//! The rasterization itself lives in the core, so every diagram type gets
//! PNG output for free once it can produce SVG.

use std::io::Write;

use crate::build::model::DocumentSet;
use crate::core::formats;
use crate::render::{RenderError, RenderOptions, Renderer};

/// Rasterizes the canonical SVG.
pub struct PngRenderer;

impl Renderer for PngRenderer {
    fn render(
        &self,
        set: &DocumentSet,
        out: &mut dyn Write,
        options: &RenderOptions,
    ) -> Result<(), RenderError> {
        let svg = raster_svg(set, options)?;
        let png =
            formats::svg_to_png(&svg, &options.font, options.scale).map_err(RenderError::Format)?;
        out.write_all(&png)?;
        Ok(())
    }
}

/// Generate the intermediate SVG. `--embed-font` is irrelevant here because the
/// font is loaded directly into `usvg`; `--text-as-paths` is honoured.
fn raster_svg(set: &DocumentSet, options: &RenderOptions) -> Result<String, RenderError> {
    let mut svg_options = options.clone();
    svg_options.embed_font = false;
    crate::render::svg::render_set(set, &svg_options)
}
