//! The examples/ directory is a tested corpus.
//!
//! Every `.diag` file must load, lay out and render as both SVG and HTML. The
//! drill-down examples additionally have their `data-target` links checked so a
//! bundle can never point at a diagram that was not inlined.
//!
//! The corpus also contains sequence diagrams (`.seq`). They are rendered
//! through the plugin registry, which shows that the example corpus does not
//! have to know about the diagram type it is testing.

use std::path::{Path, PathBuf};

use swimflow::build::{LoadOptions, load_file};
use swimflow::core::overlay::Overlay;
use swimflow::plugin::{PluginRequest, Registry, RenderContext, RenderMode};
use swimflow::render::{self, Format, RenderOptions};

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples")
}

fn collect_diagrams(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();

    for path in entries {
        if path.is_dir() {
            collect_diagrams(&path, out);
        } else if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("diag") | Some("seq") | Some("tree") | Some("tmap") | Some("mfst")
        ) {
            out.push(path);
        }
    }
}

/// The files git ignores (local scratch diagrams listed in `.gitignore` or
/// `.git/info/exclude`), as absolute paths. Empty when git is not available,
/// for example in an unpacked source archive.
fn ignored_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Ok(output) = std::process::Command::new("git")
        .args(["ls-files", "--others", "--ignored", "--exclude-standard"])
        .arg("examples")
        .current_dir(root)
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| root.join(line))
        .collect()
}

/// Every example of the corpus; scratch files that git ignores are not part
/// of it.
fn example_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_diagrams(&examples_dir(), &mut files);
    let ignored = ignored_files();
    files.retain(|path| !ignored.contains(path));
    files
}

fn example_numbers(name: &str) -> Vec<(String, Option<u32>)> {
    let path = examples_dir().join(name);
    let set = load_file(&path, &LoadOptions::default())
        .unwrap_or_else(|e| panic!("failed to load {}: {e}", path.display()));
    set.root().diagrams[0]
        .nodes
        .iter()
        .map(|node| (node.name.clone(), node.number))
        .collect()
}

fn render_example(path: &Path) -> (String, String) {
    // Sequence diagrams go through the plugin registry; the activity diagrams
    // keep using the low-level pipeline, so both paths stay covered.
    if path.extension().and_then(|ext| ext.to_str()) != Some("diag") {
        return render_with_plugin(path);
    }

    let set = load_file(path, &LoadOptions::default())
        .unwrap_or_else(|e| panic!("failed to load {}: {e}", path.display()));
    assert!(
        set.issues.is_empty(),
        "unexpected link issues in {}: {:?}",
        path.display(),
        set.issues
    );

    let mut svg = Vec::new();
    render::render(Format::Svg, &set, &mut svg, &RenderOptions::default())
        .unwrap_or_else(|e| panic!("failed to render SVG for {}: {e}", path.display()));
    let svg = String::from_utf8(svg).expect("SVG is UTF-8");
    usvg::Tree::from_str(&svg, &usvg::Options::default())
        .unwrap_or_else(|e| panic!("invalid SVG for {}: {e}", path.display()));

    let mut html = Vec::new();
    render::render(Format::Html, &set, &mut html, &RenderOptions::default())
        .unwrap_or_else(|e| panic!("failed to render HTML for {}: {e}", path.display()));
    let html = String::from_utf8(html).expect("HTML is UTF-8");
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("data-diagram="));

    (svg, html)
}

/// Render an example through the plugin registry, whatever its diagram type
/// is. This is the path the command line takes.
fn render_with_plugin(path: &Path) -> (String, String) {
    let source = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    let registry = Registry::builtin();
    let plugin = registry
        .resolve(&PluginRequest {
            type_name: None,
            path: Some(path),
            source: &source,
        })
        .unwrap_or_else(|e| panic!("no plugin for {}: {e}", path.display()));

    let options = RenderOptions::default();
    let load = LoadOptions::default();
    let overlay = Overlay::new();

    let standalone = plugin
        .render(&RenderContext {
            source: &source,
            path: Some(path),
            options: &options,
            mode: RenderMode::Standalone,
            load: &load,
            pixels: false,
            overlay: &overlay,
        })
        .unwrap_or_else(|e| panic!("failed to render {}: {e}", path.display()));
    let svg = standalone
        .first()
        .unwrap_or_else(|| panic!("{} rendered no diagram", path.display()))
        .svg
        .clone();
    usvg::Tree::from_str(&svg, &usvg::Options::default())
        .unwrap_or_else(|e| panic!("invalid SVG for {}: {e}", path.display()));

    let bundled = plugin
        .render(&RenderContext {
            source: &source,
            path: Some(path),
            options: &options,
            mode: RenderMode::Bundle,
            load: &load,
            pixels: false,
            overlay: &overlay,
        })
        .unwrap_or_else(|e| panic!("failed to bundle {}: {e}", path.display()));
    let root = bundled
        .root
        .clone()
        .or_else(|| bundled.first().map(|section| section.key.clone()))
        .expect("a root diagram");
    let html = swimflow::core::html::bundle(&bundled.sections, &root, "");

    (svg, html)
}

