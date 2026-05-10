/*!
EPUB CFI (Canonical Fragment Identifier) sub-path for an XML document.

# Spec reference
EPUB CFI 1.0: https://idpf.org/epub/linking/cfi/epub-cfi.html

## Relevant rules

### Step direction
CFI counts child nodes using **all node types** (element, text, PI, comment),
assigning even numbers to element nodes and odd numbers to text/PI/comment nodes.
The count is 1-based and starts at 2 for the first element child.

Concretely, for a sequence of children:
```text
<p>         → step 2  (first element)
"hello "    → step 3  (text node after element)
&amp;       → still part of the same text node
<br/>       → step 4  (second element)
" world"    → step 5  (text node after element)
</p>
```

> **Key insight**: whitespace-only text nodes between tags *do* count.
> Be careful here: many XML documents have indentation whitespace that counts
> as text nodes for CFI purposes.

### Character offsets
When pointing into a text node, append `:N` where N is the 0-based character
offset (matching the browser Range API). Entities count as 1 character.

### Assertions (`:~...~`)
We do NOT generate assertions in this implementation. Assertions are optional
and used for robustness; they can be added later.

### Spine step
The full CFI includes a spine step (`/6/4[chap01]!`) before the in-document
path; we only generate the in-document sub-path here.

## Example
```text
<html>
  <body>
    <p>hello &amp; world</p>
  </body>
</html>
```
Byte index pointing at `w` in `world`:
  CFI: `/2/2/2:9`
  Meaning: root-element(2) → body(2) → p(2) → text, char-offset 9
  (h-e-l-l-o-' '-&amp;-' '-w → 9th char (0-based))
*/

use crate::xml::parts::{FullSpan, element::ElementSpan, text::TextSpan};

// ─── CfiStep ─────────────────────────────────────────────────────────────────

/// A single CFI step integer.
///
/// Even = element, odd = text/non-element node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CfiStep(pub usize);

impl std::fmt::Display for CfiStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "/{}", self.0)
    }
}

// ─── EpubCfi ─────────────────────────────────────────────────────────────────

/// EPUB CFI sub-path for a location within an XML document.
///
/// Does not include the spine step or the `epubcfi(...)` wrapper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EpubCfi {
    /// Steps from the root element downward.
    pub steps: Vec<CfiStep>,
    /// 0-based character offset into a text node, or `None` for element targets.
    pub char_offset: Option<usize>,
}

impl EpubCfi {
    /// Build an EPUB CFI from the path returned by [`XmlDoc::path_at`].
    pub fn from_path(src: &str, path: &[&ElementSpan], byte_idx: usize) -> Self {
        let mut steps = Vec::with_capacity(path.len());
        let mut char_offset = None;

        for (depth, &node) in path.iter().enumerate() {
            let siblings: &[ElementSpan] = if depth == 0 {
                &[] // root has no siblings
            } else {
                path[depth - 1].children()
            };

            let step = cfi_step_for(siblings, node);
            steps.push(step);

            if let ElementSpan::Text(text) = node {
                char_offset = Some(char_offset_in_text(src, text, byte_idx));
            }
        }

        Self { steps, char_offset }
    }
}

impl std::fmt::Display for EpubCfi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for step in &self.steps {
            write!(f, "{step}")?;
        }
        if let Some(offset) = self.char_offset {
            write!(f, ":{offset}")?;
        }
        Ok(())
    }
}

// ─── CfiRange ─────────────────────────────────────────────────────────────────

/// A range between two EPUB CFI locations within the same document.
///
/// CFI ranges are defined in the EPUB CFI spec as a shared path prefix up to
/// the point where the two endpoints diverge, followed by separate step
/// suffixes for each endpoint.
///
/// # Example
/// For a range that starts at `/2/4:3` and ends at `/2/4:10` (same text node,
/// different character offsets):
///   - `shared_steps`: `[/2, /4]`
///   - `start_suffix`: `[]` with `start_char_offset: Some(3)`
///   - `end_suffix`:   `[]` with `end_char_offset: Some(10)`
///
/// For a range spanning two different elements:
///   - `shared_steps`: `[/2]`       (common ancestor)
///   - `start_suffix`: `[/2, /1]`   (path from common ancestor to start)
///   - `end_suffix`:   `[/4, /1]`   (path from common ancestor to end)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CfiRange {
    /// Steps shared by both endpoints (the longest common prefix).
    pub shared_steps: Vec<CfiStep>,
    /// Steps specific to the start endpoint, after the shared prefix diverges.
    pub start_suffix: Vec<CfiStep>,
    /// 0-based character offset for the start position (if in a text node).
    pub start_char_offset: Option<usize>,
    /// Steps specific to the end endpoint, after the shared prefix diverges.
    pub end_suffix: Vec<CfiStep>,
    /// 0-based character offset for the end position (if in a text node).
    pub end_char_offset: Option<usize>,
}

