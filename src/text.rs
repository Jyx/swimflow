//! Text measurement with an embedded font, built on `cosmic-text`.
//!
//! The reference node/lane/frame geometry is **font-independent**: text is centred
//! inside a fixed box at draw time and never resizes it (see `metrics.rs`).
//! These measurements therefore do not feed back into layout; they are the
//! foundation for the renderer — centring and folding text, and the
//! `--text-as-paths` output.
//!
//! The default font is embedded in the binary (VL Gothic, the same family the
//! reference test suite uses), so measurement is self-contained and
//! deterministic: no system fonts or fontconfig are consulted.

use std::path::Path;
use std::sync::Arc;

use cosmic_text::{
    Attrs, Buffer, CacheKey, Command, Family, FontSystem, LayoutRun, Metrics, Shaping, SwashCache,
    Wrap,
};

/// Embedded default font.
pub const DEFAULT_FONT: &[u8] = include_bytes!("assets/default.ttf");
/// Family name declared inside [`DEFAULT_FONT`].
pub const DEFAULT_FAMILY: &str = "VL Gothic";
/// Blockdiag's `BASE_FONTSIZE`.
pub const DEFAULT_FONT_SIZE: f32 = 11.0;
/// One line plus blockdiag's `line_spacing` of 2px.
pub const DEFAULT_LINE_HEIGHT: f32 = DEFAULT_FONT_SIZE + 2.0;

/// A concrete font to measure and draw text with: a family name and the raw
/// font bytes. The default is the embedded VL Gothic build, so layout-free
/// text rendering is reproducible without any system font dependency.
#[derive(Debug, Clone)]
pub struct FontSpec {
    /// Family name used for `@font-face` and `font-family`. For a font loaded
    /// from disk or a system family this is the name declared by the font.
    pub family: String,
    /// Raw TrueType/OpenType bytes.
    pub data: Arc<[u8]>,
}

impl Default for FontSpec {
    fn default() -> Self {
        FontSpec {
            family: DEFAULT_FAMILY.to_string(),
            data: Arc::from(DEFAULT_FONT),
        }
    }
}

/// Errors produced while resolving a [`FontSpec`].
#[derive(Debug, thiserror::Error)]
pub enum FontError {
    #[error("cannot read font {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("font {0} contains no usable face")]
    NoFace(String),
    #[error("no installed font matches the family {0:?}")]
    NoFamily(String),
}

impl FontSpec {
    /// Resolve an explicit font file, an installed family, or the embedded
    /// default. The file wins when both are given.
    pub fn resolve(path: Option<&Path>, family: Option<&str>) -> Result<FontSpec, FontError> {
        match (path, family) {
            (Some(path), _) => Self::from_file(path),
            (None, Some(family)) => Self::from_family(family),
            (None, None) => Ok(FontSpec::default()),
        }
    }

    /// Load a TTF/OTF from disk and read its declared family name.
    pub fn from_file(path: &Path) -> Result<FontSpec, FontError> {
        let data = std::fs::read(path).map_err(|source| FontError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_data(data, &path.display().to_string())
    }

    /// Look up an installed font by family name via `fontdb`/fontconfig.
    pub fn from_family(family: &str) -> Result<FontSpec, FontError> {
        let mut database = fontdb::Database::new();
        database.load_system_fonts();
        let query = fontdb::Query {
            families: &[fontdb::Family::Name(family)],
            ..fontdb::Query::default()
        };
        let id = database
            .query(&query)
            .ok_or_else(|| FontError::NoFamily(family.to_string()))?;
        let resolved = database
            .face(id)
            .and_then(|face| face.families.first())
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| family.to_string());
        let data = database
            .with_face_data(id, |bytes, _index| bytes.to_vec())
            .ok_or_else(|| FontError::NoFace(family.to_string()))?;
        Ok(FontSpec {
            family: resolved,
            data: Arc::from(data),
        })
    }

    /// Build a spec from raw font bytes, reading the family from the font.
    fn from_data(data: Vec<u8>, label: &str) -> Result<FontSpec, FontError> {
        let mut database = fontdb::Database::new();
        database.load_font_data(data.clone());
        let family = database
            .faces()
            .next()
            .and_then(|face| face.families.first())
            .map(|(name, _)| name.clone())
            .ok_or_else(|| FontError::NoFace(label.to_string()))?;
        Ok(FontSpec {
            family,
            data: Arc::from(data),
        })
    }
}

/// A measured text extent in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextSize {
    pub width: f32,
    pub height: f32,
}

/// A glyph positioned inside a laid-out text block, with its vector outline
/// (used by `--text-as-paths`).
#[derive(Debug, Clone)]
pub struct PositionedGlyph {
    /// X position relative to the left of the text block.
    pub x: f32,
    /// Baseline Y position relative to the top of the text block.
    pub y: f32,
    /// Glyph outline commands (font units scaled to pixels, y-up).
    pub commands: Vec<Command>,
}

