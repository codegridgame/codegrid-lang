/// A half-open byte range in the original UTF-8 source.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn empty(at: usize) -> Self {
        Self { start: at, end: at }
    }
}

/// Indexes physical source lines while keeping all columns in UTF-8 bytes.
#[derive(Clone, Debug)]
pub struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(source: &str) -> Self {
        let mut line_starts = vec![0];
        let bytes = source.as_bytes();
        let mut offset = 0;
        while offset < bytes.len() {
            match bytes[offset] {
                b'\r' if bytes.get(offset + 1) == Some(&b'\n') => {
                    offset += 2;
                    line_starts.push(offset);
                }
                b'\r' | b'\n' => {
                    offset += 1;
                    line_starts.push(offset);
                }
                _ => offset += 1,
            }
        }
        Self { line_starts }
    }

    /// Returns a zero-based line and byte column for a source byte offset.
    pub fn line_and_byte_column(&self, source: &str, byte_offset: usize) -> (usize, usize) {
        let mut offset = byte_offset.min(source.len());
        while !source.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self
            .line_starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line].min(source.len());
        let line_end = self.line_end(source, line).unwrap_or(source.len());
        (line, offset.min(line_end) - line_start)
    }

    /// Returns the byte offset at which a zero-based line starts.
    pub fn line_start(&self, line: usize) -> Option<usize> {
        self.line_starts.get(line).copied()
    }

    /// Returns the byte offset immediately before a line ending.
    pub fn line_end(&self, source: &str, line: usize) -> Option<usize> {
        let line_start = self.line_starts.get(line).copied()?.min(source.len());
        let line_end = self
            .line_starts
            .get(line + 1)
            .map(|next_start| {
                let next_start = (*next_start).min(source.len());
                match next_start
                    .checked_sub(1)
                    .and_then(|last| source.as_bytes().get(last).copied())
                {
                    Some(b'\n')
                        if next_start >= 2
                            && source.as_bytes().get(next_start - 2) == Some(&b'\r') =>
                    {
                        next_start - 2
                    }
                    Some(b'\n' | b'\r') => next_start - 1,
                    _ => next_start,
                }
            })
            .unwrap_or(source.len())
            .max(line_start);
        Some(line_end)
    }
}

#[cfg(test)]
mod tests {
    use super::LineIndex;

    #[test]
    fn maps_utf8_byte_offsets_across_crlf_without_counting_newline_bytes() {
        let source = "a🙂\r\nb";
        let index = LineIndex::new(source);

        assert_eq!(index.line_and_byte_column(source, 1), (0, 1));
        assert_eq!(index.line_and_byte_column(source, 5), (0, 5));
        assert_eq!(index.line_and_byte_column(source, 6), (0, 5));
        assert_eq!(index.line_and_byte_column(source, 7), (1, 0));
        assert_eq!(index.line_and_byte_column(source, 8), (1, 1));
    }

    #[test]
    fn rounds_an_offset_inside_a_utf8_code_point_down_to_a_boundary() {
        let source = "a🙂b";
        let index = LineIndex::new(source);

        assert_eq!(index.line_and_byte_column(source, 2), (0, 1));
    }

    #[test]
    fn maps_offsets_after_an_invalid_bare_carriage_return_to_the_next_line() {
        let source = "a\rb";
        let index = LineIndex::new(source);

        assert_eq!(index.line_and_byte_column(source, 2), (1, 0));
    }
}
