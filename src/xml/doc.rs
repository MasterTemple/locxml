use std::ops::Range;

use crate::{
    utils::ref_owner::{RefTarget, TryRefTarget},
    xml::{parts::element::ElementSpan, tree::XmlSpanTree},
};
use chumsky::span::SimpleSpan;
use from_nested_tuple::FromTuple;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, FromTuple)]
pub struct XmlSpanDoc<'a> {
    pub xml: &'a str,
    pub tree: XmlSpanTree,
}

impl<'a> XmlSpanDoc<'a> {
    pub fn new(xml: &'a str, tree: XmlSpanTree) -> Self {
        Self { xml, tree }
    }
    pub fn parse(xml: &'a str) -> Result<Self, ()> {
        let tree = XmlSpanTree::parse(xml)?;
        Ok(Self::new(xml, tree))
    }
    pub fn get_str(&self, range: impl Into<Range<usize>>) -> &str {
        let range = range.into();
        &self.xml[range]
    }
    /**
    What are the things I would want?
    - All tags names: `a > b > c`
    - Index offsets for EPUB CFI
    - The element at that index
    */
    pub fn get_path(&self, idx: usize) -> Vec<&ElementSpan> {
        let mut path = vec![];
        self.tree.root.get_path(idx, &mut path);
        path
    }

    pub fn get_element(&self, idx: usize) -> Option<&ElementSpan> {
        if self.tree.root.is_in(idx) {
            Some(self.tree.root.get_element(idx))
        } else {
            None
        }
    }
}

impl<'a> TryRefTarget<'a, str> for XmlSpanDoc<'a> {
    type Err = ();
    fn try_from_ref(target: &'a str) -> Result<Self, Self::Err> {
        let target = target.as_ref();
        Ok(Self {
            xml: target,
            tree: XmlSpanTree::parse(target)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use super::*;

    #[test]
    fn xml_doc() {
        let target = String::from(r#"<root><h1><a big="true">1</a><b>2</b><c>3</c></h1></root>"#);
        let len = target.len();
        let doc = XmlSpanDoc::try_from_owned(target).unwrap();
        for i in 0..len {
            println!(
                "[{i}] {}",
                doc.get_path(i)
                    .iter()
                    .map(|el| el.name_span().map(|s| doc.get_str(s)).unwrap_or("Text"))
                    .join(" > ")
            );
        }
    }
}
