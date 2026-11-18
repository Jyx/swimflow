//! The interactive HTML bundle: several rendered diagrams in one
//! self-contained page.
//!
//! The page shell (CSS, navigation runtime, breadcrumb) is diagram-agnostic:
//! it only knows about *sections* — a key, a label and the SVG of one
//! diagram. The activity renderer produces its sections with drill-down
//! links; a sequence diagram produces one section without links. Both end up
//! in the same standalone HTML file.

use crate::text::FontSpec;

/// One rendered diagram inside a bundle: a stable key (the deep-link
/// fragment), a human-readable label and the SVG markup.
#[derive(Debug, Clone)]
pub struct Section {
    /// Unique key inside the page; also the `#fragment` of the deep link.
    pub key: String,
    /// Label shown in the breadcrumb.
    pub label: String,
    /// The SVG markup of the diagram.
    pub svg: String,
    /// Behavior scripts the diagram needs in a bundle (for example the
    /// runtime that folds the branches of a tree). A script shared by several
    /// sections is included once.
    pub scripts: Vec<&'static str>,
    /// HTML controls shown in the page bar while the section is active (for
    /// example the "Collapse all" buttons of a tree). Empty when there are
    /// none.
    pub toolbar: String,
}

impl Section {
    /// A section with the given key, label and SVG.
    pub fn new(key: impl Into<String>, label: impl Into<String>, svg: impl Into<String>) -> Self {
        Section {
            key: key.into(),
            label: label.into(),
            svg: svg.into(),
            scripts: Vec::new(),
            toolbar: String::new(),
        }
    }

    /// Add a behavior script the section needs in a bundle.
    pub fn with_script(mut self, script: &'static str) -> Self {
        self.scripts.push(script);
        self
    }

    /// Add HTML controls shown in the page bar while this section is active.
    pub fn with_toolbar(mut self, toolbar: &str) -> Self {
        self.toolbar.push_str(toolbar);
        self
    }
}

/// Build the `@font-face` rule for the page.
///
/// When `embed` is set the font is inlined as base64; otherwise `url` is
/// used (the live preview serves the font once as a static asset).
pub fn font_face_css(font: &FontSpec, embed: bool, url: Option<&str>) -> String {
    let family = html_escape(&font.family);
    if embed {
        format!(
            "@font-face{{font-family:'{family}';src:url(data:font/ttf;base64,{}) \
             format('truetype');}}",
            crate::core::svg::base64(&font.data)
        )
    } else if let Some(url) = url {
        format!(
            "@font-face{{font-family:'{family}';src:url('{}') format('truetype');}}",
            html_escape(url)
        )
    } else {
        String::new()
    }
}

/// Render the bundle page: one `<section>` per diagram plus the navigation
/// runtime.
pub fn bundle(sections: &[Section], root: &str, font_face: &str) -> String {
    let mut body = String::new();
    let mut toolbars = String::new();
    for section in sections {
        let id = html_escape(&section.key);
        let label = html_escape(&section.label);
        let root_attr = if section.key == root {
            " data-root=\"true\""
        } else {
            ""
        };
        body.push_str(&format!(
            "<section class=\"swimflow-diagram\" id=\"{id}\" data-diagram=\"{id}\" \
             data-label=\"{label}\"{root_attr} aria-label=\"{label}\">\n{svg}</section>\n",
            id = id,
            label = label,
            root_attr = root_attr,
            svg = section.svg,
        ));
        if !section.toolbar.is_empty() {
            let active = if section.key == root {
                " is-active"
            } else {
                ""
            };
            toolbars.push_str(&format!(
                "<div class=\"swimflow-toolbar-slot{active}\" data-diagram=\"{id}\">\
                 {toolbar}</div>\n",
                id = id,
                toolbar = section.toolbar,
            ));
        }
    }

    let mut scripts: Vec<&'static str> = Vec::new();
    for script in sections.iter().flat_map(|section| section.scripts.iter()) {
        if !scripts.contains(script) {
            scripts.push(script);
        }
    }
    let extra: String = scripts
        .iter()
        .map(|script| format!("<script>\n{script}\n</script>\n"))
        .collect();

    document_html(root, &body, &toolbars, font_face, &extra)
}

