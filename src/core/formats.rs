//! Output formats derived from a canonical SVG document.
//!
//! SVG is the canonical artifact of every diagram type (see `crate::plugin`),
//! so PNG and PDF are produced from an SVG *string*. This is what makes the
//! formats available to every plugin for free: a new diagram type only has to
//! draw SVG, and it immediately gets PNG and PDF output.

use crate::text::FontSpec;

/// Errors produced while converting an SVG document to another format.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("failed to parse the generated SVG: {0}")]
    Svg(String),
    #[error("failed to allocate the raster canvas")]
    Raster,
    #[error("failed to encode PNG: {0}")]
    Png(String),
    #[error("failed to convert to PDF: {0}")]
    Pdf(String),
}

/// `usvg` options that use only the selected font (no system fonts), so PNG
/// and PDF shaping matches the SVG measurement.
pub fn usvg_options(font: &FontSpec) -> usvg::Options<'static> {
    let mut options = usvg::Options::default();
    options.fontdb_mut().load_font_data(font.data.to_vec());
    options
}

/// Parse an SVG string into a `usvg` tree using the selected font.
pub fn parse_svg(svg: &str, font: &FontSpec) -> Result<usvg::Tree, FormatError> {
    usvg::Tree::from_str(svg, &usvg_options(font)).map_err(|e| FormatError::Svg(e.to_string()))
}

/// Rasterize an SVG document into a PNG image.
///
/// The font is loaded directly into `usvg`'s database, so the raster output
/// shapes text exactly like the SVG measurement.
pub fn svg_to_png(svg: &str, font: &FontSpec, scale: f32) -> Result<Vec<u8>, FormatError> {
    let tree = parse_svg(svg, font)?;
    let size = tree.size();
    let scale = if scale > 0.0 { scale } else { 1.0 };

    let width = (size.width() * scale).ceil().max(1.0) as u32;
    let height = (size.height() * scale).ceil().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).ok_or(FormatError::Raster)?;

    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    pixmap
        .encode_png()
        .map_err(|e| FormatError::Png(e.to_string()))
}

/// Convert an SVG document into a PDF document.
pub fn svg_to_pdf(svg: &str, font: &FontSpec) -> Result<Vec<u8>, FormatError> {
    let tree = parse_svg(svg, font)?;

    svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .map_err(|e| FormatError::Pdf(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"20\" height=\"10\">\
        <rect x=\"0\" y=\"0\" width=\"20\" height=\"10\" fill=\"#ff0000\"/></svg>";

    #[test]
    fn png_has_the_expected_dimensions() {
        let font = FontSpec::default();
        let png = svg_to_png(SVG, &font, 1.0).expect("png");
        assert_eq!(&png[1..4], b"PNG");
        // IHDR width/height are big-endian u32 at offset 16.
        let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
        assert_eq!((width, height), (20, 10));
    }

    #[test]
    fn pdf_starts_with_the_pdf_header() {
        let font = FontSpec::default();
        let pdf = svg_to_pdf(SVG, &font).expect("pdf");
        assert!(pdf.starts_with(b"%PDF-"));
    }

    #[test]
    fn invalid_svg_is_reported() {
        let font = FontSpec::default();
        let err = svg_to_png("<not-svg", &font, 1.0).unwrap_err();
        assert!(matches!(err, FormatError::Svg(_)));
    }
}
