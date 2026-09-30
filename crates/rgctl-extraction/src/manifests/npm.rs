//! package.json → DependencyDeclaration.

use super::{loc, DependencyDeclaration};
use std::path::Path;

pub fn extract(path: &Path, source: &[u8]) -> Vec<DependencyDeclaration> {
    let Ok(text) = std::str::from_utf8(source) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (field, scope) in [
        ("dependencies", "runtime"),
        ("devDependencies", "dev"),
        ("peerDependencies", "peer"),
        ("optionalDependencies", "optional"),
    ] {
        if let Some(obj) = v.get(field).and_then(|x| x.as_object()) {
            for (name, ver) in obj {
                let version = ver.as_str().map(str::to_string);
                out.push(DependencyDeclaration {
                    name: name.clone(),
                    version_requirement: version,
                    scope: Some(scope.to_string()),
                    ecosystem: "npm".to_string(),
                    location: loc(path, 1, 1),
                    optional: scope == "optional",
                    unresolved: false,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npm_deps() {
        let src = br#"{"dependencies":{"lodash":"^4.17.21"},"devDependencies":{"jest":"29.0.0"}}"#;
        let decls = extract(Path::new("package.json"), src);
        assert!(decls.iter().any(|d| d.name == "lodash"));
        assert!(decls.iter().any(|d| d.name == "jest" && d.scope.as_deref() == Some("dev")));
    }
}
