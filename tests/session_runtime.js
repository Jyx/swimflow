'use strict';
// Regression harness for the live-preview session buffers.
//
// The session logic is inlined in the app shell (see `src/server/web.rs`) and
// exposed on `window.swimflowSession`. This harness extracts that script,
// evaluates it without a DOM, and checks the buffer bookkeeping: unique
// names, add/remove/rename, persistence and the request payload.
//
// Usage: node tests/session_runtime.js <shell.html>

const fs = require('fs');
const vm = require('vm');

const { bodyOf } = require('./shell_body.js');

const path = process.argv[2];
if (!path) {
  console.error('usage: node session_runtime.js <shell.html>');
  process.exit(2);
}

const html = bodyOf(fs.readFileSync(path, 'utf8'));
const scripts = Array.from(html.matchAll(/<script>([\s\S]*?)<\/script>/g), (m) => m[1]);
const source = scripts.find((script) => script.includes('swimflowSession'));
if (!source) {
  console.error('could not find the session script');
  process.exit(2);
}

const sandbox = { window: {} };
vm.runInNewContext(source, sandbox);
const session = sandbox.window.swimflowSession;
if (!session || typeof session.encodeBody !== 'function') {
  console.error('window.swimflowSession is not available');
  process.exit(2);
}

let failures = 0;
function check(condition, message) {
  if (!condition) {
    failures += 1;
    console.error('FAIL: ' + message);
  }
}

// A minimal in-memory `sessionStorage` replacement.
function fakeStorage() {
  const data = new Map();
  return {
    getItem: (key) => (data.has(key) ? data.get(key) : null),
    setItem: (key, value) => data.set(key, String(value)),
    removeItem: (key) => data.delete(key),
  };
}

// --- names -----------------------------------------------------------------
// The extension check is driven by the diagram type, so the store can hold
// files of every type.
check(session.extensionOf('a.diag') === 'diag', 'the extension is read');
check(session.extensionOf('sub/A.SEQ') === 'seq', 'the extension is lower cased');
check(session.extensionOf('a') === '', 'a name without an extension has none');

check(session.matchesExtension('a.diag', ['diag', 'actdiag']), 'a .diag file is accepted');
check(session.matchesExtension('A.DIAG', ['diag', 'actdiag']), 'the extension is case insensitive');
check(session.matchesExtension('a.seq', ['seq']), 'a .seq file is accepted for the sequence type');
check(!session.matchesExtension('a.seq', ['diag', 'actdiag']), 'a file of another type is rejected');
check(!session.matchesExtension('a.txt', ['diag']), 'other files are rejected');
check(!session.matchesExtension('', ['diag']), 'an empty name is rejected');

// --- create ----------------------------------------------------------------
const store = {};
const first = session.create(store, '{ A; }', 'untitled.diag');
const second = session.create(store, '{ A; }', 'untitled.diag');
check(first === 'untitled.diag', 'the first buffer keeps its name');
check(second === 'untitled-2.diag', 'a second buffer gets a free name: ' + second);
const third = session.create(store, 'sequence { }', 'untitled.seq');
const fourth = session.create(store, 'sequence { }', 'untitled.seq');
check(third === 'untitled.seq', 'a buffer of another type keeps its extension: ' + third);
check(fourth === 'untitled-2.seq', 'the free name keeps the extension: ' + fourth);
check(store[first] === '{ A; }', 'the template is stored');

// --- add / remove / rename -------------------------------------------------
session.add(store, 'sub/other.diag', '{ B; }');
check(session.list(store).join(',') ===
  'sub/other.diag,untitled-2.diag,untitled-2.seq,untitled.diag,untitled.seq',
  'the listing is sorted: ' + session.list(store).join(','));
check(session.rename(store, 'sub/other.diag', 'sub/renamed.diag') === 'sub/renamed.diag',
  'a buffer can be renamed');
check(session.rename(store, 'sub/renamed.diag', 'untitled.diag') === null,
  'renaming onto an existing name is refused');
check(session.remove(store, 'sub/renamed.diag') === true, 'a buffer can be removed');
check(session.list(store).indexOf('sub/renamed.diag') < 0, 'the removed buffer is gone');

// --- persistence ------------------------------------------------------------
const storage = fakeStorage();
check(session.save(store, storage) === true, 'the session is saved');
const restored = session.load(storage);
check(Object.keys(restored).length === Object.keys(store).length, 'every buffer is restored');
check(restored['untitled.diag'] === '{ A; }', 'the content survives the round trip');
check(session.load({ getItem: () => 'not json' }) !== null, 'a broken store does not throw');
check(Object.keys(session.load({ getItem: () => 'not json' })).length === 0,
  'a broken store yields an empty session');

// --- related files ----------------------------------------------------------
check(session.isRelated('a.diag', 'b.diag'), 'files in the same directory are related');
check(session.isRelated('sub/a.diag', 'sub/b.diag'), 'files in the same subdirectory are related');
check(session.isRelated('sub/a.diag', 'other.diag'), 'a file in a subdirectory is related to the root');
check(session.isRelated('a.diag', 'sub/b.diag'), 'a file in the root is related to a subdirectory');
check(!session.isRelated('one/a.diag', 'two/b.diag'), 'unrelated directories are not related');

// --- request payload --------------------------------------------------------
const body = session.encodeBody('{ A -> B; }', [
  { path: 'sub/other.diag', content: '{ B; }' },
]);
check(body.indexOf('source=%7B%20A%20-%3E%20B%3B%20%7D') === 0,
  'the source is the first field: ' + body);
check(body.indexOf('file:sub%2Fother.diag=%7B%20B%3B%20%7D') > 0,
  'session files are sent as `file:` fields: ' + body);

// The server splits the body on `&` and `=`, so those characters must be
// escaped in both the source and the file names.
const tricky = session.encodeBody('a & b = c', [{ path: 'x&y=z.diag', content: '1 & 2' }]);
const parts = tricky.split('&');
check(parts.length === 2, 'the payload has two fields: ' + parts.length);
check(parts[0] === 'source=a%20%26%20b%20%3D%20c', 'the source is escaped: ' + parts[0]);
check(parts[1] === 'file:x%26y%3Dz.diag=1%20%26%202', 'the file name is escaped: ' + parts[1]);

if (failures > 0) {
  process.exit(1);
}
console.log('session runtime OK');
