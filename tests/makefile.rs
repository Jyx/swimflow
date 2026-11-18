//! Makefile workflow tests.
//!
//! These use `make -n` so they check the recipes, format selection and install
//! destination without building or writing anything. If `make` is not
//! installed the tests are skipped, following the pattern used by the Neovim
//! smoke tests.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn make(args: &[&str]) -> Option<std::process::Output> {
    match Command::new("make")
        .current_dir(root())
        .env_remove("MAKEFLAGS")
        .args(args)
        .output()
    {
        Ok(output) => Some(output),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => panic!("failed to run make: {err}"),
    }
}

fn dry_run(args: &[&str]) -> Option<String> {
    let output = make(args)?;
    assert!(
        output.status.success(),
        "make {args:?} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn collect_diagrams(dir: &Path, prefix: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();

    for path in entries {
        if path.is_dir() {
            collect_diagrams(&path, prefix, out);
        } else if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("diag") | Some("seq") | Some("tree") | Some("tmap") | Some("mfst")
        ) {
            out.push(
                path.strip_prefix(prefix)
                    .expect("relative example path")
                    .into(),
            );
        }
    }
}

fn example_diagrams() -> Vec<PathBuf> {
    let examples = root().join("examples");
    let mut diagrams = Vec::new();
    collect_diagrams(&examples, &examples, &mut diagrams);
    diagrams
}

#[test]
fn top_level_targets_are_defined() {
    for target in [
        "debug",
        "release",
        "install",
        "serve",
        "examples",
        "clean",
        "clean-all",
    ] {
        if dry_run(&["--no-print-directory", "-n", target]).is_none() {
            eprintln!("make not installed; skipping Makefile target test");
            return;
        }
    }
}

#[test]
fn examples_build_svg_by_default() {
    let Some(stdout) = dry_run(&["--no-print-directory", "-C", "examples", "-n"]) else {
        eprintln!("make not installed; skipping Makefile examples test");
        return;
    };

    for source in example_diagrams() {
        let source = source.to_string_lossy().replace('\\', "/");
        let svg = Path::new(&source).with_extension("svg");
        assert!(
            stdout.contains(&format!("\"{source}\" -f \"svg\" -o \"{}\"", svg.display())),
            "default examples recipe does not render {source}:\n{stdout}"
        );
        assert!(
            !stdout.contains(&format!(
                "\"{}\"",
                Path::new(&source).with_extension("html").display()
            )),
            "default examples recipe unexpectedly renders HTML for {source}"
        );
    }
}

#[test]
fn examples_type_selects_the_output_format() {
    let Some(stdout) = dry_run(&["--no-print-directory", "-C", "examples", "-n", "TYPE=html"])
    else {
        eprintln!("make not installed; skipping Makefile examples test");
        return;
    };

    for source in example_diagrams() {
        let source = source.to_string_lossy().replace('\\', "/");
        let html = Path::new(&source).with_extension("html");
        assert!(
            stdout.contains(&format!(
                "\"{source}\" -f \"html\" -o \"{}\"",
                html.display()
            )),
            "TYPE=html does not render {source}:\n{stdout}"
        );
        assert!(
            !stdout.contains(&format!(
                "\"{}\"",
                Path::new(&source).with_extension("svg").display()
            )),
            "TYPE=html unexpectedly renders SVG for {source}"
        );
    }
}

#[test]
fn clean_removes_examples_and_clean_all_also_cleans_cargo() {
    let Some(clean) = dry_run(&["--no-print-directory", "-n", "clean"]) else {
        eprintln!("make not installed; skipping Makefile clean test");
        return;
    };
    assert!(
        clean.contains("hello.svg"),
        "clean does not remove SVG output"
    );
    assert!(
        clean.contains("hello.html"),
        "clean does not remove HTML output"
    );
    assert!(
        !clean.contains("cargo clean"),
        "plain clean must not remove Cargo artifacts"
    );

    let Some(clean_all) = dry_run(&["--no-print-directory", "-n", "clean-all"]) else {
        eprintln!("make not installed; skipping Makefile clean-all test");
        return;
    };
    assert!(
        clean_all.contains("hello.svg"),
        "clean-all does not remove example output"
    );
    assert!(
        clean_all.contains("cargo clean"),
        "clean-all does not remove Cargo artifacts"
    );
}

#[test]
fn serve_runs_the_release_binary_in_server_mode() {
    let Some(stdout) = dry_run(&["--no-print-directory", "-n", "serve"]) else {
        eprintln!("make not installed; skipping Makefile serve test");
        return;
    };
    assert!(
        stdout.contains("target/release/swimflow") && stdout.contains("--serve"),
        "serve target does not run the release binary in server mode:\n{stdout}"
    );
    assert!(
        stdout.contains("cargo build --release"),
        "serve target does not depend on the release build:\n{stdout}"
    );

    let Some(explicit) = dry_run(&[
        "--no-print-directory",
        "-n",
        "serve",
        "ENV_FILE=config/serve.env",
    ]) else {
        return;
    };
    assert!(
        explicit.contains("--env-file \"config/serve.env\""),
        "ENV_FILE is not forwarded to --env-file:\n{explicit}"
    );
}

#[test]
fn install_uses_home_bin() {
    let Some(output) = make(&["--no-print-directory", "-n", "install"]) else {
        eprintln!("make not installed; skipping Makefile install test");
        return;
    };
    assert!(output.status.success(), "make -n install failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let home = std::env::var("HOME").expect("HOME is set");
    assert!(
        stdout.contains(&format!("{home}/bin/swimflow")),
        "install target does not copy to $HOME/bin:\n{stdout}"
    );
    assert!(
        stdout.contains("cargo build --release"),
        "install target does not build the release binary:\n{stdout}"
    );
}
