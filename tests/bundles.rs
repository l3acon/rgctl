//! Tier 1 language plugin integration tests

use rgctl::languages::registry::LanguageRegistry;
use std::path::Path;

#[test]
fn test_all_tier1_plugins_registered() {
    let registry = LanguageRegistry::new();
    // Full catalog (~60+) is registered by default; keep a floor so we notice
    // accidental shrink without freezing the exact count on every language add.
    assert!(
        registry.stats().language_plugins >= 60,
        "expected full language catalog, got {}",
        registry.stats().language_plugins
    );
    for id in [
        "rust",
        "python",
        "javascript",
        "typescript",
        "go",
        "java",
        "csharp",
        "c",
        "cpp",
        "markdown",
        // legacy / enterprise migration languages (#143)
        "cobol",
        "pli",
        "jcl",
        "apex",
        "pascal",
        "perl",
        "ada",
        "matlab",
    ] {
        assert!(registry.has_plugin(id), "missing plugin {id}");
    }
}

#[test]
fn test_extension_routing() {
    let registry = LanguageRegistry::new();
    for (file, id) in [
        ("main.rs", "rust"),
        ("app.py", "python"),
        ("index.js", "javascript"),
        ("index.ts", "typescript"),
        ("main.go", "go"),
        ("App.java", "java"),
        ("Program.cs", "csharp"),
        ("main.c", "c"),
        ("types.h", "c"),
        ("main.cpp", "cpp"),
        ("types.hpp", "cpp"),
    ] {
        let plugin = registry
            .get_plugin_for_file(Path::new(file))
            .unwrap_or_else(|_| panic!("no plugin for {file}"));
        assert_eq!(plugin.language_id(), id);
    }
}
