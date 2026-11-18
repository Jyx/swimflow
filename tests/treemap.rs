//! Integration tests for the treemap diagram: values, nested rectangles,
//! labels, the bundle value toggle and its browser runtime.

use std::path::PathBuf;
use std::process::Command;

mod common;

use swimflow::build::LoadOptions;
use swimflow::core::html;
use swimflow::core::overlay::Overlay;
use swimflow::plugin::treemap::TreemapPlugin;
use swimflow::plugin::treemap::layout::{self, Layout};
use swimflow::plugin::treemap::model;
use swimflow::plugin::treemap::parser;
use swimflow::plugin::{DiagramPlugin, RenderContext, RenderMode, Rendered};
use swimflow::render::RenderOptions;

fn render(source: &str, mode: RenderMode) -> Rendered {
    let options = RenderOptions::default();
    let load = LoadOptions::default();
    let overlay = Overlay::new();
    let context = RenderContext {
        source,
        path: None,
        options: &options,
        mode,
        load: &load,
        pixels: false,
        overlay: &overlay,
    };
    TreemapPlugin.render(&context).expect("render")
}

fn svg(source: &str) -> String {
    render(source, RenderMode::Standalone)
        .first()
        .expect("a section")
        .svg
        .clone()
}

fn layout_of(source: &str) -> Layout {
    let ast = parser::parse(source).expect("parse");
    let document = model::build(&ast).unwrap();
    layout::layout(&document.diagrams[0])
}

fn area(box_: &swimflow::core::geom::PixelBox) -> i64 {
    box_.width() as i64 * box_.height() as i64
}

#[test]
fn a_treemap_renders_nested_rectangles() {
    let svg = svg("treemap { A: 2 { A1: 1; A2: 1; } B: 1; }");
    assert!(svg.contains("<svg"), "{svg}");
    assert!(svg.contains("class=\"swimflow-treemap\""), "{svg}");
    assert_eq!(svg.matches("class=\"treemap-node\"").count(), 5, "{svg}");
    usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("valid SVG");
}

#[test]
fn areas_follow_the_numbered_parameter() {
    let layout = layout_of("treemap { A: 1; B: 4; C: 2; }");
    let a = area(&layout.nodes[1].box_);
    let b = area(&layout.nodes[2].box_);
    let c = area(&layout.nodes[3].box_);
    assert!(b > c && c > a, "{a} {c} {b}");
    let ratio = b as f64 / a as f64;
    assert!((ratio - 4.0).abs() < 0.75, "area ratio was {ratio}");
}

#[test]
fn every_child_stays_inside_its_parent_and_siblings_do_not_overlap() {
    let layout =
        layout_of("treemap { A: 10 { A1: 4; A2: 6; } B: 5 { B1: 1; B2: 2; B3: 2; } C: 2; }");
    for node in &layout.nodes {
        if let Some(parent) = node.parent {
            let p = &layout.nodes[parent].box_;
            assert!(node.box_.x1 >= p.x1, "{node:?} outside {p:?}");
            assert!(node.box_.y1 >= p.y1, "{node:?} outside {p:?}");
            assert!(node.box_.x2 <= p.x2, "{node:?} outside {p:?}");
            assert!(node.box_.y2 <= p.y2, "{node:?} outside {p:?}");
        }
    }
    for left in 1..layout.nodes.len() {
        for right in left + 1..layout.nodes.len() {
            if layout.nodes[left].parent != layout.nodes[right].parent {
                continue;
            }
            let a = &layout.nodes[left].box_;
            let b = &layout.nodes[right].box_;
            let overlap = a.x1 < b.x2 && b.x1 < a.x2 && a.y1 < b.y2 && b.y1 < a.y2;
            assert!(!overlap, "{a:?} overlaps {b:?}");
        }
    }
}

#[test]
fn a_leaf_without_a_value_counts_one_and_a_parent_sums_its_children() {
    let layout = layout_of("treemap { A; B: 3; C { D; E; } }");
    assert_eq!(layout.nodes[1].value, 1.0);
    assert_eq!(layout.nodes[2].value, 3.0);
    assert_eq!(layout.nodes[3].value, 2.0);
}

#[test]
fn values_are_drawn_by_default_and_showvalue_can_hide_one() {
    let shown = svg("treemap { A: 42; }");
    assert!(shown.contains("class=\"treemap-value\""), "{shown}");
    assert!(shown.contains(">42</text>"), "{shown}");

    let hidden = svg("treemap { showvalue = false; A: 42; }");
    assert!(!hidden.contains("class=\"treemap-value\""), "{hidden}");
    assert!(hidden.contains("<title>A: 42</title>"), "{hidden}");

    let one = svg("treemap { A: 42 [showvalue = false]; B: 1; }");
    let a = one
        .split("<g class=\"treemap-node\"")
        .find(|node| node.contains("data-label=\"A\""))
        .expect("node A");
    assert!(!a.contains("class=\"treemap-value\""), "{a}");
}

