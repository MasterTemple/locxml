use crate::{
    loc::{
        byte::ByteRange,
        cfi::{CfiRange, EpubCfi},
        line::{LineCol, LineColRange, build_newline_table},
        xpath::{XPathLocation, XPathRange},
    },
    utils::ref_owner::TryRefTarget,
    xml::{
        parts::{FullSpan, element::ElementSpan},
        tree::XmlSpanTree,
    },
};

// ─── XmlDoc ──────────────────────────────────────────────────────────────────

/// A parsed XML document with its source string and span tree.
///
/// `XmlDoc` is the primary entry point for all coordinate-conversion operations.
/// It owns both the parse tree (spans) and a reference to the source string;
/// together these are sufficient to derive any of the four location types.
///
/// # Lifetime note
/// The `'src` lifetime ties the doc to the underlying string slice.  If you
/// need an owned version (e.g. to store in a struct), use [`XmlDoc::from_owned`]
/// via the [`TryRefTarget`] impl.
///
/// # Newline table
/// A sorted table of `\n` byte offsets is built lazily on first use of any
/// line/column method and reused for subsequent calls.  This makes the first
/// `line_col_at` call O(bytes) and all subsequent ones O(log lines).
pub struct XmlDoc<'src> {
    /// The raw XML source.
    pub src: &'src str,
    /// Span-only parse tree.
    pub tree: XmlSpanTree,
    /// Lazily-initialized sorted newline offset table.
    ///
    /// Built once on first call to `line_col_at` / `line_col_range`.
    /// `Option::None` means "not yet built".
    newline_table: std::cell::OnceCell<Vec<usize>>,
}

impl<'src> XmlDoc<'src> {
    pub fn parse(src: &'src str) -> Result<Self, ()> {
        let tree = XmlSpanTree::parse(src)?;
        Ok(Self {
            src,
            tree,
            newline_table: std::cell::OnceCell::new(),
        })
    }

    // ── Convenience slicing ──────────────────────────────────────────────────

    /// Slice the source at an arbitrary byte range.
    pub fn slice(&self, span: impl Into<std::ops::Range<usize>>) -> &str {
        &self.src[span.into()]
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    /// Return (or lazily build) the newline offset table for this document.
    fn newlines(&self) -> &[usize] {
        self.newline_table
            .get_or_init(|| build_newline_table(self.src))
    }

    // ── Byte-index lookups ───────────────────────────────────────────────────

    /// Return the ancestor chain (root → deepest) for the node that contains
    /// byte `idx`, or `None` if `idx` is out of range.
    pub fn path_at(&self, idx: usize) -> Option<Vec<&ElementSpan>> {
        if !self.tree.root.is_in(idx) {
            return None;
        }
        let mut path = Vec::new();
        self.tree.root.push_path(idx, &mut path);
        Some(path)
    }

    /// Return the deepest node that contains byte `idx`.
    pub fn element_at(&self, idx: usize) -> Option<&ElementSpan> {
        if self.tree.root.is_in(idx) {
            Some(self.tree.root.get_element(idx))
        } else {
            None
        }
    }

    // ── XPath ────────────────────────────────────────────────────────────────

    /// Compute the XPath location for the node containing byte `idx`.
    ///
    /// Returns `None` if `idx` is out of the document range.
    pub fn xpath_at(&self, idx: usize) -> Option<XPathLocation> {
        let path = self.path_at(idx)?;
        Some(XPathLocation::from_path(self.src, &path, idx))
    }

    /// Compute an [`XPathRange`] for the two byte indices `[start, end)`.
    ///
    /// This corresponds to the browser's `Range` object.  The common ancestor
    /// of both endpoints can be retrieved via [`XPathRange::common_ancestor_path`].
    ///
    /// Returns `None` if either index is outside the document.
    pub fn xpath_range(&self, start: usize, end: usize) -> Option<XPathRange> {
        XPathRange::from_byte_range(self.src, &self.tree.root, start, end)
    }

    // ── EPUB CFI ─────────────────────────────────────────────────────────────

    /// Compute the EPUB CFI sub-path for the node containing byte `idx`.
    ///
    /// Returns `None` if `idx` is out of the document range.
    pub fn cfi_at(&self, idx: usize) -> Option<EpubCfi> {
        let path = self.path_at(idx)?;
        Some(EpubCfi::from_path(self.src, &path, idx))
    }

    /// Compute a [`CfiRange`] for the two byte indices `[start, end)`.
    ///
    /// The range is formatted as `shared,start-suffix:offset,end-suffix:offset`
    /// per the EPUB CFI spec §3.1.
    ///
    /// Returns `None` if either index is outside the document.
    pub fn cfi_range(&self, start: usize, end: usize) -> Option<CfiRange> {
        CfiRange::from_byte_range(self.src, &self.tree.root, start, end)
    }

    // ── Line/column ──────────────────────────────────────────────────────────

    /// Compute the line/column pair for byte `idx`.
    ///
    /// Lines and columns are both **1-indexed** (matching most editors).
    /// A `\n` byte is the last byte of its line.
    ///
    /// Uses a lazily-built newline table for O(log lines) lookup after the
    /// first call.
    pub fn line_col_at(&self, idx: usize) -> Option<LineCol> {
        if idx >= self.src.len() {
            return None;
        }
        Some(LineCol::from_byte_index(idx, self.newlines()))
    }

    /// Compute a [`LineColRange`] for the byte range `[start, end)`.
    ///
    /// Returns `None` if either index is out of the document.
    pub fn line_col_range(&self, start: usize, end: usize) -> Option<LineColRange> {
        if start >= self.src.len() || end > self.src.len() {
            return None;
        }
        Some(LineColRange::from_byte_indices(start, end, self.newlines()))
    }

    // ── Byte range ───────────────────────────────────────────────────────────

    /// Return the [`ByteRange`] of the deepest node containing byte `idx`.
    ///
    /// This is the most direct way to obtain the source extent of a node.
    pub fn byte_range_at(&self, idx: usize) -> Option<ByteRange> {
        let el = self.element_at(idx)?;
        let span = el.full_span();
        Some(ByteRange::new(span.start, span.end))
    }
}

// ─── TryRefTarget impl (owned-string API) ─────────────────────────────────────

impl<'src> TryRefTarget<'src, str> for XmlDoc<'src> {
    type Err = ();

