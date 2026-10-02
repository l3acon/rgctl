//! Bundled / vendored dependency scanners (opt-in; not on default discover).
//!
//! Sync CPU work — callers should use `spawn_blocking` from async contexts.

use rgctl_extraction::extract_declarations;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::ZipArchive;

use crate::error::SecurityError;

/// A dependency hit discovered from a manifest or bundled artifact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DepHit {
    pub ecosystem: String,
    pub name: String,
    pub version: Option<String>,
    pub provenance: String,
}

/// Options for bundled scans (all opt-in).
#[derive(Debug, Clone, Default)]
pub struct BundledScanOpts {
    /// Glob roots for JARs/WARs (e.g. `lib`).
    pub jar_roots: Vec<PathBuf>,
    /// Scan `node_modules/*/package.json` under these roots.
    pub node_modules_roots: Vec<PathBuf>,
    pub max_jars: usize,
    pub max_node_packages: usize,
}

impl BundledScanOpts {
    pub fn with_defaults() -> Self {
        Self {
            jar_roots: Vec::new(),
            node_modules_roots: Vec::new(),
            max_jars: 500,
            max_node_packages: 2000,
        }
    }
}

/// Scan declared manifests under `repo` (pom.xml, Cargo.toml, package.json, go.mod, gradle).
pub fn scan_manifests(repo: &Path) -> Result<Vec<DepHit>, SecurityError> {
    let mut hits = Vec::new();
    let names = [
        "pom.xml",
        "Cargo.toml",
        "package.json",
        "go.mod",
        "build.gradle",
        "build.gradle.kts",
    ];
    for ent in WalkDir::new(repo)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = ent.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        // Skip node_modules / target / vendor noise for root manifests
        let path_str = path.to_string_lossy();
        if path_str.contains("/node_modules/")
            || path_str.contains("/target/")
            || path_str.contains("/.git/")
        {
            continue;
        }
        if !names.iter().any(|n| name.eq_ignore_ascii_case(n)) {
            continue;
        }
        let bytes = fs::read(path).map_err(|e| SecurityError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        for d in extract_declarations(path, &bytes) {
            hits.push(DepHit {
                ecosystem: d.ecosystem,
                name: d.name,
                version: d.version_requirement,
                provenance: path.display().to_string(),
            });
        }
    }
    Ok(hits)
}

/// Scan JARs for embedded `META-INF/maven/**/pom.xml` dependencies.
pub fn scan_jars(repo: &Path, opts: &BundledScanOpts) -> Result<Vec<DepHit>, SecurityError> {
    let mut hits = Vec::new();
    let mut jar_count = 0usize;
    for root in &opts.jar_roots {
        let abs = if root.is_absolute() {
            root.clone()
        } else {
            repo.join(root)
        };
        if !abs.exists() {
            continue;
        }
        for ent in WalkDir::new(&abs).into_iter().filter_map(|e| e.ok()) {
            if jar_count >= opts.max_jars {
                break;
            }
            let path = ent.path();
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if !ext.eq_ignore_ascii_case("jar") && !ext.eq_ignore_ascii_case("war") {
                continue;
            }
            jar_count += 1;
            hits.extend(scan_one_jar(path)?);
        }
    }
    Ok(hits)
}

fn scan_one_jar(path: &Path) -> Result<Vec<DepHit>, SecurityError> {
    let file = fs::File::open(path).map_err(|e| SecurityError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let mut archive = ZipArchive::new(file).map_err(|e| {
        SecurityError::Msg(format!("zip open {}: {e}", path.display()))
    })?;
    let mut hits = Vec::new();
    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();
    for name in names {
        if !(name.starts_with("META-INF/maven/") && name.ends_with("/pom.xml")) {
            continue;
        }
        let mut file = archive.by_name(&name).map_err(|e| {
            SecurityError::Msg(format!("zip entry {name}: {e}"))
        })?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).map_err(|e| SecurityError::Io {
            path: format!("{}!{}", path.display(), name),
            source: e,
        })?;
        let virt_path = PathBuf::from(format!("{}!{}", path.display(), name));
        for d in extract_declarations(&virt_path, &buf) {
            hits.push(DepHit {
                ecosystem: d.ecosystem,
                name: d.name,
                version: d.version_requirement,
                provenance: format!("{}!{}", path.display(), name),
            });
        }
    }
    Ok(hits)
}

