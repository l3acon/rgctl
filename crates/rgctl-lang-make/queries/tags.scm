; Tier-2 Make tags for rgctl.

(rule
  (targets) @name) @definition.function

(variable_assignment
  name: (word) @name) @definition.constant

(include_directive
  filenames: (_) @import.path)

