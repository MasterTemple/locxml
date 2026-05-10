Rust + Chumsky
I am creating an XML parser that will let me effeciently traverse the tree in specific ways optimal for my use cases.
My goal is to convert between the following:
- EPUB CFI (at least the sub-path for this XML document)
- XPath (with character offsets where an entity is length 1; I am choosing this because that is how the Browser Range API calculates it),
- Line column pair of an XML file (trivial from byte index)
- Byte index (usize) of an XML file

The idea is that given one of the above locations, I will find the corresponding byte index range, and then convert it into the alternate location

I am at the stage where I have constructed an initial XML tree, but I want the tree to be optimal for this.

Here is what I was thinking:

- I just need to store the spans of each open/close tag pair, each self-closing tag, each entity (since entities only count as 1 character), and each text node.
- I also store some spans related to parsing later

For example: I store the span of the attributes in a tag (all together with 1 span)
- Note: This is without any surrounding padding: padding is managed by the parent who can calculate it from the difference of 2 spanned elements (or from fixed offsets that can never be padded)
I store the span because I don't need to create a map for most of the elements' attributes
However, when I want to, I can parse the slice that corresponds to the attributes, given the spans and the source
Since my parsers are the same, then I know that the parse will always succeed
See `./src/xml/parts/attributes.rs` for context


There is other data I need to parse beyond the spans, such as tag names.
I don't want to do this all upfront, only as needed
However, I don't want to redo/re-parse what has already been parsed

To do this effeciently, I think this would result in me creating 2 trees
This also means I am creating 2 structs: one that is just the spans, the other that is parsed

Alternatively, it could be 1 tree each storing something like this
```rust
pub enum ParseEntry<E: FromParsedSpan> {
    // Wrap in a box to keep enum size small
    Parsed(Box<E>),
    Unparsed(E::Span),
}
```

Either way, I still have the `FromParsedSpan` trait

Instructions:

1. Implement the tree (or trees) with lazy, cached full-parsing
2. Implement optimal traversals to get the data necessary for constructing the XPATH range (just use tag/element type and count) or EPUB CFI (no assertions needed, just the ints, with an optional character offsets)

Other Notes:

1. I am using a custom `Span` type because `SimpleSpan` doesn't have a nice constructor, and allegedly `std::ops::Range` holds an iterator, but please improve this as you see fit

2. I have not yet implemented processing instructions, the xml declaration, cdata, and comments, but I will do so later

3. Correct me where I am making mistakes or poor design choices.
Improve my code and make it more ergonomic.

4. Find common pitfalls and things I need to look out for in my project, and pre-emptively resolve them (add comments explaining your actions)

5. Here is my project directory

```
├── Cargo.toml
└── src
    ├── lib.rs
    ├── loc
    │   ├── byte/mod.rs
    │   ├── cfi/mod.rs
    │   ├── line/mod.rs
    │   ├── mod.rs
    │   └── xpath/mod.rs
    ├── utils
    │   ├── mod.rs
    │   └── ref_owner.rs
    └── xml
        ├── doc.rs
        ├── mod.rs
        ├── parts
        │   ├── attributes.rs
        │   ├── cdata.rs
        │   ├── comment.rs
        │   ├── declaration.rs
        │   ├── element.rs
        │   ├── entity.rs
        │   ├── instruction.rs
        │   ├── mod.rs
        │   ├── parent.rs
        │   ├── text.rs
        │   └── unit.rs
        └── tree.rs
```

---

```rust
impl<T: Into<Span>> FullSpan for T {
    fn full_span(&self) -> Span {
        self.into()
    }
}
```

---

Rust + Chumsky

I am creating an XML parser that will let me effeciently traverse the tree in specific ways optimal for my use cases.

My goal is to convert between the following:

- EPUB CFI (at least the sub-path for this XML document)
- XPath (with character offsets where an entity is length 1; I am choosing this because that is how the Browser Range API calculates it),
- Line column pair of an XML file (trivial from byte index)
- Byte index (usize) of an XML file

The idea is that given one of the above locations, I will find the corresponding byte index range, and then convert it into the alternate location

I have an XML tree that is mostly spans, only converting to another type with `FromParsedSpan` when necessary (and doing so lazily, because it is not important for my use case)

Instructions

1. Create structs to hold location ranges (start and end):
- an XPath range
- a line-column range
- a byte range
- a cfi range (also create a struct with shared steps, then the unique steps and char offset for when they branch off)

Create traversals that are optimized for finding the start and end in 1 pass
Create any other methods that would be helpful, practical, or ergonomic

2. Write the span parsers for
- CDATA
- comment
- declaration
- instruction

Also include them in the appropriate places for parsing the tree

3. Investigate and see what XML parsing features are missed

4. Correct all mistakes and add explanatory comments as appropriate

I have attached my Cargo.toml and the src directory as a zip
