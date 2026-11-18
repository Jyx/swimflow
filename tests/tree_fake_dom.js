'use strict';
// A tiny fake DOM for the tree harnesses (tests/tree_dom_runtime.js and
// tests/tree_fold_all_runtime.js). jsdom is not a dependency: it implements
// exactly what src/plugin/tree/runtime.js uses, on top of the SVG markup.

class Element {
  constructor(tagName, attrs, parent) {
    this.tagName = tagName;
    this.attrs = new Map(Object.entries(attrs));
    this.parent = parent;
    this.children = [];
    this.text = '';
  }
  getAttribute(name) { return this.attrs.has(name) ? this.attrs.get(name) : null; }
  setAttribute(name, value) { this.attrs.set(name, String(value)); }
  removeAttribute(name) { this.attrs.delete(name); }
  hasAttribute(name) { return this.attrs.has(name); }
  get firstElementChild() { return this.children[0] || null; }
  get nextElementSibling() {
    if (!this.parent) return null;
    return this.parent.children[this.parent.children.indexOf(this) + 1] || null;
  }
  get ownerSVGElement() {
    let node = this.parent;
    while (node && node.tagName !== 'svg') node = node.parent;
    return node;
  }
  matches(selector) {
    const m = selector.match(/^([a-z]+)?((?:\.[\w-]+)*)((?:\[[^\]]+\])*)$/);
    if (!m) throw new Error('unsupported selector ' + selector);
    if (m[1] && m[1] !== this.tagName) return false;
    const classes = (this.getAttribute('class') || '').split(/\s+/);
    for (const cls of m[2].split('.').filter(Boolean)) if (!classes.includes(cls)) return false;
    for (const part of m[3].matchAll(/\[([\w-]+)="([^"]*)"\]/g)) {
      if (this.getAttribute(part[1]) !== part[2]) return false;
    }
    return true;
  }
  closest(selector) {
    for (let node = this; node; node = node.parent) if (node.matches && node.matches(selector)) return node;
    return null;
  }
  querySelectorAll(selector) {
    const out = [];
    const walk = (node) => {
      for (const child of node.children) {
        if (child.matches(selector)) out.push(child);
        walk(child);
      }
    };
    walk(this);
    return out;
  }
  querySelector(selector) { return this.querySelectorAll(selector)[0] || null; }
}

function parseMarkup(source) {
  const root = new Element('#document', {}, null);
  let current = root;
  for (const match of source.matchAll(/<(\/?)([a-zA-Z]+)([^>]*?)(\/?)>/g)) {
    const [, closing, name, rest, selfClosing] = match;
    if (closing) { current = current.parent; continue; }
    const attrs = {};
    for (const a of rest.matchAll(/([\w:-]+)="([^"]*)"/g)) attrs[a[1]] = a[2];
    const element = new Element(name, attrs, current);
    current.children.push(element);
    if (!selfClosing) current = element;
  }
  return root;
}

// The <svg> element of a rendered diagram.
function parseSvg(source) {
  return parseMarkup(source).children.find((child) => child.tagName === 'svg');
}

module.exports = { Element, parseMarkup, parseSvg };
