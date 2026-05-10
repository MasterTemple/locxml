/*!
Basically the idea is to lazily parse: get top level structure (elements), but dont parse entities or attributes until requested (but I know that what I have inside the span is parseable)
*/

use std::{
    ops::{Index, Range},
    slice::SliceIndex,
};

use chumsky::span::SimpleSpan;

pub mod attributes;
pub mod cdata;
pub mod comment;
pub mod declaration;
/// Parent, Unit, or Text
pub mod element;
pub mod entity;
/// Processing Instruction
pub mod instruction;
/// Element with child
pub mod parent;
pub mod text;
/// Self-Closing Tag
pub mod unit;

/// Used to map between the span-parsed/validated span, and another struct with more helpful data
pub trait FromParsedSpan<'a> {
    type ParsedSpan;
    fn from_parsed(span: Self::ParsedSpan, source: &'a str) -> Self;
}

/// Used to get the full span from the beginning of the first part to the end of the last part
pub trait FullSpan {
    fn full_span(&self) -> Span;
    /// Used especially by [`FromParsedSpan::from_parsed`]
    fn get_slice<'a>(&self, source: &'a str) -> &'a str {
        let Span { start, end } = self.full_span();
        &source[start..end]
    }
}

/// Maybe not super helpful
pub trait NameSpan {
    fn name_span(&self) -> Span;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    start: usize,
    /// This is exclusive
    end: usize,
}
// TODO: impl usize/slice for Span

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
    pub fn merge(start: Span, end: Span) -> Self {
        Self::new(start.start, end.end)
    }
    pub fn contains(&self, usize: usize) -> bool {
        self.start <= usize && usize < self.end
    }
}
impl From<SimpleSpan> for Span {
    fn from(SimpleSpan { start, end, .. }: SimpleSpan) -> Self {
        Self::new(start, end)
    }
}

impl From<Range<usize>> for Span {
    fn from(Range { start, end, .. }: Range<usize>) -> Self {
        Self::new(start, end)
    }
}

impl Into<Range<usize>> for Span {
    fn into(self) -> Range<usize> {
        self.start..self.end
    }
}
