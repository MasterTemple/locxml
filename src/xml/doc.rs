use crate::{
    loc::{cfi::EpubCfi, xpath::XPathLocation},
    utils::ref_owner::TryRefTarget,
    xml::{parts::element::ElementSpan, tree::XmlSpanTree},
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
pub struct XmlDoc<'src> {
    /// The raw XML source.
    pub src: &'src str,
    /// Span-only parse tree.
    pub tree: XmlSpanTree,
}

impl<'src> XmlDoc<'src> {
    pub fn parse(src: &'src str) -> Result<Self, ()> {
        let tree = XmlSpanTree::parse(src)?;
        Ok(Self { src, tree })
    }

    // ── Convenience slicing ──────────────────────────────────────────────────

    /// Slice the source at an arbitrary byte range.
    pub fn slice(&self, span: impl Into<std::ops::Range<usize>>) -> &str {
        &self.src[span.into()]
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

    // ── EPUB CFI ─────────────────────────────────────────────────────────────

    /// Compute the EPUB CFI sub-path for the node containing byte `idx`.
    ///
    /// Returns `None` if `idx` is out of the document range.
    pub fn cfi_at(&self, idx: usize) -> Option<EpubCfi> {
        let path = self.path_at(idx)?;
        Some(EpubCfi::from_path(self.src, &path, idx))
    }

    // ── Line/column ──────────────────────────────────────────────────────────

    /// Compute the line/column pair for byte `idx`.
    ///
    /// Lines and columns are both **1-indexed** (matching most editors).
    /// A `\n` byte is the last byte of its line.
    pub fn line_col_at(&self, idx: usize) -> Option<(usize, usize)> {
        if idx >= self.src.len() {
            return None;
        }
        let before = &self.src[..idx];
        let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
        let col = before.rfind('\n').map(|n| idx - n).unwrap_or(idx + 1);
        Some((line, col))
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
        let idx = SRC.find('1').unwrap(); // inside <a>1</a>
        let path = doc.path_at(idx).unwrap();
        // root → h1 → a → text
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
        assert_eq!(doc.line_col_at(4), Some((2, 1)));
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
