//! Layout: the sequence model -> pixel geometry.
//!
//! The layout is a straightforward top-down pass: participants are placed in
//! a row, every statement gets a horizontal band, and the frames of the
//! blocks are drawn around the bands they contain. Text is measured with the
//! shared text measurer, so the sequence diagram uses exactly the same font
//! handling as the activity diagram.

use crate::core::geom::PixelBox;
use crate::text::TextMeasurer;

use super::ast::{BlockKind, MessageKind};
use super::model::{Diagram, Item};

/// Space around the drawing.
pub const MARGIN: i32 = 24;
/// Horizontal padding inside a participant box.
pub const PARTICIPANT_PADDING_X: i32 = 14;
/// Height of a participant box.
pub const PARTICIPANT_HEIGHT: i32 = 34;
/// Minimum width of a participant box.
pub const PARTICIPANT_MIN_WIDTH: i32 = 100;
/// Space between two participant boxes.
pub const PARTICIPANT_GAP: i32 = 44;
/// Space between the participant header and the first statement.
pub const HEADER_GAP: i32 = 26;
/// Space below a statement.
pub const ROW_GAP: i32 = 16;
/// Space between a message label and its arrow.
pub const MESSAGE_LABEL_GAP: i32 = 8;
/// Padding inside a note.
pub const NOTE_PADDING: i32 = 8;
/// Padding around a frame.
pub const BLOCK_PADDING: i32 = 12;
/// Height of the label tab of a frame.
pub const BLOCK_TAB_HEIGHT: i32 = 18;
/// Width of the loop of a self message.
pub const SELF_WIDTH: i32 = 40;
/// Height of the loop of a self message.
pub const SELF_HEIGHT: i32 = 26;
/// Length of an arrow head.
pub const ARROW_LENGTH: i32 = 10;
/// The width a message label may use at most.
pub const MAX_LABEL_WIDTH: i32 = 260;

/// The complete geometry of a sequence diagram.
#[derive(Debug, Clone)]
pub struct Layout {
    pub width: i32,
    pub height: i32,
    pub participants: Vec<ParticipantLayout>,
    pub lifelines: Vec<Lifeline>,
    pub messages: Vec<MessageLayout>,
    pub notes: Vec<NoteLayout>,
    pub blocks: Vec<BlockLayout>,
}

/// A participant header.
#[derive(Debug, Clone)]
pub struct ParticipantLayout {
    pub label: String,
    pub box_: PixelBox,
}

/// The vertical dashed line below a participant.
#[derive(Debug, Clone)]
pub struct Lifeline {
    pub x: i32,
    pub y1: i32,
    pub y2: i32,
}

/// A message and its label.
#[derive(Debug, Clone)]
pub struct MessageLayout {
    pub from: usize,
    pub to: usize,
    pub kind: MessageKind,
    pub text: String,
    /// The box the label is drawn in.
    pub text_box: PixelBox,
    /// The width the label text is wrapped to.
    pub wrap_width: i32,
    /// The horizontal extent of the arrow.
    pub x1: i32,
    pub x2: i32,
    /// The vertical position of the arrow.
    pub y: i32,
    /// Whether the message is a message to oneself.
    pub is_self: bool,
}

/// A note.
#[derive(Debug, Clone)]
pub struct NoteLayout {
    pub text: String,
    pub box_: PixelBox,
    /// The width the note text is wrapped to.
    pub wrap_width: i32,
}

/// A frame around a group of statements.
#[derive(Debug, Clone)]
pub struct BlockLayout {
    pub kind: BlockKind,
    pub label: Option<String>,
    /// The frame itself.
    pub box_: PixelBox,
    /// The tab that carries the frame's label.
    pub tab: PixelBox,
}

