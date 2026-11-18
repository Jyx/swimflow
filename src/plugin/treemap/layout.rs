//! Layout: the treemap -> pixel geometry.
//!
//! The layout is a squarified treemap: the children of a rectangle are sorted
//! by weight (largest first, ties in source order), packed into rows so the
//! rectangles stay close to square, and then placed in integer pixels. Every
//! container reserves a header band for its label; the children fill the
//! interior. The result is deterministic and independent of the font.

use crate::core::geom::PixelBox;

use super::ast::{Attr, attr_value};
use super::model::{Diagram, Node};

/// Space around the whole treemap.
pub const MARGIN: i32 = 24;
/// Default canvas size.
pub const DEFAULT_WIDTH: i32 = 800;
pub const DEFAULT_HEIGHT: i32 = 500;
/// Height of the label band of a container.
pub const HEADER_HEIGHT: i32 = 26;
/// Padding inside a rectangle.
pub const PADDING: i32 = 6;
/// Gap between siblings; half of it is inset from each side.
pub const SIBLING_GAP: i32 = 3;
/// The minimum height a container needs before it gets a header band.
pub const MIN_LABEL_HEIGHT: i32 = 18;

/// The complete geometry of a treemap.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub width: i32,
    pub height: i32,
    /// The root followed by every placed node in depth-first source order.
    pub nodes: Vec<NodeLayout>,
}

/// A placed rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeLayout {
    /// The text drawn in the rectangle.
    pub label: String,
    /// The resolved weight.
    pub value: f64,
    /// The attributes the renderer reads.
    pub attrs: Vec<Attr>,
    /// The outer box of the rectangle.
    pub box_: PixelBox,
    /// The distance from the root.
    pub depth: usize,
    /// Index of the parent (`None` for the root).
    pub parent: Option<usize>,
    /// Whether the node has children at all.
    pub has_children: bool,
    /// Whether the node has a separate header band for its label.
    pub header: bool,
    /// Whether the value should be drawn (source `showvalue`).
    pub show_value: bool,
}

impl NodeLayout {
    /// The value of the attribute `key`, if it has one.
    pub fn attr(&self, key: &str) -> Option<&str> {
        attr_value(&self.attrs, key)
    }
}

/// Compute the geometry of a treemap.
pub fn layout(diagram: &Diagram) -> Layout {
    let width = dimension(&diagram.attrs, "width", DEFAULT_WIDTH);
    let height = dimension(&diagram.attrs, "height", DEFAULT_HEIGHT);
    let default_show_value = show_value(&diagram.attrs, true);

    let canvas = PixelBox {
        x1: 0,
        y1: 0,
        x2: width,
        y2: height,
    };
    let root_box = PixelBox {
        x1: MARGIN,
        y1: MARGIN,
        x2: width - MARGIN,
        y2: height - MARGIN,
    };
    let root_box = if root_box.width() > 0 && root_box.height() > 0 {
        root_box
    } else {
        canvas
    };

    let root_has_children = !diagram.children.is_empty();
    let root_header = root_has_children && has_room_for_header(&root_box);
    let root_show_value = show_value(&diagram.root_attrs, default_show_value);
    let mut nodes = vec![NodeLayout {
        label: diagram.label.clone(),
        value: diagram.weight,
        attrs: diagram.root_attrs.clone(),
        box_: root_box,
        depth: 0,
        parent: None,
        has_children: root_has_children,
        header: root_header,
        show_value: root_show_value,
    }];

    place_children(
        &diagram.children,
        root_box,
        root_header,
        0,
        1,
        default_show_value,
        &mut nodes,
    );

    Layout {
        width,
        height,
        nodes,
    }
}

