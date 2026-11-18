'use strict';
// DOM harness for the "Collapse all" and "Expand all" buttons of a tree in an
// HTML bundle (src/plugin/tree/runtime.js).
//
// The section markup (toolbar and SVG) is parsed into the fake DOM and the
// runtime runs for real: a click on "Collapse all" must show the same picture
// as the Rust rendering of the tree with every branch folded, and a click on
// "Expand all" must bring back the picture the section started with.
//
// Usage: node tests/tree_fold_all_runtime.js <section.html> <all-folded.svg>

const fs = require('fs');
const path = require('path');
const vm = require('vm');
const { parseMarkup } = require('./tree_fake_dom.js');

const [, , sectionPath, foldedPath] = process.argv;
if (!foldedPath) {
  console.error('usage: node tree_fold_all_runtime.js <section.html> <all-folded.svg>');
  process.exit(2);
}

const doc = parseMarkup(fs.readFileSync(sectionPath, 'utf8'));
const section = doc.querySelector('.swimflow-diagram');
const slot = doc.querySelector('.swimflow-toolbar-slot');
const svg = section.querySelector('svg');
const collapseButton = slot ? slot.querySelector('.tree-collapse-all') : null;
const expandButton = slot ? slot.querySelector('.tree-expand-all') : null;
if (!collapseButton || !expandButton) {
  console.error('the page bar has no "Collapse all" and "Expand all" buttons');
  process.exit(2);
}

const listeners = {};
const sandbox = {
  document: {
    addEventListener: (type, fn) => { listeners[type] = fn; },
    getElementById: (id) => (id === 'tree' ? section : null),
  },
  console,
};
sandbox.globalThis = sandbox;
vm.createContext(sandbox);
vm.runInContext(
  fs.readFileSync(path.join(__dirname, '..', 'src', 'plugin', 'tree', 'runtime.js'), 'utf8'),
  sandbox
);

function click(button) {
  let prevented = false;
  listeners.click({ target: button, preventDefault() { prevented = true; } });
  if (!prevented) { console.error('the click on ' + button.getAttribute('class') + ' was not handled'); process.exit(1); }
}

let failures = 0;
function same(what, actual, expected) {
  if (String(actual) !== String(expected)) { failures++; console.error(`MISMATCH ${what}: dom ${actual} != rust ${expected}`); }
}
function moved(element, axis) {
  const t = (element.getAttribute('transform') || '').match(/translate\((-?\d+),(-?\d+)\)/);
  const delta = t ? +t[axis === 'x' ? 1 : 2] : 0;
  return +element.getAttribute('data-' + axis) + delta;
}
function nodeId(element) {
  return element.getAttribute('data-id') || element.getAttribute('data-for');
}

// The picture the section starts with, kept for the last comparison.
const start = {
  width: svg.getAttribute('width'),
  height: svg.getAttribute('height'),
  nodes: svg.querySelectorAll('.tree-node').map((node) => [nodeId(node), node.getAttribute('display'), moved(node, 'x'), moved(node, 'y')]),
};

// ---- Collapse all: every branch folded, as the Rust renderer draws it -----

click(collapseButton);
const folded = parseMarkup(fs.readFileSync(foldedPath, 'utf8')).querySelector('svg');
same('collapsed width', svg.getAttribute('width'), folded.getAttribute('width'));
same('collapsed height', svg.getAttribute('height'), folded.getAttribute('height'));

const foldedNodes = folded.querySelectorAll('.tree-node');
svg.querySelectorAll('.tree-node').forEach((element, i) => {
  const id = nodeId(element);
  same(`collapsed node ${id} display`, element.getAttribute('display'), foldedNodes[i].getAttribute('display'));
  if (foldedNodes[i].getAttribute('display') !== 'none') {
    same(`collapsed node ${id} x`, moved(element, 'x'), +foldedNodes[i].getAttribute('data-x'));
    same(`collapsed node ${id} y`, moved(element, 'y'), +foldedNodes[i].getAttribute('data-y'));
  }
});
svg.querySelectorAll('.tree-toggle').forEach((toggle) => {
  same(`collapsed toggle ${nodeId(toggle)} label`, toggle.getAttribute('aria-label'), 'Expand');
});
// Every node with a circle is folded, including the ones hidden in a branch.
svg.querySelectorAll('.tree-toggle').forEach((toggle) => {
  const node = svg.querySelector(`.tree-node[data-id="${nodeId(toggle)}"]`);
  same(`collapsed node ${nodeId(toggle)} state`, node.getAttribute('data-collapsed'), '1');
});

// ---- Expand all: back to the picture the section started with -------------

click(expandButton);
same('expanded width', svg.getAttribute('width'), start.width);
same('expanded height', svg.getAttribute('height'), start.height);
svg.querySelectorAll('.tree-node').forEach((element, i) => {
  const [id, display, x, y] = start.nodes[i];
  same(`expanded node ${id} display`, element.getAttribute('display'), display);
  same(`expanded node ${id} x`, moved(element, 'x'), x);
  same(`expanded node ${id} y`, moved(element, 'y'), y);
  same(`expanded node ${id} state`, element.getAttribute('data-collapsed'), '0');
});
svg.querySelectorAll('.tree-toggle').forEach((toggle) => {
  same(`expanded toggle ${nodeId(toggle)} label`, toggle.getAttribute('aria-label'), 'Collapse');
});

if (failures) { console.error(failures + ' mismatches'); process.exit(1); }
console.log('ok');
