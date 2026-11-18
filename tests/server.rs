//! End-to-end tests for `swimflow --serve`.
//!
//! These bind an ephemeral port on localhost and speak raw HTTP/1.1 so no
//! client dependency is needed. The serving threads are detached: the test
//! process exits once the tests finish, which tears them down.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use swimflow::server::{ServeConfig, bind, serve};

mod common;

fn temp_dir(name: &str) -> common::TempDir {
    let dir = common::TempDir::new(&format!("swimflow-server-it-{name}-"));
    std::fs::write(dir.join("hello.diag"), "{ A -> B; }\n").unwrap();
    std::fs::write(
        dir.join("chat.seq"),
        "sequence \"Chat\" {\n  participant \"User\" as user;\n           participant \"Server\";\n  user -> Server: \"hello\";\n}\n",
    )
    .unwrap();
    dir
}

fn spawn(dir: &Path, token: Option<&str>) -> SocketAddr {
    let config = ServeConfig {
        dir: std::fs::canonicalize(dir).unwrap(),
        host: "127.0.0.1".to_string(),
        port: 0,
        debounce_ms: 50,
        scale: 1.0,
        token: token.map(str::to_string),
        font: swimflow::text::FontSpec::default(),
    };
    let server = bind(&config).expect("bind ephemeral port");
    let addr = server.server_addr().to_ip().expect("ip address");
    std::thread::spawn(move || {
        let _ = serve(server, config);
    });
    addr
}

