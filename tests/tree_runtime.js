'use strict';
// Parity harness for the tree runtime (src/plugin/tree/runtime.js).
//
// The runtime lays a tree out again in the browser when a branch is folded or
// unfolded; the Rust layout (src/plugin/tree/layout.rs) draws the initial
// state. Both must agree to the pixel, or the diagram would jump on the first
// click. This harness reads the geometry out of a rendered SVG and compares it
// with what the runtime computes for the same state, and for the state after
// folding a node.
//
// Usage: node tests/tree_runtime.js <state.svg> [<node label>=<folded.svg> ...]
//
// For every pair, <folded.svg> is the Rust rendering of the same tree with
// that node collapsed; the runtime must reproduce it from <state.svg>.

const fs = require('fs');
const path = require('path');

const [, , statePath, ...pairs] = process.argv;
if (!statePath) {
  console.error('usage: node tree_runtime.js <state.svg> [label=folded.svg ...]');
  process.exit(2);
}

// The runtime exports its layout function through `globalThis` when there is
// no document.
require(path.join(__dirname, '..', 'src', 'plugin', 'tree', 'runtime.js'));
const { layoutTree } = globalThis.swimflowTree;

function attrsOf(tag) {
  const out = {};
  for (const match of tag.matchAll(/([\w:-]+)="([^"]*)"/g)) out[match[1]] = match[2];
  return out;
}

// Read the geometry the Rust renderer wrote into an SVG.
function parse(svg) {
  const root = attrsOf(svg.match(/<svg [^>]*>/)[0]);
  const group = attrsOf(svg.match(/<g class="swimflow-tree"[^>]*>/)[0]);
  const nodes = [];
  for (const match of svg.matchAll(/<g class="tree-node"[^>]*>/g)) {
    const attrs = attrsOf(match[0]);
    const after = svg.slice(match.index + match[0].length);
    const rect = attrsOf(after.match(/<rect [^>]*>/)[0]);
    const text = (after.match(/<text [^>]*>([^<]*)<\/text>/) || [])[1];
    nodes.push({ attrs, rect, text });
  }
  const find = (cls, id) => {
    const re = new RegExp(`<(?:path|g) class="${cls}" data-for="${id}"[^>]*>`);
    const found = svg.match(re);
    return found ? attrsOf(found[0]) : null;
  };
  return { root, group, nodes, find };
}

function modelOf(parsed, foldedIndex) {
  return parsed.nodes.map((node, index) => ({
    parent: node.attrs['data-parent'] ? parseInt(node.attrs['data-parent'].slice(1), 10) : -1,
    depth: +node.attrs['data-depth'],
    edgeDashed: node.attrs['data-edge-dashed'] === '1',
    w: +node.attrs['data-w'],
    h: +node.attrs['data-h'],
    edgeW: +(node.attrs['data-edge-w'] || 0),
    edgeH: +(node.attrs['data-edge-h'] || 0),
    collapsed: index === foldedIndex ? true : node.attrs['data-collapsed'] === '1',
  }));
}

// A path of several polylines (the stretches of a bus) is a list of lists.
function dsOf(paths) { return (paths || []).map(dOf).join(' '); }

function dOf(points) {
  if (!points || !points.length) return '';
  return points.map((p, i) => (i ? 'L' : 'M') + p[0] + ',' + p[1]).join(' ');
}

let failures = 0;
function expectEqual(what, actual, expected) {
  if (String(actual) !== String(expected)) {
    failures++;
    console.error(`MISMATCH ${what}: runtime ${actual} != rust ${expected}`);
  }
}

// Compare the runtime's layout of `model` with the geometry in `parsed`.
function compare(title, model, parsed) {
  const result = layoutTree(model, parsed.group['data-orientation']);
  expectEqual(`${title} width`, result.width, parsed.root.width);
  expectEqual(`${title} height`, result.height, parsed.root.height);
  parsed.nodes.forEach((node, index) => {
    const id = `n${index}`;
    const where = `${title} ${node.text}`;
    const box = result.boxes[index];
    expectEqual(`${where} x`, box.x1, node.rect.x);
    expectEqual(`${where} y`, box.y1, node.rect.y);
    expectEqual(`${where} w`, box.x2 - box.x1, node.rect.width);
    expectEqual(`${where} h`, box.y2 - box.y1, node.rect.height);
    expectEqual(`${where} visible`, result.visible[index], node.attrs.display !== 'none');

    const trunkStem = parsed.find('tree-stem', id);
    if (trunkStem) {
      const trunk = result.trunks[index];
      expectEqual(`${where} stem`, dOf(trunk.stem), trunkStem.d);
      expectEqual(`${where} stem dashed`, trunk.stemDashed, trunkStem['stroke-dasharray'] !== undefined);
      expectEqual(`${where} bus`, dsOf(trunk.bus), parsed.find('tree-bus', id).d);
      const dashedBus = parsed.find('tree-bus-dashed', id);
      expectEqual(`${where} dashed bus`, dsOf(trunk.busDashed), dashedBus ? dashedBus.d : '');
    }
    const drop = parsed.find('tree-drop', id);
    if (drop) expectEqual(`${where} drop`, dOf(result.drops[index]), drop.d);

    const label = parsed.find('tree-label', id);
    if (label) {
      const box = result.labels[index];
      expectEqual(`${where} label x`, box.x1, label['data-x']);
      expectEqual(`${where} label y`, box.y1, label['data-y']);
    }
    const toggle = parsed.find('tree-toggle', id);
    if (toggle) {
      expectEqual(`${where} toggle x`, result.toggles[index][0], toggle['data-x']);
      expectEqual(`${where} toggle y`, result.toggles[index][1], toggle['data-y']);
    }
  });
}

const state = parse(fs.readFileSync(statePath, 'utf8'));
compare('initial state', modelOf(state), state);

for (const pair of pairs) {
  const split = pair.lastIndexOf('=');
  const label = pair.slice(0, split);
  const folded = parse(fs.readFileSync(pair.slice(split + 1), 'utf8'));
  const index = state.nodes.findIndex((node) => node.text === label);
  if (index < 0) {
    console.error(`no node labelled ${label}`);
    process.exit(2);
  }
  compare(`after folding ${label}`, modelOf(state, index), folded);
}

if (failures) {
  console.error(`${failures} mismatches`);
  process.exit(1);
}
console.log('ok');
