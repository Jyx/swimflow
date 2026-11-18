//! SVG renderer tests: `usvg` validation, option behaviour and snapshots.
//!
//! Snapshots are normalized through `usvg` before comparison and use `--text-as-paths` so the expected bytes
//! are independent of system fonts. Regenerate with:
//!
//! ```console
//! $ UPDATE_GOLDEN=1 cargo test --test svg
//! ```

use std::path::PathBuf;

use swimflow::build;
use swimflow::render::{self, Format, RenderOptions};

fn render(src: &str, options: &RenderOptions) -> String {
    let set = build::build_str(src).expect("build");
    let mut out = Vec::new();
    render::render(Format::Svg, &set, &mut out, options).expect("render");
    String::from_utf8(out).expect("utf-8")
}

fn paths_options() -> RenderOptions {
    RenderOptions {
        text_as_paths: true,
        ..RenderOptions::default()
    }
}

/// Parse with `usvg` and re-serialize, normalizing XML formatting.
fn normalize(svg: &str) -> String {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).expect("usvg parse");
    let options = usvg::WriteOptions {
        preserve_text: true,
        ..usvg::WriteOptions::default()
    };
    tree.to_string(&options)
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(format!("{name}.svg"))
}

fn assert_snapshot(name: &str, src: &str) {
    let normalized = normalize(&render(src, &paths_options()));
    let path = golden_path(name);

    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &normalized).unwrap();
        return;
    }

    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing golden {} (run UPDATE_GOLDEN=1)", path.display()));
    assert_eq!(
        expected, normalized,
        "SVG snapshot mismatch for {name} (run UPDATE_GOLDEN=1 to update)"
    );
}

const SIMPLE: &str = include_str!("fixtures/simple.diag");
const BRANCHING: &str = "{ A -> B; A -> C; B -> D; C -> D; }";
const CYCLE: &str = "{ lane a { A; } lane b { B; C; } A -> B; B -> C; C -> A; }";
const EDGE_LABELS: &str = "{ A -> B [label = \"commit\"]; B -> C [label = \"pass\"]; }";
const NUMBERS: &str = "{ numbering = topological; A -> B -> C; }";
const LEGEND: &str = "diagram D { numbering = auto; \
    class approve [color = \"#fff3cd\", shape = roundedbox]; \
    legend [title = \"Legend\"] { class approve [label = \"Approval\"]; \
    entry \"Automated\" [color = \"#e8f5e9\"]; numbered; } \
    lane CI { A -> B; } lane Ops { C; } B -> C; }";
const LANDSCAPE: &str = "{ orientation = landscape; A -> B; A -> C; }";

#[test]
fn default_output_parses_with_usvg() {
    let svg = render(SIMPLE, &RenderOptions::default());
    assert!(svg.contains("<svg"));
    assert!(svg.contains("<text"));
    // Must not panic and must round-trip through usvg.
    let _ = usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("usvg parse");
}

#[test]
fn text_as_paths_has_no_text_and_no_font_dependency() {
    let svg = render(SIMPLE, &paths_options());
    assert!(!svg.contains("<text"), "text-as-paths must not emit <text>");
    assert!(!svg.contains("@font-face"));
    assert!(!svg.contains("font-family"));
    assert!(svg.contains("<path"), "expected glyph outlines");
    usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("usvg parse");
}

#[test]
fn embed_font_is_self_contained() {
    let options = RenderOptions {
        embed_font: true,
        ..RenderOptions::default()
    };
    let svg = render(SIMPLE, &options);
    assert!(svg.contains("@font-face"));
    assert!(svg.contains("base64,"));
    assert!(svg.contains("font-family=\"VL Gothic, sans-serif\""));
    assert!(!svg.contains("url(http"), "embedded font must not fetch");
    usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("usvg parse");
}

#[test]
fn selected_font_family_flows_into_svg() {
    let font = swimflow::text::FontSpec {
        family: "Swimflow Test".to_string(),
        data: swimflow::text::DEFAULT_FONT.to_vec().into(),
    };
    let options = RenderOptions {
        embed_font: true,
        font,
        ..RenderOptions::default()
    };
    let svg = render(SIMPLE, &options);
    assert!(svg.contains("font-family=\"Swimflow Test, sans-serif\""));
    assert!(svg.contains("@font-face{font-family:'Swimflow Test'"));
    usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("usvg parse");
}

