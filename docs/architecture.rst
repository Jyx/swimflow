Architecture
============

``swimflow`` turns a text DSL into diagrams in a single Rust binary. A
diagram-agnostic core (fonts, text measurement, SVG primitives, PNG/PDF and
the HTML bundle) serves one plugin per diagram type. The activity plugin also
preserves the reference grammar, integer-grid layout and pixel metrics.

1. Pipeline
-----------

.. code-block:: text

    .diag text ──lexer──► tokens ──parser──► AST ──build──► Document (IR, arena)
                                                                 │
                                                                 ▼
                                                         grid layout (pure)
                                                                 │
                                                                 ▼
                                                         pixel metrics
                                                                 │
                                                                 ▼
                                                         geometry (boxes, routes)
                                                                 │
                                              ┌──────────────────┼──────────────────┐
                                              ▼                  ▼                  ▼
                                          svg writer        svg → resvg → png   svg → svg2pdf → pdf
                                              │
                                              ▼
                                  (optional) bundle.html + zoom runtime

The diagram type is a plugin: the pipeline above is the *activity* plugin,
and the SVG at the end of the chain is the contract between a plugin and the
core. The sequence, tree, treemap and manifest plugins follow the same
pipeline with their own parsers and layouts. A plugin only has to produce
SVG; the PNG/PDF conversion and the HTML bundle are core services (see
`plugin-architecture.rst <plugin-architecture.rst>`_).

The grid layout is **font-independent integer logic**; only ``metrics`` and
``text`` introduce font measurement. This is the same split as the reference
implementation's ``builder.py`` (grid) versus ``metrics.py`` (pixels).

2. Crate / module layout
------------------------

.. code-block:: text

    swimflow/
      Cargo.toml
      src/
        main.rs            entry point, input/output, format inference
        cli.rs             clap argument definitions (shared with repository tests)
        lib.rs             re-exports for testing + embedding
        lexer.rs           tokenizer matching the reference token specs
        parser.rs          recursive-descent parser → ast
        ast.rs             syntactic tree (mirrors parser.py namedtuples)
        build/
          mod.rs           AST → Document; multi-diagram expansion; parallel layout
          model.rs         arena graph, ids, attributes
          layout.rs        DiagramTreeBuilder port (lane assignment, ordering)
          grid.rs          DiagramLayoutManager port (detect_circulars, set_node_height…)
          link.rs          ref/href resolution across diagrams/files
        metrics.rs         grid → pixel boxes; lane frame/header; SpreadSheet port
        text.rs            cosmic-text measurement + wrapping, embedded font
        render/
          mod.rs           Renderer trait + format dispatch
          svg.rs           geometry → SVG (canonical)
          labels.rs        edge-label placement: inline gap / beside / offset
          legend.rs        legend band geometry
          png.rs           SVG → resvg → PNG
          pdf.rs           SVG → svg2pdf → PDF
          bundle.rs        multi-diagram HTML + navigation runtime
        plugins/
          autolane.rs      port of plugins/autolane.py
        assets/
          default.ttf      include_bytes! embedded default font
      man/
        swimflow.1         manual page (checked against the CLI in tests)
      tests/
        golden/            .diag inputs + expected geometry (JSON) + SVG snapshots
      completions/
        swimflow.bash      generated bash completion
      .github/workflows/
        ci.yml             fmt / clippy / test + static musl build
        release.yml        tag `v*` -> gate, cross-platform binaries, GitHub release

3. Intermediate representation
------------------------------

The IR carries links, sequence numbers and legend entries alongside the graph
itself.

