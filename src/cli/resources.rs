//! `rgctl resources` — first-party parse of JEE descriptors (no Kantra).

use super::context::CliContext;
use super::OutputFormat;
use anyhow::Result;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const RESOURCES_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Default, Serialize)]
struct ResourcesResult {
    schema_version: u32,
    command: &'static str,
    persistence: Vec<PersistenceUnit>,
    datasources: Vec<NamedBinding>,
    jms: Vec<NamedBinding>,
    ejb_bindings: Vec<NamedBinding>,
    cdi: Vec<CdiHint>,
    files_scanned: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PersistenceUnit {
    file: String,
    name: Option<String>,
    provider: Option<String>,
    jta_data_source: Option<String>,
    non_jta_data_source: Option<String>,
    schema_generation: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct NamedBinding {
    kind: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    jndi: Option<String>,
    file: String,
}

#[derive(Debug, Clone, Serialize)]
struct CdiHint {
    file: String,
    bean_discovery_mode: Option<String>,
}

/// `rgctl resources`
pub fn run_resources(ctx: &CliContext, extra_file: Option<String>) -> Result<()> {
    let mut out = ResourcesResult {
        schema_version: RESOURCES_SCHEMA_VERSION,
        command: "resources",
        ..Default::default()
    };

    let mut files = discover_descriptor_files(&ctx.repo)?;
    if let Some(f) = extra_file {
        files.push(PathBuf::from(f));
    }

    for path in &files {
        let rel = path
            .strip_prefix(&ctx.repo)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        out.files_scanned.push(rel.clone());
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if name == "persistence.xml" || rel.ends_with("META-INF/persistence.xml") {
            parse_persistence(&text, &rel, &mut out.persistence);
        } else if name.starts_with("weblogic") || name.starts_with("jboss-") {
            parse_server_bindings(&text, &rel, &mut out);
        } else if name == "web.xml" {
            parse_web_xml(&text, &rel, &mut out);
        } else if name == "beans.xml" {
            parse_beans_xml(&text, &rel, &mut out.cdi);
        }
    }

    if ctx.format == OutputFormat::Json {
        let v = serde_json::to_value(&out)?;
        ctx.emit_json_value(&v)?;
    } else {
        ctx.stdout_line(&format!(
            "resources: {} persistence, {} datasources, {} jms, {} ejb, {} cdi ({} files)",
            out.persistence.len(),
            out.datasources.len(),
            out.jms.len(),
            out.ejb_bindings.len(),
            out.cdi.len(),
            out.files_scanned.len()
        ))?;
        for p in &out.persistence {
            ctx.stdout_line(&format!(
                "  persistence provider={} jta={:?}",
                p.provider.as_deref().unwrap_or("?"),
                p.jta_data_source
            ))?;
        }
    }
    Ok(())
}

fn discover_descriptor_files(repo: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let walker = ignore::WalkBuilder::new(repo)
        .hidden(false)
        .git_ignore(true)
        .build();
    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let ok = name == "persistence.xml"
            || name == "web.xml"
            || name == "beans.xml"
            || (name.starts_with("weblogic") && name.ends_with(".xml"))
            || (name.starts_with("jboss-") && name.ends_with(".xml"));
        if ok {
            out.push(path.to_path_buf());
        }
    }
    out.sort();
    Ok(out)
}

fn parse_persistence(text: &str, file: &str, out: &mut Vec<PersistenceUnit>) {
    // Lightweight tag scrape (namespaces ignored).
    for unit in split_tags(text, "persistence-unit") {
        let name = attr_value(&unit, "name");
        let provider = tag_text(&unit, "provider");
        let jta = tag_text(&unit, "jta-data-source");
        let non_jta = tag_text(&unit, "non-jta-data-source");
        let mut schema = Vec::new();
        for prop in split_tags(&unit, "property") {
            if let Some(n) = attr_value(&prop, "name") {
                if n.contains("schema-generation") || n.contains("ddl") {
                    let v = attr_value(&prop, "value").unwrap_or_default();
                    schema.push(format!("{n}={v}"));
                }
            }
        }
        out.push(PersistenceUnit {
            file: file.into(),
            name,
            provider,
            jta_data_source: jta,
            non_jta_data_source: non_jta,
            schema_generation: schema,
        });
    }
}

fn parse_server_bindings(text: &str, file: &str, out: &mut ResourcesResult) {
    // weblogic / jboss: capture common JNDI-ish attributes and resource-ref names.
    for (tag, kind) in [
        ("resource-description", "resource"),
        ("resource-env-description", "resource-env"),
        ("ejb-local-reference-description", "ejb"),
        ("ejb-reference-description", "ejb"),
        ("message-destination-description", "jms"),
        ("connection-factory", "jms-factory"),
        ("topic", "jms-topic"),
        ("queue", "jms-queue"),
        ("datasource", "datasource"),
    ] {
        for block in split_tags(text, tag) {
            let name = tag_text(&block, "res-ref-name")
                .or_else(|| tag_text(&block, "resource-env-ref-name"))
                .or_else(|| tag_text(&block, "ejb-ref-name"))
                .or_else(|| attr_value(&block, "name"))
                .unwrap_or_else(|| tag.to_string());
            let jndi = tag_text(&block, "jndi-name")
                .or_else(|| tag_text(&block, "lookup-name"))
                .or_else(|| attr_value(&block, "jndi-name"));
            let binding = NamedBinding {
                kind: kind.into(),
                name,
                jndi,
                file: file.into(),
            };
            match kind {
                "jms" | "jms-factory" | "jms-topic" | "jms-queue" => out.jms.push(binding),
                "ejb" => out.ejb_bindings.push(binding),
                "datasource" => out.datasources.push(binding),
                _ => {
                    if binding
                        .jndi
                        .as_deref()
                        .unwrap_or("")
                        .contains("jdbc")
                        || binding.name.contains("jdbc")
                        || binding.name.contains("DataSource")
                        || binding.name.contains("DS")
                    {
                        out.datasources.push(binding);
                    } else if binding.name.contains("jms")
                        || binding
                            .jndi
                            .as_deref()
                            .unwrap_or("")
                            .contains("jms")
                    {
                        out.jms.push(binding);
                    } else {
                        out.ejb_bindings.push(binding);
                    }
                }
            }
        }
    }
    // Coolstore-shaped: plain <jndi-name>jdbc/CoolstoreDS</jndi-name>
    if out.datasources.is_empty() {
        if let Some(jndi) = tag_text(text, "jndi-name") {
            if jndi.contains("jdbc") || jndi.contains("DS") {
                out.datasources.push(NamedBinding {
                    kind: "jndi".into(),
                    name: jndi.clone(),
                    jndi: Some(jndi),
                    file: file.into(),
                });
            }
        }
    }
}

fn parse_web_xml(text: &str, file: &str, out: &mut ResourcesResult) {
    for block in split_tags(text, "resource-ref") {
        let name = tag_text(&block, "res-ref-name").unwrap_or_else(|| "resource-ref".into());
        let jndi = tag_text(&block, "lookup-name");
        out.datasources.push(NamedBinding {
            kind: "resource-ref".into(),
            name,
            jndi,
            file: file.into(),
        });
    }
}

fn parse_beans_xml(text: &str, file: &str, out: &mut Vec<CdiHint>) {
    let mode = attr_value(text, "bean-discovery-mode").or_else(|| {
        // sometimes on beans root
        text.find("bean-discovery-mode=\"")
            .and_then(|i| {
                let rest = &text[i + "bean-discovery-mode=\"".len()..];
                rest.split('"').next().map(|s| s.to_string())
            })
    });
    out.push(CdiHint {
        file: file.into(),
        bean_discovery_mode: mode,
    });
}

fn split_tags(hay: &str, tag: &str) -> Vec<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut rest = hay;
    while let Some(start) = rest.find(&open) {
        let from = &rest[start..];
        if let Some(end) = from.find(&close) {
            out.push(from[..end + close.len()].to_string());
            rest = &from[end + close.len()..];
        } else {
            // self-closing or truncated
            if let Some(gt) = from.find('>') {
                out.push(from[..=gt].to_string());
                rest = &from[gt + 1..];
            } else {
                break;
            }
        }
    }
    out
}

