//! Built-in HTTP server for live diagram preview (`swimflow --serve`).
//!
//! The server is deliberately small and synchronous: it scans a configured
//! directory for diagram files, serves a single-page editor, and renders the
//! posted source to the same self-contained HTML bundle the CLI produces. The
//! font is served once as an asset instead of being embedded in every render.
//!
//! Every diagram type is supported: the server asks the plugin registry which
//! plugin owns the requested file and renders it with that plugin, so the
//! live preview offers the same features for a `.diag`, `.seq`, `.tree` or
//! `.tmap` file.
//!
//! It is meant for a trusted network. By default it binds all interfaces and
//! has no authentication; set `SWIMFLOW_TOKEN` to require a shared secret.

mod config;
mod tree;
mod web;

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::build::LoadOptions;
use crate::core::overlay::Overlay;
use crate::core::view::ViewState;
use crate::plugin::{self, DiagramPlugin, Registry, RenderContext, RenderMode, TypeInfo};
use crate::render::{Format, RenderOptions};

pub use config::ServeConfig;

/// URL path of the embedded font asset referenced by the preview CSS.
pub const FONT_URL: &str = "/assets/font.ttf";

/// Errors produced while starting or running the server.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("server I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("cannot bind {addr}: {source}")]
    Bind {
        addr: String,
        source: std::io::Error,
    },

    #[error("{0}")]
    Config(String),
}

/// A method + path + query + body, decoupled from the HTTP library so the
/// routing logic can be unit-tested without sockets.
#[derive(Debug)]
pub struct Request<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub body: &'a [u8],
}

/// A response ready to be handed to the HTTP library.
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
    pub headers: Vec<(String, String)>,
}

impl Response {
    pub fn html(body: impl Into<String>) -> Self {
        Response {
            status: 200,
            content_type: "text/html; charset=utf-8",
            body: body.into().into_bytes(),
            headers: Vec::new(),
        }
    }

    pub fn text(status: u16, body: impl Into<String>) -> Self {
        Response {
            status,
            content_type: "text/plain; charset=utf-8",
            body: body.into().into_bytes(),
            headers: Vec::new(),
        }
    }

    pub fn bytes(status: u16, content_type: &'static str, body: Vec<u8>) -> Self {
        Response {
            status,
            content_type,
            body,
            headers: Vec::new(),
        }
    }

    fn with_header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.to_string(), value.into()));
        self
    }
}

/// Route one request. Pure apart from filesystem reads and the renderer.
pub fn handle(config: &ServeConfig, request: &Request<'_>) -> Response {
    if let Some(token) = config.token.as_deref()
        && !authorized(request, token)
    {
        return Response::text(
            401,
            "unauthorized: open this page with ?token=<SWIMFLOW_TOKEN>\n",
        );
    }

    match (request.method, request.path) {
        ("GET", "/") => app_shell(config, request),
        ("GET", "/assets/font.ttf") => Response::bytes(200, "font/ttf", config.font.data.to_vec())
            .with_header("Cache-Control", "public, max-age=31536000, immutable"),
        ("GET", "/api/source") => api_source(config, request),
        ("GET", "/api/render") => api_render(config, request),
        ("POST", "/api/render") => api_render(config, request),
        ("GET", "/api/export") => api_export(config, request),
        ("POST", "/api/export") => api_export(config, request),
        ("GET", "/favicon.ico") => Response::text(204, ""),
        _ => Response::text(404, "not found\n"),
    }
}

