; Tier-2 D tags for rgctl.

(function_declaration
  (identifier) @name) @definition.function

(class_declaration
  (identifier) @name) @definition.class

(struct_declaration
  (identifier) @name) @definition.class

(interface_declaration
  (identifier) @name) @definition.interface

(module_declaration
  (module_fqn) @name) @definition.module

(import_declaration
  (imported
    (module_fqn) @import.path))

(call_expression
  . (identifier) @name) @reference.call
