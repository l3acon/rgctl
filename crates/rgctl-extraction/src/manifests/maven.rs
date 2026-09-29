//! Maven `pom.xml` → DependencyDeclaration.

use super::{loc, DependencyDeclaration};
use std::collections::HashMap;
use std::path::Path;

pub fn extract(path: &Path, source: &[u8]) -> Vec<DependencyDeclaration> {
    let Ok(text) = std::str::from_utf8(source) else {
        return Vec::new();
    };
    let Ok(doc) = roxmltree::Document::parse(text) else {
        return Vec::new();
    };

    let mut props: HashMap<String, String> = HashMap::new();
    if let Some(props_el) = find_child_deep(doc.root_element(), "properties") {
        for child in props_el.children().filter(|n| n.is_element()) {
            let name = child.tag_name().name().to_string();
            if let Some(val) = child.text().map(str::trim).filter(|s| !s.is_empty()) {
                props.insert(name, val.to_string());
            }
        }
    }

    let mut out = Vec::new();
    collect_deps(
        doc.root_element(),
        path,
        &props,
        &mut out,
        /*in_dep_mgmt*/ false,
    );
    out
}

fn collect_deps(
    node: roxmltree::Node<'_, '_>,
    path: &Path,
    props: &HashMap<String, String>,
    out: &mut Vec<DependencyDeclaration>,
    in_dep_mgmt: bool,
) {
    let tag = node.tag_name().name();
    let next_mgmt = in_dep_mgmt || tag == "dependencyManagement";

    if tag == "dependency" {
        let group = child_text(node, "groupId");
        let artifact = child_text(node, "artifactId");
        let version_raw = child_text(node, "version");
        let scope = child_text(node, "scope");
        let optional = child_text(node, "optional").as_deref() == Some("true");
        let typ = child_text(node, "type");

        if let (Some(g), Some(a)) = (group, artifact) {
            let g = resolve_props(&g, props);
            let a = resolve_props(&a, props);
            let version = version_raw.map(|v| resolve_props(&v, props));
            let unresolved = version.as_ref().is_some_and(|v| v.contains("${"));
            let mut scope = scope.unwrap_or_else(|| {
                if next_mgmt && typ.as_deref() == Some("pom") {
                    "import".to_string()
                } else if next_mgmt {
                    "dependencyManagement".to_string()
                } else {
                    "compile".to_string()
                }
            });
            if typ.as_deref() == Some("pom") && scope != "import" {
                // keep
                let _ = &mut scope;
            }
            let line = node.document().text_pos_at(node.range().start).row as usize;
            out.push(DependencyDeclaration {
                name: format!("{g}:{a}"),
                version_requirement: version,
                scope: Some(scope),
                ecosystem: "maven".to_string(),
                location: loc(path, line, line),
                optional,
                unresolved,
            });
        }
        return;
    }

    for child in node.children().filter(|n| n.is_element()) {
        collect_deps(child, path, props, out, next_mgmt);
    }
}

fn find_child_deep<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    name: &str,
) -> Option<roxmltree::Node<'a, 'input>> {
    if node.is_element() && node.tag_name().name() == name {
        return Some(node);
    }
    for child in node.children() {
        if let Some(found) = find_child_deep(child, name) {
            return Some(found);
        }
    }
    None
}

fn child_text(node: roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
        .and_then(|c| c.text().map(|t| t.trim().to_string()))
        .filter(|s| !s.is_empty())
}

fn resolve_props(s: &str, props: &HashMap<String, String>) -> String {
    let mut out = s.to_string();
    // Single-pass ${key} substitution from this POM's <properties>.
    for (k, v) in props {
        let needle = format!("${{{k}}}");
        out = out.replace(&needle, v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarkus_style_pom() {
        let src = r#"
<project>
  <properties>
    <quarkus.platform.version>2.16.12.Final</quarkus.platform.version>
  </properties>
  <dependencyManagement>
    <dependencies>
      <dependency>
        <groupId>io.quarkus</groupId>
        <artifactId>quarkus-bom</artifactId>
        <version>${quarkus.platform.version}</version>
        <type>pom</type>
        <scope>import</scope>
      </dependency>
    </dependencies>
  </dependencyManagement>
  <dependencies>
    <dependency>
      <groupId>io.quarkus</groupId>
      <artifactId>quarkus-hibernate-orm</artifactId>
    </dependency>
  </dependencies>
</project>
"#;
        let decls = extract(Path::new("pom.xml"), src.as_bytes());
        assert!(
            decls
                .iter()
                .any(|d| d.name == "io.quarkus:quarkus-hibernate-orm")
        );
        let bom = decls
            .iter()
            .find(|d| d.name == "io.quarkus:quarkus-bom")
            .unwrap();
        assert_eq!(
            bom.version_requirement.as_deref(),
            Some("2.16.12.Final")
        );
        assert_eq!(bom.scope.as_deref(), Some("import"));
    }
}
