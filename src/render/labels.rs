//! Edge-label placement.
//!
//! The reference centred edge labels in a fixed `metrics.edge_labelbox` rectangle,
//! which reads as a floating box (and, on "skipped" routes, lands in the middle
//! of the detour). This module computes a *placement* instead: the label rides
//! the shaft when there is room (the shaft is split around it), sits beside a
//! vertical shaft, or offsets just above/below a short run. The renderer only
//! serializes the result.
//!
//! Geometry is deterministic: integer boxes, one embedded [`TextMeasurer`].

use crate::build::model::Diagram;
use crate::metrics::{EdgeRoute, Metrics, PixelBox};
use crate::text::TextMeasurer;

/// Pixels of shaft omitted on each side of an inline label.
pub const GAP_PAD: i32 = 3;
/// Pixels between a vertical shaft and its label.
pub const SIDE_PAD: i32 = 4;
/// Pixels between a short horizontal shaft and its offset label.
pub const OFFSET_PAD: i32 = 2;
/// Below this segment length, offsetting is not worth it and the reference
/// box is used instead.
pub const MIN_OFFSET_WIDTH: i32 = 16;

/// Which side of a vertical shaft a label sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Right,
    Left,
}

/// Which side of a short horizontal shaft a label sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VertSide {
    Above,
    Below,
}

/// How an edge label is drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeLabelKind {
    /// Place the label on a horizontal shaft segment and omit `gap` from it.
    Inline {
        polyline: usize,
        segment: usize,
        gap: (i32, i32),
    },
    /// Place the label beside a vertical shaft (text stays horizontal).
    Beside { side: Side },
    /// Place the label just above/below a short horizontal shaft.
    Offset { side: VertSide },
    /// Last resort: the reference `metrics.edge_labelbox` rectangle.
    Reference,
}

/// A resolved edge-label placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeLabelLayout {
    pub kind: EdgeLabelKind,
    /// The box the text is centred in (and the halo covers).
    pub text_box: PixelBox,
}

