;;; ==========================================================================
;;; Go Type Reference Queries
;;; ==========================================================================
;;; Captures type references from function params, returns, and struct fields.
;;; Built-in types (string, int, error, etc.) are filtered in the parser.

;;; ==========================================================================
;;; FUNCTION PARAMETER TYPES
;;; ==========================================================================

;;; parameter — direct type (func foo(x Foo))
(function_declaration
    name: (identifier) @fn_param_direct_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (type_identifier) @fn_param_direct_type))) @fn_param_direct_def

;;; parameter — pointer type (func foo(x *Foo))
(function_declaration
    name: (identifier) @fn_param_ptr_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (pointer_type
                (type_identifier) @fn_param_ptr_type)))) @fn_param_ptr_def

;;; parameter — slice type (func foo(x []Foo))
(function_declaration
    name: (identifier) @fn_param_slice_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (slice_type
                element: (type_identifier) @fn_param_slice_type)))) @fn_param_slice_def

;;; parameter — slice of pointer type (func foo(x []*Foo))
(function_declaration
    name: (identifier) @fn_param_slice_ptr_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (slice_type
                element: (pointer_type
                    (type_identifier) @fn_param_slice_ptr_type))))) @fn_param_slice_ptr_def

;;; parameter — map value type (func foo(x map[K]V))
(function_declaration
    name: (identifier) @fn_param_map_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (map_type
                value: (type_identifier) @fn_param_map_type)))) @fn_param_map_def

;;; parameter — map key type (when key is user type)
(function_declaration
    name: (identifier) @fn_param_map_key_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (map_type
                key: (type_identifier) @fn_param_map_key_type)))) @fn_param_map_key_def

;;; parameter — qualified type (func foo(x pkg.Type))
(function_declaration
    name: (identifier) @fn_param_qual_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (qualified_type
                name: (type_identifier) @fn_param_qual_type)))) @fn_param_qual_def

;;; parameter — pointer to qualified type (func foo(x *pkg.Type))
(function_declaration
    name: (identifier) @fn_param_ptr_qual_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (pointer_type
                (qualified_type
                    name: (type_identifier) @fn_param_ptr_qual_type))))) @fn_param_ptr_qual_def

;;; parameter — channel type (func foo(x chan Foo))
(function_declaration
    name: (identifier) @fn_param_chan_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (channel_type
                value: (type_identifier) @fn_param_chan_type)))) @fn_param_chan_def

;;; parameter — generic type outer (func foo(x Container[T]))
(function_declaration
    name: (identifier) @fn_param_generic_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (generic_type
                type: (type_identifier) @fn_param_generic_outer)))) @fn_param_generic_def

;;; parameter — generic type inner argument (func foo(x Container[Item]))
;;; Note: type_arguments contains type_elem which contains the actual type
(function_declaration
    name: (identifier) @fn_param_generic_inner_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (generic_type
                type_arguments: (type_arguments
                    (type_elem
                        (type_identifier) @fn_param_generic_inner_type)))))) @fn_param_generic_inner_def

;;; parameter — variadic type (func foo(x ...Foo))
(function_declaration
    name: (identifier) @fn_param_variadic_fn
    parameters: (parameter_list
        (variadic_parameter_declaration
            type: (type_identifier) @fn_param_variadic_type))) @fn_param_variadic_def

;;; parameter — variadic pointer type (func foo(x ...*Foo))
(function_declaration
    name: (identifier) @fn_param_variadic_ptr_fn
    parameters: (parameter_list
        (variadic_parameter_declaration
            type: (pointer_type
                (type_identifier) @fn_param_variadic_ptr_type)))) @fn_param_variadic_ptr_def

;;; ==========================================================================
;;; METHOD RECEIVER TYPES (receiver is like first param)
;;; ==========================================================================

;;; method receiver — direct type (func (r Receiver) M())
(method_declaration
    receiver: (parameter_list
        (parameter_declaration
            type: (type_identifier) @method_recv_direct_type))
    name: (field_identifier) @method_recv_direct_fn) @method_recv_direct_def

