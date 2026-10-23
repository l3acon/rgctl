; Tier-2 Haskell tags for rgctl (`handler = "queries"`).

(function
  name: [
    (variable) @name
    (prefix_id) @name
  ]) @definition.function

(module
  (module_id) @name) @definition.module

(class
  name: (name) @name) @definition.interface

(data_type
  name: (name) @name) @definition.class

(newtype
  name: (name) @name) @definition.type

(apply
  function: (variable) @name) @reference.call

(import
  module: (module) @import.path)
