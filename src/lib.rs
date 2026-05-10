/*!
`locxml` — XML span-tree with coordinate-system conversions.

# Architecture

```text
XmlDoc
  ├── src: &str           — original source (never modified)
  └── tree: XmlSpanTree
        └── root: ElementSpan
              ├── Parent(ParentSpan)   — <tag>…</tag>
              │     ├── OpeningTagSpan
              │     ├── Vec<ElementSpan>  (children)
              │     └── ClosingTagSpan
              ├── Unit(UnitSpan)       — <tag/>
              └── Text(TextSpan)       — text + entity chunks
```

# Coordinate conversion flow

```text
byte index
   ↓  push_path()
ancestor Vec<&ElementSpan>
   ↓  XPathLocation::from_path  /  EpubCfi::from_path
XPath / CFI location
```

Line/column is derived directly from the byte index via a precomputed
newline-offset table (`loc::line::build_newline_table`).
*/

pub mod loc;
pub mod utils;
pub mod xml;
