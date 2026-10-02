//! Multi-ecosystem package coordinate → import/module prefix resolver.
//!
//! Tables + heuristics for Maven, npm, Cargo, Go, PyPI, NuGet, RubyGems, Composer.
//! Non-trivial Maven groups (e.g. `xalan`) MUST use the mapping table — never guess.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Resolved identity for structured queries (`find --package`, …).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageResolution {
    /// Original coordinate string.
    pub input: String,
    /// Normalized ecosystem (`Maven`, `npm`, `crates.io`, …).
    pub ecosystem: String,
    /// Canonical package name (GAV / crate / module).
    pub name: String,
    /// Import / module prefixes usable with `find --type import` / scope filters.
    pub import_prefixes: Vec<String>,
    /// Typically shipped with the language runtime / JDK / stdlib.
    pub runtime_bundled: bool,
    /// Honesty note when mapping is table-driven or heuristic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub honesty: Option<String>,
}

/// Resolver error.
#[derive(Debug, thiserror::Error)]
pub enum PackageResolveError {
    /// Coordinate could not be parsed.
    #[error("unrecognized package coordinate: {0}")]
    Unrecognized(String),
    /// Maven short group with no table entry (refuse silent wrong guess).
    #[error(
        "Maven group `{group}` has no import mapping (refusing heuristic guess); \
         add an entry or pass an explicit import prefix"
    )]
    UnmappedMavenGroup {
        /// Group id.
        group: String,
    },
}

/// Resolve PURL or ecosystem-native coordinates to import prefixes.
pub fn resolve_package(coords: &str) -> Result<PackageResolution, PackageResolveError> {
    let trimmed = coords.trim();
    if trimmed.is_empty() {
        return Err(PackageResolveError::Unrecognized(coords.into()));
    }
    if let Some(rest) = trimmed.strip_prefix("pkg:") {
        return resolve_purl(rest, trimmed);
    }
    // npm scoped (@scope/name) before Maven GAV (group:artifact)
    if trimmed.starts_with('@') {
        let name = trimmed.split('@').nth(1).map(|s| format!("@{s}")).unwrap_or_else(|| trimmed.to_string());
        // Actually "@types/node".split('@') => ["", "types/node"] → "@types/node"
        let name = if trimmed.starts_with('@') {
            trimmed.to_string()
        } else {
            name
        };
        return Ok(PackageResolution {
            input: trimmed.into(),
            ecosystem: "npm".into(),
            name: name.clone(),
            import_prefixes: vec![name],
            runtime_bundled: false,
            honesty: Some("npm scoped package name used as JS/TS import prefix".into()),
        });
    }
    // Maven GAV: group:artifact or group:artifact:version
    if trimmed.contains(':') && !trimmed.contains('/') {
        return resolve_maven_gav(trimmed);
    }
    // crates.io / Cargo
    if trimmed.starts_with("crates.io/") || looks_like_crate(trimmed) {
        let name = trimmed
            .strip_prefix("crates.io/")
            .unwrap_or(trimmed)
            .split('@')
            .next()
            .unwrap_or(trimmed)
            .to_string();
        return Ok(PackageResolution {
            input: trimmed.into(),
            ecosystem: "crates.io".into(),
            name: name.clone(),
            import_prefixes: vec![name.replace('-', "_")],
            runtime_bundled: false,
            honesty: Some("Cargo crate name → Rust use path (hyphens → underscores)".into()),
        });
    }
    // npm scoped or plain
    if trimmed.starts_with('@') || trimmed.contains('/') && !trimmed.contains('.') {
        let name = trimmed.split('@').next().unwrap_or(trimmed).to_string();
        return Ok(PackageResolution {
            input: trimmed.into(),
            ecosystem: "npm".into(),
            name: name.clone(),
            import_prefixes: vec![name],
            runtime_bundled: false,
            honesty: Some("npm package name used as JS/TS import prefix".into()),
        });
    }
    // Go module path
    if trimmed.contains('.') && trimmed.contains('/') {
        return Ok(PackageResolution {
            input: trimmed.into(),
            ecosystem: "Go".into(),
            name: trimmed.to_string(),
            import_prefixes: vec![trimmed.to_string()],
            runtime_bundled: false,
            honesty: Some("treated as Go module import path".into()),
        });
    }
    // PyPI bare name
    Ok(PackageResolution {
        input: trimmed.into(),
        ecosystem: "PyPI".into(),
        name: trimmed.to_string(),
        import_prefixes: vec![trimmed.replace('-', "_")],
        runtime_bundled: is_python_stdlib(trimmed),
        honesty: Some(
            "PyPI name → import heuristic (hyphens → underscores); verify for namespace packages"
                .into(),
        ),
    })
}

