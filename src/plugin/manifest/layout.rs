//! Layout: the manifest -> pixel geometry.
//!
//! From top to bottom: an optional title, the column-group band, the
//! inheritance band (one track per `extends` link, so the brackets never
//! cross), the rotated column labels, and the rows. Rows are as tall as a
//! line of text; columns are one cell wide, so the manifest grows by a cell per
//! column and stays readable with many of them. The row labels and the count
//! sit left and right of the cells.

use crate::core::geom::PixelBox;
use crate::text::TextMeasurer;

use super::model::Diagram;

/// Space around the whole manifest.
pub const MARGIN: i32 = 20;
/// The width of one column of cells.
pub const CELL_WIDTH: i32 = 30;
/// The height of one row.
pub const ROW_HEIGHT: i32 = 26;
/// Padding inside labels.
pub const PADDING: i32 = 8;
/// The height of the column-group band.
pub const GROUP_HEIGHT: i32 = 26;
/// The vertical distance between two inheritance tracks.
pub const TRACK_HEIGHT: i32 = 10;
/// The height of the title.
pub const TITLE_HEIGHT: i32 = 32;
/// The width of the count column (number and bar).
pub const COUNT_WIDTH: i32 = 90;
/// The size of the hide/show toggle of an interactive manifest.
pub const TOGGLE: i32 = 12;
/// The size of a folded (hidden) row or column in a bundle.
pub const FOLD: i32 = 12;
/// The height of the legend.
pub const LEGEND_HEIGHT: i32 = 32;

/// An `extends` link drawn in the inheritance band.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub child: usize,
    pub parent: usize,
    /// The y of the horizontal part of the bracket.
    pub y: i32,
}

/// A displayed row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowLayout {
    /// Index into `Diagram::rows`.
    pub row: usize,
    /// The whole row, from the label to the count.
    pub box_: PixelBox,
}

/// The complete geometry of a manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub width: i32,
    pub height: i32,
    pub title: Option<PixelBox>,
    /// `(group index, band box)`.
    pub groups: Vec<(usize, PixelBox)>,
    /// The band with the inheritance brackets (empty height when unused).
    pub links_band: PixelBox,
    pub links: Vec<Link>,
    /// The header box of every column (where its rotated label goes),
    /// indexed like `Diagram::columns`. A hidden column has a zero-width box.
    pub columns: Vec<PixelBox>,
    /// The columns that are drawn, left to right.
    pub visible: Vec<usize>,
    /// The header box of the count column, when it is shown.
    pub count_header: Option<PixelBox>,
    /// The left and right x of the row labels.
    pub labels: (i32, i32),
    /// The left x of the count column, when it is shown.
    pub count_x: Option<i32>,
    pub rows: Vec<RowLayout>,
    /// `(row group index, header row box)`.
    pub row_groups: Vec<(usize, PixelBox)>,
    /// The legend band, when it is shown.
    pub legend: Option<PixelBox>,
}

impl Layout {
    /// The x of the centre of column `c`.
    pub fn column_center(&self, c: usize) -> i32 {
        let b = &self.columns[c];
        (b.x1 + b.x2) / 2
    }

    /// The box of the cell at display row `r`, column `c`.
    pub fn cell(&self, r: usize, c: usize) -> PixelBox {
        let row = &self.rows[r].box_;
        let column = &self.columns[c];
        PixelBox {
            x1: column.x1,
            y1: row.y1,
            x2: column.x2,
            y2: row.y2,
        }
    }
}

fn flag(diagram: &Diagram, key: &str) -> bool {
    !matches!(diagram.attr(key), Some("false" | "no" | "0" | "off"))
}

/// Whether the count column is shown (`showcount`, default on).
pub fn show_count(diagram: &Diagram) -> bool {
    flag(diagram, "showcount")
}

/// The label of the count column (`countlabel`, default `count`).
pub fn count_label(diagram: &Diagram) -> &str {
    diagram.attr("countlabel").unwrap_or("count")
}

