//! PNG and PDF output tests.
//!
//! PNG is decoded just enough to check the signature and dimensions; PDF is
//! checked for a valid header/trailer and page geometry.

use swimflow::build;
use swimflow::render::{self, Format, RenderOptions};
const SIMPLE: &str = include_str!("fixtures/simple.diag");

fn render(src: &str, format: Format, options: &RenderOptions) -> Vec<u8> {
    let set = build::build_str(src).expect("build");
    let mut out = Vec::new();
    render::render(format, &set, &mut out, options).expect("render");
    out
}

fn png_dimensions(png: &[u8]) -> (u32, u32) {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "PNG signature");
    let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
    (width, height)
}

#[test]
fn png_has_the_diagram_dimensions() {
    let png = render(SIMPLE, Format::Png, &RenderOptions::default());
    assert_eq!(png_dimensions(&png), (448, 360));
}

#[test]
fn landscape_png_grows_to_include_the_lane_header() {
    // metrics.pagesize() is 640x200 but the frame is drawn at y=-42; the
    // raster must cover the full shifted canvas (640x242), not crop it.
    let src = "{ orientation = landscape; A -> B; A -> C; }";
    let set = build::build_str(src).expect("build");
    let mut out = Vec::new();
    render::render(Format::Png, &set, &mut out, &RenderOptions::default()).expect("render");
    assert_eq!(png_dimensions(&out), (640, 242));
}

#[test]
fn png_scale_multiplies_the_dimensions() {
    let options = RenderOptions {
        scale: 2.0,
        ..RenderOptions::default()
    };
    let png = render(SIMPLE, Format::Png, &options);
    assert_eq!(png_dimensions(&png), (896, 720));
}

#[test]
fn png_with_text_as_paths_still_renders() {
    let options = RenderOptions {
        text_as_paths: true,
        ..RenderOptions::default()
    };
    let png = render(SIMPLE, Format::Png, &options);
    assert_eq!(png_dimensions(&png), (448, 360));
}

#[test]
fn pdf_has_a_valid_header_and_trailer() {
    let pdf = render(SIMPLE, Format::Pdf, &RenderOptions::default());
    assert!(pdf.starts_with(b"%PDF-"), "PDF header");
    assert!(
        pdf.ends_with(b"%%EOF\n") || pdf.ends_with(b"%%EOF"),
        "PDF trailer"
    );
    assert!(pdf.len() > 1000, "PDF should have content");
}

#[test]
fn pdf_page_size_matches_the_diagram() {
    let pdf = render(SIMPLE, Format::Pdf, &RenderOptions::default());
    let needle = b"/MediaBox [0 0 448 360]";
    assert!(
        pdf.windows(needle.len()).any(|window| window == needle),
        "expected a 448x360 point page"
    );
}

#[test]
fn formats_are_deterministic() {
    let options = RenderOptions::default();
    assert_eq!(
        render(SIMPLE, Format::Png, &options),
        render(SIMPLE, Format::Png, &options)
    );
    assert_eq!(
        render(SIMPLE, Format::Pdf, &options),
        render(SIMPLE, Format::Pdf, &options)
    );
}

#[test]
fn png_decodes_with_visible_content() {
    let png = render(SIMPLE, Format::Png, &RenderOptions::default());
    let pixmap = resvg::tiny_skia::Pixmap::decode_png(&png).expect("decode PNG");
    assert_eq!((pixmap.width(), pixmap.height()), (448, 360));

    // The lane headers are pale yellow, the frame grey and text dark; a
    // handful of distinct colors proves shapes and text were rasterized.
    let colors: std::collections::HashSet<(u8, u8, u8)> = pixmap
        .pixels()
        .iter()
        .map(|pixel| (pixel.red(), pixel.green(), pixel.blue()))
        .collect();
    assert!(
        colors.len() > 4,
        "expected several colors, got {}",
        colors.len()
    );
    assert!(
        colors.contains(&(255, 255, 153)),
        "expected the lane header yellow"
    );
}

const ALL_FEATURES: &str = "diagram D { numbering = topological; \
    class approve [color = \"#fff3cd\", shape = roundedbox]; \
    legend [title = \"Legend\"] { class approve [label = \"Approval\"]; \
    entry \"Automated\" [color = \"#e8f5e9\"]; numbered; } \
    lane CI { A -> B [label = \"commit\"]; } lane Ops { C; } B -> C; }";

#[test]
fn all_features_render_deterministically() {
    let options = RenderOptions::default();
    let svg = String::from_utf8(render(ALL_FEATURES, Format::Svg, &options)).unwrap();
    assert!(svg.contains("commit"), "edge label");
    assert!(svg.contains(">Legend<"), "legend title");
    assert_eq!(
        svg.matches("<ellipse").count(),
        4,
        "3 badges + sequence key"
    );

    assert_eq!(
        render(ALL_FEATURES, Format::Svg, &options),
        render(ALL_FEATURES, Format::Svg, &options)
    );
    assert_eq!(
        render(ALL_FEATURES, Format::Png, &options),
        render(ALL_FEATURES, Format::Png, &options)
    );
    assert_eq!(
        render(ALL_FEATURES, Format::Pdf, &options),
        render(ALL_FEATURES, Format::Pdf, &options)
    );
}

#[test]
fn linked_diagrams_still_rasterize() {
    // The SVG `<a>` wrappers emitted for `ref` nodes must survive the usvg
    // path used by the PNG and PDF renderers.
    let src = "diagram main { A [ref = \"#detail\"]; } diagram detail { B -> C; }";
    let png = render(src, Format::Png, &RenderOptions::default());
    let (width, height) = png_dimensions(&png);
    assert!(width > 0 && height > 0);

    let pdf = render(src, Format::Pdf, &RenderOptions::default());
    assert!(pdf.starts_with(b"%PDF-"));
}
