//! Rendering: the treemap layout -> SVG.
//!
//! The drawing uses the shared core primitives (`crate::core::svg`) and the
//! palette of the activity/tree diagrams: containers get the lane-header
//! yellow, leaves are white, outlines and text are black. Values are drawn by
//! default; a bundle can hide them with the value toggle (`runtime.js`).

use crate::core::geom::PixelBox;
use crate::core::svg::{self, TextAlign, TextStyle};
use crate::render::RenderOptions;
use crate::text::TextMeasurer;

use super::layout::{self, HEADER_HEIGHT, NodeLayout, PADDING};
use super::model::Diagram;

/// Fill of the root and of every container.
const CONTAINER_FILL: &str = "#ffff99";
/// Default leaf fill.
const LEAF_FILL: &str = "#ffffff";
/// Default outline.
const NODE_LINE: &str = "#000000";
/// Default text color.
const TEXT_COLOR: &str = "#000000";
/// The outline width of the root and of nested containers.
const ROOT_STROKE_WIDTH: i32 = 2;
const NODE_STROKE_WIDTH: i32 = 1;

/// Render a treemap as an SVG document.
pub fn render(diagram: &Diagram, options: &RenderOptions) -> String {
    let mut text = TextMeasurer::from_spec(&options.font);
    let layout = layout::layout(diagram);

    let mut out = String::new();
    svg::open_document(
        &mut out,
        &svg::DocumentOptions {
            width: layout.width,
            height: layout.height,
            font: &options.font,
            embed_font: options.embed_font,
            text_as_paths: options.text_as_paths,
            nodoctype: options.nodoctype,
            noviewbox: options.noviewbox,
            transparent: options.transparent,
        },
    );

    out.push_str("<g class=\"swimflow-treemap\">\n");
    for (index, node) in layout.nodes.iter().enumerate() {
        draw_node(&mut out, &mut text, index, node, options);
    }
    out.push_str("</g>\n");

    svg::close_document(&mut out);
    out
}

/// The id of a node in the SVG.
fn node_id(index: usize) -> String {
    format!("n{index}")
}

fn draw_node(
    out: &mut String,
    text: &mut TextMeasurer,
    index: usize,
    node: &NodeLayout,
    options: &RenderOptions,
) {
    let value = fmt_value(node.value);
    out.push_str(&format!(
        "<g class=\"treemap-node\" data-id=\"{}\" data-parent=\"{}\" data-depth=\"{}\" \
         data-value=\"{}\" data-label=\"{}\" data-children=\"{}\" data-show-value=\"{}\" \
         data-x=\"{}\" data-y=\"{}\" data-w=\"{}\" data-h=\"{}\">\n",
        node_id(index),
        node.parent.map(node_id).unwrap_or_default(),
        node.depth,
        svg::escape_xml(&value),
        svg::escape_xml(&node.label),
        u8::from(node.has_children),
        u8::from(node.show_value),
        node.box_.x1,
        node.box_.y1,
        node.box_.width(),
        node.box_.height(),
    ));

    let href = node
        .attr("href")
        .filter(|href| !href.is_empty() && !options.no_external);
    if let Some(href) = href {
        out.push_str(&format!(
            "<a href=\"{}\"{}>\n",
            svg::escape_xml(href),
            link_target(node)
        ));
    }

    let default_fill = if node.depth == 0 || node.has_children {
        CONTAINER_FILL
    } else {
        LEAF_FILL
    };
    let fill = paint(node.attr("color"), default_fill);
    let stroke = paint(node.attr("linecolor"), NODE_LINE);
    let width = if node.depth == 0 {
        ROOT_STROKE_WIDTH
    } else {
        NODE_STROKE_WIDTH
    };
    svg::rect(
        out,
        &node.box_,
        &fill,
        &stroke,
        &format!(" stroke-width=\"{width}\""),
    );

    draw_label(out, text, node, options, href.is_some());
    out.push_str(&format!(
        "<title>{}</title>\n",
        svg::escape_xml(&format!("{}: {value}", node.label))
    ));

    if href.is_some() {
        out.push_str("</a>\n");
    }
    out.push_str("</g>\n");
}

