'use strict';
// DOM harness for the treemap value toggle (src/plugin/treemap/runtime.js).
//
// jsdom is not a dependency, so the section is parsed into the small fake DOM
// shared with the tree harnesses. The real runtime is run against it; clicking
// the button must hide only that section's values, keep the SVG geometry
// untouched, and toggle back on a second click.
//
// Usage: node tests/treemap_runtime.js <section.html>

const fs = require('fs');
const path = require('path');
const vm = require('vm');
const { parseMarkup } = require('./tree_fake_dom.js');

const sectionPath = process.argv[2];
if (!sectionPath) {
  console.error('usage: node treemap_runtime.js <section.html>');
  process.exit(2);
}

const source = fs.readFileSync(sectionPath, 'utf8');
const documentRoot = parseMarkup(source);
const section = documentRoot.querySelectorAll('.swimflow-diagram')[0];
if (!section) {
  console.error('the fixture has no .swimflow-diagram section');
  process.exit(2);
}

const slot = documentRoot.querySelector('.swimflow-toolbar-slot');
const listeners = {};
const sandbox = {
  document: {
    addEventListener: (type, fn) => { listeners[type] = fn; },
    getElementById: (id) => (id === 'treemap' ? section : null),
  },
  console,
};
sandbox.globalThis = sandbox;
sandbox.window = sandbox;
vm.createContext(sandbox);
vm.runInContext(
  fs.readFileSync(path.join(__dirname, '..', 'src', 'plugin', 'treemap', 'runtime.js'), 'utf8'),
  sandbox
);

const button = slot ? slot.querySelector('.treemap-values-toggle') : null;
const values = section.querySelectorAll('.treemap-value');
if (!button) { console.error('the section has no value toggle'); process.exit(2); }
if (!values.length) { console.error('the section has no values'); process.exit(2); }

const svg = section.querySelector('svg');
const viewBox = svg.getAttribute('viewBox');
let failures = 0;
function same(what, actual, expected) {
  if (String(actual) !== String(expected)) {
    failures++;
    console.error(`MISMATCH ${what}: ${actual} != ${expected}`);
  }
}

same('initial aria-pressed', button.getAttribute('aria-pressed'), 'true');
for (const value of values) same('initial display', value.getAttribute('display'), null);

let prevented = false;
listeners.click({ target: button, preventDefault() { prevented = true; } });
if (!prevented) { console.error('the click was not handled'); process.exit(1); }

same('hidden aria-pressed', button.getAttribute('aria-pressed'), 'false');
same('hidden button label', button.textContent, 'Show values');
for (const value of values) same('hidden display', value.getAttribute('display'), 'none');
same('viewBox after hiding', svg.getAttribute('viewBox'), viewBox);

listeners.click({ target: button, preventDefault() {} });
same('shown aria-pressed', button.getAttribute('aria-pressed'), 'true');
same('shown button label', button.textContent, 'Hide values');
for (const value of values) same('shown display', value.getAttribute('display'), 'inline');
same('viewBox after showing', svg.getAttribute('viewBox'), viewBox);

if (failures) { console.error(failures + ' mismatches'); process.exit(1); }
console.log('ok');