;;; method receiver — pointer type (func (r *Receiver) M())
(method_declaration
    receiver: (parameter_list
        (parameter_declaration
            type: (pointer_type
                (type_identifier) @method_recv_ptr_type)))
    name: (field_identifier) @method_recv_ptr_fn) @method_recv_ptr_def

;;; method receiver — qualified type (func (r pkg.Receiver) M())
(method_declaration
    receiver: (parameter_list
        (parameter_declaration
            type: (qualified_type
                name: (type_identifier) @method_recv_qual_type)))
    name: (field_identifier) @method_recv_qual_fn) @method_recv_qual_def

;;; method receiver — pointer to qualified type (func (r *pkg.Receiver) M())
(method_declaration
    receiver: (parameter_list
        (parameter_declaration
            type: (pointer_type
                (qualified_type
                    name: (type_identifier) @method_recv_ptr_qual_type))))
    name: (field_identifier) @method_recv_ptr_qual_fn) @method_recv_ptr_qual_def

;;; ==========================================================================
;;; METHOD PARAMETER TYPES
;;; ==========================================================================

;;; method parameter — direct type
(method_declaration
    name: (field_identifier) @method_param_direct_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (type_identifier) @method_param_direct_type))) @method_param_direct_def

;;; method parameter — pointer type
(method_declaration
    name: (field_identifier) @method_param_ptr_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (pointer_type
                (type_identifier) @method_param_ptr_type)))) @method_param_ptr_def

;;; method parameter — slice type
(method_declaration
    name: (field_identifier) @method_param_slice_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (slice_type
                element: (type_identifier) @method_param_slice_type)))) @method_param_slice_def

;;; method parameter — qualified type
(method_declaration
    name: (field_identifier) @method_param_qual_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (qualified_type
                name: (type_identifier) @method_param_qual_type)))) @method_param_qual_def

;;; method parameter — pointer to qualified type
(method_declaration
    name: (field_identifier) @method_param_ptr_qual_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (pointer_type
                (qualified_type
                    name: (type_identifier) @method_param_ptr_qual_type))))) @method_param_ptr_qual_def

;;; method parameter — channel type
(method_declaration
    name: (field_identifier) @method_param_chan_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (channel_type
                value: (type_identifier) @method_param_chan_type)))) @method_param_chan_def

;;; method parameter — generic inner type
(method_declaration
    name: (field_identifier) @method_param_generic_inner_fn
    parameters: (parameter_list
        (parameter_declaration
            type: (generic_type
                type_arguments: (type_arguments
                    (type_elem
                        (type_identifier) @method_param_generic_inner_type)))))) @method_param_generic_inner_def

;;; ==========================================================================
;;; FUNCTION RETURN TYPES
;;; ==========================================================================

;;; return — direct type (func foo() Foo)
(function_declaration
    name: (identifier) @fn_ret_direct_fn
    result: (type_identifier) @fn_ret_direct_type) @fn_ret_direct_def

;;; return — pointer type (func foo() *Foo)
(function_declaration
    name: (identifier) @fn_ret_ptr_fn
    result: (pointer_type
        (type_identifier) @fn_ret_ptr_type)) @fn_ret_ptr_def

;;; return — slice type (func foo() []Foo)
(function_declaration
    name: (identifier) @fn_ret_slice_fn
    result: (slice_type
        element: (type_identifier) @fn_ret_slice_type)) @fn_ret_slice_def

;;; return — qualified type (func foo() pkg.Type)
(function_declaration
    name: (identifier) @fn_ret_qual_fn
    result: (qualified_type
        name: (type_identifier) @fn_ret_qual_type)) @fn_ret_qual_def

;;; return — pointer to qualified type (func foo() *pkg.Type)
(function_declaration
    name: (identifier) @fn_ret_ptr_qual_fn
    result: (pointer_type
        (qualified_type
            name: (type_identifier) @fn_ret_ptr_qual_type))) @fn_ret_ptr_qual_def

;;; return — tuple with direct types (func foo() (Foo, Bar))
(function_declaration
    name: (identifier) @fn_ret_tuple_fn
    result: (parameter_list
        (parameter_declaration
            type: (type_identifier) @fn_ret_tuple_type))) @fn_ret_tuple_def

