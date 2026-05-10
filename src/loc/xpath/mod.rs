/*!
XPath location for a byte index.

# XPath node addressing rules used here

We implement the subset of XPath needed for EPUB / browser Range compatibility:

- Element steps use `element-name[n]` where `n` is the 1-based position among
  siblings **of the same element type** (i.e. same local name, ignoring
  namespace prefixes for now).
- Text nodes use `text()[n]` where `n` is the 1-based position among sibling
  text nodes.
- A character offset within a text node is appended as `[offset]` on the
  final step, matching the semantics of `Range.startOffset` /
  `Range.endOffset` in the browser DOM API.

## Character-offset counting

The browser Range API counts:
  - Each Unicode scalar value in literal text as 1.
  - Each entity reference (`&amp;`, `&#160;`, etc.) as 1.
  - It does **not** count markup bytes.

This matches what [`TextSpan::logical_char_len`] computes.

## Example

```text
<root><h1><a big="true">hello &amp; world</a><b>2</b></h1></root>
```

Byte index pointing at `w` in `world`:
  → `/root[1]/h1[1]/a[1]/text()[1]` with character offset 9
    (`hello ` = 6 chars, `&amp;` = 1, ` ` = 1, `w` is the 9th char: offset 8 zero-based = 9 one-based? )

Actually the browser Range API is **0-based** for character offsets.  We store
the raw 0-based offset to match `Range.startOffset`.
*/

use crate::xml::parts::{FullSpan, NameSpan, element::ElementSpan, text::TextSpan};

// ─── XPathStep ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XPathStep {
    /// `tagname[n]`  — 1-based position among same-name siblings.
    Element { name: String, position: usize },
    /// `text()[n]`   — 1-based position among sibling text nodes.
    Text { position: usize },
}

impl std::fmt::Display for XPathStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XPathStep::Element { name, position } => write!(f, "{name}[{position}]"),
            XPathStep::Text { position } => write!(f, "text()[{position}]"),
        }
    }
}

// ─── XPathLocation ───────────────────────────────────────────────────────────

/// A fully-qualified XPath location, optionally with a character offset.
///
/// The `char_offset` is 0-based and matches `Range.startOffset` (i.e. the
/// number of logical characters before the target position within the node).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XPathLocation {
    /// Steps from the root, not including the root itself.
    pub steps: Vec<XPathStep>,
    /// 0-based character offset into a text node, or `None` if the target
    /// is an element node rather than a position within text.
    pub char_offset: Option<usize>,
}

impl XPathLocation {
    /// Build an XPath from the path returned by [`XmlDoc::path_at`].
    ///
    /// `path` runs root→deepest; the root element itself is included and its
    /// step is generated relative to a synthetic document root.
    pub fn from_path(src: &str, path: &[&ElementSpan], byte_idx: usize) -> Self {
        let mut steps = Vec::with_capacity(path.len());
        let mut char_offset = None;

        // Walk the path.  For each node we need its position among its siblings,
        // which means looking at the *parent's* children list.
        //
        // path[0] = root element (parent is the document root, position is always 1)
        // path[i] = child of path[i-1]

        for (depth, &node) in path.iter().enumerate() {
            // Collect the siblings (children of the parent at depth-1).
            let siblings: &[ElementSpan] = if depth == 0 {
                // The root element has no siblings in a well-formed document.
                &[]
            } else {
                path[depth - 1].children()
            };

            match node {
                ElementSpan::Parent(p) => {
                    let name = p.name_span().slice(src).to_owned();
                    let pos = sibling_element_position(src, siblings, &name, node);
                    steps.push(XPathStep::Element {
                        name,
                        position: pos,
                    });
                }
                ElementSpan::Unit(u) => {
                    let name = u.name_span().slice(src).to_owned();
                    let pos = sibling_element_position(src, siblings, &name, node);
                    steps.push(XPathStep::Element {
                        name,
                        position: pos,
                    });
                    // Self-closing has no text content; no character offset.
                }
                ElementSpan::Text(t) => {
                    let pos = sibling_text_position(siblings, node);
                    steps.push(XPathStep::Text { position: pos });
                    // Compute 0-based character offset of byte_idx inside this text node.
                    char_offset = Some(char_offset_in_text(src, t, byte_idx));
                }
            }
        }

        Self { steps, char_offset }
    }
}

