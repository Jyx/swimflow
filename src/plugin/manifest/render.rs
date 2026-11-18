//! Rendering: the manifest layout -> SVG.
//!
//! The palette is the one of the other diagram types: group bands and row
//! group headers get the lane-header yellow, outlines and text are black.
//! A cell shows why the row is in the column:
//!
//! * a filled dot: a direct member;
//! * a hollow dot: inherited through `extends`;
//! * a dot with a ring: inherited and overridden;
//! * a grey cross: inherited and removed.
//!
//! Every cell, row and column carries `data-row`/`data-col` so the bundle
//! runtime (`runtime.js`) can highlight and filter.

use crate::core::geom::PixelBox;
use crate::core::svg::{self, TextAlign, TextStyle};
use crate::render::RenderOptions;
use crate::text::TextMeasurer;

use super::layout::{self, CELL_WIDTH, FOLD, Layout, PADDING, ROW_HEIGHT, TOGGLE, TRACK_HEIGHT};

/// The corner radius of a `shape = roundedbox` column group.
const GROUP_RADIUS: i32 = 6;
use super::model::{Diagram, Mark};

const GROUP_FILL: &str = "#ffff99";
const LINE: &str = "#000000";
const GRID: &str = "#cccccc";
const STRIPE: &str = "#f4f4f4";
const TEXT_COLOR: &str = "#000000";
const REMOVED: &str = "#999999";
const BAR: &str = "#bbbbbb";
/// The fill of a folded (hidden) row or column in a bundle.
const FOLD_FILL: &str = "#e9ecef";
/// The radius of a mark.
const DOT: i32 = 6;

/// Render a manifest as an SVG document.
///
/// `interactive` adds the controls of an HTML bundle: a hide toggle on every
/// row label and column header, a folded form with a `+` for every row and
/// column, and clickable legend entries. It also records the original
/// geometry (`data-x`, `data-y`, …) so `runtime.js` can fold rows and columns
/// by moving elements, without laying the manifest out again. `initial` are the
/// tokens hidden when the page opens (see [`Diagram::apply_view`]).
pub fn render(
    diagram: &Diagram,
    options: &RenderOptions,
    interactive: bool,
    initial: &[String],
) -> String {
    let mut text = TextMeasurer::from_spec(&options.font);
    let layout = layout::layout(diagram, &mut text, interactive);

    let mut out = String::new();
    svg::open_document(
        &mut out,
        &svg::DocumentOptions {
            width: layout.width,
            height: layout.height,
            font: &options.font,
            embed_font: options.embed_font,
            text_as_paths: options.text_as_paths,
            nodoctype: options.nodoctype,
            noviewbox: options.noviewbox,
            transparent: options.transparent,
        },
    );
    let mut d = Drawer {
        out: &mut out,
        text: &mut text,
        options,
        diagram,
        layout: &layout,
        interactive,
        initial,
    };
    d.out.push_str(&format!(
        "<g class=\"swimflow-manifest\" data-cells-x=\"{}\" data-fold=\"{FOLD}\" \
         data-width=\"{}\" data-height=\"{}\" data-hidden-default=\"{}\">\n",
        layout.labels.1,
        layout.width,
        layout.height,
        svg::escape_xml(&initial.join(" "))
    ));
    d.title();
    d.groups();
    d.links();
    d.column_headers();
    d.rows();
    d.separators();
    d.legend();
    d.out.push_str("</g>\n");
    svg::close_document(&mut out);
    out
}

fn row_id(r: usize) -> String {
    format!("r{r}")
}

fn column_id(c: usize) -> String {
    format!("c{c}")
}

struct Drawer<'a> {
    out: &'a mut String,
    text: &'a mut TextMeasurer,
    options: &'a RenderOptions,
    diagram: &'a Diagram,
    layout: &'a Layout,
    interactive: bool,
    initial: &'a [String],
}

