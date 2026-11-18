//! Rendering: the tree layout -> SVG.
//!
//! The drawing uses the shared core primitives (`crate::core::svg`) and the
//! palette of the activity diagram: the root gets the yellow of a lane header,
//! every other node is a white box with a black outline, and the connectors
//! are black.

use std::collections::{BTreeMap, BTreeSet};

use crate::core::geom::PixelBox;
use crate::core::svg::{self, TextAlign, TextStyle};
use crate::render::RenderOptions;
use crate::text::TextMeasurer;

use super::ast::Tree;
use super::layout::{self, Layout, NodeLayout, PADDING_X, TOGGLE_RADIUS};

/// Fill of the root (the lane header color of the activity diagram).
const ROOT_FILL: &str = "#ffff99";
/// Default node fill.
const NODE_FILL: &str = "#ffffff";
/// Default node outline.
const NODE_LINE: &str = "#000000";
/// Default text color.
const TEXT_COLOR: &str = "#000000";
/// Color of a connector.
const EDGE_COLOR: &str = "#000000";
/// Corner radius of a `roundedbox` node.
const CORNER_RADIUS: i32 = 5;

/// How the `ref` links of a tree are resolved.
#[derive(Debug, Clone, Default)]
pub struct Links {
    /// Whether the output can navigate (an HTML bundle). A standalone SVG
    /// shows a `ref` node as a disabled link.
    pub navigable: bool,
    /// The section key of every tree of the file, by tree id.
    pub keys: BTreeMap<String, String>,
}

/// What a `ref` value points to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefTarget {
    /// A tree of the same file; the section key.
    Section(String),
    /// A tree id the file does not define.
    Unknown(String),
    /// A reference to another file, which trees do not support yet.
    OtherFile,
}

impl Links {
    /// Resolve the value of a `ref` attribute (`#id`, `id`).
    pub fn resolve(&self, value: &str) -> RefTarget {
        let id = match value.strip_prefix('#') {
            Some(id) => id,
            None if value.contains('#')
                || value.ends_with(".tree")
                || value.ends_with(".treediag") =>
            {
                return RefTarget::OtherFile;
            }
            None => value,
        };
        match self.keys.get(id) {
            Some(key) => RefTarget::Section(key.clone()),
            None => RefTarget::Unknown(id.to_string()),
        }
    }
}

/// Render a tree as a standalone SVG document (no navigation).
pub fn render(tree: &Tree, options: &RenderOptions) -> String {
    render_with_links(tree, options, &Links::default(), None)
}

/// Render a tree as an SVG document.
///
/// The tree is drawn in the state the source asks for (`collapsed` branches
/// are folded), or in the state `folded` describes.
/// Every element a runtime needs to fold and unfold the branches carries the
/// geometry in `data-*` attributes, so an HTML bundle can lay the tree out
/// again without the server.
pub fn render_with_links(
    tree: &Tree,
    options: &RenderOptions,
    links: &Links,
    folded: Option<&BTreeSet<usize>>,
) -> String {
    let mut text = TextMeasurer::from_spec(&options.font);
    let layout = layout::layout_with(tree, &mut text, folded);
    let orientation = match layout::Orientation::of(tree) {
        layout::Orientation::Portrait => "portrait",
        layout::Orientation::Landscape => "landscape",
    };

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

    out.push_str(&format!(
        "<g class=\"swimflow-tree\" data-orientation=\"{orientation}\">\n"
    ));
    draw_edges(&mut out, &mut text, &layout, options);
    draw_nodes(&mut out, &mut text, &layout, options, links);
    draw_toggles(&mut out, &layout);
    out.push_str("</g>\n");

    svg::close_document(&mut out);
    out
}

/// The id of a node in the SVG.
fn node_id(index: usize) -> String {
    format!("n{index}")
}

