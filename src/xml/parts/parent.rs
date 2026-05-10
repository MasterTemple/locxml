use std::sync::OnceLock;

use chumsky::{prelude::*, text::whitespace};

use crate::xml::parts::{
    FullSpan, NameSpan, Span, attributes::AttributesSpan, element::ElementSpan,
};

// ─── TagNameSpan ─────────────────────────────────────────────────────────────

/// The byte span of a tag name (e.g. `div` in `<div id="x">`).
///
/// XML name rules (from the spec):
/// - First character: letter, `_`, or `:` (we approximate with "not disallowed").
/// - Subsequent characters: letter, digit, `-`, `.`, `_`, `:`.
/// Names starting with `xml` (any case) are reserved but we don't reject them
/// here; validation is a separate pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TagNameSpan {
    pub span: Span,
}

impl TagNameSpan {
    // Characters forbidden anywhere in an XML name.
    const DISALLOWED: &'static [char] = &[
        '!', '"', '#', '$', '%', '&', '\'', '(', ')', '*', '+', ',', '/', ';', '<', '=', '>', '?',
        '@', '[', '\\', ']', '^', '`', '{', '|', '}', '~', ' ', '\t', '\r', '\n',
    ];
    // Additionally disallowed as the *first* character.
    const DISALLOWED_FIRST: &'static [char] =
        &['-', '.', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];

    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        none_of(Self::DISALLOWED)
            .and_is(none_of(Self::DISALLOWED_FIRST))
            .then(none_of(Self::DISALLOWED).repeated())
            .to_span()
            .map(|s: SimpleSpan| TagNameSpan { span: s.into() })
    }
}

impl FullSpan for TagNameSpan {
    fn full_span(&self) -> Span {
        self.span
    }
}

impl NameSpan for TagNameSpan {
    fn name_span(&self) -> Span {
        self.span
    }
}

// ─── OpeningTagSpan ──────────────────────────────────────────────────────────

/// Span for `<name attrs?>` (the opening tag, including both angle brackets).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct OpeningTagSpan {
    /// Span of the tag name only (no `<`).
    pub name: TagNameSpan,
    /// Span of the attribute list, if any attributes were present.
    pub attributes: Option<AttributesSpan>,
    /// Whitespace between the last attribute (or name) and `>`.
    /// Stored as a span so `FullSpan` can compute the exact end byte.
    pub trailing_ws: Span,
}

impl OpeningTagSpan {
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        just('<')
            .ignore_then(TagNameSpan::parser())
            .then(
                whitespace()
                    .at_least(1)
                    .ignore_then(AttributesSpan::parser())
                    .or_not(),
            )
            .then(whitespace().to_span())
            .then_ignore(just('>'))
            .map(
                |((name, attributes), ws): ((TagNameSpan, Option<AttributesSpan>), SimpleSpan)| {
                    OpeningTagSpan {
                        name,
                        attributes,
                        trailing_ws: ws.into(),
                    }
                },
            )
    }
}

impl FullSpan for OpeningTagSpan {
    fn full_span(&self) -> Span {
        // `<` is right before name.span.start; `>` is right after trailing_ws.end.
        Span::new(self.name.span.start - 1, self.trailing_ws.end + 1)
    }
}

impl NameSpan for OpeningTagSpan {
    fn name_span(&self) -> Span {
        self.name.span
    }
}

// ─── ClosingTagSpan ───────────────────────────────────────────────────────────

/// Span for `</name>`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClosingTagSpan {
    pub name: TagNameSpan,
    pub trailing_ws: Span,
}

impl ClosingTagSpan {
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        just("</")
            .ignore_then(TagNameSpan::parser())
            .then(whitespace().to_span())
            .then_ignore(just('>'))
            .map(|(name, ws): (TagNameSpan, SimpleSpan)| ClosingTagSpan {
                name,
                trailing_ws: ws.into(),
            })
    }
}

