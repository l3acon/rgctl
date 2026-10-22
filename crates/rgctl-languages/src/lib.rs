//! Built-in Tier 1 language plugin registration.

use rgctl_registry::LanguageRegistry;

/// Register all Tier 1 language plugins.
pub fn register_languages(registry: &mut LanguageRegistry) {
    rgctl_lang_rust::register(registry);
    rgctl_lang_python::register(registry);
    rgctl_lang_javascript::register(registry);
    rgctl_lang_typescript::register(registry);
    rgctl_lang_go::register(registry);
    rgctl_lang_java::register(registry);
    rgctl_lang_csharp::register(registry);
    rgctl_lang_c::register(registry);
    rgctl_lang_cpp::register(registry);
    rgctl_lang_markdown::register(registry);
    rgctl_lang_php::register(registry);
    rgctl_lang_ruby::register(registry);
    rgctl_lang_puppet::register(registry);
    rgctl_lang_erb::register(registry);
    rgctl_lang_kotlin::register(registry);
    rgctl_lang_groovy::register(registry);
    rgctl_lang_swift::register(registry);
    rgctl_lang_scala::register(registry);
    rgctl_lang_dart::register(registry);
    rgctl_lang_elixir::register(registry);
    rgctl_lang_lua::register(registry);
    rgctl_lang_bash::register(registry);
    rgctl_lang_sql::register(registry);
    rgctl_lang_hcl::register(registry);
    // Tier-2 (`handler = "queries"`)
    rgctl_lang_zig::register(registry);
    rgctl_lang_nix::register(registry);
    rgctl_lang_proto::register(registry);
    rgctl_lang_solidity::register(registry);
}