/// One shaft segment between two consecutive polyline points.
#[derive(Debug, Clone, Copy)]
struct Segment {
    polyline: usize,
    index: usize,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl Segment {
    fn is_horizontal(&self) -> bool {
        self.y1 == self.y2 && self.x1 != self.x2
    }

    fn is_vertical(&self) -> bool {
        self.x1 == self.x2 && self.y1 != self.y2
    }

    fn len(&self) -> i32 {
        (self.x2 - self.x1).abs().max((self.y2 - self.y1).abs())
    }

    fn mid(&self) -> (i32, i32) {
        (
            self.x1 + (self.x2 - self.x1) / 2,
            self.y1 + (self.y2 - self.y1) / 2,
        )
    }

    /// The x-range of a horizontal segment, ordered low → high.
    fn x_range(&self) -> (i32, i32) {
        (self.x1.min(self.x2), self.x1.max(self.x2))
    }
}

/// Compute a placement for every edge, index-aligned with `diagram.edges`.
/// `None` means "no label" (no `label`, or `style = none`).
pub fn layout(
    diagram: &Diagram,
    metrics: &Metrics,
    routes: &[EdgeRoute],
    text: &mut TextMeasurer,
) -> Vec<Option<EdgeLabelLayout>> {
    let (page_w, _) = metrics.pagesize();
    diagram
        .edges
        .iter()
        .enumerate()
        .map(|(index, edge)| {
            if edge
                .style
                .as_deref()
                .map(|style| style.eq_ignore_ascii_case("none"))
                .unwrap_or(false)
            {
                return None;
            }
            let label = edge.label.as_deref().filter(|label| !label.is_empty())?;
            let route = routes.get(index)?;
            Some(place(label, edge, route, diagram, metrics, page_w, text))
        })
        .collect()
}

fn place(
    label: &str,
    edge: &crate::build::model::Edge,
    route: &EdgeRoute,
    diagram: &Diagram,
    metrics: &Metrics,
    page_w: i32,
    text: &mut TextMeasurer,
) -> EdgeLabelLayout {
    let segments = collect_segments(route);
    let natural = text.measure(label, None);
    let tw = natural.width.ceil() as i32;
    let th = natural.height.ceil() as i32;

    // Prefer the longest horizontal run: straight edges centre the label, and
    // "skipped" detours get it on the long run instead of in mid-air.
    let horizontal = segments
        .iter()
        .filter(|segment| segment.is_horizontal())
        .max_by_key(|segment| segment.len());
    if let Some(segment) = horizontal
        && segment.len() >= tw + 2 * GAP_PAD
    {
        let (lo, hi) = segment.x_range();
        let mid = lo + (hi - lo) / 2;
        let gap = (mid - (tw / 2 + GAP_PAD), mid + (tw - tw / 2 + GAP_PAD));
        let text_box = PixelBox {
            x1: mid - tw / 2,
            y1: segment.y1 - th / 2,
            x2: mid + (tw - tw / 2),
            y2: segment.y1 - th / 2 + th,
        };
        return EdgeLabelLayout {
            kind: EdgeLabelKind::Inline {
                polyline: segment.polyline,
                segment: segment.index,
                gap,
            },
            text_box,
        };
    }

    // Vertical runs: put the text beside the line, always left-to-right.
    let vertical = segments
        .iter()
        .filter(|segment| segment.is_vertical())
        .max_by_key(|segment| segment.len());
    if let Some(segment) = vertical {
        let (_, cy) = segment.mid();
        let side = if page_w - segment.x1 >= segment.x1 {
            Side::Right
        } else {
            Side::Left
        };
        let (x1, x2) = match side {
            Side::Right => (segment.x1 + SIDE_PAD, segment.x1 + SIDE_PAD + tw),
            Side::Left => (segment.x1 - SIDE_PAD - tw, segment.x1 - SIDE_PAD),
        };
        let text_box = PixelBox {
            x1,
            y1: cy - th / 2,
            x2,
            y2: cy - th / 2 + th,
        };
        return EdgeLabelLayout {
            kind: EdgeLabelKind::Beside { side },
            text_box,
        };
    }

    // Short horizontal run: wrap to its width and offset above (or below if a
    // node is in the way); otherwise fall back to the reference box.
    if let Some(segment) = horizontal
        && segment.len() >= MIN_OFFSET_WIDTH
    {
        let max_width = segment.len() as f32;
        let wrapped = text.measure(label, Some(max_width));
        let tw = wrapped.width.ceil() as i32;
        let th = wrapped.height.ceil() as i32;
        let (lo, hi) = segment.x_range();
        let mid = lo + (hi - lo) / 2;
        let box_ = |side: VertSide| {
            let (y1, y2) = match side {
                VertSide::Above => (segment.y1 - OFFSET_PAD - th, segment.y1 - OFFSET_PAD),
                VertSide::Below => (segment.y1 + OFFSET_PAD, segment.y1 + OFFSET_PAD + th),
            };
            PixelBox {
                x1: mid - tw / 2,
                y1,
                x2: mid - tw / 2 + tw,
                y2,
            }
        };
        for side in [VertSide::Above, VertSide::Below] {
            let candidate = box_(side);
            if !collides_with_node(&candidate, diagram, metrics) {
                return EdgeLabelLayout {
                    kind: EdgeLabelKind::Offset { side },
                    text_box: candidate,
                };
            }
        }
    }

    EdgeLabelLayout {
        kind: EdgeLabelKind::Reference,
        text_box: metrics.edge_labelbox(diagram, edge),
    }
}

fn collect_segments(route: &EdgeRoute) -> Vec<Segment> {
    let mut segments = Vec::new();
    for (polyline_index, polyline) in route.polylines.iter().enumerate() {
        for (index, pair) in polyline.windows(2).enumerate() {
            segments.push(Segment {
                polyline: polyline_index,
                index,
                x1: pair[0].0,
                y1: pair[0].1,
                x2: pair[1].0,
                y2: pair[1].1,
            });
        }
    }
    segments
}

fn collides_with_node(box_: &PixelBox, diagram: &Diagram, metrics: &Metrics) -> bool {
    diagram.nodes.iter().any(|node| {
        let node = metrics.cell(node);
        box_.x1 < node.x2 && box_.x2 > node.x1 && box_.y1 < node.y2 && box_.y2 > node.y1
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build;

    fn layouts(source: &str) -> (Vec<EdgeRoute>, Vec<Option<EdgeLabelLayout>>) {
        let set = build::build_str(source).unwrap();
        let diagram = &set.root().diagrams[0];
        let metrics = Metrics::new(diagram);
        let routes: Vec<EdgeRoute> = diagram
            .edges
            .iter()
            .map(|edge| metrics.edge_route(diagram, edge))
            .collect();
        let mut text = TextMeasurer::default_size();
        let labels = layout(diagram, &metrics, &routes, &mut text);
        (routes, labels)
    }

    #[test]
    fn long_horizontal_edge_is_inline() {
        let (routes, labels) =
            layouts(r#"{ orientation = landscape; A -> B [label = "abcdef"]; }"#);
        match &labels[0].as_ref().unwrap().kind {
            EdgeLabelKind::Inline { gap, .. } => {
                assert!(gap.0 < gap.1, "gap {gap:?} must be a range");
                // The gap must sit inside the routed segment.
                let polyline = &routes[0].polylines[0];
                assert!(gap.0 >= polyline[0].0.min(polyline[1].0));
                assert!(gap.1 <= polyline[0].0.max(polyline[1].0));
            }
            other => panic!("expected Inline, got {other:?}"),
        }
    }

    #[test]
    fn vertical_edge_places_label_beside_the_line() {
        let (_, labels) = layouts(r#"{ A -> B [label = "commit"]; }"#);
        assert_eq!(
            labels[0].as_ref().unwrap().kind,
            EdgeLabelKind::Beside { side: Side::Right }
        );
    }

    #[test]
    fn wide_label_on_a_short_run_offsets() {
        let (_, labels) =
            layouts(r#"{ orientation = landscape; A -> B [label = "averyverylonglabel"]; }"#);
        assert!(matches!(
            labels[0].as_ref().unwrap().kind,
            EdgeLabelKind::Offset { .. } | EdgeLabelKind::Reference
        ));
    }

    #[test]
    fn style_none_has_no_label() {
        let (_, labels) = layouts(r#"{ A -> B [label = "x", style = none]; }"#);
        assert!(labels[0].is_none());
    }

    #[test]
    fn placement_is_deterministic() {
        let source =
            r#"{ orientation = landscape; A -> B [label = "abcdef"]; A -> C [label = "xy"]; }"#;
        let (_, first) = layouts(source);
        let (_, second) = layouts(source);
        assert_eq!(first, second);
    }
}
