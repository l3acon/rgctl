; Tier-2 Solidity tags for rgctl (`handler = "queries"`).

(contract_declaration
  name: (identifier) @name) @definition.class

(function_definition
  name: (identifier) @name) @definition.function

(call_expression
  function: (expression (identifier) @name)) @reference.call
