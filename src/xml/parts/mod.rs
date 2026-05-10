/*!
Lazy-parse strategy: the first parse pass only records byte-range spans (cheap).
Expensive data (tag names, attributes map) is pulled out of the source slice on
demand and cached behind a `OnceLock` / `OnceCell`.

Coordinate systems that ultimately map to byte indices:
  - XPath:      element type + sibling count  (1-indexed, counts by node type)
  - EPUB CFI:   spine-relative integer steps   (2-indexed, counts all children ×2)
  - Line/col:   trivially derived from byte index via a newline-offset table
  - Byte index: the canonical form everything converts through
*/

use std::ops::Range;

use chumsky::span::SimpleSpan;

pub mod attributes;
pub mod cdata;
pub mod comment;
pub mod declaration;
pub mod element;
pub mod entity;
pub mod instruction;
pub mod parent;
pub mod text;
pub mod unit;

// ─── Span ────────────────────────────────────────────────────────────────────

/// A half-open byte range `[start, end)` into the source string.
///
/// We define our own type instead of reusing `std::ops::Range` because:
///   - `Range<usize>` implements `Iterator`, making it non-`Copy`.
///   - `SimpleSpan` lacks a simple public constructor.
///   - We want to add domain helpers (`.contains`, `.merge`, etc.) without orphan issues.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub start: usize,
    /// Exclusive upper bound (standard Rust convention).
    pub end: usize,
}

impl Span {
    #[inline]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Span covering from the start of `a` to the end of `b`.
    ///
    /// Panics (debug) if `b.start < a.start`.
    #[inline]
    pub fn merge(a: Span, b: Span) -> Self {
        debug_assert!(a.start <= b.end, "merge: spans are disjoint/reversed");
        Self::new(a.start, b.end)
    }

    #[inline]
    pub fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    #[inline]
    pub fn is_empty(self) -> bool {
        self.end <= self.start
    }

    /// Returns `true` if `idx` falls inside `[start, end)`.
    #[inline]
    pub fn contains(self, idx: usize) -> bool {
        self.start <= idx && idx < self.end
    }

    /// Slice the source string to the bytes covered by this span.
    ///
    /// # Panics
    /// Panics if the span is out of bounds for `src` — the same guarantee
    /// you get from `&str` indexing.  Because spans are always produced by
    /// the same parser that consumed the source, this should never fire in
    /// practice.
    #[inline]
    pub fn slice<'a>(self, src: &'a str) -> &'a str {
        &src[self.start..self.end]
    }
}

impl From<SimpleSpan> for Span {
    #[inline]
    fn from(s: SimpleSpan) -> Self {
        Self::new(s.start, s.end)
    }
}

impl From<Range<usize>> for Span {
    #[inline]
    fn from(r: Range<usize>) -> Self {
        Self::new(r.start, r.end)
    }
}

impl From<Span> for Range<usize> {
    #[inline]
    fn from(s: Span) -> Self {
        s.start..s.end
    }
}

// ─── Core traits ─────────────────────────────────────────────────────────────

/// Types that can report their full source extent as a single [`Span`].
///
/// "Full" means from the very first byte (e.g. the `<`) to the very last byte
/// (e.g. the `>`), inclusive of all delimiters.
pub trait FullSpan {
    fn full_span(&self) -> Span;

    /// Convenience: slice the source string to this node's full extent.
    #[inline]
    fn slice_source<'a>(&self, src: &'a str) -> &'a str {
        self.full_span().slice(src)
    }
}

/// Types that expose the span of their element/tag name.
pub trait NameSpan {
    fn name_span(&self) -> Span;

    /// Convenience: get the name string from the source.
    #[inline]
    fn name<'a>(&self, src: &'a str) -> &'a str {
        self.name_span().slice(src)
    }
}

/// Lazy re-parsing bridge.
///
/// A *span* type records only byte offsets during the fast first pass.
/// The corresponding *parsed* type is constructed on demand by slicing
/// the original source and re-running the (cheap, infallible) sub-parser.
///
/// Because the span was produced by the same parser, `from_parsed` is
/// guaranteed to succeed — use `unwrap()` inside it freely.
pub trait FromParsedSpan<'a>: Sized {
    type Span: FullSpan;

    fn from_parsed(span: Self::Span, source: &'a str) -> Self;
}
