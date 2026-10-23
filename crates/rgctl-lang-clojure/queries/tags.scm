; Tier-2 Clojure tags for rgctl (`handler = "queries"`).

(list_lit
  . (sym_lit) @_def (#any-of? @_def "defn" "defn-" "defmacro")
  . (sym_lit) @name) @definition.function

(list_lit
  . (sym_lit) @_ns (#eq? @_ns "ns")
  . (sym_lit) @name) @definition.module

(list_lit
  . (sym_lit) @name
  (#not-any-of? @name "ns" "def" "defn" "defn-" "defmacro" "defprotocol" "defrecord" "deftype" "let" "if" "when" "cond" "do" "fn" "quote" "require" "import" "use")) @reference.call

