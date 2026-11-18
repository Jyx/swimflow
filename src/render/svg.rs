//! Canonical SVG writer: pixel geometry -> a standalone SVG document.
//!
//! The writer mirrors the reference `DiagramDraw` and blockdiag's SVG
//! `ImageDraw`: lane headers and the swimlane frame are drawn first, then the
//! nodes, then the edges and their arrowheads. Text is either emitted as
//! `<text>` (optionally with an embedded `@font-face`) or, with
//! `--text-as-paths`, as filled `<path>` outlines so the file has no font
//! dependency at all.

use std::io::Write;

use crate::build::link::Link;
use crate::build::model::{Diagram, LegendPosition};
use crate::core::svg::{
    self, TextAlign, TextStyle, close_document, dash_style, escape_xml, open_document,
};
use crate::metrics::{EdgeRoute, Metrics, PixelBox};
use crate::render::labels::{self, EdgeLabelKind, EdgeLabelLayout};
use crate::render::legend::{self, LegendGlyph, LegendLayout};
use crate::render::{RenderError, RenderOptions, Renderer};
use crate::text::TextMeasurer;

/// A resolved hyperlink target for a node, used to wrap a node in an SVG
/// `<a>` element. Produced by the standalone renderer (external URLs and
/// in-document `#diagram` anchors) and by the bundle renderer (diagram keys).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeHref {
    /// `None` renders the node as an inert, aria-disabled group (broken link,
    /// or an external URL suppressed by `--no-external`).
    pub href: Option<String>,
    /// `data-target` attribute used by the bundle navigation runtime.
    pub data_target: Option<String>,
    /// Accessible description announced for the link.
    pub aria_label: Option<String>,
}

impl NodeHref {
    /// An inert placeholder for a node that carries an unusable link.
    pub fn disabled() -> Self {
        NodeHref {
            href: None,
            data_target: None,
            aria_label: None,
        }
    }
}

/// Default lane header color (the reference `NodeGroup.color`).
const LANE_COLOR: &str = "#ffff99";
/// Frame outline and separators (the reference drawer).
const FRAME_COLOR: &str = "gray";
/// Default node fill (`Element.basecolor`).
const NODE_FILL: &str = "#ffffff";
/// Default node outline (`DiagramNode.linecolor`).
const NODE_LINE: &str = "#000000";
/// Default text color (`Element.textcolor`).
const TEXT_COLOR: &str = "#000000";
/// Padding of the opaque halo behind a non-reference edge label.
const HALO_PAD: i32 = 2;

/// An axis-aligned integer rectangle used to accumulate the drawn bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bounds {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl Bounds {
    fn from_xywh(x: i32, y: i32, width: i32, height: i32) -> Bounds {
        Bounds {
            x1: x,
            y1: y,
            x2: x + width,
            y2: y + height,
        }
    }

    fn add_point(&mut self, x: i32, y: i32) {
        self.x1 = self.x1.min(x);
        self.y1 = self.y1.min(y);
        self.x2 = self.x2.max(x);
        self.y2 = self.y2.max(y);
    }

    fn add_box(&mut self, box_: &PixelBox) {
        self.add_point(box_.x1, box_.y1);
        self.add_point(box_.x2, box_.y2);
    }

    fn union(self, other: Bounds) -> Bounds {
        Bounds {
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
            x2: self.x2.max(other.x2),
            y2: self.y2.max(other.y2),
        }
    }

    fn width(&self) -> i32 {
        self.x2 - self.x1
    }

    fn height(&self) -> i32 {
        self.y2 - self.y1
    }
}

/// The canonical SVG renderer.
pub struct SvgRenderer;

impl Renderer for SvgRenderer {
    fn render(
        &self,
        set: &crate::build::model::DocumentSet,
        out: &mut dyn Write,
        options: &RenderOptions,
    ) -> Result<(), RenderError> {
        let svg = render_set(set, options)?;
        out.write_all(svg.as_bytes())?;
        Ok(())
    }
}

/// Render the root diagram of `set` to an SVG string using `options`.
pub(crate) fn render_set(
    set: &crate::build::model::DocumentSet,
    options: &RenderOptions,
) -> Result<String, RenderError> {
    let diagram = super::first_diagram(set)?;
    Ok(render_diagram(diagram, options))
}

