'use strict';
// Regression harness for the live-preview session UI.
//
// The app shell's main script is inlined in the page (see
// `src/server/web.rs`). This harness runs it against a minimal DOM stub and
// drives the session workflow: create a new diagram, import files, remove one
// again. It catches wiring mistakes (a missing element, a wrong id, a
// handler that throws) that a structural check cannot see.
//
// Usage: node tests/session_ui_runtime.js <shell.html>

const fs = require('fs');
const vm = require('vm');

const { bodyOf } = require('./shell_body.js');

const path = process.argv[2];
if (!path) {
  console.error('usage: node session_ui_runtime.js <shell.html>');
  process.exit(2);
}

const html = bodyOf(fs.readFileSync(path, 'utf8'));
const scripts = Array.from(html.matchAll(/<script>([\s\S]*?)<\/script>/g), (m) => m[1]);
const appScript = scripts.find((script) => script.includes('swimflowSession') && script.includes('renderNow'));
if (!appScript) {
  console.error('could not find the application script');
  process.exit(2);
}

// --- a very small DOM ------------------------------------------------------

class ClassList {
  constructor() { this.set = new Set(); }
  toggle(name, force) {
    const on = force === undefined ? !this.set.has(name) : !!force;
    if (on) this.set.add(name); else this.set.delete(name);
    return on;
  }
  contains(name) { return this.set.has(name); }
  add(name) { this.set.add(name); }
  remove(name) { this.set.delete(name); }
}

class Element {
  constructor(tag, id) {
    this.tagName = tag.toUpperCase();
    this.id = id || '';
    this.children = [];
    this.attributes = {};
    this.style = { setProperty() {} };
    this.classList = new ClassList();
    this.listeners = {};
    this.textContent = '';
    this._innerHTML = '';
    this.value = '';
    this.disabled = false;
    this.hidden = false;
    this.files = [];
    this.parent = null;
  }
  get innerHTML() { return this._innerHTML; }
  set innerHTML(value) {
    this._innerHTML = value;
    if (value === '') this.children = [];
  }
  appendChild(child) { child.parent = this; this.children.push(child); return child; }
  setAttribute(name, value) { this.attributes[name] = String(value); }
  getAttribute(name) { return name in this.attributes ? this.attributes[name] : null; }
  addEventListener(type, fn) { (this.listeners[type] = this.listeners[type] || []).push(fn); }
  // Dispatch an event that bubbles up to the ancestors, like the browser does
  // for the delegated listeners of the tree and the session list.
  dispatch(type) {
    const event = { type, target: this, preventDefault() {} };
    let node = this;
    while (node) {
      for (const fn of node.listeners[type] || []) fn(event);
      node = node.parent;
    }
  }
  closest(selector) {
    const matches = (el) => {
      if (selector === 'a[data-path]') return el.tagName === 'A' && el.getAttribute('data-path') !== null;
      if (selector === 'a[data-session]') return el.tagName === 'A' && el.getAttribute('data-session') !== null;
      if (selector === 'button[data-remove]') return el.tagName === 'BUTTON' && el.getAttribute('data-remove') !== null;
      if (selector === '.type-tab') return el.classList.contains('type-tab');
      return false;
    };
    let node = this;
    while (node) { if (matches(node)) return node; node = node.parent; }
    return null;
  }
  querySelectorAll(selector) {
    const out = [];
    const walk = (el) => {
      for (const child of el.children) {
        if (selector === 'a[data-path]' && child.tagName === 'A' && child.getAttribute('data-path') !== null) out.push(child);
        if (selector === 'a[data-session]' && child.tagName === 'A' && child.getAttribute('data-session') !== null) out.push(child);
        if (selector === 'li.file' && child.classList.contains('file')) out.push(child);
        if (selector === '.type-tab' && child.classList.contains('type-tab')) out.push(child);
        walk(child);
      }
    };
    walk(this);
    return out;
  }
  querySelector(selector) { return this.querySelectorAll(selector)[0] || null; }
}

const ids = ['tree', 'session-tree', 'session', 'filter', 'source', 'highlight', 'preview',
  'status', 'path', 'download', 'format', 'format-source', 'new', 'import', 'import-folder',
  'divider', 'preview-toggle', 'type-tabs', 'tree-activity', 'tree-sequence'];
