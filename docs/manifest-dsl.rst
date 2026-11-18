The manifest diagram DSL
========================

.. note::

   Examples in `../examples/manifest/ <../examples/manifest/>`_.

1. Purpose
----------

A manifest answers "which elements belong to which sets?" It is drawn like a
spreadsheet: one **column** per set, one **row** per element, and a mark in
every cell where the row belongs to the column. It adds what an Euler diagram
shows (overlap) and what a treemap shows (grouping) without their limits: it
stays readable with dozens of sets, and every element appears once.

Two things are built in because they are common in real data:

* **Column groups**: a band over several columns (for example the repository
  a set of targets comes from).
* **Inheritance**: a column may ``extends`` another one. It starts from the
  parent's rows, can ``remove`` some and ``override`` others. The cell shows
  *why* a row is there.

The diagram type is generic. It does not know what the rows and the columns
stand for, and it reads nothing but its own source. Data from elsewhere (for
example the ``gits:`` of cim ``sdk.yml`` manifests) is converted to a
``.mfst`` file by a separate tool (``scripts/cim-mfst.py``).

2. Example
----------

.. code-block:: text

    manifest "Products" {
      title = "Components per product";

      group "Line" {
        column base { core; ui; net; }
        column lite [extends = base] { remove net; }
        column pro  [extends = base] { override ui [note = "v2"]; cloud; }
      }
      column other { core; }

      row docs: base, other;
      rowgroup "Tools" { row cli: pro; }
    }

3. Grammar
----------