/// A connector path made of one or more polylines; `data-for` ties it to the
/// node whose folding hides it.
fn connector(out: &mut String, class: &str, owner: usize, paths: &[&[(i32, i32)]], dashed: bool) {
    let d = paths
        .iter()
        .map(|polyline| svg::polyline_path(polyline))
        .collect::<Vec<_>>()
        .join(" ");
    let style = if dashed {
        dashed_style()
    } else {
        String::new()
    };
    out.push_str(&format!(
        "<path class=\"{class}\" data-for=\"{}\" d=\"{d}\" fill=\"none\" stroke=\"{EDGE_COLOR}\"{style}/>\n",
        node_id(owner),
    ));
}

/// The dash pattern of a connector with `edgestyle = dashed`.
fn dashed_style() -> String {
    svg::dash_style(Some("dashed"))
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

/// The connectors and their labels.
fn draw_edges(out: &mut String, text: &mut TextMeasurer, layout: &Layout, options: &RenderOptions) {
    for trunk in &layout.trunks {
        connector(
            out,
            "tree-stem",
            trunk.parent,
            &[&trunk.stem],
            trunk.stem_dashed,
        );
        let solid: Vec<&[(i32, i32)]> = trunk.bus.iter().map(Vec::as_slice).collect();
        connector(out, "tree-bus", trunk.parent, &solid, false);
        // Only a trunk with a dashed connector gets the second path.
        if trunk.has_dashed {
            let dashed: Vec<&[(i32, i32)]> = trunk.bus_dashed.iter().map(Vec::as_slice).collect();
            connector(out, "tree-bus-dashed", trunk.parent, &dashed, true);
        }
    }
    for edge in &layout.edges {
        connector(out, "tree-drop", edge.child, &[&edge.polyline], edge.dashed);
    }
    // Labels are drawn after all connectors so no line crosses a label.
    for edge in &layout.edges {
        if let Some((label, box_)) = &edge.label {
            let hidden = if layout.nodes[edge.child].visible {
                ""
            } else {
                " display=\"none\""
            };
            out.push_str(&format!(
                "<g class=\"tree-label\" data-for=\"{}\" data-x=\"{}\" data-y=\"{}\" \
                 data-w=\"{}\" data-h=\"{}\"{hidden}>\n",
                node_id(edge.child),
                box_.x1,
                box_.y1,
                box_.width(),
                box_.height()
            ));
            svg::rect(out, box_, "#ffffff", "none", "");
            draw_text(out, text, box_, label, TEXT_COLOR, options, false);
            out.push_str("</g>\n");
        }
    }
}

/// How a node links somewhere.
enum NodeLink {
    /// An external URL.
    External(String),
    /// A tree of the same bundle.
    Section(String),
    /// A link that cannot be followed (shown dimmed).
    Disabled,
}

fn link_of(node: &NodeLayout, options: &RenderOptions, links: &Links) -> Option<NodeLink> {
    if let Some(value) = node.attr("ref") {
        return Some(match links.resolve(value) {
            RefTarget::Section(key) if links.navigable => NodeLink::Section(key),
            _ => NodeLink::Disabled,
        });
    }
    // An external link is dropped when `--no-external` is in force.
    match node.attr("href") {
        Some(href) if !options.no_external => Some(NodeLink::External(href.to_string())),
        _ => None,
    }
}

/// The `target` of an external link, as in HTML (`target = "_blank"` opens a
/// new tab). Useful in the live preview, where a link without a target would
/// load in the preview frame, which many sites refuse to be shown in. A link
/// that opens a new window never gets the opener (`rel`).
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

/// The nodes.
fn draw_nodes(
    out: &mut String,
    text: &mut TextMeasurer,
    layout: &Layout,
    options: &RenderOptions,
    links: &Links,
) {
    for (index, node) in layout.nodes.iter().enumerate() {
        let edge_size = layout
            .edges
            .iter()
            .find(|edge| edge.child == index)
            .and_then(|edge| edge.label.as_ref())
            .map(|(_, box_)| {
                format!(
                    " data-edge-w=\"{}\" data-edge-h=\"{}\"",
                    box_.width(),
                    box_.height()
                )
            })
            .unwrap_or_default();
        let edge_dashed = if layout::has_dashed_edge(node) {
            " data-edge-dashed=\"1\""
        } else {
            ""
        };
        out.push_str(&format!(
            "<g class=\"tree-node\" data-id=\"{}\" data-parent=\"{}\" data-depth=\"{}\" \
             data-x=\"{}\" data-y=\"{}\" data-w=\"{}\" data-h=\"{}\" data-children=\"{}\" \
             data-collapsed=\"{}\"{edge_size}{edge_dashed}{}>\n",
            node_id(index),
            node.parent.map(node_id).unwrap_or_default(),
            node.depth,
            node.box_.x1,
            node.box_.y1,
            node.box_.width(),
            node.box_.height(),
            u8::from(node.has_children),
            u8::from(node.collapsed),
            if node.visible {
                ""
            } else {
                " display=\"none\""
            }
        ));

        let link = link_of(node, options, links);
        match &link {
            Some(NodeLink::External(href)) => {
                out.push_str(&format!(
                    "<a href=\"{}\"{}>\n",
                    svg::escape_xml(href),
                    link_target(node)
                ));
            }
            Some(NodeLink::Section(key)) => {
                out.push_str(&format!(
                    "<a href=\"#{0}\" data-target=\"{0}\">\n",
                    svg::escape_xml(key)
                ));
            }
            Some(NodeLink::Disabled) => {
                out.push_str("<g aria-disabled=\"true\" data-link-disabled=\"true\">\n");
            }
            None => {}
        }

        let default_fill = if node.parent.is_none() {
            ROOT_FILL
        } else {
            NODE_FILL
        };
        let fill = paint(node.attr("color"), default_fill);
        let stroke = paint(node.attr("linecolor"), NODE_LINE);
        let style = svg::dash_style(node.attr("style"));
        let rounded = node
            .attr("shape")
            .map(|shape| shape.eq_ignore_ascii_case("roundedbox"))
            .unwrap_or(false);
        if rounded {
            svg::rounded_rect(out, &node.box_, CORNER_RADIUS, &fill, &stroke, &style);
        } else {
            svg::rect(out, &node.box_, &fill, &stroke, &style);
        }

        let color = paint(node.attr("textcolor"), TEXT_COLOR);
        let anchored = matches!(
            link,
            Some(NodeLink::External(_)) | Some(NodeLink::Section(_))
        );
        draw_text(
            out,
            text,
            &inner_box(node),
            &node.label,
            &color,
            options,
            anchored,
        );

        match link {
            Some(NodeLink::External(_)) | Some(NodeLink::Section(_)) => out.push_str("</a>\n"),
            Some(NodeLink::Disabled) => out.push_str("</g>\n"),
            None => {}
        }
        out.push_str("</g>\n");
    }
}

/// The circles that fold and unfold the branches: a minus while the children
/// are shown, a plus while they are folded away.
fn draw_toggles(out: &mut String, layout: &Layout) {
    let r = TOGGLE_RADIUS;
    let arm = r - 3;
    for (index, node) in layout.nodes.iter().enumerate() {
        let Some((cx, cy)) = node.toggle else {
            continue;
        };
        out.push_str(&format!(
            "<g class=\"tree-toggle\" data-for=\"{}\" data-x=\"{cx}\" data-y=\"{cy}\" \
             role=\"button\" tabindex=\"0\" cursor=\"pointer\" aria-label=\"{}\"{}>\n",
            node_id(index),
            if node.collapsed { "Expand" } else { "Collapse" },
            if node.visible {
                ""
            } else {
                " display=\"none\""
            }
        ));
        out.push_str(&format!(
            "<circle cx=\"{cx}\" cy=\"{cy}\" r=\"{r}\" fill=\"#ffffff\" stroke=\"{NODE_LINE}\"/>\n"
        ));
        svg::line(out, cx - arm, cy, cx + arm, cy, NODE_LINE, "");
        out.push_str(&format!(
            "<line class=\"tree-plus\" x1=\"{cx}\" y1=\"{}\" x2=\"{cx}\" y2=\"{}\" \
             stroke=\"{NODE_LINE}\"{}/>\n",
            cy - arm,
            cy + arm,
            if node.collapsed {
                ""
            } else {
                " display=\"none\""
            }
        ));
        out.push_str("</g>\n");
    }
}

/// The part of a node the text is drawn in.
fn inner_box(node: &NodeLayout) -> PixelBox {
    PixelBox {
        x1: node.box_.x1 + PADDING_X,
        y1: node.box_.y1,
        x2: node.box_.x2 - PADDING_X,
        y2: node.box_.y2,
    }
}

fn draw_text(
    out: &mut String,
    text: &mut TextMeasurer,
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
    svg::draw_text(out, text, box_, value, &style);
}

#[cfg(test)]
mod tests {
    use super::super::parser::parse;
    use super::*;

    fn svg_of(source: &str) -> String {
        let document = parse(source).unwrap();
        render(&document.trees[0], &RenderOptions::default())
    }

    #[test]
    fn the_root_is_yellow_and_the_children_are_white() {
        let svg = svg_of("tree { root R; A; }");
        assert!(svg.contains("fill=\"#ffff99\""), "{svg}");
        assert!(svg.contains("fill=\"#ffffff\""), "{svg}");
    }

    #[test]
    fn colors_and_links_are_applied() {
        let svg = svg_of("tree { A [color = \"#cfe8ff\", href = \"https://example.com\"]; }");
        assert!(svg.contains("#cfe8ff"), "{svg}");
        assert!(svg.contains("<a href=\"https://example.com\">"), "{svg}");
    }

    #[test]
    fn target_opens_an_external_link_in_a_new_tab() {
        let svg = svg_of("tree { A [href = \"https://example.com\", target = _blank]; }");
        assert!(
            svg.contains(
                "<a href=\"https://example.com\" target=\"_blank\" rel=\"noopener noreferrer\">"
            ),
            "{svg}"
        );
        let plain = svg_of("tree { A [href = \"https://example.com\"]; }");
        assert!(!plain.contains("target="), "{plain}");
        let own = svg_of("tree { A [href = \"https://example.com\", target = _self]; }");
        assert!(own.contains("target=\"_self\">"), "{own}");
    }

    #[test]
    fn no_external_drops_the_link() {
        let document = parse("tree { A [href = \"https://example.com\"]; }").unwrap();
        let options = RenderOptions {
            no_external: true,
            ..RenderOptions::default()
        };
        assert!(!render(&document.trees[0], &options).contains("<a "));
    }

    #[test]
    fn a_dashed_edge_is_drawn_dashed_until_it_merges_with_a_solid_one() {
        let svg = svg_of("tree { A [edgestyle = dashed]; B; C [edgestyle = dashed]; }");
        let line = |class: &str| {
            svg.lines()
                .find(|line| line.contains(class))
                .unwrap_or_default()
                .to_string()
        };
        let dashed = "stroke-dasharray=\"4\"";
        // A's drop is dashed; the bus is solid up to C, where only C uses it.
        assert!(
            svg.contains("class=\"tree-drop\" data-for=\"n1\"")
                && line("data-for=\"n1\" d").contains(dashed)
        );
        assert!(!line("class=\"tree-stem\"").contains(dashed), "{svg}");
        assert!(line("class=\"tree-bus-dashed\"").contains(dashed), "{svg}");
        assert!(!line("class=\"tree-bus\"").contains(dashed), "{svg}");
    }

    #[test]
    fn solid_trees_have_no_dashed_bus() {
        assert!(!svg_of("tree { A; B; }").contains("stroke-dasharray"));
    }

    #[test]
    fn edge_labels_are_drawn() {
        let svg = svg_of("tree { Q { A [edge = yes]; } }");
        assert!(svg.contains(">yes</text>"), "{svg}");
    }
}
