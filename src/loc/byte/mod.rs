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
