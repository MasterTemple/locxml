use chumsky::{prelude::*, text::whitespace};
use from_nested_tuple::FromTuple;

use crate::xml::parts::{NameSpan, Span, attributes::AttributesSpan, parent::TagNameSpan};

/// Self-Closing
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, FromTuple)]
pub struct UnitSpan {
    pub name: TagNameSpan,
    pub attributes: Option<AttributesSpan>,
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
            .then_ignore(just("/>"))
            .from_tuple()
    }
}

impl NameSpan for UnitSpan {
    fn name_span(&self) -> Span {
        Span::from(self.name.ident)
    }
}
