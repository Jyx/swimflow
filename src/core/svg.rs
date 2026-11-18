//! Shared SVG primitives, text drawing and document scaffolding.
//!
//! Everything in this module is *diagram-agnostic*: it knows how to draw a
//! rectangle, a line, a path, an arrow head and a piece of text, but not what
//! a swimlane or a lifeline is. Diagram-type plugins (see `crate::plugin`)
//! build their drawings from these primitives, so every diagram type shares
//! one font handling, one number formatting and one escaping rules.

use cosmic_text::Command;

use crate::core::geom::PixelBox;
use crate::text::{FontSpec, PositionedGlyph, TextMeasurer};

/// Horizontal alignment of a text block inside its box.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextAlign {
    Center,
    Left,
    Right,
}

/// How a piece of text is drawn.
#[derive(Clone, Copy, Debug)]
pub struct TextStyle<'a> {
    /// Fill color of the glyphs.
    pub color: &'a str,
    /// Outline the glyphs instead of emitting `<text>` (no font dependency).
    pub text_as_paths: bool,
    /// Horizontal alignment inside the box.
    pub align: TextAlign,
    /// Draw a line under the text (used for hyperlinks).
    pub underline: bool,
}

impl<'a> TextStyle<'a> {
    /// A centred, plain text style.
    pub fn new(color: &'a str) -> Self {
        TextStyle {
            color,
            text_as_paths: false,
            align: TextAlign::Center,
            underline: false,
        }
    }
}

/// Options for the SVG document scaffolding ([`open_document`]).
pub struct DocumentOptions<'a> {
    /// Canvas width in pixels.
    pub width: i32,
    /// Canvas height in pixels.
    pub height: i32,
    /// The font used for the `font-family` attribute and the optional
    /// `@font-face` rule.
    pub font: &'a FontSpec,
    /// Embed the font as a base64 `@font-face` so the file is standalone.
    pub embed_font: bool,
    /// Emit glyph outlines instead of `<text>`.
    pub text_as_paths: bool,
    /// Omit the XML doctype.
    pub nodoctype: bool,
    /// Emit `width`/`height` instead of a `viewBox`.
    pub noviewbox: bool,
    /// Do not paint an opaque background.
    pub transparent: bool,
}

/// Write the SVG document header: doctype, `<svg>` element, optional
/// `@font-face` and the opaque background.
///
/// The output is byte-for-byte what the activity renderer produced before the
/// core existed, so existing snapshots stay valid.
pub fn open_document(out: &mut String, options: &DocumentOptions<'_>) {
    if !options.nodoctype {
        out.push_str("<?xml version='1.0' encoding='UTF-8'?>\n");
        out.push_str(
            "<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.0//EN\" \
             \"http://www.w3.org/TR/2001/REC-SVG-20010904/DTD/svg10.dtd\">\n",
        );
    }

    out.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" ");
    out.push_str("xmlns:xlink=\"http://www.w3.org/1999/xlink\"");
    if !options.noviewbox {
        out.push_str(&format!(
            " viewBox=\"0 0 {} {}\"",
            options.width, options.height
        ));
    }
    out.push_str(&format!(
        " width=\"{}\" height=\"{}\">\n",
        options.width, options.height
    ));

    if options.embed_font && !options.text_as_paths {
        out.push_str("<style>@font-face{font-family:'");
        out.push_str(&escape_xml(&options.font.family));
        out.push_str("';src:url(data:font/ttf;base64,");
        out.push_str(&base64(&options.font.data));
        out.push_str(") format('truetype');}</style>\n");
    }

    if !options.transparent {
        out.push_str(&format!(
            "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"#ffffff\"/>\n",
            options.width, options.height
        ));
    }
}

/// Close the SVG document.
pub fn close_document(out: &mut String) {
    out.push_str("</svg>\n");
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

/// Draw an axis-aligned rectangle.
pub fn rect(out: &mut String, box_: &PixelBox, fill: &str, stroke: &str, extra: &str) {
    out.push_str(&format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{fill}\" stroke=\"{stroke}\"{extra}/>\n",
        box_.x1,
        box_.y1,
        box_.width(),
        box_.height()
    ));
}