#[test]
fn labels_are_truncated_with_three_ascii_dots() {
    let svg = svg("treemap { width = 400; height = 300; \
         \"a very long label that cannot possibly fit\" : 1; \
         B: 1; }");
    assert!(svg.contains("..."), "{svg}");
    assert!(!svg.contains('…'), "{svg}");
}

#[test]
fn classes_give_reusable_colors_and_the_node_can_override_them() {
    let svg = svg(
        "treemap { class warm [color = \"#fff3bf\", textcolor = \"#5c4b00\"]; \
         A: 1 [class = warm]; B: 1 [class = warm, color = red]; }",
    );
    assert!(svg.contains("#fff3bf"), "{svg}");
    assert!(svg.contains("fill=\"#5c4b00\""), "{svg}");
    assert!(svg.contains("fill=\"red\""), "{svg}");
}

#[test]
fn an_unknown_class_is_reported() {
    let options = RenderOptions::default();
    let load = LoadOptions::default();
    let overlay = Overlay::new();
    let context = RenderContext {
        source: "treemap { A [class = nope]; }",
        path: None,
        options: &options,
        mode: RenderMode::Standalone,
        load: &load,
        pixels: false,
        overlay: &overlay,
    };
    let err = TreemapPlugin.render(&context).unwrap_err();
    assert!(err.to_string().contains("unknown class"), "{err}");
}

#[test]
fn links_targets_and_colors_are_applied() {
    let svg = svg(
        "treemap { A: 2 [color = \"#cfe8ff\", textcolor = red, linecolor = blue, \
         href = \"https://example.com\", target = _blank]; }",
    );
    assert!(svg.contains("#cfe8ff"), "{svg}");
    assert!(svg.contains("fill=\"red\""), "{svg}");
    assert!(svg.contains("stroke=\"blue\""), "{svg}");
    assert!(
        svg.contains(
            "<a href=\"https://example.com\" target=\"_blank\" rel=\"noopener noreferrer\">"
        ),
        "{svg}"
    );
}

#[test]
fn a_bundle_gets_one_value_toggle_per_treemap_and_a_file_gets_none() {
    let source = "treemap one { A: 2; } treemap two { B: 3; }";
    let bundle = render(source, RenderMode::Bundle);
    let page = html::bundle(&bundle.sections, "one", "");
    assert_eq!(
        page.matches("class=\"treemap-values-toggle\"").count(),
        2,
        "{page}"
    );
    assert_eq!(page.matches("function setValues").count(), 1, "{page}");

    let standalone = render(source, RenderMode::Standalone);
    assert!(standalone.sections.iter().all(|s| s.toolbar.is_empty()));
    assert!(standalone.sections.iter().all(|s| s.scripts.is_empty()));
}

#[test]
fn a_treemap_without_values_has_no_toggle() {
    let bundle = render("treemap { showvalue = false; A: 2; }", RenderMode::Bundle);
    assert!(bundle.sections[0].toolbar.is_empty());
    assert!(bundle.sections[0].scripts.is_empty());
}

#[test]
fn the_layout_and_the_svg_are_deterministic() {
    let source = "treemap { A: 2 { A1: 1; A2: 1; } B: 3; C: 4; }";
    assert_eq!(layout_of(source), layout_of(source));
    assert_eq!(svg(source), svg(source));
}

#[test]
fn the_value_toggle_runs_against_the_fake_dom() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("node not installed; skipping the treemap runtime test");
        return;
    }
    let dir = common::TempDir::new("swimflow-treemap-runtime-");
    let rendered = render(
        "treemap \"P\" { root \"Portfolio\"; A: 2 { A1: 1; } B: 3; }",
        RenderMode::Bundle,
    );
    let section = &rendered.sections[0];
    let path = dir.join("section.html");
    let page = format!(
        "<div class=\"swimflow-toolbar-slot\" data-diagram=\"treemap\">{}</div>\
         <section class=\"swimflow-diagram\" id=\"treemap\">{}</section>",
        section.toolbar, section.svg
    );
    std::fs::write(&path, page).expect("write section");

    let harness = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/treemap_runtime.js");
    let output = Command::new("node")
        .arg(&harness)
        .arg(&path)
        .output()
        .expect("run node");
    assert!(
        output.status.success(),
        "treemap runtime harness failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn the_treemap_examples_render() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/treemap");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("tmap") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        let svg = svg(&source);
        assert!(svg.contains("<svg"), "{}", path.display());
        usvg::Tree::from_str(&svg, &usvg::Options::default())
            .unwrap_or_else(|e| panic!("invalid SVG for {}: {e}", path.display()));
    }
}
