/// A 1-based line/column pair.
///
/// Lines and columns are 1-indexed to match common editor conventions.
/// `\n` is the *last* byte of its line (so the byte immediately after `\n`
/// is on the next line).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LineCol {
    pub line: usize,
    pub col: usize,
}

impl LineCol {
    /// Convert a byte index into a `LineCol` using a precomputed newline table.
    ///
    /// `newline_offsets` must be a sorted list of byte indices at which `\n`
    /// occurs in the source.  Build it once with [`build_newline_table`].
    pub fn from_byte_index(byte_idx: usize, newline_offsets: &[usize]) -> Self {
        // Binary search: how many newlines occur *before* byte_idx?
        let line = newline_offsets.partition_point(|&nl| nl < byte_idx) + 1;
        let col = if line == 1 {
            byte_idx + 1
        } else {
            let prev_nl = newline_offsets[line - 2];
            byte_idx - prev_nl
        };
        LineCol { line, col }
    }
}

impl std::fmt::Display for LineCol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// Build a sorted table of byte offsets of every `\n` in `src`.
///
/// Allocate this once per document and reuse it for all `LineCol` conversions.
/// Binary search makes each lookup O(log lines) instead of O(bytes).
pub fn build_newline_table(src: &str) -> Vec<usize> {
    src.bytes()
        .enumerate()
        .filter_map(|(i, b)| if b == b'\n' { Some(i) } else { None })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line() {
        let src = "hello";
        let tbl = build_newline_table(src);
        assert_eq!(
            LineCol::from_byte_index(0, &tbl),
            LineCol { line: 1, col: 1 }
        );
        assert_eq!(
            LineCol::from_byte_index(4, &tbl),
            LineCol { line: 1, col: 5 }
        );
    }

    #[test]
    fn multi_line() {
        let src = "ab\ncd\nef";
        let tbl = build_newline_table(src);
        // 'c' is at byte 3, which is line 2 col 1
        assert_eq!(
            LineCol::from_byte_index(3, &tbl),
            LineCol { line: 2, col: 1 }
        );
        // 'e' is at byte 6, line 3 col 1
        assert_eq!(
            LineCol::from_byte_index(6, &tbl),
            LineCol { line: 3, col: 1 }
        );
    }

    #[test]
    fn newline_byte_itself() {
        let src = "ab\ncd";
        let tbl = build_newline_table(src);
        // `\n` at byte 2 is the last char of line 1
        assert_eq!(
            LineCol::from_byte_index(2, &tbl),
            LineCol { line: 1, col: 3 }
        );
    }
}
