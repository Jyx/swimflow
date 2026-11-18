//! Rendering: the sequence layout -> SVG.
//!
//! The drawing uses the shared core primitives (`crate::core::svg`), so the
//! sequence diagram gets the same font handling, the same number formatting
//! and the same `--embed-font` / `--text-as-paths` support as the activity
//! diagram.

use crate::core::geom::PixelBox;
use crate::core::svg::{self, TextAlign, TextStyle};
use crate::render::RenderOptions;
use crate::text::TextMeasurer;

use super::layout::{self, Layout, MessageLayout};
use super::model::Diagram;

/// Fill of a participant header.
const PARTICIPANT_FILL: &str = "#eef2f7";
/// Outline of a participant header.
const PARTICIPANT_STROKE: &str = "#33415c";
/// Color of a lifeline.
const LIFELINE_COLOR: &str = "#8a8a8a";
/// Color of a message.
const MESSAGE_COLOR: &str = "#1f2933";
/// Fill of a note.
const NOTE_FILL: &str = "#fff8c5";
/// Outline of a note.
const NOTE_STROKE: &str = "#c9b037";
/// Color of a frame.
const BLOCK_STROKE: &str = "#6b7280";
/// Fill of the label tab of a frame.
const BLOCK_TAB_FILL: &str = "#e5e7eb";
/// The default text color.
const TEXT_COLOR: &str = "#111827";
/// The size of the folded corner of a note.
const NOTE_FOLD: i32 = 8;

/// Render a sequence diagram as an SVG document.
pub fn render(diagram: &Diagram, options: &RenderOptions) -> String {
    let mut text = TextMeasurer::from_spec(&options.font);
    let layout = layout::layout(diagram, &mut text);

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

    draw_participants(&mut out, &mut text, &layout, options);
    draw_lifelines(&mut out, &layout);
    draw_blocks(&mut out, &mut text, &layout, options);
    draw_messages(&mut out, &mut text, &layout, options);
    draw_notes(&mut out, &mut text, &layout, options);

    svg::close_document(&mut out);
    out
}

/// The participant headers.
fn draw_participants(
    out: &mut String,
    text: &mut TextMeasurer,
    layout: &Layout,
    options: &RenderOptions,
) {
    for participant in &layout.participants {
        svg::rect(
            out,
            &participant.box_,
            PARTICIPANT_FILL,
            PARTICIPANT_STROKE,
            " stroke-width=\"1\"",
        );
        draw_text(
            out,
            text,
            &participant.box_,
            &participant.label,
            TEXT_COLOR,
            options,
            TextAlign::Center,
        );
    }
}

/// The dashed lifelines below the participant headers.
fn draw_lifelines(out: &mut String, layout: &Layout) {
    for lifeline in &layout.lifelines {
        svg::line(
            out,
            lifeline.x,
            lifeline.y1,
            lifeline.x,
            lifeline.y2,
            LIFELINE_COLOR,
            " stroke-dasharray=\"4\"",
        );
    }
}

/// The frames of the blocks.
fn draw_blocks(
    out: &mut String,
    text: &mut TextMeasurer,
    layout: &Layout,
    options: &RenderOptions,
) {
    for block in &layout.blocks {
        svg::rect(
            out,
            &block.box_,
            "none",
            BLOCK_STROKE,
            " stroke-width=\"1\"",
        );
        svg::rect(
            out,
            &block.tab,
            BLOCK_TAB_FILL,
            BLOCK_STROKE,
            " stroke-width=\"1\"",
        );

        let label = match &block.label {
            Some(label) if !label.is_empty() => {
                format!("{} [{}]", block.kind.tab_label(), label)
            }
            _ => block.kind.tab_label().to_string(),
        };
        let box_ = PixelBox {
            x1: block.tab.x1 + 4,
            y1: block.tab.y1,
            x2: block.tab.x2 - 4,
            y2: block.tab.y2,
        };
        draw_text(
            out,
            text,
            &box_,
            &label,
            TEXT_COLOR,
            options,
            TextAlign::Left,
        );
    }
}

/// The messages: the arrow, its head and the label.
fn draw_messages(
    out: &mut String,
    text: &mut TextMeasurer,
    layout: &Layout,
    options: &RenderOptions,
) {
    for message in &layout.messages {
        let style = if message.kind.is_dashed() {
            " stroke-dasharray=\"4\""
        } else {
            ""
        };

        if message.is_self {
            draw_self_message(out, message, style);
        } else {
            svg::line(
                out,
                message.x1,
                message.y,
                message.x2,
                message.y,
                MESSAGE_COLOR,
                style,
            );
            draw_head(out, message);
        }

        if !message.text.is_empty() {
            let box_ = PixelBox {
                x1: message.text_box.x1,
                y1: message.text_box.y1,
                x2: message.text_box.x1 + message.wrap_width,
                y2: message.text_box.y2,
            };
            draw_text(
                out,
                text,
                &box_,
                &message.text,
                TEXT_COLOR,
                options,
                TextAlign::Left,
            );
        }
    }
}