;;; return — tuple with pointer types (func foo() (*Foo, *Bar))
(function_declaration
    name: (identifier) @fn_ret_tuple_ptr_fn
    result: (parameter_list
        (parameter_declaration
            type: (pointer_type
                (type_identifier) @fn_ret_tuple_ptr_type)))) @fn_ret_tuple_ptr_def

;;; return — tuple with slice types (func foo() ([]Foo, error))
(function_declaration
    name: (identifier) @fn_ret_tuple_slice_fn
    result: (parameter_list
        (parameter_declaration
            type: (slice_type
                element: (type_identifier) @fn_ret_tuple_slice_type)))) @fn_ret_tuple_slice_def

;;; return — tuple with slice of pointer types (func foo() ([]*Foo, error))
(function_declaration
    name: (identifier) @fn_ret_tuple_slice_ptr_fn
    result: (parameter_list
        (parameter_declaration
            type: (slice_type
                element: (pointer_type
                    (type_identifier) @fn_ret_tuple_slice_ptr_type))))) @fn_ret_tuple_slice_ptr_def

;;; return — tuple with slice of qualified types (func foo() ([]pkg.Type, error))
(function_declaration
    name: (identifier) @fn_ret_tuple_slice_qual_fn
    result: (parameter_list
        (parameter_declaration
            type: (slice_type
                element: (qualified_type
                    package: (package_identifier) @fn_ret_tuple_slice_qual_pkg
                    name: (type_identifier) @fn_ret_tuple_slice_qual_type))))) @fn_ret_tuple_slice_qual_def

;;; return — tuple with pointer to qualified types (func foo() (*pkg.Type, error))
(function_declaration
    name: (identifier) @fn_ret_tuple_ptr_qual_fn
    result: (parameter_list
        (parameter_declaration
            type: (pointer_type
                (qualified_type
                    name: (type_identifier) @fn_ret_tuple_ptr_qual_type))))) @fn_ret_tuple_ptr_qual_def

;;; return — generic type outer (func foo() Container[T])
(function_declaration
    name: (identifier) @fn_ret_generic_fn
    result: (generic_type
        type: (type_identifier) @fn_ret_generic_outer)) @fn_ret_generic_def

;;; return — generic type inner argument (func foo() Container[Item])
(function_declaration
    name: (identifier) @fn_ret_generic_inner_fn
    result: (generic_type
        type_arguments: (type_arguments
            (type_elem
                (type_identifier) @fn_ret_generic_inner_type)))) @fn_ret_generic_inner_def

;;; ==========================================================================
;;; METHOD RETURN TYPES
;;; ==========================================================================

;;; method return — direct type
(method_declaration
    name: (field_identifier) @method_ret_direct_fn
    result: (type_identifier) @method_ret_direct_type) @method_ret_direct_def

;;; method return — pointer type
(method_declaration
    name: (field_identifier) @method_ret_ptr_fn
    result: (pointer_type
        (type_identifier) @method_ret_ptr_type)) @method_ret_ptr_def

;;; method return — slice type
(method_declaration
    name: (field_identifier) @method_ret_slice_fn
    result: (slice_type
        element: (type_identifier) @method_ret_slice_type)) @method_ret_slice_def

;;; method return — qualified type
(method_declaration
    name: (field_identifier) @method_ret_qual_fn
    result: (qualified_type
        name: (type_identifier) @method_ret_qual_type)) @method_ret_qual_def

;;; method return — pointer to qualified type
(method_declaration
    name: (field_identifier) @method_ret_ptr_qual_fn
    result: (pointer_type
        (qualified_type
            name: (type_identifier) @method_ret_ptr_qual_type))) @method_ret_ptr_qual_def

;;; method return — tuple with direct types
(method_declaration
    name: (field_identifier) @method_ret_tuple_fn
    result: (parameter_list
        (parameter_declaration
            type: (type_identifier) @method_ret_tuple_type))) @method_ret_tuple_def

;;; method return — tuple with pointer types
(method_declaration
    name: (field_identifier) @method_ret_tuple_ptr_fn
    result: (parameter_list
        (parameter_declaration
            type: (pointer_type
                (type_identifier) @method_ret_tuple_ptr_type)))) @method_ret_tuple_ptr_def

