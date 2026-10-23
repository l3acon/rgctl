; Tier-2 Racket tags for rgctl.

(list
  . (symbol) @_kw (#any-of? @_kw "define" "define-syntax")
  . (symbol) @name) @definition.function

(list
  . (symbol) @_kw (#eq? @_kw "define")
  . (list . (symbol) @name)) @definition.function

(list
  . (symbol) @_kw (#eq? @_kw "struct")
  . (symbol) @name) @definition.class

(list
  . (symbol) @_kw (#eq? @_kw "require")
  . (_) @import.path)

(list
  . (symbol) @name
  (#not-any-of? @name "define" "define-syntax" "lambda" "let" "let*" "letrec" "if" "cond" "begin" "quote" "set!" "struct" "require" "provide" "module")) @reference.call

