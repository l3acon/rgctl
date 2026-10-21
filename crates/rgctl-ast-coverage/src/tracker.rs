//! Test harness: record which AST kinds were exercised during language tests.
//!
//! Language crates call [`AstCoverageTracker::hit`] / [`hit_name`] while walking
//! fixtures, then [`assert_all_exercised`] at the end of the suite so every
//! non-`Skip` / non-`Literal` entry in `{lang}-ast-coverage.json` has ≥1 hit.
//!
//! Enable with env `RGCTL_AST_COVERAGE_TRACK=1` or construct explicitly in tests.
//! Until plugins wire hits for newly promoted kinds, prefer opt-in usage so
//! existing suites stay green.

use crate::HandlerMask;
use std::collections::HashMap;
use std::sync::Mutex;

/// Records hit counts per grammar `kind_id` (and optionally by kind name).
#[derive(Debug, Default)]
pub struct AstCoverageTracker {
    language_id: String,
    /// kind_id → hit count
    hits: Mutex<HashMap<u16, u64>>,
    /// Expected active kinds: name → kind_id (from manifest + live grammar)
    expected: HashMap<String, u16>,
}

impl AstCoverageTracker {
    /// Build a tracker for `language_id` from coverage-manifest handler map + live grammar.
    ///
    /// Only non-`Skip` / non-`Literal` handlers are required to be exercised.
    pub fn from_manifest_handlers(
        language_id: impl Into<String>,
        handlers: &HashMap<String, String>,
        lang: &tree_sitter::Language,
    ) -> Self {
        let mut expected = HashMap::new();
        for (name, label) in handlers {
            let Some(mask) = HandlerMask::from_coverage_label(label) else {
                continue;
            };
            if mask.is_passive() {
                continue;
            }
            let kind_id = lang.id_for_node_kind(name, true);
            if kind_id == 0 {
                continue;
            }
            expected.insert(name.clone(), kind_id);
        }
        Self {
            language_id: language_id.into(),
            hits: Mutex::new(HashMap::new()),
            expected,
        }
    }

    /// Parse `{lang}-ast-coverage.json` text and build a tracker.
    pub fn from_manifest_json(
        language_id: impl Into<String>,
        manifest_json: &str,
        lang: &tree_sitter::Language,
    ) -> Result<Self, String> {
        let v: serde_json::Value =
            serde_json::from_str(manifest_json).map_err(|e| format!("manifest JSON: {e}"))?;
        let handlers_obj = v["handlers"]
            .as_object()
            .ok_or_else(|| "manifest missing `handlers` object".to_string())?;
        let handlers: HashMap<String, String> = handlers_obj
            .iter()
            .map(|(k, val)| (k.clone(), val.as_str().unwrap_or("Skip").to_string()))
            .collect();
        Ok(Self::from_manifest_handlers(language_id, &handlers, lang))
    }

    /// Record that `kind_id` produced a Symbol / Relation / CFG / skeleton result.
    pub fn hit(&self, kind_id: u16) {
        if kind_id == 0 {
            return;
        }
        let mut guard = self.hits.lock().expect("AstCoverageTracker poisoned");
        *guard.entry(kind_id).or_insert(0) += 1;
    }

    /// Record a hit by grammar kind name (resolves id via `expected` or `lang`).
    pub fn hit_name(&self, kind_name: &str, lang: &tree_sitter::Language) {
        if let Some(&id) = self.expected.get(kind_name) {
            self.hit(id);
            return;
        }
        let id = lang.id_for_node_kind(kind_name, true);
        self.hit(id);
    }

    /// Number of expected active kinds.
    pub fn expected_len(&self) -> usize {
        self.expected.len()
    }

    /// Hit count for a kind id (0 if never seen).
    pub fn hit_count(&self, kind_id: u16) -> u64 {
        self.hits
            .lock()
            .expect("AstCoverageTracker poisoned")
            .get(&kind_id)
            .copied()
            .unwrap_or(0)
    }

    /// Fail if any expected kind has zero hits. Returns Ok with a short summary.
    pub fn assert_all_exercised(&self) -> Result<String, String> {
        let hits = self.hits.lock().expect("AstCoverageTracker poisoned");
        let mut missing: Vec<&str> = self
            .expected
            .iter()
            .filter(|(_, id)| hits.get(*id).copied().unwrap_or(0) == 0)
            .map(|(name, _)| name.as_str())
            .collect();
        missing.sort_unstable();
        if missing.is_empty() {
            return Ok(format!(
                "{}: all {} active AST kinds exercised",
                self.language_id,
                self.expected.len()
            ));
        }
        Err(format!(
            "{}: {} active AST kind(s) in *-ast-coverage.json have 0 test hits: {}",
            self.language_id,
            missing.len(),
            missing.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_passes_when_all_hit() {
        let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut handlers = HashMap::new();
        handlers.insert("function_item".into(), "Symbol".into());
        handlers.insert("identifier".into(), "Skip".into());
        let tracker = AstCoverageTracker::from_manifest_handlers("rust", &handlers, &lang);
        assert_eq!(tracker.expected_len(), 1);
        let id = lang.id_for_node_kind("function_item", true);
        tracker.hit(id);
        assert!(tracker.assert_all_exercised().is_ok());
    }

    #[test]
    fn tracker_fails_when_unhit() {
        let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut handlers = HashMap::new();
        handlers.insert("function_item".into(), "Symbol".into());
        let tracker = AstCoverageTracker::from_manifest_handlers("rust", &handlers, &lang);
        let err = tracker.assert_all_exercised().unwrap_err();
        assert!(err.contains("function_item"), "{err}");
    }
}
