; Tier-2 Gleam tags for rgctl.

(function
  name: (identifier) @name) @definition.function

(type_definition
  (type_name) @name) @definition.class

(type_alias
  (type_name) @name) @definition.type

(import
  module: (module) @import.path)

(function_call
  function: (identifier) @name) @reference.call

