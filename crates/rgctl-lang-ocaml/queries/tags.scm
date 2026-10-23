; Tier-2 OCaml tags for rgctl (`handler = "queries"`).

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

