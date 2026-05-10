use std::collections::BTreeMap;

use chumsky::{
    prelude::*,
    text::{ident, whitespace},
};
use derive_more::Deref;
use from_nested_tuple::FromTuple;

use crate::xml::parts::{FullSpan, Span};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, FromTuple, Deref)]
pub struct Attributes<'a> {
    #[deref]
    map: BTreeMap<&'a str, &'a str>,
    span: AttributesSpan,
}
impl<'a> Attributes<'a> {
    pub fn new(span: AttributesSpan, source: &'a str) -> Self {
        let input = span.get_slice(source);
        let map = Self::parser().parse(input).unwrap();
        Self { map, span }
    }

    pub fn parser() -> impl Parser<'a, &'a str, BTreeMap<&'a str, &'a str>> + Clone {
        ident()
            .to_slice()
            .then_ignore(just('='))
            .then(quoted())
            .separated_by(whitespace().at_least(1))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, FromTuple)]
pub struct AttributesSpan(SimpleSpan);

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

impl AttributesSpan {
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        ident()
            .to_slice()
            .then_ignore(just("="))
            .then(quoted())
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
        dbg!(Attributes::new(span, src));
    }
}

//
// fn asdf() {
//     let int = text::int::<_, extra::Err<Rich<char>>>(10)
//         .from_str()
//         .unwrapped();
//
//     // By default, accepts any number of items
//     let item = text::ascii::ident().padded().repeated();
//
//     // With configuration, we can declare an exact number of items based on a prefix length
//     let len_prefixed_arr = int.with_ctx(
//         item.configure(|repeat, ctx| repeat.exactly(*ctx))
//             .collect::<Vec<_>>(),
//     );
//
//     assert_eq!(
//         len_prefixed_arr.parse("2 foo bar").into_result(),
//         Ok(vec!["foo", "bar"]),
//     );
//
//     assert_eq!(len_prefixed_arr.parse("0").into_result(), Ok(vec![]),);
//
//     len_prefixed_arr
//         .parse("3 foo bar baz bam")
//         .into_result()
//         .unwrap_err();
//     len_prefixed_arr
//         .parse("3 foo bar")
//         .into_result()
//         .unwrap_err();
// }
