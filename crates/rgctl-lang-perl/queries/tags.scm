; Tier-2 Perl tags for rgctl (`handler = "queries"`).
; Grammar: ts-parser-perl (tree-sitter-perl 1.1 is ABI-incompatible with tree-sitter 0.25).

(package_statement
  name: (package) @name) @definition.module

(subroutine_declaration_statement
  name: (bareword) @name) @definition.function

(method_declaration_statement
  name: (bareword) @name) @definition.function

(use_statement
  module: (package) @import.path)

(require_expression
  (bareword) @import.path)

(function_call_expression
  (function) @name) @reference.call

(method_call_expression
  (method) @name) @reference.call

(ambiguous_function_call_expression
  (function) @name) @reference.call