#[test]
fn text_requests_geometric_precision() {
    let svg = render(SIMPLE, &RenderOptions::default());
    assert!(svg.contains("text-rendering=\"geometricPrecision\""));
}

#[test]
fn nodoctype_and_noviewbox_are_honoured() {
    let svg = render(
        SIMPLE,
        &RenderOptions {
            nodoctype: true,
            noviewbox: true,
            ..RenderOptions::default()
        },
    );
    assert!(!svg.contains("<!DOCTYPE"));
    assert!(!svg.contains("viewBox"));
    assert!(svg.contains("width=\"448\""));
    assert!(svg.contains("height=\"360\""));
}

#[test]
fn transparent_omits_the_background() {
    let transparent = render(
        SIMPLE,
        &RenderOptions {
            transparent: true,
            ..RenderOptions::default()
        },
    );
    let opaque = render(SIMPLE, &RenderOptions::default());
    assert!(!transparent.contains("<rect x=\"0\" y=\"0\""));
    assert!(opaque.contains("<rect x=\"0\" y=\"0\""));
}

#[test]
fn rendering_is_deterministic() {
    assert_eq!(
        render(SIMPLE, &paths_options()),
        render(SIMPLE, &paths_options())
    );
    assert_eq!(
        render(SIMPLE, &RenderOptions::default()),
        render(SIMPLE, &RenderOptions::default())
    );
}

#[test]
fn label_escaping_and_unicode() {
    let svg = render(
        r#"{ A [label = "a < b & c 世界"]; }"#,
        &RenderOptions::default(),
    );
    assert!(svg.contains("&lt;"));
    assert!(svg.contains("&amp;"));
    assert!(svg.contains("世界"));
    assert!(!svg.contains("a < b"));
    usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("usvg parse");
}

#[test]
fn snapshot_simple() {
    assert_snapshot("simple", SIMPLE);
}

#[test]
fn snapshot_branching() {
    assert_snapshot("branching", BRANCHING);
}

#[test]
fn snapshot_cycle() {
    assert_snapshot("cycle", CYCLE);
}

#[test]
fn snapshot_edge_labels() {
    assert_snapshot("edge_labels", EDGE_LABELS);
}

#[test]
fn snapshot_numbers() {
    assert_snapshot("numbers", NUMBERS);
}

#[test]
fn snapshot_legend() {
    assert_snapshot("legend", LEGEND);
}

#[test]
fn snapshot_landscape() {
    assert_snapshot("landscape", LANDSCAPE);
}

#[test]
fn landscape_lane_header_is_not_cropped() {
    // metrics.pagesize() is 640x200, but the frame header is drawn at y=-42.
    // The renderer must grow the canvas and shift the content instead of
    // cropping the yellow lane header (and its "Lane 1" label).
    let svg = render(LANDSCAPE, &RenderOptions::default());
    assert!(svg.contains("viewBox=\"0 0 640 242\""), "{svg}");
    assert!(svg.contains("translate(0,42)"), "{svg}");
    assert!(svg.contains(">Lane 1<"), "lane label was cropped: {svg}");
    usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("usvg parse");

    // `--noviewbox` must use the same corrected dimensions.
    let no_viewbox = render(
        LANDSCAPE,
        &RenderOptions {
            noviewbox: true,
            ..RenderOptions::default()
        },
    );
    assert!(!no_viewbox.contains("viewBox"));
    assert!(
        no_viewbox.contains("width=\"640\" height=\"242\""),
        "{no_viewbox}"
    );
}

#[test]
fn landscape_viewport_contains_all_content() {
    // Every geometry point, once translated by the group offset, must land
    // inside the canvas. This guards against cropping on any side.
    let svg = render(LANDSCAPE, &RenderOptions::default());
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("usvg parse");
    let size = tree.size();
    assert_eq!(size.width(), 640.0);
    assert_eq!(size.height(), 242.0);

    fn walk_group(group: &usvg::Group, size: usvg::Size) {
        check(group.abs_bounding_box(), size);
        for child in group.children() {
            walk_node(child, size);
        }
    }
    fn walk_node(node: &usvg::Node, size: usvg::Size) {
        check(node.abs_bounding_box(), size);
        if let usvg::Node::Group(group) = node {
            walk_group(group, size);
        }
    }
    fn check(bbox: usvg::Rect, size: usvg::Size) {
        assert!(
            bbox.left() >= -0.01
                && bbox.top() >= -0.01
                && bbox.right() <= size.width() + 0.01
                && bbox.bottom() <= size.height() + 0.01,
            "geometry {bbox:?} escapes the canvas {size:?}"
        );
    }
    walk_group(tree.root(), size);
}

