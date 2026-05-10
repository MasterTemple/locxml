/*!
Location types for mapping between coordinate systems within an XML document.

All systems ultimately convert through a [`byte::ByteIndex`].

| System     | Description                                                    |
|------------|----------------------------------------------------------------|
| `byte`     | Raw byte offset into the source string (canonical)             |
| `line`     | 1-based line/column pair (trivial from byte index + newline table) |
| `xpath`    | Element steps + character offset (matches browser Range API)   |
| `cfi`      | EPUB CFI integer steps + character offset                      |
*/

pub mod byte;
pub mod cfi;
pub mod line;
pub mod xpath;
