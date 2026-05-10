use chumsky::prelude::*;

use crate::xml::parts::{
    comment::CommentSpan, declaration::XmlDeclSpan, element::ElementSpan, instruction::PiSpan,
};

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
///
/// # Document prologue
/// A real XML document may start with:
///   1. An optional XML declaration  (`<?xml version="1.0"?>`)
///   2. Zero or more comments / PIs  (`<!-- ... -->`, `<?foo bar?>`)
///   3. An optional DOCTYPE declaration (not yet supported; stripped as raw bytes)
///   4. The root element
///
/// We parse and record the declaration (for informational purposes) and discard
/// comments/PIs before and after the root element so that the root index is
/// always 0 for CFI/XPath step-numbering purposes.
///
/// # Missing features (investigated per task 3)
/// - **DOCTYPE / DTD**: `<!DOCTYPE ...>` declarations are not yet parsed.  They
///   appear before the root element and are currently treated as a parse error.
///   Add a `DoctypeSpan` parser and consume it in the prologue noise to fix.
/// - **Namespace support**: prefix resolution (`xmlns:foo="..."`) is not
///   implemented.  The `name` slice includes any prefix (e.g. `"svg:g"`).
///   XPath step matching compares raw names, so `svg:g[1]` would need to be
///   compared against expanded Clark-notation names for correct ns-aware XPath.
/// - **XML 1.1**: The character ranges for valid name characters differ from
///   XML 1.0.  The current `TagNameSpan` disallowed-char approach approximates
///   XML 1.0 and may mis-accept or mis-reject some edge-case XML 1.1 names.
/// - **PI/comment nodes in CFI/XPath**: PIs and comments are currently discarded
///   rather than modelled as odd-step CFI nodes.  Spec-compliant CFI for
///   documents with PIs/comments would require keeping these in the tree.
/// - **Attribute value normalization**: Attribute values are stored raw (no
///   whitespace normalization, no entity decoding).
/// - **Encoding declaration**: The `encoding` pseudo-attribute in the XML
///   declaration is recorded but not acted upon.  The parser always operates
///   on `&str` (UTF-8); non-UTF-8 content must be transcoded by the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlSpanTree {
    pub root: ElementSpan,
    /// The XML declaration, if the document had one.
    pub declaration: Option<XmlDeclSpan>,
}

impl XmlSpanTree {
    pub fn parse(input: &str) -> Result<Self, ()> {
        // ── Full document parser ──────────────────────────────────────────
        // Structure: [decl?] [ws | comment | PI]* root [ws | comment | PI]*
        //
        // We skip (but record) the optional XML declaration, then discard any
        // interleaved whitespace, comments, and PIs before and after the root.

        // Optional XML declaration must come first.
        let decl = XmlDeclSpan::parser().or_not();

        // "Noise" between the declaration and the root element: whitespace,
        // comments, and PIs.  DOCTYPE is not yet supported; if present the
        // parse will fail with a clear error from `.map_err(|_| ())`.
        let noise = {
            let comment = CommentSpan::parser().ignored();
            // Note: PiSpan::parser() currently accepts `<?xml ...?>` because
            // it cannot do a case-insensitive target check without `src`.
            // In practice this is fine: if the input has a valid `<?xml?>`,
            // the `decl` branch above will have consumed it already, so the
            // `pi` branch in noise only fires for non-xml targets.
            let pi = PiSpan::parser().ignored();
            comment.or(pi).padded().repeated()
        };

        let root_parser = decl
            .padded()
            .then_ignore(noise.clone())
            .then(ElementSpan::parser().padded())
            .then_ignore(noise);

        let result = root_parser.parse(input).into_result().map_err(|_| ())?;

        let (declaration, root) = result;

        Ok(Self { root, declaration })
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

    #[test]
    fn with_xml_declaration() {
        let src = r#"<?xml version="1.0" encoding="UTF-8"?><root/>"#;
        let tree = XmlSpanTree::parse(src).expect("parse failed");
        assert!(tree.declaration.is_some());
        let decl = tree.declaration.unwrap();
        assert_eq!(decl.version.slice(src), "1.0");
        assert_eq!(decl.encoding.unwrap().slice(src), "UTF-8");
    }

    #[test]
    fn with_declaration_and_whitespace() {
        let src = "<?xml version=\"1.0\"?>\n\n<root/>";
        let tree = XmlSpanTree::parse(src).expect("parse failed");
        assert!(tree.declaration.is_some());
        assert!(tree.root.is_unit());
    }

    #[test]
    fn with_comment_before_root() {
        let src = "<!-- preamble comment --><root/>";
        let tree = XmlSpanTree::parse(src).expect("parse failed");
        assert!(tree.declaration.is_none());
        assert!(tree.root.is_unit());
    }

    #[test]
    fn with_pi_before_root() {
        let src = "<?stylesheet href='a.css'?><root/>";
        let tree = XmlSpanTree::parse(src).expect("parse failed");
        assert!(tree.root.is_unit());
    }

    #[test]
    fn full_prolog() {
        let src = "<?xml version=\"1.0\"?>\n<!-- comment -->\n<?pi data?>\n<root/>";
        let tree = XmlSpanTree::parse(src).expect("parse failed");
        assert!(tree.declaration.is_some());
        assert!(tree.root.is_unit());
    }

    #[test]
    fn comment_skipped_inside_element() {
        // Comments inside the document body should not produce child nodes.
        let src = "<root><!-- skip --><a/></root>";
        let tree = XmlSpanTree::parse(src).expect("parse failed");
        assert_eq!(tree.root.children().len(), 1);
    }
}