.. code-block:: rust

    // --- identity ---------------------------------------------------------------
    pub struct DiagramId(pub String);      // "overview", "detail", or "" for solo
    pub struct NodeId(pub u32);            // arena index, local to a Diagram
    pub struct EdgeId(pub u32);

    // --- top level --------------------------------------------------------------
    pub struct Document {
        pub source: Option<PathBuf>,
        pub diagrams: Vec<Diagram>,        // preserves definition order
        pub index: BTreeMap<DiagramId, usize>,
        pub meta: Meta,                    // titles, version, unknown top attrs
    }

    pub struct Diagram {
        pub id: DiagramId,
        pub nodes: Arena<Node>,
        pub edges: Vec<Edge>,
        pub lanes: Vec<Lane>,
        pub name_index: BTreeMap<String, NodeId>, // author-visible name → node
        pub orientation: Orientation,       // portrait | landscape (default: portrait)
        pub legend: Option<Legend>,
        pub numbering: Numbering,           // Off | Auto | Topological
        pub attrs: AttrMap,                 // diagram-level attributes
    }

    // --- graph entities ---------------------------------------------------------
    pub struct Node {
        pub name: String,                   // the id used in the DSL; identity key
        pub label: String,                  // display text (may be multiline)
        pub class: Option<String>,          // `class` reference
        pub lane: Option<usize>,
        pub number: Option<u32>,            // sequence badge
        pub link: Option<Link>,             // drill-down / external
        pub shape: Shape,
        pub attrs: AttrMap,                 // includes unknown/future attributes
        // layout (grid then pixels)
        pub grid: GridRect,                 // col/row/colwidth/colheight (builder.py)
        pub pixel: Option<PixelRect>,       // filled by metrics.rs
    }

    pub struct Edge {
        pub from: NodeId,
        pub to: NodeId,
        pub dir: EdgeDir,                   // -> | <- | -- | <->
        pub label: Option<String>,          // edge label
        pub attrs: AttrMap,
        pub route: Option<Vec<Point>>,      // filled by layout
    }

    pub struct Lane {
        pub id: String,                     // may be empty (anonymous lanes)
        pub label: Option<String>,
        pub nodes: Vec<NodeId>,
        pub grid: Option<GridRect>,
        pub pixel: Option<PixelRect>,
        pub attrs: AttrMap,
    }

    // --- future-feature carriers ------------------------------------------------
    pub struct Link {
        pub kind: LinkKind,
    }
    pub enum LinkKind {
        External(String),                   // href = "https://…"
        Diagram { file: Option<PathBuf>, id: DiagramId }, // ref = "other.diag#detail"
    }

    pub struct Legend {
        pub title: Option<String>,
        pub entries: Vec<LegendEntry>,      // class swatches, sequence list, etc.
        pub position: LegendPosition,
    }

    pub enum Numbering { Off, Auto, Topological }

``AttrMap = BTreeMap<String, String>`` (or a small typed enum) so unknown
attributes survive and can be promoted to real features later.

4. Arena graph (replaces global registries)
-------------------------------------------

Python uses process-global class registries:

.. code-block:: python

    node = DiagramNode.get(stmt.id)   # joins/creates by name
    DiagramNode.clear()               # lifecycle hazard

Rust replaces this with per-``Diagram`` ownership:

.. code-block:: rust

    pub struct Arena<T> { items: Vec<T> }
    impl Arena<Node> {
        fn intern(&mut self, name: &str, index: &mut BTreeMap<String, NodeId>) -> NodeId;
    }

* Author name → ``NodeId`` via ``name_index``.
* Edges hold ``NodeId`` values, not references.
* No global state, so layout is a pure ``&mut Document`` transformation and is
  trivially unit-testable and parallelizable across diagrams.

5. Layout port notes
--------------------

Port ``build/`` against these exact Python entry points:

.. list-table::
   :header-rows: 1

   * - Python
     - Rust
     - Notes
   * - ``DiagramTreeBuilder.build``
     - ``build::build_diagram``
     - lane auto-assignment pass
   * - ``DiagramTreeBuilder.instantiate``
     - ``build::instantiate``
     - node/edge/lane/attr/extension statements
   * - ``DiagramTreeBuilder.bind_edges``
     - ``build::bind_edges``
     -
   * - ``DiagramLayoutManager.run``
     - ``grid::run``
     -
   * - ``detect_circulars*``
     - ``grid::detect_circulars``
     - cycle detection
   * - ``set_node_width``, ``adjust_node_order``
     - ``grid::*``
     - integer grid
   * - ``set_node_height``, ``mark_xy``, ``is_marked``
     - ``grid::*``
     - swimlane collision avoidance
   * - ``fixiate_lanes``, ``rotate_diagram``
     - ``grid::*``
     - portrait/landscape
   * - ``metrics.frame``, ``lane_textbox``, ``lane_headerbox``
     - ``metrics::*``
     - activity-specific pixel boxes
   * - blockdiag ``SpreadSheet``
     - ``metrics::spreadsheet``
     - span widths/heights per grid cell

