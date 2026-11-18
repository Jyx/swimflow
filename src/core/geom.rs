//! Diagram-agnostic geometry primitives.
//!
//! These types used to live in `crate::metrics`, which is the *activity*
//! diagram's metric port. They are plain pixel rectangles and belong to the
//! core, because every diagram type (activity, sequence, …) needs them to
//! describe where things are drawn.

/// An axis-aligned pixel rectangle, matching blockdiag's `Box` `(x1, y1, x2, y2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelBox {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

impl PixelBox {
    pub fn width(&self) -> i32 {
        self.x2 - self.x1
    }

    pub fn height(&self) -> i32 {
        self.y2 - self.y1
    }

    /// `Box.top`, i.e. the midpoint of the top edge.
    pub fn top(&self) -> (i32, i32) {
        (self.x1 + self.width() / 2, self.y1)
    }

    /// `Box.bottom`.
    pub fn bottom(&self) -> (i32, i32) {
        (self.x1 + self.width() / 2, self.y2)
    }

    /// `Box.left`, i.e. the midpoint of the left edge.
    pub fn left(&self) -> (i32, i32) {
        (self.x1, self.y1 + self.height() / 2)
    }

    /// `Box.right`.
    pub fn right(&self) -> (i32, i32) {
        (self.x2, self.y1 + self.height() / 2)
    }

    pub fn topleft(&self) -> (i32, i32) {
        (self.x1, self.y1)
    }

    pub fn topright(&self) -> (i32, i32) {
        (self.x2, self.y1)
    }

    pub fn bottomleft(&self) -> (i32, i32) {
        (self.x1, self.y2)
    }

    pub fn bottomright(&self) -> (i32, i32) {
        (self.x2, self.y2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_and_midpoints() {
        let box_ = PixelBox {
            x1: 10,
            y1: 20,
            x2: 30,
            y2: 60,
        };
        assert_eq!(box_.width(), 20);
        assert_eq!(box_.height(), 40);
        assert_eq!(box_.top(), (20, 20));
        assert_eq!(box_.bottom(), (20, 60));
        assert_eq!(box_.left(), (10, 40));
        assert_eq!(box_.right(), (30, 40));
        assert_eq!(box_.topleft(), (10, 20));
        assert_eq!(box_.topright(), (30, 20));
        assert_eq!(box_.bottomleft(), (10, 60));
        assert_eq!(box_.bottomright(), (30, 60));
    }
}
