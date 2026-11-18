//! Build the IR from the parser AST, and load documents with link resolution.
//!
//! The AST -> `Diagram` instantiation lives in [`layout`]; the integer-grid
//! layout in [`grid`]. This module owns file loading, cross-file `ref`
//! resolution and the model dumps.

pub mod grid;
pub mod layout;
pub mod link;
pub mod model;
pub mod numbering;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::ast;
use crate::lexer::unquote;

use link::{Link, ResolvedLink};
use model::{Diagram, Direction, Document, DocumentSet, LinkIssue};

pub use grid::dump_geometry;

/// Errors produced while building the IR or resolving links.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Parse(#[from] crate::error::Error),

    #[error("{0}")]
    Message(String),

    #[error("link error in {file}: node {node:?}: {message}")]
    Link {
        file: String,
        node: String,
        message: String,
    },
}

/// Options controlling [`load_file`].
#[derive(Debug, Clone)]
pub struct LoadOptions {
    /// Keep going when a `ref` cannot be resolved (issues are recorded).
    pub allow_broken_links: bool,
    /// Transitively load files referenced by `ref = "other.diag#..."`.
    pub follow_external_files: bool,
}

impl Default for LoadOptions {
    fn default() -> Self {
        LoadOptions {
            allow_broken_links: false,
            follow_external_files: true,
        }
    }
}

/// The result of laying out one diagram block (the diagram plus warnings).
type DiagramBuild = Result<(Diagram, Vec<String>), BuildError>;

/// Files that live in memory instead of on disk.
///
/// The overlay is a core service (see `crate::core::overlay`); it is
/// re-exported here because the loader is where it is used.
pub use crate::core::overlay::{Overlay, normalize_path};

/// Build a [`Document`] from a parsed AST.
///
/// Diagram blocks are independent, so multiple blocks are laid out in parallel;
/// results are collected in definition order, keeping the output deterministic.
pub fn build(ast_doc: &ast::Document, source: Option<PathBuf>) -> Result<Document, BuildError> {
    let mut ids = Vec::with_capacity(ast_doc.diagrams.len());
    for ast_diagram in &ast_doc.diagrams {
        let id = match &ast_diagram.id {
            Some(raw) => unquote(raw),
            None => "default".to_string(),
        };
        if ids.contains(&id) {
            return Err(BuildError::Message(format!("duplicate diagram id: {id:?}")));
        }
        ids.push(id);
    }

    let built = build_diagrams(&ids, &ast_doc.diagrams)?;

    let mut document = Document {
        source,
        diagrams: Vec::with_capacity(built.len()),
        index: BTreeMap::new(),
        warnings: Vec::new(),
    };
    for (id, (diagram, warnings)) in ids.into_iter().zip(built) {
        document.warnings.extend(warnings);
        document.index.insert(id, document.diagrams.len());
        document.diagrams.push(diagram);
    }

    Ok(document)
}

/// Lay out every diagram block, using a bounded worker pool when there is more
/// than one. The per-diagram results are reassembled in definition order.
fn build_diagrams(
    ids: &[String],
    ast_diagrams: &[ast::Diagram],
) -> Result<Vec<(Diagram, Vec<String>)>, BuildError> {
    if ast_diagrams.len() <= 1 {
        return ids
            .iter()
            .zip(ast_diagrams)
            .map(|(id, ast_diagram)| layout::build_diagram(id.clone(), ast_diagram))
            .collect();
    }

    let workers = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .min(ast_diagrams.len());
    let next = AtomicUsize::new(0);

    let chunks: Vec<Vec<(usize, DiagramBuild)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                let next = &next;
                scope.spawn(move || {
                    let mut out = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        if index >= ast_diagrams.len() {
                            break;
                        }
                        let result =
                            layout::build_diagram(ids[index].clone(), &ast_diagrams[index]);
                        out.push((index, result));
                    }
                    out
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("diagram layout thread panicked"))
            .collect()
    });

    let mut slots: Vec<Option<DiagramBuild>> = (0..ast_diagrams.len()).map(|_| None).collect();
    for chunk in chunks {
        for (index, result) in chunk {
            slots[index] = Some(result);
        }
    }

    slots
        .into_iter()
        .map(|slot| slot.expect("every diagram index is visited"))
        .collect()
}