fn authorized(request: &Request<'_>, token: &str) -> bool {
    let expected = format!("swimflow_token={token}");
    if request
        .query
        .get("token")
        .is_some_and(|value| value == token)
    {
        return true;
    }
    if request
        .headers
        .get("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|value| value == token)
    {
        return true;
    }
    request.headers.get("cookie").is_some_and(|cookie| {
        cookie
            .split(';')
            .map(str::trim)
            .any(|part| part == expected.as_str())
    })
}

fn app_shell(config: &ServeConfig, request: &Request<'_>) -> Response {
    let registry = Registry::builtin();
    let types = registry.types();

    let scan = tree::scan(&config.dir, &types);

    let mut panels = String::new();
    for (index, info) in types.iter().enumerate() {
        let hidden = if index == 0 { "" } else { " hidden" };
        let body = match &scan {
            Ok(trees) => tree::render(trees.get(info.name).map(Vec::as_slice).unwrap_or(&[])),
            Err(err) => format!(
                "<p class=\"tree-empty\">{}</p>",
                html_escape(&err.to_string())
            ),
        };
        panels.push_str(&format!(
            "<div id=\"tree-{name}\" class=\"tree-panel\" data-type=\"{name}\" \
             aria-label=\"{label}\"{hidden}>{tree}</div>\n",
            name = html_escape(info.name),
            label = html_escape(info.description),
            tree = body,
        ));
    }

    let root = config.dir.display().to_string();
    let html = web::SHELL
        .replace("__ROOT__", &html_escape(&root))
        .replace("__ROOT_JSON__", &json_string(&root))
        .replace("__TABS__", &web::render_tabs(&types))
        .replace("__TYPES_JSON__", &json_types(&registry))
        .replace("__TREES__", &panels)
        .replace("__DEBOUNCE__", &config.debounce_ms.to_string());

    let mut response = Response::html(html);
    if let Some(token) = config.token.as_deref()
        && request
            .query
            .get("token")
            .is_some_and(|value| value == token)
    {
        response = response.with_header(
            "Set-Cookie",
            format!("swimflow_token={token}; Path=/; HttpOnly; SameSite=Lax"),
        );
    }
    response
}

fn api_source(config: &ServeConfig, request: &Request<'_>) -> Response {
    let Some(rel) = request.query.get("path") else {
        return Response::text(400, "missing `path` query parameter\n");
    };
    let path = match resolve_existing(config, rel) {
        Ok(path) => path,
        Err(err) => return Response::text(400, format!("{err}\n")),
    };
    match std::fs::read_to_string(&path) {
        Ok(source) => Response::text(200, source),
        Err(err) => Response::text(404, format!("cannot read {rel}: {err}\n")),
    }
}

/// A request payload: the diagram source plus the files that only exist in
/// the browser session.
struct Payload {
    /// Source of the edited diagram.
    source: String,
    /// Files that exist in memory only (imported or newly created).
    overlay: Overlay,
    /// The state of the preview an export should reproduce.
    view: ViewState,
}

/// Why a payload could not be read.
enum PayloadError {
    /// The request itself is malformed.
    BadRequest(String),
    /// The requested file does not exist.
    NotFound(String),
}

fn api_render(config: &ServeConfig, request: &Request<'_>) -> Response {
    let Some(rel) = request.query.get("path") else {
        return Response::text(400, "missing `path` query parameter\n");
    };
    let base = match resolve_base(config, rel) {
        Ok(path) => path,
        Err(err) => return Response::text(400, format!("{err}\n")),
    };
    let registry = Registry::builtin();
    let plugin = match resolve_plugin(&registry, request, rel) {
        Ok(plugin) => plugin,
        Err(err) => return Response::text(400, format!("{err}\n")),
    };
    let payload = match parse_payload(config, request, &base, rel) {
        Ok(payload) => payload,
        Err(PayloadError::BadRequest(message)) => {
            return Response::text(400, format!("{message}\n"));
        }
        Err(PayloadError::NotFound(message)) => return Response::text(404, format!("{message}\n")),
    };

    // A live editor should show partially-written diagrams rather than fail,
    // so unresolved `ref`s are recorded as issues instead of aborting.
    let load_options = LoadOptions {
        allow_broken_links: true,
        follow_external_files: true,
    };
    let options = RenderOptions {
        font: config.font.clone(),
        ..RenderOptions::default()
    };
    let context = RenderContext {
        source: &payload.source,
        path: Some(&base),
        options: &options,
        mode: RenderMode::Bundle,
        load: &load_options,
        pixels: false,
        overlay: &payload.overlay,
    };
    let rendered = match plugin.render(&context) {
        Ok(rendered) => rendered,
        Err(err) => return Response::text(422, format!("{err}\n")),
    };

    // The font is served once as an asset instead of being embedded in every
    // render.
    match plugin::render_output_with_font_url(Format::Html, &rendered, &options, Some(FONT_URL)) {
        Ok(bytes) => Response::html(String::from_utf8_lossy(&bytes).into_owned()),
        Err(err) => Response::text(422, format!("{err}\n")),
    }
}

/// Export the current diagram in any supported format as a download.
///
/// `GET` re-reads the file; `POST` renders the posted source, so unsaved
/// editor edits are included. The response carries the format's content type
/// and an attachment filename.
fn api_export(config: &ServeConfig, request: &Request<'_>) -> Response {
    let Some(rel) = request.query.get("path") else {
        return Response::text(400, "missing `path` query parameter\n");
    };
    let Some(requested) = request.query.get("format") else {
        return Response::text(400, "missing `format` query parameter\n");
    };
    let base = match resolve_base(config, rel) {
        Ok(path) => path,
        Err(err) => return Response::text(400, format!("{err}\n")),
    };
    let registry = Registry::builtin();
    let plugin = match resolve_plugin(&registry, request, rel) {
        Ok(plugin) => plugin,
        Err(err) => return Response::text(400, format!("{err}\n")),
    };
    let payload = match parse_payload(config, request, &base, rel) {
        Ok(payload) => payload,
        Err(PayloadError::BadRequest(message)) => {
            return Response::text(400, format!("{message}\n"));
        }
        Err(PayloadError::NotFound(message)) => return Response::text(404, format!("{message}\n")),
    };
    let source = payload.source;

    // The source format of a diagram type (`.diag`, `.seq`, …) is returned
    // unchanged: it is the editable source.
    if let Some(info) = type_info(&registry, plugin)
        && requested.eq_ignore_ascii_case(info.source_extension())
    {
        let disposition = attachment(rel, info.source_extension());
        return Response::bytes(200, "text/plain; charset=utf-8", source.into_bytes())
            .with_header("Content-Disposition", disposition);
    }

    let source_format = type_info(&registry, plugin)
        .map(|info| info.source_extension().to_string())
        .unwrap_or_else(|| "diag".to_string());
    let Some(format) = Format::from_name(requested) else {
        return Response::text(
            400,
            format!(
                "unknown export format {requested:?}: expected {source_format}, svg, png, pdf or html\n"
            ),
        );
    };

    // Like `/api/render`, keep going on unresolved `ref`s so a draft can be
    // exported; rendering still fails only when the source cannot be built.
    let load_options = LoadOptions {
        allow_broken_links: true,
        follow_external_files: true,
    };
    // Standalone exports embed the font so a downloaded file works offline.
    // The file is made to look like the preview: the folded branches and the
    // open diagram come from the page.
    let options = RenderOptions {
        embed_font: true,
        scale: config.scale,
        font: config.font.clone(),
        view: payload.view.clone(),
        ..RenderOptions::default()
    };
    let context = RenderContext {
        source: &source,
        path: Some(&base),
        options: &options,
        mode: if format == Format::Html {
            RenderMode::Bundle
        } else {
            RenderMode::Standalone
        },
        load: &load_options,
        pixels: false,
        overlay: &payload.overlay,
    };
    let mut rendered = match plugin.render(&context) {
        Ok(rendered) => rendered,
        Err(err) => return Response::text(422, format!("{err}\n")),
    };
    if let Some(active) = &payload.view.active {
        rendered.show_first(active);
    }

    let body = match plugin::render_output(format, &rendered, &options) {
        Ok(body) => body,
        Err(err) => return Response::text(422, format!("{err}\n")),
    };
    Response::bytes(200, format.content_type(), body)
        .with_header("Content-Disposition", attachment(rel, format.as_str()))
}

/// The plugin that handles a request.
///
/// The type is taken from the `type` query parameter when the client sends
/// one, and from the file extension otherwise.
fn resolve_plugin<'a>(
    registry: &'a Registry,
    request: &Request<'_>,
    rel: &str,
) -> Result<&'a dyn DiagramPlugin, ServerError> {
    if let Some(name) = request.query.get("type") {
        return registry.find(name).ok_or_else(|| {
            ServerError::Config(format!(
                "unknown diagram type {name:?} (available: {})",
                registry.available()
            ))
        });
    }

    let extension = Path::new(rel)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    let name = registry.type_for_extension(extension).ok_or_else(|| {
        ServerError::Config(format!(
            "{rel} is not a diagram file (known types: {})",
            registry.available()
        ))
    })?;
    registry
        .find(name)
        .ok_or_else(|| ServerError::Config(format!("unknown diagram type {name:?}")))
}

