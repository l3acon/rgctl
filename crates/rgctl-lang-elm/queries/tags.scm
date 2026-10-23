; Tier-2 Elm tags for rgctl.

(module_declaration
  (upper_case_qid) @name) @definition.module

(value_declaration
  (function_declaration_left
    (lower_case_identifier) @name)) @definition.function

(type_declaration
  (upper_case_identifier) @name) @definition.class

(type_alias_declaration
  (upper_case_identifier) @name) @definition.type

(import_clause
  (upper_case_qid) @import.path)

(function_call_expr
  (value_expr
    (value_qid
      (lower_case_identifier) @name))) @reference.call