/// Parse and build from a string, resolving only in-document links.
pub fn build_str(text: &str) -> Result<DocumentSet, BuildError> {
    build_str_with_options(text, &LoadOptions::default())
}

/// Like [`build_str`], with explicit link-loading options.
///
/// Standard input has no path, so it cannot follow external files; the
/// options still decide whether broken in-document links are tolerated.
pub fn build_str_with_options(
    text: &str,
    options: &LoadOptions,
) -> Result<DocumentSet, BuildError> {
    let ast_doc = crate::parser::parse(text)?;
    let document = build(&ast_doc, None)?;
    let mut set = DocumentSet {
        documents: vec![document],
        root: 0,
        issues: Vec::new(),
    };
    resolve_links(&mut set, &BTreeMap::new(), options)?;
    Ok(set)
}

/// Parse and build from a string as if it lived at `source`, following
/// external `ref` files relative to `source`.
///
/// This is the live-editor variant of [`load_file`]: the root document comes
/// from memory (unsaved edits) while referenced files are still read from
/// disk, so cross-file drill-down keeps working while typing.
pub fn load_str(
    text: &str,
    source: impl AsRef<Path>,
    options: &LoadOptions,
) -> Result<DocumentSet, BuildError> {
    load_str_with_overlay(text, source, &Overlay::new(), options)
}

/// Like [`load_str`], but referenced files may also come from `overlay`.
///
/// This is what the live preview uses for diagrams that only exist in the
/// browser session: they are loaded from memory instead of from disk, and a
/// `ref` to them resolves like any other cross-file reference.
pub fn load_str_with_overlay(
    text: &str,
    source: impl AsRef<Path>,
    overlay: &Overlay,
    options: &LoadOptions,
) -> Result<DocumentSet, BuildError> {
    let source = source.as_ref();
    let canonical = std::fs::canonicalize(source).unwrap_or_else(|_| source.to_path_buf());
    let ast_doc = crate::parser::parse(text)?;
    let document = build(&ast_doc, Some(canonical.clone()))?;

    let mut set = DocumentSet {
        documents: vec![document],
        root: 0,
        issues: Vec::new(),
    };
    let mut loaded: BTreeMap<PathBuf, usize> = BTreeMap::new();
    loaded.insert(canonical.clone(), 0);
    // The root may not exist on disk (a session-only buffer); index it under
    // its normalized path too, so references to it resolve.
    let normalized = normalize_path(&canonical);
    if normalized != canonical {
        loaded.insert(normalized, 0);
    }

    if options.follow_external_files {
        let mut referenced = Vec::new();
        {
            let document = &set.documents[0];
            for diagram in &document.diagrams {
                for node in &diagram.nodes {
                    if let Some(Link::Diagram(spec)) = &node.link
                        && let Some(file) = &spec.file
                    {
                        let base = canonical
                            .parent()
                            .map(Path::to_path_buf)
                            .unwrap_or_else(|| PathBuf::from("."));
                        referenced.push(base.join(file));
                    }
                }
            }
        }
        for file in referenced {
            // A referenced file that does not exist is recorded as an issue
            // by the link resolver instead of aborting the load.
            try_load(&file, options, &mut set, &mut loaded, overlay)?;
        }
    }

    resolve_links(&mut set, &loaded, options)?;
    Ok(set)
}

/// Load a root file plus any transitively referenced files, then resolve links.
pub fn load_file(path: impl AsRef<Path>, options: &LoadOptions) -> Result<DocumentSet, BuildError> {
    load_file_with_overlay(path, &Overlay::new(), options)
}

/// Like [`load_file`], but referenced files may also come from `overlay`.
pub fn load_file_with_overlay(
    path: impl AsRef<Path>,
    overlay: &Overlay,
    options: &LoadOptions,
) -> Result<DocumentSet, BuildError> {
    let mut set = DocumentSet {
        documents: Vec::new(),
        root: 0,
        issues: Vec::new(),
    };
    let mut loaded: BTreeMap<PathBuf, usize> = BTreeMap::new();
    let root = load_recursive(path.as_ref(), options, &mut set, &mut loaded, overlay)?;
    set.root = root;
    resolve_links(&mut set, &loaded, options)?;
    Ok(set)
}

