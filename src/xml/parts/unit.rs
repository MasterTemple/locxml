use chumsky::{prelude::*, text::whitespace};

use crate::xml::parts::{
    FullSpan, NameSpan, Span, attributes::AttributesSpan, parent::TagNameSpan,
};

// ─── UnitSpan ─────────────────────────────────────────────────────────────────

/// A self-closing element tag: `<name attrs?/>`.
///
/// Self-closing elements have no children and do not contribute to the
/// XPath position count of sibling *element* nodes the same way parent
/// elements do — they still count as elements.  They never contain text,
/// so they only appear at EPUB CFI even-numbered steps.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnitSpan {
    pub name: TagNameSpan,
    pub attributes: Option<AttributesSpan>,
    /// Whitespace before the `/>` terminator.
    pub trailing_ws: Span,
}

impl UnitSpan {
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
            .then_ignore(just("/>"))
            .map(
                |((name, attributes), ws): ((TagNameSpan, Option<AttributesSpan>), SimpleSpan)| {
                    UnitSpan {
                        name,
                        attributes,
                        trailing_ws: ws.into(),
                    }
                },
            )
    }
}

impl NameSpan for UnitSpan {
    fn name_span(&self) -> Span {
        self.name.span
    }
}

impl FullSpan for UnitSpan {
    fn full_span(&self) -> Span {
        // `<` is at name.span.start - 1; `/>` is two bytes after trailing_ws.end.
        Span::new(self.name.span.start - 1, self.trailing_ws.end + 2)
    }
}