impl Drawer<'_> {
    fn label(&mut self, box_: &PixelBox, value: &str, color: &str, align: TextAlign) {
        let style = TextStyle {
            color,
            text_as_paths: self.options.text_as_paths,
            align,
            underline: false,
        };
        // A single line: a label never wraps.
        let box_ = PixelBox {
            x2: box_
                .x2
                .max(box_.x1 + self.text.measure(value, None).width.ceil() as i32),
            ..*box_
        };
        svg::draw_text(self.out, self.text, &box_, value, &style);
    }

    fn open_link(&mut self, href: Option<&str>) -> bool {
        match href.filter(|h| !h.is_empty() && !self.options.no_external) {
            Some(href) => {
                self.out
                    .push_str(&format!("<a href=\"{}\">\n", svg::escape_xml(href)));
                true
            }
            None => false,
        }
    }

    fn title(&mut self) {
        if let (Some(b), Some(title)) = (self.layout.title, self.diagram.attr("title")) {
            self.out.push_str("<g class=\"manifest-title\">\n");
            self.label(&b, title, TEXT_COLOR, TextAlign::Left);
            self.out.push_str("</g>\n");
        }
    }

    fn groups(&mut self) {
        for &(g, b) in &self.layout.groups {
            let group = &self.diagram.groups[g];
            let fill = paint(group.attr("color"), GROUP_FILL);
            let members: Vec<String> = self
                .layout
                .visible
                .iter()
                .filter(|&&c| self.diagram.columns[c].group == Some(g))
                .map(|&c| column_id(c))
                .collect();
            self.out.push_str(&format!(
                "<g class=\"manifest-group\" data-group=\"g{g}\" data-cols=\"{}\" data-x=\"{}\" \
                 data-w=\"{}\">\n",
                members.join(" "),
                b.x1,
                b.width()
            ));
            if group.attr("shape") == Some("roundedbox") {
                svg::rounded_rect(self.out, &b, GROUP_RADIUS, &fill, LINE, "");
            } else {
                svg::rect(self.out, &b, &fill, LINE, "");
            }
            let inner = inset(&b, PADDING / 2);
            let fitted = fit(self.text, &group.label, inner.width() as f32);
            if let Some(fitted) = fitted {
                let color = paint(group.attr("textcolor"), TEXT_COLOR);
                self.out.push_str("<g class=\"manifest-group-label\">\n");
                self.label(&inner, &fitted, &color, TextAlign::Center);
                self.out.push_str("</g>\n");
            }
            self.out.push_str(&format!(
                "<title>{}</title>\n</g>\n",
                svg::escape_xml(&group.label)
            ));
        }
    }

    /// The `extends` brackets: from the child up to its track, across, and
    /// down to the parent, which gets a hollow triangle (as a UML
    /// generalization).
    fn links(&mut self) {
        let bottom = self.layout.links_band.y2;
        for link in &self.layout.links {
            let child = self.layout.column_center(link.child);
            let parent = self.layout.column_center(link.parent);
            let tip = bottom - 1;
            let base = tip - TRACK_HEIGHT / 2 - 1;
            self.out.push_str(&format!(
                "<g class=\"manifest-link\" data-col=\"{}\" data-parent=\"{}\" data-track=\"{}\" \
                 data-bottom=\"{bottom}\" data-base=\"{base}\">\n",
                column_id(link.child),
                column_id(link.parent),
                link.y
            ));
            svg::path(
                self.out,
                &[
                    (child, bottom),
                    (child, link.y),
                    (parent, link.y),
                    (parent, base),
                ],
                LINE,
                "",
            );
            svg::polygon(
                self.out,
                &[(parent - 4, base), (parent + 4, base), (parent, tip)],
                "#ffffff",
                LINE,
            );
            self.out.push_str(&format!(
                "<title>{}</title>\n</g>\n",
                svg::escape_xml(&format!(
                    "{} extends {}",
                    self.diagram.columns[link.child].label, self.diagram.columns[link.parent].label
                ))
            ));
        }
    }

    /// A label rotated a quarter turn, reading upwards from the bottom of
    /// `b`, centred on its width.
    fn rotated(&mut self, b: &PixelBox, value: &str, color: &str) {
        let center = (b.x1 + b.x2) / 2;
        let height = CELL_WIDTH;
        self.out.push_str(&format!(
            "<g transform=\"translate({center} {}) rotate(-90)\">\n",
            b.y2 - PADDING
        ));
        let local = PixelBox {
            x1: 0,
            y1: -height / 2,
            x2: (b.height() - 2 * PADDING).max(1),
            y2: height / 2,
        };
        self.label(&local, value, color, TextAlign::Left);
        self.out.push_str("</g>\n");
    }

    /// The hide/show control of a row or a column (bundle only): a small
    /// circle with a cross. The runtime toggles the token on a click.
    fn toggle(&mut self, token: &str, cx: i32, cy: i32) {
        if !self.interactive {
            return;
        }
        let k = TOGGLE / 2 - 3;
        self.out.push_str(&format!(
            "<g class=\"manifest-toggle\" data-toggle=\"{token}\">\n"
        ));
        svg::ellipse(self.out, cx, cy, TOGGLE / 2, TOGGLE / 2, "#ffffff", REMOVED);
        let style = " stroke-width=\"1.5\"";
        svg::line(self.out, cx - k, cy - k, cx + k, cy + k, REMOVED, style);
        svg::line(self.out, cx - k, cy + k, cx + k, cy - k, REMOVED, style);
        self.out.push_str("<title>Hide or show</title>\n</g>\n");
    }

    /// The `+` that unfolds a hidden row or column (bundle only).
    fn expand(&mut self, token: &str, cx: i32, cy: i32) {
        let radius = FOLD / 2 - 1;
        let k = radius - 2;
        self.out.push_str(&format!(
            "<g class=\"manifest-expand\" data-toggle=\"{token}\">\n"
        ));
        svg::ellipse(self.out, cx, cy, radius, radius, "#ffffff", REMOVED);
        let style = " stroke-width=\"1.5\"";
        svg::line(self.out, cx - k, cy, cx + k, cy, REMOVED, style);
        svg::line(self.out, cx, cy - k, cx, cy + k, REMOVED, style);
        self.out.push_str("<title>Show again</title>\n</g>\n");
    }

    /// The bottom of the last row.
    fn table_bottom(&self) -> i32 {
        self.layout.rows.last().map_or(0, |row| row.box_.y2)
    }

    fn column_headers(&mut self) {
        for c in self.layout.visible.clone() {
            let column = &self.diagram.columns[c];
            let b = self.layout.columns[c];
            self.out.push_str(&format!(
                "<g class=\"manifest-column\" data-col=\"{}\" data-x=\"{}\">\n",
                column_id(c),
                b.x1
            ));
            let linked = self.open_link(column.attr("href"));
            let fill = paint(column.attr("color"), "none");
            svg::rect(self.out, &b, &fill, GRID, "");
            let color = paint(column.attr("textcolor"), TEXT_COLOR);
            self.rotated(&b, &column.label, &color);
            let mut tip = column.label.clone();
            if let Some(parent) = column.parent {
                tip.push_str(&format!(
                    " (extends {})",
                    self.diagram.columns[parent].label
                ));
            }
            let members = (0..self.diagram.rows.len())
                .filter(|&r| self.diagram.cells[r][c].mark.present())
                .count();
            tip.push_str(&format!(": {members}"));
            self.out
                .push_str(&format!("<title>{}</title>\n", svg::escape_xml(&tip)));
            if linked {
                self.out.push_str("</a>\n");
            }
            self.toggle(&column_id(c), (b.x1 + b.x2) / 2, b.y1 + TOGGLE / 2 + 2);
            self.out.push_str("</g>\n");
            if self.interactive {
                let height = self.table_bottom().max(b.y2) - b.y1;
                self.out.push_str(&format!(
                    "<g class=\"manifest-column-fold\" data-col=\"{}\" display=\"none\">\n\
                     <rect x=\"{}\" y=\"{}\" width=\"{FOLD}\" height=\"{height}\" \
                     data-h=\"{height}\" fill=\"{FOLD_FILL}\" stroke=\"{GRID}\"/>\n",
                    column_id(c),
                    b.x1,
                    b.y1
                ));
                self.expand(&column_id(c), b.x1 + FOLD / 2, b.y1 + TOGGLE / 2 + 2);
                self.out.push_str("</g>\n");
            }
        }
        if let Some(b) = self.layout.count_header {
            self.out.push_str("<g class=\"manifest-count-header\">\n");
            let label = layout::count_label(self.diagram).to_string();
            self.rotated(&b, &label, TEXT_COLOR);
            self.out.push_str("</g>\n");
        }
    }

    fn rows(&mut self) {
        for &(g, b) in &self.layout.row_groups {
            let group = &self.diagram.row_groups[g];
            let fill = paint(group.attr("color"), GROUP_FILL);
            self.out.push_str(&format!(
                "<g class=\"manifest-rowgroup\" data-group=\"rg{g}\" data-y=\"{}\">\n",
                b.y1
            ));
            svg::rect(self.out, &b, &fill, LINE, "");
            let color = paint(group.attr("textcolor"), TEXT_COLOR);
            let inner = inset(&b, PADDING);
            self.label(&inner, &group.label, &color, TextAlign::Left);
            self.out.push_str("</g>\n");
        }

        let columns = self.diagram.columns.len();
        let visible = self.layout.visible.clone();
        let toggle_room = if self.interactive {
            TOGGLE + PADDING / 2
        } else {
            0
        };
        for (i, row_layout) in self.layout.rows.iter().enumerate() {
            let r = row_layout.row;
            let row = &self.diagram.rows[r];
            let b = row_layout.box_;
            let members: Vec<String> = (0..columns)
                .filter(|&c| self.diagram.cells[r][c].mark.present())
                .map(column_id)
                .collect();
            self.out.push_str(&format!(
                "<g class=\"manifest-row\" data-row=\"{}\" data-cols=\"{}\" data-count=\"{}\" \
                 data-y=\"{}\" data-h=\"{ROW_HEIGHT}\">\n<g class=\"manifest-row-body\">\n",
                row_id(r),
                members.join(" "),
                row.count,
                b.y1
            ));
            let stripe = if i % 2 == 1 { STRIPE } else { "#ffffff" };
            svg::rect(
                self.out,
                &b,
                &paint(row.attr("color"), stripe),
                "none",
                &format!(" class=\"manifest-stripe\" data-w=\"{}\"", b.width()),
            );

            // The label.
            let linked = self.open_link(row.attr("href"));
            let label_box = PixelBox {
                x1: self.layout.labels.0 + PADDING,
                y1: b.y1,
                x2: self.layout.labels.1 - PADDING - toggle_room,
                y2: b.y2,
            };
            let color = paint(row.attr("textcolor"), TEXT_COLOR);
            self.out.push_str(&format!(
                "<g class=\"manifest-row-label\" data-row=\"{}\">\n",
                row_id(r)
            ));
            self.label(&label_box, &row.label, &color, TextAlign::Left);
            self.out.push_str(&format!(
                "<title>{}</title>\n</g>\n",
                svg::escape_xml(&format!("{}: {}", row.label, row.count))
            ));
            if linked {
                self.out.push_str("</a>\n");
            }
            self.toggle(
                &row_id(r),
                self.layout.labels.1 - PADDING / 2 - TOGGLE / 2,
                (b.y1 + b.y2) / 2,
            );

            for &c in &visible {
                self.cell(i, r, c);
            }
            self.count(&b, r);
            self.out.push_str("</g>\n");
            if self.interactive {
                self.out.push_str(&format!(
                    "<g class=\"manifest-row-fold\" display=\"none\">\n\
                     <rect x=\"{}\" y=\"{}\" width=\"{w}\" height=\"{FOLD}\" data-w=\"{w}\" \
                     fill=\"{FOLD_FILL}\" stroke=\"{GRID}\"/>\n",
                    b.x1,
                    b.y1,
                    w = b.width()
                ));
                self.expand(&row_id(r), b.x1 - TOGGLE / 2 - 2, b.y1 + FOLD / 2);
                self.out.push_str("</g>\n");
            }
            self.out.push_str("</g>\n");
        }
    }

    fn cell(&mut self, i: usize, r: usize, c: usize) {
        let b = self.layout.cell(i, c);
        let cell = &self.diagram.cells[r][c];
        let column = &self.diagram.columns[c];
        self.out.push_str(&format!(
            "<g class=\"manifest-cell\" data-row=\"{}\" data-col=\"{}\" data-mark=\"{}\"{}>\n",
            row_id(r),
            column_id(c),
            cell.mark.name(),
            if self.starts_hidden(cell.mark.name()) {
                " data-mark-hidden=\"1\""
            } else {
                ""
            }
        ));
        svg::rect(self.out, &b, "none", GRID, "");
        let (cx, cy) = ((b.x1 + b.x2) / 2, (b.y1 + b.y2) / 2);
        let color = paint(
            self.diagram.rows[r]
                .attr("markcolor")
                .or(column.attr("markcolor")),
            LINE,
        );
        if self.diagram.hidden_marks.contains(cell.mark.name()) {
            self.out.push_str("</g>\n");
            return;
        }
        glyph(self.out, cell.mark.name(), cx, cy, &color);
        let from = |from: usize| self.diagram.columns[from].label.clone();
        let why = match cell.mark {
            Mark::None => None,
            Mark::Direct => Some(String::new()),
            Mark::Inherited { from: f } => Some(format!(" (inherited from {})", from(f))),
            Mark::Override { from: f } => Some(format!(" (overrides {})", from(f))),
            Mark::Removed { from: f } => Some(format!(" (removed, inherited from {})", from(f))),
        };
        if let Some(why) = why {
            let mut tip = format!("{} in {}{why}", self.diagram.rows[r].label, column.label);
            if let Some(note) = &cell.note {
                tip.push_str(&format!(": {note}"));
            }
            self.out
                .push_str(&format!("<title>{}</title>\n", svg::escape_xml(&tip)));
        }
        self.out.push_str("</g>\n");
    }

    /// Whether marks of `kind` start hidden in a bundle.
    fn starts_hidden(&self, kind: &str) -> bool {
        self.interactive
            && self
                .initial
                .iter()
                .any(|t| t.strip_prefix("m:") == Some(kind))
    }

    /// The number of columns the row is in, and a bar of the same length.
    fn count(&mut self, b: &PixelBox, r: usize) {
        let Some(x) = self.layout.count_x else {
            return;
        };
        let count = self.diagram.rows[r].count;
        let columns = self.layout.visible.len().max(1);
        self.out.push_str("<g class=\"manifest-count\">\n");
        let number = PixelBox {
            x1: x,
            y1: b.y1,
            x2: x + CELL_WIDTH - PADDING / 2,
            y2: b.y2,
        };
        self.label(&number, &count.to_string(), TEXT_COLOR, TextAlign::Right);
        let full = layout::COUNT_WIDTH - CELL_WIDTH - PADDING;
        let bar = (full * count as i32) / columns as i32;
        if bar > 0 {
            let bar_box = PixelBox {
                x1: x + CELL_WIDTH,
                y1: b.y1 + 7,
                x2: x + CELL_WIDTH + bar,
                y2: b.y2 - 7,
            };
            svg::rect(self.out, &bar_box, BAR, "none", "");
        }
        self.out.push_str("</g>\n");
    }

    /// Outlines: around the cells, and a heavier line between column groups.
    /// Every line says which edge it follows (`data-edge`: `labels`, `end`,
    /// `l:cN` or `r:cN`), so the runtime can move it when columns fold.
    fn separators(&mut self) {
        let (Some(first), Some(last)) = (self.layout.rows.first(), self.layout.rows.last()) else {
            return;
        };
        let top = self
            .layout
            .visible
            .first()
            .map_or(first.box_.y1, |&c| self.layout.columns[c].y1);
        let bottom = last.box_.y2;
        let outline = PixelBox {
            x1: self.layout.labels.0,
            y1: first.box_.y1,
            x2: first.box_.x2,
            y2: bottom,
        };
        svg::rect(
            self.out,
            &outline,
            "none",
            LINE,
            &format!(
                " class=\"manifest-outline\" data-w=\"{}\" data-h=\"{}\"",
                outline.width(),
                outline.height()
            ),
        );
        let mut edges: Vec<(i32, String, i32)> =
            vec![(self.layout.labels.1, "labels".into(), first.box_.y1)];
        for &(g, _) in &self.layout.groups {
            let members: Vec<usize> = self
                .layout
                .visible
                .iter()
                .copied()
                .filter(|&c| self.diagram.columns[c].group == Some(g))
                .collect();
            if let (Some(&a), Some(&z)) = (members.first(), members.last()) {
                edges.push((
                    self.layout.columns[a].x1,
                    format!("l:{}", column_id(a)),
                    top,
                ));
                edges.push((
                    self.layout.columns[z].x2,
                    format!("r:{}", column_id(z)),
                    top,
                ));
            }
        }
        if let Some(x) = self.layout.count_x {
            edges.push((x, "end".into(), first.box_.y1));
        }
        edges.sort_by_key(|(x, _, y)| (*x, *y));
        edges.dedup_by_key(|(x, _, _)| *x);
        for (x, edge, y1) in edges {
            svg::line(
                self.out,
                x,
                y1,
                x,
                bottom,
                LINE,
                &format!(" data-edge=\"{edge}\" data-y2=\"{bottom}\""),
            );
        }
    }

    fn legend(&mut self) {
        let Some(b) = self.layout.legend else {
            return;
        };
        let used = |name: &str| {
            !self.diagram.hidden_marks.contains(name)
                && self
                    .diagram
                    .cells
                    .iter()
                    .flatten()
                    .any(|cell| cell.mark.name() == name)
        };
        let entries: Vec<(&str, &str)> = [
            ("direct", "member"),
            ("inherited", "inherited"),
            ("override", "overridden"),
            ("removed", "removed"),
        ]
        .into_iter()
        .filter(|(name, _)| used(name))
        .collect();
        if entries.len() < 2 {
            return;
        }
        self.out.push_str(&format!(
            "<g class=\"manifest-legend\" data-y=\"{}\">\n",
            b.y1
        ));
        let cy = (b.y1 + b.y2) / 2;
        let mut x = b.x1 + PADDING;
        for (name, label) in entries {
            let cx = x + DOT;
            if self.interactive {
                let hidden = if self.starts_hidden(name) {
                    " data-hidden=\"1\""
                } else {
                    ""
                };
                self.out.push_str(&format!(
                    "<g class=\"manifest-legend-entry\" data-toggle=\"m:{name}\"{hidden}>\n\
                     <title>Hide or show every {label} mark</title>\n"
                ));
            }
            glyph(self.out, name, cx, cy, LINE);
            let width = self.text.measure(label, None).width.ceil() as i32;
            let text_box = PixelBox {
                x1: cx + DOT + PADDING,
                y1: b.y1,
                x2: cx + DOT + PADDING + width,
                y2: b.y2,
            };
            self.label(&text_box, label, TEXT_COLOR, TextAlign::Left);
            if self.interactive {
                // A transparent hit area over the mark and its label.
                let hit = PixelBox {
                    x1: x - PADDING / 2,
                    y1: b.y1,
                    x2: text_box.x2 + PADDING / 2,
                    y2: b.y2,
                };
                svg::rect(self.out, &hit, "transparent", "none", "");
                self.out.push_str("</g>\n");
            }
            x = text_box.x2 + 2 * PADDING;
        }
        self.out.push_str("</g>\n");
    }
}

