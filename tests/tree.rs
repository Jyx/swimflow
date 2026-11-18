//! Integration tests for the tree diagram: `ref` links, folded branches and
//! the parity of the browser runtime with the Rust layout.

use std::path::PathBuf;
use std::process::Command;

mod common;

use swimflow::build::LoadOptions;
use swimflow::core::overlay::Overlay;
use swimflow::plugin::tree::TreePlugin;
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
    TreePlugin.render(&context).expect("render")
}

fn svg(source: &str) -> String {
    render(source, RenderMode::Standalone)
        .first()
        .expect("a section")
        .svg
        .clone()
}

#[test]
fn a_branch_gets_a_minus_circle_and_a_folded_branch_a_plus() {
    let open = svg("tree { A { B; } C; }");
    // One toggle for the root and one for A; both are expanded, so the
    // vertical bar of the plus is hidden.
    assert_eq!(open.matches("class=\"tree-toggle\"").count(), 2, "{open}");
    assert!(open.contains("aria-label=\"Collapse\""), "{open}");
    assert!(
        open.contains("class=\"tree-plus\"") && open.contains("display=\"none\"/>"),
        "{open}"
    );

    let folded = svg("tree { A [collapsed] { B; } C; }");
    assert!(folded.contains("aria-label=\"Expand\""), "{folded}");
    let plus_of_a = folded
        .split("class=\"tree-plus\"")
        .nth(2)
        .expect("the plus of A");
    assert!(
        !plus_of_a.split("/>").next().unwrap().contains("display"),
        "{plus_of_a}"
    );
    // B is still in the document (so it can be unfolded) but hidden.
    let b = folded
        .split("<g class=\"tree-node\"")
        .find(|node| node.contains(">B</text>"))
        .expect("node B");
    assert!(
        b.split('>').next().unwrap().contains("display=\"none\""),
        "{b}"
    );
}