/// Render one diagram to an SVG string. Deterministic: the same diagram and
/// options always produce the same bytes.
pub fn render_diagram(diagram: &Diagram, options: &RenderOptions) -> String {
    let links = default_links(diagram, options);
    render_diagram_links(diagram, options, &links)
}

/// Render one diagram with an explicit per-node hyperlink mapping. The slice is
/// indexed by node position; a missing entry means "no link". The bundle
/// renderer uses this to point `ref` nodes at in-page diagram sections.
pub fn render_diagram_links(
    diagram: &Diagram,
    options: &RenderOptions,
    links: &[Option<NodeHref>],
) -> String {
    let mut text = TextMeasurer::from_spec(&options.font);
    render_diagram_with_text(diagram, options, links, &mut text)
}

/// Render one diagram reusing an existing text measurer. Creating a
/// [`TextMeasurer`] loads the embedded font, so the bundle renderer shares one
/// across all of its diagrams.
pub fn render_diagram_with_text(
    diagram: &Diagram,
    options: &RenderOptions,
    links: &[Option<NodeHref>],
    text: &mut TextMeasurer,
) -> String {
    let metrics = Metrics::new(diagram);
    let (diagram_w, diagram_h) = metrics.pagesize();

    // Route every edge once and resolve its label placement; both the canvas
    // bounds and the drawing passes reuse the result.
    let routes: Vec<EdgeRoute> = diagram
        .edges
        .iter()
        .map(|edge| metrics.edge_route(diagram, edge))
        .collect();
    let labels = labels::layout(diagram, &metrics, &routes, text);

    let legend = legend::layout(diagram, text);

    let (legend_w, legend_h) = legend
        .as_ref()
        .map(|layout| (layout.width, layout.height))
        .unwrap_or((0, 0));
    let position = legend
        .as_ref()
        .map(|layout| layout.position)
        .unwrap_or(LegendPosition::Bottom);

    // A legend reserves a band next to the diagram; the rest of the geometry is
    // untouched and simply translated into place. `canvas_bounds` also covers
    // the (possibly negative) lane frame in landscape orientation, so nothing
    // is cropped.
    let mut total = canvas_bounds(diagram, &metrics, diagram_w, diagram_h, &routes, &labels);
    let (legend_band_x, legend_band_y) = match position {
        LegendPosition::Top => (0, -legend_h),
        LegendPosition::Bottom => (0, diagram_h),
        LegendPosition::Left => (-legend_w, 0),
        LegendPosition::Right => (diagram_w, 0),
    };
    if legend.is_some() {
        total = total.union(Bounds::from_xywh(
            legend_band_x,
            legend_band_y,
            legend_w,
            legend_h,
        ));
    }

    let width = total.width();
    let height = total.height();
    let dx = -total.x1;
    let dy = -total.y1;
    let legend_x = legend_band_x + dx;
    let legend_y = legend_band_y + dy;

    let mut out = String::new();

    open_document(
        &mut out,
        &svg::DocumentOptions {
            width,
            height,
            font: &options.font,
            embed_font: options.embed_font,
            text_as_paths: options.text_as_paths,
            nodoctype: options.nodoctype,
            noviewbox: options.noviewbox,
            transparent: options.transparent,
        },
    );

    let shifted = dx != 0 || dy != 0;
    if shifted {
        out.push_str(&format!("<g transform=\"translate({dx},{dy})\">\n"));
    }

    draw_lanes(&mut out, text, diagram, &metrics, options);
    draw_frame(&mut out, diagram, &metrics);
    draw_nodes(&mut out, text, diagram, &metrics, options, links);
    draw_edge_shafts(&mut out, diagram, &routes, &labels);
    draw_edge_labels(&mut out, text, diagram, &labels, options);
    draw_badges(&mut out, text, diagram, &metrics, options);

    if shifted {
        out.push_str("</g>\n");
    }

    if let Some(layout) = &legend {
        draw_legend(&mut out, text, layout, legend_x, legend_y, options);
    }

    close_document(&mut out);
    out
}

