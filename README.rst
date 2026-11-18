swimflow
========

Activity, sequence, tree, treemap and manifest diagrams from a small text DSL.

``swimflow`` is a single static binary. It reads a diagram source, lays it out
deterministically and writes standalone files: self-contained SVG by default,
PNG or PDF derived from the same SVG, and an interactive HTML bundle with
drill-down navigation. The live-preview server edits sources in the browser;
a download reproduces exactly what is on screen.

Five diagram types are built in:

* activity/swimlane (``.diag``)
* sequence (``.seq``)
* tree (``.tree``)
* treemap (``.tmap``)
* manifest (``.mfst``)

Each type is a plugin that owns its parser and drawing code. Fonts, PNG/PDF
conversion and the HTML bundle are shared. The activity language is a
compatible port of the original Python implementation, with additive features
such as edge labels, legends, sequence numbers and cross-diagram links.

Install
-------

Prebuilt binaries for Linux (x86_64 and aarch64, static musl), macOS (Intel
and Apple silicon) and Windows (x86_64) are on the `releases page
<https://github.com/Jyx/swimflow/releases>`_.

On Linux or macOS, install the latest release into ``$HOME/.local/bin``:

.. code-block:: console

   $ curl -fsSL https://raw.githubusercontent.com/Jyx/swimflow/main/install.sh | sh

The installer checks the download against the release's ``SHA256SUMS`` and
prints a ``PATH`` hint when needed. See ``install.sh --help`` for the version
and directory options. On Windows, download the release zip and put
``swimflow.exe`` on ``PATH``. You can also build from source, below.

Build
-----

.. code-block:: console

   $ make debug             # target/debug/swimflow
   $ make release           # target/release/swimflow
   $ make install           # build release and copy to $HOME/bin/swimflow
   $ make examples          # render the example corpus to SVG

Use ``TYPE=html``, ``TYPE=png`` or ``TYPE=pdf`` with ``make examples`` to
render another format. ``make clean`` removes generated example files.
``make serve`` starts the live-preview server from ``.env``; copy
``.env.example`` to ``.env`` first.

Usage
-----

.. code-block:: console

   swimflow [OPTIONS] [FILE]

``FILE`` defaults to standard input (``-``). Output defaults to the input path
with the format's extension, or to standard output when reading stdin.

.. code-block:: console

   # SVG next to the input file.
   $ swimflow simple.diag

   # PNG, PDF, and self-contained SVG with the font embedded.
   $ swimflow simple.diag -f png --scale 2 -o simple.png
   $ swimflow simple.diag -f pdf -o simple.pdf
   $ swimflow simple.diag --embed-font -o simple.svg

   # An interactive HTML bundle.
   $ swimflow manual.diag --bundle manual.html --embed-font

   # Read from stdin and write SVG to stdout.
   $ echo '{ A -> B; }' | swimflow -o -

   # List the known diagram types, or force one.
   $ swimflow --list-types
   $ swimflow --type sequence file.txt -o -

Run ``swimflow --help`` for every option, or see ``man/swimflow.1``. Options
that have no live-preview equivalent are grouped under ``CLI only`` in the
help; ``--serve`` ignores them and uses its own defaults.

Diagram types
-------------

Activity and swimlane
~~~~~~~~~~~~~~~~~~~~~

An activity diagram is a graph of nodes and edges. A node belongs to the lane
it is declared in, and edges may cross lanes. Links (``ref``) open another
diagram in an HTML bundle, or become anchors in standalone SVG. From
``examples/hello.diag``:

.. include:: examples/hello.diag
   :literal:
   :start-line: 5
   :end-line: 8

Edge operators need surrounding whitespace: write ``A -> B``, not ``A->B``.
The complete grammar is in `docs/dsl.rst <docs/dsl.rst>`_.

Sequence
~~~~~~~~

A sequence diagram shows participants, messages, replies, notes and frames.
From ``examples/sequence/checkout.seq``:

.. include:: examples/sequence/checkout.seq
   :literal:
   :start-line: 5
   :end-line: 25

The complete grammar is in `docs/sequence-dsl.rst <docs/sequence-dsl.rst>`_.

Tree
~~~~

A tree diagram draws a hierarchy as one SVG. Nodes with children can be folded
in an HTML bundle or in the live preview. From ``examples/tree/org-chart.tree``:

.. include:: examples/tree/org-chart.tree
   :literal:
   :start-line: 4
   :end-line: 28

The complete grammar is in `docs/tree-dsl.rst <docs/tree-dsl.rst>`_.

Treemap
~~~~~~~

A treemap draws a hierarchy as nested rectangles whose areas follow a number.
From ``examples/treemap/budget.tmap``:

.. include:: examples/treemap/budget.tmap
   :literal:
   :start-line: 0
   :end-line: 20

The complete grammar is in `docs/treemap-dsl.rst <docs/treemap-dsl.rst>`_.

Manifest
~~~~~~~~

A manifest shows which rows belong to which columns, like a spreadsheet with a
mark in every member cell. From ``examples/manifest/editions.mfst``:

.. include:: examples/manifest/editions.mfst
   :literal:
   :start-line: 3
   :end-line: 32

The manifest type is used for the manifests and gits of `Code in Motion (cim)
<https://analogdevicesinc.github.io/cim>`_, a tool for managing code
manifests. Columns can extend other columns; inherited, overridden and
removed rows get different marks. The complete grammar is in
`docs/manifest-dsl.rst <docs/manifest-dsl.rst>`_.

Live preview
------------

``swimflow --serve`` (or ``make serve``) serves a directory of diagram files
as an editable browser workspace. It lists the files, shows the selected
source next to its rendered output and re-renders after a short pause. Every
diagram type gets its own tab. Edits stay in browser session buffers; the
server never writes to the source files. **Download** exports the current view
in the chosen format. **Expand** (or Escape to restore) lets the rendered
diagram fill the window below the toolbar, hiding the file list and editor.

.. code-block:: console

   $ cp .env.example .env
   $ swimflow --serve

Set ``SWIMFLOW_TOKEN`` on untrusted networks. ``SWIMFLOW_SCALE`` controls the
raster scale of downloaded PNGs (default ``1.0``, the live-preview equivalent
of ``--scale``). Configuration is read from ``.env``; process environment
variables override it.

Compatibility
-------------

* The parser accepts every file the reference implementation accepts and
  rejects the rest.
* Integer-grid geometry and pixel boxes match the reference exactly.
* New syntax is additive and does not change how existing files parse.
* The docutils/Sphinx options ``:width:``, ``:height:`` and ``:scale:`` are
  not supported; use ``--scale`` for raster scaling (or ``SWIMFLOW_SCALE``
  for downloads from the live preview).
* The committed corpora in ``tests/*_compat.rs`` check this without Python.

Development
-----------

.. code-block:: console

   $ cargo fmt --check
   $ cargo clippy --all-targets -- -D warnings
   $ cargo test

Further documentation
---------------------

* `docs/ <docs/>`_ — language references and architecture
* `examples/README.rst <examples/README.rst>`_ — a runnable tour of every type
* `AGENTS.md <AGENTS.md>`_ — working guide for contributors
* `man/swimflow.1 <man/swimflow.1>`_ — manual page
* `completions/swimflow.bash <completions/swimflow.bash>`_ — bash completion
* `editors/nvim/README.rst <editors/nvim/README.rst>`_ — Neovim support

Acknowledgements
----------------

The activity/swimlane language and its layout model come from `actdiag
<https://github.com/blockdiag/actdiag>`_ (part of the blockdiag family), which
is no longer maintained. swimflow keeps those files working while adding new
diagram types, PNG/PDF output and interactive HTML.

License
-------

Apache-2.0. See `LICENSE <LICENSE>`_.