/// Compute the geometry of a diagram.
pub fn layout(diagram: &Diagram, text: &mut TextMeasurer) -> Layout {
    // 1. The participant headers.
    let mut widths = Vec::with_capacity(diagram.participants.len());
    for participant in &diagram.participants {
        let size = text.measure(&participant.name, None);
        let width = (size.width.ceil() as i32) + 2 * PARTICIPANT_PADDING_X;
        widths.push(width.max(PARTICIPANT_MIN_WIDTH));
    }

    let mut cursor = MARGIN;
    let mut boxes = Vec::with_capacity(widths.len());
    for width in &widths {
        boxes.push(PixelBox {
            x1: cursor,
            y1: MARGIN,
            x2: cursor + width,
            y2: MARGIN + PARTICIPANT_HEIGHT,
        });
        cursor += width + PARTICIPANT_GAP;
    }
    let content_width = if widths.is_empty() {
        0
    } else {
        cursor - PARTICIPANT_GAP - MARGIN
    };

    let mut builder = Builder {
        text,
        lifeline_x: boxes
            .iter()
            .map(|box_| box_.x1 + box_.width() / 2)
            .collect(),
        y: MARGIN + PARTICIPANT_HEIGHT + HEADER_GAP,
        messages: Vec::new(),
        notes: Vec::new(),
        blocks: Vec::new(),
    };

    builder.layout_items(&diagram.items, 0);

    let bottom = builder.y;
    let header_bottom = MARGIN + PARTICIPANT_HEIGHT;

    let participants = diagram
        .participants
        .iter()
        .zip(boxes.iter())
        .map(|(participant, box_)| ParticipantLayout {
            label: participant.name.clone(),
            box_: *box_,
        })
        .collect();

    let lifelines = builder
        .lifeline_x
        .iter()
        .map(|x| Lifeline {
            x: *x,
            y1: header_bottom,
            y2: bottom,
        })
        .collect();

    let width = (content_width + 2 * MARGIN)
        .max(boxes.last().map(|box_| box_.x2).unwrap_or(0) + MARGIN)
        .max(
            builder
                .blocks
                .iter()
                .map(|block| block.box_.x2)
                .max()
                .unwrap_or(0)
                + MARGIN,
        )
        .max(
            builder
                .notes
                .iter()
                .map(|note| note.box_.x2)
                .max()
                .unwrap_or(0)
                + MARGIN,
        )
        .max(
            builder
                .messages
                .iter()
                .map(|message| {
                    if message.is_self {
                        message.text_box.x2.max(message.x1 + SELF_WIDTH)
                    } else {
                        message.text_box.x2
                    }
                })
                .max()
                .unwrap_or(0)
                + MARGIN,
        );

    Layout {
        width,
        height: bottom + MARGIN,
        participants,
        lifelines,
        messages: builder.messages,
        notes: builder.notes,
        blocks: builder.blocks,
    }
}

/// The layout pass.
struct Builder<'a> {
    text: &'a mut TextMeasurer,
    /// The horizontal position of every lifeline.
    lifeline_x: Vec<i32>,
    /// The current vertical position.
    y: i32,
    messages: Vec<MessageLayout>,
    notes: Vec<NoteLayout>,
    blocks: Vec<BlockLayout>,
}

impl<'a> Builder<'a> {
    /// Lay out a list of statements; returns the vertical extent.
    fn layout_items(&mut self, items: &[Item], depth: usize) -> (i32, i32) {
        let start = self.y;
        for item in items {
            match item {
                Item::Message(message) => self.layout_message(message),
                Item::Note(note) => self.layout_note(note),
                Item::Block(block) => {
                    self.layout_block(block, depth);
                }
            }
        }
        (start, self.y)
    }