/// Lay out a manifest.
///
/// An `interactive` manifest (an HTML bundle) reserves room for the hide/show
/// toggles next to the row labels and above the column labels.
pub fn layout(diagram: &Diagram, text: &mut TextMeasurer, interactive: bool) -> Layout {
    let toggle = if interactive { TOGGLE + PADDING / 2 } else { 0 };
    let width_of =
        |text: &mut TextMeasurer, value: &str| text.measure(value, None).width.ceil() as i32;

    let label_width = diagram
        .rows
        .iter()
        .map(|row| row.label.as_str())
        .chain(diagram.row_groups.iter().map(|g| g.label.as_str()))
        .map(|label| width_of(text, label))
        .max()
        .unwrap_or(0)
        .max(40)
        + 2 * PADDING
        + toggle;
    let show_count = show_count(diagram);
    let header_height = diagram
        .columns
        .iter()
        .map(|column| column.label.as_str())
        .chain(show_count.then(|| count_label(diagram)))
        .map(|label| width_of(text, label))
        .max()
        .unwrap_or(0)
        .max(24)
        + 2 * PADDING
        + toggle;

    let mut y = MARGIN;
    let title = diagram.attr("title").map(|_| {
        let b = PixelBox {
            x1: MARGIN,
            y1: y,
            x2: MARGIN,
            y2: y + TITLE_HEIGHT,
        };
        y += TITLE_HEIGHT;
        b
    });

    let labels = (MARGIN, MARGIN + label_width);
    let cells_x = labels.1;
    // The slot of every column: hidden columns take no room.
    let visible = diagram.visible_columns();
    let mut slot = vec![0usize; diagram.columns.len()];
    let mut next = 0;
    for (c, s) in slot.iter_mut().enumerate() {
        *s = next;
        if !diagram.columns[c].hidden {
            next += 1;
        }
    }
    // The width of every visible slot: a cell, or wider when the columns of
    // a group are too narrow for its label (the extra is shared out).
    let mut widths = vec![CELL_WIDTH; visible.len()];
    let mut i = 0;
    while i < visible.len() {
        let group = diagram.columns[visible[i]].group;
        let start = i;
        while i < visible.len() && diagram.columns[visible[i]].group == group {
            i += 1;
        }
        if let Some(g) = group {
            let need = width_of(text, &diagram.groups[g].label) + 2 * PADDING;
            let count = (i - start) as i32;
            let extra = (need - count * CELL_WIDTH).max(0);
            for (k, w) in widths[start..i].iter_mut().enumerate() {
                let k = k as i32;
                *w += extra * (k + 1) / count - extra * k / count;
            }
        }
    }
    let mut offsets = vec![cells_x];
    for w in &widths {
        offsets.push(offsets.last().unwrap() + w);
    }
    let column_x = |slot: usize| offsets[slot];
    let cells_end = column_x(visible.len());

    // Column groups: one band over each run of visible columns of a group.
    let mut groups = Vec::new();
    if visible.iter().any(|&c| diagram.columns[c].group.is_some()) {
        let mut i = 0;
        while i < visible.len() {
            let group = diagram.columns[visible[i]].group;
            let start = i;
            while i < visible.len() && diagram.columns[visible[i]].group == group {
                i += 1;
            }
            if let Some(group) = group {
                groups.push((
                    group,
                    PixelBox {
                        x1: column_x(start),
                        y1: y,
                        x2: column_x(i),
                        y2: y + GROUP_HEIGHT,
                    },
                ));
            }
        }
        y += GROUP_HEIGHT;
    }

    // Inheritance links between visible columns, each on the first track
    // where it fits.
    let mut spans: Vec<(usize, usize)> = visible
        .iter()
        .filter_map(|&c| diagram.columns[c].parent.map(|p| (c, p)))
        .filter(|&(_, p)| !diagram.columns[p].hidden)
        .collect();
    spans.sort_by_key(|&(c, p)| (slot[c].min(slot[p]), slot[c].max(slot[p])));
    let mut track_ends: Vec<usize> = Vec::new();
    let mut assigned = Vec::new();
    for (child, parent) in spans {
        let (lo, hi) = (slot[child].min(slot[parent]), slot[child].max(slot[parent]));
        let track = match track_ends.iter().position(|&end| end < lo) {
            Some(t) => t,
            None => {
                track_ends.push(0);
                track_ends.len() - 1
            }
        };
        track_ends[track] = hi;
        assigned.push((child, parent, track));
    }
    let band_height = if track_ends.is_empty() {
        0
    } else {
        track_ends.len() as i32 * TRACK_HEIGHT + TRACK_HEIGHT
    };
    let links_band = PixelBox {
        x1: cells_x,
        y1: y,
        x2: cells_end,
        y2: y + band_height,
    };
    // Track 0 is the lowest, closest to the labels.
    let links = assigned
        .into_iter()
        .map(|(child, parent, track)| Link {
            child,
            parent,
            y: links_band.y2 - TRACK_HEIGHT * (track as i32 + 1),
        })
        .collect();
    y += band_height;

    let columns = (0..diagram.columns.len())
        .map(|c| {
            let width = if diagram.columns[c].hidden { 0 } else { 1 };
            PixelBox {
                x1: column_x(slot[c]),
                y1: y,
                x2: column_x(slot[c] + width),
                y2: y + header_height,
            }
        })
        .collect();
    let count_x = show_count.then_some(cells_end);
    let count_header = count_x.map(|x| PixelBox {
        x1: x,
        y1: y,
        x2: x + CELL_WIDTH,
        y2: y + header_height,
    });
    y += header_height;
    let right = cells_end + if show_count { COUNT_WIDTH } else { 0 };

    let mut rows = Vec::new();
    let mut row_groups = Vec::new();
    let mut current = None;
    for r in diagram.row_order() {
        let group = diagram.rows[r].group;
        if group != current {
            if let Some(g) = group {
                row_groups.push((
                    g,
                    PixelBox {
                        x1: MARGIN,
                        y1: y,
                        x2: right,
                        y2: y + ROW_HEIGHT,
                    },
                ));
                y += ROW_HEIGHT;
            }
            current = group;
        }
        rows.push(RowLayout {
            row: r,
            box_: PixelBox {
                x1: MARGIN,
                y1: y,
                x2: right,
                y2: y + ROW_HEIGHT,
            },
        });
        y += ROW_HEIGHT;
    }

    let legend = flag(diagram, "legend").then(|| {
        let b = PixelBox {
            x1: MARGIN,
            y1: y + PADDING,
            x2: right,
            y2: y + PADDING + LEGEND_HEIGHT,
        };
        y = b.y2;
        b
    });

    let mut width = right + MARGIN;
    if let Some(title) = diagram.attr("title") {
        width = width.max(2 * MARGIN + width_of(text, title));
    }
    let title = title.map(|b| PixelBox {
        x2: width - MARGIN,
        ..b
    });

    Layout {
        width,
        height: y + MARGIN,
        title,
        groups,
        links_band,
        links,
        columns,
        visible,
        count_header,
        labels,
        count_x,
        rows,
        row_groups,
        legend,
    }
}

