use chumsky::prelude::*;
use derive_more::Deref;
use from_nested_tuple::FromTuple;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, FromTuple, Deref)]
pub struct TextSpan(SimpleSpan);

impl TextSpan {
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        none_of("<").repeated().at_least(1).to_span().from_tuple()
    }
}
