//! Interactive drill-down bundle.
//!
//! The bundle is a single self-contained HTML file, so it is checked
//! structurally: diagram sections and their ids, `data-target` links that match
//! those ids, root selection, link gating, and the presence of the navigation
//! runtime. No browser is required; a structural check stands in for
//! a headless test.

use std::path::PathBuf;
use std::process::Command;

mod common;

use swimflow::build::{self, LoadOptions, load_file};
use swimflow::render::RenderOptions;
use swimflow::render::bundle;

fn nested(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/nested")
        .join(name)
}

fn render_str(src: &str, options: &RenderOptions) -> String {
    let set = build::build_str(src).expect("build");
    bundle::render(&set, options).expect("bundle")
}

#[test]
fn same_file_drilldown_inlines_both_diagrams() {
    let src = include_str!("fixtures/drilldown.diag");
    let html = render_str(src, &RenderOptions::default());

    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("class=\"swimflow-diagram\""));
    assert!(html.contains("id=\"overview\""));
    assert!(html.contains("id=\"detail\""));
    assert!(html.contains("data-diagram=\"overview\""));
    assert!(html.contains("data-diagram=\"detail\""));
    assert!(html.contains("data-target=\"detail\""));
    assert!(html.contains("href=\"#detail\""));
    assert!(html.contains("data-root=\"overview\""));

    // Exactly the root diagram is marked as the entry point.
    assert_eq!(html.matches("data-root=\"true\"").count(), 1);
}

#[test]
fn deep_link_id_matches_the_diagram_key() {
    // `out.html#detail` relies on the section id being the diagram id.
    let html = render_str(
        "diagram overview { A [ref = \"#detail\"]; } diagram detail { B; }",
        &RenderOptions::default(),
    );
    assert!(html.contains("id=\"detail\""));
    assert!(
        html.contains("location.hash"),
        "runtime routes the URL hash"
    );
}

#[test]
fn every_data_target_resolves_to_a_section() {
    let html = render_str(
        "diagram a { A [ref = \"#b\"]; } diagram b { B [ref = \"#c\"]; } diagram c { C; }",
        &RenderOptions::default(),
    );

    for target in data_targets(&html) {
        assert!(
            html.contains(&format!("id=\"{target}\"")),
            "data-target {target:?} has no matching section id"
        );
    }
}

#[test]
fn cross_file_references_are_inlined() {
    let set = load_file(nested("root.diag"), &LoadOptions::default()).expect("load");
    let html = bundle::render(&set, &RenderOptions::default()).expect("bundle");

    assert!(html.contains("id=\"main\""));
    assert!(html.contains("id=\"detail\""));
    assert!(html.contains("data-target=\"detail\""));
    assert!(html.contains("Prepare"));
}

#[test]
fn cyclic_references_both_directions_are_present() {
    let set = load_file(nested("cycle_a.diag"), &LoadOptions::default()).expect("load");
    let html = bundle::render(&set, &RenderOptions::default()).expect("bundle");

    // Both A -> B and B -> A are emitted; generation is static, so a cycle
    // cannot recurse.
    assert!(html.contains("data-diagram=\"a\""));
    assert!(html.contains("data-diagram=\"b\""));
    assert!(html.contains("data-target=\"b\""));
    assert!(html.contains("data-target=\"a\""));
    assert_eq!(html.matches("class=\"swimflow-diagram\"").count(), 2);
}

#[test]
fn broken_links_render_but_are_inert() {
    let options = LoadOptions {
        allow_broken_links: true,
        ..LoadOptions::default()
    };
    let set = load_file(nested("broken.diag"), &options).expect("load");
    let html = bundle::render(&set, &RenderOptions::default()).expect("bundle");

    assert!(html.contains("id=\"main\""));
    // The broken node is drawn but has no href and is marked disabled.
    assert!(html.contains("data-link-disabled=\"true\""));
    assert!(!html.contains("data-target=\"nope\""));
}

#[test]
fn no_external_suppresses_external_links_only() {
    let src = "diagram main { \
        A [href = \"https://example.com\"]; \
        B [ref = \"#detail\"]; } \
        diagram detail { C; }";

    let linked = render_str(src, &RenderOptions::default());
    assert!(linked.contains("href=\"https://example.com\""));
    assert!(linked.contains("href=\"#detail\""));

    let gated = render_str(
        src,
        &RenderOptions {
            no_external: true,
            ..RenderOptions::default()
        },
    );
    assert!(!gated.contains("href=\"https://example.com\""));
    assert!(gated.contains("href=\"#detail\""), "refs stay navigable");
}

