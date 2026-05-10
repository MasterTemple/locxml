use chumsky::prelude::*;

use crate::xml::parts::{
    FullSpan, NameSpan, Span,
    cdata::CdataSpan,
    comment::CommentSpan,
    instruction::PiSpan,
    parent::{ClosingTagSpan, OpeningTagSpan, ParentSpan},
    text::TextSpan,
    unit::UnitSpan,
};

// ─── ElementSpan ─────────────────────────────────────────────────────────────

/// A single XML node.
///
/// The three *content* variants correspond to what you can legally encounter
/// as a child inside an element's content model in well-formed XML:
///
/// ```text
/// <parent>              ← ParentSpan
///   <unit/>             ← UnitSpan
///   text &amp; more     ← TextSpan (chunks of raw text + EntitySpan)
///   <!-- comment -->    ← (consumed and discarded — not stored)
///   <?pi data?>         ← (consumed and discarded — not stored)
///   <![CDATA[...]]>     ← folded into a TextSpan
/// </parent>
/// ```
///
/// ## Comments and PIs
/// XML comments (`<!-- ... -->`) and processing instructions (`<?target data?>`)
/// are **parsed and consumed** during tree building but are NOT represented as
/// children in `ElementSpan::Parent`.  This means they:
///   - Do not occupy CFI steps.
///   - Do not appear as XPath nodes.
///
/// This matches the behaviour of the browser Range API and simplifies CFI /
/// XPath arithmetic for the EPUB use-case.  If you need PI/comment nodes,
/// add a `Comment(CommentSpan)` / `Pi(PiSpan)` variant here and update the
/// step-counting logic in `cfi` and `xpath`.
///
/// ## CDATA sections
/// CDATA content is collapsed into a `TextSpan` containing a single `Raw`
/// chunk that covers the decoded character content.  This is correct for all
/// character-offset calculations because CDATA is semantically plain text.
///
/// # Design: no separate wrapper
/// We store all traversal logic directly on `ElementSpan` rather than
/// scattering it across each variant, so callers only have to import one type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ElementSpan {
    Parent(ParentSpan),
    Unit(UnitSpan),
    Text(TextSpan),
}

impl ElementSpan {
    // ── Parser ──────────────────────────────────────────────────────────────

    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self> + Clone {
        recursive(|element| {
            // ── Noise: comments and PIs are consumed but produce no node ──
            // We parse them as `Option<ElementSpan>` (→ None) so we can mix
            // them with real child parsers in a single `repeated()`.
            let comment_none = CommentSpan::parser().map(|_| None);
            let pi_none = PiSpan::parser().map(|_| None);

            // ── CDATA: fold into TextSpan ──────────────────────────────────
            // A CDATA section is semantically text.  We convert it to a
            // `TextSpan` with a single `Raw` chunk so all traversal logic
            // is unchanged.
            let cdata_as_text = CdataSpan::parser().map(|cdata| {
                use crate::xml::parts::text::TextChunk;
                Some(ElementSpan::Text(TextSpan {
                    // TODO: Should this store the full span, including the delimiting tags?
                    span: cdata.full_span(),
                    chunks: vec![TextChunk::Raw(cdata.content)],
                }))
            });

            // ── Real children ──────────────────────────────────────────────
            let element_some = element.clone().map(Some);
            let text_some = TextSpan::parser().map(|t| Some(ElementSpan::Text(t)));

            // A "child slot" is one of: comment (→ None), PI (→ None),
            // CDATA (→ Some(Text)), element (→ Some(...)), or bare text (→ Some(Text)).
            //
            // `choice` tries each alternative left-to-right with backtracking.
            // Order matters: comments/PIs before elements (both start with `<`),
            // and elements before bare text (bare text must not consume `<`).
            let child_slot = choice((
                comment_none,
                pi_none,
                cdata_as_text,
                element_some,
                text_some,
            ));

            // Collect all children, filtering out the `None` noise entries.
            let children = child_slot
                .repeated()
                .collect::<Vec<_>>()
                .map(|v| v.into_iter().flatten().collect::<Vec<_>>());

            // ── Parent element ─────────────────────────────────────────────
            let parent = OpeningTagSpan::parser()
                .then(children)
                .then(ClosingTagSpan::parser())
                .map(|((open, children), close)| {
                    ElementSpan::Parent(ParentSpan::new(open, children, close))
                });

            // Self-closing element (no children).
            let unit = UnitSpan::parser().map(ElementSpan::Unit);

            // Bare text node (no `<`, no `&` sequences that look like tags).
            // Must come LAST so that `<tag>` is not consumed as text.
            let text = TextSpan::parser().map(ElementSpan::Text);

            // Ordering: parent (greedy via recursive children), then unit
            // (self-closing), then bare text.  Parent and unit both start with
            // `<name`; chumsky will backtrack from a failed parent parse and
            // try unit.  Unit commits at `/>`, which parent never emits.
            parent.or(unit).or(text)
        })
    }

