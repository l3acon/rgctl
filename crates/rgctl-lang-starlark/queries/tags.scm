; Tier-2 Starlark tags for rgctl.

(function_definition
  name: (identifier) @name) @definition.function

(call
  function: (identifier) @name) @reference.call

(call
  function: (identifier) @_load (#eq? @_load "load")
  arguments: (argument_list
    . (string) @import.path))