/// Draw the mark called `name` (see [`Mark::name`]) centred on `cx, cy`.
fn glyph(out: &mut String, name: &str, cx: i32, cy: i32, color: &str) {
    match name {
        "direct" => svg::ellipse(out, cx, cy, DOT, DOT, color, color),
        "inherited" => out.push_str(&format!(
            "<ellipse cx=\"{cx}\" cy=\"{cy}\" rx=\"{r}\" ry=\"{r}\" fill=\"#ffffff\" \
             stroke=\"{color}\" stroke-width=\"2\"/>\n",
            r = DOT - 1
        )),
        "override" => {
            svg::ellipse(out, cx, cy, DOT + 3, DOT + 3, "none", color);
            svg::ellipse(out, cx, cy, DOT - 2, DOT - 2, color, color);
        }
        "removed" => {
            let k = DOT - 1;
            let style = " stroke-width=\"2\"";
            svg::line(out, cx - k, cy - k, cx + k, cy + k, REMOVED, style);
            svg::line(out, cx - k, cy + k, cx + k, cy - k, REMOVED, style);
        }
        _ => {}
    }
}

fn inset(b: &PixelBox, by: i32) -> PixelBox {
    PixelBox {
        x1: b.x1 + by,
        y1: b.y1,
        x2: b.x2 - by,
        y2: b.y2,
    }
}