/// Scan `node_modules/<pkg>/package.json` for name/version (honesty: direct children only).
pub fn scan_node_modules(repo: &Path, opts: &BundledScanOpts) -> Result<Vec<DepHit>, SecurityError> {
    let mut hits = Vec::new();
    let mut count = 0usize;
    for root in &opts.node_modules_roots {
        let nm = if root.is_absolute() {
            root.join("node_modules")
        } else {
            repo.join(root).join("node_modules")
        };
        if !nm.is_dir() {
            continue;
        }
        for ent in fs::read_dir(&nm).map_err(|e| SecurityError::Io {
            path: nm.display().to_string(),
            source: e,
        })? {
            if count >= opts.max_node_packages {
                break;
            }
            let ent = ent.map_err(|e| SecurityError::Io {
                path: nm.display().to_string(),
                source: e,
            })?;
            let pkg_json = ent.path().join("package.json");
            if !pkg_json.is_file() {
                // scoped packages @scope/name
                if ent.path().is_dir()
                    && ent
                        .file_name()
                        .to_string_lossy()
                        .starts_with('@')
                {
                    for sub in fs::read_dir(ent.path()).into_iter().flatten().flatten() {
                        if count >= opts.max_node_packages {
                            break;
                        }
                        let pj = sub.path().join("package.json");
                        if pj.is_file() {
                            count += 1;
                            if let Some(h) = read_npm_package_json(&pj)? {
                                hits.push(h);
                            }
                        }
                    }
                }
                continue;
            }
            count += 1;
            if let Some(h) = read_npm_package_json(&pkg_json)? {
                hits.push(h);
            }
        }
    }
    Ok(hits)
}

fn read_npm_package_json(path: &Path) -> Result<Option<DepHit>, SecurityError> {
    let bytes = fs::read(path).map_err(|e| SecurityError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| {
        SecurityError::Msg(format!("package.json {}: {e}", path.display()))
    })?;
    let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("");
    if name.is_empty() {
        return Ok(None);
    }
    let version = v
        .get("version")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    Ok(Some(DepHit {
        ecosystem: "npm".into(),
        name: name.to_string(),
        version,
        provenance: path.display().to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    #[test]
    fn jar_embedded_pom_jackson() {
        let dir = tempdir().unwrap();
        let jar = dir.path().join("audit.jar");
        {
            let f = fs::File::create(&jar).unwrap();
            let mut zip = ZipWriter::new(f);
            let opts = SimpleFileOptions::default();
            zip.start_file(
                "META-INF/maven/com.enterprise/audit/pom.xml",
                opts,
            )
            .unwrap();
            zip.write_all(
                br#"<?xml version="1.0"?>
<project>
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.enterprise</groupId>
  <artifactId>audit</artifactId>
  <version>1.0.0</version>
  <dependencies>
    <dependency>
      <groupId>com.fasterxml.jackson.core</groupId>
      <artifactId>jackson-databind</artifactId>
      <version>2.13.5</version>
    </dependency>
  </dependencies>
</project>"#,
            )
            .unwrap();
            zip.finish().unwrap();
        }
        let mut opts = BundledScanOpts::with_defaults();
        opts.jar_roots = vec![dir.path().to_path_buf()];
        let hits = scan_jars(dir.path(), &opts).unwrap();
        assert!(
            hits.iter()
                .any(|h| h.name.contains("jackson-databind") && h.version.as_deref() == Some("2.13.5")),
            "{hits:?}"
        );
    }

    #[test]
    fn node_modules_package() {
        let dir = tempdir().unwrap();
        let pkg = dir.path().join("node_modules").join("lodash");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(
            pkg.join("package.json"),
            r#"{"name":"lodash","version":"4.17.21"}"#,
        )
        .unwrap();
        let mut opts = BundledScanOpts::with_defaults();
        opts.node_modules_roots = vec![PathBuf::from(".")];
        let hits = scan_node_modules(dir.path(), &opts).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "lodash");
        assert_eq!(hits[0].version.as_deref(), Some("4.17.21"));
    }
}