fn load_recursive(
    path: &Path,
    options: &LoadOptions,
    set: &mut DocumentSet,
    loaded: &mut BTreeMap<PathBuf, usize>,
    overlay: &Overlay,
) -> Result<usize, BuildError> {
    match try_load(path, options, set, loaded, overlay)? {
        Some(index) => Ok(index),
        None => Err(BuildError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("cannot open {}", path.display()),
        ))),
    }
}

/// Load one document, returning `None` when the file does not exist and
/// broken links are tolerated (the caller records an issue instead).
fn try_load(
    path: &Path,
    options: &LoadOptions,
    set: &mut DocumentSet,
    loaded: &mut BTreeMap<PathBuf, usize>,
    overlay: &Overlay,
) -> Result<Option<usize>, BuildError> {
    let normalized = normalize_path(path);
    if let Some(&idx) = loaded.get(&normalized) {
        return Ok(Some(idx));
    }

    let (text, source_path) = match overlay.get(&normalized) {
        // In-memory file: it shadows a file with the same path on disk.
        Some(text) => (text.to_string(), normalized.clone()),
        None => {
            let canonical = match std::fs::canonicalize(path) {
                Ok(canonical) => canonical,
                Err(err) => {
                    if options.allow_broken_links && err.kind() == std::io::ErrorKind::NotFound {
                        return Ok(None);
                    }
                    return Err(err.into());
                }
            };
            if let Some(&idx) = loaded.get(&canonical) {
                return Ok(Some(idx));
            }
            let bytes = std::fs::read(&canonical)?;
            let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
            (String::from_utf8_lossy(bytes).into_owned(), canonical)
        }
    };

    let ast_doc = crate::parser::parse(&text)?;
    let document = build(&ast_doc, Some(source_path.clone()))?;

    let idx = set.documents.len();
    set.documents.push(document);
    loaded.insert(source_path.clone(), idx);

    if options.follow_external_files {
        let mut referenced = Vec::new();
        {
            let document = &set.documents[idx];
            for diagram in &document.diagrams {
                for node in &diagram.nodes {
                    if let Some(Link::Diagram(spec)) = &node.link
                        && let Some(file) = &spec.file
                    {
                        let base = source_path
                            .parent()
                            .map(Path::to_path_buf)
                            .unwrap_or_else(|| PathBuf::from("."));
                        referenced.push(base.join(file));
                    }
                }
            }
        }
        for file in referenced {
            try_load(&file, options, set, loaded, overlay)?;
        }
    }

    Ok(Some(idx))
}

/// Find the document a `ref = "file.diag"` points at.
///
/// In-memory files are looked up by their normalized path first (they shadow
/// on-disk files), then the path is canonicalized as usual.
fn find_loaded(loaded: &BTreeMap<PathBuf, usize>, base: &Path, file: &Path) -> Option<usize> {
    let joined = base.join(file);
    if let Some(&idx) = loaded.get(&normalize_path(&joined)) {
        return Some(idx);
    }
    std::fs::canonicalize(&joined)
        .ok()
        .and_then(|canonical| loaded.get(&canonical).copied())
}