    // ── Containment ─────────────────────────────────────────────────────────

    /// `true` if `idx` falls within the node's *own* span (open/close tags for
    /// a `Parent`, the full span for `Unit` and `Text`).
    pub fn is_at(&self, idx: usize) -> bool {
        match self {
            ElementSpan::Parent(p) => p.is_at(idx),
            // Unit and Text are always atomic; any idx within them "is at" them.
            ElementSpan::Unit(_) | ElementSpan::Text(_) => self.full_span().contains(idx),
        }
    }

    /// `true` if `idx` falls anywhere inside the full byte range of this node
    /// (including children for `Parent`).
    pub fn is_in(&self, idx: usize) -> bool {
        self.full_span().contains(idx)
    }

    // ── Traversal ───────────────────────────────────────────────────────────

    /// Return the deepest node whose span contains `idx`.
    ///
    /// Precondition: `self.is_in(idx)` — verified with a debug-assert.
    pub fn get_element(&self, idx: usize) -> &ElementSpan {
        debug_assert!(
            self.is_in(idx),
            "get_element called with out-of-range idx {idx}"
        );
        match self {
            ElementSpan::Parent(p) => {
                if p.is_at(idx) {
                    // `idx` is inside the open/close tag itself, not a child.
                    self
                } else {
                    p.get_element(idx)
                }
            }
            ElementSpan::Unit(_) | ElementSpan::Text(_) => self,
        }
    }

    /// Append the ancestor chain from this node down to the deepest node
    /// containing `idx`, in top-down order, to `path`.
    ///
    /// Precondition: `self.is_in(idx)`.
    pub fn push_path<'a>(&'a self, idx: usize, path: &mut Vec<&'a ElementSpan>) {
        path.push(self);
        if let ElementSpan::Parent(p) = self {
            if !p.is_at(idx) {
                p.push_path(idx, path);
            }
        }
    }

    // ── Name ────────────────────────────────────────────────────────────────

    /// The span of the tag name, or `None` for text nodes.
    pub fn name_span(&self) -> Option<Span> {
        match self {
            ElementSpan::Parent(e) => Some(e.name_span()),
            ElementSpan::Unit(e) => Some(e.name_span()),
            ElementSpan::Text(_) => None,
        }
    }

    /// Borrow the tag name as a `&str` from the original source, or `None` for text.
    pub fn name<'a>(&self, src: &'a str) -> Option<&'a str> {
        self.name_span().map(|s| s.slice(src))
    }

    // ── Children ────────────────────────────────────────────────────────────

    /// Returns the direct children of this node, or an empty slice if it is a
    /// leaf node (Unit or Text).
    pub fn children(&self) -> &[ElementSpan] {
        match self {
            ElementSpan::Parent(p) => &p.children,
            ElementSpan::Unit(_) | ElementSpan::Text(_) => &[],
        }
    }

    /// Whether this node can contain child elements.
    pub fn is_parent(&self) -> bool {
        matches!(self, ElementSpan::Parent(_))
    }

    pub fn is_text(&self) -> bool {
        matches!(self, ElementSpan::Text(_))
    }

    pub fn is_unit(&self) -> bool {
        matches!(self, ElementSpan::Unit(_))
    }

    pub fn is_element(&self) -> bool {
        matches!(self, ElementSpan::Parent(_) | ElementSpan::Unit(_))
    }
}