/// The type information of a plugin.
fn type_info(registry: &Registry, plugin: &dyn DiagramPlugin) -> Option<TypeInfo> {
    registry
        .types()
        .into_iter()
        .find(|info| info.name == plugin.name())
}

/// A safe attachment filename derived from the served path.
fn attachment(rel: &str, extension: &str) -> String {
    let stem = Path::new(rel)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("diagram");
    let stem: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("attachment; filename=\"{stem}.{extension}\"")
}

/// Resolve a client-supplied relative path against the served directory and
/// verify it cannot escape it or name a file that is not a diagram.
///
/// The file must exist: this is used for reading sources from disk.
fn resolve_existing(config: &ServeConfig, rel: &str) -> Result<PathBuf, ServerError> {
    let canonical = std::fs::canonicalize(config.dir.join(relative_path(rel)?))
        .map_err(|err| ServerError::Config(format!("cannot open {rel}: {err}")))?;
    if !canonical.starts_with(&config.dir) {
        return Err(ServerError::Config(
            "path escapes the served directory".to_string(),
        ));
    }
    if !is_diagram(&canonical) {
        return Err(ServerError::Config(
            "only diagram files are served".to_string(),
        ));
    }
    Ok(canonical)
}

/// Resolve a client-supplied relative path against the served directory.
///
/// The path must be relative, must not escape the served directory and must
/// name a diagram file, but it does **not** have to exist: a path that is not
/// on disk belongs to a buffer that only lives in the browser session.
fn resolve_base(config: &ServeConfig, rel: &str) -> Result<PathBuf, ServerError> {
    let relative = relative_path(rel)?;
    if !is_diagram(&relative) {
        return Err(ServerError::Config(
            "only diagram files are served".to_string(),
        ));
    }
    let joined = config.dir.join(relative);
    // A file that exists must stay inside the served directory, even when it
    // is reached through a symbolic link.
    match std::fs::canonicalize(&joined) {
        Ok(canonical) => {
            if !canonical.starts_with(&config.dir) {
                return Err(ServerError::Config(
                    "path escapes the served directory".to_string(),
                ));
            }
            Ok(canonical)
        }
        Err(_) => Ok(joined),
    }
}

