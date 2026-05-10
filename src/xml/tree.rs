use crate::xml::parts::element::ElementSpan;
use chumsky::prelude::*;
use from_nested_tuple::FromTuple;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, FromTuple)]
pub struct XmlSpanTree {
    pub root: ElementSpan,
}

impl XmlSpanTree {
    // pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> {
    //     ElementSpan::parser()
    // }
    pub fn parse<'a>(input: &'a str) -> Result<Self, ()> {
        let root = ElementSpan::parser()
            .parse(input)
            .into_result()
            .map_err(|_| ())?;
        Ok(Self { root })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t1() {
        let input = &vec!["<root>", "</root>"].join("\n");
        let input = r#"<root></root>"#;
        assert!(dbg!(XmlSpanTree::parse(input)).is_ok());
    }
}
