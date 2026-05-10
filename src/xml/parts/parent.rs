use chumsky::{container::Seq, prelude::*, text::whitespace};
use derive_more::{Deref, From};
use from_nested_tuple::FromTuple;

use crate::xml::parts::{
    FullSpan, NameSpan, Span, attributes::AttributesSpan, element::ElementSpan,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, FromTuple, Deref, From)]
pub struct TagNameSpan {
    pub ident: SimpleSpan,
}

const DISALLOWED_TAG_SYMBOLS: &[char] = &[
    '!', '"', '#', '$', '%', '&', '\'', '(', ')', '*', '+', ',', '/', ';', '<', '=', '>', '?', '@',
    '[', '\\', ']', '^', '`', '{', '|', '}', '~',
];

const DISALLOWED_TAG_START_SYMBOLS: &[char] = &['-', '.'];

impl TagNameSpan {
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        none_of(DISALLOWED_TAG_SYMBOLS)
            .and_is(none_of(DISALLOWED_TAG_START_SYMBOLS))
            .and_is(text::int(10).not())
            .and_is(whitespace().at_least(1).not())
            .then(
                none_of(DISALLOWED_TAG_SYMBOLS)
                    .and_is(whitespace().at_least(1).not())
                    .repeated(),
            )
            .to_span()
            .from_tuple()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, FromTuple)]
pub struct ParentSpan {
    /// `<name a="1" b="2">`
    pub open: OpeningTagSpan,

    /// Everything in between
    // PERF: What if I just store a range here, don't nest anything, and have 1 big list of everything?
    // I just need to store an index to the parent Element, as well as what my index is relative
    // to the parent Element (just dont have the root element in the list, so I don't have to make
    // them all Option<T>)
    // The only tricky part is that I will have to write children when I still have yet to complete
    // this one. Perhaps a list of None with a current index and then once i pass the closing tag,
    // take the parent index from my traversal stack, and *master_list = Some(parent)
    pub children: Vec<ElementSpan>,

    /// `</name>`
    pub close: ClosingTagSpan,
}

impl ParentSpan {
    /// NOTE: This is not the same as [`is_in`]
    pub fn is_at(&self, idx: usize) -> bool {
        self.open.full_span().contains(idx) || self.close.full_span().contains(idx)
    }

    pub fn is_in(&self, idx: usize) -> bool {
        self.full_span().contains(idx)
    }

    pub fn get_element(&self, idx: usize) -> &ElementSpan {
        for child in self.children.iter() {
            if child.is_in(idx) {
                return child.get_element(idx);
            }
        }
        panic!("Beyond index, should have been stopped at root")
    }

    pub fn get_path<'a, 'v>(&'a self, idx: usize, path: &'v mut Vec<&'a ElementSpan>) {
        for child in self.children.iter() {
            if child.is_in(idx) {
                return child.get_path(idx, path);
            }
        }
        panic!("Beyond index, should have been stopped at root")
    }
}

impl NameSpan for ParentSpan {
    fn name_span(&self) -> Span {
        Span::from(self.open.name.ident)
    }
}

impl FullSpan for ParentSpan {
    fn full_span(&self) -> Span {
        // `<`
        let start = self.open.name.start - 1;
        // `>`
        let end = self.close.trailing.end + 1;
        Span::new(start, end)
    }
}

// <name a="1" b="2">
// |    |           |
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, FromTuple)]
pub struct OpeningTagSpan {
    // `name`
    pub name: TagNameSpan,

    // `a="1" b="2"`
    pub attributes: Option<AttributesSpan>,
    pub trailing: SimpleSpan,
}

impl OpeningTagSpan {
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        // TODO: What are the XML with ? and !
        just('<')
            .ignore_then(TagNameSpan::parser())
            .then(
                whitespace()
                    .at_least(1)
                    .ignore_then(AttributesSpan::parser())
                    .or_not(),
            )
            .then(whitespace().to_span())
            .then_ignore(just('>'))
            .from_tuple()
    }
}
impl FullSpan for OpeningTagSpan {
    fn full_span(&self) -> Span {
        // `<`
        let start = self.name.start - 1;
        // `>`
        let end = self.trailing.end + 1;
        Span::new(start, end)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, FromTuple)]
pub struct ClosingTagSpan {
    pub name: TagNameSpan,
    pub trailing: SimpleSpan,
}

impl ClosingTagSpan {
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        just("</")
            .ignore_then(TagNameSpan::parser())
            .then(whitespace().to_span())
            .then_ignore(just('>'))
            .from_tuple()
    }
}

impl FullSpan for ClosingTagSpan {
    fn full_span(&self) -> Span {
        // `</`
        let start = self.name.start - 2;
        // `>`
        let end = self.trailing.end + 1;
        Span::new(start, end)
    }
}
