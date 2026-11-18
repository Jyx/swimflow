swimflow examples
=================

Small, runnable diagram files that show the language and the output modes.
Most of them are ``.diag`` activity/swimlane diagrams; the ``sequence/``
directory holds ``.seq`` sequence diagrams, the ``tree/`` directory holds
``.tree`` tree diagrams, the ``treemap/`` directory holds ``.tmap`` treemaps
and the ``manifest/`` directory holds ``.mfst`` manifest diagrams, which are
different diagram types (plugins). Every file here is covered by
``tests/examples.rs``, which builds and renders it (SVG and HTML) to catch
syntax or layout regressions.

From the repository root, ``make examples`` builds the release binary and
renders every file here to SVG; ``make examples TYPE=html`` renders HTML
bundles instead (``TYPE=png`` and ``TYPE=pdf`` also work). ``make clean``
removes the generated output.

To run individual examples, build the binary first:

.. code-block:: console

   cargo build --release
   alias swimflow="$PWD/target/release/swimflow"

Then run any example directly. With no ``-o``, the output is written next to
the input with the format's extension:

.. code-block:: console

   swimflow examples/hello.diag                 # writes examples/hello.svg
   swimflow examples/deployment.diag -f png --scale 2 -o deployment.png
   swimflow examples/incident-response.diag -f pdf -o incident-response.pdf

What each example shows
-----------------------

.. list-table::
   :header-rows: 1

   * - File
     - Scenario
     - Features
   * - ``hello.diag``
     - the smallest useful flow
     - one diagram, one edge
   * - ``swimlanes.diag``
     - a hand-off across three teams
     - named lanes, cross-lane edges
   * - ``colors.diag``
     - custom palette for lanes and nodes
     - lane header ``color``, node
       ``class``/``color``/``linecolor``/``textcolor``
   * - ``deployment.diag``
     - CI/CD with an approval gate and rollback
     - classes, legend, edge labels, topological numbers
   * - ``incident-response.diag``
     - an on-call runbook with a feedback loop
     - cyclic graph, dashed edges, classes, legend
   * - ``order-processing.diag``
     - order fulfilment with an exception path
     - branching/merging, retry loop, topological numbers
   * - ``numbering-auto.diag``
     - number nodes in file order
     - ``numbering = auto``, scrambled declaration order
   * - ``numbering-topological.diag``
     - number nodes along the arrows
     - ``numbering = topological``, Kahn's algorithm
   * - ``numbering-depth.diag``
     - number nodes by process stage
     - ``numbering = depth``, parallel steps share a stage
   * - ``state-machine.diag``
     - a simple state machine
     - ``->``, ``<-``, ``<->``, ``--``; no explicit lanes
   * - ``dependency-graph.diag``
     - a Graphviz-style dependency DAG
     - styled classes, right-hand legend, topological numbers
   * - ``autolane.diag``
     - deriving lanes from node-name prefixes
     - ``plugin autolane``, ``dev_*`` / ``ops_*`` nodes
   * - ``unicode.diag``
     - a multilingual workflow
     - unicode names, quoted ids, multiline labels
   * - ``zoom-demo.diag``
     - four-level drill-down / zoom
     - multiple diagrams, ``ref``, bundle navigation, a cycle
   * - ``cross-file/overview.diag``
     - drill-down across files
     - ``ref = "details.diag#details"`` and transitive loading
   * - ``sequence/checkout.seq``
     - an order checkout
     - sequence diagram: participants, aliases, messages, replies, a note, a
       frame
   * - ``sequence/chat.seq``
     - the four message operators
     - ``->``, ``->>``, ``-->``, ``--``, a self message, a note
   * - ``sequence/frames.seq``
     - nested frames
     - ``loop``, ``alt``/``else``, ``par``, frames closed with ``end``
   * - ``tree/org-chart.tree``
     - a company organization
     - nested children, per-node ``color``
   * - ``tree/file-system.tree``
     - a directory layout
     - ``orientation = landscape;``, ``shape``, ``style``
   * - ``tree/decision-tree.tree``
     - a yes/no decision tree
     - ``edge`` labels on the connectors
   * - ``tree/product-map.tree``
     - two trees in one file
     - several ``tree`` blocks, ``href`` and ``ref`` links, ``collapsed``
   * - ``treemap/budget.tmap``
     - a monthly budget as nested rectangles
     - values (areas), nested containers, reusable ``class`` colors
   * - ``treemap/disk-usage.tmap``
     - disk usage with decimal values
     - classes, flat and nested entries, custom ``width``/``height``
   * - ``manifest/editions.mfst``
     - features per product edition
     - column ``group`` values, ``extends`` chains,
       ``remove``/``override``, ``rowgroup``, ``class``, ``sort = source``
   * - ``manifest/repositories.mfst``
     - repositories per build target
     - ``extends`` chains across groups, ``remove``/``override`` with notes

Drill-down and zoom
-------------------