/// Draw an axis-aligned rectangle with rounded corners.
pub fn rounded_rect(
    out: &mut String,
    box_: &PixelBox,
    radius: i32,
    fill: &str,
    stroke: &str,
    extra: &str,
) {
    out.push_str(&format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{radius}\" ry=\"{radius}\" \
         fill=\"{fill}\" stroke=\"{stroke}\"{extra}/>\n",
        box_.x1,
        box_.y1,
        box_.width(),
        box_.height()
    ));
}

/// Draw a straight line.
pub fn line(out: &mut String, x1: i32, y1: i32, x2: i32, y2: i32, color: &str, extra: &str) {
    out.push_str(&format!(
        "<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"{color}\"{extra}/>\n"
    ));
}

/// Draw a polyline as an SVG path.
pub fn path(out: &mut String, polyline: &[(i32, i32)], color: &str, style: &str) {
    let d = polyline_path(polyline);
    if !d.is_empty() {
        out.push_str(&format!(
            "<path d=\"{d}\" fill=\"none\" stroke=\"{color}\"{style}/>\n"
        ));
    }
}

/// Draw a closed polygon.
pub fn polygon(out: &mut String, points: &[(i32, i32)], fill: &str, stroke: &str) {
    let text: Vec<String> = points.iter().map(|(x, y)| format!("{x},{y}")).collect();
    out.push_str(&format!(
        "<polygon points=\"{}\" fill=\"{fill}\" stroke=\"{stroke}\"/>\n",
        text.join(" ")
    ));
}

/// Draw an ellipse.
pub fn ellipse(out: &mut String, cx: i32, cy: i32, rx: i32, ry: i32, fill: &str, stroke: &str) {
    out.push_str(&format!(
        "<ellipse cx=\"{cx}\" cy=\"{cy}\" rx=\"{rx}\" ry=\"{ry}\" fill=\"{fill}\" \
         stroke=\"{stroke}\"/>\n"
    ));
}

/// Build the `d` attribute of a polyline.
pub fn polyline_path(polyline: &[(i32, i32)]) -> String {
    let mut d = String::new();
    for (i, (x, y)) in polyline.iter().enumerate() {
        if i == 0 {
            d.push('M');
        } else {
            d.push('L');
        }
        d.push_str(&format!("{x},{y} "));
    }
    d.trim_end().to_string()
}

/// Serialize positioned glyph outlines (y-up font coordinates) into an SVG path
/// in the y-down document space.
pub fn glyph_path(glyphs: &[PositionedGlyph], left: f32, top: f32) -> String {
    let mut d = String::new();
    for glyph in glyphs {
        let ox = left + glyph.x;
        let oy = top + glyph.y;
        for command in &glyph.commands {
            match command {
                Command::MoveTo(p) => {
                    d.push('M');
                    push_point(&mut d, ox + p.x, oy - p.y);
                }
                Command::LineTo(p) => {
                    d.push('L');
                    push_point(&mut d, ox + p.x, oy - p.y);
                }
                Command::QuadTo(c, p) => {
                    d.push('Q');
                    push_point(&mut d, ox + c.x, oy - c.y);
                    push_point(&mut d, ox + p.x, oy - p.y);
                }
                Command::CurveTo(c1, c2, p) => {
                    d.push('C');
                    push_point(&mut d, ox + c1.x, oy - c1.y);
                    push_point(&mut d, ox + c2.x, oy - c2.y);
                    push_point(&mut d, ox + p.x, oy - p.y);
                }
                Command::Close => d.push('Z'),
            }
        }
    }
    d.trim_end().to_string()
}

fn push_point(d: &mut String, x: f32, y: f32) {
    d.push_str(&fmt(x));
    d.push(',');
    d.push_str(&fmt(y));
    d.push(' ');
}

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

