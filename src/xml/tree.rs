use chumsky::prelude::*;

use crate::xml::parts::element::ElementSpan;

// ─── XmlSpanTree ─────────────────────────────────────────────────────────────

/// The parsed span-tree for an XML document.
///
/// This is the *cheap* representation: it stores only byte ranges, not decoded
/// strings.  String data (names, attribute values) is recovered on-demand by
/// slicing `source`.
///
/// # Root-element constraint
/// Well-formed XML has exactly one root element.  We enforce this by storing
/// `root: ElementSpan` rather than `Vec<ElementSpan>`.  If you need to handle
/// XML fragments (multiple roots), wrap them in a synthetic root before parsing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlSpanTree {
    pub root: ElementSpan,
}

impl XmlSpanTree {
    pub fn parse(input: &str) -> Result<Self, ()> {
        // We allow optional leading/trailing whitespace around the root element.
        // This handles the common case of `<?xml ...?>\n<root>...</root>\n`.
        // TODO: strip XML declaration and DTD before parsing.
        let root = ElementSpan::parser()
            .padded()
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
    fn minimal() {
        assert!(XmlSpanTree::parse("<root></root>").is_ok());
    }

    #[test]
    fn nested() {
        let src = r#"<root><h1><a big="true">1</a><b>2</b><c>3</c></h1></root>"#;
        assert!(XmlSpanTree::parse(src).is_ok());
    }

    #[test]
    fn self_closing_child() {
        assert!(XmlSpanTree::parse("<root><br/></root>").is_ok());
    }

    #[test]
    fn whitespace_between_tags() {
        assert!(XmlSpanTree::parse("<root>\n  <child/>\n</root>").is_ok());
    }
}