/// A color attribute, with `none`/`transparent` meaning no paint.
fn paint(value: Option<&str>, default: &str) -> String {
    match value {
        Some(v) if v.eq_ignore_ascii_case("none") || v.eq_ignore_ascii_case("transparent") => {
            "none".to_string()
        }
        Some(v) if !v.is_empty() => v.to_string(),
        _ => default.to_string(),
    }
}

/// Truncate `value` with three dots so it fits `max_width`.
fn fit(text: &mut TextMeasurer, value: &str, max_width: f32) -> Option<String> {
    if max_width < 1.0 || value.is_empty() {
        return None;
    }
    if text.measure(value, None).width <= max_width {
        return Some(value.to_string());
    }
    let chars: Vec<char> = value.chars().collect();
    (0..chars.len()).rev().find_map(|n| {
        let candidate: String =
            chars[..n].iter().collect::<String>().trim_end().to_string() + "...";
        (text.measure(&candidate, None).width <= max_width).then_some(candidate)
    })
}

#[cfg(test)]
mod tests {
    use super::super::model;
    use super::super::parser::parse;
    use super::*;

    fn svg_of(source: &str) -> String {
        let document = model::build(&parse(source).unwrap()).unwrap();
        render(&document.diagrams[0], &RenderOptions::default(), false, &[])
    }

