The sequence diagram DSL
========================

.. note::

   The activity/swimlane language is documented in `dsl.rst <dsl.rst>`_.

A sequence diagram is a *diagram type* of its own (see
`plugin-architecture.rst <plugin-architecture.rst>`_). It lives in files with
a ``.seq`` extension (``.seqdiag`` and ``.sdiag`` work too) and is rendered by
the same binary, with the same fonts and the same output formats.

1. Grammar
----------

.. code-block:: text

    document    := block*
    block       := 'sequence' [name] '{' statement* '}'
    statement  := participant | message | note | frame
    participant := 'participant' name [ 'as' alias ] ';'
    message    := name operator name ':' text ';'
    note       := 'note' 'over' name [ ',' name ]* ':' text ';'
    frame      := frame_kind [ label ] ( '{' statement* '}' | statement* 'end' )
    frame_kind := 'loop' | 'opt' | 'par' | 'alt' | 'else' | 'and' | 'critical' | 'break'
    operator   := '->' | '->>' | '-->' | '--'

* ``name`` is an identifier (``A``, ``order_service``) or a quoted string
  (``"Order Service"``).
* ``text`` is everything up to the ``;``. A fully quoted text is unquoted;
  anything else is taken literally. A text may not span lines.
* Comments are ``# …``, ``// …`` and ``/* … */``.

2. Statements
-------------

Participants
~~~~~~~~~~~~

.. code-block:: text

    participant "Order Service" as orders;

The name is what is drawn above the lifeline; the alias is a short name that
may be used in messages. A participant that is used but never declared is
added automatically, with a warning.

Messages
~~~~~~~~

.. code-block:: text

    orders -> client: "POST /orders";

.. list-table::
   :header-rows: 1

   * - Operator
     - Meaning
     - Line
     - Head
   * - ``->``
     - a synchronous call
     - solid
     - filled
   * - ``->>``
     - an asynchronous message
     - solid
     - open
   * - ``-->``
     - a reply
     - dashed
     - open
   * - ``--``
     - a result
     - dashed
     - none

A message from a participant to itself is drawn as a small loop on the
lifeline.

Notes
~~~~~

.. code-block:: text

    note over client, orders: "the request phase";

A note is a box with a folded corner. It covers the listed participants; with
a single participant it is centered on the lifeline.

Frames
~~~~~~

.. code-block:: text

    loop "until the rollout is healthy"
      ci -> cluster: "roll out";
    end

A frame draws a labelled box around its statements. Frames nest, and the
``else``/``and`` branches of an ``alt``/``par`` frame belong to that frame:

.. code-block:: text

    alt "the health check fails"
      cluster --> ci: "unhealthy";
    else "the health check passes"
      cluster --> ci: "healthy";
    end

A frame may also be closed with a brace (``loop { … }``).

3. Example
----------

.. code-block:: text

    sequence "Checkout" {
      participant "Client" as client;
      participant "Order Service" as orders;
      participant "Payment" as payment;

      client -> orders: "POST /orders";
      orders -> payment: "charge";
      payment --> orders: "approved";
      orders --> client: "201 Created";

      note over client, orders: "the request phase";

      opt "the card is declined"
        orders -> client: "402 Payment Required";
      end
    }

Runnable versions live in ``examples/sequence/``.

4. Layout rules
---------------

The layout is a single top-down pass:

* the participant headers are laid out in a row, sized to their (measured)
  label, with a minimum width and a fixed gap;
* every statement gets a horizontal band: a message reserves the height of its
  (wrapped) label plus the arrow, a note reserves its own height, a self
  message reserves the height of its loop;
* a frame starts right below the previous statement, reserves room for its
  label tab and ends below its last statement; nested frames are inset by four
  pixels per level.

Text is measured with the shared text measurer, so a sequence diagram uses
exactly the same font, the same wrapping and the same ``--text-as-paths`` and
``--embed-font`` options as an activity diagram.

5. Known limitations
--------------------

* No activations (the vertical bars on a lifeline).
* No ``note left of`` / ``note right of`` — only ``note over``.
* A frame's branches are stacked, not drawn side by side.
* No cross-file references.
