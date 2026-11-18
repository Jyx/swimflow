DSL specification
=================

.. note::

   This document specifies the **activity/swimlane** language. The other
   diagram types have their own languages, documented in
   `sequence-dsl.rst <sequence-dsl.rst>`_, `tree-dsl.rst <tree-dsl.rst>`_,
   `treemap-dsl.rst <treemap-dsl.rst>`_ and
   `manifest-dsl.rst <manifest-dsl.rst>`_.

Two layers:

1. **Compatibility grammar** — exactly what the reference implementation's
   parser accepts. Non-negotiable: every valid file of that grammar must parse
   and render.
2. **Additive extensions** — new syntax that the reference parser would
   reject, so there is no ambiguity and no behavior change for existing files.

1. Token set (mirrors ``parser.py``)
------------------------------------

.. list-table::
   :header-rows: 1

   * - Token
     - Regex / rule
   * - Comment
     - ``/\* … \*/``, ``// …``, ``# …``
   * - NL / Space
     - whitespace
   * - Name
     - ``[A-Za-z_0-9\u0080-\uffff][A-Za-z_\-\.0-9\u0080-\uffff]*``
   * - Op
     - ``{ } ; , = [ ] <-> <- -- ->``
   * - Number
     - ``-?(.[0-9]+) | ([0-9]+(.[0-9]*)?)``
   * - String
     - ``"…"``, ``'…'``, ``"""…"""``, ``'''…'''``

Note: names may start with a digit and may contain ``-`` and ``.``, and
unicode — important to reproduce exactly.

2. Compatibility grammar (current)
----------------------------------

.. code-block:: text

    node_list      = id (',' id)*
    option_stmt    = id ('=' id)?
    option_list    = '[' option_stmt (',' option_stmt)* ']'?
    node_stmt      = node_list option_list
    edge_relation  = '->' | '--' | '<-' | '<->'
    edge_stmt      = node_list edge_relation node_list (edge_relation node_list)* option_list
    attribute_stmt = id '=' id
    extension_stmt = ('class' | 'plugin') id option_list
    lane_declare   = 'lane' id option_list
    lane_inline    = edge_stmt | attribute_stmt | node_stmt
    # `option_list` on a lane block is a swimflow extension; upstream rejects it.
    lane_stmt      = 'lane' id? option_list '{' (lane_inline ';'?)* '}'
    diagram_id     = ('diagram' | 'actdiag') id?
    diagram_inline = extension_stmt | edge_stmt | lane_stmt
                   | lane_declare | attribute_stmt | node_stmt
    diagram        = diagram_id? '{' (diagram_inline ';'?)* '}'
    dotfile        = diagram finished

Semantics worth preserving:

* ``A -> B -> C [attrs]`` expands to two edges; the attribute list is applied
  to **each** generated edge (this matters for edge labels).
* ``A, B [attrs]`` creates two nodes with the same attrs.
* ``class NAME [attrs]`` and ``plugin NAME [attrs]`` are extensions.
  ``autolane`` reads the ``"<lane>_<rest>"`` naming convention.
* Attributes land on the nearest enclosing scope: lane attrs on the lane,
  diagram attrs on the diagram, otherwise on node/edge.
* A lane's ``color`` fills its header box. It can be written inline on the
  block (``lane "CI" [color = "#c8e6c9"] { Build; }``), as a separate
  declaration (``lane "CI" [color = "#c8e6c9"];``), or as a statement inside
  the block (``lane "CI" { color = "#c8e6c9"; Build; }``) — all three are
  equivalent and produce the same ``Attr`` on the lane. See
  ``examples/colors.diag``.

3. Extensions
-------------

3.1 Multiple diagrams per file (foundation for drill-down)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: text

    dotfile = diagram+

A single unnamed block keeps its current meaning (id = file stem or ``""``).
Named blocks become addressable:

.. code-block:: text

    diagram overview {
      Start -> Review;
      Review [ref = "#detail"];
    }

    diagram detail {
      Prepare -> Approve;
    }

