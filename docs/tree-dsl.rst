The tree diagram DSL
====================

.. note::

   Other languages: `dsl.rst <dsl.rst>`_ (activity),
   `sequence-dsl.rst <sequence-dsl.rst>`_ (sequence).

A tree diagram draws a hierarchy: a root, its children, their children and so
on. It is a *diagram type* of its own (see
`plugin-architecture.rst <plugin-architecture.rst>`_) that lives in ``.tree``
files (``.treediag`` works too) and uses the same fonts, colors and output
formats as the other types. The whole tree is drawn as **one SVG**.

1. Grammar
----------

.. code-block:: text

    document  := tree*
    tree      := 'tree' [name] '{' statement* '}'
    statement := 'root' [name] [attrs] ( ';' | '{' node* '}' )
               | word '=' value ';'              # a diagram attribute
               | node
    node      := name [attrs] ( ';' | '{' node* '}' )
    attrs     := '[' [ attr ( ',' attr )* ] ']'
    attr      := word [ '=' value ]

* ``name`` and ``value`` are a bare word (``Core``, ``#``-free, may contain
  ``-`` and ``.``) or a quoted string (``"Build & CI"``).
* Comments are ``# …``, ``// …`` and ``/* … */``.
* A ``tree`` file may hold several ``tree`` blocks; each is rendered as its
  own diagram.
* ``root;``, ``root [..]`` and ``"root"`` are ordinary nodes called "root";
  only ``root`` followed by a name or ``{`` is the root statement. At most one
  per tree.
* Nodes may nest 200 levels deep.

2. Statements
-------------

The root
~~~~~~~~

``root "Acme Corp" [color = "#ffff99"];`` sets the label and look of the root.
Without it the root is labelled with the tree name, or ``root``.
``root X { … }`` is the same, with the children inline.

Nodes
~~~~~

A node is a name, optional attributes, and either ``;`` (a leaf) or a block of
children (a branch). Attributes before the brace are legal here.

.. list-table::
   :header-rows: 1

   * - Attribute
     - Meaning
   * - ``label``
     - text to draw instead of the name
   * - ``color``
     - fill (default white; the root is ``#ffff99``, the lane header yellow)
   * - ``textcolor``
     - text color (default black)
   * - ``linecolor``
     - outline (default black)
   * - ``shape``
     - ``roundedbox`` for rounded corners
   * - ``style``
     - ``dotted``, ``dashed`` or a dash pattern, as in activity diagrams
   * - ``href``
     - external link (dropped with ``--no-external``)
   * - ``target``
     - where an ``href`` opens, as in HTML: ``_blank`` for a new tab (with
       ``rel="noopener noreferrer"``), ``_self``, or a window name. Without it
       the live preview loads the page in its own frame, which many sites
       refuse
   * - ``edge``
     - a label on the connector into the node
   * - ``edgestyle``
     - ``dashed`` draws the connector into the node dashed (``style`` is the
       node's own outline). Where the connector merges with a solid one, the
       solid line wins: a shared stretch of the bus is dashed only if every
       child that uses it is dashed
   * - ``collapsed``
     - start with the children folded away (``collapsed = false`` unfolds)
   * - ``ref``
     - open another ``tree`` block of the same file (``ref = "#id"`` or
       ``ref = id``)

Unknown attributes are preserved. A ``ref`` to an unknown tree or to another
file (``other.tree#id``, not supported yet) is reported as a warning and drawn
as a disabled link.

Folding branches
~~~~~~~~~~~~~~~~

Every node with children has a circle drawn where the connector leaves the
node — the middle of its bottom edge, or of its right edge in landscape: a
**minus** while the children are shown, a **plus** while they are folded away.
In an HTML bundle (the live preview and ``--bundle``) clicking the circle (or
pressing Enter/Space on it) folds or unfolds the branch and the tree is laid
out again, so the canvas shrinks and grows. In the top bar of a bundle, next
to the zoom controls, two buttons, **Collapse all** and **Expand all**, fold
every branch (the ones inside a folded branch too) or unfold them all. They are HTML
controls, so an SVG, PNG or PDF has none. ``swimflow`` on the command line
writes the initial state (``collapsed`` branches folded). The **Download**
button of the live preview writes what is on screen: the folds the user made
are sent along, so a PNG, PDF, SVG or HTML shows the same branches folded, and
an export of a file with several trees contains the tree that is open.

How it works: the SVG carries the geometry of every node in ``data-*``
attributes and the bundle includes ``src/plugin/tree/runtime.js``, a port of
the Rust layout. ``tests/tree_runtime.js`` compares the two on every state and
``tests/tree_dom_runtime.js`` clicks through a fake DOM, so a click always
gives the picture the renderer would have drawn for that state.

Diagram attributes
~~~~~~~~~~~~~~~~~~

``orientation = portrait;`` (default, root on top) or
``orientation = landscape;`` (root on the left).

3. Layout
---------

A tidy-tree layout (folded subtrees take no room): leaves sit side by side,
every parent is centered over its children, nodes of one level start on the
same line and connectors are orthogonal (a trunk from the parent, a bus across
the children, a drop into each child, drawn once). Node width follows the
measured label (min 80, max 220 px, longer labels wrap); levels are 48 px
apart, or more when an ``edge`` label needs the room. Everything is integer
and deterministic.