fn resolve_links(
    set: &mut DocumentSet,
    loaded: &BTreeMap<PathBuf, usize>,
    options: &LoadOptions,
) -> Result<(), BuildError> {
    for doc_idx in 0..set.documents.len() {
        let source_dir = set.documents[doc_idx]
            .source
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf));
        let source_name = set.documents[doc_idx]
            .source
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<input>".to_string());

        let mut assignments: Vec<(usize, usize, ResolvedLink)> = Vec::new();
        let mut issues: Vec<LinkIssue> = Vec::new();

        {
            let document = &set.documents[doc_idx];
            for (di, diagram) in document.diagrams.iter().enumerate() {
                for (ni, node) in diagram.nodes.iter().enumerate() {
                    let Some(Link::Diagram(spec)) = &node.link else {
                        continue;
                    };

                    let target_doc = match &spec.file {
                        None => Some(doc_idx),
                        Some(file) => {
                            let base = source_dir.clone().unwrap_or_else(|| PathBuf::from("."));
                            find_loaded(loaded, &base, file)
                        }
                    };

                    let Some(target_doc) = target_doc else {
                        issues.push(LinkIssue {
                            file: source_name.clone(),
                            node: node.name.clone(),
                            message: format!(
                                "referenced file not found: {}",
                                spec.file.as_ref().unwrap().display()
                            ),
                        });
                        continue;
                    };

                    let target = &set.documents[target_doc];
                    let target_diagram = match &spec.diagram {
                        Some(id) if target.index.contains_key(id) => Some(id.clone()),
                        Some(id) => {
                            issues.push(LinkIssue {
                                file: source_name.clone(),
                                node: node.name.clone(),
                                message: format!("unknown diagram id: {id:?}"),
                            });
                            continue;
                        }
                        None => {
                            if target.diagrams.len() == 1 {
                                Some(target.diagrams[0].id.clone())
                            } else if target.index.contains_key("default") {
                                Some("default".to_string())
                            } else {
                                issues.push(LinkIssue {
                                    file: source_name.clone(),
                                    node: node.name.clone(),
                                    message: "ambiguous target file: multiple diagrams, none named `default`".to_string(),
                                });
                                continue;
                            }
                        }
                    };

                    if let Some(diagram_id) = target_diagram {
                        assignments.push((
                            di,
                            ni,
                            ResolvedLink {
                                document: target_doc,
                                diagram: diagram_id,
                            },
                        ));
                    }
                }
            }
        }

        for (di, ni, resolved) in assignments {
            set.documents[doc_idx].diagrams[di].nodes[ni].resolved = Some(resolved);
        }
        set.issues.extend(issues);
    }

    if !options.allow_broken_links
        && let Some(first) = set.issues.first()
    {
        return Err(BuildError::Link {
            file: first.file.clone(),
            node: first.node.clone(),
            message: first.message.clone(),
        });
    }

    Ok(())
}

