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

pub trait FullSpan {
    fn full_span(&self) -> Span;
    fn get_slice<'a>(&self, source: &'a str) -> &'a str {
        let Span { start, end } = self.full_span();
        &source[start..end]
    }
}

pub trait NameSpan {
    fn name_span(&self) -> Span;
}

pub type Idx = usize;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    start: Idx,
    /// I think this is exclusive
    end: Idx,
}
// TODO: impl idx/slice for Span

impl Span {
    pub fn new(start: Idx, end: Idx) -> Self {
        Self { start, end }
    }
    pub fn merge(start: Span, end: Span) -> Self {
        Self::new(start.start, end.end)
    }
    pub fn contains(&self, idx: usize) -> bool {
        self.start <= idx && idx < self.end
    }
}
impl From<SimpleSpan> for Span {
    fn from(SimpleSpan { start, end, .. }: SimpleSpan) -> Self {
        Self::new(start, end)
    }
}

impl From<Range<Idx>> for Span {
    fn from(Range { start, end, .. }: Range<Idx>) -> Self {
        Self::new(start, end)
    }
}

impl Into<Range<Idx>> for Span {
    fn into(self) -> Range<Idx> {
        self.start..self.end
    }
}
