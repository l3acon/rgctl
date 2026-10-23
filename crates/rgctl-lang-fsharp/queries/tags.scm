; Tier-2 F# tags for rgctl.

(function_or_value_defn
  (function_declaration_left
    . (identifier) @name)) @definition.function

(module_defn
  (identifier) @name) @definition.module

(type_definition
  (_
    (type_name
      type_name: (_) @name))) @definition.class

(import_decl
  (long_identifier) @import.path)

(application_expression
  . (long_identifier_or_op) @name) @reference.call
