use crate::rect::Rect;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Lighten (add the same delta to RGB channels, saturating).
    #[must_use]
    pub fn lighten(self, delta: u8) -> Self {
        Self {
            r: self.r.saturating_add(delta),
            g: self.g.saturating_add(delta),
            b: self.b.saturating_add(delta),
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::rgb(200, 200, 200)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Style {
    pub bold: bool,
    pub dim: bool,
    pub underline: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Single display character (BMP-friendly; extend later if needed).
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub style: Style,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::rgb(200, 200, 200),
            bg: Color::rgb(0, 0, 0),
            style: Style::default(),
        }
    }
}

pub struct FrameBuffer {
    pub width: u16,
    pub height: u16,
    cells: Vec<Cell>,
    prev: Vec<Cell>,
    /// Monotonically increasing per-cell generation for optional fast paths.
    gen: u32,
    cell_gen: Vec<u32>,
}

impl FrameBuffer {
    pub fn new(width: u16, height: u16) -> Self {
        let n = (width as usize) * (height as usize);
        let cell = Cell::default();
        Self {
            width,
            height,
            cells: vec![cell.clone(); n],
            prev: vec![cell; n],
            gen: 1,
            cell_gen: vec![0; n],
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        let n = (width as usize) * (height as usize);
        let cell = Cell::default();
        self.width = width;
        self.height = height;
        self.cells.resize(n, cell.clone());
        self.prev.resize(n, cell);
        self.cell_gen.resize(n, 0);
        self.gen = self.gen.wrapping_add(1);
    }

    #[inline]
    pub fn idx(&self, x: u16, y: u16) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(y as usize * self.width as usize + x as usize)
    }

    pub fn get(&self, x: u16, y: u16) -> Option<&Cell> {
        self.idx(x, y).map(|i| &self.cells[i])
    }

    pub fn set(&mut self, x: u16, y: u16, c: Cell) {
        let Some(i) = self.idx(x, y) else {
            return;
        };
        self.cells[i] = c;
        self.gen = self.gen.wrapping_add(1);
        self.cell_gen[i] = self.gen;
    }

    /// Fill a rectangle with `cell` (clipped).
    pub fn fill_rect(&mut self, r: Rect, cell: Cell) {
        let x1 = r.x.min(self.width);
        let y1 = r.y.min(self.height);
        let x2 = r.right().min(self.width);
        let y2 = r.bottom().min(self.height);
        for y in y1..y2 {
            for x in x1..x2 {
                self.set(x, y, cell.clone());
            }
        }
    }

    /// Copy current frame to `prev` for next-frame delta encoding.
    pub fn commit_frame(&mut self) {
        self.prev.clone_from(&self.cells);
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn prev_cells(&self) -> &[Cell] {
        &self.prev
    }

    pub fn dirty_count(&self) -> usize {
        self.cells
            .iter()
            .zip(self.prev.iter())
            .filter(|(a, b)| a != b)
            .count()
    }
}