    fn layout_message(&mut self, message: &super::model::Message) {
        let x1 = self.lifeline_x[message.from];
        let x2 = self.lifeline_x[message.to];
        let is_self = message.from == message.to;

        if is_self {
            let label_x = x1 + 14;
            let (text_width, text_height) = self.measure(&message.text, MAX_LABEL_WIDTH);
            let text_box = PixelBox {
                x1: label_x,
                y1: self.y,
                x2: label_x + text_width.max(1),
                y2: self.y + text_height,
            };
            let line_y = self.y + text_height + MESSAGE_LABEL_GAP;
            self.messages.push(MessageLayout {
                from: message.from,
                to: message.to,
                kind: message.kind,
                text: message.text.clone(),
                text_box,
                wrap_width: MAX_LABEL_WIDTH,
                x1,
                x2,
                y: line_y,
                is_self: true,
            });
            self.y = line_y + SELF_HEIGHT + ROW_GAP;
            return;
        }

        let span = (x2 - x1).abs();
        let max_width = (span - 16).clamp(80, MAX_LABEL_WIDTH);
        let (text_width, text_height) = self.measure(&message.text, max_width);

        // The label sits above the arrow, on the side of the sender.
        let text_x = if x2 >= x1 {
            x1 + 8
        } else {
            (x1 - text_width - 8).max(MARGIN)
        };
        let text_box = PixelBox {
            x1: text_x,
            y1: self.y,
            x2: text_x + text_width.max(1),
            y2: self.y + text_height,
        };
        let line_y = self.y + text_height + MESSAGE_LABEL_GAP;
        self.messages.push(MessageLayout {
            from: message.from,
            to: message.to,
            kind: message.kind,
            text: message.text.clone(),
            text_box,
            wrap_width: max_width,
            x1,
            x2,
            y: line_y,
            is_self: false,
        });
        self.y = line_y + ROW_GAP;
    }

    fn layout_note(&mut self, note: &super::model::Note) {
        if note.over.is_empty() {
            return;
        }
        let first = self.lifeline_x[note.over[0]];
        let last = self.lifeline_x[*note.over.last().unwrap()];

        let (text_width, text_height) = self.measure(&note.text, MAX_LABEL_WIDTH);
        let needed = text_width + 2 * NOTE_PADDING;

        let (x1, x2) = if note.over.len() > 1 {
            let left = first - 12;
            let right = last + 12;
            if right - left >= needed {
                (left, right)
            } else {
                (left, left + needed)
            }
        } else {
            let left = first - needed / 2;
            (left, left + needed)
        };

        let box_ = PixelBox {
            x1: x1.max(MARGIN / 2),
            y1: self.y,
            x2,
            y2: self.y + text_height + 2 * NOTE_PADDING,
        };
        self.notes.push(NoteLayout {
            text: note.text.clone(),
            box_,
            wrap_width: MAX_LABEL_WIDTH,
        });
        self.y = box_.y2 + ROW_GAP;
    }

    fn layout_block(&mut self, block: &super::model::Block, depth: usize) -> PixelBox {
        // The frame starts right below the previous content; its label tab
        // lives inside the frame, above the first statement.
        let frame_top = self.y;
        self.y += BLOCK_TAB_HEIGHT + 4;

        let (_top, mut bottom) = self.layout_items(&block.items, depth + 1);

        // The `else`/`and` branches are frames of their own, stacked inside
        // the frame they belong to.
        for branch in &block.branches {
            let branch_frame = self.layout_block(branch, depth + 1);
            bottom = bottom.max(branch_frame.y2);
        }

        let involved = involved_participants(&block.items);
        let mut involved = involved;
        for branch in &block.branches {
            involved.extend(involved_participants(&branch.items));
        }
        involved.sort_unstable();
        involved.dedup();

        let (min_x, max_x) = if involved.is_empty() {
            (
                self.lifeline_x.first().copied().unwrap_or(MARGIN),
                self.lifeline_x.last().copied().unwrap_or(MARGIN),
            )
        } else {
            let first = self.lifeline_x[*involved.iter().min().unwrap()];
            let last = self.lifeline_x[*involved.iter().max().unwrap()];
            (first, last)
        };

        let inset = (depth * 4) as i32;
        let frame = PixelBox {
            x1: min_x - BLOCK_PADDING + inset,
            y1: frame_top + inset,
            x2: max_x + BLOCK_PADDING - inset,
            y2: bottom + BLOCK_PADDING - inset,
        };

        // The tab is as wide as its label.
        let label = match &block.label {
            Some(label) if !label.is_empty() => {
                format!("{} [{}]", block.kind.tab_label(), label)
            }
            _ => block.kind.tab_label().to_string(),
        };
        let (label_width, _) = self.measure(&label, MAX_LABEL_WIDTH);
        let tab = PixelBox {
            x1: frame.x1,
            y1: frame.y1,
            x2: frame.x1 + label_width + 8,
            y2: frame.y1 + BLOCK_TAB_HEIGHT,
        };
        self.blocks.push(BlockLayout {
            kind: block.kind,
            label: block.label.clone(),
            box_: frame,
            tab,
        });
        // What follows the frame starts below its bottom border.
        self.y = frame.y2 + 4;
        frame
    }

