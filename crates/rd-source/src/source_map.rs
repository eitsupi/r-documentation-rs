use crate::{SourcePosition, SourceSpan};

pub(crate) struct SourceMap {
    newlines: Vec<usize>,
    input: String,
}
impl SourceMap {
    pub(crate) fn new(input: &str) -> Self {
        let mut newlines = Vec::new();
        let bytes = input.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\n' || (bytes[i] == b'\r' && bytes.get(i + 1) != Some(&b'\n')) {
                newlines.push(i);
            }
            i += 1;
        }
        Self {
            newlines,
            input: input.into(),
        }
    }
    pub(crate) fn span(&self, range: std::ops::Range<usize>) -> SourceSpan {
        SourceSpan::new(
            range.clone(),
            self.position(range.start),
            self.position(range.end),
        )
    }
    pub(crate) fn position(&self, offset: usize) -> SourcePosition {
        let line = self.newlines.partition_point(|n| *n < offset);
        let line_start = if line == 0 {
            0
        } else {
            self.newlines[line - 1] + 1
        };
        let end = offset.min(self.input.len());
        let column = self.input[line_start..end].chars().count() as u32 + 1;
        SourcePosition::new(line as u32 + 1, column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn positions_count_scalars_and_crlf_as_one_line() {
        let map = SourceMap::new("é\r\n日本語");
        assert_eq!(map.position(0), SourcePosition::new(1, 1));
        assert_eq!(map.position("é".len()), SourcePosition::new(1, 2));
        assert_eq!(map.position("é\r\n".len()), SourcePosition::new(2, 1));
        assert_eq!(map.position("é\r\n日本語".len()), SourcePosition::new(2, 4));
        assert_eq!(map.span(0..map.input.len()).bytes(), 0..map.input.len());
    }
}
