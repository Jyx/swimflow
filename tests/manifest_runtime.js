'use strict';
// DOM harness for the manifest highlight and filter (src/plugin/manifest/runtime.js).
//
// The section is parsed into the small fake DOM shared with the tree
// harnesses and the real runtime is run against it. Hovering a cell marks
// its row and its column; clicking a column header dims the rows that are not
// members of the column, and a second click shows them all again.
//
// Usage: node tests/manifest_runtime.js <section.html>

const fs = require('fs');
const path = require('path');
const vm = require('vm');
const { parseMarkup } = require('./tree_fake_dom.js');

const sectionPath = process.argv[2];
if (!sectionPath) {
  console.error('usage: node manifest_runtime.js <section.html>');
  process.exit(2);
}

const documentRoot = parseMarkup(fs.readFileSync(sectionPath, 'utf8'));
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
    getElementById: (id) => (id === 'manifest' ? section : null),
  },
  console,
};
sandbox.globalThis = sandbox;
sandbox.window = sandbox;
vm.createContext(sandbox);
vm.runInContext(
  fs.readFileSync(path.join(__dirname, '..', 'src', 'plugin', 'manifest', 'runtime.js'), 'utf8'),
  sandbox
);

let failures = 0;
function same(what, actual, expected) {
  if (String(actual) !== String(expected)) {
    failures++;
    console.error(`MISMATCH ${what}: ${actual} != ${expected}`);
  }
}
const active = (selector) => section.querySelectorAll(selector)
  .filter((el) => el.getAttribute('data-active') === '1');

// Hover the cell of row r0, column c1.
const cell = section.querySelector('.manifest-cell[data-row="r0"][data-col="c1"]');
if (!cell) { console.error('no cell r0/c1'); process.exit(2); }
listeners.mouseover({ target: cell });
same('active rows', active('.manifest-row').map((el) => el.getAttribute('data-row')).join(), 'r0');
same('active columns', active('.manifest-column').map((el) => el.getAttribute('data-col')).join(), 'c1');
const rows = section.querySelectorAll('.manifest-row').length;
same('active cells (one per row)', active('.manifest-cell').length, rows);

// Hovering outside the manifest clears the highlight.
listeners.mouseover({ target: section });
same('cleared rows', active('.manifest-row').length, 0);
same('cleared cells', active('.manifest-cell').length, 0);

// Click column c0: only its members stay opaque.
const header = section.querySelector('.manifest-column[data-col="c0"]');
let prevented = false;
listeners.click({ target: header, preventDefault() { prevented = true; } });
if (!prevented) { console.error('the click was not handled'); process.exit(1); }
for (const row of section.querySelectorAll('.manifest-row')) {
  const member = row.getAttribute('data-cols').split(' ').includes('c0');
  same('filtered ' + row.getAttribute('data-row'), row.getAttribute('opacity'), member ? null : '0.2');
}
same('filter marker', header.getAttribute('data-filter'), '1');

// A second click shows every row again.
listeners.click({ target: header, preventDefault() {} });
for (const row of section.querySelectorAll('.manifest-row')) {
  same('unfiltered ' + row.getAttribute('data-row'), row.getAttribute('opacity'), null);
}
same('filter marker cleared', header.getAttribute('data-filter'), null);

// Click the label of row r0: the columns it is not a member of are dimmed.
const rowLabel = section.querySelector('.manifest-row-label[data-row="r0"]');
const r0cols = section.querySelector('.manifest-row[data-row="r0"]').getAttribute('data-cols').split(' ');
prevented = false;
listeners.click({ target: rowLabel, preventDefault() { prevented = true; } });
if (!prevented) { console.error('the row click was not handled'); process.exit(1); }
for (const el of section.querySelectorAll('.manifest-column').concat(section.querySelectorAll('.manifest-cell'))) {
  const member = r0cols.includes(el.getAttribute('data-col'));
  same('row-filtered ' + el.getAttribute('data-col'), el.getAttribute('opacity'), member ? null : '0.2');
}
same('row filter marker', rowLabel.getAttribute('data-filter'), '1');
same('row filter state', section.getAttribute('data-manifest-filter'), 'r0');
listeners.click({ target: rowLabel, preventDefault() {} });
for (const el of section.querySelectorAll('.manifest-column')) {
  same('row-unfiltered ' + el.getAttribute('data-col'), el.getAttribute('opacity'), null);
}
same('row filter marker cleared', rowLabel.getAttribute('data-filter'), null);

