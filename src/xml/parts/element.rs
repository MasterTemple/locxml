use chumsky::prelude::*;
use from_nested_tuple::FromTuple;

use crate::xml::parts::{
    NameSpan, Span,
    parent::{ClosingTagSpan, OpeningTagSpan, ParentSpan},
    text::TextSpan,
    unit::UnitSpan,
};

/// NOTE: Only the parent is traversible
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ElementSpan {
    Parent(ParentSpan),
    Unit(UnitSpan),
    Text(TextSpan),
}

impl ElementSpan {
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        use ElementSpan as E;
        recursive(|element| {
            let parent = OpeningTagSpan::parser()
                .then(element.repeated().collect())
                .then(ClosingTagSpan::parser())
                .from_tuple()
                .map(E::Parent);

            let self_closing_tag = UnitSpan::parser().map(E::Unit);

            let text = TextSpan::parser().map(E::Text);

            parent.or(self_closing_tag).or(text)
        })
    }

    // pub fn get_element(&self, idx: usize) -> &ElementSpan {
    //     match self {
    //         ElementSpan::Parent(parent) => {
    //             if parent.is_at(idx) {
    //                 self
    //             } else {
    //                 parent.get_element(idx)
    //             }
    //         }
    //         ElementSpan::Unit(_) | ElementSpan::Text(_) => self,
    //     }
    // }

    pub fn get_element(&self, idx: usize) -> &ElementSpan {
        match self {
            ElementSpan::Parent(parent) => {
                if parent.is_at(idx) {
                    self
                } else {
                    parent.get_element(idx)
                }
            }
            ElementSpan::Unit(_) | ElementSpan::Text(_) => self,
        }
    }

    pub fn get_path<'a, 'v>(&'a self, idx: usize, path: &'v mut Vec<&'a ElementSpan>) {
        path.push(self);
        // if self.is_at(idx) {
        //     path.push(self);
        //     return path;
        // }
        match self {
            ElementSpan::Parent(parent) => {
                if parent.is_at(idx) {
                    // path
                } else {
                    parent.get_path(idx, path)
                }
            }
            ElementSpan::Unit(_) | ElementSpan::Text(_) => (),
        }
    }

    pub fn is_at(&self, idx: usize) -> bool {
        match self {
            ElementSpan::Parent(e) => e.is_at(idx),
            ElementSpan::Unit(_) => true,
            ElementSpan::Text(_) => true,
        }
    }

    pub fn is_in(&self, idx: usize) -> bool {
        match self {
            ElementSpan::Parent(e) => e.is_in(idx),
            ElementSpan::Unit(_) => true,
            ElementSpan::Text(_) => true,
        }
    }

    pub fn name_span(&self) -> Option<Span> {
        Some(match self {
            ElementSpan::Parent(e) => e.name_span(),
            ElementSpan::Unit(e) => e.name_span(),
            ElementSpan::Text(_) => None?,
        })
    }
}
