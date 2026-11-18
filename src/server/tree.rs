//! Recursive file tree for the live-preview sidebar.
//!
//! The tree is built **per diagram type**: a `.diag` file and a `.seq` file
//! live in different lists, because the editor, the syntax highlighting and
//! the export formats depend on the type.

use std::collections::BTreeMap;
use std::path::Path;

use crate::plugin::TypeInfo;

use super::ServerError;

/// One node of the sidebar tree. `rel` is the `/`-separated path relative to
/// the served directory and is what the client sends back in `path=`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub rel: String,
    pub is_dir: bool,
    pub children: Vec<Entry>,
}

/// A file or directory found on disk.
#[derive(Debug, Clone)]
struct Node {
    name: String,
    rel: String,
    is_dir: bool,
    /// The diagram type of a file, if it is a diagram at all.
    kind: Option<String>,
    children: Vec<Node>,
}

/// Scan `root` for diagram files and the directories that contain them.
///
/// The result holds one tree per diagram type. Hidden entries and symbolic
/// links are skipped (the latter avoid cycles), directories without any
/// descendant diagram are pruned, and the result is sorted deterministically
/// with directories before files.
pub fn scan(root: &Path, types: &[TypeInfo]) -> Result<BTreeMap<String, Vec<Entry>>, ServerError> {
    let nodes = scan_dir(root, "", types)?;

    let mut trees = BTreeMap::new();
    for info in types {
        trees.insert(info.name.to_string(), project(&nodes, info.name));
    }
    Ok(trees)
}

/// Scan the directory tree once, remembering the type of every diagram file.
fn scan_dir(dir: &Path, prefix: &str, types: &[TypeInfo]) -> Result<Vec<Node>, ServerError> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            let rel = join_rel(prefix, &name);
            let children = scan_dir(&entry.path(), &rel, types)?;
            if !children.is_empty() {
                dirs.push(Node {
                    name,
                    rel,
                    is_dir: true,
                    kind: None,
                    children,
                });
            }
        } else if file_type.is_file() {
            let kind = type_of(&name, types);
            if let Some(kind) = kind {
                files.push(Node {
                    rel: join_rel(prefix, &name),
                    name,
                    is_dir: false,
                    kind: Some(kind.to_string()),
                    children: Vec::new(),
                });
            }
        }
    }

    dirs.sort_by(|a, b| a.name.cmp(&b.name));
    files.sort_by(|a, b| a.name.cmp(&b.name));
    dirs.extend(files);
    Ok(dirs)
}

/// Keep the nodes that belong to `kind`, pruning the empty directories.
fn project(nodes: &[Node], kind: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for node in nodes {
        if node.is_dir {
            let children = project(&node.children, kind);
            if !children.is_empty() {
                out.push(Entry {
                    name: node.name.clone(),
                    rel: node.rel.clone(),
                    is_dir: true,
                    children,
                });
            }
        } else if node.kind.as_deref() == Some(kind) {
            out.push(Entry {
                name: node.name.clone(),
                rel: node.rel.clone(),
                is_dir: false,
                children: Vec::new(),
            });
        }
    }
    out
}

/// The diagram type of a file, decided by its extension.
fn type_of(name: &str, types: &[TypeInfo]) -> Option<&'static str> {
    let extension = Path::new(name).extension().and_then(|ext| ext.to_str())?;
    types
        .iter()
        .find(|info| info.claims(extension))
        .map(|info| info.name)
}

fn join_rel(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}/{name}")
    }
}

/// Render a tree as nested `<ul>`/`<details>` markup.
pub fn render(entries: &[Entry]) -> String {
    if entries.is_empty() {
        return "<p class=\"tree-empty\">No diagram files found.</p>".to_string();
    }
    let mut out = String::from("<ul class=\"tree\">");
    render_entries(entries, &mut out);
    out.push_str("</ul>");
    out
}

fn render_entries(entries: &[Entry], out: &mut String) {
    for entry in entries {
        if entry.is_dir {
            out.push_str("<li class=\"dir\"><details open><summary>");
            out.push_str(&super::html_escape(&entry.name));
            out.push_str("</summary><ul class=\"tree\">");
            render_entries(&entry.children, out);
            out.push_str("</ul></details></li>");
        } else {
            out.push_str("<li class=\"file\"><a href=\"#\" data-path=\"");
            out.push_str(&super::html_escape(&entry.rel));
            out.push_str("\" title=\"");
            out.push_str(&super::html_escape(&entry.rel));
            out.push_str("\">");
            out.push_str(&super::html_escape(&entry.name));
            out.push_str("</a></li>");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::Registry;

    fn types() -> Vec<TypeInfo> {
        Registry::builtin().types()
    }

    fn temp_dir(name: &str) -> crate::test_support::TempDir {
        let dir = crate::test_support::TempDir::new(&format!("swimflow-server-tree-{name}-"));
        std::fs::create_dir_all(dir.join("sub/deep")).unwrap();
        std::fs::write(dir.join("top.diag"), "{ A; }\n").unwrap();
        std::fs::write(dir.join("chat.seq"), "sequence { }\n").unwrap();
        std::fs::write(dir.join("sub/inner.diag"), "{ A; }\n").unwrap();
        std::fs::write(dir.join("sub/deep/bottom.seq"), "sequence { }\n").unwrap();
        std::fs::write(dir.join("notes.txt"), "ignore me\n").unwrap();
        std::fs::write(dir.join(".hidden.diag"), "{ A; }\n").unwrap();
        std::fs::create_dir_all(dir.join("empty")).unwrap();
        dir
    }

    #[test]
    fn scans_nested_diagrams_and_prunes_empty_or_hidden_entries() {
        let dir = temp_dir("scan");
        let trees = scan(&dir, &types()).unwrap();

        let activity = &trees["activity"];
        let names: Vec<&str> = activity.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["sub", "top.diag"]);

        let sub = &activity[0];
        let sub_names: Vec<&str> = sub.children.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(sub_names, vec!["inner.diag"]);
        assert_eq!(sub.children[0].rel, "sub/inner.diag");
    }

    #[test]
    fn every_type_gets_its_own_tree() {
        let dir = temp_dir("types");
        let trees = scan(&dir, &types()).unwrap();

        let activity = &trees["activity"];
        let names: Vec<&str> = activity.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["sub", "top.diag"]);

        let sequence = &trees["sequence"];
        let names: Vec<&str> = sequence.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["sub", "chat.seq"]);
        let sub = sequence
            .iter()
            .find(|e| e.is_dir)
            .expect("the sub directory");
        let deep = &sub.children[0];
        assert_eq!(deep.rel, "sub/deep");
        assert_eq!(deep.children[0].rel, "sub/deep/bottom.seq");
    }

    #[test]
    fn renders_links_for_every_diagram() {
        let dir = temp_dir("render");
        let trees = scan(&dir, &types()).unwrap();
        let html = render(&trees["activity"]);
        assert!(html.contains("data-path=\"top.diag\""), "{html}");
        assert!(html.contains("data-path=\"sub/inner.diag\""), "{html}");
        assert!(!html.contains("notes.txt"), "{html}");
        assert!(!html.contains(".hidden.diag"), "{html}");
        assert!(!html.contains("chat.seq"), "{html}");
    }

    #[test]
    fn empty_directory_renders_a_placeholder() {
        let dir = temp_dir("empty-scan");
        std::fs::remove_dir_all(&dir).unwrap();
        std::fs::create_dir_all(dir.join("empty")).unwrap();
        let trees = scan(&dir, &types()).unwrap();
        assert!(render(&trees["activity"]).contains("No diagram files found"));
    }
}