    /// Measure a text, returning its (width, height) when wrapped to
    /// `max_width`.
    fn measure(&mut self, text: &str, max_width: i32) -> (i32, i32) {
        if text.is_empty() {
            return (0, 0);
        }
        let lines = self.text.layout(text, Some(max_width.max(1) as f32));
        if lines.is_empty() {
            return (0, 0);
        }
        let width = lines
            .iter()
            .map(|line| line.width)
            .fold(0.0f32, f32::max)
            .ceil() as i32;
        let height = (lines.len() as f32 * self.text.line_height()).ceil() as i32;
        (width, height)
    }
}

/// The participants a group of statements mentions.
fn involved_participants(items: &[Item]) -> Vec<usize> {
    let mut out = Vec::new();
    for item in items {
        match item {
            Item::Message(message) => {
                out.push(message.from);
                out.push(message.to);
            }
            Item::Note(note) => out.extend(note.over.iter().copied()),
            Item::Block(block) => {
                out.extend(involved_participants(&block.items));
                for branch in &block.branches {
                    out.extend(involved_participants(&branch.items));
                }
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::sequence::{model, parser};

    fn layout_of(source: &str) -> Layout {
        let ast = parser::parse(source).expect("parse");
        let document = model::build(&ast);
        let mut text = TextMeasurer::default_size();
        layout(&document.diagrams[0], &mut text)
    }

    #[test]
    fn participants_are_laid_out_in_a_row() {
        let layout = layout_of("sequence { participant A; participant B; A -> B: hi; }");
        assert_eq!(layout.participants.len(), 2);
        assert!(layout.participants[0].box_.x1 < layout.participants[1].box_.x1);
        assert_eq!(layout.lifelines.len(), 2);
        assert_eq!(layout.messages.len(), 1);
        assert!(layout.height > 0 && layout.width > 0);
    }

    #[test]
    fn messages_stack_vertically() {
        let layout =
            layout_of("sequence { participant A; participant B; A -> B: one; B -> A: two; }");
        assert_eq!(layout.messages.len(), 2);
        assert!(layout.messages[1].y > layout.messages[0].y);
    }

    #[test]
    fn a_self_message_keeps_its_own_column() {
        let layout = layout_of("sequence { participant A; A -> A: think; }");
        assert!(layout.messages[0].is_self);
        assert_eq!(layout.messages[0].x1, layout.messages[0].x2);
    }

    #[test]
    fn blocks_frame_their_content() {
        let layout = layout_of("sequence { participant A; participant B; loop { A -> B: x; } }");
        assert_eq!(layout.blocks.len(), 1);
        let block = &layout.blocks[0];
        assert!(block.box_.x1 < layout.lifelines[0].x);
        assert!(block.box_.x2 > layout.lifelines[1].x);
        assert!(block.box_.y1 < layout.messages[0].y);
    }
}
