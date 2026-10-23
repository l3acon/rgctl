#!/usr/bin/env python3
"""Scaffold Tier-2 (`handler = queries`) language crates from a template."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"

LANGS = [
    {
        "id": "json",
        "title": "JSON",
        "dep": 'tree-sitter-json = "0.24"',
        "mod": "tree_sitter_json",
        "lang": "LANGUAGE",
        "extensions": ["json"],
        "aliases": ["json"],
        "function_kinds": [],
        "class_kinds": ["object"],
        "tags": """; Tier-2 JSON tags for rgctl (`handler = "queries"`).

(pair
  key: (string (string_content) @name)) @definition.constant
""",
        "test_src": '{"name": "rgctl", "version": 1}',
        "test_file": "package.json",
        "test_assert": "name",
    },
    {
        "id": "yaml",
        "title": "YAML",
        "dep": 'tree-sitter-yaml = "0.7"',
        "mod": "tree_sitter_yaml",
        "lang": "LANGUAGE",
        "extensions": ["yaml", "yml"],
        "aliases": ["yaml", "yml"],
        "function_kinds": [],
        "class_kinds": [],
        "tags": """; Tier-2 YAML tags for rgctl (`handler = "queries"`).

(block_mapping_pair
  key: (flow_node (plain_scalar (string_scalar) @name))) @definition.constant

(block_mapping_pair
  key: (flow_node (double_quote_scalar) @name)) @definition.constant
""",
        "test_src": "name: rgctl\nversion: 1\n",
        "test_file": "config.yaml",
        "test_assert": "name",
    },
    {
        "id": "julia",
        "title": "Julia",
        "dep": 'tree-sitter-julia = "0.23"',
        "mod": "tree_sitter_julia",
        "lang": "LANGUAGE",
        "extensions": ["jl"],
        "aliases": ["julia", "jl"],
        "function_kinds": ["function_definition"],
        "class_kinds": ["struct_definition"],
        "tags": """; Tier-2 Julia tags for rgctl (`handler = "queries"`).

(function_definition
  (identifier) @name) @definition.function

(struct_definition
  (identifier) @name) @definition.class

(module_definition
  (identifier) @name) @definition.module

(call_expression
  (identifier) @name) @reference.call

(macro_definition
  (identifier) @name) @definition.macro
""",
        "test_src": "module M\nfunction add(a, b)\n    return a + b\nend\nend\n",
        "test_file": "math.jl",
        "test_assert": "add",
    },
    {
        "id": "r",
        "title": "R",
        "dep": 'tree-sitter-r = "1"',
        "mod": "tree_sitter_r",
        "lang": "LANGUAGE",
        "extensions": ["r", "R"],
        "aliases": ["r", "R"],
        "function_kinds": ["function_definition"],
        "class_kinds": [],
        "tags": """; Tier-2 R tags for rgctl (`handler = "queries"`).

(binary_operator
    lhs: (identifier) @name
    operator: "<-"
    rhs: (function_definition)
) @definition.function

(binary_operator
    lhs: (identifier) @name
    operator: "="
    rhs: (function_definition)
) @definition.function

(call
    function: (identifier) @name
) @reference.call
""",
        "test_src": "add <- function(a, b) {\n  a + b\n}\nadd(1, 2)\n",
        "test_file": "math.R",
        "test_assert": "add",
    },
    {
        "id": "haskell",
        "title": "Haskell",
        "dep": 'tree-sitter-haskell = "0.23"',
        "mod": "tree_sitter_haskell",
        "lang": "LANGUAGE",
        "extensions": ["hs"],
        "aliases": ["haskell", "hs"],
        "function_kinds": ["function"],
        "class_kinds": ["class_decl"],
        "tags": """; Tier-2 Haskell tags for rgctl (`handler = "queries"`).

(function (variable) @name) @definition.function

(class_decl (class_head (class_name) @name)) @definition.class

(module (module_id) @name) @definition.module

(exp_apply (exp_name (variable) @name)) @reference.call
""",
        "test_src": "module Math where\nadd :: Int -> Int -> Int\nadd a b = a + b\n",
        "test_file": "Math.hs",
        "test_assert": "add",
    },
    {
        "id": "gdscript",
        "title": "GDScript",
        "dep": 'tree-sitter-gdscript = "6"',
        "mod": "tree_sitter_gdscript",
        "lang": "LANGUAGE",
        "extensions": ["gd"],
        "aliases": ["gdscript", "godot"],
        "function_kinds": ["function_definition"],
        "class_kinds": ["class_definition"],
        "tags": """; Tier-2 GDScript tags for rgctl (`handler = "queries"`).

