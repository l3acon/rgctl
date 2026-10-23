; Tier-2 Scheme tags for rgctl.

(list
  . (symbol) @_kw (#any-of? @_kw "define" "define-syntax")
  . (symbol) @name) @definition.function

(list
  . (symbol) @_kw (#eq? @_kw "define")
  . (list . (symbol) @name)) @definition.function

(list
  . (symbol) @_kw (#eq? @_kw "define-library")
  . (list . (symbol) @name)) @definition.module

(list
  . (symbol) @name
  (#not-any-of? @name "define" "define-syntax" "define-library" "library" "lambda" "let" "let*" "letrec" "if" "cond" "begin" "quote" "set!" "import" "export")) @reference.call

