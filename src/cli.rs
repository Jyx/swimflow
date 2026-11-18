//! Command-line interface definition.
//!
//! The CLI lives in the library (rather than in `main.rs`) so tests can
//! introspect it — `tests/repository.rs` walks the clap command tree to check
//! that the man page documents every option, and `tests/cli.rs` renders the
//! short/long help at a fixed width.
//!
//! Help is deliberately split in two: `-h` prints the terse `help` lines,
//! `--help` prints the `long_help` lines plus the examples. clap's `wrap_help`
//! feature makes both respect the terminal width on every invocation, so the
//! output re-flows after a resize.

use clap::Parser;

/// `swimflow` command-line arguments.
#[derive(Parser, Debug)]
#[command(
    name = "swimflow",
    version,
    about = "Generate activity/swimlane, sequence, tree, treemap and manifest diagrams from a text DSL",
    long_about = "swimflow reads a diagram specification and writes a standalone \
diagram. Several diagram types are supported, each one a plugin that owns the \
reading of the source and the drawing: the activity/swimlane diagram (a small \
DOT-like language compatible with the reference grammar, read from .diag files) and the \
sequence diagram (read from .seq files), the tree diagram (read from .tree files), the treemap (read from .tmap files) and the manifest (read from .mfst files). The type is detected from the file \
extension or the content, and can be selected with --type.\n\nSVG is the canonical \
output; PNG and PDF are derived from it, and --bundle produces a single \
self-contained HTML file with drill-down navigation.",
    after_long_help = "\
EXAMPLES:
  swimflow diagram.diag
  swimflow diagram.diag -f png --scale 2 -o diagram.png
  swimflow zoom-demo.diag --bundle out.html --embed-font
  swimflow checkout.seq -f png -o checkout.png
  swimflow org-chart.tree -o org-chart.svg
  swimflow budget.tmap --bundle budget.html --embed-font
  swimflow editions.mfst -o editions.svg
  swimflow simple.diag --dump-geometry
  swimflow --list-types",
    max_term_width = 100
)]
pub struct Cli {
    /// Input .diag file (default: stdin).
    #[arg(help_heading = "Input", value_name = "FILE")]
    pub input: Option<String>,

