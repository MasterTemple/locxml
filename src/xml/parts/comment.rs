//! XML comment parser: `<!-- ... -->`
//!
//! # Spec (XML 1.0 §2.5)
//! Comments begin with `<!--` and end at the first `-->`.
//! The double-dash sequence `--` must not appear in the content.
//! We enforce this to stay spec-compliant.
//!
//! # Tree placement / node counting
//! Comments are **invisible** to both XPath `comment()` node counting (at the
//! positions used here) and EPUB CFI step numbering.  The spec says comments
//! occupy odd CFI steps, but in practice EPUB CFI implementations disagree.
//! Our choice: **skip comments entirely** when counting CFI/XPath positions,
//! because:
//!   1. Most EPUB content never has comments in body markup.
//!   2. The browser Range API never returns comments as container nodes.
//!
//! When the parser encounters a comment inside element content, it should
//! consume and discard the bytes (or store the span and exclude it from child
//! counting).  See the `ElementSpan` parser for integration.

use chumsky::prelude::*;

use crate::xml::parts::{FullSpan, Span};

// ─── CommentSpan ──────────────────────────────────────────────────────────────

/// Span for an XML comment `<!-- ... -->`.
///
/// `content` covers only the text *inside* the delimiters, excluding the
/// surrounding `<!--` and `-->`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CommentSpan {
    /// Full extent `<!--` … `-->`.
    full: Span,
    /// Content between `<!--` and `-->`.
    pub content: Span,
}

impl CommentSpan {
    /// Parser for a complete XML comment.
    ///
    /// Rejects `--` inside the content per XML 1.0 §2.5 ("the string `--`
    /// must not occur within comments").
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        just("<!--")
            .ignore_then(
                // Accept any character, but not the two-char sequence `--`.
                // We achieve this by accepting chars as long as they are NOT
                // the start of `-->` or `--` (which would be invalid XML).
                any()
                    .and_is(just("--").not())
                    .repeated()
                    .to_slice()
                    .map_with(|_, extra| {
                        let s: SimpleSpan = extra.span();
                        Span::from(s)
                    }),
            )
            .then_ignore(just("-->"))
            .map_with(|content, extra| {
                let full: SimpleSpan = extra.span();
                CommentSpan {
                    full: full.into(),
                    content,
                }
            })
    }
}

impl FullSpan for CommentSpan {
    fn full_span(&self) -> Span {
        self.full
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> CommentSpan {
        CommentSpan::parser()
            .parse(src)
            .into_result()
            .expect("parse failed")
    }

    #[test]
    fn simple_comment() {
        let src = "<!-- hello -->";
        let span = parse(src);
        assert_eq!(span.content.slice(src), " hello ");
        assert_eq!(span.full_span(), Span::new(0, src.len()));
    }

    #[test]
    fn comment_with_xml_chars() {
        // XML markup characters are allowed inside comments.
        let src = "<!-- <tag> & stuff -->";
        let span = parse(src);
        assert_eq!(span.content.slice(src), " <tag> & stuff ");
    }

    #[test]
    fn empty_comment() {
        let src = "<!---->";
        let span = parse(src);
        assert_eq!(span.content.slice(src), "");
    }

    #[test]
    fn rejects_double_dash_in_content() {
        // `--` inside a comment is forbidden by the XML spec.
        assert!(
            CommentSpan::parser()
                .parse("<!-- bad -- content -->")
                .into_result()
                .is_err()
        );
    }

    #[test]
    fn rejects_unclosed() {
        assert!(
            CommentSpan::parser()
                .parse("<!-- no end")
                .into_result()
                .is_err()
        );
    }
}