;;; method return — tuple with slice types (func (r R) M() ([]Foo, error))
(method_declaration
    name: (field_identifier) @method_ret_tuple_slice_fn
    result: (parameter_list
        (parameter_declaration
            type: (slice_type
                element: (type_identifier) @method_ret_tuple_slice_type)))) @method_ret_tuple_slice_def

;;; method return — tuple with slice of pointer types (func (r R) M() ([]*Foo, error))
(method_declaration
    name: (field_identifier) @method_ret_tuple_slice_ptr_fn
    result: (parameter_list
        (parameter_declaration
            type: (slice_type
                element: (pointer_type
                    (type_identifier) @method_ret_tuple_slice_ptr_type))))) @method_ret_tuple_slice_ptr_def

;;; method return — tuple with slice of qualified types (func (r R) M() ([]pkg.Type, error))
(method_declaration
    name: (field_identifier) @method_ret_tuple_slice_qual_fn
    result: (parameter_list
        (parameter_declaration
            type: (slice_type
                element: (qualified_type
                    package: (package_identifier) @method_ret_tuple_slice_qual_pkg
                    name: (type_identifier) @method_ret_tuple_slice_qual_type))))) @method_ret_tuple_slice_qual_def

;;; method return — tuple with pointer to qualified types (func (r R) M() (*pkg.Type, error))
(method_declaration
    name: (field_identifier) @method_ret_tuple_ptr_qual_fn
    result: (parameter_list
        (parameter_declaration
            type: (pointer_type
                (qualified_type
                    name: (type_identifier) @method_ret_tuple_ptr_qual_type))))) @method_ret_tuple_ptr_qual_def

;;; method return — generic inner type
(method_declaration
    name: (field_identifier) @method_ret_generic_inner_fn
    result: (generic_type
        type_arguments: (type_arguments
            (type_elem
                (type_identifier) @method_ret_generic_inner_type)))) @method_ret_generic_inner_def

;;; ==========================================================================
;;; STRUCT FIELD TYPES
;;; ==========================================================================

;;; field — direct type (Handler Handler)
(type_declaration
    (type_spec
        name: (type_identifier) @field_direct_struct
        type: (struct_type
            (field_declaration_list
                (field_declaration
                    name: (field_identifier) @field_direct_name
                    type: (type_identifier) @field_direct_type))))) @field_direct_def

;;; field — pointer type (Cache *Cache)
(type_declaration
    (type_spec
        name: (type_identifier) @field_ptr_struct
        type: (struct_type
            (field_declaration_list
                (field_declaration
                    name: (field_identifier) @field_ptr_name
                    type: (pointer_type
                        (type_identifier) @field_ptr_type)))))) @field_ptr_def

;;; field — slice type (Items []Item)
(type_declaration
    (type_spec
        name: (type_identifier) @field_slice_struct
        type: (struct_type
            (field_declaration_list
                (field_declaration
                    name: (field_identifier) @field_slice_name
                    type: (slice_type
                        element: (type_identifier) @field_slice_type)))))) @field_slice_def

;;; field — slice of pointer type (Items []*Item)
(type_declaration
    (type_spec
        name: (type_identifier) @field_slice_ptr_struct
        type: (struct_type
            (field_declaration_list
                (field_declaration
                    name: (field_identifier) @field_slice_ptr_name
                    type: (slice_type
                        element: (pointer_type
                            (type_identifier) @field_slice_ptr_type))))))) @field_slice_ptr_def

;;; field — map value type (Meta map[string]Value)
(type_declaration
    (type_spec
        name: (type_identifier) @field_map_struct
        type: (struct_type
            (field_declaration_list
                (field_declaration
                    name: (field_identifier) @field_map_name
                    type: (map_type
                        value: (type_identifier) @field_map_type)))))) @field_map_def

;;; field — qualified type (Server http.Server)
(type_declaration
    (type_spec
        name: (type_identifier) @field_qual_struct
        type: (struct_type
            (field_declaration_list
                (field_declaration
                    name: (field_identifier) @field_qual_name
                    type: (qualified_type
                        name: (type_identifier) @field_qual_type)))))) @field_qual_def

