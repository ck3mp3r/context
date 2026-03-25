; Rust call expression extraction
; Extract function/method calls for building the Calls relationship graph

; Simple function calls: foo()
(call_expression
  function: (identifier) @call.target
) @call.site

; Method calls: obj.method(), self.foo()
(call_expression
  function: (field_expression
    field: (field_identifier) @call.target)
) @call.site

; Scoped/static calls: Type::method(), module::function()
(call_expression
  function: (scoped_identifier
    "::"
    name: (identifier) @call.target)
) @call.site

; Generic function calls: foo::<T>()
(generic_function
  function: (identifier) @call.target
) @call.site

; Generic method calls: obj.method::<T>()
(generic_function
  function: (field_expression
    field: (field_identifier) @call.target)
) @call.site

; Generic scoped calls: Type::method::<T>()
(generic_function
  function: (scoped_identifier
    name: (identifier) @call.target)
) @call.site

; Macro invocations: println!(), vec![]
(macro_invocation
  macro: (identifier) @call.target
  "!" @call.macro_marker
) @call.site