#[test]
fn folding_shrinks_the_canvas() {
    let size = |svg: &str| {
        let tag = svg.split("<svg").nth(1).unwrap();
        let width = tag
            .split("width=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        width.parse::<i32>().unwrap()
    };
    let open = svg("tree { A { A1; A2; A3; } B; }");
    let folded = svg("tree { A [collapsed] { A1; A2; A3; } B; }");
    assert!(size(&folded) < size(&open));
}

#[test]
fn collapsed_can_be_switched_off() {
    let off = svg("tree { A [collapsed = false] { B; } }");
    assert!(off.contains("aria-label=\"Collapse\""), "{off}");
}

#[test]
fn a_ref_links_to_another_tree_in_a_bundle() {
    let source = "tree overview { A [ref = \"#detail\"]; B [ref = detail]; }\n\
                  tree detail { X; }";
    let bundle = render(source, RenderMode::Bundle);
    assert_eq!(bundle.sections.len(), 2);
    assert!(bundle.warnings.is_empty(), "{:?}", bundle.warnings);
    let first = &bundle.sections[0].svg;
    assert_eq!(
        first.matches("data-target=\"detail-n1\"").count(),
        2,
        "{first}"
    );
    assert!(first.contains("href=\"#detail-n1\""), "{first}");

    // The runtime that folds the branches travels with the sections.
    assert!(!bundle.sections[0].scripts.is_empty());

    // A standalone SVG cannot navigate: the link is shown disabled.
    let standalone = svg(source);
    assert!(!standalone.contains("data-target"), "{standalone}");
    assert!(
        standalone.contains("data-link-disabled=\"true\""),
        "{standalone}"
    );
}

#[test]
fn a_ref_that_leads_nowhere_is_reported() {
    let rendered = render(
        "tree one { A [ref = missing]; B [ref = \"other.tree#x\"]; }",
        RenderMode::Bundle,
    );
    assert_eq!(rendered.warnings.len(), 2, "{:?}", rendered.warnings);
    assert!(
        rendered.warnings[0].contains("unknown tree"),
        "{:?}",
        rendered.warnings
    );
    assert!(
        rendered.warnings[1].contains("another file"),
        "{:?}",
        rendered.warnings
    );
    let svg = &rendered.sections[0].svg;
    assert!(!svg.contains("data-target"), "{svg}");
}

#[test]
fn a_bundle_has_fold_all_buttons_for_each_tree_and_a_file_does_not() {
    use swimflow::core::html;
    let source = "tree a { A { B; } } tree b { C { D; } }";
    let rendered = render(source, RenderMode::Bundle);
    let page = html::bundle(&rendered.sections, "a", "");
    assert_eq!(page.matches("class=\"tree-collapse-all\"").count(), 2);
    assert_eq!(page.matches("class=\"tree-expand-all\"").count(), 2);
    assert!(page.contains(">Collapse all</button>"));
    assert!(page.contains(">Expand all</button>"));

    // A tree without a single branch has nothing to fold, so no buttons.
    let flat = render("tree f { root A; }", RenderMode::Bundle);
    assert!(flat.sections[0].toolbar.is_empty());

    let standalone = render(source, RenderMode::Standalone);
    assert!(standalone.sections.iter().all(|s| s.toolbar.is_empty()));
    assert!(!standalone.sections[0].svg.contains("tree-collapse-all"));
}

#[test]
fn the_html_bundle_includes_the_runtime_once() {
    use swimflow::core::html;
    let rendered = render(
        "tree a { A { B; } } tree b { C { D; } }",
        RenderMode::Bundle,
    );
    let page = html::bundle(&rendered.sections, "a", "");
    assert_eq!(page.matches("function layoutTree").count(), 1);
}

/// A tree with branches at several depths, edge labels of different sizes and
/// a wide label that wraps.
fn fixture(orientation: &str, folded: &[&str]) -> String {
    let flag = |name: &str| {
        if folded.contains(&name) {
            ", collapsed"
        } else {
            ""
        }
    };
    let root = if folded.contains(&"R") {
        " [collapsed]"
    } else {
        ""
    };
    format!(
        "tree \"T\" {{\n  orientation = {orientation};\n  root R{root};\n  \
         A [color = red{a}] {{ A1 [edgestyle = dashed]; A2 [edge = x{a2}] {{ A21 [edgestyle = dashed]; A22 [edge = \"a longer label\"]; }} }}\n  \
         B [edge = yes{b}, edgestyle = dashed] {{ B1 [edgestyle = dashed]; B2 [edgestyle = dashed]; B3 [edge = \"label\"{b3}, edgestyle = dashed] {{ B31 [edge = \"two words here\", edgestyle = dashed]; }} }}\n  \
         \"A very long node label that has to wrap onto several lines\";\n  C [edgestyle = dashed];\n}}\n",
        a = flag("A"),
        a2 = flag("A2"),
        b = flag("B"),
        b3 = flag("B3"),
    )
}

#[test]
fn the_fold_all_buttons_fold_and_unfold_every_branch() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("node not installed; skipping the fold-all test");
        return;
    }

    let dir = common::TempDir::new("swimflow-fold-all-");

    let harness = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/tree_fold_all_runtime.js");
    for orientation in ["portrait", "landscape"] {
        // The section as a bundle writes it: the buttons above the diagram.
        let rendered = render(&fixture(orientation, &[]), RenderMode::Bundle);
        let section = &rendered.sections[0];
        let section_path = dir.join(format!("{orientation}-section.html"));
        std::fs::write(
            &section_path,
            format!(
                "<div class=\"swimflow-toolbar-slot\" data-diagram=\"tree\">{}</div>\
                 <section class=\"swimflow-diagram\" id=\"tree\">{}</section>",
                section.toolbar, section.svg
            ),
        )
        .unwrap();

        // Every node with children is folded: the root and each branch.
        let all_folded = dir.join(format!("{orientation}-all.svg"));
        let every = ["R", "A", "A2", "B", "B3"];
        std::fs::write(&all_folded, svg(&fixture(orientation, &every))).unwrap();

        let output = Command::new("node")
            .arg(&harness)
            .arg(&section_path)
            .arg(&all_folded)
            .output()
            .expect("run node");
        assert!(
            output.status.success(),
            "{orientation}: the fold-all buttons do not give the folded tree:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn the_browser_runtime_lays_out_exactly_like_rust() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("node not installed; skipping the tree runtime test");
        return;
    }

    let dir = common::TempDir::new("swimflow-tree-runtime-");

    let harness = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/tree_runtime.js");
    let dom_harness = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/tree_dom_runtime.js");
    for orientation in ["portrait", "landscape"] {
        // The expanded tree, and the same tree with one branch folded at a time.
        let write = |name: &str, source: String| {
            let path = dir.join(format!("{orientation}-{name}.svg"));
            std::fs::write(&path, svg(&source)).unwrap();
            path
        };
        let open = write("open", fixture(orientation, &[]));
        let mut command = Command::new("node");
        command.arg(&harness).arg(&open);
        for branch in ["A", "A2", "B", "B3"] {
            let folded = write(branch, fixture(orientation, &[branch]));
            command.arg(format!("{branch}={}", folded.display()));
        }
        // Clicking the circle of a branch in the (fake) DOM gives the folded
        // picture, and clicking again gives the expanded one.
        for branch in ["A", "A2", "B", "B3"] {
            let folded = dir.join(format!("{orientation}-{branch}.svg"));
            let output = Command::new("node")
                .arg(&dom_harness)
                .arg(&open)
                .arg(branch)
                .arg(&folded)
                .output()
                .expect("run node");
            assert!(
                output.status.success(),
                "{orientation}/{branch}: clicking does not give the folded tree:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // A state that starts folded must agree with itself, too.
        let start = write("start", fixture(orientation, &["A", "B3"]));
        let mut second = Command::new("node");
        second.arg(&harness).arg(&start);

        for mut run in [command, second] {
            let output = run.output().expect("run node");
            assert!(
                output.status.success(),
                "{orientation}: the tree runtime disagrees with the layout:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    // The same check for every tree example.
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/tree");
    for entry in std::fs::read_dir(examples).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("tree") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        let out = dir.join(format!(
            "{}.svg",
            path.file_stem().unwrap().to_string_lossy()
        ));
        std::fs::write(&out, svg(&source)).unwrap();
        let output = Command::new("node")
            .arg(&harness)
            .arg(&out)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