// Click cell r0/c1: rows outside c1 and columns outside r0 are dimmed.
listeners.click({ target: cell, preventDefault() {} });
same('cell filter state', section.getAttribute('data-manifest-filter'), 'r0 c1');
same('cell filter marker', cell.getAttribute('data-filter'), '1');
for (const row of section.querySelectorAll('.manifest-row')) {
  const member = row.getAttribute('data-cols').split(' ').includes('c1');
  same('cell-filtered row ' + row.getAttribute('data-row'), row.getAttribute('opacity'), member ? null : '0.2');
}
for (const el of section.querySelectorAll('.manifest-column')) {
  const member = r0cols.includes(el.getAttribute('data-col'));
  same('cell-filtered column ' + el.getAttribute('data-col'), el.getAttribute('opacity'), member ? null : '0.2');
}
listeners.click({ target: cell, preventDefault() {} });
same('cell filter cleared', section.getAttribute('data-manifest-filter'), '');
same('cell filter marker cleared', cell.getAttribute('data-filter'), null);

const click = (target) => {
  let handled = false;
  listeners.click({ target, preventDefault() { handled = true; } });
  return handled;
};
const num = (el, name) => parseFloat(el.getAttribute(name)) || 0;
const one = (selector) => {
  const found = section.querySelector(selector);
  if (!found) { console.error('missing ' + selector); process.exit(2); }
  return found;
};
const dy = (el) => {
  const m = (el.getAttribute('transform') || '').match(/translate\(0 (-?[\d.]+)\)/);
  return m ? Number(m[1]) : 0;
};
const dx = (el) => {
  const m = (el.getAttribute('transform') || '').match(/translate\((-?[\d.]+) 0\)/);
  return m ? Number(m[1]) : 0;
};

// The removed marks start hidden (the default of `hidemarks`).
const svg = section.querySelector('svg');
const viewBox = svg.getAttribute('viewBox');
const [, , width0, height0] = viewBox.split(' ').map(Number);
for (const c of section.querySelectorAll('.manifest-cell[data-mark="removed"]')) {
  same('removed starts hidden', c.getAttribute('data-mark-hidden'), '1');
}
same('removed legend starts greyed', one('.manifest-legend-entry[data-toggle="m:removed"]').getAttribute('data-hidden'), '1');

// The cross of row r1 folds it: its body goes, a strip with a `+` shows, and
// the rows below move up by the difference.
const fold = num(one('.swimflow-manifest'), 'data-fold');
const row1 = one('.manifest-row[data-row="r1"]');
const rowH = num(row1, 'data-h');
const below = section.querySelectorAll('.manifest-row').filter((r) => num(r, 'data-y') > num(row1, 'data-y'));
const above = section.querySelectorAll('.manifest-row').filter((r) => num(r, 'data-y') < num(row1, 'data-y'));
if (!click(one('.manifest-toggle[data-toggle="r1"]'))) { console.error('the row toggle was not handled'); process.exit(1); }
same('hidden tokens after r1', section.getAttribute('data-manifest-hidden'), 'm:removed r1');
same('r1 body hidden', row1.querySelector('.manifest-row-body').getAttribute('display'), 'none');
same('r1 strip shown', row1.querySelector('.manifest-row-fold').getAttribute('display'), 'inline');
for (const r of below) same('row below moves up ' + r.getAttribute('data-row'), dy(r), -(rowH - fold));
for (const r of above) same('row above stays ' + r.getAttribute('data-row'), dy(r), 0);
same('legend moves up', dy(one('.manifest-legend')), -(rowH - fold));
same('viewBox shorter', svg.getAttribute('viewBox'), `0 0 ${width0} ${height0 - (rowH - fold)}`);