/// Draw `value` inside `box_`, wrapping it to the box width.
///
/// The text is either emitted as `<text>` (one element per visual line) or,
/// with `text_as_paths`, as filled glyph outlines.
pub fn draw_text(
    out: &mut String,
    measurer: &mut TextMeasurer,
    box_: &PixelBox,
    value: &str,
    style: &TextStyle<'_>,
) {
    if value.is_empty() {
        return;
    }

    let max_width = Some((box_.width() as f32).max(1.0));
    let lines = measurer.layout(value, max_width);
    if lines.is_empty() {
        return;
    }

    let line_height = measurer.line_height();
    let total = lines.len() as f32 * line_height;
    let center_x = (box_.x1 as f32 + box_.x2 as f32) / 2.0;
    let center_y = (box_.y1 as f32 + box_.y2 as f32) / 2.0;
    let top = center_y - total / 2.0;
    let (anchor, origin_x) = match style.align {
        TextAlign::Center => ("middle", center_x),
        TextAlign::Left => ("start", box_.x1 as f32),
        TextAlign::Right => ("end", box_.x2 as f32),
    };

    for line in &lines {
        if line.text.is_empty() {
            continue;
        }

        let left = match style.align {
            TextAlign::Center => center_x - line.width / 2.0,
            TextAlign::Left => origin_x,
            TextAlign::Right => origin_x - line.width,
        };

        if style.text_as_paths {
            let d = glyph_path(&line.glyphs, left, top);
            if !d.is_empty() {
                out.push_str(&format!("<path d=\"{d}\" fill=\"{}\"/>\n", style.color));
            }
        } else {
            out.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-family=\"{}, sans-serif\" font-size=\"{}\" \
                 text-anchor=\"{anchor}\" text-rendering=\"geometricPrecision\" \
                 fill=\"{}\">{}</text>\n",
                fmt(origin_x),
                fmt(top + line.baseline),
                escape_xml(measurer.family()),
                fmt(measurer.font_size()),
                escape_xml(style.color),
                escape_xml(&line.text)
            ));
        }

        if style.underline {
            let y = top + line.baseline + measurer.font_size() * 0.12;
            out.push_str(&format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" \
                 stroke=\"{}\" stroke-width=\"1\" data-link-underline=\"true\"/>\n",
                fmt(left),
                fmt(y),
                fmt(left + line.width),
                fmt(y),
                escape_xml(style.color)
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Escape a string for use in XML text and attribute values.
pub fn escape_xml(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

/// Encode bytes as base64 (used for the embedded `@font-face`).
pub fn base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Port of blockdiag's `dasharray` (default `thick` is 1).
pub fn dash_style(style: Option<&str>) -> String {
    let Some(style) = style else {
        return String::new();
    };
    let value = match style.to_ascii_lowercase().as_str() {
        "dotted" => "2".to_string(),
        "dashed" => "4".to_string(),
        "none" => "0 65535".to_string(),
        pattern => {
            if pattern.is_empty()
                || !pattern
                    .split(',')
                    .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
            {
                return String::new();
            }
            pattern
                .split(',')
                .map(str::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        }
    };
    format!(" stroke-dasharray=\"{value}\"")
}

/// Deterministic, compact float formatting; never emits `-0`.
pub fn fmt(value: f32) -> String {
    let value = if value.abs() < 0.005 { 0.0 } else { value };
    let mut text = format!("{value:.2}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping_covers_the_xml_specials() {
        assert_eq!(
            escape_xml("a&b<c>d\"e'f"),
            "a&amp;b&lt;c&gt;d&quot;e&apos;f"
        );
    }

    #[test]
    fn base64_matches_the_reference_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
    }

    #[test]
    fn numbers_are_formatted_deterministically() {
        assert_eq!(fmt(0.0), "0");
        assert_eq!(fmt(-0.0), "0");
        assert_eq!(fmt(1.5), "1.5");
        assert_eq!(fmt(2.0), "2");
        assert_eq!(fmt(12.345), "12.35");
    }

    #[test]
    fn dash_styles_follow_blockdiag() {
        assert_eq!(dash_style(None), "");
        assert_eq!(dash_style(Some("dotted")), " stroke-dasharray=\"2\"");
        assert_eq!(dash_style(Some("dashed")), " stroke-dasharray=\"4\"");
        assert_eq!(dash_style(Some("2,4")), " stroke-dasharray=\"2 4\"");
        assert_eq!(dash_style(Some("bogus")), "");
    }

    #[test]
    fn document_header_contains_the_expected_parts() {
        let font = FontSpec::default();
        let mut out = String::new();
        open_document(
            &mut out,
            &DocumentOptions {
                width: 100,
                height: 50,
                font: &font,
                embed_font: false,
                text_as_paths: false,
                nodoctype: false,
                noviewbox: false,
                transparent: false,
            },
        );
        close_document(&mut out);
        assert!(out.contains("viewBox=\"0 0 100 50\""));
        assert!(out.contains("width=\"100\" height=\"50\""));
        assert!(
            out.contains("<rect x=\"0\" y=\"0\" width=\"100\" height=\"50\" fill=\"#ffffff\"/>")
        );
        assert!(out.ends_with("</svg>\n"));
    }
}
