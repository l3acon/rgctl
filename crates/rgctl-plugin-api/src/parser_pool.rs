//! Thread-local `tree_sitter::Parser` pool for Rayon extract / CFG workers.
//!
//! Avoids `Parser::new()` + grammar setup per file (and per function in legacy
//! complexity paths). One parser map is kept per OS thread.

use std::cell::RefCell;
use std::collections::HashMap;
use tree_sitter::{Language, Parser, Tree};

struct PooledParser {
    parser: Parser,
    /// True after a successful `set_language` for this entry.
    language_set: bool,
}

thread_local! {
    static PARSERS: RefCell<HashMap<String, PooledParser>> = RefCell::new(HashMap::new());
}

/// Borrow the thread-local parser for `lang_id`, ensuring `grammar` is set once
/// on first use, and run `f`.
///
/// Looks up by `&str` before allocating a new `String` key. `set_language` runs
/// only when the entry is first created (or if a prior set failed).
pub fn with_thread_local_parser<R>(
    lang_id: &str,
    grammar: Language,
    f: impl FnOnce(&mut Parser) -> R,
) -> R {
    PARSERS.with(|cell| {
        let mut map = cell.borrow_mut();
        if !map.contains_key(lang_id) {
            let mut parser = Parser::new();
            let language_set = parser.set_language(&grammar).is_ok();
            map.insert(
                lang_id.to_string(),
                PooledParser {
                    parser,
                    language_set,
                },
            );
        }
        let entry = map.get_mut(lang_id).expect("parser entry just ensured");
        if !entry.language_set {
            entry.language_set = entry.parser.set_language(&grammar).is_ok();
        }
        f(&mut entry.parser)
    })
}

/// Parse `source` using the thread-local pool.
pub fn parse_with_pool(
    lang_id: &str,
    grammar: Language,
    source: &[u8],
) -> Result<Tree, String> {
    PARSERS.with(|cell| {
        let mut map = cell.borrow_mut();
        if !map.contains_key(lang_id) {
            let mut parser = Parser::new();
            parser
                .set_language(&grammar)
                .map_err(|e| format!("set_language({lang_id}): {e}"))?;
            map.insert(
                lang_id.to_string(),
                PooledParser {
                    parser,
                    language_set: true,
                },
            );
        }
        let entry = map.get_mut(lang_id).expect("parser entry just ensured");
        if !entry.language_set {
            entry
                .parser
                .set_language(&grammar)
                .map_err(|e| format!("set_language({lang_id}): {e}"))?;
            entry.language_set = true;
        }
        entry
            .parser
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
