; Tier-2 Fortran tags for rgctl (`handler = "queries"`).

(function_statement (name) @name) @definition.function

(subroutine_statement (name) @name) @definition.function

(module_statement (name) @name) @definition.module

(derived_type_definition
  (derived_type_statement
    (type_name) @name)) @definition.class

(call_expression (identifier) @name) @reference.call

(subroutine_call
  subroutine: (identifier) @name) @reference.call

(use_statement
  (module_name) @import.path)

