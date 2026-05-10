Rust + Chumsky
I am creating an XML parser that will let me effeciently traverse the tree in specific ways optimal for my use cases.
My goal is to convert between the following:
- EPUB CFI (at least the sub-path for this XML document)
- XPath (with a character offset where an entity is length 1; I am choosing this because that is how the Browser Range API calculates it),
- Line column pair of an XML file (trivial from byte index)
- Byte index (usize) of an XML file

The idea is that given one of the above locations, I will find the corresponding byte index, and then convert it into the alternate location

I am at the stage where I want to construct an XML tree that is optimal for this.

Here is what I was thinking:

- I just need to store the spans of each open/close tag pair, each self-closing tag, each entity (since entities only count as 1 character), and each text node.
I also store some spans related to parsing later (for example, the span of the attributes in a tag),
this is without any surrounding padding: padding is managed by the parent who can calculate it from the difference of 2 saved elements

This would result in me creating 2 trees

I am creating 2 trees
This also means I am creating 2 structs: one that is just the spans, the other that is parsed

The first tree is just a list of spans
To lazily mirror that, I want a second tree

Help me

1. Come up with a good trait(s) that make what I am doing more ergonomic

2. I want both parsers to be the same, except in one I ignore outputs and convert to spans, or I actually parse the data
Alternatively stated, the first pass through validates the tree and gives me spans that are important
I want to keep the values
For example, I don't need to create a map for most of the elements' attributes
However, when I want to, I can parse the slice that corresponds to the attributes, given the spans and the source
If my parsers are the same, then I know that the parse will always succeed

(The only one I have done this before is `./src/xml/parts/attributes.rs`), but I don't have a trait, just the general idea

3. How can I traverse the tree and update the other tree to either hold None or Some(), but when it is none, then parse it and insert some

4. What are pitfalls and things I need to look out for in my project

Other Notes:

5. I am using a custom `Span` type because `SimpleSpan` doesn't have a nice constructor, and allegedly `std::ops::Range` holds an iterator, but please improve this as you see fit

```rust
use std::collections::BTreeMap;

use chumsky::{
    prelude::*,
    text::{ident, whitespace},
};
use derive_more::Deref;
use from_nested_tuple::FromTuple;

use crate::xml::parts::{FullSpan, Span};


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, FromTuple)]
pub struct AttributesSpan(SimpleSpan);

pub fn quoted<'a>() -> impl Parser<'a, &'a str, &'a str> {
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
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> {
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

    pub fn parser() -> impl Parser<'a, &'a str, BTreeMap<&'a str, &'a str>> {
        ident()
            .to_slice()
            .then_ignore(just('='))
            .then(quoted())
            .separated_by(whitespace().at_least(1))
            .collect()
    }
}
```