/// One visual (possibly wrapped) line of text.
#[derive(Debug, Clone)]
pub struct TextLine {
    pub text: String,
    pub width: f32,
    /// Baseline Y relative to the top of the text block.
    pub baseline: f32,
    pub glyphs: Vec<PositionedGlyph>,
}

/// Measures text against the embedded font.
pub struct TextMeasurer {
    font_system: FontSystem,
    family: String,
    metrics: Metrics,
    swash_cache: SwashCache,
}

impl TextMeasurer {
    /// Create a measurer using only the embedded font.
    pub fn new(font_size: f32, line_height: f32) -> Self {
        Self::with_font(font_size, line_height, &FontSpec::default())
    }

    /// Create a measurer using `font` at the given size and line height.
    pub fn with_font(font_size: f32, line_height: f32, font: &FontSpec) -> Self {
        let mut database = fontdb::Database::new();
        database.load_font_data(font.data.to_vec());
        let font_system = FontSystem::new_with_locale_and_db("en-US".to_string(), database);
        TextMeasurer {
            font_system,
            family: font.family.clone(),
            metrics: Metrics::new(font_size, line_height),
            swash_cache: SwashCache::new(),
        }
    }

    /// A measurer with the default font size and line height.
    pub fn default_size() -> Self {
        Self::new(DEFAULT_FONT_SIZE, DEFAULT_LINE_HEIGHT)
    }

    /// A measurer for `font` at the default size and line height.
    pub fn from_spec(font: &FontSpec) -> Self {
        Self::with_font(DEFAULT_FONT_SIZE, DEFAULT_LINE_HEIGHT, font)
    }

    pub fn font_size(&self) -> f32 {
        self.metrics.font_size
    }

    pub fn line_height(&self) -> f32 {
        self.metrics.line_height
    }

    /// The family name used to build this measurer.
    pub fn family(&self) -> &str {
        &self.family
    }

    /// Measure `text`. When `max_width` is `Some`, long lines wrap at word
    /// boundaries to at most that width.
    pub fn measure(&mut self, text: &str, max_width: Option<f32>) -> TextSize {
        let attrs = Attrs::new().family(Family::Name(&self.family));
        let mut buffer = Buffer::new(&mut self.font_system, self.metrics);
        let mut size = TextSize {
            width: 0.0,
            height: 0.0,
        };

        {
            let mut buffer = buffer.borrow_with(&mut self.font_system);
            buffer.set_wrap(Wrap::Word);
            buffer.set_size(max_width, None);
            buffer.set_text(text, &attrs, Shaping::Advanced, None);

            let mut lines = 0;
            for run in buffer.layout_runs() {
                size.width = size.width.max(run.line_w);
                size.height += run.line_height;
                lines += 1;
            }
            if lines == 0 {
                size.height = self.metrics.line_height;
            }
        }

        size
    }

    /// Quantize a pixel extent to the reference `cellsize` grid (round up), the way
    /// layout treats integer character cells.
    pub fn cells(&self, pixels: f32, cell_size: f32) -> i32 {
        if pixels <= 0.0 || cell_size <= 0.0 {
            return 0;
        }
        (pixels / cell_size).ceil() as i32
    }

    /// Lay out `text` into visual lines, wrapping at word boundaries when
    /// `max_width` is set. Each line carries the positioned glyph outlines for
    /// vector (`--text-as-paths`) rendering.
    pub fn layout(&mut self, text: &str, max_width: Option<f32>) -> Vec<TextLine> {
        let attrs = Attrs::new().family(Family::Name(&self.family));
        let mut buffer = Buffer::new(&mut self.font_system, self.metrics);

        // Collect plain layout data first so the `FontSystem` borrow is released
        // before outlines are fetched from the swash cache.
        let mut raw: Vec<RawLine> = Vec::new();
        {
            let mut buffer = buffer.borrow_with(&mut self.font_system);
            buffer.set_wrap(Wrap::Word);
            buffer.set_size(max_width, None);
            buffer.set_text(text, &attrs, Shaping::Advanced, None);

            for run in buffer.layout_runs() {
                let line_text = visual_line_text(&run);
                let mut glyphs = Vec::with_capacity(run.glyphs.len());
                for glyph in run.glyphs {
                    let physical = glyph.physical((0.0, run.line_y), 1.0);
                    glyphs.push(RawGlyph {
                        cache_key: physical.cache_key,
                        x: physical.x as f32,
                        y: physical.y as f32,
                    });
                }
                raw.push(RawLine {
                    text: line_text,
                    width: run.line_w,
                    baseline: run.line_y,
                    glyphs,
                });
            }
        }

        let mut lines = Vec::with_capacity(raw.len());
        for line in raw {
            let mut positioned = Vec::with_capacity(line.glyphs.len());
            for glyph in line.glyphs {
                let commands = self
                    .swash_cache
                    .get_outline_commands_uncached(&mut self.font_system, glyph.cache_key)
                    .map(|commands| commands.to_vec())
                    .unwrap_or_default();
                positioned.push(PositionedGlyph {
                    x: glyph.x,
                    y: glyph.y,
                    commands,
                });
            }
            lines.push(TextLine {
                text: line.text,
                width: line.width,
                baseline: line.baseline,
                glyphs: positioned,
            });
        }
        lines
    }
}

