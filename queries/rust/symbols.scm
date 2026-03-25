; Rust symbol extraction queries
; Extract only language-agnostic concepts

; Functions (including methods in impl blocks)
(function_item
  name: (identifier) @symbol.name
) @symbol.function

; Structs (maps to "class" concept in other languages)
(struct_item
  name: (type_identifier) @symbol.name
) @symbol.struct
