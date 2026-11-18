//! Tests for the diagram-type plugin architecture.
//!
//! These cover the parts that are new with the plugin split: the registry
//! (how a diagram type is chosen), the sequence diagram plugin (a second
//! implementation of the plugin contract) and the fact that every diagram
//! type gets the shared output formats for free.

use std::path::Path;

use swimflow::build::LoadOptions;
use swimflow::core::formats;
use swimflow::core::overlay::Overlay;
use swimflow::plugin::{
    DiagramPlugin, DumpKind, PluginRequest, Registry, RenderContext, RenderMode,
};
use swimflow::render::RenderOptions;

const SEQUENCE: &str = r#"
sequence "Demo" {
  participant "Client" as client;
  participant "Server";

  client -> Server: "hello";
  Server --> client: "hi";
  note over client, Server: "a note";
  loop "again" {
    client -> client: "think";
  }
}
"#;

fn registry() -> Registry {
    Registry::builtin()
}

fn request<'a>(
    type_name: Option<&'a str>,
    path: Option<&'a Path>,
    source: &'a str,
) -> PluginRequest<'a> {
    PluginRequest {
        type_name,
        path,
        source,
    }
}

#[test]
fn the_registry_lists_the_built_in_types() {
    let registry = registry();
    assert_eq!(
        registry.names(),
        vec!["sequence", "tree", "treemap", "manifest", "activity"]
    );
    let sequence = registry.find("sequence").expect("the sequence plugin");
    assert!(sequence.description().contains("sequence"));
    assert!(sequence.extensions().contains(&"seq"));
}

#[test]
fn the_registry_describes_every_type() {
    let registry = registry();
    let types = registry.types();

    // The types are listed in display order: the activity diagram first.
    assert_eq!(types[0].name, "activity");
    assert_eq!(types[1].name, "sequence");
    assert_eq!(types[2].name, "tree");
    assert_eq!(types[3].name, "treemap");

    let activity = &types[0];
    assert_eq!(activity.source_extension(), "diag");
    assert!(activity.claims("diag"));
    assert!(activity.claims("ACTDIAG"));
    assert!(!activity.claims("seq"));

    let sequence = &types[1];
    assert_eq!(sequence.source_extension(), "seq");
    assert!(sequence.claims("seq"));

    let tree = &types[2];
    assert_eq!(tree.source_extension(), "tree");
    assert!(tree.claims("TREEDIAG"));
    assert_eq!(registry.type_for_extension("tree"), Some("tree"));

    let treemap = &types[3];
    assert_eq!(treemap.source_extension(), "tmap");
    assert!(treemap.claims("TMAP"));
    assert_eq!(registry.type_for_extension("tmap"), Some("treemap"));

    let manifest = &types[4];
    assert_eq!(manifest.name, "manifest");
    assert_eq!(manifest.source_extension(), "mfst");
    assert!(manifest.claims("MFST"));
    assert_eq!(registry.type_for_extension("mfst"), Some("manifest"));

    assert_eq!(registry.type_for_extension("seq"), Some("sequence"));
    assert_eq!(registry.type_for_extension("diag"), Some("activity"));
    assert_eq!(registry.type_for_extension("txt"), None);
    assert_eq!(
        registry.type_for_path(Path::new("sub/chat.seq")),
        Some("sequence")
    );
}

#[test]
fn every_type_has_a_starter_template() {
    let registry = registry();
    let activity = registry
        .template("activity")
        .expect("the activity template");
    assert!(activity.contains("diagram"), "{activity}");
    let sequence = registry
        .template("sequence")
        .expect("the sequence template");
    assert!(sequence.contains("sequence"), "{sequence}");
    let tree = registry.template("tree").expect("the tree template");
    assert!(tree.contains("tree"), "{tree}");
    let treemap = registry.template("treemap").expect("the treemap template");
    assert!(treemap.contains("treemap"), "{treemap}");
    let manifest = registry
        .template("manifest")
        .expect("the manifest template");
    assert!(manifest.contains("manifest"), "{manifest}");
}

#[test]
fn a_type_is_chosen_by_name_extension_or_content() {
    let registry = registry();

    let by_name = registry
        .resolve(&request(Some("activity"), None, SEQUENCE))
        .expect("resolve by name");
    assert_eq!(by_name.name(), "activity");

    let by_extension = registry
        .resolve(&request(None, Some(Path::new("demo.seq")), ""))
        .expect("resolve by extension");
    assert_eq!(by_extension.name(), "sequence");

    let by_content = registry
        .resolve(&request(None, None, SEQUENCE))
        .expect("resolve by content");
    assert_eq!(by_content.name(), "sequence");
}

