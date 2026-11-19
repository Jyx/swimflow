//! Embedded single-page shell for the live-preview server.
//!
//! The page is dependency-free (no CDN, no build step) and inlines its CSS and
//! JavaScript so the server ships as one static binary. Placeholders are
//! replaced in `server::app_shell`.
//!
//! The shell serves every diagram type: a tab bar selects the type, and each
//! tab has its own file tree, syntax highlighting, starter template and
//! export formats.

use crate::plugin::TypeInfo;

/// HTML/CSS/JS for the editor. Placeholders: `__ROOT__` (escaped directory),
/// `__ROOT_JSON__` (JSON string), `__TABS__` (the type tabs), `__TYPES_JSON__`
/// (the type catalog), `__TREES__` (one tree panel per type) and
/// `__DEBOUNCE__` (milliseconds).
pub const SHELL: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>swimflow live preview</title>
<link rel="icon" href="/favicon.ico" sizes="any">
<style>
:root {
  color-scheme: light;
  --font-ui: system-ui, -apple-system, 'Segoe UI', sans-serif;
  --ui: .875rem;
  --ui-small: .75rem;
  --bg: #f6f6f7;
  --surface: #ffffff;
  --surface-alt: #f4f4f5;
  --surface-sunken: #fbfbfb;
  --line: #e4e4e7;
  --line-strong: #d4d4d8;
  --text: #18181b;
  --muted: #52525b;
  --faint: #71717a;
  --ghost: #a1a1aa;
  --accent: #2563eb;
  --accent-strong: #1e3a8a;
  --accent-soft: #dbeafe;
  --selection: #bfdbfe;
  --danger: #b91c1c;
  --radius: 8px;
  --shadow: 0 1px 2px rgba(0, 0, 0, .05), 0 10px 30px rgba(0, 0, 0, .06);
}
* { box-sizing: border-box; }
html, body { margin: 0; height: 100%; }
body { font-family: var(--font-ui); font-size: var(--ui); color: var(--text); background: var(--bg); display: flex; flex-direction: column; height: 100vh; height: 100dvh; }
.bar { display: flex; flex-wrap: nowrap; gap: .5rem; align-items: center; min-height: 48px; padding: .4rem .75rem; background: var(--surface); border-bottom: 1px solid var(--line); }
.brand { font-weight: 700; letter-spacing: -.01em; }
.path { color: var(--faint); font-size: var(--ui); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 30ch; min-width: 0; flex: 0 1 auto; }
.status { margin-left: auto; display: inline-flex; align-items: center; justify-content: flex-end; gap: .35rem; min-width: 10ch; font-size: var(--ui); color: var(--faint); white-space: nowrap; }
.status:not(:empty)::before { content: ""; width: .4rem; height: .4rem; flex: none; border-radius: 50%; background: currentColor; opacity: .7; }
.status.bad { color: var(--danger); }
.actions { display: flex; gap: .5rem; align-items: center; }
button, .button { appearance: none; display: inline-flex; align-items: center; justify-content: center; font: inherit; font-size: var(--ui); line-height: 1.2; height: 2rem; padding: 0 .7rem; border: 1px solid var(--line-strong); border-radius: var(--radius); background: var(--surface); color: inherit; cursor: pointer; text-decoration: none; white-space: nowrap; transition: background-color .12s, border-color .12s, color .12s; }
button:disabled { opacity: .45; cursor: default; }
button:hover:not(:disabled), .button:hover { background: var(--surface-alt); }
#download:not(:disabled) { background: var(--accent); border-color: var(--accent); color: #fff; }
#download:hover:not(:disabled) { background: #1d4ed8; border-color: #1d4ed8; }
#download:disabled { background: var(--surface); color: inherit; }
.actions select { font: inherit; font-size: var(--ui); height: 2rem; padding: 0 .5rem; border: 1px solid transparent; border-radius: var(--radius); background: transparent; color: inherit; cursor: pointer; }
.actions select:hover { background: var(--surface-alt); }
:where(button, .button, select, input):focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.layout { flex: 1; display: grid; grid-template-columns: 260px 1fr; min-height: 0; }
.sidebar { border-right: 1px solid var(--line); background: var(--surface); overflow: auto; padding: .625rem; min-height: 0; }
.type-tabs { display: flex; gap: .125rem; min-width: 0; margin-left: .25rem; padding-left: .5rem; border-left: 1px solid var(--line); overflow-x: auto; scrollbar-width: none; }
.type-tab { flex: none; display: inline-flex; align-items: center; justify-content: center; height: 2rem; padding: 0 .7rem; border: 0; border-radius: var(--radius); background: transparent; font-size: var(--ui); font-weight: 500; line-height: 1; color: var(--faint); white-space: nowrap; }
.type-tab:hover:not(:disabled) { background: var(--surface-alt); color: var(--text); }
.type-tab[aria-selected="true"] { background: var(--accent-soft); color: var(--accent-strong); font-weight: 600; box-shadow: none; }
.type-tab:focus-visible { outline-offset: -2px; }
.sidebar-toggle { display: inline-flex; align-items: center; justify-content: center; width: 2rem; height: 2rem; padding: 0; border-color: transparent; background: transparent; color: var(--muted); font-size: 1rem; }
.sidebar-toggle:hover:not(:disabled) { background: var(--surface-alt); color: var(--text); }
.preview-toggle { border-color: transparent; background: transparent; color: var(--muted); }
.preview-toggle:hover:not(:disabled) { background: var(--surface-alt); color: var(--text); }
.preview-toggle[aria-pressed="true"] { background: var(--accent-soft); color: var(--accent-strong); }
.preview-toggle[aria-pressed="true"]:hover:not(:disabled) { background: var(--accent-soft); color: var(--accent-strong); }
body.no-sidebar .layout { grid-template-columns: 1fr; }
body.no-sidebar .sidebar { display: none; }
body.preview-expanded .layout { grid-template-columns: 1fr; grid-template-rows: 1fr; }
body.preview-expanded .sidebar { display: none; }
body.preview-expanded .panes { grid-template-columns: 1fr; grid-template-rows: 1fr; }
body.preview-expanded .editor-pane,
body.preview-expanded .divider { display: none; }
body.preview-expanded .preview-pane { padding: 0; }
body.preview-expanded #preview { border: 0; border-radius: 0; box-shadow: none; }
.sidebar input[type="search"] { width: 100%; padding: .35rem .5rem; border: 1px solid var(--line-strong); border-radius: var(--radius); font: inherit; background: var(--surface); margin-bottom: .5rem; }
.sidebar input[type="search"]:focus { border-color: var(--accent); outline: 2px solid var(--accent-soft); outline-offset: 0; }
.sidebar-actions { display: flex; flex-wrap: wrap; gap: .35rem; margin-bottom: .5rem; }
.sidebar-actions .button { padding: 0 .55rem; }
ul.tree { list-style: none; margin: 0; padding-left: .75rem; }
ul.tree ul.tree { border-left: 1px solid var(--line); }
li.file > a { display: block; padding: .2rem .4rem .2rem .5rem; border-left: 2px solid transparent; border-radius: 6px; color: var(--accent); text-decoration: none; font-size: var(--ui); }
li.file > a:hover { background: var(--surface-alt); }
li.file > a.active { background: var(--accent-soft); border-left-color: var(--accent); color: var(--accent-strong); font-weight: 600; }
li.file > a:focus-visible, summary:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
summary { cursor: pointer; font-size: var(--ui); font-weight: 600; color: var(--muted); padding: .15rem 0; }
summary:hover { color: var(--text); }
.tree-empty { color: var(--faint); font-size: var(--ui); }
.session { margin-bottom: .5rem; padding-bottom: .5rem; border-bottom: 1px solid var(--line); }
.session-title { display: flex; align-items: baseline; gap: .4rem; margin: 0 0 .25rem; font-size: var(--ui-small); text-transform: uppercase; letter-spacing: .05em; color: var(--faint); }
.session-hint { text-transform: none; letter-spacing: 0; color: var(--ghost); }
li.file.session-file { display: flex; align-items: center; gap: .25rem; }
li.file.session-file > a { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
li.file.session-file > .remove { padding: 0 .3rem; font-size: var(--ui); line-height: 1.3; color: var(--ghost); border-color: transparent; background: transparent; }
li.file.session-file > .remove:hover { color: var(--danger); background: #fef2f2; }
.panes { display: grid; grid-template-columns: var(--split, 45%) 9px 1fr; min-height: 0; }
.editor-pane, .preview-pane { min-width: 0; min-height: 0; display: flex; }
.preview-pane { padding: .75rem; background: var(--bg); }
.divider { position: relative; touch-action: none; cursor: col-resize; background: transparent; border: 0; padding: 0; }
.divider::before { content: ""; position: absolute; top: 0; bottom: 0; left: 50%; width: 1px; transform: translateX(-50%); background: var(--line); }
.divider:hover::before, .divider:focus-visible::before, body.splitting .divider::before { width: 2px; background: var(--accent); }
.divider:focus-visible { outline: none; }
body.splitting { cursor: col-resize; user-select: none; }
body.splitting iframe { pointer-events: none; }
.editor { position: relative; flex: 1; min-width: 0; min-height: 0; background: var(--surface-sunken); }
#source, #highlight { position: absolute; inset: 0; margin: 0; border: 0; padding: 1rem; font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-size: var(--ui); line-height: 1.55; tab-size: 2; font-variant-ligatures: none; white-space: pre-wrap; overflow-wrap: break-word; word-break: normal; }
#highlight { overflow: hidden; pointer-events: none; color: var(--text); }
#source { overflow: auto; resize: none; outline: none; background: transparent; color: transparent; caret-color: var(--accent); z-index: 1; }
#source::placeholder { color: var(--ghost); }
#source::selection { background: var(--selection); }
#highlight .tok-comment { color: var(--faint); }
#highlight .tok-string { color: #0f766e; }
#highlight .tok-keyword { color: #7c3aed; font-weight: 600; }
#highlight .tok-attr { color: #b45309; }
#highlight .tok-edge { color: #dc2626; }
#highlight .tok-number { color: #2563eb; }
#highlight .tok-op { color: var(--muted); }
#preview { flex: 1; width: 100%; min-width: 0; min-height: 0; border: 1px solid var(--line); border-radius: 10px; background: var(--surface); box-shadow: var(--shadow); }
@media (max-width: 900px) {
  .layout { grid-template-columns: 1fr; grid-template-rows: auto 1fr; }
  .sidebar { max-height: 200px; border-right: 0; border-bottom: 1px solid var(--line); }
  .panes { grid-template-columns: 1fr; grid-template-rows: 1fr 1fr; }
  .bar { flex-wrap: wrap; }
  .path { display: none; }
  .type-tabs { margin-left: 0; padding-left: 0; border-left: 0; }
  .divider { display: none; }
  .editor-pane { border-bottom: 1px solid var(--line); }
  .preview-pane { padding: .5rem; }
}
</style>
</head>
<body>
<header class="bar">
  <button type="button" class="sidebar-toggle" id="sidebar-toggle" aria-controls="sidebar" aria-expanded="true" aria-label="Show or hide the file list" title="Show or hide the file list (Ctrl+B)">&#9776;</button>
  <span class="brand">swimflow</span>
  <div class="type-tabs" id="type-tabs" role="tablist" aria-label="Diagram type">__TABS__</div>
  <span class="path" id="path" title="Served directory">__ROOT__</span>
  <span class="status" id="status" aria-live="polite"></span>
  <span class="actions">
    <button type="button" id="preview-toggle" class="preview-toggle" aria-controls="preview" aria-pressed="false" title="Expand the rendered preview to the window">Expand</button>
    <select id="format" aria-label="Export format" title="Export format">
      <option id="format-source" value="diag">.diag source</option>
      <option value="svg" selected>SVG</option>
      <option value="png">PNG</option>
      <option value="pdf">PDF</option>
      <option value="html">HTML bundle</option>
    </select>
    <button type="button" id="download" disabled title="Export the current buffer in the selected format (Ctrl+S exports .diag)">Download</button>
  </span>
</header>
<div class="layout">
  <aside class="sidebar" id="sidebar" aria-label="Diagram files">
    <div class="sidebar-actions">
      <button type="button" id="new" title="Create a new diagram in this browser session">New</button>
      <label class="button" for="import" title="Import diagram files from your computer into this browser session">Import</label>
      <input type="file" id="import" accept=".diag" multiple hidden>
      <label class="button" for="import-folder" title="Import a folder of diagram files into this browser session">Folder</label>
      <input type="file" id="import-folder" webkitdirectory directory multiple hidden>
    </div>
    <input type="search" id="filter" placeholder="Filter files&hellip;" autocomplete="off" aria-label="Filter files">
    <section id="session" class="session" hidden aria-label="Files in this browser session">
      <h2 class="session-title">In this session<span class="session-hint">never written to disk</span></h2>
      <ul class="tree" id="session-tree"></ul>
    </section>
    <div id="tree">__TREES__</div>
  </aside>
  <main class="panes">
    <section class="editor-pane" aria-label="Diagram source">
      <div class="editor">
        <pre id="highlight" aria-hidden="true"></pre>
        <textarea id="source" spellcheck="false" placeholder="Select a .diag file&hellip;" disabled aria-label="Diagram source"></textarea>
      </div>
    </section>
    <div class="divider" id="divider" role="separator" aria-orientation="vertical" aria-label="Resize editor and preview" title="Drag to resize (double-click to reset)" tabindex="0"></div>
    <section class="preview-pane" aria-label="Rendered diagram">
      <iframe id="preview" title="Rendered diagram preview"></iframe>
    </section>
  </main>
</div>
<script>
// Dependency-free syntax highlighter for the DSL. Deliberately approximate:
// it is a highlighter, not a parser, and must never throw on partial input.
// The grammar depends on the diagram type, so the editor can highlight an
// activity, a sequence, a tree, a treemap and a manifest diagram alike. Exposed on `window` so the
// Node regression harness can exercise it without a DOM.
// See `tests/highlight_runtime.js`.
(function (global) {
  'use strict';
  var GRAMMARS = {
    activity: {
      keywords: {
        diagram: true, actdiag: true, lane: true, class: true, plugin: true,
        legend: true, entry: true, numbered: true, orientation: true
      },
      relations: ['<->', '-->', '->', '<-', '--', '=>']
    },
    sequence: {
      keywords: {
        sequence: true, participant: true, as: true, note: true, over: true,
        loop: true, opt: true, par: true, alt: true, else: true, and: true,
        critical: true, break: true, end: true
      },
      relations: ['->>', '-->', '->', '--']
    },
    tree: {
      keywords: { tree: true, root: true, orientation: true },
      relations: []
    },
    treemap: {
      keywords: { treemap: true, root: true, class: true },
      relations: []
    },
    manifest: {
      keywords: {
        manifest: true, group: true, column: true, row: true, rowgroup: true,
        remove: true, override: true, class: true
      },
      relations: []
    }
  };
  var NAME_START = /[A-Za-z_]/;
  var NAME_BODY = /[A-Za-z0-9_.\-]/;

  function escapeHtml(value) {
    return value.replace(/[&<>]/g, function (ch) {
      if (ch === '&') return '&amp;';
      if (ch === '<') return '&lt;';
      return '&gt;';
    });
  }

  function isDigit(ch) { return ch >= '0' && ch <= '9'; }

  function highlightSource(text, type) {
    var grammar = GRAMMARS[type] || GRAMMARS.activity;
    var KEYWORDS = grammar.keywords;
    var RELATIONS = grammar.relations;
    var out = '';
    var i = 0;
    var n = text.length;
    var inAttrs = false;
    while (i < n) {
      var c = text[i];
      var two = text.substr(i, 2);

      if (c === '#' || two === '//') {
        var lineEnd = text.indexOf('\n', i);
        if (lineEnd < 0) lineEnd = n;
        out += '<span class="tok-comment">' + escapeHtml(text.slice(i, lineEnd)) + '</span>';
        i = lineEnd;
        continue;
      }
      if (two === '/*') {
        var blockEnd = text.indexOf('*/', i + 2);
        blockEnd = blockEnd < 0 ? n : blockEnd + 2;
        out += '<span class="tok-comment">' + escapeHtml(text.slice(i, blockEnd)) + '</span>';
        i = blockEnd;
        continue;
      }
      if (c === '"' || c === "'") {
        var triple = text.substr(i, 3) === c + c + c;
        var delim = triple ? c + c + c : c;
        var j = i + delim.length;
        var closed = false;
        while (j < n) {
          if (text.substr(j, delim.length) === delim && text[j - 1] !== '\\') {
            j += delim.length;
            closed = true;
            break;
          }
          j++;
        }
        if (!closed) j = n;
        out += '<span class="tok-string">' + escapeHtml(text.slice(i, j)) + '</span>';
        i = j;
        continue;
      }

      var relation = null;
      for (var r = 0; r < RELATIONS.length; r++) {
        if (text.substr(i, RELATIONS[r].length) === RELATIONS[r]) { relation = RELATIONS[r]; break; }
      }
      if (relation) {
        out += '<span class="tok-edge">' + relation + '</span>';
        i += relation.length;
        continue;
      }
      if (isDigit(c) || (c === '.' && isDigit(text[i + 1])) || (c === '-' && isDigit(text[i + 1]))) {
        var numEnd = i + 1;
        while (numEnd < n && (isDigit(text[numEnd]) || text[numEnd] === '.')) numEnd++;
        out += '<span class="tok-number">' + escapeHtml(text.slice(i, numEnd)) + '</span>';
        i = numEnd;
        continue;
      }
      if (NAME_START.test(c)) {
        var nameEnd = i + 1;
        while (nameEnd < n && NAME_BODY.test(text[nameEnd])) nameEnd++;
        var name = text.slice(i, nameEnd);
        var look = nameEnd;
        while (look < n && (text[look] === ' ' || text[look] === '\t')) look++;
        var cls = 'tok-name';
        if (KEYWORDS[name]) cls = 'tok-keyword';
        else if (inAttrs && text[look] === '=') cls = 'tok-attr';
        out += '<span class="' + cls + '">' + escapeHtml(name) + '</span>';
        i = nameEnd;
        continue;
      }
      if (c === '[') inAttrs = true;
      if (c === ']') inAttrs = false;
      if ('[]{}();,=:'.indexOf(c) >= 0) {
        out += '<span class="tok-op">' + escapeHtml(c) + '</span>';
        i++;
        continue;
      }
      out += escapeHtml(c);
      i++;
    }
    return out + '\n';
  }

  global.swimflowHighlight = highlightSource;
  global.swimflowGrammars = GRAMMARS;
})(window);
</script>
<script>
// Session buffers: files that live only in this browser session.
//
// They are imported from the user's disk or created from scratch; the server
// never writes them, so they disappear with the tab. The logic is kept
// dependency-free and exposed on `window` so the Node regression harness can
// exercise it without a DOM. See `tests/session_runtime.js`.
(function (global) {
  'use strict';
  var STORAGE_KEY = 'swimflow.session';

  function has(store, key) {
    return Object.prototype.hasOwnProperty.call(store, key);
  }

  // The extension of a file name (`chat.seq` -> `seq`), or an empty string.
  function extensionOf(name) {
    var text = String(name || '');
    var base = text.split('/').pop() || '';
    var index = base.lastIndexOf('.');
    if (index <= 0) return '';
    return base.slice(index + 1).toLowerCase();
  }

  // Does the name belong to one of the extensions? The extensions come from
  // the diagram type, so the same session store holds every diagram type.
  function matchesExtension(name, extensions) {
    var list = extensions || [];
    if (!list.length) return false;
    var extension = extensionOf(name);
    if (!extension) return false;
    for (var i = 0; i < list.length; i++) {
      if (extension === String(list[i]).toLowerCase()) return true;
    }
    return false;
  }

  // The directory part of a relative path ('' for a file at the root).
  function dirOf(path) {
    var text = String(path || '');
    var index = text.lastIndexOf('/');
    return index < 0 ? '' : text.slice(0, index);
  }

  // Are two paths close enough that a `ref` between them may resolve?
  // A reference is resolved relative to the referring file, so the same
  // directory, an ancestor or a descendant directory are all relevant.
  function isRelated(a, b) {
    var da = dirOf(a);
    var db = dirOf(b);
    if (da === db || da === '' || db === '') return true;
    return da.indexOf(db + '/') === 0 || db.indexOf(da + '/') === 0;
  }

  function list(store) {
    return Object.keys(store).sort();
  }

  // A name that is not taken yet (`name.diag`, `name-2.diag`, …).
  function uniqueName(store, name) {
    var wanted = name || 'untitled.diag';
    if (!has(store, wanted)) return wanted;
    var extension = extensionOf(wanted);
    var suffix = extension ? '.' + extension : '';
    var base = extension ? wanted.slice(0, wanted.length - extension.length - 1) : wanted;
    var index = 2;
    while (has(store, base + '-' + index + suffix)) index++;
    return base + '-' + index + suffix;
  }

  function create(store, template, name) {
    var path = uniqueName(store, name);
    store[path] = template == null ? '' : String(template);
    return path;
  }

  function add(store, path, content) {
    store[path] = content == null ? '' : String(content);
    return path;
  }

  function remove(store, path) {
    if (!has(store, path)) return false;
    delete store[path];
    return true;
  }

  // Rename a buffer; refuses to overwrite an existing one.
  function rename(store, from, to) {
    if (!to || to === from || !has(store, from) || has(store, to)) return null;
    var content = store[from];
    delete store[from];
    store[to] = content;
    return to;
  }

  // Persist the session; storage may be unavailable or full, in which case
  // the buffers simply stay in memory.
  function save(store, storage) {
    try {
      storage.setItem(STORAGE_KEY, JSON.stringify(store));
      return true;
    } catch (error) {
      return false;
    }
  }

  function load(storage) {
    var store = {};
    try {
      var raw = storage.getItem(STORAGE_KEY);
      if (!raw) return store;
      var parsed = JSON.parse(raw);
      if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return store;
      Object.keys(parsed).forEach(function (key) {
        if (typeof parsed[key] === 'string') store[key] = parsed[key];
      });
    } catch (error) {
      return {};
    }
    return store;
  }

  // Encode the render/export payload: the edited source plus the session
  // files that may be referenced by it.
  function encodeBody(source, files) {
    var parts = ['source=' + encodeURIComponent(source == null ? '' : source)];
    for (var i = 0; i < (files || []).length; i++) {
      parts.push('file:' + encodeURIComponent(files[i].path) + '=' + encodeURIComponent(files[i].content));
    }
    return parts.join('&');
  }

  global.swimflowSession = {
    STORAGE_KEY: STORAGE_KEY,
    extensionOf: extensionOf,
    matchesExtension: matchesExtension,
    dirOf: dirOf,
    isRelated: isRelated,
    list: list,
    uniqueName: uniqueName,
    create: create,
    add: add,
    remove: remove,
    rename: rename,
    save: save,
    load: load,
    encodeBody: encodeBody
  };
})(window);
</script>
<script>
(function () {
  'use strict';
  var config = {
    root: __ROOT_JSON__,
    debounce: __DEBOUNCE__,
    types: __TYPES_JSON__
  };
  var typeTabs = document.getElementById('type-tabs');
  var sessionTree = document.getElementById('session-tree');
  var sessionBox = document.getElementById('session');
  var filter = document.getElementById('filter');
  var source = document.getElementById('source');
  var highlight = document.getElementById('highlight');
  var preview = document.getElementById('preview');
  var status = document.getElementById('status');
  var pathLabel = document.getElementById('path');
  var download = document.getElementById('download');
  // The bar shows the path relative to the served directory; the absolute
  // path stays available as a tooltip.
  var rootLabel = config.root.replace(/\/+$/, '').split('/').pop() || config.root;
  var formatSelect = document.getElementById('format');
  var newButton = document.getElementById('new');
  var importInput = document.getElementById('import');
  var importFolderInput = document.getElementById('import-folder');
  var panes = document.querySelector('.panes');
  var divider = document.getElementById('divider');
  var previewToggle = document.getElementById('preview-toggle');
  var current = null;
  var currentIsSession = false;
  var activeType = config.types.length ? config.types[0].name : '';
  var timer = null;
  var lastRendered = null;
  var SPLIT_KEY = 'swimflow.split';
  var DEFAULT_SPLIT = 0.45;
  var split = DEFAULT_SPLIT;
  try {
    var stored = parseFloat(localStorage.getItem(SPLIT_KEY));
    if (stored > 0.1 && stored < 0.9) split = stored;
  } catch (error) { /* localStorage may be unavailable */ }

  // Session buffers: imported from the user's disk or created here. They
  // live in the browser only — the server never writes them to disk.
  var storage = (function () {
    try {
      var probe = window.sessionStorage;
      probe.setItem('swimflow.probe', '1');
      probe.removeItem('swimflow.probe');
      return probe;
    } catch (error) {
      return {
        getItem: function () { return null; },
        setItem: function () { },
        removeItem: function () { }
      };
    }
  })();
  var session = window.swimflowSession.load(storage);

  // --- diagram types -----------------------------------------------------

  // The catalog comes from the server: every registered diagram type with
  // its extensions, its source extension and a starter template.
  function typeInfo(name) {
    for (var i = 0; i < config.types.length; i++) {
      if (config.types[i].name === name) return config.types[i];
    }
    return null;
  }

  function activeInfo() {
    return typeInfo(activeType);
  }

  // The type that owns a path, decided by its extension.
  function typeForPath(path) {
    var extension = window.swimflowSession.extensionOf(path);
    if (!extension) return null;
    for (var i = 0; i < config.types.length; i++) {
      var info = config.types[i];
      for (var j = 0; j < info.extensions.length; j++) {
        if (String(info.extensions[j]).toLowerCase() === extension) return info.name;
      }
    }
    return null;
  }

  // The file tree of a type.
  function treePanel(name) {
    return document.getElementById('tree-' + name);
  }

  function acceptList() {
    var info = activeInfo();
    if (!info) return '';
    var parts = [];
    for (var i = 0; i < info.extensions.length; i++) parts.push('.' + info.extensions[i]);
    return parts.join(',');
  }

  // Switch to another diagram type: the tree, the syntax highlighting, the
  // export formats and the session list all follow.
  function setType(name, keepFile) {
    if (!typeInfo(name)) return;
    activeType = name;

    if (typeTabs) {
      var tabs = typeTabs.querySelectorAll('.type-tab');
      for (var i = 0; i < tabs.length; i++) {
        var selected = tabs[i].getAttribute('data-type') === name;
        tabs[i].setAttribute('aria-selected', selected ? 'true' : 'false');
      }
    }

    for (var t = 0; t < config.types.length; t++) {
      var panel = treePanel(config.types[t].name);
      if (panel) panel.hidden = config.types[t].name !== name;
    }

    var info = activeInfo();
    source.placeholder = 'Select a .' + info.source + ' file…';
    updateFormatOptions();
    importInput.setAttribute('accept', acceptList());
    importFolderInput.setAttribute('accept', acceptList());
    updateHighlight();
    renderSession();
    applyFilter();

    // A file of another type does not belong to this tab.
    if (!keepFile && current && typeForPath(current) !== name) closeFile();
  }

  // The export format of the current source; it depends on the type of the
  // file that is open.
  function sourceFormat() {
    var info = typeInfo(currentType()) || activeInfo();
    return info ? info.source : 'diag';
  }

  function updateFormatOptions() {
    var info = activeInfo();
    if (!info) return;
    var option = document.getElementById('format-source');
    if (option) {
      option.value = info.source;
      option.textContent = '.' + info.source + ' source';
    }
  }

  function setPath(path, isSession) {
    if (!path) {
      pathLabel.textContent = rootLabel;
      pathLabel.title = config.root;
      return;
    }
    var suffix = isSession ? ' (session)' : '';
    pathLabel.textContent = path + suffix;
    pathLabel.title = config.root.replace(/\/+$/, '') + '/' + path + suffix;
  }

  function closeFile() {
    current = null;
    currentIsSession = false;
    source.value = '';
    source.disabled = true;
    download.disabled = true;
    preview.srcdoc = '';
    lastRendered = null;
    setPath(null);
    markActive(null, false);
    setStatus('');
  }

  function applySplit() {
    panes.style.setProperty('--split', (split * 100) + '%');
  }

  function saveSplit() {
    try { localStorage.setItem(SPLIT_KEY, split.toFixed(4)); } catch (error) { /* ignore */ }
  }

  function setSplit(value) {
    if (value < 0.15) value = 0.15;
    if (value > 0.85) value = 0.85;
    split = value;
    applySplit();
    saveSplit();
  }

  applySplit();

  function encode(path) { return encodeURIComponent(path); }

  // The <textarea> is transparent and stacked on a <pre> that mirrors its text
  // with colour spans; both layers are kept aligned by size and scroll sync.
  function syncHighlightPadding() {
    // Reserve the textarea's scrollbar width so the two layers wrap alike.
    // Keep the 16 in step with the 1rem padding of #source and #highlight.
    var scrollbar = source.offsetWidth - source.clientWidth;
    highlight.style.paddingRight = (16 + (scrollbar > 0 ? scrollbar : 0)) + 'px';
  }

  function updateHighlight() {
    highlight.innerHTML = window.swimflowHighlight(source.value, activeType);
    syncHighlightPadding();
    syncHighlightScroll();
  }

  function syncHighlightScroll() {
    highlight.scrollTop = source.scrollTop;
    highlight.scrollLeft = source.scrollLeft;
  }

  function setStatus(text, bad) {
    status.textContent = text || '';
    status.className = bad ? 'status bad' : 'status';
  }

  function markActive(path, isSession) {
    var panel = treePanel(activeType);
    if (panel) {
      var links = panel.querySelectorAll('a[data-path]');
      for (var i = 0; i < links.length; i++) {
        var link = links[i];
        var active = !isSession && link.getAttribute('data-path') === path;
        link.classList.toggle('active', active);
        if (active) {
          var details = link.closest('details');
          while (details) {
            details.open = true;
            details = details.parentElement ? details.parentElement.closest('details') : null;
          }
        }
      }
    }
    var sessionLinks = sessionTree.querySelectorAll('a[data-session]');
    for (var j = 0; j < sessionLinks.length; j++) {
      var sessionLink = sessionLinks[j];
      sessionLink.classList.toggle('active', !!isSession && sessionLink.getAttribute('data-session') === path);
    }
  }

  function errorPage(message) {
    function esc(value) {
      return String(value).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }
    return '<!DOCTYPE html><html><head><meta charset="utf-8"><style>' +
      'body{font-family:system-ui,sans-serif;margin:2rem;color:#b91c1c}' +
      'pre{white-space:pre-wrap;background:#fef2f2;padding:1rem;border-radius:.5rem;color:#7f1d1d}' +
      '</style></head><body><h1>Could not render</h1><pre>' + esc(message) + '</pre></body></html>';
  }

  // The session files that may be referenced by the current diagram: the
  // ones in the same directory (or an ancestor/descendant of it).
  function sessionFiles() {
    var files = [];
    var paths = window.swimflowSession.list(session);
    for (var i = 0; i < paths.length; i++) {
      var path = paths[i];
      if (!window.swimflowSession.isRelated(path, current)) continue;
      // A file on disk wins over a session file of the same name.
      if (path === current && !currentIsSession) continue;
      files.push({
        path: path,
        content: path === current ? source.value : session[path]
      });
    }
    return files;
  }

  function requestBody() {
    return window.swimflowSession.encodeBody(source.value, sessionFiles());
  }

  // The type of the file that is open; it is the type of the active tab.
  function currentType() {
    return typeForPath(current) || activeType;
  }

  function renderNow() {
    if (!current) return;
    var payload = requestBody();
    if (payload === lastRendered) return;
    setStatus('rendering…');
    fetch('/api/render?path=' + encode(current) + '&type=' + encode(currentType()), {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded; charset=utf-8' },
      body: payload
    }).then(function (response) {
      return response.text().then(function (text) {
        if (!response.ok) throw new Error(text || ('HTTP ' + response.status));
        return text;
      });
    }).then(function (html) {
      preview.srcdoc = html;
      lastRendered = payload;
      setStatus('rendered');
    }).catch(function (error) {
      preview.srcdoc = errorPage(error && error.message ? error.message : error);
      setStatus('render error', true);
    });
  }

  function scheduleRender() {
    setStatus('edited');
    if (timer) clearTimeout(timer);
    timer = setTimeout(renderNow, config.debounce);
  }

  function hashFor(path, isSession) {
    return isSession ? '#session:' + encode(path) : '#' + encode(path);
  }

  function afterLoad() {
    source.disabled = false;
    download.disabled = false;
    updateHighlight();
    lastRendered = null;
    history.replaceState(null, '', hashFor(current, currentIsSession));
    renderNow();
  }

  function load(path, isSession) {
    if (!path) return;
    if (isSession && !Object.prototype.hasOwnProperty.call(session, path)) {
      setStatus('the session file is gone', true);
      return;
    }
    // Opening a file switches to the tab of its type.
    var type = typeForPath(path);
    if (type && type !== activeType) setType(type, true);
    current = path;
    currentIsSession = !!isSession;
    markActive(path, currentIsSession);
    setPath(path, currentIsSession);
    if (currentIsSession) {
      setStatus('loading…');
      source.value = session[path];
      afterLoad();
      return;
    }
    setStatus('loading…');
    fetch('/api/source?path=' + encode(path)).then(function (response) {
      if (!response.ok) throw new Error('HTTP ' + response.status);
      return response.text();
    }).then(function (text) {
      source.value = text;
      afterLoad();
    }).catch(function (error) {
      setStatus('cannot load: ' + error.message, true);
    });
  }

  function pathFromHash() {
    var raw = location.hash.replace(/^#/, '');
    if (!raw) return null;
    if (raw.indexOf('session:') === 0) {
      var rest = raw.slice(8);
      try { return { path: decodeURIComponent(rest), isSession: true }; }
      catch (error) { return { path: rest, isSession: true }; }
    }
    try { return { path: decodeURIComponent(raw), isSession: false }; }
    catch (error) { return { path: raw, isSession: false }; }
  }

  function filenameFromDisposition(header, fallback) {
    if (!header) return fallback;
    var match = /filename="?([^";]+)"?/.exec(header);
    return match ? match[1] : fallback;
  }

  function fallbackName(path, format) {
    var stem = (path.split('/').pop() || 'diagram').replace(/\.[^.]*$/, '');
    return stem + '.' + format;
  }

  // What the preview shows right now: the open diagram and, for a tree, the
  // branches that are folded. The export reproduces it, so the downloaded
  // file looks like the screen. The preview is a same-origin `srcdoc` frame;
  // when it cannot be read (or shows an error page) the export is the plain
  // render of the source.
  function viewBody() {
    var parts = [];
    try {
      var doc = preview.contentDocument;
      if (!doc) return '';
      var active = doc.querySelector('.swimflow-diagram.is-active');
      if (active) {
        parts.push('view:active=' + encodeURIComponent(active.getAttribute('data-diagram') || ''));
      }
      var sections = doc.querySelectorAll('.swimflow-diagram');
      for (var i = 0; i < sections.length; i++) {
        var nodes = sections[i].querySelectorAll('.tree-node');
        if (!nodes.length) continue;
        var folded = [];
        for (var j = 0; j < nodes.length; j++) {
          if (nodes[j].getAttribute('data-collapsed') === '1') {
            folded.push((nodes[j].getAttribute('data-id') || '').slice(1));
          }
        }
        parts.push('view:collapsed:' + encodeURIComponent(sections[i].getAttribute('data-diagram') || '') +
          '=' + folded.join(','));
      }
      // A manifest keeps what is hidden on its section (see
      // src/plugin/manifest/runtime.js); the column, row or cell filter becomes `only:cN` / `only:rN` (both for a cell).
      for (var k = 0; k < sections.length; k++) {
        var manifest = sections[k].querySelector('.swimflow-manifest');
        if (!manifest) continue;
        // Before the first click the manifest shows its defaults (`hidemarks`).
        var hidden = sections[k].getAttribute('data-manifest-hidden');
        if (hidden === null) hidden = manifest.getAttribute('data-hidden-default') || '';
        var tokens = hidden.split(' ').filter(Boolean);
        var only = sections[k].getAttribute('data-manifest-filter');
        if (only) {
          var filters = only.split(' ').filter(Boolean);
          for (var f = 0; f < filters.length; f++) tokens.push('only:' + filters[f]);
        }
        parts.push('view:hidden:' + encodeURIComponent(sections[k].getAttribute('data-diagram') || '') +
          '=' + encodeURIComponent(tokens.join(',')));
      }
    } catch (error) {
      return '';
    }
    return parts.length ? '&' + parts.join('&') : '';
  }

  function downloadExport(format) {
    if (!current) return;
    setStatus('exporting ' + format + '…');
    fetch('/api/export?path=' + encode(current) + '&format=' + encode(format) +
      '&type=' + encode(currentType()), {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded; charset=utf-8' },
      body: requestBody() + viewBody()
    }).then(function (response) {
      if (!response.ok) {
        return response.text().then(function (text) {
          throw new Error(text || ('HTTP ' + response.status));
        });
      }
      var name = filenameFromDisposition(
        response.headers.get('Content-Disposition'),
        fallbackName(current, format)
      );
      return response.blob().then(function (blob) { return { blob: blob, name: name }; });
    }).then(function (result) {
      var url = URL.createObjectURL(result.blob);
      var anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = result.name;
      document.body.appendChild(anchor);
      anchor.click();
      document.body.removeChild(anchor);
      setTimeout(function () { URL.revokeObjectURL(url); }, 0);
      setStatus('exported ' + result.name);
    }).catch(function (error) {
      setStatus('export error: ' + (error && error.message ? error.message : error), true);
    });
  }

  // --- session buffers ---------------------------------------------------

  function persistSession() {
    window.swimflowSession.save(session, storage);
  }

  // The session files of the active type; the other types have their own tab.
  function sessionPaths() {
    var info = activeInfo();
    var paths = window.swimflowSession.list(session);
    if (!info) return [];
    var out = [];
    for (var i = 0; i < paths.length; i++) {
      if (window.swimflowSession.matchesExtension(paths[i], info.extensions)) out.push(paths[i]);
    }
    return out;
  }

  function renderSession() {
    var paths = sessionPaths();
    sessionBox.hidden = paths.length === 0;
    sessionTree.innerHTML = '';
    for (var i = 0; i < paths.length; i++) {
      var path = paths[i];
      var item = document.createElement('li');
      item.className = 'file session-file';

      var link = document.createElement('a');
      link.href = '#';
      link.setAttribute('data-session', path);
      link.textContent = path;
      link.title = path + ' (double-click to rename)';

      var remove = document.createElement('button');
      remove.type = 'button';
      remove.className = 'remove';
      remove.setAttribute('data-remove', path);
      remove.title = 'Remove from this session';
      remove.setAttribute('aria-label', 'Remove ' + path + ' from this session');
      remove.textContent = '×';

      item.appendChild(link);
      item.appendChild(remove);
      sessionTree.appendChild(item);
    }
  }

  function readFile(file) {
    return new Promise(function (resolve, reject) {
      var reader = new FileReader();
      reader.onload = function () { resolve(String(reader.result)); };
      reader.onerror = function () { reject(reader.error || new Error('cannot read file')); };
      reader.readAsText(file);
    });
  }

  function importFiles(fileList) {
    var info = activeInfo();
    var entries = [];
    for (var i = 0; i < fileList.length; i++) {
      var file = fileList[i];
      var path = file.webkitRelativePath || file.name;
      if (!window.swimflowSession.matchesExtension(path, info.extensions)) continue;
      entries.push({ path: path, file: file });
    }
    if (!entries.length) {
      setStatus('no ' + (info ? '.' + info.source : 'diagram') + ' files selected', true);
      return;
    }
    var done = 0;
    var failed = 0;
    var first = null;
    function finish() {
      done++;
      if (done < entries.length) return;
      persistSession();
      renderSession();
      var imported = entries.length - failed;
      var message = 'imported ' + imported + ' file' + (imported === 1 ? '' : 's');
      if (failed) message += ', ' + failed + ' unreadable';
      setStatus(message, failed > 0);
      // Open the first imported file when nothing is open yet.
      if (!current && first) load(first, true);
    }
    entries.forEach(function (entry) {
      readFile(entry.file).then(function (text) {
        window.swimflowSession.add(session, entry.path, text);
        if (!first) first = entry.path;
        finish();
      }).catch(function () {
        failed++;
        finish();
      });
    });
  }

  // --- events ------------------------------------------------------------

  // The type tabs.
  typeTabs.addEventListener('click', function (event) {
    var target = event.target;
    var tab = target && target.closest ? target.closest('.type-tab') : null;
    if (!tab) return;
    event.preventDefault();
    var name = tab.getAttribute('data-type');
    if (!name || name === activeType) return;
    setType(name);
    // Open the first file of the new type, if there is one.
    var panel = treePanel(name);
    var first = panel ? panel.querySelector('a[data-path]') : null;
    if (first) load(first.getAttribute('data-path'), false);
  });

  typeTabs.addEventListener('keydown', function (event) {
    if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return;
    var tabs = typeTabs.querySelectorAll('.type-tab');
    if (!tabs.length) return;
    var index = 0;
    for (var i = 0; i < tabs.length; i++) {
      if (tabs[i].getAttribute('data-type') === activeType) index = i;
    }
    var next = event.key === 'ArrowRight' ? (index + 1) % tabs.length : (index - 1 + tabs.length) % tabs.length;
    event.preventDefault();
    tabs[next].focus();
    tabs[next].dispatchEvent(new MouseEvent('click'));
  });

  document.getElementById('tree').addEventListener('click', function (event) {
    var anchor = event.target && event.target.closest ? event.target.closest('a[data-path]') : null;
    if (!anchor) return;
    event.preventDefault();
    load(anchor.getAttribute('data-path'), false);
  });

  sessionTree.addEventListener('click', function (event) {
    var target = event.target;
    if (!target || !target.closest) return;
    var remove = target.closest('button[data-remove]');
    if (remove) {
      event.preventDefault();
      var removed = remove.getAttribute('data-remove');
      window.swimflowSession.remove(session, removed);
      persistSession();
      renderSession();
      if (currentIsSession && current === removed) {
        current = null;
        currentIsSession = false;
      }
      setStatus('removed ' + removed);
      return;
    }
    var link = target.closest('a[data-session]');
    if (!link) return;
    event.preventDefault();
    load(link.getAttribute('data-session'), true);
  });

  sessionTree.addEventListener('dblclick', function (event) {
    var target = event.target;
    if (!target || !target.closest) return;
    var link = target.closest('a[data-session]');
    if (!link) return;
    var path = link.getAttribute('data-session');
    var next = window.prompt('Rename file', path);
    if (next === null) return;
    next = next.trim();
    if (!next) return;
    var info = activeInfo();
    if (!window.swimflowSession.matchesExtension(next, info.extensions)) {
      setStatus('keep the .' + info.source + ' extension', true);
      return;
    }
    var renamed = window.swimflowSession.rename(session, path, next);
    if (!renamed) {
      setStatus('cannot rename to ' + next, true);
      return;
    }
    persistSession();
    renderSession();
    if (currentIsSession && current === path) load(renamed, true);
  });

  newButton.addEventListener('click', function () {
    var info = activeInfo();
    var path = window.swimflowSession.create(session, info.template, 'untitled.' + info.source);
    persistSession();
    renderSession();
    load(path, true);
  });

  importInput.addEventListener('change', function () {
    importFiles(importInput.files);
    importInput.value = '';
  });

  importFolderInput.addEventListener('change', function () {
    importFiles(importFolderInput.files);
    importFolderInput.value = '';
  });

  function filterItems(container, needle) {
    var items = container.querySelectorAll('li.file');
    for (var i = 0; i < items.length; i++) {
      var match = !needle || items[i].textContent.toLowerCase().indexOf(needle) >= 0;
      items[i].style.display = match ? '' : 'none';
    }
  }

  function applyFilter() {
    var needle = filter.value.toLowerCase();
    var panel = treePanel(activeType);
    if (panel) filterItems(panel, needle);
    filterItems(sessionTree, needle);
  }

  filter.addEventListener('input', applyFilter);

  source.addEventListener('input', function () {
    updateHighlight();
    if (currentIsSession && current) {
      // Keep the session copy in sync so exports and references see the edit.
      session[current] = source.value;
      persistSession();
    }
    scheduleRender();
  });
  source.addEventListener('scroll', syncHighlightScroll);
  download.addEventListener('click', function () { downloadExport(formatSelect.value); });

  divider.addEventListener('pointerdown', function (event) {
    if (event.pointerType === 'mouse' && event.button !== 0) return;
    event.preventDefault();
    document.body.classList.add('splitting');
    try { divider.setPointerCapture(event.pointerId); } catch (error) { /* ignore */ }
    setSplit((event.clientX - panes.getBoundingClientRect().left) / panes.getBoundingClientRect().width);
  });

  divider.addEventListener('pointermove', function (event) {
    if (!document.body.classList.contains('splitting')) return;
    var rect = panes.getBoundingClientRect();
    if (!rect.width) return;
    setSplit((event.clientX - rect.left) / rect.width);
  });

  function endSplit() {
    document.body.classList.remove('splitting');
    saveSplit();
  }

  divider.addEventListener('pointerup', endSplit);
  divider.addEventListener('pointercancel', endSplit);
  divider.addEventListener('lostpointercapture', endSplit);
  divider.addEventListener('dblclick', function () { setSplit(DEFAULT_SPLIT); });

  divider.addEventListener('keydown', function (event) {
    var step = event.shiftKey ? 0.1 : 0.02;
    var next;
    if (event.key === 'ArrowLeft') next = split - step;
    else if (event.key === 'ArrowRight') next = split + step;
    else if (event.key === 'Home') next = 0.15;
    else if (event.key === 'End') next = 0.85;
    else if (event.key === 'Enter' || event.key === ' ') next = DEFAULT_SPLIT;
    else return;
    event.preventDefault();
    setSplit(next);
  });

  // The file list can be hidden so the editor and preview get the width.
  var sidebarToggle = document.getElementById('sidebar-toggle');
  var SIDEBAR_KEY = 'swimflow.sidebar';
  function setSidebar(visible) {
    if (!sidebarToggle || !document.body || !document.body.classList) return;
    document.body.classList.toggle('no-sidebar', !visible);
    sidebarToggle.setAttribute('aria-expanded', visible ? 'true' : 'false');
    try { localStorage.setItem(SIDEBAR_KEY, visible ? 'shown' : 'hidden'); } catch (error) { /* ignore */ }
  }
  function toggleSidebar() {
    setSidebar(document.body.classList.contains('no-sidebar'));
  }
  if (sidebarToggle) {
    var sidebarStored = null;
    try { sidebarStored = localStorage.getItem(SIDEBAR_KEY); } catch (error) { /* ignore */ }
    if (sidebarStored === 'hidden') setSidebar(false);
    sidebarToggle.addEventListener('click', toggleSidebar);
  }

  // The preview can fill the window below the bar, so a large diagram gets
  // the whole viewport without hiding the controls.
  var PREVIEW_KEY = 'swimflow.preview';
  function setPreviewExpanded(expanded) {
    if (!previewToggle || !document.body || !document.body.classList) return;
    document.body.classList.toggle('preview-expanded', expanded);
    previewToggle.setAttribute('aria-pressed', expanded ? 'true' : 'false');
    previewToggle.setAttribute('aria-label', expanded ? 'Restore preview' : 'Expand preview');
    previewToggle.title = expanded
      ? 'Restore the editor and the rendered preview'
      : 'Expand the rendered preview to the window';
    previewToggle.textContent = expanded ? 'Restore' : 'Expand';
    try { localStorage.setItem(PREVIEW_KEY, expanded ? 'expanded' : 'split'); } catch (error) { /* ignore */ }
  }
  function togglePreviewExpanded() {
    setPreviewExpanded(!document.body.classList.contains('preview-expanded'));
  }
  if (previewToggle) {
    var previewStored = null;
    try { previewStored = localStorage.getItem(PREVIEW_KEY); } catch (error) { /* ignore */ }
    if (previewStored === 'expanded') setPreviewExpanded(true);
    previewToggle.addEventListener('click', togglePreviewExpanded);
  }

  document.addEventListener('keydown', function (event) {
    if (event.key === 'Escape' && previewToggle &&
        document.body.classList.contains('preview-expanded')) {
      event.preventDefault();
      setPreviewExpanded(false);
    } else if (sidebarToggle && (event.ctrlKey || event.metaKey) && event.key === 'b') {
      event.preventDefault();
      toggleSidebar();
    } else if ((event.ctrlKey || event.metaKey) && event.key === 's') {
      // The server never writes files; export the current source instead.
      // Ctrl+S always downloads the editable source, regardless of the picker.
      event.preventDefault();
      downloadExport(sourceFormat());
    } else if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
      event.preventDefault();
      renderNow();
    } else if (event.key === 'Tab' && event.target === source) {
      event.preventDefault();
      var start = source.selectionStart;
      var end = source.selectionEnd;
      source.value = source.value.slice(0, start) + '  ' + source.value.slice(end);
      source.selectionStart = source.selectionEnd = start + 2;
      updateHighlight();
      scheduleRender();
    }
  });

  window.addEventListener('hashchange', function () {
    var target = pathFromHash();
    if (target) load(target.path, target.isSession);
  });
  window.addEventListener('resize', syncHighlightPadding);

  // --- start-up ----------------------------------------------------------

  setType(activeType, true);
  setPath(null);
  renderSession();

  var initial = pathFromHash();
  if (initial) {
    load(initial.path, initial.isSession);
  } else {
    // Open the first file of the first type that has one.
    var opened = false;
    for (var t = 0; t < config.types.length && !opened; t++) {
      var name = config.types[t].name;
      if (name !== activeType) setType(name, true);
      var panel = treePanel(name);
      var first = panel ? panel.querySelector('a[data-path]') : null;
      if (first) {
        load(first.getAttribute('data-path'), false);
        opened = true;
      }
    }
    if (!opened) {
      setType(activeType, true);
      var buffers = sessionPaths();
      if (buffers.length) load(buffers[0], true);
    }
  }
})();
</script>
</script>
</body>
</html>
"##;