/// The complete HTML document.
fn document_html(
    root: &str,
    body: &str,
    toolbars: &str,
    font_face: &str,
    extra_scripts: &str,
) -> String {
    format!(
        "<!DOCTYPE html>\n\
<html lang=\"en\">\n\
<head>\n\
<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<title>swimflow diagram</title>\n\
<script>document.documentElement.className += ' js';</script>\n\
<style>\n\
{font_face}{css}</style>\n\
</head>\n\
<body>\n\
<div id=\"swimflow\" class=\"swimflow\" data-root=\"{root}\">\n\
<header class=\"swimflow-bar\">\n\
<button type=\"button\" class=\"swimflow-back\" aria-label=\"Go back\">&#8592; Back</button>\n\
<nav class=\"swimflow-breadcrumb\" aria-label=\"Breadcrumb\"></nav>\n\
<div class=\"swimflow-controls\">\n\
<div class=\"swimflow-toolbars\">{toolbars}</div>\n\
<div class=\"swimflow-zoom\" role=\"group\" aria-label=\"Zoom\">\n\
<button type=\"button\" data-zoom=\"out\" aria-label=\"Zoom out\" title=\"Zoom out (-)\">&minus;</button>\n\
<span class=\"swimflow-zoom-level\" aria-live=\"polite\">100%</span>\n\
<button type=\"button\" data-zoom=\"in\" aria-label=\"Zoom in\" title=\"Zoom in (+)\">+</button>\n\
<button type=\"button\" data-zoom=\"reset\" title=\"Actual size (1)\">100%</button>\n\
<button type=\"button\" data-zoom=\"fit\" title=\"Fit to window (0)\">Fit</button>\n\
</div>\n\
</div>\n\
</header>\n\
<main class=\"swimflow-stage\">\n\
{body}</main>\n\
</div>\n\
<script>\n{js}\n\
</script>\n\
<script>\n{zoom}\n\
</script>\n\
{extra_scripts}\
</body>\n\
</html>\n",
        root = html_escape(root),
        toolbars = toolbars,
        css = CSS,
        js = JS,
        zoom = ZOOM_JS,
        extra_scripts = extra_scripts,
    )
}