/// Validate a client-supplied relative path and return it unchanged.
fn relative_path(rel: &str) -> Result<PathBuf, ServerError> {
    if rel.is_empty() {
        return Err(ServerError::Config("empty path".to_string()));
    }
    let candidate = Path::new(rel);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ServerError::Config("invalid path".to_string()));
    }
    Ok(candidate.to_path_buf())
}

/// Read the request payload: the diagram source and the files that only exist
/// in the browser session.
///
/// * `application/x-www-form-urlencoded` — `source=<source>` plus any number
///   of `file:<path>=<content>` entries (the session files);
/// * any other content type — the whole body is the source;
/// * a `GET` (or an empty body) — the file is read from disk.
fn parse_payload(
    config: &ServeConfig,
    request: &Request<'_>,
    base: &Path,
    rel: &str,
) -> Result<Payload, PayloadError> {
    let content_type = request
        .headers
        .get("content-type")
        .map(String::as_str)
        .unwrap_or_default();
    let is_form = content_type.split(';').next().is_some_and(|value| {
        value
            .trim()
            .eq_ignore_ascii_case("application/x-www-form-urlencoded")
    });

    if is_form {
        let mut source = None;
        let mut overlay = Overlay::new();
        let mut view = ViewState::default();
        for (key, value) in parse_form(request.body) {
            if key == "source" {
                source = Some(value);
            } else if key == "view:active" {
                if !value.is_empty() {
                    view.active = Some(value);
                }
            } else if let Some(section) = key.strip_prefix("view:collapsed:") {
                // A comma separated list of node indexes; an empty list says
                // that nothing is folded.
                let nodes = value
                    .split(',')
                    .filter_map(|index| index.trim().parse::<usize>().ok())
                    .collect();
                view.collapsed.insert(section.to_string(), nodes);
            } else if let Some(section) = key.strip_prefix("view:hidden:") {
                // A comma separated list of tokens the diagram type defines;
                // an empty list says that nothing is hidden.
                let tokens = value
                    .split(',')
                    .map(str::trim)
                    .filter(|token| !token.is_empty())
                    .map(str::to_string)
                    .collect();
                view.hidden.insert(section.to_string(), tokens);
            } else if let Some(name) = key.strip_prefix("file:") {
                let path = resolve_base(config, name)
                    .map_err(|err| PayloadError::BadRequest(err.to_string()))?;
                overlay.insert(path, value);
            }
        }
        let source =
            source.ok_or_else(|| PayloadError::BadRequest("missing `source` field".to_string()))?;
        return Ok(Payload {
            source,
            overlay,
            view,
        });
    }

    if request.method != "GET" && !request.body.is_empty() {
        return Ok(Payload {
            source: String::from_utf8_lossy(request.body).into_owned(),
            overlay: Overlay::new(),
            view: ViewState::default(),
        });
    }

    let source = std::fs::read_to_string(base)
        .map_err(|err| PayloadError::NotFound(format!("cannot read {rel}: {err}")))?;
    Ok(Payload {
        source,
        overlay: Overlay::new(),
        view: ViewState::default(),
    })
}

/// Parse an `application/x-www-form-urlencoded` body into key/value pairs.
fn parse_form(body: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(body);
    text.split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (percent_decode(key), percent_decode(value))
        })
        .collect()
}

/// Whether a path names a file of one of the known diagram types.
fn is_diagram(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    Registry::builtin()
        .types()
        .iter()
        .any(|info| info.claims(extension))
}

/// Escape text for inclusion in HTML text and attribute values.
pub(crate) fn html_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The diagram types as a JSON array, for the browser side.
fn json_types(registry: &Registry) -> String {
    let mut out = String::from("[");
    for (index, info) in registry.types().iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"name\":{},\"description\":{},\"extensions\":[{}],\"source\":{},\"template\":{}}}",
            json_string(info.name),
            json_string(info.description),
            info.extensions
                .iter()
                .map(|extension| json_string(extension))
                .collect::<Vec<_>>()
                .join(","),
            json_string(info.source_extension()),
            json_string(registry.template(info.name).unwrap_or_default()),
        ));
    }
    out.push(']');
    out
}

