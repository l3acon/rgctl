; Tier-2 Pascal/Delphi tags for rgctl (`handler = "queries"`).

(unit
  (moduleName
    (identifier) @name)) @definition.module

(program
  (moduleName
    (identifier) @name)) @definition.module

(library
  (moduleName
    (identifier) @name)) @definition.module

(declType
  (identifier) @name
  (declClass)) @definition.class

(declType
  (identifier) @name
  (declIntf)) @definition.interface

(declType
  (identifier) @name
  [(declEnum) (declSet) (declArray) (declString) (declFile) (declProcRef)]) @definition.type

(declProc
  (identifier) @name) @definition.function

(defProc
  header: (declProc
    (genericDot
      (identifier) @_cls
      (identifier) @name))) @definition.method

(declUses
  (moduleName
    (identifier) @import.path))

(exprCall
  entity: (identifier) @name) @reference.call
