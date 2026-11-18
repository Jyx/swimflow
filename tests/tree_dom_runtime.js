'use strict';
// DOM harness for the tree runtime (src/plugin/tree/runtime.js).
//
// jsdom is not a dependency, so the SVG is parsed into a tiny fake DOM that
// implements exactly what the runtime uses. The runtime is then run for real:
// a click is dispatched on the toggle of a node, and the resulting document
// must show the same picture as the Rust rendering of the tree with that node
// folded (positions, visibility, connectors, circle glyph, canvas size).
//
// Usage: node tests/tree_dom_runtime.js <expanded.svg> <label> <folded.svg>

const fs = require('fs');
const path = require('path');
const vm = require('vm');
const { parseSvg } = require('./tree_fake_dom.js');

const [, , openPath, label, foldedPath] = process.argv;
if (!foldedPath) {
  console.error('usage: node tree_dom_runtime.js <expanded.svg> <label> <folded.svg>');
  process.exit(2);
}

// ---- run the real runtime against the fake DOM ----------------------------

const open = parseSvg(fs.readFileSync(openPath, 'utf8'));
const listeners = {};
const sandbox = {
  document: { addEventListener: (type, fn) => { listeners[type] = fn; } },
  console,
};
sandbox.globalThis = sandbox;
vm.createContext(sandbox);
vm.runInContext(
  fs.readFileSync(path.join(__dirname, '..', 'src', 'plugin', 'tree', 'runtime.js'), 'utf8'),
  sandbox
);

function nodeByLabel(svg, text) {
  return svg.querySelectorAll('.tree-node').find((node) => node.querySelector('text') && textOf(node) === text);
}
// The label text sits in the source order after the node's <text> start tag.
const source = fs.readFileSync(openPath, 'utf8');
function textOf(node) {
  const id = node.getAttribute('data-id');
  const start = source.indexOf(`data-id="${id}"`);
  return (source.slice(start).match(/<text [^>]*>([^<]*)<\/text>/) || [])[1];
}

const target = nodeByLabel(open, label);
if (!target) { console.error('no node labelled ' + label); process.exit(2); }
const toggle = open.querySelectorAll('.tree-toggle').find((t) => t.getAttribute('data-for') === target.getAttribute('data-id'));
if (!toggle) { console.error(label + ' has no toggle'); process.exit(2); }

// A click on the circle inside the toggle bubbles up to the document listener.
const circle = toggle.children[0];
let prevented = false;
listeners.click({ target: circle, preventDefault() { prevented = true; } });
if (!prevented) { console.error('the click was not handled'); process.exit(1); }

// ---- compare with the Rust rendering of the folded state ------------------

const folded = parseSvg(fs.readFileSync(foldedPath, 'utf8'));
let failures = 0;
function same(what, actual, expected) {
  if (String(actual) !== String(expected)) { failures++; console.error(`MISMATCH ${what}: dom ${actual} != rust ${expected}`); }
}
function moved(element, axis) {
  const t = (element.getAttribute('transform') || '').match(/translate\((-?\d+),(-?\d+)\)/);
  const delta = t ? +t[axis === 'x' ? 1 : 2] : 0;
  return +element.getAttribute('data-' + axis) + delta;
}

same('svg width', open.getAttribute('width'), folded.getAttribute('width'));
same('svg height', open.getAttribute('height'), folded.getAttribute('height'));
same('svg viewBox', open.getAttribute('viewBox'), folded.getAttribute('viewBox'));
const bgOpen = open.children.find((c) => c.tagName === 'rect');
const bgFolded = folded.children.find((c) => c.tagName === 'rect');
same('background width', bgOpen.getAttribute('width'), bgFolded.getAttribute('width'));
same('background height', bgOpen.getAttribute('height'), bgFolded.getAttribute('height'));

for (const cls of ['tree-node', 'tree-label', 'tree-toggle']) {
  const a = open.querySelectorAll('.' + cls);
  const b = folded.querySelectorAll('.' + cls);
  same(cls + ' count', a.length, b.length);
  a.forEach((element, i) => {
    const id = element.getAttribute('data-id') || element.getAttribute('data-for');
    same(`${cls} ${id} display`, element.getAttribute('display'), b[i].getAttribute('display'));
    // The folded SVG draws hidden nodes at their (parent's) position too, but
    // only where the node is visible does the position matter.
    if (b[i].getAttribute('display') !== 'none') {
      same(`${cls} ${id} x`, moved(element, 'x'), +b[i].getAttribute('data-x'));
      same(`${cls} ${id} y`, moved(element, 'y'), +b[i].getAttribute('data-y'));
    }
  });
}
for (const cls of ['tree-stem', 'tree-bus', 'tree-drop']) {
  const a = open.querySelectorAll('.' + cls);
  const b = folded.querySelectorAll('.' + cls);
  a.forEach((element, i) => same(`${cls} ${element.getAttribute('data-for')}`, element.getAttribute('d'), b[i].getAttribute('d')));
}
open.querySelectorAll('.tree-toggle').forEach((element, i) => {
  const other = folded.querySelectorAll('.tree-toggle')[i];
  same('aria-label ' + element.getAttribute('data-for'), element.getAttribute('aria-label'), other.getAttribute('aria-label'));
  same('plus display ' + element.getAttribute('data-for'),
    element.querySelector('.tree-plus').getAttribute('display'), other.querySelector('.tree-plus').getAttribute('display'));
});

// A second click unfolds again: the document must be the expanded one, as
// the Rust renderer drew it.
listeners.click({ target: circle, preventDefault() {} });
const reopened = parseSvg(source);
same('reopened width', open.getAttribute('width'), reopened.getAttribute('width'));
same('reopened height', open.getAttribute('height'), reopened.getAttribute('height'));
for (const cls of ['tree-node', 'tree-label', 'tree-toggle']) {
  const a = open.querySelectorAll('.' + cls);
  const b = reopened.querySelectorAll('.' + cls);
  a.forEach((element, i) => {
    const id = element.getAttribute('data-id') || element.getAttribute('data-for');
    same(`reopened ${cls} ${id} display`, element.getAttribute('display'), b[i].getAttribute('display'));
    same(`reopened ${cls} ${id} x`, moved(element, 'x'), +b[i].getAttribute('data-x'));
    same(`reopened ${cls} ${id} y`, moved(element, 'y'), +b[i].getAttribute('data-y'));
  });
}
for (const cls of ['tree-stem', 'tree-bus', 'tree-drop']) {
  open.querySelectorAll('.' + cls).forEach((element, i) => {
    same(`reopened ${cls} ${element.getAttribute('data-for')}`, element.getAttribute('d'),
      reopened.querySelectorAll('.' + cls)[i].getAttribute('d'));
  });
}
same('reopened toggle label', toggle.getAttribute('aria-label'), 'Collapse');

if (failures) { console.error(failures + ' mismatches'); process.exit(1); }
console.log('ok');
