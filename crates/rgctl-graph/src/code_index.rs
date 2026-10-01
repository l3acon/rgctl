//! Code body hashing and lookup index (Phase 12.0).

use blake3;
use rgctl_error::{Error, Result};
use rgctl_plugin_api::SourceLocation;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Location of a hashed code fragment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeLocation {
    /// Source file path
    pub file_path: String,
    /// Start line (1-based)
    pub start_line: usize,
    /// End line (1-based)
    pub end_line: usize,
    /// Optional code text that was hashed.
    ///
    /// Default discover leaves this empty to avoid multi-GB RAM / `code_index.json`
    /// payloads. Enable [`CodeIndex::store_bodies`] only when a caller needs
    /// `get_code` lookups.
    #[serde(default)]
    pub code: String,
}

/// BLAKE3-backed index for fast change detection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodeIndex {
    hash_to_code: HashMap<String, CodeLocation>,
    #[serde(skip)]
    cache_file: Option<PathBuf>,
    /// When true, [`Self::add_code`] retains full body text (expensive at scale).
    #[serde(skip)]
    store_bodies: bool,
}

impl CodeIndex {
    /// Create an empty in-memory index (span metadata only; no body text).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an index backed by a cache file path (span metadata only).
    pub fn with_cache_file(cache_file: PathBuf) -> Self {
        Self {
            hash_to_code: HashMap::new(),
            cache_file: Some(cache_file),
            store_bodies: false,
        }
    }

    /// Retain full function bodies in the index (opt-in; multi-GB on large repos).
    pub fn store_bodies(mut self, enabled: bool) -> Self {
        self.store_bodies = enabled;
        self
    }

    /// Whether full body text is retained.
    pub fn stores_bodies(&self) -> bool {
        self.store_bodies
    }

    /// Hash code and record its location. Returns the hex digest.
    ///
    /// By default only path/line span metadata is stored (empty `code`).
    pub fn add_code(&mut self, code: &str, location: &SourceLocation) -> String {
        let hash = hash_code(code);
        self.hash_to_code.insert(
            hash.clone(),
            CodeLocation {
                file_path: location.file.clone(),
                start_line: location.start_line,
                end_line: location.end_line,
                code: if self.store_bodies {
                    code.to_string()
                } else {
                    String::new()
                },
            },
        );
        hash
    }

    /// Returns true when `stored_hash` differs from the hash of `current_code`.
    pub fn has_changed(stored_hash: &str, current_code: &str) -> bool {
        hash_code(current_code) != stored_hash
    }

    /// Look up code by hash (only populated when [`Self::store_bodies`] is enabled).
    pub fn get_code(&self, hash: &str) -> Option<&str> {
        self.hash_to_code
            .get(hash)
            .map(|loc| loc.code.as_str())
            .filter(|code| !code.is_empty())
    }

    /// Number of indexed fragments.
    pub fn len(&self) -> usize {
        self.hash_to_code.len()
    }

    /// Returns true when the index has no entries.
    pub fn is_empty(&self) -> bool {
        self.hash_to_code.is_empty()
    }

    /// Persist the index to the configured cache file.
    ///
    /// Skips writing when the map is empty (default discover path).
    pub fn save(&self) -> Result<()> {
        let Some(path) = &self.cache_file else {
            return Ok(());
        };
        if self.hash_to_code.is_empty() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(&self.hash_to_code)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load an index from disk, or return empty if missing.
    ///
    /// Does **not** enable body storage; loaded `code` fields are kept as-is for
    /// callers that opt in later via [`Self::store_bodies`].
    pub fn load(cache_file: PathBuf) -> Result<Self> {
        if cache_file.exists() {
            let json = std::fs::read_to_string(&cache_file)?;
            let hash_to_code: HashMap<String, CodeLocation> =
                serde_json::from_str(&json).map_err(|e| Error::SerdeError(e.to_string()))?;
            Ok(Self {
                hash_to_code,
                cache_file: Some(cache_file),
                store_bodies: false,
            })
        } else {
            Ok(Self::with_cache_file(cache_file))
        }
    }

    /// Open a cache path without reading an existing on-disk index into RAM.
    ///
    /// Use for default discover so a stale multi-GB `code_index.json` cannot
    /// inflate cold RSS.
    pub fn open_empty(cache_file: PathBuf) -> Self {
        Self::with_cache_file(cache_file)
    }

    /// Default cache path under a repository root.
    pub fn default_cache_path(repo_root: &Path) -> PathBuf {
        repo_root.join(".rgctl").join("code_index.json")
    }
}

/// Compute a BLAKE3 hex digest of code text.
pub fn hash_code(code: &str) -> String {
    blake3::hash(code.as_bytes()).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_hash_stable() {
        let h1 = hash_code("fn main() {}");
        let h2 = hash_code("fn main() {}");
        assert_eq!(h1, h2);
        assert_ne!(h1, hash_code("fn main() { }"));
    }

    #[test]
    fn test_code_index_change_detection() {
        let mut index = CodeIndex::new().store_bodies(true);
        let loc = SourceLocation {
            file: "main.rs".to_string(),
            start_line: 1,
            end_line: 1,
            start_column: 0,
            end_column: 0,
        };
        let hash = index.add_code("fn old() {}", &loc);
        assert!(!CodeIndex::has_changed(&hash, "fn old() {}"));
        assert!(CodeIndex::has_changed(&hash, "fn new() {}"));
        assert_eq!(index.get_code(&hash), Some("fn old() {}"));
    }

    #[test]
    fn default_add_code_omits_body_text() {
        let mut index = CodeIndex::new();
        let loc = SourceLocation {
            file: "main.rs".to_string(),
            start_line: 1,
            end_line: 1,
            start_column: 0,
            end_column: 0,
        };
        let hash = index.add_code("fn body() {}", &loc);
        assert!(index.get_code(&hash).is_none());
        assert!(index.hash_to_code[&hash].code.is_empty());
    }
}
