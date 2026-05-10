use chumsky::prelude::*;

use crate::xml::parts::{
    FullSpan, NameSpan, Span,
    parent::{ClosingTagSpan, OpeningTagSpan, ParentSpan},
    text::TextSpan,
    unit::UnitSpan,
};

// ─── ElementSpan ─────────────────────────────────────────────────────────────

/// A single XML node.
///
/// The three variants correspond to what you can legally encounter as a child
/// inside an element's content model in well-formed XML (ignoring PI/comments/CDATA
/// which are added later):
///
/// ```text
/// <parent>          ← ParentSpan
///   <unit/>         ← UnitSpan
///   text &amp; more ← TextSpan (contains chunks of raw text + EntitySpan)
/// </parent>
/// ```
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
            let parent = OpeningTagSpan::parser()
                .then(element.repeated().collect::<Vec<_>>())
                .then(ClosingTagSpan::parser())
                .map(|((open, children), close)| {
                    ElementSpan::Parent(ParentSpan::new(open, children, close))
                });

            // TODO: Why did Claude say this? It shouldn't matter since a parent open tag will not
            // parse a unit tag?

            // Self-closing must be tried before parent, because `<foo/>` has no
            // closing tag and we need to commit at `/>` rather than backtracking
            // through the children parser.
            let unit = UnitSpan::parser().map(ElementSpan::Unit);

            let text = TextSpan::parser().map(ElementSpan::Text);

            // Order matters: try parent first (greedily consumes children),
            // then self-closing, then bare text.
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
    fn get_path_into_nested() {
        let src = r#"<root><h1><a big="true">1</a><b>2</b></h1></root>"#;
        let el = parse(src);

        // src.find('1') would return byte 8, which is the '1' inside "<h1>" — not what we want.
        // The text content "1" inside <a>...</a> sits at byte 24 (after the closing '>').
        // Use rfind on a unique byte-string to be explicit.
        let idx = src.find(">1<").unwrap() + 1; // byte of the '1' between '>' and '<'
        assert_eq!(&src[idx..idx + 1], "1");

        let mut path = vec![];
        el.push_path(idx, &mut path);
        let names: Vec<_> = path.iter().filter_map(|e| e.name(src)).collect();
        // root → h1 → a  (the text node itself has no name, so it doesn't appear in `names`)
        assert_eq!(names, vec!["root", "h1", "a"]);
        // The deepest entry is the text node
        assert!(path.last().unwrap().is_text());
    }

    #[test]
    fn path_stops_at_opening_tag() {
        let src = r#"<root><h1><a big="true">1</a></h1></root>"#;
        let el = parse(src);
        // Byte 8 is the '1' inside "<h1>", i.e. inside the opening tag's bytes — not a child.
        let idx = 8;
        assert_eq!(&src[idx..idx + 1], "1");
        let mut path = vec![];
        el.push_path(idx, &mut path);
        let names: Vec<_> = path.iter().filter_map(|e| e.name(src)).collect();
        // Stops at h1 because idx is inside h1's own open-tag bytes.
        assert_eq!(names, vec!["root", "h1"]);
    }
}
