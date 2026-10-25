; Tier-2 Salesforce Apex tags for rgctl (`handler = "queries"`).

(class_declaration
  name: (identifier) @name) @definition.class

(interface_declaration
  name: (identifier) @name) @definition.interface

(enum_declaration
  name: (identifier) @name) @definition.enum

(trigger_declaration
  name: (identifier) @name) @definition.module

(method_declaration
  name: (identifier) @name) @definition.method

(constructor_declaration
  name: (identifier) @name) @definition.method

(method_invocation
  name: (identifier) @name) @reference.call

(superclass
  (type_identifier) @name) @reference.class

(interfaces
  (type_list
    (type_identifier) @name)) @reference.implementation
