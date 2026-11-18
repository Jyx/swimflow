//! Tests for the helper scripts in `scripts/`.
//!
//! `scripts/cim-mfst.py` turns the gits of cim manifest targets
//! into a `.mfst` manifest. The test builds a small manifest folder, runs the
//! script and renders its output with the manifest plugin.
//!
//! The script needs PyYAML (`scripts/requirements.txt`). The tests use the
//! virtual environment `scripts/.venv` when it exists, otherwise `python3` if
//! it can import `yaml`, and are skipped when neither works.

use std::path::{Path, PathBuf};
use std::process::Command;

mod common;

use swimflow::plugin::manifest::model::{self, Mark};
use swimflow::plugin::manifest::parser;

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/cim-mfst.py")
}

/// A Python that can import PyYAML, if there is one.
fn python() -> Option<PathBuf> {
    let venv = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/.venv/bin/python");
    [venv, PathBuf::from("python3")].into_iter().find(|python| {
        Command::new(python)
            .args(["-c", "import yaml"])
            .output()
            .is_ok_and(|output| output.status.success())
    })
}

fn write(dir: &Path, target: &str, yaml: &str) {
    let folder = dir.join("targets").join(target);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("sdk.yml"), yaml).unwrap();
}

/// A base target and a target extending it, using the YAML the reader must
/// cope with: comments, quotes, text blocks, inline lists and an alias.
fn manifests(dir: &Path) {
    write(
        dir,
        "base",
        "# A base target\n\
         common: &steps\n  build: |\n    make all # not a comment\n    make install\n\
         gits:\n  - name: core   # the core\n    url: https://example.com/core.git\n    \
         commit: main\n    group: [one, two]\n    build: *steps\n  \
         - name: \"ui\"\n    commit: 'v1.0'\n  - name: net\n    commit: main\n\
         install:\n  - name: tool\n    commands: |\n      echo gits: not a key\n",
    );
    write(
        dir,
        "child",
        "extends: base\n\
         gits:\n  - name: extra\n    commit: main\n\
         overlay:\n  gits:\n    remove:\n      - net\n    modify:\n      \
         - name: ui\n        commit: v2.0\n",
    );
    write(dir, "empty", "gits:\n");
}

fn run(python: &Path, args: &[&std::ffi::OsStr]) -> std::process::Output {
    Command::new(python)
        .arg(script())
        .args(args)
        .output()
        .expect("run python3")
}

