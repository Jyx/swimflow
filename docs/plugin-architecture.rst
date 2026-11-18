The diagram-type plugin architecture
====================================

.. note::

   Built-in types: activity, sequence, tree, treemap and manifest.

1. The problem
--------------

``swimflow`` began around one diagram type: the activity/swimlane diagram.
Every part of the program knew about it — the lexer and the parser implement
the reference grammar, the IR models lanes, nodes and edges, the grid layout
computes lane assignment, the metrics port the reference code, and the SVG
writer draws lanes, frames and arrowheads.

Adding a second diagram type (a sequence diagram) to that code would have
meant either

* duplicating the front end, the font handling, the SVG writer, the PNG/PDF
  conversion and the HTML bundle, or
* threading "is this a sequence diagram?" through every stage.

Neither is acceptable. This document describes the split that was made
instead: **a core of diagram-agnostic services** and **a plugin per diagram
type**.

2. The shape of the solution
----------------------------

.. code-block:: text

                        ┌──────────────────────────────────────────┐
                        │                core                       │
       .diag ──┐       │  fonts · text measurement · pixel boxes    │
               │       │  SVG primitives · PNG/PDF · HTML bundle    │
       .seq  ──┼──▶ plugin registry ──▶ plugin.render() ──▶ Section │
               │       │                                          │
               └───────┘  activity plugin   sequence plugin        │
                        └──────────────────────────────────────────┘

* A **plugin** owns the reading of a source file and the drawing of the
  result. It knows nothing about output formats.
* The **core** owns everything that is not about a diagram vocabulary: the
  font, the text measurement, the SVG primitives, the raster/PDF conversion
  and the interactive HTML bundle.
* The **registry** decides which plugin handles an input.

The contract is deliberately small (``src/plugin/mod.rs``):