/// Default registry with config formats and all built-in languages.
pub fn default_registry() -> LanguageRegistry {
    let mut registry = LanguageRegistry::with_config_formats();
    register_languages(&mut registry);
    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn default_registry_registers_markdown() {
        let registry = default_registry();
        assert!(registry.has_plugin("markdown"));
        let plugin = registry
            .get_plugin_for_file(Path::new("docs/guide.md"))
            .expect("md plugin");
        assert_eq!(plugin.language_id(), "markdown");
        let mdx = registry
            .get_plugin_for_file(Path::new("docs/overview.mdx"))
            .expect("mdx plugin");
        assert_eq!(mdx.language_id(), "markdown");
    }

    #[test]
    fn default_registry_can_process_markdown_files() {
        let registry = default_registry();
        assert!(registry.can_process_file(Path::new("README.md")));
        assert!(registry.can_process_file(Path::new("notes/page.mdx")));
        let langs = registry.supported_languages();
        assert!(langs.iter().any(|l| l == "markdown"));
    }

    #[test]
    fn default_registry_can_process_php_files() {
        let registry = default_registry();
        assert!(registry.can_process_file(Path::new("app/Service.php")));
        let plugin = registry
            .get_plugin_for_file(Path::new("src/User.php"))
            .expect("php plugin");
        assert_eq!(plugin.language_id(), "php");
    }

    #[test]
    fn default_registry_can_process_ruby_files() {
        let registry = default_registry();
        assert!(registry.can_process_file(Path::new("app/models/user.rb")));
        let plugin = registry
            .get_plugin_for_file(Path::new("lib/order.rb"))
            .expect("ruby plugin");
        assert_eq!(plugin.language_id(), "ruby");
    }

    #[test]
    fn default_registry_can_process_puppet_files() {
        let registry = default_registry();
        assert!(registry.can_process_file(Path::new("modules/nginx/manifests/init.pp")));
        let plugin = registry
            .get_plugin_for_file(Path::new("manifests/site.pp"))
            .expect("puppet plugin");
        assert_eq!(plugin.language_id(), "puppet");
    }

    #[test]
    fn default_registry_can_process_erb_files() {
        let registry = default_registry();
        assert!(registry.can_process_file(Path::new("modules/nginx/templates/nginx.conf.erb")));
        let plugin = registry
            .get_plugin_for_file(Path::new("templates/index.html.erb"))
            .expect("erb plugin");
        assert_eq!(plugin.language_id(), "erb");
    }

    #[test]
    fn default_registry_can_process_kotlin_files() {
        let registry = default_registry();
        assert!(registry.can_process_file(Path::new("src/main/kotlin/App.kt")));
        let plugin = registry
            .get_plugin_for_file(Path::new("UserService.kt"))
            .expect("kotlin plugin");
        assert_eq!(plugin.language_id(), "kotlin");
        // Manifest basename wins over `.kts` language extension
        assert!(registry.is_manifest_file(Path::new("build.gradle.kts")));
        assert!(registry.get_plugin_for_file(Path::new("build.gradle.kts")).is_err());
    }

    #[test]
    fn default_registry_can_process_groovy_files() {
        let registry = default_registry();
        assert!(registry.can_process_file(Path::new("src/Deploy.groovy")));
        let plugin = registry
            .get_plugin_for_file(Path::new("scripts/Job.groovy"))
            .expect("groovy plugin");
        assert_eq!(plugin.language_id(), "groovy");
        assert!(registry.is_manifest_file(Path::new("build.gradle")));
        assert!(registry.get_plugin_for_file(Path::new("build.gradle")).is_err());
    }

    #[test]
    fn default_registry_can_process_tier2_query_languages() {
        let registry = default_registry();
        for (path, id) in [
            ("src/main.zig", "zig"),
            ("flake.nix", "nix"),
            ("api/v1.proto", "proto"),
            ("contracts/Token.sol", "solidity"),
        ] {
            assert!(
                registry.can_process_file(Path::new(path)),
                "expected can_process {path}"
            );
            let plugin = registry
                .get_plugin_for_file(Path::new(path))
                .unwrap_or_else(|_| panic!("{id} plugin for {path}"));
            assert_eq!(plugin.language_id(), id);
        }
    }

    #[test]
    fn default_registry_can_process_new_tier1_languages() {
        let registry = default_registry();
        for (path, id) in [
            ("Sources/App/Main.swift", "swift"),
            ("src/main/scala/App.scala", "scala"),
            ("scripts/Job.sc", "scala"),
            ("lib/main.dart", "dart"),
            ("lib/app.ex", "elixir"),
            ("test/app_test.exs", "elixir"),
            ("src/util.lua", "lua"),
            ("scripts/setup.sh", "bash"),
            ("scripts/run.bash", "bash"),
            ("db/schema.sql", "sql"),
            ("infra/main.tf", "hcl"),
            ("infra/vars.tfvars", "hcl"),
            ("config/app.hcl", "hcl"),
        ] {
            assert!(
                registry.can_process_file(Path::new(path)),
                "expected can_process {path}"
            );
            let plugin = registry
                .get_plugin_for_file(Path::new(path))
                .unwrap_or_else(|_| panic!("{id} plugin for {path}"));
            assert_eq!(plugin.language_id(), id, "path {path}");
        }
    }

    #[test]
    fn markdown_not_treated_as_yaml_config() {
        let registry = default_registry();
        assert!(
            registry
                .get_config_plugin_for_file(Path::new("readme.md"))
                .is_err()
        );
    }

    /// Bi-directional sync: each Tier-1 plugin's `declared_ast_handlers()` vs
    /// `{lang}-ast-coverage.json` (non-tautological — handlers come from plugin code).
    #[test]
    fn tier1_declared_ast_handlers_sync_with_coverage_manifests() {
        let registry = default_registry();
        let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut failures = Vec::new();

        for spec in rgctl_ast_coverage::bundled_specs() {
            if spec.id == "markdown" {
                // Markdown stays on the legacy multi-pass path (not CST jump-table).
                continue;
            }
            let Some(plugin) = registry.get_language_plugin(spec.id) else {
                failures.push(format!("{}: not in default_registry()", spec.id));
                continue;
            };
            let Some(table) = plugin.declared_ast_handlers() else {
                failures.push(format!(
                    "{}: declared_ast_handlers() returned None",
                    spec.id
                ));
                continue;
            };
            let manifest_path = crates_dir
                .join(spec.crate_dir)
                .join(spec.manifest_file);
            let manifest_json = match std::fs::read_to_string(&manifest_path) {
                Ok(s) => s,
                Err(e) => {
                    failures.push(format!("{}: read {}: {e}", spec.id, manifest_path.display()));
                    continue;
                }
            };
            let lang = (spec.language)();
            let issues = rgctl_ast_coverage::check_plugin_manifest_sync(
                spec.id,
                &manifest_json,
                table,
                &lang,
            );
            for issue in issues {
                failures.push(format!("{}: {}", spec.id, issue.message));
            }
        }

        assert!(
            failures.is_empty(),
            "plugin ↔ ast-coverage sync failures:\n{}",
            failures.join("\n")
        );
    }
}
