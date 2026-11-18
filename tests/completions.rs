//! Bash completion drift test.
//!
//! `completions/swimflow.bash` is committed so a fresh checkout can install it
//! without running a generator. This test re-renders it from the live clap
//! command tree and fails if the two diverge. Regenerate with:
//!
//! ```console
//! $ UPDATE_COMPLETIONS=1 cargo test --test completions
//! ```

use std::path::PathBuf;
use std::process::Command;

use clap::CommandFactory;
use clap_complete::{Shell, generate};
use swimflow::cli::Cli;

fn completion_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("completions/swimflow.bash")
}

fn render() -> Vec<u8> {
    let mut command = Cli::command();
    let mut buf = Vec::new();
    generate(Shell::Bash, &mut command, "swimflow", &mut buf);
    buf
}

#[test]
fn completions_file_matches_the_cli() {
    let path = completion_path();
    let expected = String::from_utf8(render()).expect("bash completion is UTF-8");

    if std::env::var_os("UPDATE_COMPLETIONS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &expected).unwrap();
        return;
    }

    let actual = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()));
    assert_eq!(
        actual,
        expected,
        "{} is stale; run `UPDATE_COMPLETIONS=1 cargo test --test completions`",
        path.display()
    );
}

#[test]
fn completion_covers_every_long_option() {
    let text = std::fs::read_to_string(completion_path()).expect("completions/swimflow.bash");
    let command = Cli::command();
    for arg in command.get_arguments() {
        let Some(long) = arg.get_long() else {
            continue;
        };
        assert!(
            text.contains(long),
            "bash completion is missing --{long}:\n{text}"
        );
    }
}

#[test]
fn completion_file_is_valid_bash() {
    let path = completion_path();
    // `bash -n` parses without executing; skip silently where bash is absent.
    let status = Command::new("bash").arg("-n").arg(&path).status();
    match status {
        Ok(status) => assert!(status.success(), "bash -n rejected {}", path.display()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => panic!("failed to run bash: {err}"),
    }
}
