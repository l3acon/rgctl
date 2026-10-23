; Tier-2 Erlang tags for rgctl.

(fun_decl
  clause: (function_clause
    name: (atom) @name)) @definition.function

(module_attribute
  (atom) @name) @definition.module

(record_decl
  name: (atom) @name) @definition.class

(type_alias
  name: (type_name
    name: (atom) @name)) @definition.type

(call
  expr: (atom) @name) @reference.call

(call
  expr: (remote
    fun: (atom) @name)) @reference.call