fn parse_query(raw: &str) -> BTreeMap<String, String> {
    let mut query = BTreeMap::new();
    for pair in raw.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        query.insert(percent_decode(key), percent_decode(value));
    }
    query
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                match (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                    (Some(high), Some(low)) => {
                        out.push(high * 16 + low);
                        index += 3;
                    }
                    _ => {
                        out.push(bytes[index]);
                        index += 1;
                    }
                }
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// The address the server would bind, for logging and tests.
pub fn bind(config: &ServeConfig) -> Result<tiny_http::Server, ServerError> {
    let addr = format!("{}:{}", config.host, config.port);
    tiny_http::Server::http(addr.as_str()).map_err(|err| ServerError::Bind {
        addr,
        source: std::io::Error::other(err.to_string()),
    })
}

/// Bind and serve until the process is interrupted.
pub fn run(config: ServeConfig) -> Result<(), ServerError> {
    let server = bind(&config)?;
    print_listening(&server, &config);
    serve(server, config)
}

/// Serve requests on an already-bound server. Blocks forever (or until the
/// server is unblocked), which is why it lives apart from [`run`]: tests bind
/// an ephemeral port first and then drive this in a background thread.
pub fn serve(server: tiny_http::Server, config: ServeConfig) -> Result<(), ServerError> {
    let server = Arc::new(server);
    let workers = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .clamp(1, 8);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            let server = Arc::clone(&server);
            let config = &config;
            scope.spawn(move || worker_loop(&server, config));
        }
    });
    Ok(())
}

fn print_listening(server: &tiny_http::Server, config: &ServeConfig) {
    let addr = server
        .server_addr()
        .to_ip()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|| format!("{}:{}", config.host, config.port));
    eprintln!(
        "swimflow: serving {} at http://{addr}/",
        config.dir.display()
    );
    if config.host == "0.0.0.0" || config.host == "::" {
        eprintln!(
            "swimflow: reachable from other machines at http://<this-host>:{}/",
            config.port
        );
    }
    if config.token.is_some() {
        eprintln!("swimflow: token required; open the URL with ?token=<SWIMFLOW_TOKEN>");
    }
    eprintln!("swimflow: press Ctrl+C to stop");
}

fn worker_loop(server: &tiny_http::Server, config: &ServeConfig) {
    for mut request in server.incoming_requests() {
        let response = build_response(config, &mut request);
        if let Err(err) = request.respond(response) {
            eprintln!("swimflow: failed to respond: {err}");
        }
    }
}

fn build_response(
    config: &ServeConfig,
    request: &mut tiny_http::Request,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let method = request.method().as_str().to_string();
    let url = request.url().to_string();
    let (path, raw_query) = match url.split_once('?') {
        Some((path, query)) => (path, query),
        None => (url.as_str(), ""),
    };
    let query = parse_query(raw_query);
    let headers = request
        .headers()
        .iter()
        .map(|header| {
            (
                header.field.as_str().as_str().to_ascii_lowercase(),
                header.value.as_str().to_string(),
            )
        })
        .collect();
    let mut body = Vec::new();
    if let Err(err) = request.as_reader().read_to_end(&mut body) {
        eprintln!("swimflow: failed to read request body: {err}");
    }

    into_tiny(handle(
        config,
        &Request {
            method: &method,
            path,
            query,
            headers,
            body: &body,
        },
    ))
}

