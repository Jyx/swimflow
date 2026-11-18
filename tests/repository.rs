//! Repository metadata and cleanliness tests.
//!
//! These keep the man page, license, Cargo metadata, example corpus and
//! repository hygiene in sync without depending on generated artifacts or the
//! reference Python environment.

use std::path::{Path, PathBuf};

use clap::CommandFactory;
use swimflow::cli::Cli;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read source directory") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn man_page_documents_every_option() {
    let man = std::fs::read_to_string(root().join("man/swimflow.1")).expect("man/swimflow.1");
    // roff escapes hyphens as `\-`; normalize before matching option names.
    let man = man.replace("\\-", "-");
    let command = Cli::command();
    for arg in command.get_arguments() {
        let Some(long) = arg.get_long() else {
            continue;
        };
        assert!(
            man.contains(&format!("--{long}")),
            "man page is missing --{long}"
        );
    }
}

#[test]
fn man_page_is_a_structured_manual() {
    let man = std::fs::read_to_string(root().join("man/swimflow.1")).unwrap();
    for section in [".SH NAME", ".SH SYNOPSIS", ".SH DESCRIPTION", ".SH OPTIONS"] {
        assert!(man.contains(section), "man page is missing {section}");
    }
    assert!(man.contains(".TH SWIMFLOW 1"));
    assert!(man.contains("activity/swimlane, sequence, tree, treemap and manifest"));
    assert!(man.contains("diagrams from a text DSL"));
}

#[test]
fn man_page_documents_shell_completion() {
    let man = std::fs::read_to_string(root().join("man/swimflow.1")).unwrap();
    assert!(man.contains(".SH SHELL COMPLETION"));
    assert!(man.contains("completions/swimflow.bash"));
}

#[test]
fn license_is_apache_2() {
    let license = std::fs::read_to_string(root().join("LICENSE")).unwrap();
    assert!(license.contains("Apache License"));
    assert!(license.contains("Version 2.0, January 2004"));
}

#[test]
fn cargo_metadata_matches_the_project() {
    let cargo = std::fs::read_to_string(root().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("name = \"swimflow\""));
    assert!(cargo.contains("license = \"Apache-2.0\""));
    assert!(cargo.contains("readme = \"README.rst\""));
    assert!(cargo.contains("[[bin]]"));
}

#[test]
fn example_corpus_is_present() {
    let examples = root().join("examples");
    assert!(examples.join("README.rst").is_file());
    assert!(examples.join("zoom-demo.diag").is_file());
    assert!(examples.join("cross-file/overview.diag").is_file());
}

#[test]
fn bash_completion_is_present() {
    assert!(root().join("completions/swimflow.bash").is_file());
}

#[test]
fn install_script_matches_the_release_assets() {
    let script = std::fs::read_to_string(root().join("install.sh")).expect("install.sh");
    assert!(
        script.starts_with("#!/bin/sh"),
        "install.sh must stay POSIX sh"
    );
    assert!(
        script.contains("SHA256SUMS"),
        "install.sh verifies checksums"
    );
    assert!(script.contains("Jyx/swimflow"));

    // The installer maps each supported platform to the archive the release
    // workflow publishes.
    let workflow = std::fs::read_to_string(root().join(".github/workflows/release.yml"))
        .expect("release workflow");
    for target in [
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-musl",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
    ] {
        assert!(script.contains(target), "install.sh is missing {target}");
    }
    for target in [
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-musl",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ] {
        assert!(workflow.contains(target), "release.yml is missing {target}");
    }

    let readme = std::fs::read_to_string(root().join("README.rst")).unwrap();
    assert!(
        readme.contains("install.sh"),
        "README must document the installer"
    );
}

#[test]
fn serve_example_env_is_present_and_documented() {
    let env = std::fs::read_to_string(root().join(".env.example")).expect(".env.example");
    for key in [
        "SWIMFLOW_DIR",
        "SWIMFLOW_HOST",
        "SWIMFLOW_PORT",
        "SWIMFLOW_DEBOUNCE_MS",
        "SWIMFLOW_SCALE",
        "SWIMFLOW_TOKEN",
    ] {
        assert!(env.contains(key), ".env.example is missing {key}");
    }
}

#[test]
fn tests_use_self_cleaning_scratch_directories() {
    // Hand-rolled PID-named directories under the system temp dir leaked tens
    // of thousands of `swimflow-*` folders. Tests must use `TempDir`
    // (`src/test_support.rs`, `tests/common/mod.rs`) which removes itself.
    let root = root();
    let mut sources = Vec::new();
    rust_sources(&root.join("src"), &mut sources);
    rust_sources(&root.join("tests"), &mut sources);
    // Split so this test does not match itself.
    let needle = ["std::env::", "temp_dir()"].concat();
    // The helpers themselves are the one place that may name the temp dir.
    let helpers = [
        root.join("src/test_support.rs"),
        root.join("tests/common/mod.rs"),
    ];
    let offenders: Vec<String> = sources
        .iter()
        .filter(|path| !helpers.contains(path))
        .filter(|path| {
            std::fs::read_to_string(path)
                .expect("read source")
                .contains(&needle)
        })
        .map(|path| path.strip_prefix(&root).unwrap().display().to_string())
        .collect();
    assert!(
        offenders.is_empty(),
        "use TempDir instead of hand-rolled scratch dirs: {offenders:?}"
    );
}