/// Draw the name and (when enabled) the value of a node.
fn draw_label(
    out: &mut String,
    text: &mut TextMeasurer,
    node: &NodeLayout,
    options: &RenderOptions,
    underline: bool,
) {
    let color = paint(node.attr("textcolor"), TEXT_COLOR);
    let value = fmt_value(node.value);

    if node.header {
        let header = PixelBox {
            x1: node.box_.x1 + PADDING,
            y1: node.box_.y1,
            x2: node.box_.x2 - PADDING,
            y2: node.box_.y1 + HEADER_HEIGHT,
        };
        if header.width() <= 0 {
            return;
        }
        let value_width = if node.show_value {
            text.measure(&value, None).width + 8.0
        } else {
            0.0
        };
        let label_width = (header.width() as f32 - value_width).max(1.0);
        draw_fitted(
            out,
            text,
            &header,
            &node.label,
            label_width,
            &color,
            options,
            TextAlign::Left,
            underline,
            "treemap-label",
        );
        if node.show_value && value_width <= header.width() as f32 {
            draw_fitted(
                out,
                text,
                &header,
                &value,
                header.width() as f32,
                &color,
                options,
                TextAlign::Right,
                false,
                "treemap-value",
            );
        }
        return;
    }

    let inner = PixelBox {
        x1: node.box_.x1 + PADDING,
        y1: node.box_.y1 + PADDING / 2,
        x2: node.box_.x2 - PADDING,
        y2: node.box_.y2 - PADDING / 2,
    };
    if inner.width() <= 0 || inner.height() <= 0 {
        return;
    }

    let line_height = text.line_height();
    if node.show_value && inner.height() >= (2.0 * line_height).ceil() as i32 {
        let middle = inner.y1 + inner.height() / 2;
        let label_box = PixelBox {
            x1: inner.x1,
            y1: inner.y1,
            x2: inner.x2,
            y2: middle,
        };
        let value_box = PixelBox {
            x1: inner.x1,
            y1: middle,
            x2: inner.x2,
            y2: inner.y2,
        };
        draw_fitted(
            out,
            text,
            &label_box,
            &node.label,
            label_box.width() as f32,
            &color,
            options,
            TextAlign::Center,
            underline,
            "treemap-label",
        );
        draw_fitted(
            out,
            text,
            &value_box,
            &value,
            value_box.width() as f32,
            &color,
            options,
            TextAlign::Center,
            false,
            "treemap-value",
        );
    } else {
        draw_fitted(
            out,
            text,
            &inner,
            &node.label,
            inner.width() as f32,
            &color,
            options,
            TextAlign::Center,
            underline,
            "treemap-label",
        );
    }
}

/// Draw `value` in `box_`, truncated with `...` when it does not fit.
#[allow(clippy::too_many_arguments)]
fn draw_fitted(
    out: &mut String,
    text: &mut TextMeasurer,
    box_: &PixelBox,
    value: &str,
    max_width: f32,
    color: &str,
    options: &RenderOptions,
    align: TextAlign,
    underline: bool,
    class: &str,
) {
    let Some(fitted) = fit(text, value, max_width) else {
        return;
    };
    out.push_str(&format!("<g class=\"{class}\">\n"));
    let style = TextStyle {
        color,
        text_as_paths: options.text_as_paths,
        align,
        underline,
    };
    svg::draw_text(out, text, box_, &fitted, &style);
    out.push_str("</g>\n");
}

