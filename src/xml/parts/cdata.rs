//! CDATA section parser: `<![CDATA[ ... ]]>`
//!
//! # Spec (XML 1.0 §2.7)
//! A CDATA section begins with `<![CDATA[` and ends at the first `]]>`.
//! The content between the delimiters is treated as **literal character data**:
//! no entity or markup recognition occurs inside CDATA.
//!
//! # Character-offset semantics
//! Each Unicode scalar value inside a CDATA section counts as **one** logical
//! character for XPath and EPUB CFI purposes — the same rule as raw text.
//! Entities inside CDATA (e.g. `&amp;`) are NOT decoded; they count as their
//! literal byte sequence parsed as Unicode chars.
//!
//! # Tree placement
//! For XPath / EPUB CFI node counting, a CDATA section is equivalent to a
//! text node — it occupies an **odd** CFI step and is counted as a `text()`
//! node in XPath.  In the current implementation we represent it as a
//! `TextSpan` containing a single `Raw` chunk covering the decoded content.
//! (Future work: a dedicated `CdataSpan` variant for round-trip fidelity.)

use chumsky::prelude::*;

use crate::xml::parts::{FullSpan, Span};

// ─── CdataSpan ───────────────────────────────────────────────────────────────

/// Span for a CDATA section `<![CDATA[ ... ]]>`.
///
/// `content` covers only the literal text *inside* the delimiters (i.e.
/// excludes the `<![CDATA[` prefix and `]]>` suffix).  The full span (via
/// [`FullSpan`]) covers the entire construct including delimiters.
///
/// When computing character offsets treat `content` as a raw Unicode string
/// with no entity decoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CdataSpan {
    /// Byte span of the entire `<![CDATA[...]]>` construct.
    full: Span,
    /// Byte span of the content between `<![CDATA[` and `]]>`.
    pub content: Span,
}

impl CdataSpan {
    /// Parser for a complete CDATA section.
    ///
    /// We use `take_until` to scan up to the first `]]>` terminator, which is
    /// correct per the spec (the first occurrence of `]]>` ends the section,
    /// even if the content itself looks like XML).
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        // Match `<![CDATA[`, then accumulate until `]]>`.
        just("<![CDATA[")
            .ignore_then(
                // Collect raw content bytes up to (but not including) `]]>`.
                any()
                    .and_is(just("]]>").not())
                    .repeated()
                    .to_slice()
                    // Capture the span of just the content.
                    .map_with(|_, extra| {
                        let s: SimpleSpan = extra.span();
                        Span::from(s)
                    }),
            )
            .then_ignore(just("]]>"))
            // Capture the full span (including both delimiters).
            .map_with(|content, extra| {
                let full: SimpleSpan = extra.span();
                CdataSpan {
                    full: full.into(),
                    content,
                }
            })
    }

    /// The number of logical characters in the content (Unicode scalars, no
    /// entity decoding because CDATA is raw).
    pub fn logical_char_len(self, src: &str) -> usize {
        self.content.slice(src).chars().count()
    }
}

impl FullSpan for CdataSpan {
    fn full_span(&self) -> Span {
        self.full
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> CdataSpan {
        CdataSpan::parser()
            .parse(src)
            .into_result()
            .expect("parse failed")
    }

    #[test]
    fn simple_cdata() {
        let src = "<![CDATA[hello world]]>";
        let span = parse(src);
        assert_eq!(span.content.slice(src), "hello world");
        assert_eq!(span.full_span(), Span::new(0, src.len()));
    }

    #[test]
    fn cdata_with_markup_inside() {
        // Markup characters inside CDATA are literal, not parsed.
        let src = "<![CDATA[<b>not markup</b>]]>";
        let span = parse(src);
        assert_eq!(span.content.slice(src), "<b>not markup</b>");
    }

    #[test]
    fn cdata_with_entity_literal() {
        // &amp; inside CDATA is 5 literal characters, not decoded to '&'.
        let src = "<![CDATA[a &amp; b]]>";
        let span = parse(src);
        assert_eq!(span.content.slice(src), "a &amp; b");
        // Logical char len: 9 Unicode scalars (no entity decoding).
        assert_eq!(span.logical_char_len(src), 9);
    }

    #[test]
    fn empty_cdata() {
        let src = "<![CDATA[]]>";
        let span = parse(src);
        assert_eq!(span.content.slice(src), "");
        assert_eq!(span.logical_char_len(src), 0);
    }

    #[test]
    fn rejects_missing_terminator() {
        // A CDATA section without `]]>` must fail.
        assert!(
            CdataSpan::parser()
                .parse("<![CDATA[unclosed")
                .into_result()
                .is_err()
        );
    }

    #[test]
    fn rejects_non_cdata() {
        assert!(
            CdataSpan::parser()
                .parse("<p>text</p>")
                .into_result()
                .is_err()
        );
    }
}
