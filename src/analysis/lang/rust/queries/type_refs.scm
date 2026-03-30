;;; parameter types — top-level functions (direct)
(function_item
    name: (identifier) @param_type_fn
    parameters: (parameters
        (parameter
            type: (type_identifier) @param_type_name))) @param_type_def

;;; parameter types — top-level functions (reference: & or &mut)
(function_item
    name: (identifier) @param_ref_type_fn
    parameters: (parameters
        (parameter
            type: (reference_type
                type: (type_identifier) @param_ref_type_name)))) @param_ref_type_def

;;; parameter types — methods inside impl (direct)
(impl_item
    body: (declaration_list
        (function_item
            name: (identifier) @method_param_type_fn
            parameters: (parameters
                (parameter
                    type: (type_identifier) @method_param_type_name))))) @method_param_type_def

;;; parameter types — methods inside impl (reference)
(impl_item
    body: (declaration_list
        (function_item
            name: (identifier) @method_param_ref_type_fn
            parameters: (parameters
                (parameter
                    type: (reference_type
                        type: (type_identifier) @method_param_ref_type_name)))))) @method_param_ref_type_def

;;; parameter types — trait method signatures (direct)
(trait_item
    body: (declaration_list
        (function_signature_item
            name: (identifier) @trait_param_type_fn
            parameters: (parameters
                (parameter
                    type: (type_identifier) @trait_param_type_name))))) @trait_param_type_def

;;; parameter types — trait method signatures (reference)
(trait_item
    body: (declaration_list
        (function_signature_item
            name: (identifier) @trait_param_ref_type_fn
            parameters: (parameters
                (parameter
                    type: (reference_type
                        type: (type_identifier) @trait_param_ref_type_name)))))) @trait_param_ref_type_def

;;; return types — top-level functions (direct)
(function_item
    name: (identifier) @ret_type_fn
    return_type: (type_identifier) @ret_type_name) @ret_type_def

;;; return types — top-level functions (generic, e.g. Result<Foo>)
(function_item
    name: (identifier) @ret_generic_type_fn
    return_type: (generic_type
        type: (type_identifier) @ret_generic_type_name)) @ret_generic_type_def

;;; return types — methods inside impl (direct)
(impl_item
    body: (declaration_list
        (function_item
            name: (identifier) @method_ret_type_fn
            return_type: (type_identifier) @method_ret_type_name))) @method_ret_type_def

;;; return types — methods inside impl (generic)
(impl_item
    body: (declaration_list
        (function_item
            name: (identifier) @method_ret_generic_type_fn
            return_type: (generic_type
                type: (type_identifier) @method_ret_generic_type_name)))) @method_ret_generic_type_def

;;; return types — trait method signatures (direct)
(trait_item
    body: (declaration_list
        (function_signature_item
            name: (identifier) @trait_ret_type_fn
            return_type: (type_identifier) @trait_ret_type_name))) @trait_ret_type_def

;;; return types — trait method signatures (generic)
(trait_item
    body: (declaration_list
        (function_signature_item
            name: (identifier) @trait_ret_generic_type_fn
            return_type: (generic_type
                type: (type_identifier) @trait_ret_generic_type_name)))) @trait_ret_generic_type_def

;;; struct field types — direct (field: Foo)
(struct_item
    name: (type_identifier) @field_type_struct
    body: (field_declaration_list
        (field_declaration
            name: (field_identifier) @field_type_field
            type: (type_identifier) @field_type_name))) @field_type_def

;;; struct field types — generic type argument (field: Vec<Foo>)
(struct_item
    name: (type_identifier) @field_generic_type_struct
    body: (field_declaration_list
        (field_declaration
            name: (field_identifier) @field_generic_type_field
            type: (generic_type
                type_arguments: (type_arguments
                    (type_identifier) @field_generic_type_arg))))) @field_generic_type_def

;;; struct field types — reference (field: &Foo)
(struct_item
    name: (type_identifier) @field_ref_type_struct
    body: (field_declaration_list
        (field_declaration
            name: (field_identifier) @field_ref_type_field
            type: (reference_type
                type: (type_identifier) @field_ref_type_name)))) @field_ref_type_def

;;; struct field types — dynamic type inside nested generics (e.g. HashMap<K, Box<dyn Handler>>)
(struct_item
    name: (type_identifier) @field_dyn_type_struct
    body: (field_declaration_list
        (field_declaration
            name: (field_identifier) @field_dyn_type_field
            type: (generic_type
                type_arguments: (type_arguments
                    (generic_type
                        type_arguments: (type_arguments
                            (dynamic_type
                                (type_identifier) @field_dyn_type_name)))))))) @field_dyn_type_def
