; Tier-2 Ada tags for rgctl (`handler = "queries"`).

(package_declaration
  name: (identifier) @name) @definition.module

(package_body
  name: (identifier) @name) @definition.module

(subprogram_body
  (function_specification
    name: (identifier) @name)) @definition.function

(subprogram_body
  (procedure_specification
    name: (identifier) @name)) @definition.function

(subprogram_declaration
  (function_specification
    name: (identifier) @name)) @definition.function

(subprogram_declaration
  (procedure_specification
    name: (identifier) @name)) @definition.function

(full_type_declaration
  (identifier) @name) @definition.type

(private_type_declaration
  (identifier) @name) @definition.type

(task_type_declaration
  (identifier) @name) @definition.class

(protected_type_declaration
  (identifier) @name) @definition.class

(with_clause
  [(identifier) (selected_component)] @import.path)

(function_call
  (identifier) @name) @reference.call

(procedure_call_statement
  name: (identifier) @name) @reference.call