/// Layout data collected before outlines are resolved.
struct RawLine {
    text: String,
    width: f32,
    baseline: f32,
    glyphs: Vec<RawGlyph>,
}

struct RawGlyph {
    cache_key: CacheKey,
    x: f32,
    y: f32,
}

/// Reconstruct the text covered by a visual (wrapped) line.
fn visual_line_text(run: &LayoutRun<'_>) -> String {
    match (run.glyphs.first(), run.glyphs.last()) {
        (Some(first), Some(last)) => run.text[first.start..last.end].to_string(),
        _ => String::new(),
    }
}

impl Default for TextMeasurer {
    fn default() -> Self {
        Self::default_size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_height_is_one_line() {
        let mut measurer = TextMeasurer::default_size();
        let size = measurer.measure("A", None);
        assert!(size.width > 0.0, "width should be positive");
        assert!((size.height - DEFAULT_LINE_HEIGHT).abs() < 0.01);
    }

    #[test]
    fn newlines_add_lines() {
        let mut measurer = TextMeasurer::default_size();
        let one = measurer.measure("A", None);
        let two = measurer.measure("A\nB", None);
        assert!(two.height > one.height);
        assert!((two.height - one.height * 2.0).abs() < 0.01);
    }

    #[test]
    fn wrapping_grows_height_and_respects_width() {
        let mut measurer = TextMeasurer::default_size();
        let text = "one two three four five six seven eight";
        let wide = measurer.measure(text, None);
        let narrow = measurer.measure(text, Some(40.0));
        assert!(narrow.height > wide.height, "wrapped text should be taller");
        assert!(narrow.width <= 40.0 + 0.5, "wrapped width {}", narrow.width);
    }

    #[test]
    fn measurement_is_deterministic() {
        let mut measurer = TextMeasurer::default_size();
        let a = measurer.measure("Hello, 世界", None);
        let b = measurer.measure("Hello, 世界", None);
        assert_eq!(a, b);
    }

    #[test]
    fn cell_quantization_rounds_up() {
        let measurer = TextMeasurer::default_size();
        assert_eq!(measurer.cells(0.0, 8.0), 0);
        assert_eq!(measurer.cells(0.1, 8.0), 1);
        assert_eq!(measurer.cells(8.0, 8.0), 1);
        assert_eq!(measurer.cells(8.1, 8.0), 2);
    }

    #[test]
    fn layout_returns_lines_with_outlines() {
        let mut measurer = TextMeasurer::default_size();
        let lines = measurer.layout("A\nB", None);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "A");
        assert_eq!(lines[1].text, "B");
        for line in &lines {
            assert!(line.width > 0.0);
            assert!(!line.glyphs.is_empty());
        }
        assert!(
            lines
                .iter()
                .flat_map(|line| &line.glyphs)
                .any(|g| !g.commands.is_empty())
        );
    }

    #[test]
    fn default_font_is_embedded() {
        let font = FontSpec::default();
        assert_eq!(font.family, DEFAULT_FAMILY);
        assert_eq!(&*font.data, DEFAULT_FONT);
    }

    #[test]
    fn resolve_without_arguments_is_the_default() {
        let font = FontSpec::resolve(None, None).unwrap();
        assert_eq!(font.family, DEFAULT_FAMILY);
    }

    #[test]
    fn from_file_reads_the_declared_family() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/assets/default.ttf");
        let font = FontSpec::from_file(&path).unwrap();
        assert_eq!(font.family, DEFAULT_FAMILY);
        assert_eq!(&*font.data, DEFAULT_FONT);
    }

    #[test]
    fn from_file_reports_a_missing_font() {
        let err = FontSpec::from_file(Path::new("/nonexistent/swimflow-font.ttf")).unwrap_err();
        assert!(matches!(err, FontError::Io { .. }));
    }

    #[test]
    fn from_family_reports_an_unknown_family() {
        let err = FontSpec::from_family("swimflow-definitely-not-a-font-xyz").unwrap_err();
        assert!(matches!(err, FontError::NoFamily(_)));
    }

    #[test]
    fn custom_font_measurer_uses_the_family() {
        let font = FontSpec {
            family: "Swimflow Test".to_string(),
            data: Arc::from(DEFAULT_FONT),
        };
        let measurer = TextMeasurer::from_spec(&font);
        assert_eq!(measurer.family(), "Swimflow Test");
        assert_eq!(measurer.font_size(), DEFAULT_FONT_SIZE);
    }

    #[test]
    fn layout_wraps_to_the_requested_width() {
        let mut measurer = TextMeasurer::default_size();
        let lines = measurer.layout("one two three four five six", Some(40.0));
        assert!(lines.len() > 1, "expected wrapping, got {lines:?}");
        assert!(lines.iter().all(|line| line.width <= 40.5));
    }
}