    #[test]
    fn every_mark_has_its_own_drawing_and_tooltip() {
        let svg = svg_of(
            "manifest { column a { x; y; z; } column b [extends = a] { remove y; override z [note = n]; w; } }",
        );
        for mark in ["direct", "inherited", "override", "removed", "none"] {
            assert!(
                svg.contains(&format!("data-mark=\"{mark}\"")),
                "{mark}: {svg}"
            );
        }
        assert!(
            svg.contains("<title>x in b (inherited from a)</title>"),
            "{svg}"
        );
        assert!(
            svg.contains("<title>z in b (overrides a): n</title>"),
            "{svg}"
        );
        assert!(svg.contains("class=\"manifest-link\""), "{svg}");
        assert!(svg.contains("class=\"manifest-legend\""), "{svg}");
        usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("valid SVG");
    }

    #[test]
    fn rows_carry_their_member_columns() {
        let svg = svg_of("manifest { column a { x; } column b { x; y; } }");
        assert!(
            svg.contains("data-row=\"r0\" data-cols=\"c0 c1\" data-count=\"2\""),
            "{svg}"
        );
        assert!(
            svg.contains("data-row=\"r1\" data-cols=\"c1\" data-count=\"1\""),
            "{svg}"
        );
    }

    #[test]
    fn groups_titles_and_links() {
        let svg = svg_of(
            "manifest { title = \"Uses\"; group g [color = \"#cfe8ff\"] { column a [href = \"https://e.x\"]; } row x [href = y]: a; }",
        );
        assert!(svg.contains("class=\"manifest-title\""), "{svg}");
        assert!(svg.contains("#cfe8ff"), "{svg}");
        assert!(svg.contains("<a href=\"https://e.x\">"), "{svg}");
        assert!(svg.contains("<a href=\"y\">"), "{svg}");
        assert!(svg.contains("rotate(-90)"), "{svg}");
    }