(function_definition (name) @name) @definition.function

(class_definition (name) @name) @definition.class

(call (identifier) @name) @reference.call
""",
        "test_src": "func add(a, b):\n\treturn a + b\n",
        "test_file": "math.gd",
        "test_assert": "add",
    },
    {
        "id": "fortran",
        "title": "Fortran",
        "dep": 'tree-sitter-fortran = "0.5"',
        "mod": "tree_sitter_fortran",
        "lang": "LANGUAGE",
        "extensions": ["f90", "f95", "f03", "f08", "F90"],
        "aliases": ["fortran", "f90"],
        "function_kinds": ["function", "subroutine"],
        "class_kinds": ["derived_type_definition"],
        "tags": """; Tier-2 Fortran tags for rgctl (`handler = "queries"`).

(function_statement (name) @name) @definition.function

(subroutine_statement (name) @name) @definition.function

(module_statement (name) @name) @definition.module

(call_expression (identifier) @name) @reference.call
""",
        "test_src": (
            "module math\ncontains\n  function add(a, b) result(c)\n"
            "    integer :: a, b, c\n    c = a + b\n  end function\nend module\n"
        ),
        "test_file": "math.f90",
        "test_assert": "add",
    },
    {
        "id": "verilog",
        "title": "Verilog",
        "dep": 'tree-sitter-verilog = "1"',
        "mod": "tree_sitter_verilog",
        "lang": "LANGUAGE",
        "extensions": ["v", "sv", "svh"],
        "aliases": ["verilog", "systemverilog", "sv"],
        "function_kinds": ["function_declaration"],
        "class_kinds": ["class_declaration", "module_declaration"],
        "tags": """; Tier-2 Verilog/SystemVerilog tags for rgctl (`handler = "queries"`).

(module_declaration
  (module_header (simple_identifier) @name)) @definition.module

(function_declaration
  (function_body_declaration
    (function_identifier (simple_identifier) @name))) @definition.function

(class_declaration
  (class_identifier (simple_identifier) @name)) @definition.class
""",
        "test_src": "module adder;\nendmodule\n",
        "test_file": "adder.v",
        "test_assert": "adder",
    },
    {
        "id": "objc",
        "title": "Objective-C",
        "dep": 'tree-sitter-objc = "3"',
        "mod": "tree_sitter_objc",
        "lang": "LANGUAGE",
        "extensions": ["m", "mm"],
        "aliases": ["objc", "objective-c", "objectivec"],
        "function_kinds": ["function_definition", "method_definition"],
        "class_kinds": ["class_interface", "class_implementation"],
        "tags": """; Tier-2 Objective-C tags for rgctl (`handler = "queries"`).

