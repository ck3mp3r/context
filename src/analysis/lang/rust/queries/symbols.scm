;;; top-level function (not inside impl/trait blocks)
(source_file
    (function_item
        name: (identifier) @fn_name
        parameters: (parameters) @fn_params
        return_type: (_)? @fn_ret) @fn_def)

;;; function inside mod block
(mod_item
    body: (declaration_list
        (function_item
            name: (identifier) @fn_name
            parameters: (parameters) @fn_params
            return_type: (_)? @fn_ret) @fn_def))

;;; struct_item
(struct_item
    name: (type_identifier) @struct_name) @struct_def

;;; enum_item
(enum_item
    name: (type_identifier) @enum_name) @enum_def

;;; trait_item
(trait_item
    name: (type_identifier) @trait_name) @trait_def

;;; mod_item
(mod_item
    name: (identifier) @mod_name) @mod_def

;;; const_item
(const_item
    name: (identifier) @const_name) @const_def

;;; static_item
(static_item
    name: (identifier) @static_name) @static_def

;;; type_item (type alias)
(type_item
    name: (type_identifier) @type_alias_name) @type_alias_def

;;; macro_definition
(macro_definition
    name: (identifier) @macro_def_name) @macro_def

;;; impl_item — trait impl (concrete trait, concrete type)
(impl_item
    trait: (type_identifier) @impl_trait
    type: (type_identifier) @impl_type) @impl_trait_def

;;; impl_item — trait impl (generic trait, concrete type)
(impl_item
    trait: (generic_type
        type: (type_identifier) @impl_generic_trait_name)
    type: (type_identifier) @impl_generic_trait_type) @impl_generic_trait_def

;;; impl_item — trait impl (concrete trait, generic type)
(impl_item
    trait: (type_identifier) @impl_concrete_trait_generic_type_trait
    type: (generic_type
        type: (type_identifier) @impl_concrete_trait_generic_type_type)) @impl_concrete_trait_generic_type_def

;;; impl_item — trait impl (generic trait, generic type)
(impl_item
    trait: (generic_type
        type: (type_identifier) @impl_both_generic_trait)
    type: (generic_type
        type: (type_identifier) @impl_both_generic_type)) @impl_both_generic_def

;;; impl_item — inherent impl (no trait, concrete type)
(impl_item
    !trait
    type: (type_identifier) @inherent_impl_type) @impl_inherent_def

;;; impl_item — inherent impl (no trait, generic type)
(impl_item
    !trait
    type: (generic_type
        type: (type_identifier) @inherent_generic_impl_type)) @impl_inherent_generic_def

;;; method inside impl — concrete impl type
(impl_item
    type: (type_identifier) @method_impl_type
    body: (declaration_list
        (function_item
            name: (identifier) @method_name
            parameters: (parameters) @method_params
            return_type: (_)? @method_ret) @method_def)) @method_impl

;;; method inside impl — generic impl type
(impl_item
    type: (generic_type
        type: (type_identifier) @method_impl_type)
    body: (declaration_list
        (function_item
            name: (identifier) @method_name
            parameters: (parameters) @method_params
            return_type: (_)? @method_ret) @method_def)) @method_impl

;;; struct field declarations (with parent struct for containment)
(struct_item
    name: (type_identifier) @field_parent
    body: (field_declaration_list
        (field_declaration
            name: (field_identifier) @field_name) @field_def))

;;; trait method signatures (function_signature_item inside trait body)
(trait_item
    name: (type_identifier) @trait_sig_parent
    body: (declaration_list
        (function_signature_item
            name: (identifier) @trait_sig_name) @trait_sig_def))

;;; attribute — simple (#[test], #[no_mangle])
(attribute_item
    (attribute
        (identifier) @attr_simple_name)) @attr_simple

;;; attribute — scoped (#[tokio::main], #[tokio::test])
(attribute_item
    (attribute
        (scoped_identifier
            path: (_) @attr_scope
            name: (identifier) @attr_scoped_name))) @attr_scoped

;;; attribute — cfg with arguments (#[cfg(test)])
(attribute_item
    (attribute
        (identifier) @attr_cfg_name
        arguments: (token_tree) @attr_cfg_args)) @attr_cfg

;;; call_expression — plain function
(call_expression
    function: (identifier) @call_free_name) @call_free

;;; call_expression — method call (obj.method())
(call_expression
    function: (field_expression
        value: (_) @call_method_receiver
        field: (field_identifier) @call_method_name)) @call_method

;;; call_expression — scoped call (Foo::bar())
(call_expression
    function: (scoped_identifier
        path: (_) @call_scoped_path
        name: (identifier) @call_scoped_name)) @call_scoped

;;; call_expression — generic function call (collect::<Vec<_>>())
(call_expression
    function: (generic_function
        function: (identifier) @call_generic_fn_name)) @call_generic_fn

;;; call_expression — generic method call (iter.collect::<Vec<_>>())
(call_expression
    function: (generic_function
        function: (field_expression
            value: (_) @call_generic_method_receiver
            field: (field_identifier) @call_generic_method_name))) @call_generic_method

;;; struct_expression — struct literal construction (Config { port: 8080 })
(struct_expression
    name: (type_identifier) @struct_expr_name) @struct_expr

;;; use_declaration
(use_declaration
    argument: (_) @use_path) @use_decl

;;; macro_invocation
(macro_invocation
    macro: (identifier) @macro_name) @macro_call

;;; write access — field assignment (obj.field = value)
(assignment_expression
    left: (field_expression
        value: (_) @write_assign_receiver
        field: (field_identifier) @write_assign_field)
    right: (_)) @write_assign

;;; write access — compound assignment (obj.field += value)
(compound_assignment_expr
    left: (field_expression
        value: (_) @write_compound_receiver
        field: (field_identifier) @write_compound_field)
    right: (_)) @write_compound

;;; visibility — public items (captures name + start line to correlate with symbols)
(function_item (visibility_modifier) @vis name: (identifier) @vis_name) @vis_def
(struct_item (visibility_modifier) @vis name: (type_identifier) @vis_name) @vis_def
(enum_item (visibility_modifier) @vis name: (type_identifier) @vis_name) @vis_def
(trait_item (visibility_modifier) @vis name: (type_identifier) @vis_name) @vis_def
(mod_item (visibility_modifier) @vis name: (identifier) @vis_name) @vis_def
(const_item (visibility_modifier) @vis name: (identifier) @vis_name) @vis_def
(static_item (visibility_modifier) @vis name: (identifier) @vis_name) @vis_def
(type_item (visibility_modifier) @vis name: (type_identifier) @vis_name) @vis_def
(field_declaration (visibility_modifier) @vis name: (field_identifier) @vis_name) @vis_def