    fn try_from_ref(target: &'src str) -> Result<Self, Self::Err> {
        Self::parse(target)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use super::*;

    const SRC: &str = r#"<root><h1><a big="true">1</a><b>2</b><c>3</c></h1></root>"#;

    fn doc() -> XmlDoc<'static> {
        XmlDoc::parse(SRC).expect("parse failed")
    }

    #[test]
    fn path_at_root_tag() {
        let doc = doc();
        // Byte 0 is inside `<root>` — the root open tag.
        let path = doc.path_at(0).unwrap();
        assert_eq!(path.len(), 1);
        assert_eq!(path[0].name(SRC), Some("root"));
    }

    #[test]
    fn path_at_text() {
        let doc = doc();
        // src.find('1') hits byte 8 — the '1' in the tag name "<h1>", not text content.
        // Find the text "1" that sits between '>' and '<' to get the actual content byte.
        let idx = SRC.find(">1<").unwrap() + 1;
        assert_eq!(&SRC[idx..idx + 1], "1");

        let path = doc.path_at(idx).unwrap();
        // root → h1 → a → (text node, no name)
        let names: Vec<_> = path.iter().filter_map(|e| e.name(SRC)).collect();
        assert_eq!(names, vec!["root", "h1", "a"]);
        assert!(path.last().unwrap().is_text());
    }

    #[test]
    fn out_of_range() {
        let doc = doc();
        assert!(doc.path_at(SRC.len()).is_none());
    }

    #[test]
    fn line_col() {
        let doc = XmlDoc::parse("<a>\n<b/>\n</a>").unwrap();
        // `<b` starts at byte 4 (line 2, col 1)
        assert_eq!(
            doc.line_col_at(4),
            Some(crate::loc::line::LineCol { line: 2, col: 1 })
        );
    }

    #[test]
    fn line_col_range_test() {
        let src = "<a>\n<b/>\n</a>";
        let doc = XmlDoc::parse(src).unwrap();
        // Byte 0..4 spans line 1, col 1 → line 1, col 4 (the '\n').
        let range = doc.line_col_range(0, 3).unwrap();
        assert_eq!(range.start, crate::loc::line::LineCol { line: 1, col: 1 });
        assert_eq!(range.end, crate::loc::line::LineCol { line: 1, col: 4 });
    }

    #[test]
    fn byte_range_at_text() {
        let src = "<p>hello</p>";
        let doc = XmlDoc::parse(src).unwrap();
        let idx = src.find('h').unwrap();
        let range = doc.byte_range_at(idx).unwrap();
        // "hello" spans bytes 3..8.
        assert_eq!(range.start.0, 3);
        assert_eq!(range.end.0, 8);
    }

    #[test]
    fn xpath_range_test() {
        let src = "<p>hello world</p>";
        let doc = XmlDoc::parse(src).unwrap();
        let start = src.find('h').unwrap();
        let end = src.find('w').unwrap();
        let range = doc.xpath_range(start, end).unwrap();
        // Same text node → same steps, different char offsets.
        assert_eq!(range.start.steps, range.end.steps);
        assert_eq!(range.start.char_offset, Some(0));
        assert_eq!(range.end.char_offset, Some(6));
    }

    #[test]
    fn cfi_range_test() {
        let src = "<p>hello world</p>";
        let doc = XmlDoc::parse(src).unwrap();
        let start = src.find('h').unwrap();
        let end = src.find('w').unwrap();
        let range = doc.cfi_range(start, end).unwrap();
        assert!(range.start_suffix.is_empty());
        assert_eq!(range.start_char_offset, Some(0));
        assert_eq!(range.end_char_offset, Some(6));
    }

    #[test]
    fn with_xml_declaration() {
        let src = r#"<?xml version="1.0"?><root><p>hi</p></root>"#;
        let doc = XmlDoc::parse(src).unwrap();
        // Declaration must not shift node positions.
        let idx = src.find("hi").unwrap();
        let loc = doc.xpath_at(idx).unwrap();
        // Should see root[1]/p[1]/text()[1], not something shifted by the decl.
        let s = loc.to_string();
        assert!(s.contains("root[1]"), "xpath: {s}");
        assert!(s.contains("p[1]"), "xpath: {s}");
    }

    #[test]
    fn print_all_paths() {
        // Smoke test: walk every byte and print its path.
        let doc = doc();
        for i in 0..SRC.len() {
            if let Some(path) = doc.path_at(i) {
                let _ = path.iter().filter_map(|e| e.name(SRC)).join(" > ");
            }
        }
    }
}
