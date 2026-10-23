; Tier-2 Julia tags for rgctl (`handler = "queries"`).

(function_definition
  (signature
    (call_expression
      (identifier) @name))) @definition.function

(function_definition
  (signature
    (identifier) @name)) @definition.function

(module_definition
  name: (identifier) @name) @definition.module

(struct_definition
  (type_head
    (identifier) @name)) @definition.class

(struct_definition
  (type_head
    (binary_expression
      .
      (identifier) @name))) @definition.class

(struct_definition
  (type_head
    (parametrized_type_expression
      .
      (identifier) @name))) @definition.class

(macro_definition
  (signature
    (call_expression
      (identifier) @name))) @definition.macro

(import_statement
  (import_path) @import.path)

(using_statement
  (import_path) @import.path)

(call_expression
  .
  (identifier) @name) @reference.call
