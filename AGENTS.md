# AGENTS.md — working guide for swimflow

Read this first when starting a session from scratch. It explains the project,
the working rules, and the reference-based testing setup.

## 1. What swimflow is

`swimflow` turns a small text DSL into diagrams. One **single static binary**
(no Python/runtime dependencies) reads a diagram source, lays it out
deterministically, and emits **standalone files**: self-contained SVG by
default, PNG/PDF derived from the same SVG, and an interactive HTML bundle for
drill-down navigation.

Five built-in **diagram types** — activity/swimlane, sequence, tree, treemap
and manifest (`.diag`, `.seq`, `.tree`, `.tmap`, `.mfst`) — each a plugin that
owns parsing and drawing; fonts, PNG/PDF conversion and the HTML bundle are
shared (see [`docs/plugin-architecture.rst`](docs/plugin-architecture.rst)). The
live-preview server (`swimflow --serve`) edits sources in the browser, with
session buffers, and downloads reproduce the view on screen.

The activity diagram began as a port of the original Python implementation
and still reads the same `.diag` files, with layout/metrics parity frozen in
the committed corpora. On top of that grammar it adds edge labels, legends,
node sequence numbers and drill-down (`ref`) across diagrams and files. The
language references live in [`docs/`](docs/): `dsl.rst` (activity),
`sequence-dsl.rst`, `tree-dsl.rst`, `treemap-dsl.rst` and `manifest-dsl.rst`, plus
`architecture.rst`.

## 2. Diagram types

The plugins live under `src/plugin/`; `src/core/` holds what they share.

* **Activity / swimlane** (`.diag`, `src/plugin/activity.rs` over `build/` and
  `render/`): the compatibility-grammar type, with edge labels, legends, sequence
  numbers and drill-down (`ref`) across diagrams and files.
* **Sequence** (`.seq`, `src/plugin/sequence/`): participants, messages,
  replies, notes and frames.
* **Tree** (`.tree`, `src/plugin/tree/`): every node with children has a
  plus/minus circle that folds the branch in an HTML bundle (`runtime.js`, a
  port of the Rust layout that the Node harnesses keep in step with it); `ref`
  opens another `tree` block; `collapsed` starts a branch folded; a tree with
  branches has **Collapse all** / **Expand all** buttons in a bundle.
* **Treemap** (`.tmap`, `src/plugin/treemap/`): nested rectangles sized by a
  number after a colon (`Rent: 1200;`), laid out with a deterministic
  squarified treemap; a bundle gets a **Hide values / Show values** toggle.
  `class NAME [attrs];` and `[class = NAME]` work like the activity diagram's
  classes.
* **Manifest** (`.mfst`, `src/plugin/manifest/`): rows × columns with a mark
  per member cell; columns can be grouped and can `extends` another column
  (`remove` / `override` edit the inherited rows). A bundle highlights the
  hovered row/column and filters by a clicked column. It is generic: domain
  data is converted to `.mfst` outside the binary, e.g. by
  `scripts/cim-mfst.py` for cim manifests (PyYAML from
  `scripts/requirements.txt`, in the git-ignored venv `scripts/.venv`), tested
  by `tests/scripts.rs`. `scripts/cim-tree.py` draws the targets
  (`extends:`) and release branches of manifest repositories as a `.tree`.

The live-preview server (`swimflow --serve`, `make serve`) serves one tab per
diagram type, with session buffers; a download reproduces the view on screen
(the open diagram, the folded branches) through a `ViewState` in
`RenderOptions`.

## 3. Repository layout

