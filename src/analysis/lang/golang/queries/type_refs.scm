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