const elements = {};
for (const id of ids) elements[id] = new Element('div', id);
elements.panes = new Element('main', 'panes');
elements.tree = new Element('nav', 'tree');
elements['type-tabs'] = new Element('div', 'type-tabs');
elements['tree-activity'] = new Element('nav', 'tree-activity');
elements['tree-sequence'] = new Element('nav', 'tree-sequence');

// The served tree holds one diagram per type.
function treeLink(panel, path) {
  const link = new Element('a');
  link.setAttribute('data-path', path);
  link.textContent = path;
  panel.appendChild(link);
}
treeLink(elements['tree-activity'], 'hello.diag');
treeLink(elements['tree-sequence'], 'chat.seq');

// The type tabs.
for (const type of ['activity', 'sequence']) {
  const tab = new Element('button');
  tab.classList.add('type-tab');
  tab.setAttribute('data-type', type);
  tab.setAttribute('aria-selected', type === 'activity' ? 'true' : 'false');
  elements['type-tabs'].appendChild(tab);
}

const document = {
  getElementById: (id) => elements[id] || null,
  querySelector: (selector) => (selector === '.panes' ? elements.panes : null),
  querySelectorAll: () => [],
  createElement: (tag) => new Element(tag),
  addEventListener: () => {},
  body: new Element('body'),
};

function storage() {
  const data = new Map();
  return {
    getItem: (key) => (data.has(key) ? data.get(key) : null),
    setItem: (key, value) => data.set(key, String(value)),
    removeItem: (key) => data.delete(key),
  };
}

const calls = [];
const sandbox = {
  console,
  document,
  history: { replaceState() {} },
  location: { hash: '' },
  localStorage: storage(),
  sessionStorage: storage(),
  setTimeout: (fn) => { fn(); return 0; },
  clearTimeout: () => {},
  URL: { createObjectURL: () => 'blob:x', revokeObjectURL: () => {} },
  Promise,
  addEventListener: () => {},
  MouseEvent: class {
    constructor(type) { this.type = type; }
  },
  FileReader: class {
    readAsText(file) {
      this.result = file.content;
      if (this.onload) this.onload({ target: this });
    }
  },
  fetch: (url, options) => {
    calls.push({ url, options });
    return Promise.resolve({
      ok: true,
      status: 200,
      text: () => Promise.resolve('<html>ok</html>'),
      blob: () => Promise.resolve({}),
      headers: { get: () => null },
    });
  },
};
sandbox.window = sandbox;
sandbox.window.prompt = () => 'renamed.diag';
vm.createContext(sandbox);

let failures = 0;
function check(condition, message) {
  if (!condition) {
    failures += 1;
    console.error('FAIL: ' + message);
  }
}

for (const script of scripts) {
  try {
    vm.runInContext(script, sandbox);
  } catch (error) {
    console.error('FAIL: the shell script threw: ' + error);
    process.exit(1);
  }
}

const session = sandbox.window.swimflowSession;
check(!!session, 'the session module is exposed');

const names = () => elements['session-tree'].children.map((li) => {
  const link = li.children.find((c) => c.tagName === 'A');
  return link ? link.getAttribute('data-session') : null;
});

// The first diagram of the served tree is loaded on start-up.
check(
  calls.some((call) => call.url === '/api/source?path=hello.diag'),
  'the first file is loaded: ' + calls.map((c) => c.url).join(', ')
);

// --- expand the preview ---------------------------------------------------
elements['preview-toggle'].dispatch('click');
check(
  document.body.classList.contains('preview-expanded'),
  'the preview expands below the bar'
);
check(
  elements['preview-toggle'].getAttribute('aria-pressed') === 'true',
  'the expanded preview toggle is pressed'
);
check(
  elements['preview-toggle'].textContent === 'Restore',
  'the expanded preview toggle offers to restore: ' + elements['preview-toggle'].textContent
);
elements['preview-toggle'].dispatch('click');
check(
  !document.body.classList.contains('preview-expanded'),
  'the preview collapses again'
);

// --- create ---------------------------------------------------------------
elements.new.dispatch('click');
check(names().length === 1, 'a new file appears in the session list');
check(names()[0] === 'untitled.diag', 'the new file is named untitled.diag: ' + names()[0]);
const render = calls[calls.length - 1];
check(render.url === '/api/render?path=untitled.diag&type=activity',
  'the new file is rendered: ' + render.url);