    #[test]
    fn a_group_can_have_rounded_corners() {
        let plain = svg_of("manifest { group g { column a; } row x: a; }");
        let rounded = svg_of("manifest { group g [shape = roundedbox] { column a; } row x: a; }");
        let corners = |svg: &str| svg.matches(" rx=").count();
        assert_eq!(corners(&rounded), corners(&plain) + 1, "{rounded}");
    }

    #[test]
    fn interactive_output_has_toggles_and_clickable_legend() {
        let document = model::build(
            &parse("manifest { column a { x; } column b [extends = a] { remove x; y; } }").unwrap(),
        )
        .unwrap();
        let svg = render(&document.diagrams[0], &RenderOptions::default(), true, &[]);
        assert!(svg.contains("data-toggle=\"r0\""), "{svg}");
        assert!(svg.contains("data-toggle=\"c1\""), "{svg}");
        assert!(
            svg.contains("class=\"manifest-legend-entry\" data-toggle=\"m:removed\""),
            "{svg}"
        );
        usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("valid SVG");
        let plain = render(&document.diagrams[0], &RenderOptions::default(), false, &[]);
        assert!(!plain.contains("manifest-toggle"), "{plain}");
    }

    #[test]
    fn a_view_leaves_out_hidden_rows_columns_and_marks() {
        let mut document =
            model::build(&parse("manifest { column a { x; y; } column b { x; } }").unwrap())
                .unwrap();
        let d = &mut document.diagrams[0];
        d.apply_view(["c0", "m:direct"]);
        let svg = render(d, &RenderOptions::default(), false, &[]);
        assert!(!svg.contains("data-col=\"c0\""), "{svg}");
        assert!(svg.contains("data-col=\"c1\""), "{svg}");
        assert!(!svg.contains("<ellipse"), "{svg}");
    }

    #[test]
    fn a_simple_manifest_needs_no_legend() {
        let svg = svg_of("manifest { column a { x; } }");
        assert!(!svg.contains("manifest-legend"), "{svg}");
    }
}
