//! Integration tests for the manifest diagram: membership, inheritance through
//! `extends`, the SVG, the bundle runtime and the examples.

use std::path::PathBuf;
use std::process::Command;

mod common;

use swimflow::build::LoadOptions;
use swimflow::core::overlay::Overlay;
use swimflow::plugin::manifest::ManifestPlugin;
use swimflow::plugin::manifest::model::{self, Mark};
use swimflow::plugin::manifest::parser;
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
    ManifestPlugin.render(&context).expect("render")
}

fn svg(source: &str) -> String {
    render(source, RenderMode::Standalone)
        .first()
        .expect("a section")
        .svg
        .clone()
}

const PRODUCTS: &str = "manifest \"P\" {\n  group \"Line\" {\n    column base { core; ui; net; }\n    \
column lite [extends = base] { remove net; }\n    \
column pro [extends = base] { override ui [note = v2]; cloud; }\n  }\n  \
column other { core; }\n  row docs: base, other;\n}\n";

#[test]
fn a_manifest_renders_one_cell_per_row_and_column() {
    let svg = svg(PRODUCTS);
    assert!(svg.contains("class=\"swimflow-manifest\""), "{svg}");
    // 5 rows (core, ui, net, cloud, docs) x 4 columns.
    assert_eq!(svg.matches("class=\"manifest-cell\"").count(), 20, "{svg}");
    assert_eq!(svg.matches("class=\"manifest-row\"").count(), 5, "{svg}");
    assert_eq!(svg.matches("class=\"manifest-link\"").count(), 2, "{svg}");
    usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("valid SVG");
}

#[test]
fn inheritance_is_resolved_into_marks_and_counts() {
    let document = model::build(&parser::parse(PRODUCTS).unwrap()).unwrap();
    let d = &document.diagrams[0];
    let row = |name: &str| d.rows.iter().position(|r| r.name == name).unwrap();
    let col = |name: &str| d.columns.iter().position(|c| c.name == name).unwrap();
    assert_eq!(
        d.cells[row("net")][col("lite")].mark,
        Mark::Removed { from: col("base") }
    );
    assert_eq!(
        d.cells[row("ui")][col("pro")].mark,
        Mark::Override { from: col("base") }
    );
    assert_eq!(
        d.cells[row("core")][col("lite")].mark,
        Mark::Inherited { from: col("base") }
    );
    assert_eq!(d.rows[row("core")].count, 4);
    assert_eq!(d.rows[row("net")].count, 2);
    // The most shared row comes first.
    assert_eq!(d.row_order()[0], row("core"));
}

#[test]
fn the_layout_and_the_svg_are_deterministic() {
    assert_eq!(svg(PRODUCTS), svg(PRODUCTS));
}

#[test]
fn every_format_is_produced() {
    let rendered = render(PRODUCTS, RenderMode::Standalone);
    let options = RenderOptions::default();
    for format in [
        swimflow::render::Format::Svg,
        swimflow::render::Format::Png,
        swimflow::render::Format::Pdf,
        swimflow::render::Format::Html,
    ] {
        let bytes = swimflow::plugin::render_output(format, &rendered, &options).expect("output");
        assert!(!bytes.is_empty(), "{format:?}");
    }
}

#[test]
fn the_runtime_highlights_and_filters_against_the_fake_dom() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("node not installed; skipping the manifest runtime test");
        return;
    }
    let dir = common::TempDir::new("swimflow-manifest-runtime-");
    let rendered = render(PRODUCTS, RenderMode::Bundle);
    let section = &rendered.sections[0];
    let path = dir.join("section.html");
    let page = format!(
        "<div class=\"swimflow-toolbar-slot\" data-diagram=\"manifest\">{}</div>\
         <section class=\"swimflow-diagram\" id=\"manifest\">{}</section>",
        section.toolbar, section.svg
    );
    std::fs::write(&path, page).expect("write section");

    let harness = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/manifest_runtime.js");
    let output = Command::new("node")
        .arg(&harness)
        .arg(&path)
        .output()
        .expect("run node");
    assert!(
        output.status.success(),
        "manifest runtime harness failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn the_manifest_examples_render() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/manifest");
    let mut seen = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("mfst") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        let svg = svg(&source);
        usvg::Tree::from_str(&svg, &usvg::Options::default())
            .unwrap_or_else(|e| panic!("invalid SVG for {}: {e}", path.display()));
        seen += 1;
    }
    assert!(seen >= 2, "expected the manifest examples");
}

#[test]
fn removed_marks_are_left_out_of_a_plain_export_unless_asked_for() {
    let source = "manifest { column a { x; } column b [extends = a] { remove x; } }";
    let svg = svg(source);
    assert!(svg.contains("data-mark=\"removed\""), "{svg}");
    assert!(
        !svg.contains("stroke=\"#999999\""),
        "no cross is drawn: {svg}"
    );
    assert!(!svg.contains(">removed</text>"), "no legend entry: {svg}");

    let shown = self::svg(&source.replace("manifest {", "manifest { hidemarks = none;"));
    assert!(shown.contains("stroke=\"#999999\""), "{shown}");
}

#[test]
fn a_row_whose_columns_are_all_hidden_is_left_out() {
    let source = "manifest { column a { x; } column b { y; } }";
    let options = RenderOptions {
        view: swimflow::core::view::ViewState {
            hidden: [("default".to_string(), ["c1".to_string()].into())].into(),
            ..Default::default()
        },
        ..RenderOptions::default()
    };
    let load = LoadOptions::default();
    let overlay = Overlay::new();
    let context = RenderContext::new(source, &options, &load, &overlay);
    let svg = ManifestPlugin.render(&context).unwrap().sections[0]
        .svg
        .clone();
    assert!(svg.contains("data-row=\"r0\""), "{svg}");
    assert!(!svg.contains("data-row=\"r1\""), "{svg}");
}
