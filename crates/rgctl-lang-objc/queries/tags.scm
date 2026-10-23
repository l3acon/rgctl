; Tier-2 Objective-C tags for rgctl (`handler = "queries"`).

(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

(method_definition
  (identifier) @name) @definition.method

(method_definition
  (keyword_declarator
    .
    (identifier) @name)) @definition.method

(class_interface
  .
  (identifier) @name) @definition.class

(class_implementation
  .
  (identifier) @name) @definition.class

(call_expression
  function: (identifier) @name) @reference.call

(message_expression
  method: (identifier) @name) @reference.call