impl FullSpan for ClosingTagSpan {
    fn full_span(&self) -> Span {
        // `</` is two bytes before the name; `>` is one past trailing_ws.
        Span::new(self.name.span.start - 2, self.trailing_ws.end + 1)
    }
}

// ─── ParentSpan ───────────────────────────────────────────────────────────────

/// An element that has child nodes: `<name attrs?>...children...</name>`.
///
/// The `name` string is stored as a `OnceLock`-cached `Box<str>` so we pay the
/// allocation at most once, and only if the name is actually requested.
/// (In hot traversal paths like CFI/XPath you *always* need the name, but the
/// lock overhead is negligible compared to the alternative of a second tree.)
pub struct ParentSpan {
    pub open: OpeningTagSpan,
    pub children: Vec<ElementSpan>,
    pub close: ClosingTagSpan,

    /// Lazily allocated, heap-interned copy of the tag name.
    ///
    /// We cannot store `&'src str` here because `ParentSpan` is owned and
    /// self-referential storage would require `Pin`.  A `Box<str>` is the
    /// smallest owned alternative (same size as `String` but non-growable).
    name_cache: OnceLock<Box<str>>,
}

// Manual Clone/Debug/PartialEq because OnceLock doesn't derive them nicely.
impl Clone for ParentSpan {
    fn clone(&self) -> Self {
        Self {
            open: self.open.clone(),
            children: self.children.clone(),
            close: self.close.clone(),
            name_cache: OnceLock::new(), // cache is per-instance; cheaply reset
        }
    }
}

impl std::fmt::Debug for ParentSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParentSpan")
            .field("open", &self.open)
            .field("children", &self.children)
            .field("close", &self.close)
            .finish()
    }
}

impl PartialEq for ParentSpan {
    fn eq(&self, other: &Self) -> bool {
        self.open == other.open && self.children == other.children && self.close == other.close
    }
}

impl Eq for ParentSpan {}

impl ParentSpan {
    pub fn new(open: OpeningTagSpan, children: Vec<ElementSpan>, close: ClosingTagSpan) -> Self {
        Self {
            open,
            children,
            close,
            name_cache: OnceLock::new(),
        }
    }

    /// Get the tag name, allocating a heap copy at most once.
    ///
    /// If you already have the source `&str` and don't need to cache, prefer
    /// `self.open.name_span().slice(src)` for zero-allocation access.
    pub fn cached_name<'a>(&'a self, src: &str) -> &'a str {
        self.name_cache
            .get_or_init(|| self.open.name.span.slice(src).into())
    }

    /// `true` if `idx` falls in the opening or closing tag (not in the children).
    pub fn is_at(&self, idx: usize) -> bool {
        self.open.full_span().contains(idx) || self.close.full_span().contains(idx)
    }

    /// `true` if `idx` is anywhere within `[open_start, close_end)`.
    pub fn is_in(&self, idx: usize) -> bool {
        self.full_span().contains(idx)
    }

    /// Descend to the deepest node containing `idx`, or panic if `idx` is out
    /// of range (caller must ensure containment first via `is_in`).
    pub fn get_element(&self, idx: usize) -> &ElementSpan {
        for child in &self.children {
            if child.is_in(idx) {
                return child.get_element(idx);
            }
        }
        // `idx` is in the open/close tags; the caller already checked `is_at`.
        panic!(
            "ParentSpan::get_element: idx {idx} not found in children (is_at should have returned true)"
        );
    }

    /// Append all ancestors of `idx` (inclusive of this element) to `path`.
    pub fn push_path<'a>(&'a self, idx: usize, path: &mut Vec<&'a ElementSpan>) {
        for child in &self.children {
            if child.is_in(idx) {
                child.push_path(idx, path);
                return;
            }
        }
        // `idx` is in the open/close tag — we are already the deepest ancestor.
    }
}

impl NameSpan for ParentSpan {
    fn name_span(&self) -> Span {
        self.open.name.span
    }
}

impl FullSpan for ParentSpan {
    fn full_span(&self) -> Span {
        Span::merge(self.open.full_span(), self.close.full_span())
    }
}
