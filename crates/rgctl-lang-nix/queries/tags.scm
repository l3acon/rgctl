; Tier-2 Nix tags for rgctl (`handler = "queries"`).

(binding
  attrpath: (attrpath attr: (identifier) @name)
  expression: (function_expression)) @definition.function

(apply_expression
  function: (variable_expression name: (identifier) @name)) @reference.call