/// Escape a string for use in HTML text and attribute values.
pub fn html_escape(value: &str) -> String {
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

const CSS: &str = r#":root {
  --font-ui: system-ui, -apple-system, 'Segoe UI', sans-serif;
  --ui: .875rem;
}
* { box-sizing: border-box; }
html, body { margin: 0; padding: 0; }
body { font-family: var(--font-ui); font-size: var(--ui); line-height: 1.4; background: #f4f4f5; color: #18181b; }
.swimflow { min-height: 100vh; }
.swimflow-bar { position: sticky; top: 0; z-index: 10; display: flex; flex-wrap: wrap; gap: .5rem; align-items: center; min-height: 48px; padding: .4rem .75rem; background: rgba(255,255,255,.92); border-bottom: 1px solid #d4d4d8; backdrop-filter: blur(4px); }
.swimflow-bar button, .swimflow-toolbar button { appearance: none; display: inline-flex; align-items: center; justify-content: center; font: inherit; font-size: var(--ui); line-height: 1.2; height: 2rem; padding: 0 .7rem; border: 1px solid #a1a1aa; border-radius: .5rem; background: #fff; color: inherit; cursor: pointer; white-space: nowrap; }
.swimflow-bar button:hover:not([disabled]), .swimflow-toolbar button:hover { background: #f4f4f5; }
.swimflow-bar button[disabled] { opacity: .4; cursor: default; }
.swimflow-controls { display: flex; flex-wrap: wrap; align-items: center; gap: .5rem; margin-left: auto; }
.swimflow-toolbars { display: flex; align-items: center; gap: .5rem; }
.swimflow-toolbar-slot { display: none; align-items: center; }
.swimflow-toolbar-slot.is-active { display: flex; }
.swimflow-toolbar { display: flex; flex-wrap: wrap; align-items: center; gap: .5rem; margin: 0; }
html:not(.js) .swimflow-zoom { display: none; }
html:not(.js) .swimflow-toolbar-slot { display: none; }
.swimflow-zoom { display: flex; gap: .25rem; align-items: center; }
.swimflow-zoom-level { min-width: 3.5em; text-align: center; font-variant-numeric: tabular-nums; }
.swimflow-breadcrumb { display: flex; flex-wrap: wrap; gap: .3rem; align-items: center; font-size: var(--ui); }
.swimflow-back[hidden], .swimflow-breadcrumb[hidden] { display: none; }
.swimflow-breadcrumb a { color: #1d4ed8; text-decoration: none; }
.swimflow-breadcrumb a:hover { text-decoration: underline; }
.swimflow-breadcrumb .sep { color: #71717a; }
.swimflow-stage { display: grid; place-items: start center; padding: 1rem; }
.swimflow-diagram { display: none; max-width: 100%; }
.swimflow-diagram > svg { display: block; max-width: 100%; height: auto; }
.swimflow-diagram.is-active { display: block; animation: swimflow-zoom .28s ease-out; }
.js .swimflow-diagram, .js .swimflow-diagram > svg { max-width: none; }
html:not(.js) .swimflow-diagram:target { display: block; }
html:not(.js) .swimflow:not(:has(.swimflow-diagram:target)) .swimflow-diagram[data-root] { display: block; }
@keyframes swimflow-zoom { from { opacity: 0; transform: scale(.97); } to { opacity: 1; transform: none; } }
@media (prefers-reduced-motion: reduce) { .swimflow-diagram.is-active { animation: none; } }
.swimflow-diagram g[aria-disabled='true'] { opacity: .65; }
.swimflow-diagram a { cursor: pointer; }
"#;

/// Zoom: the bar buttons, the keys and Ctrl/Cmd + wheel (`zoom.js`).
const ZOOM_JS: &str = include_str!("zoom.js");

const JS: &str = r#"(function () {
  'use strict';
  var container = document.getElementById('swimflow');
  if (!container) return;
  var root = container.getAttribute('data-root') || '';
  var sections = Array.prototype.slice.call(document.querySelectorAll('.swimflow-diagram'));
  var toolbarSlots = Array.prototype.slice.call(document.querySelectorAll('.swimflow-toolbar-slot'));
  var byKey = {};
  sections.forEach(function (section) { byKey[section.getAttribute('data-diagram')] = section; });
  var breadcrumb = container.querySelector('.swimflow-breadcrumb');
  var back = container.querySelector('.swimflow-back');
  var stack = [root];
  var current = null;

  function keyFromHash() {
    var value = location.hash.replace(/^#/, '');
    if (!value) return root;
    try { value = decodeURIComponent(value); } catch (error) { /* keep raw */ }
    if (byKey[value]) return value;
    var match = /^(.*)-n[0-9]+$/.exec(value);
    if (match && byKey[match[1]]) return match[1];
    return root;
  }

  function labelFor(key) {
    var section = byKey[key];
    return section ? (section.getAttribute('data-label') || key) : key;
  }

  function paintBar() {
    // A single diagram needs no breadcrumb; the live preview already shows
    // the path. Hide it so the zoom controls stay on one row.
    breadcrumb.hidden = sections.length < 2;
    breadcrumb.textContent = '';
    stack.forEach(function (key, index) {
      if (index) {
        var sep = document.createElement('span');
        sep.className = 'sep';
        sep.textContent = '\u203a';
        breadcrumb.appendChild(sep);
      }
      if (index === stack.length - 1) {
        var span = document.createElement('span');
        span.textContent = labelFor(key);
        span.setAttribute('aria-current', 'page');
        breadcrumb.appendChild(span);
      } else {
        var link = document.createElement('a');
        link.href = '#' + key;
        link.setAttribute('data-target', key);
        link.textContent = labelFor(key);
        breadcrumb.appendChild(link);
      }
    });
    back.disabled = stack.length < 2;
    back.hidden = stack.length < 2;
  }

  function render(key) {
    if (!key || !byKey[key]) key = keyFromHash();
    if (!byKey[key]) key = root;
    if (current !== key) {
      var existing = stack.indexOf(key);
      if (existing >= 0) stack = stack.slice(0, existing + 1);
      else stack.push(key);
      current = key;
      sections.forEach(function (section) {
        section.classList.toggle('is-active', section === byKey[key]);
      });
      toolbarSlots.forEach(function (slot) {
        slot.classList.toggle('is-active', slot.getAttribute('data-diagram') === key);
      });
      // The zoom script sizes the diagram that has just become visible.
      try { document.dispatchEvent(new CustomEvent('swimflow:show', { detail: { key: key } })); } catch (error) { /* no zoom */ }
    }
    paintBar();
  }

  function syncUrl(key) {
    // Deep links are nice to have, not required: embedded contexts such as an
    // `<iframe srcdoc>` preview can forbid history updates. Never let a URL
    // failure stop the navigation itself.
    try {
      history.pushState({ key: key }, '', '#' + key);
    } catch (error) {
      try { location.hash = key; } catch (ignored) { /* ignore */ }
    }
  }

  function navigate(key, push) {
    if (!byKey[key]) return;
    if (push && key !== current) syncUrl(key);
    render(key);
  }

  document.addEventListener('click', function (event) {
    var target = event.target;
    var anchor = target && target.closest ? target.closest('a[data-target]') : null;
    if (!anchor) return;
    event.preventDefault();
    navigate(anchor.getAttribute('data-target'), true);
  });

  back.addEventListener('click', function () {
    if (stack.length > 1) navigate(stack[stack.length - 2], true);
  });

  window.addEventListener('popstate', function () { render(); });
  window.addEventListener('hashchange', function () { render(); });

  document.addEventListener('keydown', function (event) {
    if (event.key === 'Escape' && !back.disabled) {
      event.preventDefault();
      navigate(stack[stack.length - 2], true);
    }
  });

  if (!byKey[root] && sections.length) {
    root = sections[0].getAttribute('data-diagram');
    stack = [root];
  }
  render();
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_become_page_sections() {
        let sections = vec![
            Section::new("main", "main", "<svg/>"),
            Section::new("detail", "detail", "<svg/>"),
        ];
        let html = bundle(&sections, "main", "");
        assert!(html.contains("id=\"main\""));
        assert!(html.contains("id=\"detail\""));
        assert!(html.contains("data-root=\"true\""));
        assert!(html.contains("data-label=\"detail\""));
    }

    #[test]
    fn section_toolbars_live_in_the_page_bar() {
        let sections = vec![
            Section::new("main", "main", "<svg/>").with_toolbar(
                "<div class=\"swimflow-toolbar\"><button>Collapse all</button></div>",
            ),
            Section::new("detail", "detail", "<svg/>")
                .with_toolbar("<div class=\"swimflow-toolbar\"><button>Expand all</button></div>"),
        ];
        let html = bundle(&sections, "main", "");
        let bar = &html[..html.find("<main").unwrap()];
        let stage = &html[html.find("<main").unwrap()..];
        assert!(bar.contains("swimflow-toolbar-slot is-active\" data-diagram=\"main\""));
        assert!(bar.contains("swimflow-toolbar-slot\" data-diagram=\"detail\""));
        assert!(bar.contains("Collapse all"));
        assert!(bar.contains("Expand all"));
        assert!(!stage.contains("Collapse all"), "{stage}");
        assert!(!stage.contains("Expand all"), "{stage}");
    }

    #[test]
    fn the_font_face_is_embedded_on_request() {
        let font = FontSpec::default();
        let css = font_face_css(&font, true, None);
        assert!(css.contains("@font-face"));
        assert!(css.contains("base64,"));
        let url = font_face_css(&font, false, Some("/assets/font.ttf"));
        assert!(url.contains("url('/assets/font.ttf')"));
        assert!(font_face_css(&font, false, None).is_empty());
    }

    #[test]
    fn html_is_escaped() {
        assert_eq!(html_escape("a&b<c>"), "a&amp;b&lt;c&gt;");
    }
}