(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

(method_definition
  (identifier) @name) @definition.method

(class_interface
  (identifier) @name) @definition.class

(class_implementation
  (identifier) @name) @definition.class

(call_expression
  function: (identifier) @name) @reference.call
""",
        "test_src": (
            "@interface Foo : NSObject\n- (void)bar;\n@end\n"
            "@implementation Foo\n- (void)bar {}\n@end\n"
        ),
        "test_file": "Foo.m",
        "test_assert": "Foo",
    },
    {
        "id": "clojure",
        "title": "Clojure",
        "dep": 'tree-sitter-clojure = "0.1"',
        "mod": "tree_sitter_clojure",
        "lang": "LANGUAGE",
        "extensions": ["clj", "cljs", "cljc"],
        "aliases": ["clojure", "clj"],
        "function_kinds": ["list_lit"],
        "class_kinds": [],
        "tags": """; Tier-2 Clojure tags for rgctl (`handler = "queries"`).

(list_lit
  . (sym_lit) @_def (#any-of? @_def "defn" "defn-" "defmacro")
  . (sym_lit) @name) @definition.function

(list_lit
  . (sym_lit) @_ns (#eq? @_ns "ns")
  . (sym_lit) @name) @definition.module
""",
        "test_src": "(ns math)\n(defn add [a b] (+ a b))\n",
        "test_file": "math.clj",
        "test_assert": "add",
    },
    {
        "id": "ocaml",
        "title": "OCaml",
        "dep": 'tree-sitter-ocaml = "0.24"',
        "mod": "tree_sitter_ocaml",
        "lang": "LANGUAGE_OCAML",
        "extensions": ["ml"],
        "aliases": ["ocaml", "ml"],
        "function_kinds": ["value_definition"],
        "class_kinds": ["class_definition"],
        "tags": """; Tier-2 OCaml tags for rgctl (`handler = "queries"`).

(module_definition
  (module_binding (module_name) @name)) @definition.module

(value_definition
  (let_binding
    pattern: (value_name) @name
    body: [(fun_expression) (function_expression)])) @definition.function

(value_definition
  (let_binding pattern: (value_name) @name (parameter))) @definition.function

(application_expression
  function: (value_path (value_name) @name)) @reference.call

(class_definition
  (class_binding (class_name) @name)) @definition.class

(method_definition (method_name) @name) @definition.method
""",
        "test_src": "let add a b = a + b\n",
        "test_file": "math.ml",
        "test_assert": "add",
    },
]


def quote_list(items: list[str]) -> str:
    return ", ".join(f'"{x}"' for x in items)


def write_crate(lang: dict) -> None:
    cid = lang["id"]
    crate = f"rgctl-lang-{cid}"
    crate_dir = CRATES / crate
    (crate_dir / "src").mkdir(parents=True, exist_ok=True)
    (crate_dir / "queries").mkdir(parents=True, exist_ok=True)

    (crate_dir / "Cargo.toml").write_text(
        f"""[package]
name = "{crate}"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
description = "rgctl Tier-2 language plugin: {cid} (tags.scm queries)"
license = "MIT OR Apache-2.0"

[dependencies]
rgctl-ast-coverage = {{ workspace = true }}
rgctl-lang-runtime = {{ workspace = true }}
rgctl-plugin-api = {{ workspace = true }}
rgctl-registry = {{ workspace = true }}
tree-sitter = {{ workspace = true }}
{lang["dep"]}

[lints]
workspace = true
"""
    )
    (crate_dir / "queries" / "tags.scm").write_text(lang["tags"].lstrip() + "\n")

    (crate_dir / "src" / "config.rs").write_text(
        f"""//! Static Tier-2 config for {cid} (`handler = "queries"`).

use rgctl_lang_runtime::LanguageConfig;

/// Embedded `queries/tags.scm`.
pub const TAGS_SCM: &str = include_str!("../queries/tags.scm");

/// {lang["title"]} language configuration for [`rgctl_lang_runtime::QueryLanguagePlugin`].
pub static CONFIG: LanguageConfig = LanguageConfig {{
    id: "{cid}",
    extensions: &[{quote_list(lang["extensions"])}],
    function_kinds: &[{quote_list(lang["function_kinds"])}],
    class_kinds: &[{quote_list(lang["class_kinds"])}],
    enable_complexity: false,
    enable_type_inference: false,
    regex_patterns: None,
    tags_scm: Some(TAGS_SCM),
}};
"""
    )

    grammar_fn = f"|| {lang['mod']}::{lang['lang']}.into()"
    src = lang["test_src"]
    (crate_dir / "src" / "lib.rs").write_text(
        f"""//! {lang["title"]} Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{{CONFIG, TAGS_SCM}};

/// Register this Tier-2 query plugin.
pub fn register(registry: &mut LanguageRegistry) {{
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, {grammar_fn})
        .expect("compile {cid} tags.scm");
    registry.register_language_plugin(Arc::new(plugin));
}}

#[cfg(test)]
mod tests {{
    use super::*;
    use rgctl_ast_coverage::check_tags_scm;
    use rgctl_plugin_api::LanguagePlugin;
    use std::path::Path;

    #[test]
    fn tags_scm_passes_ast_coverage_vocabulary_gate() {{
        let issues = check_tags_scm("{cid}", TAGS_SCM);
        assert!(issues.is_empty(), "{{issues:?}}");
    }}

    #[test]
    fn extract_smoke() {{
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, {grammar_fn}).expect("compile");
        let source = br###"{src}"###;
        let result = plugin
            .extract_all(Path::new("{lang["test_file"]}"), source)
            .expect("extract_all");
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name.contains("{lang["test_assert"]}")),
            "expected symbol containing '{lang["test_assert"]}', got {{:?}}",
            result.symbols
        );
    }}
}}
"""
    )


def main() -> None:
    for lang in LANGS:
        write_crate(lang)
        print(f"created rgctl-lang-{lang['id']}")


if __name__ == "__main__":
    main()
