//! go.mod → DependencyDeclaration.

use super::{loc, DependencyDeclaration};
use std::path::Path;

pub fn extract(path: &Path, source: &[u8]) -> Vec<DependencyDeclaration> {
    let Ok(text) = std::str::from_utf8(source) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut in_require = false;
    for (idx, line) in text.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim();
        if trimmed.starts_with("require (") || trimmed == "require (" {
            in_require = true;
            continue;
        }
        if in_require {
            if trimmed == ")" {
                in_require = false;
                continue;
            }
            if let Some(decl) = parse_require_line(path, trimmed, line_no) {
                out.push(decl);
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("require ")
            && let Some(decl) = parse_require_line(path, rest.trim(), line_no)
        {
            out.push(decl);
        }
    }
    out
}

fn parse_require_line(path: &Path, rest: &str, line_no: usize) -> Option<DependencyDeclaration> {
    let parts: Vec<&str> = rest.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }
    let name = parts[0].trim_matches('"').to_string();
    if name.is_empty() || name == "//" {
        return None;
    }
    let version = parts.get(1).map(|s| s.trim_matches('"').to_string());
    Some(DependencyDeclaration {
        name,
        version_requirement: version,
        scope: Some("require".to_string()),
        ecosystem: "golang".to_string(),
        location: loc(path, line_no, line_no),
        optional: false,
        unresolved: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_mod_require() {
        let src = b"module example.com/app\n\nrequire (\n\tgithub.com/foo/bar v1.2.3\n)\n";
        let decls = extract(Path::new("go.mod"), src);
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].name, "github.com/foo/bar");
        assert_eq!(decls[0].version_requirement.as_deref(), Some("v1.2.3"));
    }
}