/// The arrow head of a message.
///
/// The head is always drawn solid: only the message line carries the dash
/// pattern, so a reply (`-->`) has a dotted line but a clean open head.
fn draw_head(out: &mut String, message: &MessageLayout) {
    let direction = if message.x2 >= message.x1 { 1 } else { -1 };
    let tip = message.x2;
    let base = message.x2 - direction * layout::ARROW_LENGTH;

    if message.kind.has_filled_head() {
        svg::polygon(
            out,
            &[
                (tip, message.y),
                (base, message.y - 4),
                (base, message.y + 4),
            ],
            MESSAGE_COLOR,
            MESSAGE_COLOR,
        );
    } else if message.kind.has_head() {
        svg::line(out, tip, message.y, base, message.y - 4, MESSAGE_COLOR, "");
        svg::line(out, tip, message.y, base, message.y + 4, MESSAGE_COLOR, "");
    }
}

/// A message to oneself: a small loop on the lifeline.
fn draw_self_message(out: &mut String, message: &MessageLayout, style: &str) {
    let x = message.x1;
    let y = message.y;
    let right = x + layout::SELF_WIDTH;
    let bottom = y + layout::SELF_HEIGHT;

    svg::path(
        out,
        &[(x, y), (right, y), (right, bottom), (x, bottom)],
        MESSAGE_COLOR,
        style,
    );

    if message.kind.has_filled_head() {
        svg::polygon(
            out,
            &[
                (x, bottom),
                (x + layout::ARROW_LENGTH, bottom - 4),
                (x + layout::ARROW_LENGTH, bottom + 4),
            ],
            MESSAGE_COLOR,
            MESSAGE_COLOR,
        );
    } else if message.kind.has_head() {
        svg::line(
            out,
            x,
            bottom,
            x + layout::ARROW_LENGTH,
            bottom - 4,
            MESSAGE_COLOR,
            "",
        );
        svg::line(
            out,
            x,
            bottom,
            x + layout::ARROW_LENGTH,
            bottom + 4,
            MESSAGE_COLOR,
            "",
        );
    }
}

/// The notes.
fn draw_notes(out: &mut String, text: &mut TextMeasurer, layout: &Layout, options: &RenderOptions) {
    for note in &layout.notes {
        let box_ = note.box_;
        let fold = NOTE_FOLD.min(box_.width() / 2).max(1);
        let path = format!(
            "M{} {} H{} L{} {} V{} H{} Z",
            box_.x1,
            box_.y1,
            box_.x2 - fold,
            box_.x2,
            box_.y1 + fold,
            box_.y2,
            box_.x1
        );
        out.push_str(&format!(
            "<path d=\"{path}\" fill=\"{NOTE_FILL}\" stroke=\"{NOTE_STROKE}\" \
             stroke-width=\"1\"/>\n"
        ));
        out.push_str(&format!(
            "<path d=\"M{} {} V{} H{}\" fill=\"none\" stroke=\"{NOTE_STROKE}\" \
             stroke-width=\"1\"/>\n",
            box_.x2 - fold,
            box_.y1,
            box_.y1 + fold,
            box_.x2
        ));

        if !note.text.is_empty() {
            let inner = PixelBox {
                x1: box_.x1 + layout::NOTE_PADDING,
                y1: box_.y1 + layout::NOTE_PADDING,
                x2: box_.x1 + layout::NOTE_PADDING + note.wrap_width,
                y2: box_.y2 - layout::NOTE_PADDING,
            };
            draw_text(
                out,
                text,
                &inner,
                &note.text,
                TEXT_COLOR,
                options,
                TextAlign::Left,
            );
        }
    }
}

/// Draw a text, honouring the shared text options.
fn draw_text(
    out: &mut String,
    text: &mut TextMeasurer,
    box_: &PixelBox,
    value: &str,
    color: &str,
    options: &RenderOptions,
    align: TextAlign,
) {
    let style = TextStyle {
        color,
        text_as_paths: options.text_as_paths,
        align,
        underline: false,
    };
    svg::draw_text(out, text, box_, value, &style);
}

#[cfg(test)]
mod tests {
    use super::super::ast::MessageKind;
    use super::*;

    fn message(kind: MessageKind) -> MessageLayout {
        MessageLayout {
            from: 0,
            to: 1,
            kind,
            text: String::new(),
            text_box: PixelBox {
                x1: 0,
                y1: 0,
                x2: 0,
                y2: 0,
            },
            wrap_width: 0,
            x1: 200,
            x2: 100,
            y: 100,
            is_self: false,
        }
    }

    #[test]
    fn a_reply_head_is_solid_even_though_the_shaft_is_dashed() {
        let mut out = String::new();
        draw_head(&mut out, &message(MessageKind::Reply));
        assert!(out.contains("<line"), "the head is drawn: {out}");
        assert!(
            !out.contains("stroke-dasharray"),
            "the head must not inherit the shaft dash pattern: {out}"
        );
    }

    #[test]
    fn a_self_reply_head_is_solid() {
        let mut message = message(MessageKind::Reply);
        message.is_self = true;
        let mut out = String::new();
        draw_self_message(&mut out, &message, " stroke-dasharray=\"4\"");
        // The loop is dashed, but neither head stroke may be.
        assert!(out.contains("<path"), "the loop is drawn: {out}");
        for line in out.lines().filter(|line| line.contains("<line")) {
            assert!(
                !line.contains("stroke-dasharray"),
                "the self head must be solid: {line}"
            );
        }
    }
}