```
Makefile, .env.example  targets (debug/release/install/examples); --serve defaults
src/
  main.rs, cli.rs     CLI entry + clap definitions (introspectable in tests)
  lexer.rs, parser.rs,
  ast.rs, error.rs    the activity diagram's front end (still at the crate root)
  core/               diagram-agnostic services: geometry, SVG primitives,
                      SVG->PNG/PDF, HTML shell, zoom.js, ViewState
  plugin/             the diagram types (activity.rs, sequence/, tree/,
                      treemap/, manifest/); each owns ast/parser, and most add
                      model/layout/render plus a runtime.js for the bundle
  plugins/            activity *feature* plugins, e.g. autolane.rs
  build/              activity AST -> arena IR, link resolution, lane/grid layout
  render/             activity SVG writer, label/legend geometry, bundle, PNG/PDF
  metrics.rs, text.rs grid -> pixel boxes; cosmic-text measurement
  server/             `--serve`: HTTP routing, config, file scan, embedded editor
  assets/             the pinned font and its licences
tests/
  *.rs                one integration test per area; *_compat.rs are the frozen
                      upstream-parity corpora
  *_runtime.js        Node harnesses for the embedded JS (tree, treemap,
                      manifest, bundle, zoom, server sessions/highlighter),
                      with a few shared fake-DOM/body helpers
  fixtures/, golden/  inputs and usvg-normalized SVG snapshots
docs/                 language references (dsl.rst, *-dsl.rst) + architecture
examples/             runnable corpus per diagram type + Makefile + README
man/                  swimflow.1 (checked against the CLI)
completions/          bash completion (checked for drift)
editors/nvim/         ftdetect/syntax/ftplugin
scripts/              cim .mfst/.tree converters + requirements.txt
.github/workflows/    ci.yml (gate + MSRV + static musl build), release.yml (tagged releases)
```

## 4. Reference compatibility

The differential work against the original Python implementation is frozen into
the committed corpora under `tests/` (`parser_compat.rs`, `model_compat.rs`,
`geometry_compat.rs`, `metrics_compat.rs`), which `cargo test` runs without
Python. Do not vendor the reference implementation or re-add a Python
toolchain: if a corpus ever needs a new case, capture the behaviour in a Rust
test instead.

## 5. Definition of done (gate)

Every change must pass, with **no warnings**:

```console
$ cargo fmt --check
$ cargo clippy --all-targets -- -D warnings
$ cargo test
```

Rules:

- Commit **only** when the gate is green.
- One commit per logical change, with a short `area: summary` message.
- Keep `Cargo.lock` committed.

## 6. How we test

- **Unit tests** live next to the code in `#[cfg(test)]` modules.
- **Compatibility corpora** generated from the reference are committed so
  `cargo test` needs no Python:
  - `tests/parser_compat.rs` — accept/reject parity;
  - `tests/model_compat.rs` — lane assignment, labels, edges.
- **Differential testing** against Python is a one-off step used to capture
  goldens; the result is committed as a Rust test. The reference environment is
  not part of the repo (see section 4).
- **Example corpus** — `tests/examples.rs` loads every `examples/*.diag`,
  renders it as SVG and HTML, validates the SVG with `usvg` and checks the
  drill-down `data-target`s. Keep the examples readable; they are the public
  showcase as well as a regression test.
- **Scratch files** — tests must use the self-cleaning `TempDir` helper
  (`src/test_support.rs`, `tests/common/mod.rs`), never hand-rolled PID-named
  directories under `/tmp`. `tests/repository.rs` enforces this.
- **Determinism** matters: embedded/pinned font, fixed number formatting, stable
  ordering. Same input ⇒ same bytes.
- Grid geometry is compared **strictly** on integer grid coordinates.
  Pixel/SVG comparisons use explicit, documented tolerances.

## 7. Compatibility quirks — match, don't "fix"

These are faithful to the reference implementation. Changing them changes
which files we accept:

- `Name` is greedy and may contain `-` and `.`, so `A->B` is a **lex error**
  and `A--B` is a single name. Edges need whitespace (`A -> B`).
- `Number` only matches `-?\.digits`. `-1` and `-1.5` are lex errors; `.5` is a
  number; `42` is a name.
- Ordered choice with backtracking: `class;`, `plugin;`, `class = red;`,
  `class -> B;` parse as a node/attribute/edge, not as extensions.