fn resolve_purl(rest: &str, original: &str) -> Result<PackageResolution, PackageResolveError> {
    // pkg:maven/group/artifact@version
    let (eco, rem) = rest
        .split_once('/')
        .ok_or_else(|| PackageResolveError::Unrecognized(original.into()))?;
    let eco_l = eco.to_ascii_lowercase();
    match eco_l.as_str() {
        "maven" => {
            let (path, _ver) = rem.split_once('@').unwrap_or((rem, ""));
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() < 2 {
                return Err(PackageResolveError::Unrecognized(original.into()));
            }
            let group = parts[0].replace('%', "."); // rarely encoded
            let group = group.replace("%2E", ".");
            let artifact = parts[1];
            resolve_maven_parts(&format!("{group}:{artifact}"), &group, artifact)
        }
        "npm" => {
            let (path, _) = rem.split_once('@').unwrap_or((rem, ""));
            // scoped: @scope/name encoded as %40scope/name or @scope/name
            let name = path.replace("%40", "@");
            Ok(PackageResolution {
                input: original.into(),
                ecosystem: "npm".into(),
                name: name.clone(),
                import_prefixes: vec![name],
                runtime_bundled: false,
                honesty: None,
            })
        }
        "cargo" | "crate" => {
            let (name, _) = rem.split_once('@').unwrap_or((rem, ""));
            Ok(PackageResolution {
                input: original.into(),
                ecosystem: "crates.io".into(),
                name: name.into(),
                import_prefixes: vec![name.replace('-', "_")],
                runtime_bundled: false,
                honesty: None,
            })
        }
        "golang" | "go" => {
            let (path, _) = rem.split_once('@').unwrap_or((rem, ""));
            Ok(PackageResolution {
                input: original.into(),
                ecosystem: "Go".into(),
                name: path.into(),
                import_prefixes: vec![path.into()],
                runtime_bundled: false,
                honesty: None,
            })
        }
        "pypi" => {
            let (name, _) = rem.split_once('@').unwrap_or((rem, ""));
            Ok(PackageResolution {
                input: original.into(),
                ecosystem: "PyPI".into(),
                name: name.into(),
                import_prefixes: vec![name.replace('-', "_")],
                runtime_bundled: is_python_stdlib(name),
                honesty: None,
            })
        }
        "nuget" => {
            let (name, _) = rem.split_once('@').unwrap_or((rem, ""));
            Ok(PackageResolution {
                input: original.into(),
                ecosystem: "NuGet".into(),
                name: name.into(),
                import_prefixes: vec![name.replace('.', "/"), name.into()],
                runtime_bundled: false,
                honesty: Some("NuGet id → C# namespace heuristic".into()),
            })
        }
        "gem" | "rubygems" => {
            let (name, _) = rem.split_once('@').unwrap_or((rem, ""));
            Ok(PackageResolution {
                input: original.into(),
                ecosystem: "RubyGems".into(),
                name: name.into(),
                import_prefixes: vec![name.replace('-', "/"), name.replace('-', "_")],
                runtime_bundled: false,
                honesty: Some("RubyGems → require path heuristic".into()),
            })
        }
        "composer" => {
            let (path, _) = rem.split_once('@').unwrap_or((rem, ""));
            Ok(PackageResolution {
                input: original.into(),
                ecosystem: "Composer".into(),
                name: path.into(),
                import_prefixes: vec![path.replace('/', "\\")],
                runtime_bundled: false,
                honesty: Some("Composer vendor/package → PHP namespace stub".into()),
            })
        }
        _ => Err(PackageResolveError::Unrecognized(original.into())),
    }
}

fn resolve_maven_gav(gav: &str) -> Result<PackageResolution, PackageResolveError> {
    let parts: Vec<&str> = gav.split(':').collect();
    if parts.len() < 2 {
        return Err(PackageResolveError::Unrecognized(gav.into()));
    }
    resolve_maven_parts(gav, parts[0], parts[1])
}

fn resolve_maven_parts(
    input: &str,
    group: &str,
    artifact: &str,
) -> Result<PackageResolution, PackageResolveError> {
    let name = format!("{group}:{artifact}");
    let runtime = is_jdk_bundled(group, artifact);
    if let Some(prefix) = maven_table().get(&format!("{group}:{artifact}")) {
        return Ok(PackageResolution {
            input: input.into(),
            ecosystem: "Maven".into(),
            name,
            import_prefixes: vec![prefix.clone()],
            runtime_bundled: runtime,
            honesty: Some("table-mapped Maven coordinate".into()),
        });
    }
    if let Some(prefix) = maven_table().get(group) {
        return Ok(PackageResolution {
            input: input.into(),
            ecosystem: "Maven".into(),
            name,
            import_prefixes: vec![prefix.clone()],
            runtime_bundled: runtime,
            honesty: Some("table-mapped Maven group".into()),
        });
    }
    // Short / non-dotted groups without a table entry: refuse silent guess.
    if !group.contains('.') {
        return Err(PackageResolveError::UnmappedMavenGroup {
            group: group.into(),
        });
    }
    // Dotted group → Java package prefix (drop trailing segments that look like artifact org).
    let prefix = heuristic_maven_prefix(group, artifact);
    Ok(PackageResolution {
        input: input.into(),
        ecosystem: "Maven".into(),
        name,
        import_prefixes: vec![prefix],
        runtime_bundled: runtime,
        honesty: Some("heuristic: dotted Maven group → Java package prefix".into()),
    })
}

