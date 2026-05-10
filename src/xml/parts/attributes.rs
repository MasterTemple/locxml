use std::collections::BTreeMap;

use chumsky::{
    prelude::*,
    text::{ident, whitespace},
};
use derive_more::Deref;
use from_nested_tuple::FromTuple;

use crate::xml::parts::{FromParsedSpan, FullSpan, Span};

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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, FromTuple)]
pub struct AttributesSpan(SimpleSpan);

impl AttributesSpan {
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        ident()
            .to_slice()
            .then_ignore(just("="))
            .then_ignore(quoted())
            .separated_by(whitespace().at_least(1))
            .to_span()
            .map(|span| Self(span))
    }
}

impl FullSpan for AttributesSpan {
    fn full_span(&self) -> Span {
        self.0.into()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, FromTuple, Deref)]
pub struct Attributes<'a> {
    #[deref]
    map: BTreeMap<&'a str, &'a str>,
    span: AttributesSpan,
}

impl<'a> Attributes<'a> {
    pub fn parser() -> impl Parser<'a, &'a str, BTreeMap<&'a str, &'a str>> + Clone {
        ident()
            .to_slice()
            .then_ignore(just('='))
            .then(quoted())
            .separated_by(whitespace().at_least(1))
            .collect()
    }
}

impl<'a> FromParsedSpan<'a> for Attributes<'a> {
    type ParsedSpan = AttributesSpan;

    fn from_parsed(span: Self::ParsedSpan, source: &'a str) -> Self {
        let input = span.get_slice(source);
        let map = Self::parser().parse(input).unwrap();
        Self { map, span }
    }
}

#[cfg(test)]
mod tests {
    macro_rules! check {
        ($input:literal, ok) => {
            assert!(AttributesSpan::parser().parse($input).into_result().is_ok());
        };
        ($input:literal, err) => {
            assert!(
                AttributesSpan::parser()
                    .parse($input)
                    .into_result()
                    .is_err()
            );
        };
    }
    use super::*;

    #[test]
    fn attributes_span() {
        check!(r#"a="1""#, ok);
        check!(r#"a="1" b="2""#, ok);
        check!(r#"a="1" b="2" "#, err);
        check!(r#"a="1" b="2" c='quote: "'"#, ok);

        let src = r#"a="1" b="2" c='quote: "'"#;
        let span = AttributesSpan::parser().parse(src).into_result().unwrap();
        dbg!(Attributes::from_parsed(span, src));
    }
}