;;; field — pointer to qualified type (Server *http.Server)
(type_declaration
    (type_spec
        name: (type_identifier) @field_ptr_qual_struct
        type: (struct_type
            (field_declaration_list
                (field_declaration
                    name: (field_identifier) @field_ptr_qual_name
                    type: (pointer_type
                        (qualified_type
                            name: (type_identifier) @field_ptr_qual_type))))))) @field_ptr_qual_def

;;; field — channel type (Events chan Event)
(type_declaration
    (type_spec
        name: (type_identifier) @field_chan_struct
        type: (struct_type
            (field_declaration_list
                (field_declaration
                    name: (field_identifier) @field_chan_name
                    type: (channel_type
                        value: (type_identifier) @field_chan_type)))))) @field_chan_def

;;; field — generic type inner (Value Container[T])
(type_declaration
    (type_spec
        name: (type_identifier) @field_generic_struct
        type: (struct_type
            (field_declaration_list
                (field_declaration
                    name: (field_identifier) @field_generic_name
                    type: (generic_type
                        type_arguments: (type_arguments
                            (type_elem
                                (type_identifier) @field_generic_type)))))))) @field_generic_def

;;; ==========================================================================
;;; INTERFACE METHOD TYPES
;;; ==========================================================================

;;; interface method — parameter direct type
(type_declaration
    (type_spec
        name: (type_identifier) @iface_param_direct_iface
        type: (interface_type
            (method_elem
                name: (field_identifier) @iface_param_direct_fn
                parameters: (parameter_list
                    (parameter_declaration
                        type: (type_identifier) @iface_param_direct_type)))))) @iface_param_direct_def

;;; interface method — parameter pointer type
(type_declaration
    (type_spec
        name: (type_identifier) @iface_param_ptr_iface
        type: (interface_type
            (method_elem
                name: (field_identifier) @iface_param_ptr_fn
                parameters: (parameter_list
                    (parameter_declaration
                        type: (pointer_type
                            (type_identifier) @iface_param_ptr_type))))))) @iface_param_ptr_def

;;; interface method — parameter slice type
(type_declaration
    (type_spec
        name: (type_identifier) @iface_param_slice_iface
        type: (interface_type
            (method_elem
                name: (field_identifier) @iface_param_slice_fn
                parameters: (parameter_list
                    (parameter_declaration
                        type: (slice_type
                            element: (type_identifier) @iface_param_slice_type))))))) @iface_param_slice_def

;;; interface method — return direct type
(type_declaration
    (type_spec
        name: (type_identifier) @iface_ret_direct_iface
        type: (interface_type
            (method_elem
                name: (field_identifier) @iface_ret_direct_fn
                result: (type_identifier) @iface_ret_direct_type)))) @iface_ret_direct_def

;;; interface method — return pointer type
(type_declaration
    (type_spec
        name: (type_identifier) @iface_ret_ptr_iface
        type: (interface_type
            (method_elem
                name: (field_identifier) @iface_ret_ptr_fn
                result: (pointer_type
                    (type_identifier) @iface_ret_ptr_type))))) @iface_ret_ptr_def

;;; interface method — return slice type
(type_declaration
    (type_spec
        name: (type_identifier) @iface_ret_slice_iface
        type: (interface_type
            (method_elem
                name: (field_identifier) @iface_ret_slice_fn
                result: (slice_type
                    element: (type_identifier) @iface_ret_slice_type))))) @iface_ret_slice_def

;;; ============================================================================
;;; TYPE ASSERTIONS AND CONVERSIONS (Usage edges)
;;; ============================================================================

;;; type assertion — direct type (x.(MyType))
(type_assertion_expression
    type: (type_identifier) @type_assert_direct_type) @type_assert_direct_def

;;; type assertion — pointer type (x.(*MyType))
(type_assertion_expression
    type: (pointer_type
        (type_identifier) @type_assert_ptr_type)) @type_assert_ptr_def

