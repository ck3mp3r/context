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

;;; type_declaration — struct (top-level only)
(source_file
    (type_declaration
        (type_spec
            name: (type_identifier) @struct_name
            type: (struct_type))) @struct_def)

;;; type_declaration — interface (top-level only)
(source_file
    (type_declaration
        (type_spec
            name: (type_identifier) @iface_name
            type: (interface_type))) @iface_def)

;;; type_declaration — type alias (top-level only)
(source_file
    (type_declaration
        (type_spec
            name: (type_identifier) @type_alias_name
            type: (_) @type_alias_value)) @type_alias_def)

;;; const_spec
(const_declaration
    (const_spec
        name: (identifier) @const_name)) @const_def

;;; var_spec (top-level only)
(source_file
    (var_declaration
        (var_spec
            name: (identifier) @var_name))) @var_def

;;; struct field declarations with parent struct name (top-level only)
(source_file
    (type_declaration
        (type_spec
            name: (type_identifier) @field_parent
            type: (struct_type
                (field_declaration_list
                    (field_declaration
                        name: (field_identifier) @field_name) @field_def)))) @field_struct)

;;; interface method specs with parent interface name (top-level only)
(source_file
    (type_declaration
        (type_spec
            name: (type_identifier) @iface_method_parent
            type: (interface_type
                (method_elem
                    name: (field_identifier) @iface_method_name) @iface_method_def))) @iface_method_interface)

;;; struct embedding heritage (anonymous fields only — !name excludes named fields) (top-level only)
(source_file
    (type_declaration
        (type_spec
            name: (type_identifier) @heritage_class
            type: (struct_type
                (field_declaration_list
                    (field_declaration
                        !name
                        type: (type_identifier) @heritage_extends))))) @heritage_def)

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

;;; ============================================================================
;;; IDENTIFIER USES (for Uses edges)
;;; ============================================================================

;;; return_statement with identifier
(return_statement
    (expression_list
        (identifier) @uses_return_ident)) @uses_return_def

;;; short_var_declaration RHS identifier (x := myConst)
(short_var_declaration
    right: (expression_list
        (identifier) @uses_short_var_ident)) @uses_short_var_def

;;; assignment_statement RHS identifier (x = myConst)
(assignment_statement
    right: (expression_list
        (identifier) @uses_assign_ident)) @uses_assign_def

;;; binary_expression with identifier operands
(binary_expression
    left: (identifier) @uses_binop_left) @uses_binop_left_def

(binary_expression
    right: (identifier) @uses_binop_right) @uses_binop_right_def

;;; call argument that is an identifier (not a call itself)
;;; Note: func_ref_name already captures this, but we need it for Uses edges too
(call_expression
    arguments: (argument_list
        (identifier) @uses_call_arg_ident)) @uses_call_arg_def

;;; ============================================================================
;;; QUALIFIED IDENTIFIER USES (pkg.Symbol for Uses edges)
;;; ============================================================================

;;; call argument that is a qualified reference (pkg.Symbol)
(call_expression
    arguments: (argument_list
        (selector_expression
            operand: (identifier) @uses_qual_call_pkg
            field: (field_identifier) @uses_qual_call_name))) @uses_qual_call_def

;;; var declaration RHS with qualified reference (var x = pkg.Symbol)
(var_declaration
    (var_spec
        value: (expression_list
            (selector_expression
                operand: (identifier) @uses_qual_var_pkg
                field: (field_identifier) @uses_qual_var_name)))) @uses_qual_var_def

;;; short_var_declaration RHS with qualified reference (x := pkg.Symbol)
(short_var_declaration
    right: (expression_list
        (selector_expression
            operand: (identifier) @uses_qual_short_pkg
            field: (field_identifier) @uses_qual_short_name))) @uses_qual_short_def

;;; assignment_statement RHS with qualified reference (x = pkg.Symbol)
(assignment_statement
    right: (expression_list
        (selector_expression
            operand: (identifier) @uses_qual_assign_pkg
            field: (field_identifier) @uses_qual_assign_name))) @uses_qual_assign_def

;;; return_statement with qualified reference (return pkg.Symbol)
(return_statement
    (expression_list
        (selector_expression
            operand: (identifier) @uses_qual_return_pkg
            field: (field_identifier) @uses_qual_return_name))) @uses_qual_return_def
