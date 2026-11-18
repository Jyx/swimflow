//! End-to-end CLI tests: output path/format inference, help rendering and
//! error reporting.

use std::path::{Path, PathBuf};
use std::process::Command;

use clap::CommandFactory;
use swimflow::cli::Cli;

mod common;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_swimflow")
}

fn temp_dir(name: &str) -> common::TempDir {
    common::TempDir::new(&format!("swimflow-cli-{name}-"))
}

fn write_input(dir: &Path) -> PathBuf {
    let input = dir.join("simple.diag");
    std::fs::write(&input, "{ A -> B; }\n").unwrap();
    input
}

#[test]
fn defaults_to_input_svg() {
    let dir = temp_dir("default");
    let input = write_input(&dir);

    let status = Command::new(bin()).arg(&input).status().unwrap();
    assert!(status.success());

    let output = dir.join("simple.svg");
    let svg = std::fs::read_to_string(&output).expect("simple.svg should be written");
    assert!(svg.contains("<svg"));
    assert!(svg.contains("<text"));
}

#[test]
fn font_option_selects_the_font_and_reports_unknown_families() {
    let dir = temp_dir("font");
    let input = write_input(&dir);
    let font = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/assets/default.ttf");

    let output = Command::new(bin())
        .arg(&input)
        .arg("--font")
        .arg(&font)
        .args(["--embed-font", "-o", "-"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = String::from_utf8(output.stdout).unwrap();
    assert!(
        svg.contains("font-family=\"VL Gothic, sans-serif\""),
        "{svg}"
    );
    assert!(svg.contains("@font-face{font-family:'VL Gothic'"), "{svg}");

    // An unknown family is a clear error, not a silent fallback.
    let bad = Command::new(bin())
        .arg(&input)
        .args([
            "--font-family",
            "swimflow-definitely-not-a-font-xyz",
            "-o",
            "-",
        ])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(
        String::from_utf8_lossy(&bad.stderr).contains("no installed font matches"),
        "{}",
        String::from_utf8_lossy(&bad.stderr)
    );
}

#[test]
fn writes_to_stdout_with_dash() {
    let dir = temp_dir("stdout");
    let input = write_input(&dir);

    let output = Command::new(bin())
        .arg(&input)
        .args(["-o", "-"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let svg = String::from_utf8(output.stdout).unwrap();
    assert!(svg.contains("<svg"));
}

#[test]
fn infers_format_from_output_extension() {
    let dir = temp_dir("extension");
    let input = write_input(&dir);
    let output = dir.join("diagram.svg");

    let status = Command::new(bin())
        .arg(&input)
        .args(["-o", output.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(output.exists());
}

#[test]
fn unknown_format_errors_cleanly() {
    let dir = temp_dir("format");
    let input = write_input(&dir);

    let output = Command::new(bin())
        .arg(&input)
        .args(["-f", "bogus", "-o", "-"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unknown output format"), "stderr: {stderr}");
}

#[test]
fn bundle_writes_a_standalone_html_file() {
    let dir = temp_dir("bundle");
    let input = dir.join("drill.diag");
    std::fs::write(
        &input,
        "diagram main { A [ref = \"#detail\"]; } diagram detail { B -> C; }\n",
    )
    .unwrap();
    let output = dir.join("out.html");

    let status = Command::new(bin())
        .arg(&input)
        .args(["--bundle", output.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());

    let html = std::fs::read_to_string(&output).expect("bundle should be written");
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("id=\"main\""));
    assert!(html.contains("id=\"detail\""));
    assert!(html.contains("data-target=\"detail\""));
    assert!(html.contains("data-root=\"main\""));
}

#[test]
fn format_html_defaults_to_html_extension() {
    let dir = temp_dir("html-extension");
    let input = write_input(&dir);

    let status = Command::new(bin())
        .arg(&input)
        .args(["-f", "html"])
        .status()
        .unwrap();
    assert!(status.success());
    let output = dir.join("simple.html");
    assert!(output.exists(), "expected {}", output.display());
}

#[test]
fn duplicate_sequence_numbers_warn_on_stderr() {
    let dir = temp_dir("warning");
    let input = dir.join("warn.diag");
    std::fs::write(
        &input,
        "{ numbering = auto; A [numbered = 1]; B [numbered = 1]; }\n",
    )
    .unwrap();

    let output = Command::new(bin())
        .arg(&input)
        .args(["-o", "-"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("duplicate sequence number"),
        "stderr: {stderr}"
    );
}

#[test]
fn antialias_doubles_png_dimensions() {
    let dir = temp_dir("antialias");
    let input = write_input(&dir);
    let output = dir.join("aa.png");

    let status = Command::new(bin())
        .arg(&input)
        .args(["-f", "png", "--antialias", "-o", output.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());

    let png = std::fs::read(&output).unwrap();
    let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
    assert_eq!((width, height), (512, 560));
}

#[test]
fn short_help_is_compact_and_hides_debug_flags() {
    let mut cmd = Cli::command();
    let help = cmd.render_help().to_string();

    assert!(help.contains("--output"), "{help}");
    assert!(help.contains("--bundle"), "{help}");
    for hidden in ["--dump-ast", "--dump-model", "--dump-geometry", "--pixels"] {
        assert!(
            !help.contains(hidden),
            "short help should hide {hidden}:\n{help}"
        );
    }
}

#[test]
fn help_groups_cli_only_options() {
    let help = Cli::command().render_help().to_string();
    let section = |heading: &str| {
        let marker = format!("{heading}:");
        let start = help.find(&marker).expect(heading) + marker.len();
        let rest = &help[start..];
        let end = rest.find("\n\n").unwrap_or(rest.len());
        &rest[..end]
    };

    let rendering = section("Rendering");
    for flag in ["--font", "--font-family", "--scale", "--antialias"] {
        assert!(rendering.contains(flag), "{flag} should be shared:\n{help}");
    }

    let cli_only = section("CLI only");
    for flag in [
        "--root",
        "--no-external-files",
        "--allow-broken-links",
        "--embed-font",
        "--text-as-paths",
        "--no-external",
        "--nodoctype",
        "--noviewbox",
        "--transparent",
    ] {
        assert!(
            cli_only.contains(flag),
            "{flag} should be in the CLI only group:\n{help}"
        );
    }
}

#[test]
fn long_help_documents_debug_flags_and_examples() {
    let mut cmd = Cli::command();
    let help = cmd.render_long_help().to_string();

    for flag in ["--dump-ast", "--dump-model", "--dump-geometry", "--pixels"] {
        assert!(
            help.contains(flag),
            "long help should document {flag}:\n{help}"
        );
    }
    assert!(help.contains("EXAMPLES:"), "{help}");
    assert!(help.contains("--scale"), "{help}");
}

#[test]
fn long_help_is_richer_than_short_help() {
    let short = Cli::command().render_help().to_string();
    let long = Cli::command().render_long_help().to_string();
    assert!(
        long.len() > short.len(),
        "long help ({}) should be larger than short help ({})",
        long.len(),
        short.len()
    );
}

#[test]
fn help_wraps_to_the_terminal_width() {
    // `term_width` mirrors what clap's `wrap_help` feature derives from the
    // real terminal, so a resize simply re-renders at the new width.
    for width in [60usize, 80, 100] {
        let mut cmd = Cli::command().term_width(width);
        let help = cmd.render_help().to_string();
        for line in help.lines() {
            let len = line.chars().count();
            assert!(
                len <= width,
                "help line is {len} columns wide at term_width({width}): {line:?}"
            );
        }
    }
}

#[test]
fn list_types_prints_the_known_diagram_types() {
    let output = Command::new(bin())
        .arg("--list-types")
        .output()
        .expect("run swimflow --list-types");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("activity"), "{stdout}");
    assert!(stdout.contains("sequence"), "{stdout}");
    assert!(stdout.contains(".diag"), "{stdout}");
    assert!(stdout.contains(".seq"), "{stdout}");
    assert!(stdout.contains("tree"), "{stdout}");
    assert!(stdout.contains(".tree"), "{stdout}");
    assert!(stdout.contains("treemap"), "{stdout}");
    assert!(stdout.contains(".tmap"), "{stdout}");
}

#[test]
fn a_sequence_file_is_rendered_as_a_sequence_diagram() {
    let dir = temp_dir("sequence");
    let input = dir.join("chat.seq");
    std::fs::write(
        &input,
        "sequence \"Chat\" {\n  participant \"User\" as user;\n  participant \"Server\";\n  \
         user -> Server: \"hello\";\n}\n",
    )
    .unwrap();

    let output = dir.join("chat.svg");
    let status = Command::new(bin())
        .arg(&input)
        .arg("-o")
        .arg(output.to_str().unwrap())
        .status()
        .unwrap();
    assert!(status.success());

    let svg = std::fs::read_to_string(&output).expect("chat.svg should be written");
    assert!(svg.contains("<svg"));
    assert!(svg.contains("User"));
    assert!(svg.contains("Server"));
}

#[test]
fn a_tree_file_is_rendered_as_a_tree_diagram() {
    let dir = temp_dir("tree");
    let input = dir.join("org.tree");
    std::fs::write(
        &input,
        "tree \"Org\" {\n  root \"Boss\";\n  Team { Dev; Ops; }\n}\n",
    )
    .unwrap();

    let output = dir.join("org.svg");
    let status = Command::new(bin())
        .arg(&input)
        .arg("-o")
        .arg(output.to_str().unwrap())
        .status()
        .unwrap();
    assert!(status.success());

    let svg = std::fs::read_to_string(&output).expect("org.svg should be written");
    assert!(svg.contains("Boss") && svg.contains("Team") && svg.contains("Ops"));

    // The same source on standard input is detected by its content.
    let piped = Command::new(bin())
        .args(["-o", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map(|mut child| {
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(b"tree T {\n  Leaf;\n}\n")
                .unwrap();
            child.wait_with_output().unwrap()
        })
        .unwrap();
    assert!(piped.status.success());
    assert!(String::from_utf8_lossy(&piped.stdout).contains("Leaf"));
}

#[test]
fn a_tmap_file_is_rendered_as_a_treemap() {
    let dir = temp_dir("treemap");
    let input = dir.join("budget.tmap");
    std::fs::write(
        &input,
        "treemap \"Budget\" {\n  root \"Monthly\";\n  Housing: 3 { Rent: 2; Utilities: 1; }\n  Food: 1;\n}\n",
    )
    .unwrap();

    let output = dir.join("budget.svg");
    let status = Command::new(bin())
        .arg(&input)
        .arg("-o")
        .arg(output.to_str().unwrap())
        .status()
        .unwrap();
    assert!(status.success());

    let svg = std::fs::read_to_string(&output).expect("budget.svg should be written");
    assert!(svg.contains("swimflow-treemap"), "{svg}");
    assert!(svg.contains("Housing") && svg.contains("Rent"), "{svg}");

    // The same source on standard input is detected by its content.
    let piped = Command::new(bin())
        .args(["-o", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map(|mut child| {
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(b"treemap T {\n  Leaf: 1;\n}\n")
                .unwrap();
            child.wait_with_output().unwrap()
        })
        .unwrap();
    assert!(piped.status.success());
    assert!(String::from_utf8_lossy(&piped.stdout).contains("swimflow-treemap"));
}

#[test]
fn a_broken_treemap_file_reports_its_position() {
    let dir = temp_dir("treemap-broken");
    let input = dir.join("bad.tmap");
    std::fs::write(&input, "treemap {\n  A: nope;\n}\n").unwrap();
    let output = Command::new(bin()).arg(&input).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("line 2"), "{stderr}");
}

#[test]
fn a_broken_tree_file_reports_its_position() {
    let dir = temp_dir("tree-broken");
    let input = dir.join("bad.tree");
    std::fs::write(&input, "tree {\n  A\n}\n").unwrap();
    let output = Command::new(bin()).arg(&input).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("line 3"), "{stderr}");
}

#[test]
fn the_type_flag_selects_the_diagram_type() {
    let dir = temp_dir("type-flag");
    // A file with an unknown extension is detected by its content; the flag
    // overrides the detection.
    let input = dir.join("diagram.txt");
    std::fs::write(
        &input,
        "sequence \"Chat\" {\n  participant \"User\" as user;\n  user -> user: \"hi\";\n}\n",
    )
    .unwrap();

    let output = dir.join("forced.svg");
    let status = Command::new(bin())
        .args(["--type", "sequence"])
        .arg(&input)
        .arg("-o")
        .arg(output.to_str().unwrap())
        .status()
        .unwrap();
    assert!(status.success());
    assert!(output.exists());

    let status = Command::new(bin())
        .args(["--type", "nope"])
        .arg(&input)
        .arg("-o")
        .arg(dir.join("bad.svg").to_str().unwrap())
        .output()
        .unwrap();
    assert!(!status.status.success());
    let stderr = String::from_utf8_lossy(&status.stderr);
    assert!(stderr.contains("unknown diagram type"), "{stderr}");
    assert!(stderr.contains("activity"), "{stderr}");
}

#[test]
fn root_selects_the_entry_diagram_for_every_bundle_type() {
    let dir = temp_dir("bundle-root");
    let cases = [
        (
            "multi.seq",
            "sequence \"main\" { participant A; }\n\
             sequence \"detail\" { participant B; }\n",
        ),
        (
            "multi.tree",
            "tree \"main\" { root \"Main\"; A; }\n\
             tree \"detail\" { root \"Detail\"; B; }\n",
        ),
        (
            "multi.tmap",
            "treemap \"main\" { A: 1; }\ntreemap \"detail\" { B: 2; }\n",
        ),
        (
            "multi.mfst",
            "manifest \"main\" { column a { row; } }\n\
             manifest \"detail\" { column b { other; } }\n",
        ),
    ];

    for (name, source) in cases {
        let input = dir.join(name);
        std::fs::write(&input, source).unwrap();
        let output = dir.join(format!("{name}.html"));

        let result = Command::new(bin())
            .arg(&input)
            .args(["--bundle", output.to_str().unwrap(), "--root", "detail"])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );

        let html = std::fs::read_to_string(&output).unwrap();
        assert!(
            html.contains("data-root=\"detail-n1\""),
            "{name}: --root should select the second diagram:\n{html}"
        );
        assert!(
            !html.contains("data-root=\"main\""),
            "{name}: the first diagram must not stay the root"
        );
    }

    // An unknown diagram id is an error, as it is for the activity type.
    let input = dir.join("multi.tree");
    let result = Command::new(bin())
        .arg(&input)
        .args([
            "--bundle",
            dir.join("bad.html").to_str().unwrap(),
            "--root",
            "nope",
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("unknown root diagram"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn allow_broken_links_warns_from_a_file_and_from_stdin() {
    let dir = temp_dir("broken-links");
    let input = dir.join("broken.diag");
    let source = "diagram main { A [ref = \"#missing\"]; }\n";
    std::fs::write(&input, source).unwrap();

    let file = Command::new(bin())
        .arg(&input)
        .args(["--allow-broken-links", "-o", "-"])
        .output()
        .unwrap();
    assert!(file.status.success());
    let stderr = String::from_utf8_lossy(&file.stderr);
    assert!(stderr.contains("warning"), "stderr: {stderr}");
    assert!(stderr.contains("unknown diagram id"), "stderr: {stderr}");

    // Standard input has no path but must honour the same option.
    let piped = Command::new(bin())
        .args(["--allow-broken-links", "-o", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map(|mut child| {
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(source.as_bytes())
                .unwrap();
            child.wait_with_output().unwrap()
        })
        .unwrap();
    assert!(
        piped.status.success(),
        "{}",
        String::from_utf8_lossy(&piped.stderr)
    );
    assert!(
        String::from_utf8_lossy(&piped.stderr).contains("unknown diagram id"),
        "{}",
        String::from_utf8_lossy(&piped.stderr)
    );
}
