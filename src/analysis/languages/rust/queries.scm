; Tree-sitter queries for extracting Rust code relationships
; Used by Phase 2 relationship extraction

; ========================================
; 1. FUNCTION CALLS
; ========================================

; Simple function call: foo()
(call_expression
  function: (identifier) @call.simple) @call.expr

; Method call: obj.method()
(call_expression
  function: (field_expression
    field: (field_identifier) @call.method)) @call.expr

; Qualified call: Module::function()
(call_expression
  function: (scoped_identifier) @call.qualified) @call.expr

; ========================================
; 2. TYPE REFERENCES
; ========================================

; Function parameter types
(function_item
  name: (identifier) @func.name
  parameters: (parameters
    (parameter
      type: (_) @param.type)))

; Function return types  
(function_item
  name: (identifier) @func.name
  return_type: (_) @return.type)

; Variable type annotations
(let_declaration
  type: (_) @let.type)

; ========================================
; 3. TRAIT IMPLEMENTATIONS
; ========================================

; Trait implementation: impl Trait for Type
(impl_item
  trait: (type_identifier) @trait.name
  "for"
  type: (_) @impl.type) @trait.impl

; ========================================
; 4. SYMBOL CONTAINMENT
; ========================================

; Methods in impl blocks
(impl_item
  type: (_) @impl.target
  body: (declaration_list
    (function_item
      name: (identifier) @method.name))) @impl.block