/// Produce a deterministic, line-oriented summary of a document set.
///
/// Used by `--dump-model` and by the model compatibility tests. Lane ids are
/// omitted because upstream generates random UUIDs for anonymous lanes.
pub fn dump(set: &DocumentSet) -> String {
    let mut out = String::new();
    for document in &set.documents {
        for diagram in &document.diagrams {
            out.push_str(&format!("diagram {}\n", diagram.id));
            for (i, lane) in diagram.lanes.iter().enumerate() {
                let nodes: Vec<&str> = lane
                    .nodes
                    .iter()
                    .map(|n| diagram.node(*n).name.as_str())
                    .collect();
                out.push_str(&format!("lane {} nodes={}\n", i, nodes.join(",")));
            }
            for node in &diagram.nodes {
                out.push_str(&format!(
                    "node {} label={} lane={}\n",
                    node.name,
                    node.label,
                    node.lane
                        .map(|l| l.to_string())
                        .unwrap_or_else(|| "-".to_string())
                ));
            }
            for edge in &diagram.edges {
                let dir = match edge.dir {
                    Direction::Forward => "f",
                    Direction::Reverse => "r",
                    Direction::Undirected => "u",
                    Direction::Bidirectional => "b",
                };
                out.push_str(&format!(
                    "edge {} {} {}\n",
                    diagram.node(edge.from).name,
                    diagram.node(edge.to).name,
                    dir
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_str_ok(text: &str) -> DocumentSet {
        build_str(text).unwrap_or_else(|e| panic!("build failed for {text:?}: {e}"))
    }

    #[test]
    fn simple_diagram_structure() {
        let set = build_str_ok(include_str!("../../tests/fixtures/simple.diag"));
        let diagram = &set.root().diagrams[0];
        assert_eq!(diagram.nodes.len(), 3);
        assert_eq!(diagram.lanes.len(), 2);
        assert_eq!(diagram.edges.len(), 2);

        let lane_of = |name: &str| diagram.node(diagram.find_node(name).unwrap()).lane;
        assert_eq!(lane_of("A"), Some(0));
        assert_eq!(lane_of("C"), Some(0));
        assert_eq!(lane_of("B"), Some(1));
    }

    #[test]
    fn nodes_without_lanes_get_a_default_lane() {
        let set = build_str_ok("{ A -> B; }");
        let diagram = &set.root().diagrams[0];
        assert_eq!(diagram.lanes.len(), 1);
        assert_eq!(diagram.node(diagram.find_node("A").unwrap()).lane, Some(0));
        assert_eq!(diagram.node(diagram.find_node("B").unwrap()).lane, Some(0));
    }

    #[test]
    fn lane_is_inherited_from_parent() {
        let set = build_str_ok("{ lane foo { A; } A -> B; }");
        let diagram = &set.root().diagrams[0];
        let a = diagram.find_node("A").unwrap();
        let b = diagram.find_node("B").unwrap();
        assert_eq!(diagram.node(a).lane, Some(0));
        assert_eq!(diagram.node(b).lane, Some(0));
        assert_eq!(diagram.lanes.len(), 1);
    }

    #[test]
    fn unknown_class_is_an_error() {
        let err = build_str("{ A [class = nope]; }").unwrap_err();
        assert!(matches!(err, BuildError::Message(_)));
    }

    #[test]
    fn class_attributes_are_applied_and_overridable() {
        let set = build_str_ok("{ class red [color = \"#f00\"]; A [class = red, shape = box]; }");
        let diagram = &set.root().diagrams[0];
        let a = diagram.node(diagram.find_node("A").unwrap());
        assert_eq!(a.class.as_deref(), Some("red"));
        assert_eq!(a.color.as_deref(), Some("#f00"));
        assert_eq!(a.shape.as_deref(), Some("box"));
    }

    #[test]
    fn lane_block_inline_attrs_set_lane_color() {
        let set = build_str_ok("{ lane foo [color = \"#ff0000\"] { A; } }");
        let diagram = &set.root().diagrams[0];
        assert_eq!(diagram.lanes[0].color.as_deref(), Some("#ff0000"));
    }

    #[test]
    fn node_belongs_to_two_lanes_is_an_error() {
        let err = build_str("{ lane a { X; } lane b { X; } }").unwrap_err();
        assert!(matches!(err, BuildError::Message(_)));
    }

    #[test]
    fn empty_lanes_are_removed() {
        let set = build_str_ok("{ lane empty { } A; }");
        let diagram = &set.root().diagrams[0];
        assert_eq!(diagram.lanes.len(), 1);
        assert_eq!(diagram.node(diagram.find_node("A").unwrap()).lane, Some(0));
    }

    #[test]
    fn duplicate_edges_merge() {
        let set = build_str_ok("{ A -> B; A -> B [color = red]; }");
        let diagram = &set.root().diagrams[0];
        assert_eq!(diagram.edges.len(), 1);
        assert_eq!(diagram.edges[0].color.as_deref(), Some("red"));
    }

    #[test]
    fn explicit_number_and_link() {
        let set = build_str_ok("{ A [numbered = 2, ref = \"#detail\"]; } diagram detail { B; }");
        let diagram = &set.root().diagrams[0];
        let a = diagram.node(diagram.find_node("A").unwrap());
        assert_eq!(a.number, Some(2));
        assert!(matches!(a.link, Some(Link::Diagram(_))));
    }

    #[test]
    fn ref_and_href_together_is_an_error() {
        let err = build_str("{ A [ref = x, href = y]; }").unwrap_err();
        assert!(matches!(err, BuildError::Message(_)));
    }

    #[test]
    fn same_document_link_resolves() {
        let set = build_str_ok("diagram main { A [ref = \"#other\"]; } diagram other { B; }");
        let diagram = &set.root().diagrams[0];
        let a = diagram.node(diagram.find_node("A").unwrap());
        let resolved = a.resolved.as_ref().expect("resolved");
        assert_eq!(resolved.document, 0);
        assert_eq!(resolved.diagram, "other");
    }

    #[test]
    fn broken_link_errors_by_default() {
        let err =
            build_str("diagram main { A [ref = \"#missing\"]; } diagram other { B; }").unwrap_err();
        assert!(matches!(err, BuildError::Link { .. }));
    }

    #[test]
    fn legend_is_parsed_into_ir() {
        let set = build_str_ok(
            "{ class red [color = red]; legend [title = L] { class red; numbered; } A; }",
        );
        let leg = set.root().diagrams[0].legend.as_ref().expect("legend");
        assert_eq!(leg.title.as_deref(), Some("L"));
        assert_eq!(leg.entries.len(), 2);
    }

    #[test]
    fn autolane_plugin_assigns_lanes() {
        let set =
            build_str_ok("{ plugin autolane; lane dev { } lane ops { } dev_build; ops_deploy; }");
        let diagram = &set.root().diagrams[0];
        let build = diagram.node(diagram.find_node("dev_build").unwrap());
        let deploy = diagram.node(diagram.find_node("ops_deploy").unwrap());
        assert_eq!(build.label, "build");
        assert_eq!(build.lane, Some(0));
        assert_eq!(deploy.label, "deploy");
        assert_eq!(deploy.lane, Some(1));
    }

    #[test]
    fn many_diagrams_build_in_definition_order() {
        let mut src = String::new();
        for i in 0..64 {
            src.push_str(&format!("diagram d{i} {{ A{i} -> B{i}; }}\n"));
        }
        let first = build_str_ok(&src);
        let second = build_str_ok(&src);

        assert_eq!(first.root().diagrams.len(), 64);
        for (i, diagram) in first.root().diagrams.iter().enumerate() {
            assert_eq!(diagram.id, format!("d{i}"));
        }
        // The parallel worker pool must not change the definition-ordered output.
        assert_eq!(dump(&first), dump(&second));
    }

    #[test]
    fn overlay_files_resolve_like_files_on_disk() {
        let dir = crate::test_support::TempDir::new("swimflow-overlay-");

        let mut overlay = Overlay::new();
        overlay.insert(dir.join("other.diag"), "diagram detail { B; }");

        let options = LoadOptions {
            allow_broken_links: true,
            follow_external_files: true,
        };
        let set = load_str_with_overlay(
            "diagram main { A [ref = \"other.diag#detail\"]; }",
            dir.join("main.diag"),
            &overlay,
            &options,
        )
        .expect("load with overlay");

        assert_eq!(set.documents.len(), 2);
        let diagram = &set.root().diagrams[0];
        let a = diagram.node(diagram.find_node("A").unwrap());
        let resolved = a.resolved.as_ref().expect("resolved");
        assert_eq!(resolved.document, 1);
        assert_eq!(resolved.diagram, "detail");
        assert!(set.issues.is_empty(), "{:?}", set.issues);
    }

    #[test]
    fn overlay_files_shadow_files_on_disk() {
        let dir = crate::test_support::TempDir::new("swimflow-overlay-shadow-");
        std::fs::write(dir.join("other.diag"), "diagram detail { OnDisk; }").unwrap();

        let mut overlay = Overlay::new();
        overlay.insert(dir.join("other.diag"), "diagram detail { InMemory; }");

        let options = LoadOptions::default();
        let set = load_str_with_overlay(
            "diagram main { A [ref = \"other.diag#detail\"]; }",
            dir.join("main.diag"),
            &overlay,
            &options,
        )
        .expect("load with overlay");

        let target = &set.documents[1].diagrams[0];
        assert!(target.find_node("InMemory").is_some(), "the overlay wins");
        assert!(target.find_node("OnDisk").is_none());
    }

    #[test]
    fn a_missing_referenced_file_is_an_issue_when_broken_links_are_allowed() {
        let dir = crate::test_support::TempDir::new("swimflow-missing-ref-");

        let options = LoadOptions {
            allow_broken_links: true,
            follow_external_files: true,
        };
        let set = load_str_with_overlay(
            "diagram main { A [ref = \"missing.diag\"]; }",
            dir.join("main.diag"),
            &Overlay::new(),
            &options,
        )
        .expect("a missing file must not abort the load");

        assert_eq!(set.documents.len(), 1);
        assert_eq!(set.issues.len(), 1);
        assert!(
            set.issues[0].message.contains("not found"),
            "{:?}",
            set.issues[0]
        );
    }

    #[test]
    fn a_missing_referenced_file_is_an_error_by_default() {
        let dir = crate::test_support::TempDir::new("swimflow-missing-ref-");

        let err = load_str_with_overlay(
            "diagram main { A [ref = \"missing.diag\"]; }",
            dir.join("main.diag"),
            &Overlay::new(),
            &LoadOptions::default(),
        )
        .expect_err("a missing file is an error by default");
        assert!(matches!(err, BuildError::Io(_)), "{err:?}");
    }

    #[test]
    fn overlay_paths_are_normalized() {
        let mut overlay = Overlay::new();
        overlay.insert("/tmp/dir/./sub/../other.diag", "x");
        assert!(overlay.get(Path::new("/tmp/dir/other.diag")).is_some());
        assert!(
            overlay
                .get(Path::new("/tmp/dir/sub/../other.diag"))
                .is_some()
        );
        assert!(
            overlay
                .get(Path::new("/tmp/dir/nested/other.diag"))
                .is_none()
        );
        assert_eq!(overlay.len(), 1);
    }
}
