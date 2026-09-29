//! Gradle build scripts — best-effort static dependency extraction.

use super::{loc, DependencyDeclaration};
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

/// `implementation 'group:name:version'` / `"..."` / Kotlin `("...")`.
static DEP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?x)
        (?P<scope>implementation|api|compileOnly|runtimeOnly|testImplementation|testCompileOnly|testRuntimeOnly)
        \s*
        (?:\(\s*)?
        ['"](?P<coord>[^'"]+)['"]
        "#,
    )
    .expect("gradle dep regex")
});

pub fn extract(path: &Path, source: &[u8]) -> Vec<DependencyDeclaration> {
    let Ok(text) = std::str::from_utf8(source) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let line_no = idx + 1;
        for cap in DEP_RE.captures_iter(line) {
            let scope = cap.name("scope").map(|m| m.as_str().to_string());
            let coord = cap.name("coord").map(|m| m.as_str()).unwrap_or("");
            let parts: Vec<&str> = coord.split(':').collect();
            if parts.len() < 2 {
                continue;
            }
            let name = if parts.len() >= 2 {
                format!("{}:{}", parts[0], parts[1])
            } else {
                coord.to_string()
            };
            let version = parts.get(2).map(|s| (*s).to_string());
            out.push(DependencyDeclaration {
                name,
                version_requirement: version,
                scope,
                ecosystem: "gradle".to_string(),
                location: loc(path, line_no, line_no),
                optional: false,
                unresolved: false,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradle_implementation() {
        let src = b"dependencies {\n  implementation 'com.google.guava:guava:31.1-jre'\n}\n";
        let decls = extract(Path::new("build.gradle"), src);
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].name, "com.google.guava:guava");
        assert_eq!(decls[0].version_requirement.as_deref(), Some("31.1-jre"));
    }
}