fn tag_text(hay: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = hay.find(&open)?;
    let after_open = &hay[start..];
    let gt = after_open.find('>')?;
    let body_start = &after_open[gt + 1..];
    let end = body_start.find(&close)?;
    Some(body_start[..end].trim().to_string())
}

fn attr_value(hay: &str, attr: &str) -> Option<String> {
    let key = format!("{attr}=\"");
    let i = hay.find(&key)?;
    let rest = &hay[i + key.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_persistence_eclipselink_coolstore() {
        let xml = r#"<?xml version="1.0"?>
        <persistence>
          <persistence-unit name="coolstore">
            <provider>org.eclipse.persistence.jpa.PersistenceProvider</provider>
            <jta-data-source>jdbc/CoolstoreDS</jta-data-source>
            <properties>
              <property name="javax.persistence.schema-generation.database.action" value="drop-and-create"/>
            </properties>
          </persistence-unit>
        </persistence>"#;
        let mut units = Vec::new();
        parse_persistence(xml, "META-INF/persistence.xml", &mut units);
        assert_eq!(units.len(), 1);
        assert!(units[0]
            .provider
            .as_deref()
            .unwrap()
            .contains("eclipse.persistence"));
        assert_eq!(
            units[0].jta_data_source.as_deref(),
            Some("jdbc/CoolstoreDS")
        );
        assert!(!units[0].schema_generation.is_empty());
    }

    #[test]
    fn parse_weblogic_jndi() {
        let xml = r#"
        <weblogic-ejb-jar>
          <message-destination-description>
            <message-destination-name>orders</message-destination-name>
            <jndi-name>jms/orders</jndi-name>
          </message-destination-description>
          <resource-description>
            <res-ref-name>jdbc/CoolstoreDS</res-ref-name>
            <jndi-name>jdbc/CoolstoreDS</jndi-name>
          </resource-description>
        </weblogic-ejb-jar>"#;
        let mut out = ResourcesResult::default();
        parse_server_bindings(xml, "WEB-INF/weblogic-ejb-jar.xml", &mut out);
        assert!(!out.jms.is_empty() || !out.datasources.is_empty() || !out.ejb_bindings.is_empty());
        assert!(
            out.datasources.iter().any(|d| d.name.contains("CoolstoreDS"))
                || out.datasources.iter().any(|d| d
                    .jndi
                    .as_deref()
                    .unwrap_or("")
                    .contains("CoolstoreDS"))
        );
    }
}
