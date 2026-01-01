/// One frame worth of lightweight profiling data (cheap to record).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameSample {
    /// Wall time to build + encode last frame (nanoseconds), if measured externally.
    pub encode_nanos: u64,
    pub cells_dirty: u32,
    pub bytes_written: u32,
    pub terminal_w: u16,
    pub terminal_h: u16,
}

pub struct FrameStatsRing {
    buf: Vec<FrameSample>,
    head: usize,
    len: usize,
}

impl FrameStatsRing {
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self {
            buf: vec![FrameSample::default(); cap],
            head: 0,
            len: 0,
        }
    }

    pub fn push(&mut self, s: FrameSample) {
        let cap = self.buf.len();
        self.buf[self.head] = s;
        self.head = (self.head + 1) % cap;
        self.len = (self.len + 1).min(cap);
    }

    /// Newest-first, up to `max` samples.
    pub fn recent(&self, max: usize) -> Vec<FrameSample> {
        let n = self.len.min(max);
        let cap = self.buf.len();
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let idx = (self.head + cap - 1 - i) % cap;
            out.push(self.buf[idx]);
        }
        out
    }

    pub fn last(&self) -> Option<FrameSample> {
        if self.len == 0 {
            return None;
        }
        let cap = self.buf.len();
        let idx = (self.head + cap - 1) % cap;
        Some(self.buf[idx])
    }
}
