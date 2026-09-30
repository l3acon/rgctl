//! Shared helpers for span-accurate config key extraction.

use rgctl_plugin_api::SourceLocation;

/// Map a 0-based byte offset into 1-indexed line/column using UTF-8 line starts.
pub fn line_col_at(source: &str, byte_offset: usize) -> (usize, usize) {
    let offset = byte_offset.min(source.len());
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, b) in source.bytes().enumerate() {
        if i >= offset {
            break;
        }
        if b == b'\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

pub fn loc(file: &str, start_line: usize, end_line: usize, start_col: usize, end_col: usize) -> SourceLocation {
    SourceLocation {
        file: file.to_string(),
        start_line: start_line.max(1),
        end_line: end_line.max(start_line.max(1)),
        start_column: start_col.max(1),
        end_column: end_col.max(start_col.max(1)),
    }
}

/// Find the first unused occurrence of `"leaf"` in JSON/text for span approximation.
pub fn find_quoted_key_span(
    source: &str,
    leaf: &str,
    used: &mut Vec<usize>,
) -> Option<(usize, usize, usize, usize)> {
    let needle = format!("\"{leaf}\"");
    let mut search_from = 0usize;
    while let Some(rel) = source[search_from..].find(&needle) {
        let abs = search_from + rel;
        if used.contains(&abs) {
            search_from = abs + needle.len();
            continue;
        }
        used.push(abs);
        let (sl, sc) = line_col_at(source, abs);
        let (el, ec) = line_col_at(source, abs + needle.len());
        return Some((sl, el, sc, ec));
    }
    None
}
