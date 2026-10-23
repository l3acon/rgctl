; Tier-2 CUDA tags for rgctl.

(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

(struct_specifier
  name: (type_identifier) @name) @definition.class

(class_specifier
  name: (type_identifier) @name) @definition.class

(preproc_include
  path: (_) @import.path)

(call_expression
  function: (identifier) @name) @reference.call

