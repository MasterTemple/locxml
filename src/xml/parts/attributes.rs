use std::collections::BTreeMap;

use chumsky::{
    prelude::*,
    text::{ident, whitespace},
};

use crate::xml::parts::{FromParsedSpan, FullSpan, Span};

// ─── Quoted string ────────────────────────────────────────────────────────────

/// Parses a single- or double-quoted string, returning only the inner slice
/// (without the surrounding quotes).
pub fn quoted<'a>() -> impl Parser<'a, &'a str, &'a str> + Clone {
    choice((
        just('"')
            .ignore_then(none_of('"').repeated().to_slice())
            .then_ignore(just('"')),
        just('\'')
            .ignore_then(none_of('\'').repeated().to_slice())
            .then_ignore(just('\'')),
    ))
}

// ─── AttributesSpan ──────────────────────────────────────────────────────────

/// The byte range covering *all* attributes in a tag, as a single blob.
///
/// We defer parsing individual key=value pairs until they're actually needed;
/// that way the common case (traversal without attribute inspection) is free.
///
/// # Example
/// ```text
/// <div id="main" class="foo bar">
///      ^^^^^^^^^^^^^^^^^^^^^^^^^   ← AttributesSpan covers this
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AttributesSpan(pub Span);

impl AttributesSpan {
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        // We parse (and discard) full key="value" pairs, recording only the
        // overall span.  The key insight: because this runs inside the outer
        // element parser, we know the input is already valid XML.
        ident()
            .to_slice()
            .then_ignore(just('='))
            .then_ignore(quoted())
            .separated_by(whitespace().at_least(1))
            // TODO: Why did Claude add this?
            .at_least(1)
            .to_span()
            .map(|s: SimpleSpan| Self(s.into()))
    }
}

impl FullSpan for AttributesSpan {
    fn full_span(&self) -> Span {
        self.0
    }
}

// ─── Attributes (parsed / lazy) ──────────────────────────────────────────────

/// Fully-parsed attribute map, produced lazily from an [`AttributesSpan`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attributes<'a> {
    pub map: BTreeMap<&'a str, &'a str>,
    pub span: AttributesSpan,
}

impl<'a> Attributes<'a> {
    /// Parser for the *individual* key=value pairs (used inside `from_parsed`).
    fn kv_parser() -> impl Parser<'a, &'a str, BTreeMap<&'a str, &'a str>> + Clone {
        ident()
            .to_slice()
            .then_ignore(just('='))
            .then(quoted())
            .separated_by(whitespace().at_least(1))
            .at_least(1)
            .collect()
    }

    pub fn get(&self, key: &str) -> Option<&'a str> {
        self.map.get(key).copied()
    }
}

// TODO: Why did Claude do this?
// impl std::ops::Deref for Attributes<'_> {
//     type Target = BTreeMap<&'static str, &'static str>;
//     fn deref(&self) -> &Self::Target {
//         // SAFETY: This is a workaround; the real map holds &'a str.
//         // In practice callers use `.get()` directly.
//         // This Deref impl intentionally omits the lifetime to keep the API
//         // ergonomic; see `.map` field for direct access when needed.
//         unimplemented!("use .map or .get() instead of Deref on Attributes")
//     }
// }

impl<'a> FromParsedSpan<'a> for Attributes<'a> {
    type Span = AttributesSpan;

    fn from_parsed(span: AttributesSpan, source: &'a str) -> Self {
        let input = span.full_span().slice(source);
        // The span was produced by the same grammar, so this is infallible.
        let map = Self::kv_parser()
            .parse(input)
            .into_result()
            .expect("AttributesSpan re-parse should be infallible");
        Self { map, span }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! check {
        ($input:literal, ok) => {
            assert!(
                AttributesSpan::parser().parse($input).into_result().is_ok(),
                "expected ok for {:?}",
                $input
            );
        };
        ($input:literal, err) => {
            assert!(
                AttributesSpan::parser()
                    .parse($input)
                    .into_result()
                    .is_err(),
                "expected err for {:?}",
                $input
            );
        };
    }

    #[test]
    fn attributes_span() {
        check!(r#"a="1""#, ok);
        check!(r#"a="1" b="2""#, ok);
        // Trailing whitespace is not part of attributes; the parser stops before it.
        check!(r#"a="1" b="2" "#, err);
        check!(r#"a="1" b="2" c='quote: "'"#, ok);

        let src = r#"a="1" b="2" c='quote: "'"#;
        let span = AttributesSpan::parser().parse(src).into_result().unwrap();
        let attrs = Attributes::from_parsed(span, src);
        assert_eq!(attrs.map.get("a"), Some(&"1"));
        assert_eq!(attrs.map.get("b"), Some(&"2"));
        assert_eq!(attrs.map.get("c"), Some(&r#"quote: ""#));
    }
}
