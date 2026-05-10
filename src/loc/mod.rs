/*!
Location types for mapping between coordinate systems within an XML document.

All systems ultimately convert through a [`byte::ByteIndex`].

| System     | Range type         | Description                                                    |
|------------|--------------------|----------------------------------------------------------------|
| `byte`     | [`byte::ByteRange`]   | Raw byte offset / range (canonical)                         |
| `line`     | [`line::LineColRange`]| 1-based line/column range (from byte index + newline table) |
| `xpath`    | [`xpath::XPathRange`] | Element steps + character offset (matches browser Range API)|
| `cfi`      | [`cfi::CfiRange`]     | EPUB CFI shared path + per-endpoint suffixes                |

## Conversion flow

```text
byte index  →  XmlDoc::path_at()  →  ancestor Vec<&ElementSpan>
                                         ↓
                               XPathLocation::from_path()
                               EpubCfi::from_path()
```

Line/column is derived directly from the byte index via a precomputed
newline-offset table (`loc::line::build_newline_table`).
*/

pub mod byte;
pub mod cfi;
pub mod line;
pub mod xpath;