#[test]
fn an_unknown_type_is_an_error() {
    let registry = registry();
    let err = match registry.resolve(&request(Some("nope"), None, "")) {
        Ok(_) => panic!("an unknown type must not resolve"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("sequence"));
    assert!(err.to_string().contains("activity"));
    assert!(err.to_string().contains("tree"));
}

/// Render a source with a plugin, in the given mode.
fn render(
    plugin: &dyn DiagramPlugin,
    source: &str,
    mode: RenderMode,
) -> swimflow::plugin::Rendered {
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
    plugin.render(&context).expect("render")
}

#[test]
fn the_sequence_plugin_renders_a_valid_svg() {
    let registry = registry();
    let plugin = registry.find("sequence").expect("the sequence plugin");
    let rendered = render(plugin, SEQUENCE, RenderMode::Standalone);

    let section = rendered.first().expect("a section");
    assert!(section.svg.contains("<svg"));
    assert!(section.svg.contains("Client"));
    assert!(section.svg.contains("Server"));
    // The participants, the lifelines and the messages are all drawn.
    assert!(section.svg.contains("stroke-dasharray=\"4\""));
    assert!(section.svg.contains("<polygon"));

    usvg::Tree::from_str(&section.svg, &usvg::Options::default())
        .expect("the sequence diagram is a valid SVG");
}

#[test]
fn every_diagram_type_gets_png_and_pdf_for_free() {
    let registry = registry();
    for name in registry.names() {
        let plugin = registry.find(name).expect("plugin");
        let source = match name {
            "sequence" => SEQUENCE,
            "tree" => "tree demo { A { B; } }",
            "treemap" => "treemap demo { A: 1; B: 2; }",
            "manifest" => "manifest demo { column a { x; } }",
            _ => "diagram demo { A -> B; }",
        };
        let rendered = render(plugin, source, RenderMode::Standalone);
        let svg = &rendered.first().expect("a section").svg;

        let font = swimflow::text::FontSpec::default();
        let png = formats::svg_to_png(svg, &font, 1.0).expect("png");
        assert_eq!(&png[1..4], b"PNG", "{name} should produce a PNG");

        let pdf = formats::svg_to_pdf(svg, &font).expect("pdf");
        assert!(pdf.starts_with(b"%PDF-"), "{name} should produce a PDF");
    }
}

#[test]
fn the_tree_plugin_renders_a_valid_svg() {
    let registry = registry();
    let plugin = registry.find("tree").expect("the tree plugin");
    let rendered = render(
        plugin,
        "tree \"T\" { root R; A { B; C; } D [edge = x]; }",
        RenderMode::Standalone,
    );
    let section = rendered.first().expect("a section");
    for label in ["R", "A", "B", "C", "D"] {
        assert!(section.svg.contains(&format!(">{label}</text>")), "{label}");
    }
    usvg::Tree::from_str(&section.svg, &usvg::Options::default())
        .expect("the tree diagram is a valid SVG");
}

#[test]
fn the_tree_plugin_warns_about_a_dangling_ref_and_dumps_its_model() {
    let registry = registry();
    let plugin = registry.find("tree").expect("the tree plugin");
    let rendered = render(
        plugin,
        "tree { A [collapsed] { B [ref = \"x\"]; } }",
        RenderMode::Standalone,
    );
    assert_eq!(rendered.warnings.len(), 1, "{:?}", rendered.warnings);
    assert!(rendered.warnings[0].contains("unknown tree"));

    let options = RenderOptions::default();
    let load = LoadOptions::default();
    let overlay = Overlay::new();
    let context = RenderContext::new("tree { A { B; } }", &options, &load, &overlay);
    let model = plugin.dump(DumpKind::Model, &context).expect("model dump");
    assert!(model.contains("  node A\n    node B"), "{model}");
    let geometry = plugin
        .dump(DumpKind::Geometry, &context)
        .expect("geometry dump");
    assert!(geometry.contains("depth 2"), "{geometry}");
}

#[test]
fn a_tree_is_detected_by_its_content() {
    let registry = registry();
    let by_content = registry
        .resolve(&request(None, None, "tree \"x\" {\n  A;\n}\n"))
        .expect("resolve by content");
    assert_eq!(by_content.name(), "tree");
    // An activity node that happens to be called `tree` stays activity.
    let activity = registry
        .resolve(&request(None, None, "{ tree -> B; }"))
        .expect("resolve by content");
    assert_eq!(activity.name(), "activity");
}

#[test]
fn a_sequence_diagram_can_be_packaged_as_an_html_bundle() {
    let registry = registry();
    let plugin = registry.find("sequence").expect("the sequence plugin");
    let rendered = render(plugin, SEQUENCE, RenderMode::Bundle);
    assert_eq!(rendered.sections.len(), 1);

    let html = swimflow::core::html::bundle(&rendered.sections, "Demo", "");
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("id=\"Demo\""));
    assert!(html.contains("data-diagram=\"Demo\""));
    assert!(html.contains("<svg"));
}

#[test]
fn the_sequence_plugin_reports_unknown_participants() {
    let registry = registry();
    let plugin = registry.find("sequence").expect("the sequence plugin");
    let rendered = render(plugin, "sequence { A -> B: hi; }", RenderMode::Standalone);
    assert_eq!(rendered.warnings.len(), 2);
    assert!(rendered.warnings[0].contains("unknown participant"));
}

#[test]
fn the_sequence_plugin_dumps_its_model() {
    let registry = registry();
    let plugin = registry.find("sequence").expect("the sequence plugin");
    let options = RenderOptions::default();
    let load = LoadOptions::default();
    let overlay = Overlay::new();
    let context = RenderContext {
        source: SEQUENCE,
        path: None,
        options: &options,
        mode: RenderMode::Standalone,
        load: &load,
        pixels: false,
        overlay: &overlay,
    };

    let ast = plugin
        .dump(swimflow::plugin::DumpKind::Ast, &context)
        .expect("ast dump");
    assert!(ast.contains("Participant"));

    let model = plugin
        .dump(swimflow::plugin::DumpKind::Model, &context)
        .expect("model dump");
    assert!(model.contains("participant 0 Client"));
    assert!(model.contains("message 0 -> 1 -> hello"));

    let geometry = plugin
        .dump(swimflow::plugin::DumpKind::Geometry, &context)
        .expect("geometry dump");
    assert!(geometry.contains("size "));
    assert!(geometry.contains("lifeline"));
}
