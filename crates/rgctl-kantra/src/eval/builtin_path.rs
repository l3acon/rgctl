//! `builtin.xml` XPath (minimal) and `builtin.json` path checks via on-demand reparse.

use crate::eval::filecontent::SourceCache;
use crate::eval::{MatchSite, violation};
use crate::findings::KantraViolation;
use std::path::Path;

/// Very small XPath subset: `//tag`, `//tag[@attr='val']`, `/root/...`.
pub fn eval_builtin_xml(
    rule_id: &str,
    xpath: &str,
    file_pattern: Option<&str>,
    repo_root: &Path,
    files: &[std::path::PathBuf],
    sources: &SourceCache,
) -> Vec<KantraViolation> {
    let mut out = Vec::new();
    for path in files {
        let rel = path
            .strip_prefix(repo_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if !rel.ends_with(".xml") {
            continue;
        }
        if let Some(pat) = file_pattern
            && !rel.contains(pat.trim_matches('*'))
            && !glob_match(pat, &rel)
        {
            continue;
        }
        let text = if let Some(s) = sources.get(&rel) {
            s.as_str().to_string()
        } else if let Ok(bytes) = std::fs::read(path) {
            String::from_utf8_lossy(&bytes).into_owned()
        } else {
            continue;
        };
        let Ok(doc) = roxmltree::Document::parse(&text) else {
            continue;
        };
        if xpath_matches(&doc, xpath) {
            out.push(violation(
                rule_id,
                "builtin.xml",
                &MatchSite::new(rel, 1),
            ));
        }
    }
    out
}

/// JSONPath-ish: `$.a.b` exact object path presence.
pub fn eval_builtin_json(
    rule_id: &str,
    jsonpath: &str,
    file_pattern: Option<&str>,
    repo_root: &Path,
    files: &[std::path::PathBuf],
    sources: &SourceCache,
) -> Vec<KantraViolation> {
    let mut out = Vec::new();
    for path in files {
        let rel = path
            .strip_prefix(repo_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if !rel.ends_with(".json") {
            continue;
        }
        if let Some(pat) = file_pattern
            && !glob_match(pat, &rel)
            && !rel.contains(pat.trim_matches('*'))
        {
            continue;
        }
        let text = if let Some(s) = sources.get(&rel) {
            s.as_str().to_string()
        } else if let Ok(bytes) = std::fs::read(path) {
            String::from_utf8_lossy(&bytes).into_owned()
        } else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if json_path_exists(&v, jsonpath) {
            out.push(violation(
                rule_id,
                "builtin.json",
                &MatchSite::new(rel, 1),
            ));
        }
    }
    out
}

fn glob_match(pat: &str, path: &str) -> bool {
    if let Ok(g) = glob::Pattern::new(pat) {
        return g.matches(path);
    }
    path.contains(pat.trim_matches('*'))
}

fn xpath_matches(doc: &roxmltree::Document<'_>, xpath: &str) -> bool {
    let xpath = xpath.trim();
    // `//tag`
    if let Some(tag) = xpath.strip_prefix("//") {
        let tag = tag.split('[').next().unwrap_or(tag).trim();
        if tag.is_empty() {
            return false;
        }
        return doc.descendants().any(|n| n.is_element() && n.tag_name().name() == tag);
    }
    false
}

fn json_path_exists(v: &serde_json::Value, path: &str) -> bool {
    let path = path.trim().trim_start_matches('$').trim_start_matches('.');
    if path.is_empty() {
        return true;
    }
    let mut cur = v;
    for part in path.split('.') {
        match cur {
            serde_json::Value::Object(map) => {
                if let Some(next) = map.get(part) {
                    cur = next;
                } else {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}