Backward compatible: a file with exactly one block behaves exactly as before.
If several blocks exist and one is unnamed, it is assigned id ``default``.

3.2 Node links: ``ref`` (internal) versus ``href`` (external)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Keep the existing ``href`` attribute for external URLs. Add ``ref`` for
diagrams:

.. list-table::
   :header-rows: 1

   * - ``ref`` value
     - Meaning
   * - ``"detail"``
     - diagram ``detail`` in the same document
   * - ``"#detail"``
     - same
   * - ``"other.diag"``
     - the default/single diagram in ``other.diag``
   * - ``"other.diag#detail"``
     - diagram ``detail`` in ``other.diag``

Resolution is a dedicated pass (``build/link.rs``). Unresolved internal refs:

* default: **error** with file/line of the referring node;
* ``--allow-broken-links``: keep the node, render as disabled, warn.

``ref`` and ``href`` are mutually exclusive; specifying both is an error.

3.3 Edge labels
~~~~~~~~~~~~~~~

Canonical form uses the existing attribute syntax:

.. code-block:: text

    A -> B [label = "approved"];
    A -> B -> C [label = "step"];     # applies to both hops (reference semantics)

3.4 Legend block
~~~~~~~~~~~~~~~~

``legend { … }`` is new syntax (today ``legend {`` is a parse error because a
node name may not be followed by ``{``). Node statements named ``legend;``
remain valid.

.. code-block:: text

    legend [title = "Legend", position = bottom] {
      class red;                        # pull swatch from an existing class
      class manual [label = "Manual step"];
      entry "System" [color = "#cce5ff", shape = box];
      numbered;                         # include the sequence-number key
    }

* ``class NAME`` references a ``class`` defined in the document; label
  defaults to the class name. ``[label = …]`` overrides it.
* ``entry "TEXT" [attrs]`` adds a free legend row with a swatch.
* ``numbered`` adds an explanatory entry for sequence badges.
* ``position`` is one of ``top | bottom | left | right`` (default ``bottom``).

3.5 Sequence numbers
~~~~~~~~~~~~~~~~~~~~

Diagram-level switch plus per-node override:

.. code-block:: text

    diagram {
      numbering = auto;                 # off | auto | topological | depth
      A;
      B [numbered = 2];                 # explicit override
      C;
    }

* ``auto`` — assignment in definition order.
* ``topological`` — Kahn's algorithm over the edges; cycle-closing (back)
  edges are dropped first, so a feedback loop never collapses the whole
  diagram to definition order.
* ``depth`` — longest-path distance from a start over the cycle-free edges,
  then definition order. Parallel steps share a stage and consecutive
  numbers.
* ``numbered = N`` — explicit, always wins.
* ``numbered = none`` — opt a node out under ``auto``, ``topological`` or
  ``depth``.

The existing blockdiag attribute name ``numbered`` is reused for compatibility
(the RST ``:desctable:`` era used ``numbered`` plus ``description``).

3.6 Forward-compatibility rule
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Any attribute not recognized is stored verbatim in the node/edge/lane/diagram
``AttrMap`` and ignored by renderers. This lets older binaries read newer
files without failing and lets new features be attribute-driven.

4. Worked example
-----------------

.. code-block:: text

    diagram "Deployment" {
      numbering = topological;
      orientation = landscape;

      legend [title = "Legend"] {
        class approve [label = "Approval gate"];
        entry "Automated" [color = "#e8f5e9"];
      }

      class approve [color = "#fff3cd", shape = roundedbox];

      lane "CI" {
        Build -> Test;
      }
      lane "Ops" {
        Test -> Deploy;
        Deploy [ref = "#rollback"];
      }
      Build -> Test [label = "commit"];
      Test -> Deploy [label = "pass"];
    }

    diagram "Rollback" {
      Detect -> Revert -> Verify;
    }

A bundle built from the above starts at ``Deployment``; clicking ``Deploy``
zooms into ``Rollback``.