/// Union bounding box of everything `draw_*` emits (in diagram/page
/// coordinates). The lane frame is included explicitly because in landscape
/// orientation it is drawn above the page origin (negative `y`).
fn canvas_bounds(
    diagram: &Diagram,
    metrics: &Metrics,
    diagram_w: i32,
    diagram_h: i32,
    routes: &[EdgeRoute],
    labels: &[Option<EdgeLabelLayout>],
) -> Bounds {
    let mut bounds = Bounds::from_xywh(0, 0, diagram_w, diagram_h);

    let frame = metrics.frame(&diagram.lanes);
    bounds.add_box(&frame.headerbox);
    bounds.add_box(&frame.outline);

    let badge_radius = crate::metrics::CELLSIZE * 3 / 2;
    for node in &diagram.nodes {
        let cell = metrics.cell(node);
        bounds.add_box(&cell);
        if node.number.is_some() {
            bounds.add_box(&PixelBox {
                x1: cell.x1 - badge_radius,
                y1: cell.y1 - badge_radius,
                x2: cell.x2 + badge_radius,
                y2: cell.y2 + badge_radius,
            });
        }
    }

    for route in routes {
        for polyline in &route.polylines {
            for &(x, y) in polyline {
                bounds.add_point(x, y);
            }
        }
        for head in &route.heads {
            for &(x, y) in head {
                bounds.add_point(x, y);
            }
        }
    }

    // Include the label text boxes (and their halo) so beside/offset labels
    // cannot be cropped either.
    for layout in labels.iter().flatten() {
        let mut box_ = layout.text_box;
        box_.x1 -= HALO_PAD;
        box_.y1 -= HALO_PAD;
        box_.x2 += HALO_PAD;
        box_.y2 += HALO_PAD;
        bounds.add_box(&box_);
    }

    bounds
}

fn draw_lanes(
    out: &mut String,
    text: &mut TextMeasurer,
    diagram: &Diagram,
    metrics: &Metrics,
    options: &RenderOptions,
) {
    for (i, lane) in diagram.lanes.iter().enumerate() {
        let header = metrics.lane_headerbox(lane);
        let color = paint(lane.color.as_deref(), LANE_COLOR);
        svg::rect(out, &header, &color, &color, "");

        let textbox = metrics.lane_textbox(lane);
        let label = lane_label(lane, i);
        draw_text(out, text, &textbox, &label, TEXT_COLOR, options, false);
    }
}

fn draw_frame(out: &mut String, diagram: &Diagram, metrics: &Metrics) {
    let frame = metrics.frame(&diagram.lanes);
    svg::rect(out, &frame.outline, "none", FRAME_COLOR, "");
    for (a, b) in &frame.separators {
        out.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{FRAME_COLOR}\"/>\n",
            a.0, a.1, b.0, b.1
        ));
    }
}

fn draw_nodes(
    out: &mut String,
    text: &mut TextMeasurer,
    diagram: &Diagram,
    metrics: &Metrics,
    options: &RenderOptions,
    links: &[Option<NodeHref>],
) {
    for (index, node) in diagram.nodes.iter().enumerate() {
        let link = links.get(index).and_then(|link| link.as_ref());
        let anchored = link.map(|link| link.href.is_some()).unwrap_or(false);
        if let Some(link) = link {
            if anchored {
                push_anchor_open(out, link);
            } else {
                out.push_str("<g aria-disabled=\"true\" data-link-disabled=\"true\">\n");
            }
        }

        let box_ = metrics.cell(node);
        let fill = paint(node.color.as_deref(), NODE_FILL);
        let stroke = paint(node.linecolor.as_deref(), NODE_LINE);
        let style = dash_style(node.attrs.get("style").and_then(|v| v.as_deref()));
        let rounded = node
            .shape
            .as_deref()
            .map(|shape| shape.eq_ignore_ascii_case("roundedbox"))
            .unwrap_or(false);

        if rounded {
            svg::rounded_rect(out, &box_, 5, &fill, &stroke, &style);
        } else {
            svg::rect(out, &box_, &fill, &stroke, &style);
        }

        let textcolor = paint(node.textcolor.as_deref(), TEXT_COLOR);
        draw_text(out, text, &box_, &node.label, &textcolor, options, anchored);

        if anchored {
            out.push_str("</a>\n");
        } else if link.is_some() {
            out.push_str("</g>\n");
        }
    }
}