/// Render the type tabs of the sidebar.
///
/// One tab per diagram type; the first one is selected by default. The
/// client switches the file tree, the syntax highlighting and the export
/// formats when a tab is chosen.
pub fn render_tabs(types: &[TypeInfo]) -> String {
    let mut out = String::new();
    for (index, info) in types.iter().enumerate() {
        let selected = if index == 0 { "true" } else { "false" };
        out.push_str(&format!(
            "<button type=\"button\" class=\"type-tab\" role=\"tab\" \
             id=\"type-tab-{name}\" aria-controls=\"tree-{name}\" data-type=\"{name}\" \
             aria-selected=\"{selected}\" title=\"{title}\">{label}</button>",
            name = html_escape(info.name),
            selected = selected,
            title = html_escape(info.description),
            label = html_escape(&capitalize(info.name)),
        ));
    }
    out
}

/// Capitalize the first letter of a type name, for the tab label.
fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Escape text for inclusion in HTML text and attribute values.
fn html_escape(value: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::Registry;

    #[test]
    fn the_tabs_list_every_type() {
        let types = Registry::builtin().types();
        let html = render_tabs(&types);
        assert!(html.contains("data-type=\"activity\""), "{html}");
        assert!(html.contains("data-type=\"sequence\""), "{html}");
        assert!(html.contains("data-type=\"tree\""), "{html}");
        assert!(html.contains("data-type=\"treemap\""), "{html}");
        assert!(html.contains("data-type=\"manifest\""), "{html}");
        assert!(html.contains("aria-selected=\"true\""), "{html}");
        assert!(html.contains("aria-controls=\"tree-activity\""), "{html}");
        assert!(html.contains(">Activity</button>"), "{html}");
        assert!(html.contains(">Sequence</button>"), "{html}");
        assert!(html.contains(">Tree</button>"), "{html}");
        assert!(html.contains(">Treemap</button>"), "{html}");
        assert!(html.contains(">Manifest</button>"), "{html}");
        // The tab bar is for navigation; the file extensions only add noise.
        assert!(!html.contains("type-ext"), "{html}");
        assert!(!html.contains(".diag"), "{html}");
        assert!(!html.contains(".seq"), "{html}");
        assert!(!html.contains(".tmap"), "{html}");
        assert!(!html.contains(".mfst"), "{html}");
    }
}