``zoom-demo.diag`` is the showcase for the headline feature. It continues the
``tests/fixtures/drilldown.diag`` idea with four levels:

.. code-block:: console

   swimflow examples/zoom-demo.diag \
     --bundle examples/zoom-demo.html --embed-font

Open ``zoom-demo.html`` and click **Review changes → Approve → Deploy → Back
to overview**. The last link is a cycle, which the bundle handles with a
history stack. The same file also works as a normal SVG; the linked nodes
become ``#diagram`` anchors, with their labels underlined to signal that they
are clickable.

Cross-file references work the same way:

.. code-block:: console

   swimflow examples/cross-file/overview.diag \
     --bundle examples/cross-file/overview.html

The bundle loads ``details.diag``, inlines both diagrams and keeps a single
self-contained HTML file. Use ``--no-external-files`` to skip loading
referenced files, or ``--allow-broken-links`` to render an unresolved
reference as disabled.

Sequence diagrams
-----------------

The ``sequence/`` directory holds sequence diagrams. They use their own file
extension (``.seq``) and their own small language, but the same binary, the
same fonts and the same output formats:

.. code-block:: console

   swimflow examples/sequence/checkout.seq           # writes examples/sequence/checkout.svg
   swimflow examples/sequence/checkout.seq -f png -o checkout.png

The language:

.. include:: sequence/checkout.seq
   :literal:
   :start-line: 5
   :end-line: 25

A frame may also be written with braces (``opt { ... }``). The diagram type is
detected from the extension or from the content; ``--type sequence`` forces
it. The complete grammar is in `../docs/sequence-dsl.rst
<../docs/sequence-dsl.rst>`_.

Tree diagrams
-------------

The ``tree/`` directory holds tree diagrams: a root with nested children,
drawn as one SVG in the same fonts and colors as the other types. They use the
``.tree`` extension:

.. code-block:: console

   swimflow examples/tree/org-chart.tree             # writes examples/tree/org-chart.svg

.. include:: tree/org-chart.tree
   :literal:
   :start-line: 4
   :end-line: 28

Node attributes: ``label``, ``color``, ``textcolor``, ``linecolor``,
``shape = roundedbox``, ``style = dashed``, ``href``, ``target = _blank``
(open the link in a new tab, as in HTML), ``edge`` (a label on the connector
into the node) and ``edgestyle = dashed`` (the connector into the node is
dashed until it merges with a solid one). ``orientation = landscape;`` grows
the tree from left to right.