/// Open an SVG `<a>` with the resolved link attributes.
fn push_anchor_open(out: &mut String, link: &NodeHref) {
    let mut attrs = String::new();
    if let Some(value) = &link.href {
        attrs.push_str(&format!(" href=\"{}\"", escape_xml(value)));
    }
    if let Some(value) = &link.data_target {
        attrs.push_str(&format!(" data-target=\"{}\"", escape_xml(value)));
    }
    if let Some(value) = &link.aria_label {
        attrs.push_str(&format!(" aria-label=\"{}\"", escape_xml(value)));
    }
    out.push_str(&format!("<a{attrs}>\n"));
}

/// Default standalone link mapping: external `href` as-is and internal `ref` as
/// an in-page `#diagram` anchor. `--no-external` turns external links inert.
fn default_links(diagram: &Diagram, options: &RenderOptions) -> Vec<Option<NodeHref>> {
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
            Some(Link::Diagram(_)) => match &node.resolved {
                Some(resolved) => Some(NodeHref {
                    href: Some(format!("#{}", resolved.diagram)),
                    data_target: Some(resolved.diagram.clone()),
                    aria_label: Some(format!(
                        "{}: go to diagram {}",
                        node.label, resolved.diagram
                    )),
                }),
                None => Some(NodeHref::disabled()),
            },
        })
        .collect()
}

/// Draw every edge shaft and arrowhead. An inline-labelled segment is split so
/// the shaft stops at the text.
fn draw_edge_shafts(
    out: &mut String,
    diagram: &Diagram,
    routes: &[EdgeRoute],
    labels: &[Option<EdgeLabelLayout>],
) {
    for (index, edge) in diagram.edges.iter().enumerate() {
        if edge
            .style
            .as_deref()
            .map(|style| style.eq_ignore_ascii_case("none"))
            .unwrap_or(false)
        {
            continue;
        }

        let Some(route) = routes.get(index) else {
            continue;
        };
        let color = paint(edge.color.as_deref(), TEXT_COLOR);
        let style = dash_style(edge.style.as_deref());
        let inline = labels
            .get(index)
            .and_then(|layout| layout.as_ref())
            .and_then(|layout| match &layout.kind {
                EdgeLabelKind::Inline {
                    polyline,
                    segment,
                    gap,
                } => Some((*polyline, *segment, *gap)),
                _ => None,
            });

        for (polyline_index, polyline) in route.polylines.iter().enumerate() {
            match inline {
                Some((target, segment, gap)) if target == polyline_index => {
                    for part in split_at_gap(polyline, segment, gap) {
                        svg::path(out, &part, &color, &style);
                    }
                }
                _ => svg::path(out, polyline, &color, &style),
            }
        }

        for head in &route.heads {
            if head.is_empty() {
                continue;
            }
            let points: Vec<String> = head.iter().map(|(x, y)| format!("{x},{y}")).collect();
            out.push_str(&format!(
                "<polygon points=\"{}\" fill=\"{color}\" stroke=\"{color}\"/>\n",
                points.join(" ")
            ));
        }
    }
}

/// Draw every edge label above the shafts, so its halo covers crossing edges.
fn draw_edge_labels(
    out: &mut String,
    text: &mut TextMeasurer,
    diagram: &Diagram,
    labels: &[Option<EdgeLabelLayout>],
    options: &RenderOptions,
) {
    for (edge, layout) in diagram.edges.iter().zip(labels) {
        let Some(layout) = layout else {
            continue;
        };
        let Some(label) = edge.label.as_deref().filter(|label| !label.is_empty()) else {
            continue;
        };
        let color = edge_label_color(edge);

        match &layout.kind {
            EdgeLabelKind::Reference => {
                // The reference box keeps its outline for compatibility.
                svg::rect(out, &layout.text_box, "#ffffff", TEXT_COLOR, "");
            }
            _ if !options.transparent => {
                svg::rect(
                    out,
                    &inflate(&layout.text_box, HALO_PAD),
                    "#ffffff",
                    "none",
                    "",
                );
            }
            _ => {}
        }

        draw_text(out, text, &layout.text_box, label, &color, options, false);
    }
}