#[test]
fn the_script_writes_a_manifest_with_inheritance() {
    let Some(python) = python() else {
        eprintln!("no Python with PyYAML (see scripts/requirements.txt); skipping");
        return;
    };
    let dir = common::TempDir::new("swimflow-script-");
    let repo = dir.join("repo");
    manifests(&repo);
    let out = dir.join("gits");

    let output = run(&python, &[repo.as_os_str(), "-o".as_ref(), out.as_os_str()]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // `.mfst` is added to an output without an extension.
    let source = std::fs::read_to_string(dir.join("gits.mfst")).expect("gits.mfst");
    assert!(source.contains("group \"repo\""), "{source}");
    assert!(
        source.contains("column \"child\" [extends = \"base\"]"),
        "{source}"
    );
    assert!(source.contains("column \"empty\";"), "{source}");
    // Only the gits are read: not the install steps nor text blocks.
    assert!(!source.contains("tool"), "{source}");
    assert!(!source.contains("not a key"), "{source}");

    let document = model::build(&parser::parse(&source).expect("parse")).expect("model");
    let d = &document.diagrams[0];
    let row = |name: &str| d.rows.iter().position(|r| r.name == name).unwrap();
    let col = |name: &str| d.columns.iter().position(|c| c.name == name).unwrap();
    let (base, child) = (col("base"), col("child"));
    assert_eq!(
        d.cells[row("core")][child].mark,
        Mark::Inherited { from: base }
    );
    assert_eq!(
        d.cells[row("net")][child].mark,
        Mark::Removed { from: base }
    );
    assert_eq!(
        d.cells[row("ui")][child].mark,
        Mark::Override { from: base }
    );
    assert_eq!(
        d.cells[row("ui")][child].note.as_deref(),
        Some("commit: v2.0")
    );
    assert_eq!(d.cells[row("extra")][child].mark, Mark::Direct);
}

#[test]
fn several_folders_get_one_group_each_and_unique_columns() {
    let Some(python) = python() else {
        eprintln!("no Python with PyYAML (see scripts/requirements.txt); skipping");
        return;
    };
    let dir = common::TempDir::new("swimflow-script-two-");
    let (first, second) = (dir.join("first"), dir.join("second"));
    manifests(&first);
    manifests(&second);

    let output = run(
        &python,
        &[
            first.as_os_str(),
            second.as_os_str(),
            "--title".as_ref(),
            "T".as_ref(),
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let source = String::from_utf8(output.stdout).unwrap();
    assert!(source.contains("manifest \"T\""), "{source}");
    // The same target name in two folders: the columns are qualified, the
    // label stays the name, and extends stays inside its folder.
    assert!(
        source.contains("column \"second/child\" [label = \"child\", extends = \"second/base\"]"),
        "{source}"
    );
    let document = model::build(&parser::parse(&source).expect("parse")).expect("model");
    assert_eq!(document.diagrams[0].groups.len(), 2);
    assert_eq!(document.diagrams[0].columns.len(), 6);
}

#[test]
fn a_missing_parent_is_a_warning_and_a_bad_folder_an_error() {
    let Some(python) = python() else {
        eprintln!("no Python with PyYAML (see scripts/requirements.txt); skipping");
        return;
    };
    let dir = common::TempDir::new("swimflow-script-errors-");
    write(&dir, "orphan", "extends: nowhere\ngits:\n  - name: a\n");
    let output = run(&python, &[dir.as_os_str()]);
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("nowhere"), "{stderr}");
    let source = String::from_utf8(output.stdout).unwrap();
    model::build(&parser::parse(&source).expect("parse")).expect("model");

    let missing = dir.join("missing");
    let output = run(&python, &[missing.as_os_str()]);
    assert!(!output.status.success());
    let empty = dir.join("targets").join("orphan");
    std::fs::remove_file(empty.join("sdk.yml")).unwrap();
    let output = run(&python, &[empty.as_os_str()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no sdk.yml"));

    // A broken file is reported with its path.
    write(&dir, "broken", "gits:\n  - name: [unclosed\n");
    let output = run(&python, &[dir.as_os_str()]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("broken") && stderr.contains("sdk.yml"),
        "{stderr}"
    );
}

#[test]
fn match_and_exclude_select_targets_by_pattern() {
    let Some(python) = python() else {
        eprintln!("no Python with PyYAML (see scripts/requirements.txt); skipping");
        return;
    };
    let dir = common::TempDir::new("swimflow-script-filter-");
    let repo = dir.join("repo");
    manifests(&repo);
    let source = |args: &[&str]| {
        let mut all = vec![repo.as_os_str()];
        all.extend(args.iter().map(std::ffi::OsStr::new));
        let output = run(&python, &all);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };

    // Exact names, comma-separated; the dropped parent's gits become direct.
    let only = source(&["-m", "child,empty"]);
    assert!(!only.contains("column \"base\""), "{only}");
    assert!(only.contains("column \"child\" {"), "{only}");
    assert!(only.contains("column \"empty\";"), "{only}");
    let document = model::build(&parser::parse(&only).expect("parse")).expect("model");
    let d = &document.diagrams[0];
    let rows: Vec<&str> = d.rows.iter().map(|r| r.name.as_str()).collect();
    for git in ["core", "ui", "extra"] {
        assert!(rows.contains(&git), "{git} missing: {only}");
    }
    assert!(!rows.contains(&"net"), "net is removed by child: {only}");

    // Globs, repeated flags and exclusion.
    let globbed = source(&["-m", "b*", "-m", "e?pty", "-x", "base"]);
    assert!(globbed.contains("column \"empty\";"), "{globbed}");
    assert!(!globbed.contains("\"base\""), "{globbed}");

    let output = run(
        &python,
        &[repo.as_os_str(), "-m".as_ref(), "nothing".as_ref()],
    );
    assert!(!output.status.success());
}

// ---------------------------------------------------------------------------
// scripts/cim-tree.py
// ---------------------------------------------------------------------------

fn tree_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/cim-tree.py")
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .expect("run git");
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
}

/// A manifest git repository with local release branches (no `origin`).
fn tree_repo(repo: &Path) {
    manifests(repo);
    write(repo, "base-extra", "gits:\n");
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["add", "."]);
    git(repo, &["commit", "-q", "-m", "init"]);
    for branch in [
        "base-v0.10.0",
        "base-v0.9.0",
        "base-v1.0.0-d.1",
        "base-extra-v2.0",
        "base-wip",
        "child-v1.0",
    ] {
        git(repo, &["branch", branch]);
    }
}

fn run_tree(python: &Path, repo: &Path, args: &[&str]) -> String {
    let output = Command::new(python)
        .arg(tree_script())
        .arg(repo)
        .args(args)
        .output()
        .expect("run python3");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn tree_names(node: &swimflow::plugin::tree::ast::Node) -> Vec<String> {
    node.children.iter().map(|c| c.name.clone()).collect()
}

#[test]
fn the_tree_script_nests_targets_and_releases() {
    let Some(python) = python() else {
        eprintln!("no Python with PyYAML (see scripts/requirements.txt); skipping");
        return;
    };
    if Command::new("git").arg("--version").output().is_err() {
        eprintln!("no git; skipping");
        return;
    }
    use swimflow::plugin::tree::parser as tree_parser;
    let dir = common::TempDir::new("swimflow-script-tree-");
    let repo = dir.join("repo");
    tree_repo(&repo);

    let source = run_tree(&python, &repo, &[]);
    let document = tree_parser::parse(&source).expect("parse");
    let tree = &document.trees[0];
    assert_eq!(tree.root.name.as_deref(), Some("repo"));
    let top: Vec<&str> = tree.children.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(top, ["base", "base-extra", "empty"], "{source}");
    let base = &tree.children[0];
    assert_eq!(tree_names(base), ["child", "releases"], "{source}");
    // Natural order; `base-wip` is not a version; `base-extra-v2.0` belongs
    // to `base-extra`.
    assert_eq!(
        tree_names(&base.children[1]),
        ["v0.9.0", "v0.10.0", "v1.0.0-d.1"],
        "{source}"
    );
    assert_eq!(tree_names(&tree.children[1].children[0]), ["v2.0"]);
    assert_eq!(tree_names(&base.children[0].children[0]), ["v1.0"]);

    // Filtering: a dropped parent lifts its children, versions are filtered.
    let source = run_tree(&python, &repo, &["-x", "base", "-X", "*-d.*,v0.9*"]);
    let tree = &tree_parser::parse(&source).expect("parse").trees[0];
    let top: Vec<&str> = tree.children.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(top, ["base-extra", "child", "empty"], "{source}");
    assert!(!source.contains("v0.10.0"), "{source}");

    let source = run_tree(
        &python,
        &repo,
        &["-m", "base", "-M", "v1*", "--version-pattern", "*"],
    );
    assert!(source.contains("\"v1.0.0-d.1\""), "{source}");
    assert!(
        !source.contains("v0.9.0") && !source.contains("child"),
        "{source}"
    );

    let source = run_tree(&python, &repo, &["--no-releases"]);
    assert!(!source.contains("releases"), "{source}");
}

#[test]
fn the_tree_script_reads_a_git_url_as_upstream_has_it() {
    let Some(python) = python() else {
        eprintln!("no Python with PyYAML (see scripts/requirements.txt); skipping");
        return;
    };
    if Command::new("git").arg("--version").output().is_err() {
        eprintln!("no git; skipping");
        return;
    }
    use swimflow::plugin::tree::parser as tree_parser;
    let dir = common::TempDir::new("swimflow-script-tree-url-");
    let repo = dir.join("upstream.git");
    tree_repo(&repo);
    let url = format!("file://{}", repo.display());

    let output = Command::new(&python)
        .arg(tree_script())
        .arg(&url)
        .output()
        .expect("run python3");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let source = String::from_utf8(output.stdout).unwrap();
    let tree = &tree_parser::parse(&source).expect("parse").trees[0];
    // The name drops `.git`; the branches come from the URL itself.
    assert_eq!(tree.root.name.as_deref(), Some("upstream"), "{source}");
    assert!(source.contains("\"v0.10.0\""), "{source}");

    // URL#BRANCH reads the manifests of that branch.
    write(&repo, "later", "gits:\n");
    git(&repo, &["checkout", "-q", "-b", "next"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "later"]);
    git(&repo, &["checkout", "-q", "main"]);
    let main = run_tree(&python, Path::new(&url), &[]);
    let next = run_tree(&python, Path::new(&format!("{url}#next")), &[]);
    assert!(!main.contains("\"later\""), "{main}");
    assert!(next.contains("\"later\""), "{next}");

    let output = Command::new(&python)
        .arg(tree_script())
        .arg(format!("file://{}", dir.join("missing").display()))
        .output()
        .expect("run python3");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot clone"));
}

#[test]
fn several_tree_sources_share_an_artificial_root() {
    let Some(python) = python() else {
        eprintln!("no Python with PyYAML (see scripts/requirements.txt); skipping");
        return;
    };
    if Command::new("git").arg("--version").output().is_err() {
        eprintln!("no git; skipping");
        return;
    }
    use swimflow::plugin::tree::parser as tree_parser;
    let dir = common::TempDir::new("swimflow-script-tree-many-");
    let (first, second, third) = (dir.join("first"), dir.join("second"), dir.join("third"));
    for repo in [&first, &second, &third] {
        tree_repo(repo);
    }
    let output = Command::new(&python)
        .arg(tree_script())
        .args([&second, &first, &third])
        .output()
        .expect("run python3");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let source = String::from_utf8(output.stdout).unwrap();
    let document = tree_parser::parse(&source).expect("parse");
    assert_eq!(document.trees.len(), 1, "{source}");
    let tree = &document.trees[0];
    assert_eq!(tree.root.name.as_deref(), Some("manifests"), "{source}");
    // One subtree per repository, in the order given, holding its targets.
    let repos: Vec<&str> = tree.children.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(repos, ["second", "first", "third"], "{source}");
    assert_eq!(
        tree_names(&tree.children[0]),
        ["base", "base-extra", "empty"]
    );

    let source = run_tree(&python, &first, &["--root", "ignored"]);
    let tree = &tree_parser::parse(&source).expect("parse").trees[0];
    assert_eq!(tree.root.name.as_deref(), Some("first"), "{source}");

    let output = Command::new(&python)
        .arg(tree_script())
        .args([first.as_os_str(), second.as_os_str()])
        .args(["--root", "all"])
        .output()
        .expect("run python3");
    let source = String::from_utf8(output.stdout).unwrap();
    assert!(source.contains("root \"all\""), "{source}");
}

#[test]
fn a_git_name_with_a_path_is_its_last_part_unless_full_names() {
    let Some(python) = python() else {
        eprintln!("no Python with PyYAML (see scripts/requirements.txt); skipping");
        return;
    };
    let dir = common::TempDir::new("swimflow-script-paths-");
    let (first, second) = (dir.join("first"), dir.join("second"));
    write(&first, "a", "gits:\n  - name: sdk\n");
    write(
        &second,
        "b",
        "gits:\n  - name: ws/sdk\n  - name: other\n\
         overlay:\n  gits:\n    modify:\n      - name: ws/sdk\n        commit: v2\n",
    );
    write(
        &second,
        "c",
        "extends: b\noverlay:\n  gits:\n    remove:\n      - ws/sdk\n",
    );

    let output = run(&python, &[first.as_os_str(), second.as_os_str()]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let source = String::from_utf8(output.stdout).unwrap();
    assert!(!source.contains("ws/sdk"), "{source}");
    assert!(source.contains("remove \"sdk\";"), "{source}");
    let document = model::build(&parser::parse(&source).expect("parse")).expect("model");
    let d = &document.diagrams[0];
    let names: Vec<&str> = d.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names.iter().filter(|n| **n == "sdk").count(),
        1,
        "{names:?}"
    );

    let output = run(
        &python,
        &[
            first.as_os_str(),
            second.as_os_str(),
            "--full-names".as_ref(),
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let source = String::from_utf8(output.stdout).unwrap();
    assert!(source.contains("\"ws/sdk\";"), "{source}");
    assert!(source.contains("remove \"ws/sdk\";"), "{source}");
}