/// Truncate `value` with three ASCII dots so it fits `max_width`.
fn fit(text: &mut TextMeasurer, value: &str, max_width: f32) -> Option<String> {
    if max_width < 1.0 || value.is_empty() {
        return None;
    }
    if text.measure(value, None).width <= max_width {
        return Some(value.to_string());
    }
    let dots = "...";
    if text.measure(dots, None).width > max_width {
        return None;
    }
    let chars: Vec<char> = value.chars().collect();
    let mut low = 0usize;
    let mut high = chars.len();
    while low < high {
        let middle = (low + high).div_ceil(2);
        let mut candidate: String = chars[..middle].iter().collect();
        candidate.push_str(dots);
        if text.measure(&candidate, None).width <= max_width {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let mut fitted: String = chars[..low].iter().collect();
    fitted.truncate(fitted.trim_end().len());
    fitted.push_str(dots);
    Some(fitted)
}

/// A color attribute, with `none`/`transparent` meaning no paint.
fn paint(value: Option<&str>, default: &str) -> String {
    match value {
        Some(v) if v.eq_ignore_ascii_case("none") || v.eq_ignore_ascii_case("transparent") => {
            "none".to_string()
        }
        Some(v) if !v.is_empty() => v.to_string(),
        _ => default.to_string(),
    }
}

/// The `target` of an external link, as in HTML.
fn link_target(node: &NodeLayout) -> String {
    match node.attr("target") {
        Some(target) if !target.is_empty() => {
            let rel = if target.eq_ignore_ascii_case("_self") {
                ""
            } else {
                " rel=\"noopener noreferrer\""
            };
            format!(" target=\"{}\"{rel}", svg::escape_xml(target))
        }
        _ => String::new(),
    }
}

fn fmt_value(value: f64) -> String {
    svg::fmt(value as f32)
}

#[cfg(test)]
mod tests {
    use super::super::model;
    use super::super::parser::parse;
    use super::*;

    fn svg_of(source: &str) -> String {
        let ast = parse(source).unwrap();
        let document = model::build(&ast).unwrap();
        render(&document.diagrams[0], &RenderOptions::default())
    }

    fn fitted(source: &str, width: f32) -> Option<String> {
        let mut text = TextMeasurer::default_size();
        fit(&mut text, source, width)
    }

    #[test]
    fn containers_are_yellow_and_leaves_are_white() {
        let svg = svg_of("treemap { A: 2 { A1: 1; A2: 1; } B: 1; }");
        assert!(svg.contains("fill=\"#ffff99\""), "{svg}");
        assert!(svg.contains("fill=\"#ffffff\""), "{svg}");
    }

    #[test]
    fn values_are_drawn_by_default_and_can_be_switched_off() {
        let shown = svg_of("treemap { A: 42; }");
        assert!(shown.contains("class=\"treemap-value\""), "{shown}");
        assert!(shown.contains(">42</text>"), "{shown}");

        let hidden = svg_of("treemap { showvalue = false; A: 42; }");
        assert!(!hidden.contains("class=\"treemap-value\""), "{hidden}");
        assert!(hidden.contains("<title>A: 42</title>"), "{hidden}");

        let one = svg_of("treemap { A: 42 [showvalue = false]; B: 1; }");
        let a = one
            .split("<g class=\"treemap-node\"")
            .find(|node| node.contains("data-label=\"A\""))
            .expect("node A");
        assert!(!a.contains("class=\"treemap-value\""), "{a}");
    }

    #[test]
    fn colors_labels_links_and_targets_are_applied() {
        let svg = svg_of(
            "treemap { A: 2 [label = Alpha, color = \"#cfe8ff\", textcolor = red, \
             linecolor = blue, href = \"https://example.com\", target = _blank]; }",
        );
        assert!(svg.contains("#cfe8ff"), "{svg}");
        assert!(svg.contains("fill=\"red\""), "{svg}");
        assert!(svg.contains("stroke=\"blue\""), "{svg}");
        assert!(svg.contains("Alpha"), "{svg}");
        assert!(
            svg.contains(
                "<a href=\"https://example.com\" target=\"_blank\" rel=\"noopener noreferrer\">"
            ),
            "{svg}"
        );
    }

    #[test]
    fn no_external_drops_the_link() {
        let ast = parse("treemap { A [href = \"https://example.com\"]; }").unwrap();
        let document = model::build(&ast).unwrap();
        let options = RenderOptions {
            no_external: true,
            ..RenderOptions::default()
        };
        assert!(!render(&document.diagrams[0], &options).contains("<a "));
    }

    #[test]
    fn a_long_label_is_truncated_with_three_dots() {
        assert_eq!(fitted("abcdefghij", 1_000.0).as_deref(), Some("abcdefghij"));
        let short = fitted("a very long label", 35.0).unwrap();
        assert!(short.ends_with("..."), "{short:?}");
        assert!(!short.contains('…'), "{short:?}");
    }

    #[test]
    fn nodes_carry_the_data_the_toggle_needs() {
        let svg = svg_of("treemap { A: 2; }");
        assert!(svg.contains("data-id=\"n0\""), "{svg}");
        assert!(svg.contains("data-id=\"n1\""), "{svg}");
        assert!(svg.contains("data-value=\"2\""), "{svg}");
        assert!(svg.contains("data-show-value=\"1\""), "{svg}");
    }
}
