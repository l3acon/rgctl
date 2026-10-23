; Tier-2 Emacs Lisp tags for rgctl.

(function_definition
  name: (symbol) @name) @definition.function

(macro_definition
  name: (symbol) @name) @definition.macro

(list
  . (symbol) @_req (#eq? @_req "require")
  . (quote (symbol) @import.path))

(list
  . (symbol) @name
  (#not-any-of? @name "let" "let*" "if" "when" "unless" "cond" "progn" "setq" "defun" "defmacro" "defvar" "defcustom" "lambda" "quote" "require" "provide")) @reference.call