.. code-block:: rust

    pub trait DiagramPlugin: Send + Sync {
        fn name(&self) -> &'static str;              // "activity", "sequence"
        fn description(&self) -> &'static str;       // shown by --list-types
        fn extensions(&self) -> &'static [&'static str];
        fn detect(&self, source: &str) -> bool;      // content sniffing
        fn render(&self, context: &RenderContext<'_>) -> Result<Rendered, PluginError>;
        fn dump(&self, kind: DumpKind, context: &RenderContext<'_>) -> Result<String, PluginError>;
    }

``render`` returns *sections*: a key, a label and an SVG document. The core
turns those into the requested format:

.. list-table::
   :header-rows: 1

   * - Format
     - Produced by
   * - SVG
     - the section's SVG, unchanged
   * - PNG
     - ``core::formats::svg_to_png`` (resvg)
   * - PDF
     - ``core::formats::svg_to_pdf`` (svg2pdf)
   * - HTML
     - ``core::html::bundle`` (one ``<section>`` per diagram plus the runtime)

A new diagram type therefore only has to produce SVG; PNG, PDF and the HTML
bundle come for free.

3. What moved into the core
---------------------------

.. list-table::
   :header-rows: 1

   * - Module
     - Contents
     - Why
   * - ``core/geom.rs``
     - ``PixelBox``
     - a rectangle is not a swimlane concept
   * - ``core/svg.rs``
     - ``rect``, ``line``, ``path``, ``polygon``, ``ellipse``, ``draw_text``,
       ``open_document``, ``escape_xml``, ``base64``, ``dash_style``, ``fmt``
     - one drawing vocabulary, one number format, one escaping rule, one font
       handling
   * - ``core/formats.rs``
     - ``svg_to_png``, ``svg_to_pdf``, ``parse_svg``
     - the raster/PDF backends work on an SVG *string*, not on a document set
   * - ``core/html.rs``
     - ``Section``, ``bundle``, ``font_face_css``
     - the bundle shell is a page, not a diagram

The activity renderer (``src/render/``) was **not** rewritten: it now calls
the core helpers instead of its own private copies, and its output is
byte-for-byte identical to the previous release (verified by comparing every
example before and after).

4. The plugins
--------------

4.1 ``activity`` — ``src/plugin/activity.rs``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

A thin adapter over the existing pipeline: ``build::load_str`` (which resolves
``ref`` links across files) → the arena IR → the SVG renderer. It exists to
prove that the *old* diagram type could be expressed through the *new*
contract without changing a byte of its output.

4.2 ``sequence`` — ``src/plugin/sequence/``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

A complete second implementation, written from scratch:

.. list-table::
   :header-rows: 1

   * - Module
     - Role
   * - ``ast.rs``
     - the syntax tree of the sequence DSL
   * - ``parser.rs``
     - a hand-written scanner and a recursive-descent parser (line/column
       errors)
   * - ``model.rs``
     - the AST with participants resolved to indices
   * - ``layout.rs``
     - the model → pixel geometry (participants, lifelines, messages, notes,
       frames)
   * - ``render.rs``
     - the geometry → SVG, using only ``core::svg``

The sequence language is documented in `sequence-dsl.rst
<sequence-dsl.rst>`_.

4.3 ``tree`` — ``src/plugin/tree/``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The third plugin: a hierarchy drawn as one SVG (``.tree`` files).

.. list-table::
   :header-rows: 1

   * - Module
     - Role
   * - ``ast.rs``, ``parser.rs``
     - the syntax tree and a scanner + recursive-descent parser (line/column
       errors)
   * - ``layout.rs``
     - a tidy-tree layout over measured nodes, portrait or landscape; folded
       branches take no room
   * - ``render.rs``
     - the geometry → SVG with ``core::svg``; ``ref`` links, fold circles,
       ``data-*`` geometry for the runtime
   * - ``runtime.js``
     - folds and unfolds branches in an HTML bundle; a port of ``layout.rs``

It is the first plugin with *interactive* behaviour of its own. It does not
touch the bundle runtime: the section carries its script
(``Section::with_script``) and the bundle includes each script once. The
language is documented in `tree-dsl.rst <tree-dsl.rst>`_.

4.4 ``treemap`` — ``src/plugin/treemap/``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The fourth plugin: a hierarchy drawn as nested rectangles whose areas follow a
number (``.tmap`` files).

.. list-table::
   :header-rows: 1

   * - Module
     - Role
   * - ``ast.rs``, ``parser.rs``
     - the syntax tree and a scanner + recursive-descent parser (``Name: 42;``)
   * - ``model.rs``
     - values resolved: a leaf defaults to 1, a container to the sum of its
       children
   * - ``layout.rs``
     - a deterministic squarified treemap over a fixed canvas
       (``width``/``height``, default 800 × 500)
   * - ``render.rs``
     - the geometry → SVG with ``core::svg``; labels truncated with ``...``,
       values on by default
   * - ``runtime.js``
     - hides/shows the values in an HTML bundle (``Hide values`` /
       ``Show values``)

It is static: there is no drill-down (``ref``) or folding. The language is
documented in `treemap-dsl.rst <treemap-dsl.rst>`_.

4.5 ``manifest`` — ``src/plugin/manifest/``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The fifth plugin: which rows belong to which columns (``.mfst`` files), with
column groups and column inheritance (``extends``, ``remove``, ``override``).

.. list-table::
   :header-rows: 1

   * - Module
     - Role
   * - ``ast.rs``, ``parser.rs``
     - the syntax tree and a recursive-descent parser over the treemap scanner
   * - ``model.rs``
     - membership resolved per cell (direct, inherited, overridden, removed),
       parents first
   * - ``layout.rs``
     - one cell per column, one line per row, inheritance brackets on tracks
   * - ``render.rs``
     - the geometry → SVG with ``core::svg``; rotated column labels, marks,
       count bars, legend
   * - ``runtime.js``
     - hover highlight and column filter in an HTML bundle

The language is documented in `manifest-dsl.rst <manifest-dsl.rst>`_.

5. Choosing a plugin
--------------------

``Registry::resolve`` tries, in order:

1. the type given with ``--type`` (an error lists the known types);
2. the file extension (``.diag`` and its legacy alias → activity; ``.seq``,
   ``.seqdiag``, ``.sdiag`` → sequence; ``.tree``, ``.treediag`` → tree;
   ``.tmap``, ``.treemap`` → treemap; ``.mfst``, ``.manifest`` → manifest);
3. the content of the file (``sequence {`` or a ``participant`` declaration
   → sequence; a ``tree … {`` block header → tree; a ``treemap … {`` block
   header → treemap; a ``manifest … {`` block header → manifest; otherwise
   activity).

``--list-types`` prints the known types and their descriptions.

6. The live-preview server
--------------------------

The server is plugin-aware: it asks the registry which plugin owns the
requested file (by the ``type`` query parameter, or by the file extension) and
renders it with that plugin. The browser gets one **tab per diagram type**:

* the tab selects the file tree (the server builds one tree per type),
* the syntax highlighting follows the type (the highlighter takes the type
  name),
* a new file starts from the template of the type (the plugin provides it),
* the import accepts the files of the type, and
* the export formats follow the type (the source format of a sequence diagram
  is ``.seq``, not ``.diag``).

The rendered preview can be expanded to fill the window below the toolbar
(the file list and editor are hidden) and restored again; the choice is
remembered in the browser.

A **download** reproduces the preview. The page reads the state of the
preview frame — the diagram that is open and the folded nodes of every tree —
and sends it with the export request (``view:active``,
``view:collapsed:<key>``). The server turns it into a ``ViewState``
(``core/view.rs``) in the render options; a plugin reads the parts it
understands (the tree plugin applies the folds, the server puts the open
diagram first). The treemap value toggle is not part of ``ViewState``, so an
export follows ``showvalue``. The source is never changed.

The server resolves the same HTML bundle as the command line, through the
shared ``plugin::render_output_with_font_url``, so the preview and the
exported file are identical. PNG downloads use ``SWIMFLOW_SCALE`` for their
raster scale (default 1.0), the server-side counterpart of ``--scale``.

Two things made this possible:

* the plugin context carries an **overlay** — the files that live in memory
  only (the browser session). The activity plugin resolves a ``ref`` to such
  a file through it, exactly like a file on disk.
* the plugin interface exposes the **type catalog** (name, description,
  extensions, source extension, template), so the user interface does not
  have to know about the types either.

7. What is *not* plugin-aware
-----------------------------

* **The Neovim syntax file** highlights the activity DSL. Sequence, tree and
  treemap syntax files would be separate ``ft`` groups (the live preview's
  editor does highlight all four).
* **Cross-file ``ref`` values** are an activity feature. A plugin that wants
  them needs a way to load other files; the interface passes the source, the
  path and the overlay, and the plugin decides what to do with them.

8. Lessons / decisions
----------------------

* **SVG is the only contract between a plugin and the core.** Everything else
  (PNG, PDF, HTML) is derived from it, so a plugin cannot get the formats
  "half right".
* **The plugin does not see the output format.** The core decides whether a
  diagram is rendered as a standalone document or as a bundle section; the
  plugin is told which via ``RenderMode``.
* **The type catalog is part of the interface.** The user interface (the
  live preview) needs the name, the extensions, the source extension and a
  starter template of every type; asking the plugin for them keeps the
  browser side free of hard-coded knowledge.
* **Detection is a fallback, not a promise.** An explicit ``--type`` always
  wins, so a user is never stuck with a bad guess.
* **The activity output stayed byte-identical.** The refactor was accepted
  only because the existing golden corpora and the example corpus still
  produce the same bytes; that is the strongest available regression test
  for a refactor of this size.
* **A second implementation is the real test of an abstraction.** The
  sequence diagram could not be written without the split, and it needed
  *less* code than expected precisely because the core already provided the
  fonts, the SVG primitives and the formats.

9. How to add another diagram type
----------------------------------

1. Add ``src/plugin/<name>/`` with an ``ast.rs``, a ``parser.rs``, a
   ``model.rs``, a ``layout.rs`` and a ``render.rs`` (the split is a
   convention, not a rule — a plugin only has to implement
   ``DiagramPlugin``).
2. Implement ``DiagramPlugin`` for ``<Name>Plugin``.
3. Register it in ``Registry::builtin``.
4. Add an example under ``examples/``, a grammar for the editor highlighter in
   ``src/server/web.rs``, and tests in ``tests/plugin.rs``.
5. If the diagram is interactive, ship the script with the section
   (``Section::with_script``) and test it against the Rust side the way
   ``tests/tree.rs`` does.

No change to the core, the CLI or the formats is required.