#[test]
fn every_example_builds_and_renders() {
    let files = example_files();
    assert!(
        files.len() >= 10,
        "expected a useful examples corpus, found {}",
        files.len()
    );

    for path in files {
        let (svg, _) = render_example(&path);
        assert!(svg.contains("<svg"), "no SVG root for {}", path.display());
    }
}

#[test]
fn every_example_renders_through_the_plugin_registry() {
    for path in example_files() {
        let (svg, html) = render_with_plugin(&path);
        assert!(svg.contains("<svg"), "no SVG root for {}", path.display());
        assert!(
            html.contains("data-diagram="),
            "no bundle section for {}",
            path.display()
        );
    }
}

#[test]
fn examples_readme_documents_every_file() {
    let readme = std::fs::read_to_string(examples_dir().join("README.rst")).expect("README");
    for path in example_files() {
        let name = path.file_name().unwrap().to_string_lossy();
        assert!(
            readme.contains(name.as_ref()),
            "examples/README.rst does not mention {name}"
        );
    }
}

#[test]
fn zoom_demo_bundle_has_every_level_and_valid_targets() {
    let path = examples_dir().join("zoom-demo.diag");
    let set = load_file(&path, &LoadOptions::default()).expect("load zoom-demo");
    let mut html_bytes = Vec::new();
    render::render(
        Format::Html,
        &set,
        &mut html_bytes,
        &RenderOptions::default(),
    )
    .expect("bundle");
    let html = String::from_utf8(html_bytes).expect("HTML is UTF-8");

    for id in ["overview", "details", "release", "rollback"] {
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "zoom-demo bundle is missing diagram {id}"
        );
    }
    assert!(html.contains("data-root=\"overview\""));
    assert_eq!(html.matches("data-root=\"true\"").count(), 1);

    let mut targets = Vec::new();
    let mut rest = html.as_str();
    while let Some(start) = rest.find("data-target=\"") {
        let value = &rest[start + "data-target=\"".len()..];
        let end = value.find('"').expect("closed data-target attribute");
        targets.push(value[..end].to_string());
        rest = &value[end + 1..];
    }
    assert_eq!(targets.len(), 4, "expected one link per zoom level");
    for target in targets {
        assert!(
            html.contains(&format!("id=\"{target}\"")),
            "zoom-demo data-target {target:?} has no matching diagram"
        );
    }
}

#[test]
fn numbering_examples_contrast_file_and_topological_order() {
    // Both files declare the exact same scrambled graph; only `numbering`
    // differs. `auto` hands out numbers in file order, `topological` follows
    // the edges.
    assert_eq!(
        example_numbers("numbering-auto.diag"),
        vec![
            ("Deploy".to_string(), Some(1)),
            ("Test".to_string(), Some(2)),
            ("Plan".to_string(), Some(3)),
            ("Verify".to_string(), Some(4)),
            ("Build".to_string(), Some(5)),
            ("Release".to_string(), Some(6)),
        ]
    );
    assert_eq!(
        example_numbers("numbering-topological.diag"),
        vec![
            ("Deploy".to_string(), Some(5)),
            ("Test".to_string(), Some(3)),
            ("Plan".to_string(), Some(1)),
            ("Verify".to_string(), Some(6)),
            ("Build".to_string(), Some(2)),
            ("Release".to_string(), Some(4)),
        ]
    );
}

#[test]
fn topological_numbering_survives_a_feedback_loop() {
    // `order-processing` is one big feedback cycle (Refund -> Order). The
    // cycle-closing edge is dropped, so the badges follow the arrows and the
    // receive step is numbered last, not second.
    assert_eq!(
        example_numbers("order-processing.diag"),
        vec![
            ("Order".to_string(), Some(1)),
            ("Receive".to_string(), Some(9)),
            ("Check".to_string(), Some(2)),
            ("Pick".to_string(), Some(3)),
            ("Pack".to_string(), Some(4)),
            ("Charge".to_string(), Some(5)),
            ("Refund".to_string(), Some(6)),
            ("Ship".to_string(), Some(7)),
            ("Deliver".to_string(), Some(8)),
        ]
    );
}

#[test]
fn depth_numbering_groups_parallel_steps() {
    // `ManualReview` starts at the same time as `Start`, so both are stage 0
    // even though `ManualReview` is declared last.
    assert_eq!(
        example_numbers("numbering-depth.diag"),
        vec![
            ("Start".to_string(), Some(1)),
            ("Build".to_string(), Some(3)),
            ("Test".to_string(), Some(4)),
            ("ManualReview".to_string(), Some(2)),
        ]
    );
}

#[test]
fn cross_file_example_inlines_the_referenced_diagram() {
    let path = examples_dir().join("cross-file/overview.diag");
    let set = load_file(&path, &LoadOptions::default()).expect("load cross-file example");
    assert_eq!(set.documents.len(), 2, "details.diag should be loaded");

    let mut html = Vec::new();
    render::render(Format::Html, &set, &mut html, &RenderOptions::default()).expect("bundle");
    let html = String::from_utf8(html).expect("HTML is UTF-8");

    assert!(html.contains("data-diagram=\"overview\""));
    assert!(html.contains("data-diagram=\"details\""));
    assert!(html.contains("data-target=\"details\""));
    assert!(html.contains("Prepare"));
}