fn into_tiny(response: Response) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let mut out = tiny_http::Response::from_data(response.body)
        .with_status_code(response.status)
        // Always send a `Content-Length` instead of switching to chunked
        // transfer encoding: the body is fully known, and clients (and the
        // test harnesses) can then read it as one contiguous document.
        .with_chunked_threshold(usize::MAX)
        .with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], response.content_type.as_bytes())
                .expect("valid content type"),
        );
    for (name, value) in response.headers {
        if let Ok(header) = tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes()) {
            out.add_header(header);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> crate::test_support::TempDir {
        let dir = crate::test_support::TempDir::new(&format!("swimflow-server-{name}-"));
        std::fs::write(dir.join("simple.diag"), "{ A -> B; }\n").unwrap();
        std::fs::write(dir.join("secret.txt"), "do not serve\n").unwrap();
        std::fs::write(dir.join("bad.diag"), "{ A -> ; }\n").unwrap();
        dir
    }

    fn config(dir: &Path) -> ServeConfig {
        ServeConfig {
            dir: dir.to_path_buf(),
            host: "127.0.0.1".to_string(),
            port: 0,
            debounce_ms: 250,
            scale: 1.0,
            token: None,
            font: crate::text::FontSpec::default(),
        }
    }

    fn request<'a>(
        method: &'a str,
        path: &'a str,
        query: &[(&str, &str)],
        body: &'a [u8],
    ) -> Request<'a> {
        Request {
            method,
            path,
            query: query
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
            headers: BTreeMap::new(),
            body,
        }
    }

    fn request_with_headers<'a>(
        method: &'a str,
        path: &'a str,
        query: &[(&str, &str)],
        headers: &[(&str, &str)],
        body: &'a [u8],
    ) -> Request<'a> {
        Request {
            method,
            path,
            query: query
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
            headers: headers
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
            body,
        }
    }

    /// Percent-encode a value the way the browser does (`encodeURIComponent`).
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

    /// Build an `application/x-www-form-urlencoded` body.
    fn form(fields: &[(&str, &str)]) -> Vec<u8> {
        fields
            .iter()
            .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
            .collect::<Vec<_>>()
            .join("&")
            .into_bytes()
    }

    fn form_request<'a>(
        method: &'a str,
        path: &'a str,
        query: &[(&str, &str)],
        fields: &[(&str, &str)],
    ) -> Request<'a> {
        // The body has to outlive the request, so it is leaked into the test
        // process on purpose (the tests are short-lived).
        let body: &'a [u8] = Box::leak(form(fields).into_boxed_slice());
        request_with_headers(
            method,
            path,
            query,
            &[(
                "content-type",
                "application/x-www-form-urlencoded; charset=utf-8",
            )],
            body,
        )
    }

    fn body(response: &Response) -> String {
        String::from_utf8(response.body.clone()).unwrap()
    }

    #[test]
    fn app_shell_embeds_the_tree_and_debounce() {
        let dir = temp_dir("shell");
        let response = handle(&config(&dir), &request("GET", "/", &[], b""));
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/html; charset=utf-8");
        let html = body(&response);
        assert!(html.contains("data-path=\"simple.diag\""), "{html}");
        assert!(html.contains("debounce: 250"), "{html}");
        assert!(!html.contains("secret.txt"), "{html}");
    }

    #[test]
    fn app_shell_has_a_resizable_splitter() {
        let dir = temp_dir("splitter");
        let response = handle(&config(&dir), &request("GET", "/", &[], b""));
        assert_eq!(response.status, 200);
        let html = body(&response);
        assert!(html.contains("id=\"divider\""), "{html}");
        assert!(html.contains("role=\"separator\""), "{html}");
        assert!(html.contains("--split"), "{html}");
        assert!(html.contains("swimflow.split"), "{html}");
    }

    #[test]
    fn app_shell_can_expand_the_preview() {
        let dir = temp_dir("preview-expand");
        let response = handle(&config(&dir), &request("GET", "/", &[], b""));
        assert_eq!(response.status, 200);
        let html = body(&response);
        assert!(html.contains("id=\"preview-toggle\""), "{html}");
        assert!(html.contains("aria-controls=\"preview\""), "{html}");
        assert!(html.contains("preview-expanded"), "{html}");
        assert!(html.contains("swimflow.preview"), "{html}");
    }

    #[test]
    fn app_shell_has_an_export_format_picker() {
        let dir = temp_dir("export-picker");
        let response = handle(&config(&dir), &request("GET", "/", &[], b""));
        assert_eq!(response.status, 200);
        let html = body(&response);
        assert!(html.contains("id=\"format\""), "{html}");
        assert!(html.contains("<option value=\"png\">"), "{html}");
        assert!(html.contains("downloadExport"), "{html}");
        assert!(html.contains("/api/export"), "{html}");
    }

    #[test]
    fn app_shell_can_import_and_create_session_files() {
        let dir = temp_dir("session-shell");
        let response = handle(&config(&dir), &request("GET", "/", &[], b""));
        assert_eq!(response.status, 200);
        let html = body(&response);
        assert!(html.contains("id=\"new\""), "{html}");
        assert!(html.contains("id=\"import\""), "{html}");
        assert!(html.contains("webkitdirectory"), "{html}");
        assert!(html.contains("id=\"session-tree\""), "{html}");
        assert!(html.contains("swimflowSession"), "{html}");
    }

    #[test]
    fn source_endpoint_returns_file_contents() {
        let dir = temp_dir("source");
        let response = handle(
            &config(&dir),
            &request("GET", "/api/source", &[("path", "simple.diag")], b""),
        );
        assert_eq!(response.status, 200);
        assert_eq!(body(&response), "{ A -> B; }\n");
    }

    #[test]
    fn render_endpoint_returns_a_bundle() {
        let dir = temp_dir("render");
        let response = handle(
            &config(&dir),
            &request(
                "POST",
                "/api/render",
                &[("path", "simple.diag")],
                b"{ A -> B; }\n",
            ),
        );
        assert_eq!(response.status, 200, "{}", body(&response));
        let html = body(&response);
        assert!(html.contains("<!DOCTYPE html>"), "{html}");
        assert!(html.contains("<svg"), "{html}");
        assert!(html.contains("id=\"default\""), "{html}");
        assert!(html.contains("/assets/font.ttf"), "{html}");
    }

    #[test]
    fn render_endpoint_reports_parse_errors() {
        let dir = temp_dir("render-error");
        let response = handle(
            &config(&dir),
            &request(
                "POST",
                "/api/render",
                &[("path", "simple.diag")],
                b"{ A -> ; }\n",
            ),
        );
        assert_eq!(response.status, 422);
        assert!(
            body(&response).contains("expected identifier"),
            "{response:?}"
        );
    }

    #[test]
    fn export_endpoint_renders_every_format() {
        let dir = temp_dir("export");
        let config = config(&dir);
        let cases = [
            ("diag", "text/plain; charset=utf-8", b"{".as_slice()),
            ("svg", "image/svg+xml", b"<?xml".as_slice()),
            ("png", "image/png", b"\x89PNG\r\n\x1a\n".as_slice()),
            ("pdf", "application/pdf", b"%PDF".as_slice()),
            (
                "html",
                "text/html; charset=utf-8",
                b"<!DOCTYPE html>".as_slice(),
            ),
        ];
        for (format, content_type, magic) in cases {
            let response = handle(
                &config,
                &request(
                    "POST",
                    "/api/export",
                    &[("path", "simple.diag"), ("format", format)],
                    b"{ A -> B; }\n",
                ),
            );
            assert_eq!(
                response.status,
                200,
                "format {format}: {}",
                String::from_utf8_lossy(&response.body)
            );
            assert_eq!(response.content_type, content_type, "format {format}");
            assert!(
                response.body.starts_with(magic),
                "format {format} has the wrong magic bytes"
            );
            let disposition = response
                .headers
                .iter()
                .find(|(name, _)| name == "Content-Disposition")
                .map(|(_, value)| value.as_str())
                .unwrap_or_default();
            assert!(
                disposition.contains(&format!("simple.{format}")),
                "format {format} disposition {disposition:?}"
            );
        }
    }

    #[test]
    fn export_endpoint_honors_the_png_scale() {
        let dir = temp_dir("export-scale");
        let mut config = config(&dir);
        let png_size = |config: &ServeConfig| {
            let response = handle(
                config,
                &request(
                    "POST",
                    "/api/export",
                    &[("path", "simple.diag"), ("format", "png")],
                    b"{ A -> B; }\n",
                ),
            );
            assert_eq!(response.status, 200, "{}", body(&response));
            let bytes = &response.body;
            assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
            (
                u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
                u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
            )
        };

        let single = png_size(&config);
        config.scale = 2.0;
        let double = png_size(&config);
        assert!(
            double.0 > single.0 && double.1 > single.1,
            "scale 1 gave {single:?}, scale 2 gave {double:?}"
        );
    }

    #[test]
    fn export_endpoint_reads_the_file_on_get_and_rejects_unknown_formats() {
        let dir = temp_dir("export-get");
        let config = config(&dir);
        let response = handle(
            &config,
            &request(
                "GET",
                "/api/export",
                &[("path", "simple.diag"), ("format", "diag")],
                b"",
            ),
        );
        assert_eq!(response.status, 200);
        assert_eq!(body(&response), "{ A -> B; }\n");

        let response = handle(
            &config,
            &request(
                "GET",
                "/api/export",
                &[("path", "simple.diag"), ("format", "jpg")],
                b"",
            ),
        );
        assert_eq!(response.status, 400);
        assert!(body(&response).contains("unknown export format"));
    }

    #[test]
    fn rejects_traversal_absolute_and_non_diag_paths() {
        let dir = temp_dir("safety");
        let config = config(&dir);
        for path in [
            "../secret.txt",
            "/etc/passwd",
            "secret.txt",
            "sub/../../x.diag",
        ] {
            let response = handle(
                &config,
                &request("GET", "/api/source", &[("path", path)], b""),
            );
            assert_eq!(response.status, 400, "path {path:?} should be rejected");
        }
    }

    #[test]
    fn session_buffers_render_without_touching_the_disk() {
        let dir = temp_dir("session");
        let config = config(&dir);
        let response = handle(
            &config,
            &form_request(
                "POST",
                "/api/render",
                &[("path", "imported.diag")],
                &[
                    ("source", "diagram main { A -> B; }"),
                    ("file:imported.diag", "diagram main { A -> B; }"),
                ],
            ),
        );
        assert_eq!(response.status, 200, "{}", body(&response));
        let html = body(&response);
        assert!(html.contains("<svg"), "{html}");
        assert!(html.contains("id=\"main\""), "{html}");
        // The buffer only lives in the browser: nothing was written.
        assert!(!dir.join("imported.diag").exists());
    }

    #[test]
    fn session_files_resolve_cross_references() {
        let dir = temp_dir("session-refs");
        let config = config(&dir);
        let response = handle(
            &config,
            &form_request(
                "POST",
                "/api/render",
                &[("path", "main.diag")],
                &[
                    (
                        "source",
                        "diagram main { A [ref = \"other.diag#detail\"]; }",
                    ),
                    ("file:other.diag", "diagram detail { B; }"),
                ],
            ),
        );
        assert_eq!(response.status, 200, "{}", body(&response));
        let html = body(&response);
        assert!(html.contains("data-target=\"detail\""), "{html}");
        assert!(html.contains("id=\"detail\""), "{html}");
    }

    #[test]
    fn session_files_cannot_escape_the_served_directory() {
        let dir = temp_dir("session-escape");
        let config = config(&dir);
        let response = handle(
            &config,
            &form_request(
                "POST",
                "/api/render",
                &[("path", "main.diag")],
                &[
                    ("source", "diagram main { A; }"),
                    ("file:../evil.diag", "diagram evil { A; }"),
                ],
            ),
        );
        assert_eq!(response.status, 400, "{}", body(&response));
        assert!(body(&response).contains("invalid path"), "{response:?}");
    }

    #[test]
    fn session_buffers_can_be_exported() {
        let dir = temp_dir("session-export");
        let config = config(&dir);
        let response = handle(
            &config,
            &form_request(
                "POST",
                "/api/export",
                &[("path", "sketch.diag"), ("format", "svg")],
                &[("source", "diagram main { A -> B; }")],
            ),
        );
        assert_eq!(response.status, 200, "{}", body(&response));
        assert_eq!(response.content_type, "image/svg+xml");
        assert!(response.body.starts_with(b"<?xml"));
        let disposition = response
            .headers
            .iter()
            .find(|(name, _)| name == "Content-Disposition")
            .map(|(_, value)| value.as_str())
            .unwrap_or_default();
        assert!(disposition.contains("sketch.svg"), "{disposition:?}");
    }

    #[test]
    fn a_form_body_without_a_source_is_rejected() {
        let dir = temp_dir("session-no-source");
        let response = handle(
            &config(&dir),
            &form_request("POST", "/api/render", &[("path", "main.diag")], &[]),
        );
        assert_eq!(response.status, 400);
        assert!(body(&response).contains("missing `source`"), "{response:?}");
    }

    #[test]
    fn a_plain_text_body_still_renders() {
        let dir = temp_dir("plain-body");
        let response = handle(
            &config(&dir),
            &request(
                "POST",
                "/api/render",
                &[("path", "simple.diag")],
                b"{ A -> B; }\n",
            ),
        );
        assert_eq!(response.status, 200, "{}", body(&response));
        assert!(body(&response).contains("<svg"));
    }

    #[test]
    fn unknown_paths_are_not_found() {
        let dir = temp_dir("missing");
        let response = handle(
            &config(&dir),
            &request("GET", "/api/source", &[("path", "missing.diag")], b""),
        );
        assert_eq!(response.status, 400);
        let response = handle(&config(&dir), &request("GET", "/nope", &[], b""));
        assert_eq!(response.status, 404);
    }

    #[test]
    fn token_gate_requires_a_secret() {
        let dir = temp_dir("token");
        let mut config = config(&dir);
        config.token = Some("s3cret".to_string());

        let denied = handle(&config, &request("GET", "/", &[], b""));
        assert_eq!(denied.status, 401);

        let allowed = handle(&config, &request("GET", "/", &[("token", "s3cret")], b""));
        assert_eq!(allowed.status, 200);
        assert!(
            allowed.headers.iter().any(
                |(name, value)| name == "Set-Cookie" && value.contains("swimflow_token=s3cret")
            )
        );
    }

    #[test]
    fn percent_decoding_handles_paths() {
        let query = parse_query("path=sub%2Finner.diag&x=a+b");
        assert_eq!(query.get("path").unwrap(), "sub/inner.diag");
        assert_eq!(query.get("x").unwrap(), "a b");
    }

    #[test]
    fn json_string_escapes_special_characters() {
        assert_eq!(json_string("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
    }
}
