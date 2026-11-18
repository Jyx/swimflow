//! Legend layout.
//!
//! The legend is *not* part of the integer grid: it is a reserved band added to
//! the resolved page size, so lane/frame geometry is untouched. `svg.rs` draws
//! the band using the geometry computed here.
//!
//! Entries:
//! * `class NAME` pulls the swatch from a class (label/attrs overridable);
//! * `entry "TEXT" [attrs]` is a free swatch row;
//! * `numbered` explains the sequence-number badge.

use crate::build::model::{Diagram, LegendEntry, LegendPosition};
use crate::text::TextMeasurer;

/// Band padding.
pub const PADDING: i32 = 8;
/// Swatch square size.
pub const SWATCH: i32 = 16;
/// Gap between a swatch and its label.
pub const GAP: i32 = 8;
/// Height of one entry row.
pub const ROW_HEIGHT: i32 = 22;
/// Height of the title row.
pub const TITLE_HEIGHT: i32 = 22;

/// The glyph shown to the left of a legend row.
#[derive(Debug, Clone, PartialEq)]
pub enum LegendGlyph {
    /// A class/free entry swatch.
    Swatch {
        color: Option<String>,
        shape: Option<String>,
    },
    /// A sequence-number badge (with or without a sample number).
    Badge { number: Option<u32> },
}

/// One legend line.
#[derive(Debug, Clone, PartialEq)]
pub struct LegendRow {
    pub glyph: LegendGlyph,
    pub label: String,
}

/// Computed legend band geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct LegendLayout {
    pub width: i32,
    pub height: i32,
    pub position: LegendPosition,
    pub title: Option<String>,
    pub rows: Vec<LegendRow>,
}

impl LegendLayout {
    /// Total height taken by the title row (0 when absent).
    pub fn title_height(&self) -> i32 {
        if self.title.is_some() {
            TITLE_HEIGHT
        } else {
            0
        }
    }
}

/// Build the legend layout for `diagram`, or `None` when there is nothing to
/// draw. Measurement uses the embedded font so the band is deterministic.
pub fn layout(diagram: &Diagram, measurer: &mut TextMeasurer) -> Option<LegendLayout> {
    let legend = diagram.legend.as_ref()?;

    let mut rows = Vec::new();
    for entry in &legend.entries {
        match entry {
            LegendEntry::Class { name, label, attrs } => {
                let class = diagram.classes.get(name);
                rows.push(LegendRow {
                    glyph: LegendGlyph::Swatch {
                        color: attr(attrs, class, "color"),
                        shape: attr(attrs, class, "shape"),
                    },
                    label: label.clone().unwrap_or_else(|| name.clone()),
                });
            }
            LegendEntry::Free { label, attrs } => {
                rows.push(LegendRow {
                    glyph: LegendGlyph::Swatch {
                        color: attr(attrs, None, "color"),
                        shape: attr(attrs, None, "shape"),
                    },
                    label: label.clone(),
                });
            }
            LegendEntry::SequenceKey => rows.push(LegendRow {
                glyph: LegendGlyph::Badge { number: None },
                label: "Sequence number".to_string(),
            }),
            LegendEntry::Description {
                number,
                label,
                description,
            } => {
                let text = if description.is_empty() {
                    label.clone()
                } else {
                    format!("{label} \u{2014} {description}")
                };
                rows.push(LegendRow {
                    glyph: LegendGlyph::Badge { number: *number },
                    label: text,
                });
            }
        }
    }

    if rows.is_empty() {
        return None;
    }

    let mut text_width: f32 = 0.0;
    for row in &rows {
        text_width = text_width.max(measurer.measure(&row.label, None).width);
    }
    let title_width = legend
        .title
        .as_deref()
        .map(|title| measurer.measure(title, None).width)
        .unwrap_or(0.0);

    let content = text_width.max(title_width).ceil() as i32;
    let width = PADDING * 2 + SWATCH + GAP + content;
    let height = PADDING * 2
        + rows.len() as i32 * ROW_HEIGHT
        + if legend.title.is_some() {
            TITLE_HEIGHT
        } else {
            0
        };

    Some(LegendLayout {
        width,
        height,
        position: legend.position,
        title: legend.title.clone(),
        rows,
    })
}

/// Entry attributes override class attributes.
fn attr(
    entry: &crate::build::model::AttrMap,
    class: Option<&crate::build::model::AttrMap>,
    name: &str,
) -> Option<String> {
    entry.get(name).and_then(|value| value.clone()).or_else(|| {
        class
            .and_then(|attrs| attrs.get(name))
            .and_then(|v| v.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build;

    fn layout_for(src: &str) -> Option<LegendLayout> {
        let set = build::build_str(src).unwrap();
        let diagram = &set.root().diagrams[0];
        let mut measurer = TextMeasurer::default_size();
        layout(diagram, &mut measurer)
    }

    #[test]
    fn no_legend_has_no_layout() {
        assert!(layout_for("{ A; }").is_none());
    }

    #[test]
    fn empty_legend_is_omitted() {
        assert!(layout_for("{ legend [title = \"L\"] { } A; }").is_none());
    }

    #[test]
    fn class_and_free_entries_become_rows() {
        let layout = layout_for(
            "{ class red [color = \"#f00\", shape = roundedbox]; \
              legend { class red; entry \"Manual\" [color = \"#0f0\"]; numbered; } A; }",
        )
        .expect("legend");
        assert_eq!(layout.rows.len(), 3);
        assert_eq!(layout.rows[0].label, "red");
        assert_eq!(
            layout.rows[0].glyph,
            LegendGlyph::Swatch {
                color: Some("#f00".to_string()),
                shape: Some("roundedbox".to_string())
            }
        );
        assert_eq!(layout.rows[1].label, "Manual");
        assert_eq!(layout.rows[2].glyph, LegendGlyph::Badge { number: None });
        assert!(layout.width > PADDING * 2 + SWATCH + GAP);
        assert!(layout.height > PADDING * 2);
    }

    #[test]
    fn class_label_can_be_overridden() {
        let layout = layout_for(
            "{ class red [color = \"#f00\"]; legend { class red [label = \"Critical\"]; } A; }",
        )
        .expect("legend");
        assert_eq!(layout.rows[0].label, "Critical");
    }
}