// The cross of column c1 folds it: the columns right of it, their cells and
// the count move left; the brackets to it disappear. It does not filter.
const head0 = one('.manifest-column[data-col="c0"]');
const cellW = num(head0.querySelector('rect'), 'width');
click(one('.manifest-toggle[data-toggle="c1"]'));
same('hidden tokens after c1', section.getAttribute('data-manifest-hidden'), 'm:removed r1 c1');
same('c1 header hidden', one('.manifest-column[data-col="c1"]').getAttribute('display'), 'none');
same('c1 strip shown', one('.manifest-column-fold[data-col="c1"]').getAttribute('display'), 'inline');
same('c0 stays', dx(head0), 0);
same('c2 moves left', dx(one('.manifest-column[data-col="c2"]')), -(cellW - fold));
for (const c of section.querySelectorAll('.manifest-cell[data-col="c2"]')) same('c2 cell moves left', dx(c), -(cellW - fold));
for (const c of section.querySelectorAll('.manifest-cell[data-col="c1"]')) same('c1 cell hidden', c.getAttribute('display'), 'none');
same('count moves left', dx(one('.manifest-count')), -(cellW - fold));
const stripe = one('.manifest-row[data-row="r0"]').querySelector('.manifest-stripe');
same('stripe narrower', num(stripe, 'width'), num(stripe, 'data-w') - (cellW - fold));
same('link to lite hidden', one('.manifest-link[data-col="c1"]').getAttribute('display'), 'none');
same('link to pro shown', one('.manifest-link[data-col="c2"]').getAttribute('display'), 'inline');
same('no filter from a toggle', section.getAttribute('data-manifest-filter'), '');
same('viewBox narrower', svg.getAttribute('viewBox'),
  `0 0 ${width0 - (cellW - fold)} ${height0 - (rowH - fold)}`);

// A legend entry hides every mark of its kind; a second click shows them.
const entry = one('.manifest-legend-entry[data-toggle="m:inherited"]');
click(entry);
same('legend entry greyed out', entry.getAttribute('data-hidden'), '1');
for (const c of section.querySelectorAll('.manifest-cell[data-mark="inherited"]')) same('inherited mark hidden', c.getAttribute('data-mark-hidden'), '1');
for (const c of section.querySelectorAll('.manifest-cell[data-mark="direct"]')) same('direct mark shown', c.getAttribute('data-mark-hidden'), null);
click(one('.manifest-legend-entry[data-toggle="m:removed"]'));
for (const c of section.querySelectorAll('.manifest-cell[data-mark="removed"]')) same('removed shown on request', c.getAttribute('data-mark-hidden'), null);

// The `+` of the folded row unfolds it and everything moves back.
click(row1.querySelector('.manifest-expand'));
same('hidden tokens after unfolding r1', section.getAttribute('data-manifest-hidden'), 'c1 m:inherited');
same('r1 body back', row1.querySelector('.manifest-row-body').getAttribute('display'), 'inline');
for (const r of below) same('row back ' + r.getAttribute('data-row'), dy(r), 0);

// Show all clears every fold, every hidden mark and the filter.
click(header);
click(slot.querySelector('.manifest-show-all'));
same('hidden tokens after show all', section.getAttribute('data-manifest-hidden'), '');
same('filter after show all', section.getAttribute('data-manifest-filter'), '');
same('c1 back', one('.manifest-column[data-col="c1"]').getAttribute('display'), 'inline');
same('c2 back', dx(one('.manifest-column[data-col="c2"]')), 0);
same('legend after show all', entry.getAttribute('data-hidden'), null);
same('viewBox restored', svg.getAttribute('viewBox'), viewBox);
for (const row of section.querySelectorAll('.manifest-row')) same('row after show all', row.getAttribute('opacity'), null);

if (failures) { console.error(failures + ' mismatches'); process.exit(1); }
console.log('ok');
