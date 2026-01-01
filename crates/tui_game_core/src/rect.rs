//! Axis-aligned rectangles in terminal cell space.

/// Top-left origin: `x` grows right, `y` grows down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    pub const fn new(x: u16, y: u16, w: u16, h: u16) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(self, cx: u16, cy: u16) -> bool {
        cx >= self.x
            && cy >= self.y
            && cx < self.x.saturating_add(self.w)
            && cy < self.y.saturating_add(self.h)
    }

    pub fn right(self) -> u16 {
        self.x.saturating_add(self.w)
    }

    pub fn bottom(self) -> u16 {
        self.y.saturating_add(self.h)
    }
}