check(
  render.options.headers['Content-Type'].indexOf('x-www-form-urlencoded') >= 0,
  'the payload is form encoded'
);
check(render.options.body.indexOf('source=') === 0, 'the payload carries the source');

// --- import ---------------------------------------------------------------
// Reading a file is asynchronous, so the import is awaited before checking.
async function main() {
  elements['import'].files = [
    { name: 'imported.diag', webkitRelativePath: '', content: 'diagram x { A; }' },
    { name: 'notes.txt', webkitRelativePath: '', content: 'ignored' },
  ];
  elements['import'].dispatch('change');
  await new Promise((resolve) => setImmediate(resolve));

  check(names().length === 2, 'the imported file is added: ' + names().join(', '));
  check(names().indexOf('imported.diag') >= 0, 'the imported file is listed');
  check(names().indexOf('notes.txt') < 0, 'a file that is not a diagram is skipped');

  // The imported file is sent along, so cross references resolve.
  const withFiles = calls[calls.length - 1];
  check(
    withFiles.options.body.indexOf('file:imported.diag=') > 0,
    'the session file is sent with the request'
  );

  // --- the type tabs ------------------------------------------------------
  const tabs = elements['type-tabs'].children;
  const sequenceTab = tabs.find((tab) => tab.getAttribute('data-type') === 'sequence');
  check(!!sequenceTab, 'the shell offers a tab per diagram type');
  sequenceTab.dispatch('click');
  check(
    calls.some((call) => call.url === '/api/source?path=chat.seq'),
    'the sequence file of the new tab is loaded: ' + calls.map((c) => c.url).join(', ')
  );
  check(
    elements['format-source'].value === 'seq',
    'the source export follows the type: ' + elements['format-source'].value
  );
  check(
    elements['import'].getAttribute('accept').indexOf('.seq') >= 0,
    'the import accepts the files of the type'
  );
  check(
    tabs.find((tab) => tab.getAttribute('data-type') === 'sequence').getAttribute('aria-selected') === 'true',
    'the sequence tab is selected'
  );

  // A new file follows the type: the sequence tab creates a `.seq` buffer
  // from the sequence template.
  elements.new.dispatch('click');
  const sessionNames = elements['session-tree'].children.map((li) => {
    const link = li.children.find((c) => c.tagName === 'A');
    return link ? link.getAttribute('data-session') : null;
  });
  check(sessionNames.indexOf('untitled.seq') >= 0,
    'the new file is a sequence diagram: ' + sessionNames.join(', '));
  check(
    elements['session-tree'].children.some((li) =>
      li.children.some((c) => c.tagName === 'A' && c.getAttribute('data-session') === 'untitled.seq')),
    'the sequence buffer is listed in the session'
  );
  check(
    String(elements['session-tree'].children[0].children[0].textContent || '').indexOf('untitled.seq') >= 0,
    'the sequence buffer is named after the type'
  );
  const sequenceRender = calls[calls.length - 1];
  check(sequenceRender.url === '/api/render?path=untitled.seq&type=sequence',
    'the sequence buffer is rendered as a sequence diagram: ' + sequenceRender.url);
  check(decodeURIComponent(sequenceRender.options.body).indexOf('sequence') >= 0,
    'the new buffer starts from the sequence template');

  // --- remove -------------------------------------------------------------
  // Back to the activity tab: the session list of the other type is hidden.
  tabs.find((tab) => tab.getAttribute('data-type') === 'activity').dispatch('click');
  const remove = elements['session-tree'].children
    .map((li) => li.children.find((c) => c.getAttribute('data-remove') === 'imported.diag'))
    .find(Boolean);
  check(!!remove, 'the imported file has a remove button');
  remove.dispatch('click');
  check(names().length === 1, 'the file is removed again: ' + names().join(', '));
  check(names().indexOf('imported.diag') < 0, 'the removed file is gone');

  if (failures > 0) {
    process.exit(1);
  }
  console.log('session UI runtime OK');
}

main().catch((error) => {
  console.error('FAIL: ' + (error && error.stack ? error.stack : error));
  process.exit(1);
});