impl std::fmt::Display for XPathLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "/")?;
        for (i, step) in self.steps.iter().enumerate() {
            if i > 0 {
                write!(f, "/")?;
            }
            write!(f, "{step}")?;
        }
        if let Some(offset) = self.char_offset {
            write!(f, ":{offset}")?;
        }
        Ok(())
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// 1-based position of `target` among siblings with the same element name.
fn sibling_element_position(
    src: &str,
    siblings: &[ElementSpan],
    name: &str,
    target: &ElementSpan,
) -> usize {
    if siblings.is_empty() {
        // Root element: position is always 1.
        return 1;
    }
    let target_span = target.full_span();
    let mut pos = 0;
    for sib in siblings {
        if sib.name(src).as_deref() == Some(name) {
            pos += 1;
            if sib.full_span() == target_span {
                return pos;
            }
        }
    }
    // Should be unreachable if the path was derived from the same tree.
    panic!("sibling_element_position: target not found among siblings");
}

/// 1-based position of `target` among sibling text nodes.
fn sibling_text_position(siblings: &[ElementSpan], target: &ElementSpan) -> usize {
    let target_span = target.full_span();
    let mut pos = 0;
    for sib in siblings {
        if sib.is_text() {
            pos += 1;
            if sib.full_span() == target_span {
                return pos;
            }
        }
    }
    panic!("sibling_text_position: target text node not found among siblings");
}

/// 0-based logical character offset of byte `byte_idx` within a text node.
///
/// We iterate through the chunks of the text node, accumulating logical chars
/// (raw chars count normally; entities count as 1) until we reach `byte_idx`.
fn char_offset_in_text(src: &str, text: &TextSpan, byte_idx: usize) -> usize {
    use crate::xml::parts::text::TextChunk;

    let mut offset = 0usize;
    for chunk in &text.chunks {
        let span = chunk.full_span();
        if byte_idx >= span.end {
            // The target byte is past this chunk; count the whole chunk.
            offset += chunk.logical_char_len(src);
        } else if byte_idx >= span.start {
            // The target byte is inside this chunk.
            match chunk {
                TextChunk::Raw(raw_span) => {
                    // Count chars up to (but not including) byte_idx.
                    let sub = &src[raw_span.start..byte_idx];
                    offset += sub.chars().count();
                }
                TextChunk::Entity(_) => {
                    // If we're pointing into an entity, the offset is at the
                    // entity's position (not past it).
                    // No-op: the entity itself has not been fully consumed.
                }
            }
            break;
        } else {
            // byte_idx is before this chunk; we're done.
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

    fn xpath(src: &str, byte_idx: usize) -> XPathLocation {
        XmlDoc::parse(src).unwrap().xpath_at(byte_idx).unwrap()
    }

    #[test]
    fn root_element() {
        let src = "<root></root>";
        let loc = xpath(src, 1); // inside `<root>`
        assert_eq!(
            loc.steps,
            vec![XPathStep::Element {
                name: "root".into(),
                position: 1
            }]
        );
        assert_eq!(loc.char_offset, None);
    }

    #[test]
    fn text_offset() {
        let src = "<p>hello world</p>";
        // Byte index of 'w' in "world"
        let idx = src.find('w').unwrap();
        let loc = xpath(src, idx);
        // Steps: p[1] / text()[1]
        assert_eq!(loc.steps.len(), 2);
        assert_eq!(loc.char_offset, Some(6)); // "hello " = 6 chars
    }

    #[test]
    fn entity_in_text() {
        let src = "<p>a &amp; b</p>";
        // Byte index of 'b' after the entity
        let idx = src.rfind('b').unwrap();
        let loc = xpath(src, idx);
        // "a " = 2, "&amp;" = 1, " " = 1 → 'b' is at offset 4
        assert_eq!(loc.char_offset, Some(4));
    }

    #[test]
    fn sibling_elements() {
        let src = "<root><a/><b/><a/></root>";
        // Byte idx inside the *second* <a/>
        let second_a = src.rfind("<a").unwrap();
        let loc = xpath(src, second_a + 1);
        // Should be a[2]
        assert_eq!(
            loc.steps.last().unwrap(),
            &XPathStep::Element {
                name: "a".into(),
                position: 2
            }
        );
    }

    #[test]
    fn display() {
        let src = "<root><p>hi</p></root>";
        let idx = src.find('h').unwrap();
        let loc = xpath(src, idx);
        let s = loc.to_string();
        assert!(s.contains("root[1]"));
        assert!(s.contains("p[1]"));
        assert!(s.contains("text()[1]"));
    }
}