/// Split a horizontal polyline segment around an inline label gap. The gap is
/// an x-range; the segment's `y` is preserved.
fn split_at_gap(polyline: &[(i32, i32)], segment: usize, gap: (i32, i32)) -> Vec<Vec<(i32, i32)>> {
    if segment + 1 >= polyline.len() {
        return vec![polyline.to_vec()];
    }
    let a = polyline[segment];
    let b = polyline[segment + 1];
    let lo = a.0.min(b.0);
    let hi = a.0.max(b.0);
    let gap0 = lo.max(gap.0);
    let gap1 = hi.min(gap.1);
    if gap0 >= gap1 {
        return vec![polyline.to_vec()];
    }

    let (first_end, second_start) = if a.0 <= b.0 {
        ((gap0, a.1), (gap1, a.1))
    } else {
        ((gap1, a.1), (gap0, a.1))
    };
    let mut first: Vec<(i32, i32)> = polyline[..=segment].to_vec();
    first.push(first_end);
    let mut second: Vec<(i32, i32)> = vec![second_start];
    second.extend_from_slice(&polyline[segment + 1..]);
    vec![first, second]
}

fn inflate(box_: &PixelBox, pad: i32) -> PixelBox {
    PixelBox {
        x1: box_.x1 - pad,
        y1: box_.y1 - pad,
        x2: box_.x2 + pad,
        y2: box_.y2 + pad,
    }
}

/// Sequence-number badges, drawn above the edges.
fn draw_badges(
    out: &mut String,
    text: &mut TextMeasurer,
    diagram: &Diagram,
    metrics: &Metrics,
    options: &RenderOptions,
) {
    for node in &diagram.nodes {
        let Some(number) = node.number else {
            continue;
        };

        let box_ = metrics.cell(node);
        let (cx, cy) = badge_center(&box_, node);
        let radius = crate::metrics::CELLSIZE * 3 / 2;
        let badge = PixelBox {
            x1: cx - radius,
            y1: cy - radius,
            x2: cx + radius,
            y2: cy + radius,
        };
        let line = paint(node.linecolor.as_deref(), NODE_LINE);
        out.push_str(&format!(
            "<ellipse cx=\"{cx}\" cy=\"{cy}\" rx=\"{radius}\" ry=\"{radius}\" \
             fill=\"#ffc0cb\" stroke=\"{line}\"/>\n"
        ));
        let textcolor = paint(node.textcolor.as_deref(), TEXT_COLOR);
        draw_text(
            out,
            text,
            &badge,
            &number.to_string(),
            &textcolor,
            options,
            false,
        );
    }
}

/// The badge centre for a node (`number_position`, default top-left).
fn badge_center(box_: &PixelBox, node: &crate::build::model::Node) -> (i32, i32) {
    match node
        .attrs
        .get("number_position")
        .and_then(|value| value.as_deref())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("top-right") => box_.topright(),
        Some("bottom-left") => box_.bottomleft(),
        Some("bottom-right") => box_.bottomright(),
        _ => box_.topleft(),
    }
}

fn edge_label_color(edge: &crate::build::model::Edge) -> String {
    edge.attrs
        .get("label_color")
        .and_then(|value| value.as_deref())
        .or_else(|| {
            edge.attrs
                .get("textcolor")
                .and_then(|value| value.as_deref())
        })
        .map(str::to_string)
        .unwrap_or_else(|| TEXT_COLOR.to_string())
}

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

fn draw_text(
    out: &mut String,
    measurer: &mut TextMeasurer,
    box_: &PixelBox,
    value: &str,
    color: &str,
    options: &RenderOptions,
    underline: bool,
) {
    let style = TextStyle {
        color,
        text_as_paths: options.text_as_paths,
        align: TextAlign::Center,
        underline,
    };
    svg::draw_text(out, measurer, box_, value, &style);
}

fn draw_text_aligned(
    out: &mut String,
    measurer: &mut TextMeasurer,
    box_: &PixelBox,
    value: &str,
    style: &TextStyle<'_>,
) {
    svg::draw_text(out, measurer, box_, value, style);
}

// ---------------------------------------------------------------------------
// Legend
// ---------------------------------------------------------------------------