**Important:** ``colwidth`` and ``colheight`` are counts of character cells
set from node attributes (default ``1``), **not** from text measurement. The
reference implementation never sizes a node from its label; ``width`` and
``height`` are optional explicit pixel sizes and otherwise default to
``node_width`` and ``node_height``. Node, lane and frame boxes are therefore
font-independent and are compared with the reference at tolerance 0
(``tests/metrics_compat.rs``), not within a pixel tolerance.

6. Text pipeline
----------------

* ``cosmic-text::FontSystem`` plus ``Buffer::layout_runs()`` → glyph advances.
* Default font embedded via ``include_bytes!`` (``src/assets/default.ttf``,
  VL Gothic) and loaded without ``fontconfig`` or system fonts, so measurement
  is deterministic and the binary stays self-contained.
* API used by the renderer, *not* by layout:

  * ``TextMeasurer::measure(&str, Option<max_width>) -> TextSize`` handles
    ``\n`` and (optionally) word wrapping.
  * ``TextMeasurer::cells(pixels, cell_size)`` quantizes to the cell grid.

* Optional flags:

  * ``--embed-font`` puts a base64 ``@font-face`` inside SVG for true
    standalone viewing.
  * ``--text-as-paths`` outlines glyphs (zero font dependency, no text
    selection).

* PNG and PDF **must** be rasterized with the same embedded font so shaping
  matches.

7. Renderer abstraction
-----------------------

.. code-block:: rust

    pub trait Renderer {
        fn render(&self, doc: &Document, out: &mut dyn Write, opts: &RenderOptions) -> Result<()>;
    }

* ``SvgRenderer`` is canonical and owns all drawing.
* ``PngRenderer`` = ``SvgRenderer`` → ``usvg::Tree`` → ``resvg`` →
  ``tiny_skia::Pixmap``.
* ``PdfRenderer`` = ``SvgRenderer`` → ``usvg::Tree`` → ``svg2pdf``.
* ``BundleRenderer`` = many ``SvgRenderer`` outputs inlined into one HTML
  document with the navigation runtime.

Link emission is a render option: when bundling, internal ``ref`` values become
``href="#<diagramId>"``; standalone SVG emits file refs (``detail.svg``).

8. Determinism
--------------

* Fixed embedded font, pinned version.
* Round coordinates to 1 decimal; single formatting routine.
* Stable ordering: ``Vec`` in definition order and ``BTreeMap`` for indices;
  sort edges and nodes where the Python code sorts.
* Timestamps and IDs **must not** appear in output unless requested.

9. CLI surface
--------------

.. code-block:: text

    swimflow [OPTIONS] [FILE]
      -o, --output <PATH>
      -f, --format <svg|png|pdf|html>       # inferred from -o extension if omitted
          --fontpath <FILE>                 # override embedded font
          --embed-font                      # self-contained SVG text
          --text-as-paths                   # outline text
          --nodefaultfont
      -T, --transparent
          --nodoctype                       # SVG doctype control (compat)
          --noviewbox
      -b, --bundle <PATH>                   # emit multi-diagram interactive HTML
          --root <DIAGRAM_ID>               # entry diagram for bundle
          --allow-broken-links              # warn instead of erroring on bad refs
          --no-external                     # suppress external href links
          --no-external-files               # do not load referenced files

Compatibility aliases from the Python CLI (``--antialias``,
``:width:``/``:height:`` style options) are recognized where meaningful;
docutils-only options are ignored with a warning.

10. Error handling
------------------

* ``thiserror`` for typed errors (``ParseError``, ``LinkError``,
  ``RenderError``).
* ``anyhow`` at the ``main.rs`` boundary.
* Parser errors report ``line:column``, matching the spirit of the reference
  parser's ``ParseException``.
