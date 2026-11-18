'use strict';
// Regression harness for the bundle navigation runtime.
//
// The runtime is inlined in the generated HTML. Here it is executed with a
// minimal DOM mock and a `history.pushState` that always throws, which is what
// browsers may do inside an `<iframe srcdoc>` (the live-preview server). The
// runtime must still switch the active diagram in that case.
//
// Usage: node tests/bundle_runtime.js <bundle.html>

const fs = require('fs');

const path = process.argv[2];
if (!path) {
  console.error('usage: node bundle_runtime.js <bundle.html>');
  process.exit(2);
}
const html = fs.readFileSync(path, 'utf8');

// The navigation runtime is the inline script that defines `navigate`.
const scripts = Array.from(html.matchAll(/<script>([\s\S]*?)<\/script>/g), (match) => match[1]);
const runtime = scripts.find((script) => script.includes('function navigate'));
if (!runtime) {
  console.error('could not find the navigation runtime script');
  process.exit(2);
}

function makeClassList() {
  const classes = new Set();
  return {
    add(name) { classes.add(name); },
    remove(name) { classes.delete(name); },
    toggle(name, force) {
      const on = force === undefined ? !classes.has(name) : force;
      if (on) classes.add(name);
      else classes.delete(name);
      return on;
    },
    contains(name) { return classes.has(name); },
  };
}

function makeElement(tag) {
  return {
    tagName: tag,
    children: [],
    attributes: {},
    className: '',
    textContent: '',
    disabled: false,
    classList: makeClassList(),
    appendChild(child) { this.children.push(child); return child; },
    removeChild(child) { this.children = this.children.filter((item) => item !== child); },
    setAttribute(name, value) { this.attributes[name] = String(value); },
    getAttribute(name) { return name in this.attributes ? this.attributes[name] : null; },
    querySelector() { return null; },
    querySelectorAll() { return []; },
    addEventListener() {},
    closest() { return null; },
  };
}

const sections = (html.match(/<section\b[^>]*>/g) || []).map((tag) => {
  const section = makeElement('section');
  const diagram = /data-diagram="([^"]*)"/.exec(tag);
  const label = /data-label="([^"]*)"/.exec(tag);
  if (diagram) section.setAttribute('data-diagram', diagram[1]);
  if (label) section.setAttribute('data-label', label[1]);
  return section;
});

const rootMatch = /<div id="swimflow"[^>]*data-root="([^"]*)"/.exec(html);
const rootKey = rootMatch ? rootMatch[1] : '';

const breadcrumb = makeElement('nav');
const back = makeElement('button');
const container = makeElement('div');
container.setAttribute('data-root', rootKey);
container.querySelector = (selector) => {
  if (selector === '.swimflow-breadcrumb') return breadcrumb;
  if (selector === '.swimflow-back') return back;
  return null;
};

const listeners = {};
const document = {
  documentElement: makeElement('html'),
  getElementById: (id) => (id === 'swimflow' ? container : null),
  querySelector: () => null,
  querySelectorAll: (selector) => (selector === '.swimflow-diagram' ? sections : []),
  createElement: makeElement,
  addEventListener: (type, handler) => { listeners[type] = handler; },
};

const window = { addEventListener() {} };

let hash = '';
const location = {
  get hash() { return hash; },
  set hash(value) { hash = String(value).startsWith('#') ? String(value) : '#' + value; },
};

let pushAttempts = 0;
const history = {
  pushState() {
    pushAttempts += 1;
    throw new Error('SecurityError: history.pushState is not available here');
  },
};

// Execute the inline runtime in this scope so it sees the mocks above.
eval(runtime); // eslint-disable-line no-eval

const click = listeners['click'];
if (typeof click !== 'function') {
  console.error('runtime did not register a click listener');
  process.exit(2);
}

const targetMatch = /data-target="([^"]*)"/.exec(html);
if (!targetMatch) {
  console.error('bundle has no data-target link to activate');
  process.exit(2);
}
const target = targetMatch[1];

let prevented = false;
click({
  target: { closest: () => ({ getAttribute: () => target }) },
  preventDefault: () => { prevented = true; },
});

const active = sections
  .filter((section) => section.classList.contains('is-active'))
  .map((section) => section.getAttribute('data-diagram'));

if (pushAttempts === 0) {
  console.error('runtime never attempted to update the URL');
  process.exit(1);
}
if (!prevented) {
  console.error('runtime did not prevent the default link navigation');
  process.exit(1);
}
if (!active.includes(target)) {
  console.error(`expected ${target} to be active, got ${JSON.stringify(active)}`);
  process.exit(1);
}

console.log(`ok: navigated to ${target} with a failing history.pushState`);
