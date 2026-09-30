//! Path-based ingest routing for discover (manifest vs config vs workflow vs ignore).
//!
//! Classification is basename/path-based so ambiguous extensions (`.xml`, `.toml`,
//! `.json`, `.yml`) do not dual-emit ConfigKeys and Dependency nodes.

use std::path::Path;

/// How discover should ingest a file after language plugins are considered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IngestRoute {
    /// Build manifests (`pom.xml`, `Cargo.toml`, …) → Dependency graph.
    Manifest,
    /// Configuration files → ConfigKey with spans.
    Config,
    /// CI workflow files (Job/BuildStep; may use config extractors until workflow emitters land).
    Workflow,
    /// Skip (lockfiles, unknown XML, etc.).
    Ignore,
}

/// Classify a repository-relative or absolute path into an ingest route.
///
/// Language plugins (`.java`, `.rs`, …) are checked by the registry *before*
/// this function; callers should only use this for non-language files.
pub fn classify_ingest_path(path: &Path) -> IngestRoute {
    let path_str = path.to_string_lossy().replace('\\', "/");
    let basename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // Lockfiles / generated dependency pins — never flat-config or re-parse as manifests.
    if is_lockfile(&basename) {
        return IngestRoute::Ignore;
    }

    // Manifest basenames (exclusive — do not also treat as Config).
    if matches!(
        basename.as_str(),
        "pom.xml"
            | "cargo.toml"
            | "package.json"
            | "go.mod"
            | "build.gradle"
            | "build.gradle.kts"
    ) {
        return IngestRoute::Manifest;
    }

    // GitHub Actions workflows.
    if (path_str.contains("/.github/workflows/") || path_str.starts_with(".github/workflows/"))
        && (basename.ends_with(".yml") || basename.ends_with(".yaml"))
    {
        return IngestRoute::Workflow;
    }

    // XML: POM already returned as Manifest. Allowlisted config XML only —
    // all other `.xml` stays Ignore (avoids node explosion / Gate A noise).
    if basename.ends_with(".xml") {
        if is_allowlisted_config_xml(&basename, &path_str) {
            return IngestRoute::Config;
        }
        return IngestRoute::Ignore;
    }

    // Generic config extensions (properties, yaml, toml, json, ini).
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext = ext.to_ascii_lowercase();
        if matches!(
            ext.as_str(),
            "properties" | "ini" | "yaml" | "yml" | "toml" | "json"
        ) {
            return IngestRoute::Config;
        }
    }

    IngestRoute::Ignore
}

fn is_lockfile(basename: &str) -> bool {
    matches!(
        basename,
        "cargo.lock"
            | "package-lock.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "pnpm-lock.yml"
            | "go.sum"
            | "composer.lock"
            | "poetry.lock"
            | "gemfile.lock"
    )
}

/// Non-POM XML that is safe to flatten as ConfigKeys (resources / known names).
fn is_allowlisted_config_xml(basename: &str, path_str: &str) -> bool {
    matches!(
        basename,
        "web.xml"
            | "persistence.xml"
            | "beans.xml"
            | "applicationcontext.xml"
            | "config.xml"
            | "settings.xml"
    ) || path_str.contains("/src/main/resources/") && basename.ends_with(".xml")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn pom_is_manifest() {
        assert_eq!(
            classify_ingest_path(Path::new("app/pom.xml")),
            IngestRoute::Manifest
        );
    }

    #[test]
    fn application_properties_is_config() {
        assert_eq!(
            classify_ingest_path(Path::new("src/main/resources/application.properties")),
            IngestRoute::Config
        );
    }

    #[test]
    fn cargo_toml_is_manifest() {
        assert_eq!(
            classify_ingest_path(Path::new("crates/foo/Cargo.toml")),
            IngestRoute::Manifest
        );
    }

    #[test]
    fn package_json_and_go_mod_are_manifest() {
        assert_eq!(
            classify_ingest_path(Path::new("package.json")),
            IngestRoute::Manifest
        );
        assert_eq!(
            classify_ingest_path(Path::new("go.mod")),
            IngestRoute::Manifest
        );
    }

    #[test]
    fn gradle_basenames_are_manifest() {
        assert_eq!(
            classify_ingest_path(Path::new("build.gradle")),
            IngestRoute::Manifest
        );
        assert_eq!(
            classify_ingest_path(Path::new("build.gradle.kts")),
            IngestRoute::Manifest
        );
    }

    #[test]
    fn random_xml_is_ignored() {
        // Policy: non-allowlisted XML is Ignore (pom.xml is Manifest).
        assert_eq!(
            classify_ingest_path(Path::new("docs/something.xml")),
            IngestRoute::Ignore
        );
        assert_eq!(
            classify_ingest_path(Path::new("META-INF/persistence.xml")),
            IngestRoute::Config
        );
        assert_eq!(
            classify_ingest_path(Path::new("config.xml")),
            IngestRoute::Config
        );
    }

    #[test]
    fn lockfiles_ignored() {
        assert_eq!(
            classify_ingest_path(Path::new("Cargo.lock")),
            IngestRoute::Ignore
        );
        assert_eq!(
            classify_ingest_path(Path::new("package-lock.json")),
            IngestRoute::Ignore
        );
        assert_eq!(
            classify_ingest_path(Path::new("go.sum")),
            IngestRoute::Ignore
        );
    }

    #[test]
    fn github_workflow_is_workflow() {
        let p = PathBuf::from(".github/workflows/ci.yml");
        assert_eq!(classify_ingest_path(&p), IngestRoute::Workflow);
    }
}
