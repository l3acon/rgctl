; Tier-2 Zig tags for rgctl (`handler = "queries"`).
; Capture vocabulary: tree-sitter code-navigation + import.path.

(function_declaration
  name: (identifier) @name) @definition.function

; Zig structs are typically `const Name = struct { ... }` — name is on the var decl.
(variable_declaration
  (identifier) @name
  (struct_declaration)) @definition.class

(variable_declaration
  (identifier) @name
  (builtin_function)) @definition.constant

(call_expression
  function: (identifier) @name) @reference.call

(builtin_function
  (builtin_identifier) @_bi
  (#eq? @_bi "@import")
  (arguments
    (string
      (string_content) @import.path)))