impl FullSpan for ElementSpan {
    fn full_span(&self) -> Span {
        match self {
            ElementSpan::Parent(e) => e.full_span(),
            ElementSpan::Unit(e) => e.full_span(),
            ElementSpan::Text(e) => e.full_span(),
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> ElementSpan {
        ElementSpan::parser()
            .parse(src)
            .into_result()
            .expect("parse failed")
    }

    #[test]
    fn simple_parent() {
        let src = "<root></root>";
        let el = parse(src);
        assert!(el.is_parent());
        assert_eq!(el.name(src), Some("root"));
        assert!(el.is_in(0));
        assert!(el.is_in(src.len() - 1));
    }

    #[test]
    fn self_closing() {
        let src = "<br/>";
        let el = parse(src);
        assert!(el.is_unit());
        assert_eq!(el.name(src), Some("br"));
    }

    #[test]
    fn nested_with_text() {
        let src = "<p>hello &amp; world</p>";
        let el = parse(src);
        assert!(el.is_parent());
        let children = el.children();
        assert_eq!(children.len(), 1);
        assert!(children[0].is_text());
    }

    #[test]
    fn comment_skipped_in_children() {
        // The comment must be consumed but must NOT appear as a child node.
        let src = "<root><!-- skip me --><a/></root>";
        let el = parse(src);
        assert!(el.is_parent());
        let children = el.children();
        assert_eq!(children.len(), 1, "comment should not produce a child node");
        assert!(children[0].is_unit());
    }

    #[test]
    fn pi_skipped_in_children() {
        let src = "<root><?myapp data?><a/></root>";
        let el = parse(src);
        let children = el.children();
        assert_eq!(children.len(), 1, "PI should not produce a child node");
        assert!(children[0].is_unit());
    }

    #[test]
    fn cdata_becomes_text_child() {
        // CDATA sections are folded into a TextSpan.
        let src = "<root><![CDATA[hello world]]></root>";
        let el = parse(src);
        let children = el.children();
        assert_eq!(children.len(), 1);
        assert!(children[0].is_text(), "CDATA should appear as a text child");
    }

    #[test]
    fn mixed_content_with_noise() {
        // Comment between elements: only real elements count.
        let src = "<root><a/><!-- noise --><b/></root>";
        let el = parse(src);
        let children = el.children();
        assert_eq!(children.len(), 2);
        assert!(children[0].is_unit());
        assert!(children[1].is_unit());
    }

    #[test]
    fn get_path_into_nested() {
        let src = r#"<root><h1><a big="true">1</a><b>2</b></h1></root>"#;
        let el = parse(src);

        let idx = src.find(">1<").unwrap() + 1;
        assert_eq!(&src[idx..idx + 1], "1");

        let mut path = vec![];
        el.push_path(idx, &mut path);
        let names: Vec<_> = path.iter().filter_map(|e| e.name(src)).collect();
        assert_eq!(names, vec!["root", "h1", "a"]);
        assert!(path.last().unwrap().is_text());
    }

    #[test]
    fn path_stops_at_opening_tag() {
        let src = r#"<root><h1><a big="true">1</a></h1></root>"#;
        let el = parse(src);
        let idx = 8;
        assert_eq!(&src[idx..idx + 1], "1");
        let mut path = vec![];
        el.push_path(idx, &mut path);
        let names: Vec<_> = path.iter().filter_map(|e| e.name(src)).collect();
        assert_eq!(names, vec!["root", "h1"]);
    }
}