Every node with children has a small circle where its connector leaves it:
click it (in the live preview or in an HTML bundle) to fold the branch, and
again to unfold it. The **Collapse all** and **Expand all** buttons in the top
bar fold or unfold every branch at once. A minus means the
children are shown, a plus means they are folded away; ``collapsed`` starts a
branch folded. ``ref = "other"`` makes a node open another ``tree`` block of
the same file (the bundle's back button returns).

The targets and releases of cim manifest repositories can be drawn as a tree
with `../scripts/cim-tree.py
<../scripts/cim-tree.py>`_ (PyYAML, set up as for the `manifest
script <#manifest-diagrams>`_). With one source the tree is rooted at the
repository; with several, an artificial root ``manifests`` (``--root NAME``)
gets one subtree per repository. Below a repository, a target hangs under the
target it ``extends:``, and its releases, the git branches named
``<target>-<version>`` (cached ``origin`` refs, ``--live`` asks the remote),
are dashed nodes under a ``releases`` node:

.. code-block:: console

   scripts/cim-tree.py path/to/manifests -o targets   # writes targets.tree
   scripts/cim-tree.py first/ second/ -o all.tree
   scripts/cim-tree.py https://example.com/org/manifests.git -o upstream
   scripts/cim-tree.py https://example.com/org/manifests.git#develop local/ -o both
   scripts/cim-tree.py manifests -m 'base*' -X '*-rc.*' -o some.tree

A source can be a git URL (``https://``, ``ssh://``, ``git@host:path``,
``file://``, optionally ``#BRANCH``): it is shallow-cloned into a temporary
directory and its branches are listed with ``git ls-remote``, so the tree
shows what upstream has rather than what a local copy has.

``-m`` / ``-x`` keep or drop targets and ``-M`` / ``-X`` keep or drop
versions, by comma-separated shell patterns; ``--version-pattern`` (default
``v[0-9]*``) says what a version looks like and ``--no-releases`` leaves them
out.

Treemap diagrams
----------------

The ``treemap/`` directory holds treemaps: nested rectangles whose areas
follow a number. They use the ``.tmap`` extension and the same fonts, palette
and output formats as the other types:

.. code-block:: console

   swimflow examples/treemap/budget.tmap            # writes examples/treemap/budget.svg
   swimflow examples/treemap/budget.tmap --bundle budget.html --embed-font

.. include:: treemap/budget.tmap
   :literal:
   :start-line: 0
   :end-line: 20

A leaf without a number counts 1 and a container without one uses the sum of
its children. An explicit container value only decides its own share among its
siblings; its children still fill its interior. ``width = 760;`` and
``height = 460;`` set the canvas (default ``800 × 500``). Values are drawn by
default; ``showvalue = false`` hides one, and an HTML bundle gets a **Hide
values** / **Show values** button in the top bar.

Node attributes: ``label``, ``color``, ``textcolor``, ``linecolor``,
``href``, ``target = _blank`` and ``showvalue``. ``class NAME [attrs];``
defines reusable attributes; apply them with ``[class = NAME]`` and override
individual values on the node. Labels that do not fit are shortened with
``...``. The complete grammar is in `../docs/treemap-dsl.rst
<../docs/treemap-dsl.rst>`_.

Manifest diagrams
-----------------

The ``manifest/`` directory holds manifest diagrams: which *rows* belong to
which *columns*, drawn like a spreadsheet with a dot in every member cell.
They use the ``.mfst`` extension:

.. code-block:: console

   swimflow examples/manifest/editions.mfst                 # writes examples/manifest/editions.svg
   swimflow examples/manifest/editions.mfst --bundle editions.html --embed-font

.. include:: manifest/editions.mfst
   :literal:
   :start-line: 3
   :end-line: 32

A column that ``extends`` another one inherits its rows (hollow dots);
``remove`` drops an inherited row (a grey cross) and ``override`` keeps it but
marks it changed (a ringed dot). Chains of ``extends`` are followed. The count
column on the right shows how many columns each row is in, and rows are sorted
by it (``sort = count``, ``name`` or ``source``). In an HTML bundle hovering
a cell highlights its row and column; the cross next to a row or column hides
it, a legend entry hides every mark of its kind, clicking a column header
shows only that column's rows, clicking a row label shows only that row's
columns, clicking a cell does both, and **Show all** undoes it. A hidden row
or column folds to a thin strip with a ``+`` that brings it back, and a
live-preview download leaves it out. The removed marks start hidden
(``hidemarks = none;`` shows them).

A ``.mfst`` file can be generated from the ``sdk.yml`` files of a cim manifest
repository by `../scripts/cim-mfst.py
<../scripts/cim-mfst.py>`_: swimflow itself knows nothing about
manifests. The script reads YAML with PyYAML, installed once into a virtual
environment:

.. code-block:: console

   python3 -m venv scripts/.venv
   scripts/.venv/bin/pip install -r scripts/requirements.txt
   source scripts/.venv/bin/activate
   scripts/cim-mfst.py path/to/manifests -o gits          # writes gits.mfst
   scripts/cim-mfst.py first/ second/ --title "Gits" -o all.mfst
   scripts/cim-mfst.py manifests -m base,product-a -o some.mfst
   scripts/cim-mfst.py manifests -x 'test-*' -o most.mfst

A git ``name`` with a path (``ws/zephyr``, checked out in a subfolder) becomes
the row after its last ``/`` (``zephyr``), so the same git checked out in
different places is one row; ``--full-names`` keeps the whole name.

``-m`` / ``--match`` keeps only the targets whose name matches one of the
comma-separated shell patterns (``*``, ``?``, ``[...]``; a plain name matches
exactly) and ``-x`` / ``--exclude`` drops them. A kept target that extends a
dropped one shows the dropped target's gits as its own.

Every folder becomes a column group and every ``sdk.yml`` below it a column;
``extends`` and the overlay's ``remove`` / ``modify`` become inherited,
removed and overridden cells. With several folders a column is called
``FOLDER/TARGET``, so targets with the same name do not clash. The complete
grammar is in `../docs/manifest-dsl.rst <../docs/manifest-dsl.rst>`_.

Syntax notes
------------

* Edge operators need surrounding whitespace: ``A -> B``, not ``A->B``.
* A node belongs to the lane it is declared in; edges may cross lanes.
* A lane header takes its colour from the lane's ``color``; put it on the
  block (``lane "CI" [color = "#c8e6c9"] { ... }``), on a separate declaration
  (``lane "CI" [color = "#c8e6c9"];``), or inside the block
  (``color = "...";``).
* Node fills use ``color``; ``linecolor`` sets the outline and ``textcolor``
  the label. Define reusable ``class NAME [attrs]`` and attach with
  ``[class = NAME]`` (see ``colors.diag``).
* Diagram attributes (``orientation``, ``numbering``, ``node_width``, …) are
  statements such as ``orientation = landscape;``.
* ``class NAME [attrs]`` defines reusable styling; attach it with
  ``A [class = NAME]``.
* ``ref`` points at another diagram (``#detail``, ``other.diag``,
  ``other.diag#detail``); ``href`` points at an external URL. They are
  mutually exclusive. Treemaps support ``href``; their ``ref`` drill-down is a
  backlog feature.
* Unknown attributes are preserved, so examples can use newer attributes
  without breaking older binaries.

See `../docs/ <../docs/>`_ for the complete grammars and `../README.rst
<../README.rst>`_ for all command-line options.