#[cfg(test)]
mod tests {
    use super::super::model;
    use super::super::parser::parse;
    use super::*;

    fn layout_of(source: &str) -> Layout {
        let document = model::build(&parse(source).unwrap()).unwrap();
        layout(
            &document.diagrams[0],
            &mut TextMeasurer::default_size(),
            false,
        )
    }

    #[test]
    fn columns_are_one_cell_wide_and_rows_one_line_tall() {
        let l = layout_of("manifest { column a { x; y; } column b { y; } }");
        assert_eq!(l.columns[0].width(), CELL_WIDTH);
        assert_eq!(l.columns[1].x1, l.columns[0].x2);
        assert_eq!(l.rows.len(), 2);
        assert_eq!(l.rows[1].box_.y1, l.rows[0].box_.y2);
        assert!(l.rows[0].box_.y1 >= l.columns[0].y2);
    }

    #[test]
    fn overlapping_links_get_separate_tracks() {
        let l = layout_of(
            "manifest { column a; column b [extends = a]; column c [extends = a]; column d [extends = c]; }",
        );
        assert_eq!(l.links.len(), 3);
        let ab = l.links.iter().find(|k| k.child == 1).unwrap();
        let ac = l.links.iter().find(|k| k.child == 2).unwrap();
        let cd = l.links.iter().find(|k| k.child == 3).unwrap();
        assert_ne!(ab.y, ac.y);
        // c -> d starts where a -> c ends, so they cannot share a track.
        assert_ne!(ac.y, cd.y);
        for link in &l.links {
            assert!(link.y > l.links_band.y1 && link.y < l.links_band.y2);
        }
    }

    #[test]
    fn no_links_no_band_and_no_count_when_disabled() {
        let l = layout_of("manifest { showcount = false; legend = false; column a { x; } }");
        assert_eq!(l.links_band.height(), 0);
        assert!(l.count_x.is_none());
        assert!(l.legend.is_none());
        assert_eq!(l.width, l.columns[0].x2 + MARGIN);
    }

    #[test]
    fn hidden_columns_take_no_room_and_lose_their_links() {
        let source = "manifest { column a { x; } column b [extends = a]; column c [extends = a]; }";
        let mut d = model::build(&parse(source).unwrap())
            .unwrap()
            .diagrams
            .remove(0);
        d.apply_view(["c1"]);
        let l = layout(&d, &mut TextMeasurer::default_size(), false);
        assert_eq!(l.visible, vec![0, 2]);
        assert_eq!(l.columns[1].width(), 0);
        assert_eq!(l.columns[2].x1, l.columns[0].x2);
        assert_eq!(l.links.len(), 1);
        assert_eq!(l.links[0].child, 2);
    }

    #[test]
    fn a_group_band_grows_to_fit_its_label() {
        let l = layout_of(
            "manifest { group \"a rather long group name\" { column a { x; } } column b { x; } }",
        );
        let band = l.groups[0].1;
        assert!(band.width() > CELL_WIDTH);
        assert_eq!(
            band,
            PixelBox {
                y1: band.y1,
                y2: band.y2,
                ..l.columns[0]
            }
        );
        assert_eq!(l.columns[1].width(), CELL_WIDTH);
        assert_eq!(l.columns[1].x1, l.columns[0].x2);
    }

    #[test]
    fn row_groups_get_a_header_row() {
        let l = layout_of("manifest { column a { x; } rowgroup g { y: a; } }");
        assert_eq!(l.row_groups.len(), 1);
        assert_eq!(l.row_groups[0].1.y2, l.rows[1].box_.y1);
    }
}
