'use strict';
// Regression harness for the live-preview syntax highlighter.
//
// The highlighter is inlined in the app shell (see `src/server/web.rs`) and
// exposed on `window.swimflowHighlight`. This harness extracts that script,
// evaluates it without a DOM, and checks the token classes and robustness
// against partial/invalid input.
//
// Usage: node tests/highlight_runtime.js <shell.html>

const fs = require('fs');
const vm = require('vm');

const { bodyOf } = require('./shell_body.js');

const path = process.argv[2];
if (!path) {
  console.error('usage: node highlight_runtime.js <shell.html>');
  process.exit(2);
}

const html = bodyOf(fs.readFileSync(path, 'utf8'));
const scripts = Array.from(html.matchAll(/<script>([\s\S]*?)<\/script>/g), (m) => m[1]);
const source = scripts.find((script) => script.includes('swimflowHighlight'));
if (!source) {
  console.error('could not find the syntax highlighter script');
  process.exit(2);
}

const sandbox = { window: {} };
vm.runInNewContext(source, sandbox);
const highlight = sandbox.window.swimflowHighlight;
if (typeof highlight !== 'function') {
  console.error('window.swimflowHighlight is not a function');
  process.exit(2);
}

let failures = 0;
function check(condition, message) {
  if (!condition) {
    failures += 1;
    console.error('FAIL: ' + message);
  }
}

const sample = [
  '# a comment',
  'diagram hello {',
  '  lane foo [color = "red"] {',
  '    A -> B [label = "go"];',
  '    B --> C;',
  '  }',
  '  numbered = 3;',
  '}',
].join('\n');

const out = highlight(sample);
check(out.includes('<span class="tok-comment"># a comment</span>'), 'line comment is highlighted');
check(out.includes('<span class="tok-keyword">diagram</span>'), 'diagram keyword is highlighted');
check(out.includes('<span class="tok-keyword">lane</span>'), 'lane keyword is highlighted');
check(out.includes('<span class="tok-string">"red"</span>'), 'string is highlighted');
check(out.includes('<span class="tok-attr">color</span>'), 'attribute name is highlighted');
check(out.includes('<span class="tok-edge">-></span>'), 'arrow is highlighted');
check(out.includes('<span class="tok-edge">--></span>'), 'long arrow is highlighted');
check(out.includes('<span class="tok-number">3</span>'), 'number is highlighted');
check(out.includes('<span class="tok-op">{</span>'), 'operator is highlighted');
// The sequence grammar has its own keywords and operators.
const sequence = [
  'sequence "Chat" {',
  '  participant "User" as user;',
  '  user -> server: "hello";',
  '  server ->> user: "hi";',
  '  server --> user: "reply";',
  '  note over user, server: "a note";',
  '  loop "again" {',
  '    user -> user: "think";',
  '  }',
  '}',
].join('\n');

const sequenceOut = highlight(sequence, 'sequence');
check(sequenceOut.includes('<span class="tok-keyword">sequence</span>'), 'the sequence keyword is highlighted');
check(sequenceOut.includes('<span class="tok-keyword">participant</span>'), 'participant is highlighted');
check(sequenceOut.includes('<span class="tok-keyword">note</span>'), 'note is highlighted');
check(sequenceOut.includes('<span class="tok-keyword">loop</span>'), 'loop is highlighted');
check(sequenceOut.includes('<span class="tok-keyword">as</span>'), 'as is highlighted');
check(sequenceOut.includes('<span class="tok-edge">->></span>'), 'the async arrow is highlighted');
check(sequenceOut.includes('<span class="tok-edge">--></span>'), 'the reply arrow is highlighted');
check(sequenceOut.includes('<span class="tok-edge">-></span>'), 'the call arrow is highlighted');
check(!sequenceOut.includes('<span class="tok-keyword">lane</span>'), 'the activity keywords are not used');
// The tree grammar knows `tree` and `root`, and has no arrows.
const treeOut = highlight('tree "P" {\n  root "R";\n  A [color = "red"] { B; }\n}', 'tree');
check(treeOut.includes('<span class="tok-keyword">tree</span>'), 'the tree keyword is highlighted');
check(treeOut.includes('<span class="tok-keyword">root</span>'), 'root is highlighted');
check(treeOut.includes('<span class="tok-attr">color</span>'), 'tree attributes are highlighted');
check(!treeOut.includes('<span class="tok-keyword">lane</span>'), 'the activity keywords are not used for trees');
// The treemap grammar knows `treemap`, `root` and the numeric settings.
const treemapOut = highlight(
  'treemap "Budget" {\n  class warm [color = "red"];\n  width = 800;\n  root "Budget";\n  A: 1 [class = warm];\n}',
  'treemap'
);
check(treemapOut.includes('<span class="tok-keyword">treemap</span>'), 'the treemap keyword is highlighted');
check(treemapOut.includes('<span class="tok-keyword">root</span>'), 'the treemap root is highlighted');
check(treemapOut.includes('<span class="tok-keyword">class</span>'), 'the treemap class keyword is highlighted');
check(treemapOut.includes('<span class="tok-number">1</span>'), 'treemap values are numbers');
check(treemapOut.includes('<span class="tok-op">:</span>'), 'the treemap value separator is an operator');
check(!treemapOut.includes('<span class="tok-keyword">lane</span>'), 'the activity keywords are not used for treemaps');
// The manifest grammar knows its block keywords and the inheritance edits.
const manifestOut = highlight(
  'manifest "M" {\n  group g {\n    column a { x; }\n    column b [extends = a] { remove x; override y; }\n  }\n  rowgroup t { row z: a; }\n}',
  'manifest'
);
for (const word of ['manifest', 'group', 'column', 'remove', 'override', 'rowgroup', 'row']) {
  check(manifestOut.includes('<span class="tok-keyword">' + word + '</span>'), 'the manifest keyword ' + word + ' is highlighted');
}
check(!manifestOut.includes('<span class="tok-keyword">lane</span>'), 'the activity keywords are not used for manifest diagrams');
// The activity grammar must not highlight the sequence keywords.
check(!highlight('sequence { }').includes('<span class="tok-keyword">sequence</span>'),
  'the activity grammar does not know the sequence keywords');
// An unknown type falls back to the activity grammar.
check(highlight('diagram { }', 'nope').includes('<span class="tok-keyword">diagram</span>'),
  'an unknown type falls back to the activity grammar');

// HTML metacharacters in source must be escaped, never emitted raw.
check(highlightSourceEscapes(highlight), 'source metacharacters are escaped');

function highlightSourceEscapes(fn) {
  const escaped = fn('A [label = "a<b>c&d"];');
  return escaped.includes('&lt;b&gt;') && escaped.includes('&amp;') && !escaped.includes('<b>');
}

// Partial or invalid input must never throw; the editor is always editable.
for (const partial of ['', 'diagram {', '"unterminated', "'unterminated", '/* unclosed', 'A -> ', '\\']) {
  try {
    highlight(partial);
  } catch (error) {
    failures += 1;
    console.error('FAIL: highlighter threw on partial input ' + JSON.stringify(partial) + ': ' + error);
  }
}

if (failures > 0) {
  process.exit(1);
}
console.log('highlight runtime OK');