;;; type assertion — qualified type (x.(pkg.Type))
(type_assertion_expression
    type: (qualified_type
        name: (type_identifier) @type_assert_qual_type)) @type_assert_qual_def

;;; type assertion — pointer to qualified type (x.(*pkg.Type))
(type_assertion_expression
    type: (pointer_type
        (qualified_type
            name: (type_identifier) @type_assert_ptr_qual_type))) @type_assert_ptr_qual_def

;;; ============================================================================
;;; COMPOSITE LITERALS (Usage edges)
;;; ============================================================================
;;; When a type is instantiated via composite literal (MyType{} or &MyType{}),
;;; this creates a Usage edge from the enclosing function to the type.

;;; composite literal — direct type (MyType{...})
(composite_literal
    type: (type_identifier) @composite_direct_type) @composite_direct_def

;;; composite literal — pointer to type (&MyType{...})
(unary_expression
    operand: (composite_literal
        type: (type_identifier) @composite_ptr_type)) @composite_ptr_def

;;; composite literal — qualified type (pkg.MyType{...})
(composite_literal
    type: (qualified_type
        name: (type_identifier) @composite_qual_type)) @composite_qual_def

;;; composite literal — pointer to qualified type (&pkg.MyType{...})
(unary_expression
    operand: (composite_literal
        type: (qualified_type
            name: (type_identifier) @composite_ptr_qual_type))) @composite_ptr_qual_def

;;; composite literal — slice of type ([]MyType{...})
(composite_literal
    type: (slice_type
        element: (type_identifier) @composite_slice_type)) @composite_slice_def

;;; composite literal — slice of qualified type ([]pkg.MyType{...})
(composite_literal
    type: (slice_type
        element: (qualified_type
            name: (type_identifier) @composite_slice_qual_type))) @composite_slice_qual_def

;;; composite literal — map with user type value (map[string]MyType{...})
(composite_literal
    type: (map_type
        value: (type_identifier) @composite_map_val_type)) @composite_map_val_def

;;; composite literal — map with user type key (map[MyType]string{...})
(composite_literal
    type: (map_type
        key: (type_identifier) @composite_map_key_type)) @composite_map_key_def

;;; ============================================================================
;;; VARIABLE DECLARATIONS (TypeAnnotation edges)
;;; ============================================================================
;;; When a variable is declared with an explicit type annotation (var x Type),
;;; this creates a TypeAnnotation edge from the enclosing function to the type.

;;; var declaration — direct type (var x MyType)
(var_declaration
    (var_spec
        type: (type_identifier) @var_direct_type)) @var_direct_def

;;; var declaration — pointer type (var x *MyType)
(var_declaration
    (var_spec
        type: (pointer_type
            (type_identifier) @var_ptr_type))) @var_ptr_def

;;; var declaration — qualified type (var x pkg.MyType)
(var_declaration
    (var_spec
        type: (qualified_type
            name: (type_identifier) @var_qual_type))) @var_qual_def

;;; var declaration — pointer to qualified type (var x *pkg.MyType)
(var_declaration
    (var_spec
        type: (pointer_type
            (qualified_type
                name: (type_identifier) @var_ptr_qual_type)))) @var_ptr_qual_def

;;; var declaration — slice type (var x []MyType)
(var_declaration
    (var_spec
        type: (slice_type
            element: (type_identifier) @var_slice_type))) @var_slice_def

;;; var declaration — slice of qualified type (var x []pkg.MyType)
(var_declaration
    (var_spec
        type: (slice_type
            element: (qualified_type
                name: (type_identifier) @var_slice_qual_type)))) @var_slice_qual_def

;;; var declaration — map with user type value (var x map[string]MyType)
(var_declaration
    (var_spec
        type: (map_type
            value: (type_identifier) @var_map_val_type))) @var_map_val_def

;;; var declaration — map with user type key (var x map[MyType]string)
(var_declaration
    (var_spec
        type: (map_type
            key: (type_identifier) @var_map_key_type))) @var_map_key_def

;;; var declaration — channel type (var x chan MyType)
(var_declaration
    (var_spec
        type: (channel_type
            value: (type_identifier) @var_chan_type))) @var_chan_def