fn draw_legend(
    out: &mut String,
    text: &mut TextMeasurer,
    layout: &LegendLayout,
    x: i32,
    y: i32,
    options: &RenderOptions,
) {
    let mut cursor = y + legend::PADDING;
    let left = x + legend::PADDING;
    let right = x + layout.width - legend::PADDING;

    if let Some(title) = &layout.title {
        let box_ = PixelBox {
            x1: left,
            y1: cursor,
            x2: right,
            y2: cursor + legend::TITLE_HEIGHT,
        };
        draw_text_aligned(out, text, &box_, title, &left_style(options));
        cursor += legend::TITLE_HEIGHT;
    }

    let text_x = left + legend::SWATCH + legend::GAP;
    for row in &layout.rows {
        let glyph = PixelBox {
            x1: left,
            y1: cursor + (legend::ROW_HEIGHT - legend::SWATCH) / 2,
            x2: left + legend::SWATCH,
            y2: cursor + (legend::ROW_HEIGHT + legend::SWATCH) / 2,
        };

        match &row.glyph {
            LegendGlyph::Swatch { color, shape } => {
                draw_swatch(out, &glyph, color.as_deref(), shape.as_deref());
            }
            LegendGlyph::Badge { number } => {
                let radius = legend::SWATCH / 2;
                let cx = glyph.x1 + radius;
                let cy = glyph.y1 + radius;
                out.push_str(&format!(
                    "<ellipse cx=\"{cx}\" cy=\"{cy}\" rx=\"{radius}\" ry=\"{radius}\" \
                     fill=\"#ffc0cb\" stroke=\"{TEXT_COLOR}\"/>\n"
                ));
                if let Some(number) = number {
                    let badge = PixelBox {
                        x1: cx - radius,
                        y1: cy - radius,
                        x2: cx + radius,
                        y2: cy + radius,
                    };
                    draw_text(
                        out,
                        text,
                        &badge,
                        &number.to_string(),
                        TEXT_COLOR,
                        options,
                        false,
                    );
                }
            }
        }

        let box_ = PixelBox {
            x1: text_x,
            y1: cursor,
            x2: right,
            y2: cursor + legend::ROW_HEIGHT,
        };
        draw_text_aligned(out, text, &box_, &row.label, &left_style(options));
        cursor += legend::ROW_HEIGHT;
    }
}

/// A left-aligned text style in the default color.
fn left_style(options: &RenderOptions) -> TextStyle<'static> {
    TextStyle {
        color: TEXT_COLOR,
        text_as_paths: options.text_as_paths,
        align: TextAlign::Left,
        underline: false,
    }
}

fn draw_swatch(out: &mut String, box_: &PixelBox, color: Option<&str>, shape: Option<&str>) {
    let fill = paint(color, NODE_FILL);
    match shape.map(|shape| shape.to_ascii_lowercase()) {
        Some(shape) if shape == "ellipse" || shape == "circle" => {
            let cx = (box_.x1 + box_.x2) / 2;
            let cy = (box_.y1 + box_.y2) / 2;
            let rx = box_.width() / 2;
            let ry = box_.height() / 2;
            out.push_str(&format!(
                "<ellipse cx=\"{cx}\" cy=\"{cy}\" rx=\"{rx}\" ry=\"{ry}\" fill=\"{fill}\" \
                 stroke=\"{TEXT_COLOR}\"/>\n"
            ));
        }
        Some(shape) if shape == "roundedbox" => {
            out.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"4\" ry=\"4\" \
                 fill=\"{fill}\" stroke=\"{TEXT_COLOR}\"/>\n",
                box_.x1,
                box_.y1,
                box_.width(),
                box_.height()
            ));
        }
        _ => svg::rect(out, box_, &fill, TEXT_COLOR, ""),
    }
}

// ---------------------------------------------------------------------------
// Primitives and helpers
// ---------------------------------------------------------------------------

fn lane_label(lane: &crate::build::model::Lane, index: usize) -> String {
    lane.label
        .as_deref()
        .filter(|label| !label.is_empty())
        .or(lane.id.as_deref())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Lane {}", index + 1))
}

/// Resolve an attribute color to an SVG paint value.
fn paint(value: Option<&str>, default: &str) -> String {
    match value {
        Some(v) if v.eq_ignore_ascii_case("none") || v.eq_ignore_ascii_case("transparent") => {
            "none".to_string()
        }
        Some(v) if !v.is_empty() => v.to_string(),
        _ => default.to_string(),
    }
}
