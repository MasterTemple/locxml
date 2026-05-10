use chumsky::prelude::*;

use crate::xml::parts::{FullSpan, Span, entity::EntitySpan};

// ─── TextChunk ────────────────────────────────────────────────────────────────

/// A single run of either raw characters or a single entity reference,
/// within a text node.
///
/// This fine-grained split is required for correct character-offset computation:
/// each `EntitySpan` counts as exactly one logical character, whereas each
/// `Raw` slice contributes `str::chars().count()` characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TextChunk {
    /// A run of literal characters (no `<` or `&`).
    Raw(Span),
    /// A character entity reference (`&...;`), counts as 1 logical char.
    Entity(EntitySpan),
}

impl TextChunk {
    /// Number of *logical* characters this chunk contributes.
    ///
    /// Used by XPath and EPUB CFI character-offset calculations.
    pub fn logical_char_len(self, src: &str) -> usize {
        match self {
            TextChunk::Raw(span) => span.slice(src).chars().count(),
            TextChunk::Entity(e) => e.logical_char_len(),
        }
    }
}

impl FullSpan for TextChunk {
    fn full_span(&self) -> Span {
        match self {
            TextChunk::Raw(s) => *s,
            TextChunk::Entity(e) => e.full_span(),
        }
    }
}

// ─── TextSpan ────────────────────────────────────────────────────────────────

/// A text node: one or more runs of raw text and/or entity references,
/// terminated by `<` (the start of the next tag).
///
/// The `chunks` vec is required (not optional) because character-offset
/// traversal must distinguish entities from raw text. This is the only
/// place we *need* that granularity at the span stage.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextSpan {
    /// Covers the entire text content from the first byte to the last.
    pub span: Span,
    /// Ordered runs making up the text content.
    pub chunks: Vec<TextChunk>,
}

impl TextSpan {
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        let entity = EntitySpan::parser().map(TextChunk::Entity);

        // Raw text: any run of bytes that aren't `<` (tag start) or `&` (entity start).
        let raw = none_of("<&")
            .repeated()
            .at_least(1)
            .to_span()
            .map(|s: SimpleSpan| TextChunk::Raw(s.into()));

        entity
            .or(raw)
            .repeated()
            .at_least(1)
            .collect::<Vec<_>>()
            .map_with(|chunks, extra| {
                // Build the overall span from the first chunk start to the last chunk end.
                let span: SimpleSpan = extra.span();
                TextSpan {
                    span: span.into(),
                    chunks,
                }
            })
    }

    /// Total logical character length of this text node.
    ///
    /// This is the value used by XPath `string-length()` and EPUB CFI character offsets.
    pub fn logical_char_len(&self, src: &str) -> usize {
        self.chunks.iter().map(|c| c.logical_char_len(src)).sum()
    }
}

impl FullSpan for TextSpan {
    fn full_span(&self) -> Span {
        self.span
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text() {
        let src = "hello world";
        let t = TextSpan::parser().parse(src).into_result().unwrap();
        assert_eq!(t.logical_char_len(src), 11);
        assert_eq!(t.chunks.len(), 1);
    }

    #[test]
    fn text_with_entities() {
        let src = "a &lt; b &amp; c";
        let t = TextSpan::parser().parse(src).into_result().unwrap();
        // "a " + entity + " b " + entity + " c"
        // logical chars: 2 + 1 + 3 + 1 + 2 = 9
        assert_eq!(t.logical_char_len(src), 9);
    }

    #[test]
    fn entity_only() {
        let src = "&nbsp;";
        let t = TextSpan::parser().parse(src).into_result().unwrap();
        assert_eq!(t.logical_char_len(src), 1);
    }
}