fn http(addr: SocketAddr, method: &str, target: &str, body: &str) -> String {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let request = format!(
        "{method} {target} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\
         Content-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

/// The same, but with a content type (the session payload is form encoded).
fn http_form(addr: SocketAddr, method: &str, target: &str, body: &str) -> String {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let request = format!(
        "{method} {target} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\
         Content-Type: application/x-www-form-urlencoded; charset=utf-8\r\n\
         Content-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

/// The same, but the body is read as bytes (for binary formats).
fn http_form_bytes(addr: SocketAddr, method: &str, target: &str, body: &str) -> Vec<u8> {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let request = format!(
        "{method} {target} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\
         Content-Type: application/x-www-form-urlencoded\r\n\
         Content-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    response
}

/// Percent-encode a value the way the browser does.
fn encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn form(fields: &[(&str, &str)]) -> String {
    fields
        .iter()
        .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

#[test]
fn serves_the_shell_source_and_rendered_bundle() {
    let dir = temp_dir("http");
    let addr = spawn(&dir, None);

    let shell = http(addr, "GET", "/", "");
    assert!(shell.starts_with("HTTP/1.1 200"), "{shell}");
    assert!(shell.contains("data-path=\"hello.diag\""), "{shell}");

    let source = http(addr, "GET", "/api/source?path=hello.diag", "");
    assert!(source.starts_with("HTTP/1.1 200"), "{source}");
    assert!(source.ends_with("{ A -> B; }\n"), "{source}");

    let rendered = http(addr, "POST", "/api/render?path=hello.diag", "{ A -> B; }\n");
    assert!(rendered.starts_with("HTTP/1.1 200"), "{rendered}");
    assert!(rendered.contains("<svg"), "{rendered}");
    assert!(rendered.contains("/assets/font.ttf"), "{rendered}");
}

#[test]
fn shell_has_a_syntax_highlight_overlay() {
    let dir = temp_dir("highlight");
    let addr = spawn(&dir, None);
    let shell = http(addr, "GET", "/", "");
    assert!(shell.starts_with("HTTP/1.1 200"), "{shell}");
    assert!(shell.contains("id=\"highlight\""), "{shell}");
    assert!(shell.contains("class=\"editor\""), "{shell}");
    assert!(shell.contains("swimflowHighlight"), "{shell}");
    assert!(shell.contains("updateHighlight"), "{shell}");
    assert!(shell.contains("tok-keyword"), "{shell}");
}

#[test]
fn syntax_highlighter_tokenizes_the_dsl() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("node not installed; skipping highlighter runtime test");
        return;
    }

    let dir = temp_dir("highlight-node");
    let addr = spawn(&dir, None);
    let shell = http(addr, "GET", "/", "");
    let path = dir.join("shell.html");
    std::fs::write(&path, &shell).expect("write shell");

    let harness = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/highlight_runtime.js");
    let output = Command::new("node")
        .arg(&harness)
        .arg(&path)
        .output()
        .expect("run node");
    let _ = std::fs::remove_file(&path);

    assert!(
        output.status.success(),
        "highlighter harness failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn serves_the_embedded_font_as_an_asset() {
    let dir = temp_dir("font");
    let addr = spawn(&dir, None);

    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .write_all(b"GET /assets/font.ttf HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let header = String::from_utf8_lossy(&response);
    assert!(header.starts_with("HTTP/1.1 200"), "{header}");
    assert!(header.contains("font/ttf"), "{header}");
    // The body is the embedded TrueType font (several megabytes).
    assert!(response.len() > 1_000_000, "font body too small");
}

#[test]
fn token_gate_is_enforced_over_http() {
    let dir = temp_dir("http-token");
    let addr = spawn(&dir, Some("letmein"));

    let denied = http(addr, "GET", "/", "");
    assert!(denied.starts_with("HTTP/1.1 401"), "{denied}");

    let allowed = http(addr, "GET", "/?token=letmein", "");
    assert!(allowed.starts_with("HTTP/1.1 200"), "{allowed}");
    assert!(
        allowed.contains("Set-Cookie: swimflow_token=letmein"),
        "{allowed}"
    );
}

#[test]
fn renders_session_buffers_without_writing_them_to_disk() {
    let dir = temp_dir("http-session");
    let addr = spawn(&dir, None);

    let body = form(&[
        ("source", "diagram main { A -> B; }"),
        ("file:imported.diag", "diagram main { A -> B; }"),
    ]);
    let rendered = http_form(addr, "POST", "/api/render?path=imported.diag", &body);
    assert!(rendered.starts_with("HTTP/1.1 200"), "{rendered}");
    assert!(rendered.contains("<svg"), "{rendered}");
    assert!(rendered.contains("id=\"main\""), "{rendered}");

    // The buffer lives in the browser only.
    assert!(!dir.join("imported.diag").exists());
}

#[test]
fn session_buffers_resolve_cross_references_over_http() {
    let dir = temp_dir("http-session-refs");
    let addr = spawn(&dir, None);

    let body = form(&[
        (
            "source",
            "diagram main { A [ref = \"other.diag#detail\"]; }",
        ),
        ("file:other.diag", "diagram detail { B; }"),
    ]);
    let rendered = http_form(addr, "POST", "/api/render?path=main.diag", &body);
    assert!(rendered.starts_with("HTTP/1.1 200"), "{rendered}");
    assert!(rendered.contains("data-target=\"detail\""), "{rendered}");
    assert!(rendered.contains("id=\"detail\""), "{rendered}");
}

#[test]
fn session_buffers_can_be_exported_over_http() {
    let dir = temp_dir("http-session-export");
    let addr = spawn(&dir, None);

    let body = form(&[("source", "diagram main { A -> B; }")]);
    let exported = http_form(
        addr,
        "POST",
        "/api/export?path=sketch.diag&format=svg",
        &body,
    );
    assert!(exported.starts_with("HTTP/1.1 200"), "{exported}");
    assert!(exported.contains("image/svg+xml"), "{exported}");
    assert!(exported.contains("filename=\"sketch.svg\""), "{exported}");
}

#[test]
fn session_runtime_behaves_like_the_browser_side_store() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("node not installed; skipping session runtime test");
        return;
    }

    let dir = temp_dir("http-session-node");
    let addr = spawn(&dir, None);
    let shell = http(addr, "GET", "/", "");
    let path = dir.join("shell.html");
    std::fs::write(&path, &shell).expect("write shell");

    let harness = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/session_runtime.js");
    let output = Command::new("node")
        .arg(&harness)
        .arg(&path)
        .output()
        .expect("run node");
    let _ = std::fs::remove_file(&path);

    assert!(
        output.status.success(),
        "session harness failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn session_ui_runtime_drives_the_browser_workflow() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("node not installed; skipping session UI test");
        return;
    }

    let dir = temp_dir("http-session-ui");
    let addr = spawn(&dir, None);
    let shell = http(addr, "GET", "/", "");
    let path = dir.join("shell.html");
    std::fs::write(&path, &shell).expect("write shell");

    let harness = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/session_ui_runtime.js");
    let output = Command::new("node")
        .arg(&harness)
        .arg(&path)
        .output()
        .expect("run node");
    let _ = std::fs::remove_file(&path);

    assert!(
        output.status.success(),
        "session UI harness failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn the_shell_offers_a_tab_per_diagram_type() {
    let dir = temp_dir("tabs");
    let addr = spawn(&dir, None);
    let shell = http(addr, "GET", "/", "");
    assert!(shell.starts_with("HTTP/1.1 200"), "{shell}");

    // One tab per registered type, the first one selected.
    assert!(shell.contains("data-type=\"activity\""), "{shell}");
    assert!(shell.contains("data-type=\"sequence\""), "{shell}");
    assert!(shell.contains("aria-selected=\"true\""), "{shell}");

    // One file tree per type, each with its own files.
    assert!(shell.contains("id=\"tree-activity\""), "{shell}");
    assert!(shell.contains("id=\"tree-sequence\""), "{shell}");

    // The type catalog is handed to the browser.
    assert!(shell.contains("\"name\":\"activity\""), "{shell}");
    assert!(shell.contains("\"source\":\"seq\""), "{shell}");
}

#[test]
fn the_shell_offers_a_tree_tab() {
    let dir = temp_dir("tree-tab");
    std::fs::write(dir.join("org.tree"), "tree { A { B; } }\n").unwrap();
    let addr = spawn(&dir, None);
    let shell = http(addr, "GET", "/", "");
    assert!(shell.contains("data-type=\"tree\""), "{shell}");
    assert!(shell.contains("id=\"tree-tree\""), "{shell}");
    assert!(shell.contains("\"source\":\"tree\""), "{shell}");
    assert!(shell.contains("org.tree"), "{shell}");
}

#[test]
fn the_shell_offers_a_treemap_tab() {
    let dir = temp_dir("treemap-tab");
    std::fs::write(dir.join("budget.tmap"), "treemap { A: 1; }\n").unwrap();
    let addr = spawn(&dir, None);
    let shell = http(addr, "GET", "/", "");
    assert!(shell.contains("data-type=\"treemap\""), "{shell}");
    assert!(shell.contains("id=\"tree-treemap\""), "{shell}");
    assert!(shell.contains("\"source\":\"tmap\""), "{shell}");
    assert!(shell.contains("budget.tmap"), "{shell}");
}

#[test]
fn a_treemap_file_is_rendered_and_exported() {
    let dir = temp_dir("treemap-render");
    std::fs::write(dir.join("budget.tmap"), "treemap { A: 1; }\n").unwrap();
    let addr = spawn(&dir, None);
    let source = "treemap \"B\" { root \"Budget\"; A: 2; }\n";

    let rendered = http(
        addr,
        "POST",
        "/api/render?path=budget.tmap&type=treemap",
        source,
    );
    assert!(rendered.starts_with("HTTP/1.1 200"), "{rendered}");
    assert!(rendered.contains("<svg"), "{rendered}");
    assert!(rendered.contains("Budget"), "{rendered}");
    assert!(rendered.contains("treemap-values-toggle"), "{rendered}");

    let svg = http_form(
        addr,
        "POST",
        "/api/export?path=budget.tmap&format=svg",
        &form(&[("source", source)]),
    );
    assert!(svg.starts_with("HTTP/1.1 200"), "{svg}");
    assert!(svg.contains("filename=\"budget.svg\""), "{svg}");
    assert!(svg.contains("swimflow-treemap"), "{svg}");
    // A downloaded SVG has no interactive toolbar.
    assert!(!svg.contains("treemap-values-toggle"), "{svg}");
}

#[test]
fn a_tree_file_is_rendered_and_exported() {
    let dir = temp_dir("tree-render");
    std::fs::write(dir.join("org.tree"), "tree { A { B; } }\n").unwrap();
    let addr = spawn(&dir, None);
    let source = "tree \"Org\" { root \"Boss\"; Team { Dev; } }\n";

    let rendered = http(addr, "POST", "/api/render?path=org.tree&type=tree", source);
    assert!(rendered.starts_with("HTTP/1.1 200"), "{rendered}");
    assert!(rendered.contains("<svg"), "{rendered}");
    assert!(rendered.contains("Boss"), "{rendered}");

    let svg = http_form(
        addr,
        "POST",
        "/api/export?path=org.tree&format=svg",
        &form(&[("source", source)]),
    );
    assert!(svg.starts_with("HTTP/1.1 200"), "{svg}");
    assert!(svg.contains("filename=\"org.svg\""), "{svg}");

    let broken = http(
        addr,
        "POST",
        "/api/render?path=org.tree&type=tree",
        "tree { A ",
    );
    assert!(!broken.starts_with("HTTP/1.1 200"), "{broken}");
}

/// The bytes after the headers of a raw HTTP response.
fn payload_of(response: &[u8]) -> &[u8] {
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("end of the headers");
    &response[split + 4..]
}

/// The (width, height) of a PNG.
fn png_size(png: &[u8]) -> (u32, u32) {
    let word = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap());
    (word(16), word(20))
}

#[test]
fn a_tree_export_looks_like_the_preview() {
    let dir = temp_dir("tree-view");
    std::fs::write(dir.join("org.tree"), "tree { A { A1; A2; } B; }\n").unwrap();
    let addr = spawn(&dir, None);
    // n0 is the root, n1 is A, n2 is B, n3 is A1, n4 is A2.
    let source = "tree { A { A1; A2; } B; }\n";
    let export = |format: &str, extra: &[(&str, &str)], source: &str| {
        let mut fields = vec![("source", source)];
        fields.extend_from_slice(extra);
        http_form_bytes(
            addr,
            "POST",
            &format!("/api/export?path=org.tree&format={format}"),
            &form(&fields),
        )
    };

    // Without a view the export is the plain render: nothing is folded.
    let plain = export("svg", &[], source);
    let plain = String::from_utf8_lossy(payload_of(&plain)).into_owned();
    assert!(plain.contains("data-collapsed=\"0\""), "{plain}");
    assert!(
        !plain.contains("display=\"none\" "),
        "no node is hidden: {plain}"
    );

    // The page says that A is folded: the children are hidden in the SVG ...
    let folded = export("svg", &[("view:collapsed:default", "1")], source);
    let folded = String::from_utf8_lossy(payload_of(&folded)).into_owned();
    let a1 = folded
        .split("<g class=\"tree-node\"")
        .find(|node| node.contains(">A1</text>"))
        .expect("A1 is still in the document");
    assert!(
        a1.split('>').next().unwrap().contains("display=\"none\""),
        "{a1}"
    );
    assert!(folded.contains("aria-label=\"Expand\""), "{folded}");

    // ... and every raster/vector format shows the same smaller picture.
    let big = export("png", &[], source);
    let small = export("png", &[("view:collapsed:default", "1")], source);
    let (big, small) = (png_size(payload_of(&big)), png_size(payload_of(&small)));
    assert!(small.0 < big.0 && small.1 < big.1, "{small:?} vs {big:?}");
    let pdf = export("pdf", &[("view:collapsed:default", "1")], source);
    assert!(payload_of(&pdf).starts_with(b"%PDF"));

    // An HTML bundle starts in the state of the page.
    let html = export("html", &[("view:collapsed:default", "1")], source);
    let html = String::from_utf8_lossy(payload_of(&html)).into_owned();
    assert!(
        html.contains("data-id=\"n1\"") && html.contains("data-collapsed=\"1\""),
        "{html}"
    );

    // The page is the truth: it can unfold what the source folds, too.
    let folded_source = "tree { A [collapsed] { A1; } }\n";
    let by_source = export("svg", &[], folded_source);
    assert!(String::from_utf8_lossy(payload_of(&by_source)).contains("aria-label=\"Expand\""));
    let unfolded = export("svg", &[("view:collapsed:default", "")], folded_source);
    let unfolded = String::from_utf8_lossy(payload_of(&unfolded)).into_owned();
    assert!(unfolded.contains("aria-label=\"Collapse\""), "{unfolded}");
    assert!(!unfolded.contains("aria-label=\"Expand\""), "{unfolded}");
}

#[test]
fn an_export_shows_the_diagram_that_is_open_in_the_preview() {
    let dir = temp_dir("tree-active");
    std::fs::write(
        dir.join("two.tree"),
        "tree one { Alpha; } tree two { Beta; }\n",
    )
    .unwrap();
    let addr = spawn(&dir, None);
    let source = "tree one { Alpha; } tree two { Beta; }\n";
    let export = |format: &str, extra: &[(&str, &str)]| {
        let mut fields = vec![("source", source)];
        fields.extend_from_slice(extra);
        let response = http_form_bytes(
            addr,
            "POST",
            &format!("/api/export?path=two.tree&format={format}"),
            &form(&fields),
        );
        String::from_utf8_lossy(payload_of(&response)).into_owned()
    };

    assert!(export("svg", &[]).contains(">Alpha</text>"));
    // `two` is the tree on screen (its section key is `two-n1`).
    let svg = export("svg", &[("view:active", "two-n1")]);
    assert!(
        svg.contains(">Beta</text>") && !svg.contains(">Alpha</text>"),
        "{svg}"
    );
    // The bundle opens with it.
    let html = export("html", &[("view:active", "two-n1")]);
    assert!(html.contains("data-root=\"two-n1\""), "{html}");
    // An unknown key is ignored.
    assert!(export("svg", &[("view:active", "nope")]).contains(">Alpha</text>"));
}

#[test]
fn the_view_the_page_sends_is_understood_by_the_server() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("node not installed; skipping the view body test");
        return;
    }
    let dir = temp_dir("view-body-e2e");
    std::fs::write(
        dir.join("two.tree"),
        "tree one { A { B; } } tree two { C { D; } E; }\n",
    )
    .unwrap();
    let addr = spawn(&dir, None);
    let shell = http(addr, "GET", "/", "");
    let page = dir.join("page.html");
    std::fs::write(&page, &shell).unwrap();
    let harness = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/view_body_runtime.js");
    let view = |scenario: &str| {
        let output = Command::new("node")
            .arg(&harness)
            .arg(&page)
            .arg(scenario)
            .output()
            .expect("run node");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };

    // The fields are what the server reads.
    assert_eq!(
        view("trees"),
        "&view:active=two-n1&view:collapsed:one=1&view:collapsed:two-n1=1,3"
    );
    assert_eq!(
        view("unfolded"),
        "&view:active=default&view:collapsed:default="
    );
    assert_eq!(view("activity"), "&view:active=detail");
    assert_eq!(
        view("manifest"),
        "&view:active=default&view:hidden:default=r1%2Cc0%2Conly%3Ac2"
    );
    assert_eq!(
        view("plainmanifest"),
        "&view:active=default&view:hidden:default=m%3Aremoved"
    );
    assert_eq!(
        view("shownmanifest"),
        "&view:active=default&view:hidden:default="
    );
    assert_eq!(view("error"), "");

    // End to end: the second tree is open and its first branch (C) is folded.
    let body = format!(
        "source={}{}",
        encode("tree one { A { B; } } tree two { C { D; } E; }\n"),
        view("trees").replace("two-n1=1,3", "two-n1=1")
    );
    let response = http_form_bytes(addr, "POST", "/api/export?path=two.tree&format=svg", &body);
    let svg = String::from_utf8_lossy(payload_of(&response)).into_owned();
    assert!(
        svg.contains(">E</text>") && !svg.contains(">A</text>"),
        "the second tree: {svg}"
    );
    let d = svg
        .split("<g class=\"tree-node\"")
        .find(|node| node.contains(">D</text>"))
        .expect("D is in the document");
    assert!(
        d.split('>').next().unwrap().contains("display=\"none\""),
        "D is folded away: {d}"
    );
}

#[test]
fn a_manifest_download_leaves_out_what_is_hidden() {
    let dir = temp_dir("manifest-view");
    let source = "manifest { column a { x; y; } column b { x; } column c { z; } }\n";
    std::fs::write(dir.join("m.mfst"), source).unwrap();
    let addr = spawn(&dir, None);
    let export = |view: &str| {
        let body = format!("source={}&view:hidden:default={view}", encode(source));
        let response = http_form_bytes(addr, "POST", "/api/export?path=m.mfst&format=svg", &body);
        String::from_utf8_lossy(payload_of(&response)).into_owned()
    };

    let all = export("");
    assert!(all.contains("data-col=\"c2\""), "{all}");
    // Column c (c2) and row y (r1) are hidden; the filter on a keeps x only.
    let some = export("c2%2Cr1%2Conly%3Ac0");
    assert!(!some.contains("data-col=\"c2\""), "{some}");
    assert!(some.contains("data-row=\"r0\""), "{some}");
    assert!(!some.contains("data-row=\"r1\""), "{some}");
    assert!(!some.contains("data-row=\"r2\""), "{some}");
    // Without the hidden column the file is narrower.
    let width = |svg: &str| {
        let at = svg.find("viewBox=\"").unwrap() + 9;
        svg[at..].split(' ').nth(2).unwrap().parse::<i32>().unwrap()
    };
    assert!(width(&some) < width(&all), "{some}");
    // A PNG of the same view is produced too.
    let body = format!("source={}&view:hidden:default=c2", encode(source));
    let png = http_form_bytes(addr, "POST", "/api/export?path=m.mfst&format=png", &body);
    assert_eq!(&payload_of(&png)[1..4], b"PNG");
}

#[test]
fn the_page_sends_its_view_with_every_download() {
    let dir = temp_dir("view-body");
    let addr = spawn(&dir, None);
    let shell = http(addr, "GET", "/", "");
    assert!(shell.contains("function viewBody()"), "{shell}");
    assert!(
        shell.contains("body: requestBody() + viewBody()"),
        "{shell}"
    );
}

#[test]
fn a_sequence_file_is_served_and_rendered() {
    let dir = temp_dir("sequence");
    let addr = spawn(&dir, None);

    let source = http(addr, "GET", "/api/source?path=chat.seq", "");
    assert!(source.starts_with("HTTP/1.1 200"), "{source}");
    assert!(source.contains("sequence"), "{source}");

    let rendered = http(
        addr,
        "POST",
        "/api/render?path=chat.seq&type=sequence",
        "sequence { participant A; A -> A: \"hi\"; }\n",
    );
    assert!(rendered.starts_with("HTTP/1.1 200"), "{rendered}");
    assert!(rendered.contains("<svg"), "{rendered}");
    assert!(rendered.contains("/assets/font.ttf"), "{rendered}");
}

#[test]
fn a_sequence_file_is_exported_in_every_format() {
    let dir = temp_dir("sequence-export");
    let addr = spawn(&dir, None);
    let source = "sequence { participant A; A -> A: \"hi\"; }\n";

    let svg = http_form(
        addr,
        "POST",
        "/api/export?path=chat.seq&format=svg",
        &form(&[("source", source)]),
    );
    assert!(svg.starts_with("HTTP/1.1 200"), "{svg}");
    assert!(svg.contains("image/svg+xml"), "{svg}");
    assert!(svg.contains("filename=\"chat.svg\""), "{svg}");

    let png = http_form_bytes(
        addr,
        "POST",
        "/api/export?path=chat.seq&format=png",
        &form(&[("source", source)]),
    );
    let header = String::from_utf8_lossy(&png);
    assert!(header.starts_with("HTTP/1.1 200"), "{header}");
    assert!(header.contains("image/png"), "{header}");
    // The body is a PNG image.
    let start = png
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("the header");
    assert_eq!(&png[start + 4..start + 8], b"\x89PNG", "the body is a PNG");

    // The source of a sequence diagram is exported as `.seq`.
    let raw = http_form(
        addr,
        "POST",
        "/api/export?path=chat.seq&format=seq",
        &form(&[("source", source)]),
    );
    assert!(raw.starts_with("HTTP/1.1 200"), "{raw}");
    assert!(raw.contains("filename=\"chat.seq\""), "{raw}");

    // The source format of another type is refused.
    let wrong = http_form(
        addr,
        "POST",
        "/api/export?path=chat.seq&format=diag",
        &form(&[("source", source)]),
    );
    assert!(wrong.starts_with("HTTP/1.1 400"), "{wrong}");
    assert!(wrong.contains("expected seq"), "{wrong}");
}

#[test]
fn the_diagram_type_decides_how_a_file_is_rendered() {
    let dir = temp_dir("type");
    let addr = spawn(&dir, None);

    // An unknown type is refused with the list of the known ones.
    let unknown = http(
        addr,
        "POST",
        "/api/render?path=hello.diag&type=nope",
        "{ A -> B; }\n",
    );
    assert!(unknown.starts_with("HTTP/1.1 400"), "{unknown}");
    assert!(unknown.contains("unknown diagram type"), "{unknown}");

    // A file that is not a diagram is not served.
    let other = http(addr, "GET", "/api/source?path=notes.txt", "");
    assert!(other.starts_with("HTTP/1.1 400"), "{other}");
}
