//! Cross-file link loading and resolution.

use std::path::PathBuf;

use swimflow::build::{LoadOptions, load_file};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/nested")
        .join(name)
}

#[test]
fn cross_file_link_loads_and_resolves() {
    let set = load_file(fixture("root.diag"), &LoadOptions::default()).unwrap();
    assert_eq!(set.documents.len(), 2);
    assert_eq!(set.root, 0);

    let root = &set.documents[0];
    assert_eq!(root.diagrams[0].id, "main");
    let review = root.diagrams[0].find_node("Review").unwrap();
    let resolved = root.diagrams[0].node(review).resolved.as_ref().unwrap();
    assert_eq!(resolved.document, 1);
    assert_eq!(resolved.diagram, "detail");

    assert_eq!(set.documents[1].diagrams[0].id, "detail");
    assert!(set.issues.is_empty());
}

#[test]
fn cyclic_file_references_terminate() {
    let set = load_file(fixture("cycle_a.diag"), &LoadOptions::default()).unwrap();
    assert_eq!(set.documents.len(), 2);

    // Both directions resolve.
    let a = &set.documents[0];
    let x = a.diagrams[0].find_node("X").unwrap();
    let resolved = a.diagrams[0].node(x).resolved.as_ref().unwrap();
    assert_eq!(resolved.diagram, "b");

    let b = &set.documents[1];
    let y = b.diagrams[0].find_node("Y").unwrap();
    let resolved = b.diagrams[0].node(y).resolved.as_ref().unwrap();
    assert_eq!(resolved.diagram, "a");
}

#[test]
fn missing_target_diagram_is_an_error_by_default() {
    let err = load_file(fixture("broken.diag"), &LoadOptions::default()).unwrap_err();
    assert!(matches!(err, swimflow::build::BuildError::Link { .. }));
}

#[test]
fn missing_target_diagram_can_be_tolerated() {
    let options = LoadOptions {
        allow_broken_links: true,
        ..LoadOptions::default()
    };
    let set = load_file(fixture("broken.diag"), &options).unwrap();
    assert_eq!(set.issues.len(), 1);
    let node = &set.documents[0].diagrams[0];
    let a = node.find_node("A").unwrap();
    assert!(node.node(a).resolved.is_none());
}

#[test]
fn external_files_can_be_disabled() {
    let options = LoadOptions {
        follow_external_files: false,
        allow_broken_links: true,
    };
    let set = load_file(fixture("root.diag"), &options).unwrap();
    assert_eq!(set.documents.len(), 1);
    assert_eq!(set.issues.len(), 1);
}
