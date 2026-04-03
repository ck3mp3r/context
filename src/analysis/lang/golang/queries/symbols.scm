;;; package_clause
(package_clause
    (package_identifier) @pkg_name) @package

;;; function_declaration
(function_declaration
    name: (identifier) @fn_name) @fn_def

;;; method_declaration
(method_declaration
    receiver: (parameter_list) @method_receiver
    name: (field_identifier) @method_name) @method_def

;;; type_declaration — struct
(type_declaration
    (type_spec
        name: (type_identifier) @struct_name
        type: (struct_type))) @struct_def

;;; type_declaration — interface
(type_declaration
    (type_spec
        name: (type_identifier) @iface_name
        type: (interface_type))) @iface_def

;;; type_declaration — type alias
(type_declaration
    (type_spec
        name: (type_identifier) @type_alias_name
        type: (_) @type_alias_value)) @type_alias_def

;;; const_spec
(const_declaration
    (const_spec
        name: (identifier) @const_name)) @const_def

;;; var_spec (top-level only)
(source_file
    (var_declaration
        (var_spec
            name: (identifier) @var_name))) @var_def

;;; struct field declarations with parent struct name
(type_declaration
    (type_spec
        name: (type_identifier) @field_parent
        type: (struct_type
            (field_declaration_list
                (field_declaration
                    name: (field_identifier) @field_name) @field_def))))

;;; interface method specs with parent interface name
(type_declaration
    (type_spec
        name: (type_identifier) @iface_method_parent
        type: (interface_type
            (method_elem
                name: (field_identifier) @iface_method_name) @iface_method_def)))

;;; struct embedding heritage (anonymous fields only — !name excludes named fields)
(type_declaration
    (type_spec
        name: (type_identifier) @heritage_class
        type: (struct_type
            (field_declaration_list
                (field_declaration
                    !name
                    type: (type_identifier) @heritage_extends))))) @heritage_def

;;; call_expression — plain function call
(call_expression
    function: (identifier) @call_free_name) @call_free

;;; call_expression — selector call (pkg.Func() or obj.Method())
(call_expression
    function: (selector_expression
        operand: (_) @call_selector_operand
        field: (field_identifier) @call_selector_name)) @call_selector

;;; composite_literal — struct instantiation
(composite_literal
    type: (type_identifier) @composite_type) @composite_lit

;;; composite_literal — qualified struct instantiation (pkg.Type{})
(composite_literal
    type: (qualified_type
        package: (package_identifier) @composite_pkg
        name: (type_identifier) @composite_qual_type)) @composite_qual_lit

;;; function reference passed as argument (callback)
(call_expression
    arguments: (argument_list
        (identifier) @func_ref_name)) @func_ref_call

;;; qualified function reference passed as argument (callback) — pkg.Func
(call_expression
    arguments: (argument_list
        (selector_expression
            operand: (identifier) @func_ref_qual_pkg
            field: (field_identifier) @func_ref_qual_name))) @func_ref_qual_call

;;; import_declaration — single import
(import_declaration
    (import_spec
        path: (interpreted_string_literal) @import_path)) @import_decl

;;; import_declaration — grouped imports
(import_declaration
    (import_spec_list
        (import_spec
            path: (interpreted_string_literal) @import_grouped_path))) @import_grouped_decl

;;; import with alias — single
(import_declaration
    (import_spec
        name: (package_identifier) @import_alias
        path: (interpreted_string_literal) @import_alias_path)) @import_alias_decl

;;; import with alias — grouped
(import_declaration
    (import_spec_list
        (import_spec
            name: (package_identifier) @import_grouped_alias
            path: (interpreted_string_literal) @import_grouped_alias_path))) @import_grouped_alias_decl

;;; write access — field assignment (obj.field = value)
(assignment_statement
    left: (expression_list
        (selector_expression
            operand: (_) @write_assign_receiver
            field: (field_identifier) @write_assign_field))
    right: (_)) @write_assign

;;; write access — field increment (obj.field++)
(inc_statement
    (selector_expression
        operand: (_) @write_inc_receiver
        field: (field_identifier) @write_inc_field)) @write_inc

;;; write access — field decrement (obj.field--)
(dec_statement
    (selector_expression
        operand: (_) @write_dec_receiver
        field: (field_identifier) @write_dec_field)) @write_dec
