//! Thread-local `tree_sitter::Parser` pool for Rayon extract / CFG workers.
//!
//! Avoids `Parser::new()` + grammar setup per file (and per function in legacy
//! complexity paths). One parser map is kept per OS thread.

use std::cell::RefCell;
use std::collections::HashMap;
use tree_sitter::{Language, Parser, Tree};

thread_local! {
    static PARSERS: RefCell<HashMap<String, Parser>> = RefCell::new(HashMap::new());
}

/// Borrow the thread-local parser for `lang_id`, set `grammar`, and run `f`.
///
/// `lang_id` should be stable (e.g. `"rust"`, `"python"`) so consecutive files
/// of the same language skip redundant `set_language` when possible — we still
/// call `set_language` each time because the grammar object may differ across
/// versions; tree-sitter makes that cheap when unchanged.
pub fn with_thread_local_parser<R>(
    lang_id: &str,
    grammar: Language,
    f: impl FnOnce(&mut Parser) -> R,
) -> R {
    PARSERS.with(|cell| {
        let mut map = cell.borrow_mut();
        let parser = map.entry(lang_id.to_string()).or_insert_with(Parser::new);
        // Ignore set_language errors here; callers that need Result use parse_with_pool.
        let _ = parser.set_language(&grammar);
        f(parser)
    })
}

/// Parse `source` using the thread-local pool.
pub fn parse_with_pool(
    lang_id: &str,
    grammar: Language,
    source: &[u8],
) -> Result<Tree, String> {
    with_thread_local_parser(lang_id, grammar.clone(), |parser| {
        parser
            .set_language(&grammar)
            .map_err(|e| format!("set_language({lang_id}): {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| format!("parse failed for language `{lang_id}`"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_reuses_parser_same_thread() {
        let grammar: Language = tree_sitter_rust::LANGUAGE.into();
        let a = parse_with_pool("rust", grammar.clone(), b"fn main() {}").unwrap();
        let b = parse_with_pool("rust", grammar, b"fn foo() {}").unwrap();
        assert_eq!(a.root_node().kind(), "source_file");
        assert_eq!(b.root_node().kind(), "source_file");
    }
}