fn heuristic_maven_prefix(group: &str, artifact: &str) -> String {
    // Prefer longest useful prefix: group itself for jackson.core style.
    if group.starts_with("com.fasterxml.jackson") {
        return "com.fasterxml.jackson".into();
    }
    if artifact.contains('.') {
        return format!("{group}.{artifact}");
    }
    group.to_string()
}

fn maven_table() -> &'static HashMap<String, String> {
    static TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut m = HashMap::new();
        // Non-trivial short groups
        m.insert("xalan".into(), "org.apache.xalan".into());
        m.insert("xalan:xalan".into(), "org.apache.xalan".into());
        m.insert("xalan:serializer".into(), "org.apache.xml.serializer".into());
        m.insert(
            "xalan:serializer-bcel-6.6.1".into(),
            "org.apache.xml.serializer".into(),
        );
        m.insert("xerces".into(), "org.apache.xerces".into());
        m.insert("xerces:xercesImpl".into(), "org.apache.xerces".into());
        m.insert("dom4j".into(), "org.dom4j".into());
        m.insert("dom4j:dom4j".into(), "org.dom4j".into());
        m.insert("antlr".into(), "org.antlr".into());
        m.insert("asm".into(), "org.objectweb.asm".into());
        // Common aliases
        m.insert(
            "com.fasterxml.jackson.core:jackson-databind".into(),
            "com.fasterxml.jackson".into(),
        );
        m.insert(
            "com.fasterxml.jackson.core:jackson-core".into(),
            "com.fasterxml.jackson".into(),
        );
        m.insert(
            "com.fasterxml.jackson.core:jackson-annotations".into(),
            "com.fasterxml.jackson".into(),
        );
        m.insert("org.slf4j:slf4j-api".into(), "org.slf4j".into());
        m.insert("ch.qos.logback:logback-classic".into(), "ch.qos.logback".into());
        m
    })
}

fn is_jdk_bundled(group: &str, artifact: &str) -> bool {
    matches!(
        (group, artifact),
        ("javax.xml" | "jakarta.xml.bind" | "org.w3c" | "org.xml", _)
            | ("com.sun.xml" | "com.sun.org.apache", _)
    ) || group.starts_with("java.")
        || group == "jdk"
        || (group == "xml-apis" && artifact == "xml-apis")
}

fn is_python_stdlib(name: &str) -> bool {
    matches!(
        name,
        "os" | "sys" | "json" | "re" | "typing" | "pathlib" | "http" | "urllib" | "asyncio"
    )
}

fn looks_like_crate(s: &str) -> bool {
    !s.contains('.')
        && !s.contains(':')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jackson_gav_to_prefix() {
        let r = resolve_package("com.fasterxml.jackson.core:jackson-databind").unwrap();
        assert_eq!(r.ecosystem, "Maven");
        assert!(r.import_prefixes.iter().any(|p| p == "com.fasterxml.jackson"));
    }

    #[test]
    fn xalan_uses_table_not_guess() {
        let r = resolve_package("xalan:serializer").unwrap();
        assert!(r.import_prefixes.iter().any(|p| p.contains("apache")));
        assert!(!r.import_prefixes.iter().any(|p| p == "xalan"));
    }

    #[test]
    fn unmapped_short_maven_group_errors() {
        let err = resolve_package("unknownshort:thing").unwrap_err();
        assert!(matches!(err, PackageResolveError::UnmappedMavenGroup { .. }));
    }

    #[test]
    fn cargo_crate_prefix() {
        let r = resolve_package("serde_json").unwrap();
        assert_eq!(r.ecosystem, "crates.io");
        assert_eq!(r.import_prefixes, vec!["serde_json".to_string()]);
    }

    #[test]
    fn npm_scoped() {
        let r = resolve_package("@types/node").unwrap();
        assert_eq!(r.ecosystem, "npm");
        assert!(r.import_prefixes.iter().any(|p| p == "@types/node"));
    }

    #[test]
    fn jdk_runtime_flag() {
        let r = resolve_package("javax.xml:bind-api").unwrap();
        assert!(r.runtime_bundled);
    }
}
