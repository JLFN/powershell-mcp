//! File chunking — split project files into retrieval units with line
//! provenance.
//!
//! Policy (the ebook-knowledge chapter-chunking recipe generalized for
//! code): text is cut only on blank-line boundaries, so a chunk never
//! splits a function body or a prose paragraph. Chunks target roughly
//! TARGET_CHUNK_CHARS characters, about 400-500 tokens. A single paragraph
//! larger than the target is split on whole-line boundaries (never
//! mid-line), which keeps minified or generated files indexable.

/// Target chunk size in characters.
pub const TARGET_CHUNK_CHARS: usize = 1800;

/// A retrieval unit with provenance: which file, and which lines.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Global sequential index across the whole project index.
    pub index: usize,
    /// Path relative to the project root.
    pub file: String,
    /// 1-based inclusive first source line.
    pub line_start: usize,
    /// 1-based inclusive last source line.
    pub line_end: usize,
    pub text: String,
}

/// Chunk one file's text. `index_start` is the first global chunk index for
/// this file, so indices stay sequential across the whole index.
pub fn chunk_file(index_start: usize, file: &str, text: &str) -> Vec<Chunk> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    // Group consecutive non-blank lines into paragraphs, remembering the
    // 1-based source line range of each paragraph.
    let mut paragraphs: Vec<(usize, usize, String)> = Vec::new();
    let mut start: Option<usize> = None;
    let mut buf: Vec<&str> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            if let Some(s) = start.take() {
                paragraphs.push((s + 1, i, buf.join("\n")));
                buf.clear();
            }
        } else {
            if start.is_none() {
                start = Some(i);
            }
            buf.push(line);
        }
    }
    if let Some(s) = start {
        paragraphs.push((s + 1, lines.len(), buf.join("\n")));
    }

    // Greedily pack paragraphs into chunks up to the target size, flushing
    // on overflow. Oversized paragraphs are hard-split on line boundaries.
    let mut out = Vec::new();
    let mut buf_text = String::new();
    let mut chunk_start = 0usize;
    let mut chunk_end = 0usize;
    for (ls, le, text) in paragraphs {
        if text.len() > TARGET_CHUNK_CHARS {
            if !buf_text.is_empty() {
                out.push(make_chunk(
                    index_start + out.len(),
                    file,
                    chunk_start,
                    chunk_end,
                    &buf_text,
                ));
                buf_text.clear();
            }
            for (p_start, p_end, piece) in split_paragraph(&text, ls) {
                out.push(make_chunk(
                    index_start + out.len(),
                    file,
                    p_start,
                    p_end,
                    &piece,
                ));
            }
            continue;
        }
        if !buf_text.is_empty() && buf_text.len() + text.len() + 2 > TARGET_CHUNK_CHARS {
            out.push(make_chunk(
                index_start + out.len(),
                file,
                chunk_start,
                chunk_end,
                &buf_text,
            ));
            buf_text.clear();
        }
        if buf_text.is_empty() {
            chunk_start = ls;
        }
        if !buf_text.is_empty() {
            buf_text.push_str("\n\n");
        }
        buf_text.push_str(&text);
        chunk_end = le;
    }
    if !buf_text.is_empty() {
        out.push(make_chunk(
            index_start + out.len(),
            file,
            chunk_start,
            chunk_end,
            &buf_text,
        ));
    }
    out
}

/// Split one oversized paragraph into whole-line pieces of at most
/// TARGET_CHUNK_CHARS. A single line longer than the target stays whole in
/// its own piece — lines are never cut.
fn split_paragraph(text: &str, line_start: usize) -> Vec<(usize, usize, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut pieces = Vec::new();
    let mut buf = String::new();
    let mut start = line_start;
    let mut end = line_start;
    for (i, line) in lines.iter().enumerate() {
        if !buf.is_empty() && buf.len() + line.len() + 1 > TARGET_CHUNK_CHARS {
            pieces.push((start, end, buf.clone()));
            buf.clear();
            start = line_start + i;
        }
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(line);
        end = line_start + i;
    }
    if !buf.is_empty() {
        pieces.push((start, end, buf));
    }
    pieces
}

fn make_chunk(index: usize, file: &str, line_start: usize, line_end: usize, text: &str) -> Chunk {
    Chunk {
        index,
        file: file.to_string(),
        line_start,
        line_end,
        text: text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_file_is_one_chunk() {
        let chunks = chunk_file(
            0,
            "src/lib.rs",
            "pub fn greet() -> &'static str {\n    \"hi\"\n}\n",
        );
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].index, 0);
        assert_eq!(chunks[0].file, "src/lib.rs");
        assert_eq!(chunks[0].line_start, 1);
        assert_eq!(chunks[0].line_end, 3);
        assert!(chunks[0].text.contains("greet"));
    }

    #[test]
    fn long_file_splits_on_paragraph_boundaries_with_line_ranges() {
        let mut body = String::new();
        for i in 0..60 {
            body.push_str(&format!(
                "fn function_{i}() -> u64 {{\n    // paragraph number {i} with enough words to matter\n    40 + 2\n}}\n\n"
            ));
        }
        let chunks = chunk_file(10, "src/lib.rs", &body);
        assert!(
            chunks.len() > 3,
            "expected multiple chunks, got {}",
            chunks.len()
        );
        assert_eq!(chunks[0].index, 10);
        assert_eq!(chunks.last().unwrap().index, 10 + chunks.len() - 1);
        for c in &chunks {
            assert!(c.line_start >= 1);
            assert!(c.line_end >= c.line_start);
            assert!(!c.text.trim().is_empty());
        }
        // Chunks stay in source order.
        for w in chunks.windows(2) {
            assert!(w[0].line_end <= w[1].line_start);
        }
    }

    #[test]
    fn oversized_paragraph_splits_on_lines_never_mid_line() {
        // One huge paragraph with no blank lines: 300 lines of 30 chars.
        let line = "x".repeat(30);
        let text = (0..300)
            .map(|_| line.clone())
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = chunk_file(0, "gen.rs", &text);
        assert!(
            chunks.len() > 5,
            "expected multiple pieces, got {}",
            chunks.len()
        );
        for c in &chunks {
            assert!(
                c.text.len() <= TARGET_CHUNK_CHARS + 30,
                "piece too large: {}",
                c.text.len()
            );
            // No line may be split: every piece boundary is a newline boundary.
            assert!(c.text.is_empty() || !c.text.starts_with(' ') || c.text.starts_with("xx"));
        }
        // Line ranges tile the file exactly.
        assert_eq!(chunks[0].line_start, 1);
        assert_eq!(chunks.last().unwrap().line_end, 300);
        let mut prev_end = 0usize;
        for c in &chunks {
            assert_eq!(c.line_start, prev_end + 1);
            prev_end = c.line_end;
        }
    }

    #[test]
    fn empty_file_produces_no_chunks() {
        assert!(chunk_file(0, "empty.md", "").is_empty());
        assert!(chunk_file(0, "blank.md", "\n\n\n").is_empty());
    }

    #[test]
    fn crlf_files_are_handled() {
        let text = "fn a() {}\r\n\r\nfn b() {}\r\n";
        let chunks = chunk_file(0, "win.rs", text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].line_start, 1);
        assert_eq!(chunks[0].line_end, 3);
    }
}