- `A, B [attrs]` creates two nodes sharing the attributes.
- `A -> B -> C [attrs]` creates two edges sharing the attributes.
- `lane id [attrs];` stores the attributes as `Attr` statements on the lane.
- `lane foo [attrs] { ... }` is a **syntax error**.
- `sort_tree` moves `Attr`/`Extension`/`Legend` before other statements
  (stable order).
- Node ids are **unquoted** (`Element.get` unquotes), so `"A"` and `A` are the
  same node.
- Duplicate edges for the same node pair are **merged**.
- Nodes without a lane inherit it from a parent; empty lanes are removed.
- The default orientation is **portrait**.

## 8. How to add a feature

1. Implement behind the existing module boundaries (front end → `build/` →
   render). Keep layout/rendering pure and deterministic.
2. Add tests, including reference-derived goldens where applicable.
3. Update `docs/`, the README, `examples/README.rst` and the man page when user-visible
   behaviour changes.
4. Run the gate (section 5) and commit.

## 8b. Things that must stay in step

* `src/plugin/tree/layout.rs` and `src/plugin/tree/runtime.js` are two
  implementations of the same layout. Change one and the other, then run
  `cargo test --test tree`: `tests/tree_runtime.js` compares them to the
  pixel and `tests/tree_dom_runtime.js` clicks through a fake DOM.
* The dashed connectors of a tree (`edgestyle = dashed`, `bus_stretches` in
  `layout.rs`) are ported in `runtime.js` too; `tests/tree_runtime.js`
  compares the stem and both bus paths, and the fixture in `tests/tree.rs`
  carries `edgestyle` attributes for that.
* The node ids of a tree (`n0`, `n1`, …) follow the breadth-first source
  order; the live preview sends them back with a download (`viewBody()` in
  `src/server/web.rs`, read by `parse_payload` in `src/server/mod.rs`).
* A new diagram type needs an entry in `Registry::builtin`, a highlighter
  grammar in `src/server/web.rs`, an example directory, and updates to the
  man page and `examples/README.rst` (the drift tests will tell you).

## 9. How to add a DSL feature

- **Additive only**: never change how an existing file parses.
- Preserve unknown attributes (they are stored in `AttrMap`).
- Update the language reference in `docs/`, parser tests, and — if the reference
  also supports it — the compat corpus.
- If the feature changes the IR, update `src/build/model.rs` and the builder.

## 10. Environment

- Rust edition 2024 (so let-chains and the 2024 style edition are available);
  recent `rustc`/`cargo`.
- `rust-version` in `Cargo.toml` is the MSRV floor and tracks the dependencies:
  `cosmic-text` currently sets it (1.89). Bump it when a dependency bump
  raises it — it must never be lower than what the locked graph needs, or
  `cargo build` fails on the toolchain it claims to support. The `msrv` CI job
  reads `rust-version` from `Cargo.toml` and runs `cargo test --locked` on
  exactly that toolchain.
- Static binary: `rustup target add x86_64-unknown-linux-musl` and install
  `musl-tools`. Wired in CI; not built locally by default.
- Releases: pushing a `vX.Y.Z` tag (matching `Cargo.toml`) runs
  `.github/workflows/release.yml`, which publishes Linux (x86_64/aarch64 musl),
  macOS (x86_64/aarch64) and Windows (x86_64) archives plus `SHA256SUMS`;
  `install.sh` is the curl installer for the Linux/macOS ones.
- If `nvim` is available, open any `examples/*.diag` file to
  review the highlighting whenever the syntax file changes.

## 11. Do not

- Do not commit `target/` or any generated `examples/**/*.svg|png|pdf|html`.
- Do not introduce global mutable state (upstream's registries are replaced by
  arenas on purpose).
- Do not change parser/lexer behaviour without regenerating and updating the
  compat corpus.
- Do not commit with `fmt`/`clippy` warnings.