impl CfiRange {
    /// Build a `CfiRange` from two byte indices using a **single tree traversal**.
    ///
    /// ## Algorithm
    /// 1. Collect the `start` path and `end` path from the tree (each `O(depth)`).
    /// 2. Find the longest common prefix of the two step sequences — that is
    ///    the shared path.
    /// 3. The remaining suffixes are the unique tails for each endpoint.
    ///
    /// Because we call `path_at` twice rather than walking the tree once, the
    /// total cost is `O(2 * depth)` = `O(depth)`.  A single-pass alternative
    /// would require a more complex DFS that tracks both positions simultaneously;
    /// the savings would be marginal for typical document depths (< 50).
    pub fn from_byte_range(
        src: &str,
        root: &ElementSpan,
        start_byte: usize,
        end_byte: usize,
    ) -> Option<Self> {
        // Collect ancestor chains for both endpoints.
        let start_path = push_path_at(root, start_byte)?;
        let end_path = push_path_at(root, end_byte)?;

        // Build the step sequences from each path.
        let start_steps = path_to_cfi_steps(src, &start_path, start_byte);
        let end_steps = path_to_cfi_steps(src, &end_path, end_byte);

        // Find the length of the longest common prefix of step *values*.
        let shared_len = start_steps
            .steps
            .iter()
            .zip(end_steps.steps.iter())
            .take_while(|(a, b)| a == b)
            .count();

        Some(CfiRange {
            shared_steps: start_steps.steps[..shared_len].to_vec(),
            start_suffix: start_steps.steps[shared_len..].to_vec(),
            start_char_offset: start_steps.char_offset,
            end_suffix: end_steps.steps[shared_len..].to_vec(),
            end_char_offset: end_steps.char_offset,
        })
    }
}

impl std::fmt::Display for CfiRange {
    /// Formats as `shared,start-suffix:offset,end-suffix:offset`.
    ///
    /// Example: `/2/4,/2:3,/6:10`
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Shared prefix.
        for step in &self.shared_steps {
            write!(f, "{step}")?;
        }
        write!(f, ",")?;
        // Start suffix.
        for step in &self.start_suffix {
            write!(f, "{step}")?;
        }
        if let Some(off) = self.start_char_offset {
            write!(f, ":{off}")?;
        }
        write!(f, ",")?;
        // End suffix.
        for step in &self.end_suffix {
            write!(f, "{step}")?;
        }
        if let Some(off) = self.end_char_offset {
            write!(f, ":{off}")?;
        }
        Ok(())
    }
}

// ─── Internal helpers ────────────────────────────────────────────────────────

/// Walk `root` to collect the ancestor chain containing `byte_idx`.
fn push_path_at<'a>(root: &'a ElementSpan, byte_idx: usize) -> Option<Vec<&'a ElementSpan>> {
    if !root.is_in(byte_idx) {
        return None;
    }
    let mut path = Vec::new();
    root.push_path(byte_idx, &mut path);
    Some(path)
}

/// Convert an ancestor path into an `EpubCfi` (steps + optional char offset).
fn path_to_cfi_steps(src: &str, path: &[&ElementSpan], byte_idx: usize) -> EpubCfi {
    EpubCfi::from_path(src, path, byte_idx)
}

/// Compute the CFI step number for `target` within its `siblings` list.
///
/// CFI step numbering:
///   - Start counting at 2 for the first element child (even).
///   - Text/non-element nodes get odd numbers.
///   - The count increases by 1 for each node (regardless of type), then
///     the step is even for elements and odd for text.
///
/// Formally: step = 2 * (1-based-position-among-all-children) for elements,
/// and 2 * (position-of-preceding-element) + 1 for text nodes.
///
/// A simpler equivalent that matches the spec:
///   For each child at 0-based index `i`:
///     - element: step = (i + 1) * 2
///     - text:    step = i * 2 + 1   (odd, between two elements)
///
/// Wait — the spec is slightly more nuanced.  The child-step number is:
///   step = 2 * (count of element children up to and including this child)
///   for an element child, OR
///   step = 2 * (count of element children before this node) + 1
///   for a non-element child.
///
/// This implementation counts element nodes before/at the target.
pub(crate) fn cfi_step_for(siblings: &[ElementSpan], target: &ElementSpan) -> CfiStep {
    if siblings.is_empty() {
        // Root element: always step 2.
        return CfiStep(2);
    }

    let target_span = target.full_span();
    let mut element_count = 0usize;

    for sib in siblings {
        if sib.is_element() {
            element_count += 1;
        }
        if sib.full_span() == target_span {
            // Found our target.
            if target.is_element() {
                return CfiStep(element_count * 2);
            } else {
                // Text node: odd = 2 * preceding-elements + 1.
                return CfiStep(element_count * 2 + 1);
            }
        }
    }

    panic!("cfi_step_for: target not found among siblings");
}

