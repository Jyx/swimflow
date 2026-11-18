The treemap diagram DSL
=======================

.. note::

   Other languages: `dsl.rst <dsl.rst>`_ (activity),
   `sequence-dsl.rst <sequence-dsl.rst>`_ (sequence),
   `tree-dsl.rst <tree-dsl.rst>`_ (tree).

A treemap draws a hierarchy as nested rectangles whose **areas are
proportional to a number**. It is a *diagram type* of its own (see
`plugin-architecture.rst <plugin-architecture.rst>`_) that lives in ``.tmap``
files and uses the same fonts, palette and output formats as the activity,
sequence and tree diagrams. The whole treemap is drawn as **one SVG**.

The defining difference from the tree diagram is the extra **numbered
parameter**: every item has a name *and* a value, and the value decides how
much room the item gets.

1. Grammar
----------

.. code-block:: text

    document  := treemap*
    treemap   := 'treemap' [name] '{' statement* '}'
    statement := 'class' name [attrs] ';'        # a reusable class
               | 'root' [name] [attrs] ( ';' | '{' node* '}' )
               | word '=' value ';'              # a diagram attribute
               | node
    node      := name [ ':' number ] [attrs] ( ';' | '{' node* '}' )
    number    := digit+ [ '.' digit+ ]
    attrs     := '[' [ attr ( ',' attr )* ] ']'
    attr      := word [ '=' value ]

* ``name`` and ``value`` are a bare word or a quoted string, exactly as in the
  tree DSL. A bare name may contain ``-`` and ``.``; quote anything else.
* Comments are ``# …``, ``// …`` and ``/* … */``.
* A ``.tmap`` file may hold several ``treemap`` blocks; each is rendered as
  its own diagram.
* The value follows the name after a **colon**:
  ``Rent: 1200;``, ``Paris: 2.5;``,
  ``"Build & CI": 42 [color = "#e8f0ff"];``. The colon is the only value
  separator; ``=`` stays the assignment operator of attributes and diagram
  settings (``width = 800;``). ``Rent = 1200;`` is therefore a diagram
  attribute, not a node with a value.
* A pure number is still a legal *name*: ``2024;`` is a node called ``2024``
  with no explicit value. ``2024: 12;`` is the node ``2024`` with value
  ``12``. A name that contains a colon or spaces must be quoted:
  ``"Q4: Europe": 12;``.
* A node without a number takes 1 when it has no children, and the sum of its
  children when it has children. See section 2.
* ``class NAME [attrs];`` defines reusable attributes. A node applies them
  with ``[class = NAME]``; attributes written on the node win. Classes may be
  defined before or after their use. ``class;``, ``class [..];`` and
  ``class: 2;`` are still ordinary nodes called ``class``.

The syntax is deliberately close to the tree node syntax: adding ``: number``
gives the same shape a value. The colon also removes the ambiguity that a
positional number would have with numeric node names.

2. Values
---------

Every node has a **weight**:

.. list-table::
   :header-rows: 1

   * - Form
     - Weight
   * - ``Rent: 1200;``
     - ``1200``
   * - ``Rent;`` (leaf)
     - ``1``
   * - ``Housing { Rent: 1000; Utilities: 500; }``
     - ``1500`` (the sum)
   * - ``Housing: 1500 { Rent: 1000; Utilities: 500; }``
     - ``1500`` (explicit wins)
   * - ``Housing: 2000 { Rent: 1000; Utilities: 500; }``
     - ``2000`` among its siblings; its children still fill its interior

* Values are non-negative finite decimals. Negative, ``NaN`` and infinite
  values are rejected with a line/column error. ``0`` is legal but draws no
  area (a warning is a good idea).
* A parent's weight only decides **its own share among its siblings**. The
  children are laid out inside the parent's interior and always fill it; an
  explicit parent value is not required to equal the sum of its children.
  This keeps the syntax forgiving and makes an explicit aggregate value
  meaningful.
* The root is the whole canvas; its own weight is irrelevant. Its children
  share the interior.

This gives both a flat list and a nested hierarchy:

.. code-block:: text

    treemap "Disk usage" {
      root "/" {
        usr: 41.5;
        home: 28;
        var: 18.5;
        opt: 12;
      }
    }

.. code-block:: text

    treemap "Budget" {
      class housing [color = "#cfe8ff"];
      root "Monthly budget" {
        Housing: 1500 [class = housing] {
          Rent: 1000 [class = housing];
          Utilities: 500 [class = housing];
        }
        Food: 600 {
          Groceries: 420;
          Restaurants: 180;
        }
        Savings: 400;
        Transport: 200;
      }
    }

3. Attributes
-------------

Node attributes are the tree attributes that make sense for a rectangle:

.. list-table::
   :header-rows: 1

   * - Attribute
     - Meaning
   * - ``label``
     - text to draw instead of the name
   * - ``class``
     - apply the attributes of a ``class NAME [attrs];`` definition
   * - ``color``
     - fill (default: ``#ffff99`` for the root and containers, ``#ffffff`` for
       leaves)
   * - ``textcolor``
     - text color (default black)
   * - ``linecolor``
     - outline (default black)
   * - ``href``
     - external link (dropped with ``--no-external``)
   * - ``target``
     - where an ``href`` opens, as in HTML: ``_blank``, ``_self``, or a window
       name
   * - ``showvalue``
     - ``false`` hides the value for this node (default ``true``; the bundle
       adds a toggle for all values)
   * - ``ref``
     - reserved; not implemented

Unknown attributes are preserved. ``style``, ``shape`` and ``edgestyle`` are
tree-only and have no meaning here (they are accepted and ignored, as unknown
attributes are elsewhere).

