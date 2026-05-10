/// A byte index into the source string.
///
/// This is the canonical internal representation; all other location types
/// ultimately convert through a byte index.
///
/// # Why a newtype?
/// Wrapping `usize` prevents accidental mixing of byte indices, character
/// counts, and line numbers (all `usize` in Rust).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteIndex(pub usize);

impl ByteIndex {
    pub fn new(idx: usize) -> Self {
        Self(idx)
    }

    pub fn get(self) -> usize {
        self.0
    }
}

impl From<usize> for ByteIndex {
    fn from(n: usize) -> Self {
        Self(n)
    }
}

impl From<ByteIndex> for usize {
    fn from(b: ByteIndex) -> Self {
        b.0
    }
}

impl std::fmt::Display for ByteIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ─── ByteRange ────────────────────────────────────────────────────────────────

/// A byte range `[start, end)` within the source string.
///
/// Both endpoints are `ByteIndex` newtypes to prevent accidental mixing with
/// character counts or line numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteRange {
    pub start: ByteIndex,
    /// Exclusive upper bound.
    pub end: ByteIndex,
}

impl ByteRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start: ByteIndex(start),
            end: ByteIndex(end),
        }
    }

    /// Number of bytes in this range.
    pub fn len(self) -> usize {
        self.end.0.saturating_sub(self.start.0)
    }

    pub fn is_empty(self) -> bool {
        self.end.0 <= self.start.0
    }

    /// Slice the source to this range.
    pub fn slice<'a>(self, src: &'a str) -> &'a str {
        &src[self.start.0..self.end.0]
    }
}

impl std::fmt::Display for ByteRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

impl From<std::ops::Range<usize>> for ByteRange {
    fn from(r: std::ops::Range<usize>) -> Self {
        Self::new(r.start, r.end)
    }
}

impl From<ByteRange> for std::ops::Range<usize> {
    fn from(b: ByteRange) -> Self {
        b.start.0..b.end.0
    }
}