fn dimension(attrs: &[Attr], key: &str, default: i32) -> i32 {
    attr_value(attrs, key)
        .and_then(|value| value.trim().parse::<i32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

/// `showvalue` defaults to `true`; `false`, `no`, `off` and `0` turn it off.
fn show_value(attrs: &[Attr], default: bool) -> bool {
    match attrs.iter().rev().find(|attr| attr.key == "showvalue") {
        None => default,
        Some(attr) => match attr.value.as_deref() {
            None => true,
            Some(value) => !matches!(
                value.to_ascii_lowercase().as_str(),
                "false" | "no" | "off" | "0"
            ),
        },
    }
}

fn has_room_for_header(box_: &PixelBox) -> bool {
    box_.height() >= HEADER_HEIGHT + 2 * PADDING + MIN_LABEL_HEIGHT
}

/// Place the children of `parent_box` and then their descendants.
fn place_children(
    children: &[Node],
    parent_box: PixelBox,
    parent_header: bool,
    parent_index: usize,
    depth: usize,
    default_show_value: bool,
    nodes: &mut Vec<NodeLayout>,
) {
    let top = parent_box.y1 + if parent_header { HEADER_HEIGHT } else { 0 } + PADDING;
    let content = PixelBox {
        x1: parent_box.x1 + PADDING,
        y1: top,
        x2: parent_box.x2 - PADDING,
        y2: parent_box.y2 - PADDING,
    };
    if content.width() <= 0 || content.height() <= 0 {
        return;
    }

    // Zero-weight nodes take no room. The stable sort keeps source order for
    // equal weights; the layout algorithm uses the largest rectangles first.
    let mut items: Vec<(usize, f64)> = children
        .iter()
        .enumerate()
        .filter(|(_, child)| child.weight > 0.0)
        .map(|(index, child)| (index, child.weight))
        .collect();
    items.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    if items.is_empty() {
        return;
    }

    let boxes = squarify(&items, content);
    let mut placed: Vec<Option<PixelBox>> = vec![None; children.len()];
    for (index, box_) in boxes {
        placed[index] = Some(inset(box_, SIBLING_GAP / 2));
    }

    for (index, child) in children.iter().enumerate() {
        let Some(box_) = placed[index] else {
            continue;
        };
        let has_children = !child.children.is_empty();
        let header = has_children && has_room_for_header(&box_);
        let show_value = show_value(&child.attrs, default_show_value);
        let node_index = nodes.len();
        nodes.push(NodeLayout {
            label: child.label.clone(),
            value: child.weight,
            attrs: child.attrs.clone(),
            box_,
            depth,
            parent: Some(parent_index),
            has_children,
            header,
            show_value,
        });
        if has_children {
            place_children(
                &child.children,
                box_,
                header,
                node_index,
                depth + 1,
                default_show_value,
                nodes,
            );
        }
    }
}

/// Shrink a rectangle by `amount` on every side, unless it is too small.
fn inset(box_: PixelBox, amount: i32) -> PixelBox {
    let mut out = box_;
    if out.width() > 2 * amount + 1 {
        out.x1 += amount;
        out.x2 -= amount;
    }
    if out.height() > 2 * amount + 1 {
        out.y1 += amount;
        out.y2 -= amount;
    }
    out
}

/// Tile `rect` with the items, largest first.
///
/// `items` are `(index, weight)`. The result maps every item to a rectangle;
/// the rectangles are disjoint and together cover `rect` (before the caller
/// insets them for the visual gap).
fn squarify(items: &[(usize, f64)], rect: PixelBox) -> Vec<(usize, PixelBox)> {
    let mut out = Vec::new();
    if items.is_empty() || rect.width() <= 0 || rect.height() <= 0 {
        return out;
    }
    let total_weight: f64 = items.iter().map(|(_, weight)| *weight).sum();
    if total_weight <= 0.0 {
        return out;
    }
    let total_area = rect.width() as f64 * rect.height() as f64;
    let mut remaining: Vec<(usize, f64)> = items
        .iter()
        .map(|(index, weight)| (*index, weight / total_weight * total_area))
        .collect();
    let mut current = rect;

    while !remaining.is_empty() {
        let short = if current.width() >= current.height() {
            current.height() as f64
        } else {
            current.width() as f64
        };
        if short <= 0.0 {
            break;
        }

        let mut row: Vec<(usize, f64)> = vec![remaining[0]];
        let mut row_area = remaining[0].1;
        let mut worst = worst_ratio(&row, row_area, short);
        let mut take = 1;
        while take < remaining.len() {
            let candidate = remaining[take].1;
            let candidate_area = row_area + candidate;
            let candidate_worst = worst_ratio_with(&row, candidate, candidate_area, short);
            if candidate_worst <= worst {
                row.push(remaining[take]);
                row_area = candidate_area;
                worst = candidate_worst;
                take += 1;
            } else {
                break;
            }
        }

        let rest = remaining.len() - take;
        let vertical = current.width() >= current.height();
        let dimension = if vertical {
            current.width()
        } else {
            current.height()
        };
        let max_thickness = if rest > 0 {
            (dimension - 1).max(1)
        } else {
            dimension
        };
        let thickness = ((row_area / short).round() as i32).clamp(1, max_thickness.max(1));

        let start = if vertical { current.y1 } else { current.x1 };
        let end = if vertical { current.y2 } else { current.x2 };
        let mut previous = start;
        for (position, (index, area)) in row.iter().enumerate() {
            let boundary = if position + 1 == row.len() {
                end
            } else {
                let share = (area / row_area) * (end - start) as f64;
                let raw = start as f64 + share;
                (raw.round() as i32).clamp(previous, end)
            };
            let box_ = if vertical {
                PixelBox {
                    x1: current.x1,
                    y1: previous,
                    x2: current.x1 + thickness,
                    y2: boundary,
                }
            } else {
                PixelBox {
                    x1: previous,
                    y1: current.y1,
                    x2: boundary,
                    y2: current.y1 + thickness,
                }
            };
            if box_.width() > 0 && box_.height() > 0 {
                out.push((*index, box_));
            }
            previous = boundary;
        }

        remaining.drain(..take);
        if remaining.is_empty() {
            break;
        }
        if vertical {
            current.x1 += thickness;
        } else {
            current.y1 += thickness;
        }
    }

    out
}

/// The worst aspect ratio of a row, given the short side of the rectangle.
fn worst_ratio(row: &[(usize, f64)], row_area: f64, short: f64) -> f64 {
    let thickness = row_area / short;
    row.iter()
        .map(|(_, area)| {
            let length = area / thickness;
            (thickness / length).max(length / thickness)
        })
        .fold(0.0, f64::max)
}

/// The worst aspect ratio after adding one more area to the row.
fn worst_ratio_with(row: &[(usize, f64)], extra: f64, row_area: f64, short: f64) -> f64 {
    let thickness = row_area / short;
    let ratio = |area: f64| {
        let length = area / thickness;
        (thickness / length).max(length / thickness)
    };
    row.iter()
        .map(|(_, area)| ratio(*area))
        .chain(std::iter::once(ratio(extra)))
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::super::model;
    use super::super::parser::parse;
    use super::*;

    fn laid_out(source: &str) -> Layout {
        let ast = parse(source).unwrap();
        let document = model::build(&ast).unwrap();
        layout(&document.diagrams[0])
    }

    fn area(box_: &PixelBox) -> i64 {
        box_.width() as i64 * box_.height() as i64
    }

    #[test]
    fn a_flat_list_has_a_root_and_one_box_per_child() {
        let layout = laid_out("treemap { A: 1; B: 2; C: 3; }");
        assert_eq!(layout.nodes.len(), 4);
        assert_eq!(layout.nodes[0].depth, 0);
        assert_eq!(layout.nodes[1].depth, 1);
        assert_eq!(layout.nodes[3].depth, 1);
    }

    #[test]
    fn larger_values_get_larger_areas() {
        let layout = laid_out("treemap { width = 800; height = 500; A: 1; B: 4; }");
        let a = &layout.nodes[1];
        let b = &layout.nodes[2];
        assert!(area(&b.box_) > area(&a.box_), "{a:?} {b:?}");
        let ratio = area(&b.box_) as f64 / area(&a.box_) as f64;
        assert!((ratio - 4.0).abs() < 0.5, "area ratio was {ratio}");
    }

    #[test]
    fn children_stay_inside_their_parent() {
        let layout =
            laid_out("treemap { A: 10 { A1: 4; A2: 6; } B: 5 { B1: 1; B2: 2; B3: 2; } C: 2; }");
        for node in &layout.nodes {
            if let Some(parent) = node.parent {
                let p = &layout.nodes[parent].box_;
                assert!(node.box_.x1 >= p.x1, "{node:?} is left of {p:?}");
                assert!(node.box_.y1 >= p.y1, "{node:?} is above {p:?}");
                assert!(node.box_.x2 <= p.x2, "{node:?} is right of {p:?}");
                assert!(node.box_.y2 <= p.y2, "{node:?} is below {p:?}");
            }
        }
    }

    #[test]
    fn siblings_do_not_overlap() {
        let layout = laid_out("treemap { A: 2; B: 3; C: 5; D: 7; E: 11; }");
        for left in 1..layout.nodes.len() {
            for right in left + 1..layout.nodes.len() {
                let a = &layout.nodes[left].box_;
                let b = &layout.nodes[right].box_;
                let overlap = a.x1 < b.x2 && b.x1 < a.x2 && a.y1 < b.y2 && b.y1 < a.y2;
                assert!(!overlap, "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn zero_weight_nodes_take_no_room() {
        let layout = laid_out("treemap { A: 0; B: 5; }");
        assert_eq!(layout.nodes.len(), 2);
        assert_eq!(layout.nodes[1].label, "B");
    }

    #[test]
    fn dimensions_come_from_the_diagram_attributes() {
        let layout = laid_out("treemap { width = 400; height = 300; A: 1; }");
        assert_eq!((layout.width, layout.height), (400, 300));
        assert!(layout.nodes[0].box_.x2 <= 400);
        assert!(layout.nodes[0].box_.y2 <= 300);
    }

    #[test]
    fn the_layout_is_deterministic() {
        let source = "treemap { A: 2 { A1: 1; A2: 1; } B: 3; C: 4; }";
        assert_eq!(laid_out(source), laid_out(source));
    }

    #[test]
    fn showvalue_is_inherited_and_can_be_overridden() {
        let layout = laid_out("treemap { showvalue = false; A; B [showvalue = true]; }");
        assert!(!layout.nodes[1].show_value);
        assert!(layout.nodes[2].show_value);
    }
}