Diagram attributes:

.. list-table::
   :header-rows: 1

   * - Attribute
     - Meaning
   * - ``width``
     - canvas width in pixels (default ``800``)
   * - ``height``
     - canvas height in pixels (default ``500``)
   * - ``showvalue``
     - default for every node (default ``true``)

A first version does not need ``orientation``; a treemap always grows from the
top-left and is laid out by area, not by levels.

Classes
~~~~~~~

``class NAME [attrs];`` defines a reusable set of attributes, exactly like the
activity diagram's classes. Apply it with ``[class = NAME]``. The class
provides defaults and the node's own attributes override them; an unknown
class is an error.

.. code-block:: text

    treemap "Budget" {
      class housing [color = "#cfe8ff", linecolor = "#33415c"];
      class food [color = "#d9f2d9", linecolor = "#2e7d32"];

      root "Monthly budget";

      Housing: 1500 [class = housing] {
        Rent: 1000 [class = housing];
        Utilities: 320 [class = housing];
      }
      Food: 620 [class = food, textcolor = "#14532d"];
    }

Classes are resolved before the labels and weights are computed, so a class
may also carry ``label``, ``showvalue``, ``href`` or ``target``. A class that
sets ``label`` renames every node that uses it, which is useful for badges but
rarely what a treemap wants.

4. Layout
---------

The layout is a **squarified treemap** (Bruls, Huizing and van Wijk): within a
rectangle, children are sorted by weight descending (ties keep source order)
and packed into rows so that the rectangles stay as close to square as
possible.

Rules, all deterministic:

1. Start with the canvas rectangle (after the outer margin); the root's header
   band is reserved at the top.
2. Sort the children by weight descending, then by source order.
3. Add children to the current row while the worst aspect ratio improves;
   commit the row and continue with the remaining rectangle otherwise.
4. Convert the weights to integer pixel areas with cumulative rounding
   (``floor(cumulative * area / total)`` differences), so the children exactly
   fill the parent and no gap appears between siblings.
5. For a container, reserve a **header band** (for its label), then lay out
   its children in the interior with the same algorithm.
6. Every rectangle stays inside its parent; boxes never overlap.

Suggested constants, aligned with the tree diagram:

.. list-table::
   :header-rows: 1

   * - Constant
     - Value
   * - outer margin
     - 24 px
   * - container header
     - 26 px
   * - inner padding
     - 6 px
   * - gap between siblings
     - 3 px
   * - minimum label box
     - 40 × 18 px

Labels that do not fit are truncated with **``...``** (three ASCII dots, not
the Unicode ellipsis); the full name and value are always available in an SVG
``<title>``. A leaf whose box is smaller than the minimum label box gets no
text. Values are drawn on a second line below the name when there is room, and
are formatted with the shared ``core::svg::fmt`` number formatting. Values are
shown by default; ``showvalue = false`` suppresses the value of one node.

5. Theme
--------

The treemap uses the same palette as the activity/tree drawings, so a page
with several diagram types looks like one family:

.. list-table::
   :header-rows: 1

   * - Element
     - Color
   * - root and container fill
     - ``#ffff99`` (the activity lane header / tree root yellow)
   * - leaf fill
     - ``#ffffff``
   * - outline
     - ``#000000``
   * - label text
     - ``#000000``

Depth is expressed by stroke width and inset, not by new colors: the root is
drawn with 2 px, nested containers with 1 px. A ``color`` attribute overrides
the fill of that node only; children are unaffected.

The sequence diagram uses a cooler accent (``#eef2f7`` / ``#33415c``). We do
**not** introduce a third palette for treemaps: the tree/activity palette is
the closest match to "a box with a label", and a treemap is exactly that.

6. Rendering
------------

Output is one SVG document, so PNG, PDF and the HTML bundle come for free
through the existing core:

.. code-block:: text

    <g class="swimflow-treemap">
      <g class="treemap-node" data-depth="0" data-value="…">
        <rect …/>
        <text class="treemap-label">…</text>
        <text class="treemap-value">…</text>
        <title>Name: value</title>
        … children …
      </g>
    </g>

Drawing order is parents before children, so a child's rectangle sits on top
of the parent's interior. The group structure and ``data-*`` attributes leave
room for the later ``ref``/fold feature without changing the SVG contract.

The value toggle
~~~~~~~~~~~~~~~~

A standalone SVG, PNG or PDF always shows the values; it has no controls. In
an HTML bundle (and therefore in the live preview) a
``.swimflow-toolbar`` in the top bar, next to the zoom controls, carries one
button:

.. code-block:: html

    <button type="button" class="treemap-values-toggle" aria-pressed="true">Hide values</button>

The section ships ``runtime.js`` through ``Section::with_script`` and the
toolbar through ``Section::with_toolbar``. The script toggles the
``style.display`` of every ``.treemap-value`` text inside its own section,
updates ``aria-pressed`` and changes the label between **Hide values** and
**Show values**. The SVG is not re-laid out, so the picture and its zoom stay
exactly where they were. A node with ``showvalue = false`` has no
``.treemap-value`` text at all and stays valueless under the toggle.

The toggle is a **view setting**: it changes only the live page. Unlike folded
trees, it is not sent with a download, so an export always draws the values the
source asks for (``showvalue``).

Zoom
~~~~

The core bundle already applies ``core/zoom.js`` to every section: fit to the
window, the ``+ - 0 1`` keys, Ctrl/Cmd + wheel and the zoom bar. A treemap is
just another SVG section, so zoom works there with **no treemap-specific zoom
code**.
