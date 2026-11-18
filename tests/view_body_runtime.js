'use strict';
// Harness for `viewBody()` in the live-preview page: the function that tells
// the server what the preview shows (open diagram, folded branches) so that a
// download looks like the screen.
//
// The function is cut out of the served page and run against a fake preview
// document. It prints the request fields it produces.
//
// Usage: node tests/view_body_runtime.js <page.html> <scenario>

const fs = require('fs');

const [, , pagePath, scenario] = process.argv;
const html = fs.readFileSync(pagePath, 'utf8');
const start = html.indexOf('function viewBody()');
const end = html.indexOf('function downloadExport');
if (start < 0 || end < 0) {
  console.error('viewBody not found in the page');
  process.exit(2);
}
const source = html.slice(start, end);

const element = (attrs, children = {}) => ({
  getAttribute: (name) => (name in attrs ? attrs[name] : null),
  querySelectorAll: (selector) => children[selector] || [],
  querySelector: (selector) => (children[selector] || [])[0] || null,
});
const node = (id, collapsed) => element({ 'data-id': id, 'data-collapsed': collapsed ? '1' : '0' });

const scenarios = {
  // Two trees; the second one is open and has two folded nodes.
  trees: () => {
    const first = element({ 'data-diagram': 'one' }, { '.tree-node': [node('n0'), node('n1', true)] });
    const second = element(
      { 'data-diagram': 'two-n1' },
      { '.tree-node': [node('n0'), node('n1', true), node('n2'), node('n3', true)] }
    );
    return { sections: [first, second], active: second };
  },
  // A tree with nothing folded still reports itself (an empty list).
  unfolded: () => {
    const only = element({ 'data-diagram': 'default' }, { '.tree-node': [node('n0'), node('n1')] });
    return { sections: [only], active: only };
  },
  // Other diagram types have no tree nodes: only the open diagram is sent.
  activity: () => {
    const only = element({ 'data-diagram': 'detail' });
    return { sections: [only], active: only };
  },
  // A manifest with a hidden row and column and a filtered column.
  manifest: () => {
    const only = element(
      { 'data-diagram': 'default', 'data-manifest-hidden': 'r1 c0', 'data-manifest-filter': 'c2' },
      { '.swimflow-manifest': [element({})] }
    );
    return { sections: [only], active: only };
  },
  // A manifest nobody clicked yet reports the defaults it was drawn with.
  plainmanifest: () => {
    const only = element(
      { 'data-diagram': 'default' },
      { '.swimflow-manifest': [element({ 'data-hidden-default': 'm:removed' })] }
    );
    return { sections: [only], active: only };
  },
  // A manifest where everything was shown again reports an empty list.
  shownmanifest: () => {
    const only = element(
      { 'data-diagram': 'default', 'data-manifest-hidden': '' },
      { '.swimflow-manifest': [element({ 'data-hidden-default': 'm:removed' })] }
    );
    return { sections: [only], active: only };
  },
  // An error page has no diagram at all.
  error: () => ({ sections: [], active: null }),
};

const make = scenarios[scenario];
if (!make) {
  console.error('unknown scenario ' + scenario);
  process.exit(2);
}
const { sections, active } = make();
const preview = {
  contentDocument: {
    querySelector: () => active,
    querySelectorAll: () => sections,
  },
};
const encodeURIComponentFn = encodeURIComponent;
const run = new Function('preview', 'encodeURIComponent', source + '; return viewBody();');
process.stdout.write(run(preview, encodeURIComponentFn));