#[test]
fn explicit_root_selects_the_entry_diagram() {
    let options = RenderOptions {
        bundle_root: Some("detail".to_string()),
        ..RenderOptions::default()
    };
    let html = render_str(
        "diagram main { A [ref = \"#detail\"]; } diagram detail { B; }",
        &options,
    );
    assert!(html.contains("data-root=\"detail\""));
    assert_eq!(html.matches("data-root=\"true\"").count(), 1);
}

#[test]
fn bundle_is_deterministic() {
    let src = include_str!("fixtures/drilldown.diag");
    assert_eq!(
        render_str(src, &RenderOptions::default()),
        render_str(src, &RenderOptions::default())
    );
}

#[test]
fn accessibility_hooks_are_present() {
    let html = render_str(
        "diagram main { A [ref = \"#detail\"]; } diagram detail { B; }",
        &RenderOptions::default(),
    );

    // Native links with accessible names.
    assert!(html.contains("aria-label=\"A: go to diagram detail\""));
    assert!(html.contains("aria-label=\"Breadcrumb\""));
    assert!(html.contains("aria-label=\"Go back\""));
    // The runtime sets the current breadcrumb crumb and supports Escape.
    assert!(html.contains("aria-current"));
    assert!(html.contains("'Escape'"));
    // Progressive enhancement: no-JS `:target` reveal rules.
    assert!(html.contains(":target"));
}

#[test]
fn embed_font_adds_a_single_font_face() {
    let html = render_str(
        "diagram main { A -> B; }",
        &RenderOptions {
            embed_font: true,
            ..RenderOptions::default()
        },
    );
    assert_eq!(html.matches("@font-face").count(), 1);
    assert!(html.contains("data:font/ttf;base64,"));
}

#[test]
fn runtime_navigates_when_history_is_unavailable() {
    // The live-preview server embeds the bundle in an `<iframe srcdoc>`, where
    // `history.pushState` may throw. Drive the real inline runtime with Node
    // and a mocked history to prove navigation still happens. Skipped when
    // node is not installed, following the `nvim` pattern.
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("node not installed; skipping bundle runtime test");
        return;
    }

    let html = render_str(
        "diagram overview { A [ref = \"#detail\"]; } diagram detail { B; }",
        &RenderOptions::default(),
    );
    let dir = common::TempDir::new("swimflow-bundle-runtime-");
    let path = dir.join("bundle.html");
    std::fs::write(&path, html).expect("write bundle");

    let harness = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/bundle_runtime.js");
    let output = Command::new("node")
        .arg(&harness)
        .arg(&path)
        .output()
        .expect("run node");

    assert!(
        output.status.success(),
        "bundle runtime harness failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Collect all `data-target` attribute values.
fn data_targets(html: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = html;
    while let Some(index) = rest.find("data-target=\"") {
        let after = &rest[index + "data-target=\"".len()..];
        if let Some(end) = after.find('"') {
            targets.push(after[..end].to_string());
            rest = &after[end..];
        } else {
            break;
        }
    }
    targets
}

#[test]
fn an_interactive_page_has_zoom_controls_and_one_zoom_script() {
    let html = render_str(
        "diagram main { A [ref = \"#detail\"]; } diagram detail { B; }",
        &RenderOptions::default(),
    );
    for action in ["out", "in", "reset", "fit"] {
        assert!(
            html.contains(&format!("data-zoom=\"{action}\"")),
            "{action}"
        );
    }
    assert!(html.contains("class=\"swimflow-zoom-level\""));
    assert_eq!(html.matches("addEventListener('wheel'").count(), 1);
    // The navigation script announces each diagram it shows to the zoom.
    assert!(html.contains("'swimflow:show'"));
    // Without JavaScript there is nothing to zoom with.
    assert!(html.contains("html:not(.js) .swimflow-zoom { display: none; }"));
}

#[test]
fn zoom_works_in_the_page_scripts() {
    // Ctrl/Cmd + wheel, the keys, the buttons and the fit to the window run
    // against a mock DOM. Skipped when node is not installed.
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("node not installed; skipping the zoom test");
        return;
    }
    let html = render_str("diagram main { A -> B; }", &RenderOptions::default());
    let dir = common::TempDir::new("swimflow-zoom-");
    let path = dir.join("bundle.html");
    std::fs::write(&path, html).expect("write bundle");

    let harness = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/zoom_runtime.js");
    let output = Command::new("node")
        .arg(&harness)
        .arg(&path)
        .output()
        .expect("run node");

    assert!(
        output.status.success(),
        "zoom harness failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