#[test]
fn sequence_badges_render() {
    let svg = render(NUMBERS, &RenderOptions::default());
    assert_eq!(svg.matches("<ellipse").count(), 3, "one badge per node");
}

#[test]
fn edge_labels_render_text_and_halo() {
    let svg = render(EDGE_LABELS, &RenderOptions::default());
    assert!(svg.contains("commit"));
    assert!(svg.contains("pass"));
    assert!(svg.contains("fill=\"#ffffff\""), "label halo");
}

#[test]
fn horizontal_edge_label_splits_the_shaft() {
    // A straight horizontal edge with room splits into two shaft fragments and
    // centres the text in the gap ("---- label ---->").
    let svg = render(
        "{ orientation = landscape; A -> B [label = \"abcdef\"]; }",
        &RenderOptions::default(),
    );
    assert!(svg.contains(">abcdef<"), "{svg}");
    assert!(
        svg.matches("<path d=\"M").count() >= 2,
        "shaft should be split around the inline label: {svg}"
    );
    assert!(svg.contains("stroke=\"none\""), "halo has no border: {svg}");
}

#[test]
fn legend_extends_the_page_and_lists_entries() {
    let svg = render(LEGEND, &RenderOptions::default());
    assert!(svg.contains(">Legend<"));
    assert!(svg.contains(">Approval<"));
    assert!(svg.contains(">Automated<"));
    assert!(svg.contains(">Sequence number<"));
    // The legend is a band: the page grows beyond the diagram's 360px height.
    let height = svg
        .split_once("height=\"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .and_then(|(value, _)| value.parse::<i32>().ok())
        .expect("height");
    assert!(height > 360, "legend should extend the page, got {height}");
}

#[test]
fn ref_nodes_become_anchor_links() {
    let svg = render(
        "diagram main { A [ref = \"#detail\"]; } diagram detail { B; }",
        &RenderOptions::default(),
    );
    assert!(svg.contains("<a href=\"#detail\" data-target=\"detail\""));
    assert!(svg.contains("aria-label=\"A: go to diagram detail\""));
    assert!(svg.contains("</a>"));
    usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("usvg parse");
}

#[test]
fn linked_node_labels_are_underlined() {
    let src = "diagram main { A [ref = \"#detail\"]; B [href = \"https://example.com\"]; } \
               diagram detail { C; }";
    let linked = render(src, &RenderOptions::default());
    assert_eq!(
        linked.matches("data-link-underline=\"true\"").count(),
        2,
        "internal refs and external hrefs should both be underlined"
    );
    assert!(linked.contains("<text"));

    let as_paths = render(src, &paths_options());
    assert_eq!(
        as_paths.matches("data-link-underline=\"true\"").count(),
        2,
        "underlines must survive --text-as-paths"
    );
    assert!(!as_paths.contains("<text"));

    let plain = render("{ A; }", &RenderOptions::default());
    assert!(!plain.contains("data-link-underline"));

    let gated = render(
        "{ A [href = \"https://example.com\"]; }",
        &RenderOptions {
            no_external: true,
            ..RenderOptions::default()
        },
    );
    assert!(
        !gated.contains("data-link-underline"),
        "disabled links must not look active"
    );
}

#[test]
fn external_href_becomes_a_link_unless_suppressed() {
    let src = "{ A [href = \"https://example.com\"]; }";
    let linked = render(src, &RenderOptions::default());
    assert!(linked.contains("href=\"https://example.com\""));

    let gated = render(
        src,
        &RenderOptions {
            no_external: true,
            ..RenderOptions::default()
        },
    );
    assert!(!gated.contains("href=\"https://example.com\""));
    assert!(gated.contains("data-link-disabled=\"true\""));
    usvg::Tree::from_str(&gated, &usvg::Options::default()).expect("usvg parse");
}
