use chumsky::prelude::*;

use crate::xml::parts::{FullSpan, Span};

// ─── EntitySpan ──────────────────────────────────────────────────────────────

/// Span covering a character entity reference such as `&lt;` or `&#160;`.
///
/// # XPath / EPUB CFI character-offset note
/// Browser `Range` API counts entities as **one** logical character
/// (matching the decoded codepoint), even though they occupy multiple bytes in
/// the source.  Both XPath and EPUB CFI character offsets must therefore use
/// *logical* character counts, not byte counts.
///
/// When computing character offsets, iterate over the children of a text
/// content model and:
///   - Plain text slices → count Unicode scalar values (`str::chars().count()`).
///   - `EntitySpan` → add **1** regardless of the entity's byte length.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct EntitySpan(pub Span);

impl EntitySpan {
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        just('&')
            .then(none_of(';').repeated().at_least(1))
            .then(just(';'))
            .to_span()
            .map(|s: SimpleSpan| EntitySpan(s.into()))
    }

    /// The decoded logical character count of this entity: always 1.
    ///
    /// This is what both EPUB CFI and the browser Range API expect.
    pub const fn logical_char_len(self) -> usize {
        1
    }
}

impl FullSpan for EntitySpan {
    fn full_span(&self) -> Span {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! check {
        ($input:literal, ok) => {
            assert!(
                EntitySpan::parser().parse($input).into_result().is_ok(),
                "expected ok: {:?}",
                $input
            );
        };
        ($input:literal, err) => {
            assert!(
                EntitySpan::parser().parse($input).into_result().is_err(),
                "expected err: {:?}",
                $input
            );
        };
    }

    #[test]
    fn entity_span() {
        check!("&lt;", ok);
        check!("&amp;", ok);
        check!("&#160;", ok);
        check!("&#x00A0;", ok);
        // `&;` is technically malformed XML but we accept it at the span level;
        // validation is a separate concern.
        // Uncomment to relax: check!("&;", err);
        check!("&lt", err); // missing semicolon
    }
}