    /// Diagram type: activity, sequence, tree, treemap or manifest.
    #[arg(
        short = 't',
        long = "type",
        value_name = "TYPE",
        help_heading = "Input",
        long_help = "Select the diagram type of the input. The type is normally detected from \
the file extension (.diag, .seq, .tree, .tmap, .mfst) or from the content of the file, so this \
flag is only needed for files with an unusual extension or for standard input."
    )]
    pub diagram_type: Option<String>,

    /// List the available diagram types.
    #[arg(
        long,
        help_heading = "Input",
        long_help = "Print the available diagram types and their descriptions, then exit."
    )]
    pub list_types: bool,

    /// Output path (format inferred from the extension).
    #[arg(
        short = 'o',
        long,
        help_heading = "Output",
        long_help = "Write output to PATH. The special value '-' writes to standard output. \
When omitted and INPUT is a file, the output is written next to it using the extension of \
the chosen format."
    )]
    pub output: Option<String>,

    /// Output format: svg, png, pdf or html.
    #[arg(
        short = 'f',
        long,
        help_heading = "Output",
        long_help = "Select the output format. Defaults to svg, or to html when --bundle is given."
    )]
    pub format: Option<String>,

    /// Write an interactive drill-down HTML bundle.
    #[arg(
        short = 'b',
        long,
        value_name = "PATH",
        help_heading = "Drill-down",
        long_help = "Emit one self-contained interactive HTML file containing every diagram in \
the loaded document set. Nodes with a ref attribute navigate to the referenced diagram; a \
breadcrumb, back button, URL hash routing and the Escape key are provided by an inline, \
dependency-free runtime."
    )]
    pub bundle: Option<String>,

    /// Use a TTF/OTF font from PATH.
    #[arg(
        long,
        value_name = "PATH",
        help_heading = "Rendering",
        long_help = "Measure, embed and rasterize text with the TrueType/OpenType font at PATH \
instead of the built-in VL Gothic. The font's declared family name is used for the SVG \
`@font-face`/`font-family` and for the `usvg` database behind PNG/PDF. Layout is \
font-independent, so box geometry does not change."
    )]
    pub font: Option<String>,

    /// Use an installed font family by NAME.
    #[arg(
        long,
        value_name = "NAME",
        help_heading = "Rendering",
        long_help = "Look up the installed font family NAME through fontconfig and use it for \
text measurement and rendering. An unknown family is an error. Ignored when --font is given. \
Layout is font-independent."
    )]
    pub font_family: Option<String>,

    /// Raster scale for PNG output (1.0 = 96 DPI).
    #[arg(
        long,
        default_value_t = 1.0,
        help_heading = "Rendering",
        long_help = "Raster scale for PNG output (1.0 = 96 DPI). The live-preview server has \
the same setting as SWIMFLOW_SCALE."
    )]
    pub scale: f32,

    /// Render PNG at 2x (compatibility with the reference implementation).
    #[arg(
        short = 'a',
        long,
        help_heading = "Rendering",
        long_help = "Render PNG at twice the size, for compatibility with the reference \
--antialias option. In the live-preview server, set SWIMFLOW_SCALE=2.0 instead."
    )]
    pub antialias: bool,

    // These affect a one-shot render only. The live-preview server has no
    // equivalent option and uses its own defaults; they are grouped under one
    // heading so `-h` makes that difference visible.
    /// Bundle entry diagram (default: the first).
    #[arg(long, value_name = "DIAGRAM_ID", help_heading = "CLI only")]
    pub root: Option<String>,

    /// Do not load files referenced by `ref`.
    #[arg(
        long,
        help_heading = "CLI only",
        long_help = "Do not load files referenced by ref = \"other.diag#...\"; only references \
within the input document resolve. The live-preview server always follows ref files."
    )]
    pub no_external_files: bool,

    /// Keep going when a `ref` cannot be resolved.
    #[arg(
        long,
        help_heading = "CLI only",
        long_help = "Keep going when a ref target cannot be resolved: the node is rendered \
disabled and a warning is printed, instead of failing. The live-preview server always \
tolerates broken links so a draft can be exported."
    )]
    pub allow_broken_links: bool,

    /// Embed the font in the output (self-contained).
    #[arg(
        long,
        help_heading = "CLI only",
        long_help = "Embed the selected font as a base64 @font-face inside the SVG (or, for a \
bundle, once in the page CSS) so the output is fully self-contained. The live-preview \
server always embeds the font in standalone downloads."
    )]
    pub embed_font: bool,

    /// Render text as outlines (no font dependency).
    #[arg(long, help_heading = "CLI only")]
    pub text_as_paths: bool,

    /// Do not emit external `href` links.
    #[arg(long, help_heading = "CLI only")]
    pub no_external: bool,

    /// Omit the XML doctype declaration.
    #[arg(long, help_heading = "CLI only")]
    pub nodoctype: bool,

    /// Emit width/height instead of a viewBox.
    #[arg(long, help_heading = "CLI only")]
    pub noviewbox: bool,

    /// Do not paint an opaque background.
    #[arg(short = 'T', long, help_heading = "CLI only")]
    pub transparent: bool,

    // Internal inspection flags: shown by --help (and the man page), hidden
    // from the short -h summary to keep it scannable.
    /// Print the syntax tree and exit.
    #[arg(long, help_heading = "Debug", hide_short_help = true)]
    pub dump_ast: bool,

    /// Print lanes/nodes/edges and exit.
    #[arg(long, help_heading = "Debug", hide_short_help = true)]
    pub dump_model: bool,

    /// Print grid geometry as JSON and exit.
    #[arg(long, help_heading = "Debug", hide_short_help = true)]
    pub dump_geometry: bool,

    /// With --dump-geometry, print pixel boxes.
    #[arg(long, help_heading = "Debug", hide_short_help = true)]
    pub pixels: bool,

    /// Run a live-preview HTTP server instead of rendering once.
    #[arg(
        long,
        help_heading = "Server",
        long_help = "Start an HTTP server that lists the `.diag` files under a configured \
directory and renders an editable live preview in the browser. Configuration comes from a \
`.env` file (see `.env.example`): SWIMFLOW_DIR, SWIMFLOW_HOST, SWIMFLOW_PORT, \
SWIMFLOW_DEBOUNCE_MS, SWIMFLOW_SCALE and the optional SWIMFLOW_TOKEN. The server binds all \
interfaces by default so it can be reached from other machines. Options under `CLI only` \
affect a one-shot render only; the server uses its own defaults for them."
    )]
    pub serve: bool,

    /// Path to the .env file used by --serve (default: ./.env).
    #[arg(
        long,
        value_name = "PATH",
        help_heading = "Server",
        long_help = "Read server configuration from PATH instead of ./.env. A missing file is \
an error. The SWIMFLOW_ENV environment variable is an alternative way to select the file."
    )]
    pub env_file: Option<String>,
}

impl Cli {
    /// Parse the process arguments.
    ///
    /// A wrapper so `main.rs` does not need to import `clap` itself.
    pub fn parse_args() -> Cli {
        <Cli as Parser>::parse()
    }
}
