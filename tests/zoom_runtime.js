'use strict';
// Harness for the zoom of an interactive page (src/core/zoom.js): the buttons,
// the keys, Ctrl/Cmd + wheel, the fit to the window and the size that follows a
// changed viewBox. The script is taken from a real bundle and run against a
// small mock DOM.
//
// Usage: node tests/zoom_runtime.js <bundle.html>

const fs = require('fs');
const assert = require('assert');

const html = fs.readFileSync(process.argv[2], 'utf8');
const scripts = Array.from(html.matchAll(/<script>([\s\S]*?)<\/script>/g), (m) => m[1]);
const zoomScript = scripts.find((script) => script.includes("'wheel'"));
assert(zoomScript, 'the page has no zoom script');
assert.strictEqual(scripts.filter((s) => s.includes("'wheel'")).length, 1, 'zoom script once');

const WINDOW = { width: 532, height: 800 }; // 500 x 728 left for the diagram after the padding
const BAR = 40;

function svgElement(viewBox) {
  const attrs = { viewBox };
  return {
    tagName: 'svg',
    style: {},
    getAttribute: (name) => (name in attrs ? attrs[name] : null),
    setAttribute(name, value) { attrs[name] = String(value); },
    getBoundingClientRect() {
      return { left: 10, top: 20, width: parseFloat(this.style.width) || 0, height: parseFloat(this.style.height) || 0 };
    },
  };
}

function sectionElement(key, viewBox) {
  const svg = svgElement(viewBox);
  return {
    svg,
    getAttribute: (name) => (name === 'data-diagram' ? key : null),
    querySelector: (selector) => (selector === 'svg' ? svg : null),
  };
}

const small = sectionElement('small', '0 0 100 50');
const wide = sectionElement('wide', '0 0 1000 500');
const tall = sectionElement('tall', '0 0 400 2000');
let active = wide;

const label = { textContent: '' };
let groupClick;
const group = {
  querySelector: (selector) => (selector === '.swimflow-zoom-level' ? label : null),
  addEventListener: (type, handler) => { if (type === 'click') groupClick = handler; },
};
const bar = { offsetHeight: BAR };
const container = {
  querySelector: (selector) => ({ '.swimflow-zoom': group, '.swimflow-bar': bar }[selector] || null),
};

const listeners = {};
const document = {
  documentElement: { clientWidth: WINDOW.width },
  getElementById: (id) => (id === 'swimflow' ? container : null),
  querySelector: (selector) => (selector === '.swimflow-diagram.is-active' ? active : null),
  addEventListener: (type, handler) => { listeners[type] = handler; },
};
const scrolled = [];
const window = {
  innerHeight: WINDOW.height,
  addEventListener: (type, handler) => { listeners['window:' + type] = handler; },
  scrollBy: (x, y) => scrolled.push([x, y]),
};
let observed = null;
let mutate = null;
function MutationObserver(callback) { mutate = callback; this.observe = (svg) => { observed = svg; }; }

new Function('document', 'window', 'MutationObserver', zoomScript)(document, window, MutationObserver);

const size = (section) => [section.svg.style.width, section.svg.style.height];
function press(key, extra) {
  let prevented = false;
  listeners.keydown(Object.assign({ key, target: { tagName: 'BODY' }, preventDefault() { prevented = true; } }, extra));
  return prevented;
}
function click(action) {
  groupClick({ target: { closest: () => ({ getAttribute: () => action }) }, preventDefault() {} });
}
function wheel(deltaY, extra) {
  let prevented = false;
  listeners.wheel(Object.assign({ deltaY, deltaMode: 0, clientX: 110, clientY: 70, preventDefault() { prevented = true; } }, extra));
  return prevented;
}

// A diagram opens fitted to the window: 500px wide for 1000 -> 50%.
assert.deepStrictEqual(size(wide), ['500px', '250px']);
assert.strictEqual(label.textContent, '50%');
assert.strictEqual(observed, wide.svg, 'the viewBox is watched');

// A small diagram is not enlarged beyond 100%.
active = small;
listeners['swimflow:show']();
assert.deepStrictEqual(size(small), ['100px', '50px']);
assert.strictEqual(label.textContent, '100%');

// A tall diagram is fitted by its height: 728 / 2000.
active = tall;
listeners['swimflow:show']();
assert.strictEqual(label.textContent, '36%');
active = wide;
listeners['swimflow:show']();

// Buttons.
click('in');
assert.strictEqual(label.textContent, '63%');
click('out');
assert.strictEqual(label.textContent, '50%');
click('reset');
assert.deepStrictEqual(size(wide), ['1000px', '500px']);
click('fit');
assert.deepStrictEqual(size(wide), ['500px', '250px']);

// Keys, but not with a modifier and not while typing.
assert(press('1'));
assert.strictEqual(label.textContent, '100%');
assert(press('+'));
assert.strictEqual(label.textContent, '125%');
assert(press('-'));
assert(press('0'));
assert.strictEqual(label.textContent, '50%');
assert(!press('+', { ctrlKey: true }));
assert(!press('+', { metaKey: true }));
assert(!press('+', { target: { tagName: 'TEXTAREA' } }));
assert(!press('x'));
assert.strictEqual(label.textContent, '50%');

// Ctrl/Cmd + wheel zooms around the pointer; a plain wheel is left to scroll.
assert(!wheel(-40), 'a plain wheel scrolls');
assert.strictEqual(label.textContent, '50%');
assert(wheel(-40, { ctrlKey: true }));
assert(parseInt(label.textContent, 10) > 50, 'wheel up zooms in');
assert(scrolled.length > 0, 'the pointer position is kept');
const zoomedIn = parseInt(label.textContent, 10);
assert(wheel(40, { metaKey: true }));
assert(parseInt(label.textContent, 10) < zoomedIn, 'wheel down zooms out');

// The zoom stays within its limits.
for (let i = 0; i < 60; i += 1) press('+');
assert.strictEqual(label.textContent, '800%');
for (let i = 0; i < 80; i += 1) press('-');
assert.strictEqual(label.textContent, '5%');

// A fitted diagram follows the window and a changed viewBox (a folded tree).
press('0');
window.innerHeight = 400;
listeners['window:resize']();
assert.deepStrictEqual(size(wide), ['500px', '250px']);
wide.svg.setAttribute('viewBox', '0 0 1000 1000');
mutate();
assert.strictEqual(parseFloat(wide.svg.style.height), 1000 * (400 - BAR - 32) / 1000);

// A chosen zoom is kept per diagram and not reset by a changed viewBox.
press('1');
wide.svg.setAttribute('viewBox', '0 0 1000 800');
mutate();
assert.deepStrictEqual(size(wide), ['1000px', '800px']);

console.log('ok: zoom buttons, keys, Ctrl/Cmd+wheel, fit and viewBox changes');