/// 0-based logical character offset of byte `byte_idx` within a text node.
/// (Same logic as in xpath/mod.rs — consider sharing via a utility fn.)
pub(crate) fn char_offset_in_text(src: &str, text: &TextSpan, byte_idx: usize) -> usize {
    use crate::xml::parts::text::TextChunk;

    let mut offset = 0usize;
    for chunk in &text.chunks {
        let span = chunk.full_span();
        if byte_idx >= span.end {
            offset += chunk.logical_char_len(src);
        } else if byte_idx >= span.start {
            match chunk {
                TextChunk::Raw(raw_span) => {
                    let sub = &src[raw_span.start..byte_idx];
                    offset += sub.chars().count();
                }
                TextChunk::Entity(_) => {
                    // Pointing into an entity: offset is at the entity start.
                }
            }
            break;
        } else {
            break;
        }
    }
    offset
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::doc::XmlDoc;

    fn cfi(src: &str, byte_idx: usize) -> EpubCfi {
        XmlDoc::parse(src).unwrap().cfi_at(byte_idx).unwrap()
    }

    #[test]
    fn root_only() {
        // <root/> — pointing at root element itself.
        let src = "<root/>";
        let c = cfi(src, 0);
        assert_eq!(c.steps, vec![CfiStep(2)]);
        assert_eq!(c.char_offset, None);
        assert_eq!(c.to_string(), "/2");
    }

    #[test]
    fn single_text_child() {
        let src = "<p>hello</p>";
        let idx = src.find('h').unwrap();
        let c = cfi(src, idx);
        // <p> is step /2 from root, text is step /1 (first child, odd)
        // — but wait, p IS the root here. So path is [p, text].
        // p → step 2 (root element)
        // text inside p → step 1 (only child, and it's a text node: 0 elements before it → 0*2+1=1)
        assert_eq!(c.to_string(), "/2/1:0"); // 'h' is first char, offset 0
    }

    #[test]
    fn element_children_step_numbers() {
        // <root><a/><b/></root>
        // children: <a/> (element, count=1 → step 2), <b/> (element, count=2 → step 4)
        let src = "<root><a/><b/></root>";
        let a_idx = src.find("<a").unwrap() + 1;
        let b_idx = src.find("<b").unwrap() + 1;
        let ca = cfi(src, a_idx);
        let cb = cfi(src, b_idx);
        assert_eq!(ca.to_string(), "/2/2");
        assert_eq!(cb.to_string(), "/2/4");
    }

    #[test]
    fn text_between_elements_step() {
        // <root><a/>text<b/></root>
        // children: a(step 2), text(step 3, odd, 1 element before), b(step 4)
        let src = "<root><a/>text<b/></root>";
        let t_idx = src.find("text").unwrap();
        let c = cfi(src, t_idx);
        assert_eq!(c.to_string(), "/2/3:0");
    }

    #[test]
    fn entity_char_offset() {
        let src = "<p>a &amp; b</p>";
        let b_idx = src.rfind('b').unwrap();
        let c = cfi(src, b_idx);
        // "a " = 2 chars, "&amp;" = 1, " " = 1 → 'b' is offset 4
        assert_eq!(c.char_offset, Some(4));
    }

    #[test]
    fn cfi_range_same_text_node() {
        // Range within a single text node: offsets diverge, steps are shared.
        let src = "<p>hello world</p>";
        let doc = XmlDoc::parse(src).unwrap();
        let start = src.find('h').unwrap();
        let end = src.find('w').unwrap();
        let range = doc.cfi_range(start, end).unwrap();
        // Both endpoints are in the same text node → same steps, different offsets.
        assert!(range.start_suffix.is_empty());
        assert!(range.end_suffix.is_empty());
        assert_eq!(range.start_char_offset, Some(0));
        assert_eq!(range.end_char_offset, Some(6));
    }

    #[test]
    fn cfi_range_different_nodes() {
        // Range spanning two sibling elements.
        let src = "<root><a>x</a><b>y</b></root>";
        let doc = XmlDoc::parse(src).unwrap();
        let start = src.find('x').unwrap();
        let end = src.find('y').unwrap();
        let range = doc.cfi_range(start, end).unwrap();
        // shared prefix is just the root step /2; then they diverge.
        assert_eq!(range.shared_steps.len(), 1);
        assert!(!range.start_suffix.is_empty());
        assert!(!range.end_suffix.is_empty());
    }

    #[test]
    fn cfi_range_display() {
        let src = "<p>hello world</p>";
        let doc = XmlDoc::parse(src).unwrap();
        let start = src.find('h').unwrap();
        let end = src.find('w').unwrap();
        let range = doc.cfi_range(start, end).unwrap();
        let s = range.to_string();
        // Should contain commas separating shared,start,end
        assert_eq!(s.matches(',').count(), 2);
    }
}
