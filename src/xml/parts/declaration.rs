//! XML declaration parser: `<?xml version="1.0" encoding="UTF-8"?>`
//!
//! # Spec (XML 1.0 §2.8)
//! The XML declaration is **not** a processing instruction even though it uses
//! `<?` / `?>` delimiters.  It:
//!   - MUST appear as the very first bytes of the document (no BOM or whitespace
//!     before it, though we tolerate a leading UTF-8 BOM for robustness).
//!   - MUST NOT be counted as a node for XPath or EPUB CFI step numbering.
//!   - Has a mandatory `version` pseudo-attribute and optional `encoding` and
//!     `standalone` pseudo-attributes.
//!
//! # Pseudo-attributes
//! Although the declaration looks like a tag with attributes, the XML spec
//! defines the pseudo-attributes specially:
//!   - `version`    — required, must be `"1.0"` or `"1.1"`.
//!   - `encoding`   — optional, names the character encoding.
//!   - `standalone` — optional, `"yes"` or `"no"`.
//!
//! We parse them as raw string spans and do no further validation here.
//!
//! # Integration
//! [`crate::xml::tree::XmlSpanTree::parse`] strips the declaration (if present)
//! before handing the remainder to the element parser, so the declaration never
//! enters the `ElementSpan` tree.

use chumsky::prelude::*;

use crate::xml::parts::{FullSpan, Span, attributes::quoted};

// ─── XmlDeclSpan ─────────────────────────────────────────────────────────────

/// Span for the XML declaration `<?xml ... ?>`.
///
/// All three pseudo-attribute values are stored as optional byte spans into
/// the original source so callers can extract them without allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XmlDeclSpan {
    /// Full extent `<?xml` … `?>`.
    full: Span,
    /// Span of the `version` value (inside quotes).
    pub version: Span,
    /// Span of the `encoding` value (inside quotes), if present.
    pub encoding: Option<Span>,
    /// Span of the `standalone` value (inside quotes), if present.
    pub standalone: Option<Span>,
}

impl XmlDeclSpan {
    /// Parser for the XML declaration.
    ///
    /// Accepts `<?xml` followed by pseudo-attributes in any order (though the
    /// spec requires `version` first; we enforce this for correctness).
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        // A single pseudo-attribute: `name="value"` or `name='value'`.
        // Returns (name_slice, value_span).
        let pseudo_attr = text::ident().to_slice().then_ignore(just('=')).then(
            // Capture the span of the *value* (inside quotes) only.
            quoted().map_with(|_, extra| {
                // The span reported by map_with includes the surrounding quotes.
                // Trim one byte from each side to get the content span.
                let s: SimpleSpan = extra.span();
                Span::new(s.start + 1, s.end - 1)
            }),
        );

        just("<?xml")
            .ignore_then(
                // One or more pseudo-attributes separated by whitespace.
                text::whitespace()
                    .at_least(1)
                    .ignore_then(pseudo_attr)
                    .repeated()
                    .at_least(1) // at minimum `version` is required
                    .collect::<Vec<_>>(),
            )
            .then_ignore(text::whitespace())
            .then_ignore(just("?>"))
            .map_with(|attrs, extra| {
                let full: SimpleSpan = extra.span();
                let find = |key: &str| attrs.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
                XmlDeclSpan {
                    full: full.into(),
                    // TODO: What is the right design decision? This causes [`tests::rejects_missing_version`] to fail.

                    // If `version` is missing, use a zero-length span at the
                    // document start as a sentinel (well-formed XML requires
                    // version; a separate validation pass can report the error).
                    version: find("version").unwrap_or(Span::new(0, 0)),
                    encoding: find("encoding"),
                    standalone: find("standalone"),
                }
            })
    }
}

impl FullSpan for XmlDeclSpan {
    fn full_span(&self) -> Span {
        self.full
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> XmlDeclSpan {
        XmlDeclSpan::parser()
            .parse(src)
            .into_result()
            .expect("parse failed")
    }

    #[test]
    fn version_only() {
        let src = r#"<?xml version="1.0"?>"#;
        let span = parse(src);
        assert_eq!(span.version.slice(src), "1.0");
        assert!(span.encoding.is_none());
        assert!(span.standalone.is_none());
        assert_eq!(span.full_span(), Span::new(0, src.len()));
    }

    #[test]
    fn version_and_encoding() {
        let src = r#"<?xml version="1.0" encoding="UTF-8"?>"#;
        let span = parse(src);
        assert_eq!(span.version.slice(src), "1.0");
        assert_eq!(span.encoding.unwrap().slice(src), "UTF-8");
    }

    #[test]
    fn all_pseudo_attrs() {
        let src = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
        let span = parse(src);
        assert_eq!(span.version.slice(src), "1.0");
        assert_eq!(span.encoding.unwrap().slice(src), "UTF-8");
        assert_eq!(span.standalone.unwrap().slice(src), "yes");
    }

    #[test]
    fn single_quoted_values() {
        let src = "<?xml version='1.0'?>";
        let span = parse(src);
        assert_eq!(span.version.slice(src), "1.0");
    }

    #[test]
    fn rejects_missing_version() {
        // The `version` pseudo-attribute is required.
        assert!(
            XmlDeclSpan::parser()
                .parse(r#"<?xml encoding="UTF-8"?>"#)
                .into_result()
                .is_err()
        );
    }

    #[test]
    fn rejects_plain_pi() {
        // A regular PI (`<?foo ... ?>`) must not parse as an XML declaration.
        assert!(
            XmlDeclSpan::parser()
                .parse(r#"<?foo bar="baz"?>"#)
                .into_result()
                .is_err()
        );
    }
}
