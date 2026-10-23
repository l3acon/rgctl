; Tier-2 VHDL tags for rgctl.

(entity_declaration
  entity: (identifier) @name) @definition.module

(architecture_definition
  architecture: (identifier) @name) @definition.module

(component_declaration
  component: (identifier) @name) @definition.class

(subprogram_definition
  (function_specification
    function: (identifier) @name)) @definition.function

(subprogram_definition
  (procedure_specification
    procedure: (identifier) @name)) @definition.function

(procedure_call_statement
  (name) @name) @reference.call

(component_instantiation_statement
  (instantiated_unit
    entity: (name) @name)) @reference.class
