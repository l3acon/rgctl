; Tier-2 GLSL tags for rgctl.

(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

(struct_specifier
  name: (type_identifier) @name) @definition.class

(call_expression
  function: (identifier) @name) @reference.call

