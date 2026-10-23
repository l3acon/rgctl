; Tier-2 Odin tags for rgctl.

(procedure_declaration
  (identifier) @name) @definition.function

(struct_declaration
  (identifier) @name) @definition.class

(enum_declaration
  (identifier) @name) @definition.enum

(package_declaration
  (identifier) @name) @definition.module

(import_declaration
  (string) @import.path)

(call_expression
  function: (identifier) @name) @reference.call
