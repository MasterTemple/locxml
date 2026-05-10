use chumsky::{
    prelude::*,
    text::{ident, whitespace},
};
use derive_more::Deref;
use from_nested_tuple::FromTuple;

use crate::xml::parts::{FullSpan, Span};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, FromTuple, Deref)]
pub struct EntitySpan(SimpleSpan);

impl EntitySpan {
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        just('&')
            .ignore_then(none_of(';').repeated())
            .ignore_then(just(';'))
            .ignored()
            .to_span()
            .from_tuple()
    }
}

impl FullSpan for EntitySpan {
    fn full_span(&self) -> Span {
        Span::from(self.0)
    }
}

#[cfg(test)]
mod tests {
    macro_rules! check {
        ($input:literal, ok) => {
            assert!(EntitySpan::parser().parse($input).into_result().is_ok());
        };
        ($input:literal, err) => {
            assert!(EntitySpan::parser().parse($input).into_result().is_err());
        };
    }
    use super::*;
    #[test]
    fn entity_span() {
        check!("&lt;", ok);
        check!("&;", ok);
        check!("&lt", err);
    }
}
