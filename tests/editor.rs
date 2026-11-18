//! Neovim editor support.
//!
//! The Vimscript files are checked structurally even when Neovim is absent.
//! If `nvim` is installed, every example is opened headlessly to verify
//! filetype detection, ftplugin settings and a representative set of syntax
//! groups.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn editor_dir() -> PathBuf {
    root().join("editors/nvim")
}

fn nvim() -> Option<&'static str> {
    let output = Command::new("nvim").arg("--version").output().ok()?;
    output.status.success().then_some("nvim")
}

fn collect_examples(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_examples(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("diag") {
            out.push(path);
        }
    }
}

fn nvim_args() -> Vec<String> {
    vec![
        "--headless".to_string(),
        "--clean".to_string(),
        "--cmd".to_string(),
        format!("set runtimepath^={}", editor_dir().display()),
        "--cmd".to_string(),
        "filetype on".to_string(),
        "--cmd".to_string(),
        "set shortmess+=F".to_string(),
    ]
}

fn run_nvim(args: Vec<String>) -> std::process::Output {
    Command::new(nvim().expect("checked by caller"))
        .args(args)
        .current_dir(root())
        .output()
        .expect("run nvim")
}

fn output_text(output: &std::process::Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn editor_files_are_present_and_wired_up() {
    let editor = editor_dir();
    for file in [
        "ftdetect/actdiag.vim",
        "syntax/actdiag.vim",
        "ftplugin/actdiag.vim",
        "lua/swimflow/init.lua",
        "README.rst",
    ] {
        assert!(editor.join(file).is_file(), "missing editors/nvim/{file}");
    }

    let ftdetect = std::fs::read_to_string(editor.join("ftdetect/actdiag.vim")).unwrap();
    assert!(ftdetect.contains("setfiletype actdiag"));
    assert!(ftdetect.contains("*.diag"));
    assert!(ftdetect.contains("*.actdiag"));

    let syntax = std::fs::read_to_string(editor.join("syntax/actdiag.vim")).unwrap();
    for group in [
        "actdiagKeyword",
        "actdiagAttribute",
        "actdiagConstant",
        "actdiagComment",
        "actdiagString",
        "actdiagTripleString",
        "actdiagNumber",
        "actdiagOperator",
    ] {
        assert!(
            syntax.contains(group),
            "syntax file does not define {group}"
        );
    }
    assert!(syntax.contains("b:current_syntax"));

    let ftplugin = std::fs::read_to_string(editor.join("ftplugin/actdiag.vim")).unwrap();
    assert!(ftplugin.contains("commentstring=//%s"));
    assert!(ftplugin.contains("b:undo_ftplugin"));
}

#[test]
fn every_example_gets_the_actdiag_filetype() {
    let Some(nvim) = nvim() else {
        eprintln!("nvim not installed; skipping headless filetype checks");
        return;
    };

    let mut examples = Vec::new();
    collect_examples(&root().join("examples"), &mut examples);
    assert!(!examples.is_empty());

    for path in examples {
        let relative = path.strip_prefix(root()).unwrap().to_string_lossy();
        let lua = r#"lua print("FT="..vim.bo.filetype.." CS="..vim.bo.commentstring)"#;
        let mut args = nvim_args();
        args.push(format!("+edit {relative}"));
        args.push(format!("+{lua}"));
        args.push("+qa!".to_string());

        let output = run_nvim(args);
        let text = output_text(&output);
        assert!(
            output.status.success(),
            "{nvim} failed for {relative}\n{text}"
        );
        assert!(
            text.contains("FT=actdiag CS=//%s"),
            "wrong filetype/commentstring for {relative}\n{text}"
        );
    }
}

#[test]
fn representative_syntax_groups_are_applied() {
    let Some(nvim) = nvim() else {
        eprintln!("nvim not installed; skipping syntax group checks");
        return;
    };

    // (line, column, expected group) in tests/fixtures/syntax_highlight.diag.
    let cases = [
        (1, 1, "actdiagComment"),
        (2, 1, "actdiagComment"),
        (3, 1, "actdiagComment"),
        (5, 1, "actdiagKeyword"),
        (6, 3, "actdiagKeyword"),
        (7, 14, "actdiagAttribute"),
        (7, 22, "actdiagString"),
        (8, 11, "actdiagAttribute"),
        (8, 19, "actdiagString"),
        (12, 5, "actdiagOperator"),
        (12, 24, "actdiagTripleString"),
        (13, 1, "actdiagTripleString"),
        (17, 17, "actdiagNumber"),
        (19, 1, "actdiagKeyword"),
    ];
    let table = cases
        .iter()
        .map(|(line, column, group)| format!("{{{line},{column},\"{group}\"}}"))
        .collect::<Vec<_>>()
        .join(",");
    let lua = format!(
        r#"lua local cases={{{table}}}; local ok=true; for _,x in ipairs(cases) do local n=vim.fn.synIDattr(vim.fn.synID(x[1],x[2],1),"name"); if n~=x[3] then print("MISMATCH "..x[1]..":"..x[2].." got="..n.." want="..x[3]); ok=false end end; print(ok and "SYNTAX_OK" or "SYNTAX_FAIL")"#
    );

    let mut args = nvim_args();
    args.push("+edit tests/fixtures/syntax_highlight.diag".to_string());
    args.push(format!("+{lua}"));
    args.push("+if v:errmsg != '' | cquit 1 | endif".to_string());
    args.push("+qa!".to_string());

    let output = run_nvim(args);
    let text = output_text(&output);
    assert!(output.status.success(), "{nvim} reported an error\n{text}");
    assert!(
        text.contains("SYNTAX_OK"),
        "syntax group check failed\n{text}"
    );
    assert!(
        !text.contains("MISMATCH"),
        "syntax group check failed\n{text}"
    );
}