The tokens (words, strings, comments ``#``, ``//``, ``/* */``) are those of
the treemap; the scanner is shared.

.. code-block:: text

    document := manifest*
    manifest   := 'manifest' [name] '{' statement* '}'
    statement:= 'class' name [attrs] ';'
              | word '=' value ';'                       # diagram attribute
              | 'group' name [attrs] '{' column* '}'
              | column
              | 'rowgroup' name [attrs] '{' row* '}'     # 'row' optional inside
              | 'row' row
    column   := 'column' name [attrs] ( ';' | '{' entry* '}' )
    entry    := [ 'remove' | 'override' ] name ( ',' name )* [attrs] ';'
    row      := name [attrs] [ ':' name ( ',' name )* ] ';'
    attrs    := '[' [ attr ( ',' attr )* ] ']'

A keyword followed by ``;``, ``[`` or ``:`` is a name (``remove;`` is a row
called "remove"), as in the treemap.

Membership can be written per column (``column a { x; y; }``), per row
(``row x: a, b;``) or both; the result is the same. Rows appear in order of
first mention. A column named in a ``row`` statement or in ``extends`` may be
defined later in the file.

4. Resolution
-------------

Columns are resolved parents first. For a column with ``extends = P``:

1. every row present in ``P`` becomes **inherited** (the ``from`` column is
   the one where it was added directly, so chains point at the origin);
2. the column's own entries are applied in order:

   * ``name;`` — **direct** member;
   * ``remove name;`` — **removed** (kept as a trace, not a member);
   * ``override name;`` — **overridden** (still a member).

Errors, with the line: an unknown column, an unknown ``extends`` target, an
``extends`` cycle, a duplicate column, an unknown class, and ``remove`` or
``override`` of a row the column does not inherit.

The **count** of a row is the number of columns where it is a member (direct,
inherited or overridden).

5. Attributes
-------------

.. list-table::
   :header-rows: 1

   * - Where
     - Attribute
     - Meaning
   * - diagram
     - ``title``
     - a title above the manifest
   * - diagram
     - ``sort``
     - row order inside each row block: ``count`` (default, most shared first,
       ties in source order), ``name``, ``source``
   * - diagram
     - ``showcount``
     - ``false`` hides the count column
   * - diagram
     - ``countlabel``
     - the heading of the count column (default ``count``)
   * - diagram
     - ``legend``
     - ``false`` hides the legend (shown when at least two kinds of mark are
       used)
   * - diagram
     - ``hidemarks``
     - the kinds of mark hidden until the user shows them: comma separated,
       default ``removed``; ``none`` shows every mark. A file leaves them out;
       a bundle draws them hidden, with a greyed legend entry that shows them
   * - column
     - ``extends``
     - the parent column
   * - column, row, group, rowgroup
     - ``label``, ``color``, ``textcolor``
     - text and fill
   * - group
     - ``shape``
     - ``roundedbox`` rounds the corners of the band (default ``box``)
   * - column, row
     - ``href``
     - an external link on the header / label
   * - column, row
     - ``markcolor``
     - the colour of the marks (the row's wins)
   * - entry
     - ``note``
     - extra text in the cell tooltip (e.g. what an override changes)
   * - any
     - ``class``
     - apply a ``class NAME [attrs];`` definition; explicit attributes win

6. Layout
---------

Top to bottom: title, column-group band, inheritance band, rotated column
headers, rows, legend. Columns are one cell (30 px) wide and rows one line
(26 px) tall, so the size grows linearly with the data. The row labels are as
wide as the longest label; the column header is as tall as the longest column
label (the text is rotated a quarter turn).

The inheritance band draws one bracket per ``extends``: up from the child,
across, and down to the parent, with a hollow triangle at the parent (a UML
generalization). Brackets are placed on tracks greedily, left to right, so two
brackets whose spans touch never share a track.

Row groups are blocks of rows under a yellow header row; ungrouped rows come
first.

7. Marks
--------

.. list-table::
   :header-rows: 1

   * - Mark
     - Drawing
   * - direct
     - a filled dot
   * - inherited
     - a hollow dot
   * - overridden
     - a small filled dot inside a ring
   * - removed
     - a grey cross

Every cell has a tooltip: "ROW in COLUMN (inherited from P): note".

8. Bundle behaviour
-------------------

In an HTML bundle the manifest is drawn *interactive*: a small cross sits next
to every row label and on every column header (shown on hover), and the legend
entries are clickable. ``runtime.js``, included once per page:

* hovering a cell, a row label or a column header sets ``data-active`` on the
  row and on the column's cells/header (the page CSS shades them);
* the cross of a row or a column **folds** it into a thin strip (12 px) with a
  ``+`` that unfolds it: a row's ``+`` sits in the margin left of the table, a
  column's at the top of its strip. Everything below or right of it moves to
  close the gap and the canvas shrinks. The SVG records the original geometry
  (``data-x``, ``data-y``, ``data-w``, ``data-h``, ``data-edge`` on the
  separators, ``data-track``/``data-base`` on the brackets), so ``fold()``
  only moves, resizes and hides elements; there is no second layout in
  JavaScript, and unfolding restores the original exactly. A bracket with a
  folded end is hidden;
* a legend entry hides every mark of its kind and is greyed out while they are
  hidden; a second click shows them again;
* clicking a column header (outside its cross) dims every row that is not a
  member of the column; clicking it again shows every row. A header with an
  ``href`` follows the link instead;
* clicking a row label dims every column the row is not a member of (and its
  cells); clicking it again shows every column. A linked label follows the
  link instead;
* clicking a cell does both: it dims the rows not in its column and the
  columns its row is not in; the download sends ``only:rN`` and ``only:cM``;
* **Show all** in the toolbar undoes every hide and the filter.

The state lives on the section: ``data-manifest-hidden`` holds tokens and
``data-manifest-filter`` the filtered column or row (``c2``, ``r4``,
``r4 c2``). Until the first change the tokens are the defaults on the SVG
(``data-hidden-default``, from ``hidemarks``). ``tests/manifest_runtime.js``
checks the behaviour against the shared fake DOM.

9. Downloads follow the view
----------------------------

The live preview sends the state with a download (``viewBody()`` in
``src/server/web.rs``) as ``view:hidden:<diagram key>=<tokens>``, read into the
generic ``ViewState::hidden``. The tokens:

.. list-table::
   :header-rows: 1

   * - Token
     - Meaning
   * - ``rN``
     - hide row ``N`` (its index in source order, as ``data-row``)
   * - ``cN``
     - hide column ``N`` (as ``data-col``)
   * - ``m:KIND``
     - do not draw marks of a kind: ``direct``, ``inherited``, ``override``,
       ``removed``
   * - ``only:cN``
     - hide every row that is not a member of column ``N`` (the filter)
   * - ``only:rN``
     - hide every column that row ``N`` is not a member of (the row filter)

``Diagram::apply_view`` applies them before the layout, so an exported SVG,
PNG or PDF **leaves out** what is hidden and closes the gaps: hidden columns
take no room and lose their ``extends`` brackets, hidden rows are dropped, a
hidden mark kind is neither drawn nor listed in the legend, and the counts are
recomputed over the visible columns. A row that was in some column but in no
visible one is left out too, rather than shown empty with a count of 0 (a row
the source gives no columns stays). Unknown tokens are ignored. Before the
user changes anything the page sends the defaults, so a download matches what
the page shows.
