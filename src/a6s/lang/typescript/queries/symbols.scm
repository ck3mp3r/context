;;; ============================================================================
;;; TypeScript symbol extraction queries (constrained)
;;; See c5t note 49f662d3 for the audit and 7d1244e0 for the spec.
;;; Pattern: top-level decls get dual (program | export_statement) anchors;
;;; members get anchored to their container body (class_body / interface_body /
;;; enum_body). This replaces the is_top_level and is_inside_inline_object_type
;;; Rust post-filters.
;;; ============================================================================

;;; Classes — top-level (plain or exported)
(program
  (class_declaration
    name: (type_identifier) @class_name) @class_def)

(export_statement
  (class_declaration
    name: (type_identifier) @class_name) @class_def)

;;; Abstract classes — top-level
(program
  (abstract_class_declaration
    name: (type_identifier) @abstract_class_name) @abstract_class_def)

(export_statement
  (abstract_class_declaration
    name: (type_identifier) @abstract_class_name) @abstract_class_def)

;;; Interfaces — top-level
(program
  (interface_declaration
    name: (type_identifier) @interface_name) @interface_def)

(export_statement
  (interface_declaration
    name: (type_identifier) @interface_name) @interface_def)

;;; Type aliases — top-level
(program
  (type_alias_declaration
    name: (type_identifier) @typealias_name) @typealias_def)

(export_statement
  (type_alias_declaration
    name: (type_identifier) @typealias_name) @typealias_def)

;;; Enums — top-level
;;; NOTE: enum_declaration name is (identifier), NOT (type_identifier).
(program
  (enum_declaration
    name: (identifier) @enum_name) @enum_def)

(export_statement
  (enum_declaration
    name: (identifier) @enum_name) @enum_def)

;;; Enum members — only direct children of enum_body
(enum_body
  (enum_assignment
    name: (property_identifier) @enum_member_name) @enum_member_def)

;;; Functions — top-level
(program
  (function_declaration
    name: (identifier) @fn_name) @fn_def)

(export_statement
  (function_declaration
    name: (identifier) @fn_name) @fn_def)

;;; Generator functions — top-level
(program
  (generator_function_declaration
    name: (identifier) @gen_fn_name) @gen_fn_def)

(export_statement
  (generator_function_declaration
    name: (identifier) @gen_fn_name) @gen_fn_def)

;;; Methods — only direct children of class_body
;;; This is the headline fix: the bare query also matched object-literal
;;; methods (`const obj = { foo() {} }`) because `method_definition` is a
;;; child of BOTH class_body AND object. Anchoring to class_body excludes
;;; object literals (whose method_definition parent is `object`).
(class_body
  (method_definition
    name: (property_identifier) @method_name) @method_def)

;;; Abstract method signatures — only direct children of class_body
(class_body
  (abstract_method_signature
    name: (property_identifier) @abstract_method_name) @abstract_method_def)

;;; Interface method signatures — only direct children of interface_body
;;; This REPLACES the is_inside_inline_object_type Rust post-filter, which
;;; existed because method_signature is a child of BOTH interface_body (want)
;;; AND object_type (inline type literals — don't want).
(interface_body
  (method_signature
    name: (property_identifier) @method_sig_name) @method_sig_def)

;;; Class-body method signatures (overload signatures) — preserve current
;;; extraction behavior. The process_match handler maps @method_sig_name to
;;; the `interface_method` kind; keep that mapping.
(class_body
  (method_signature
    name: (property_identifier) @method_sig_name) @method_sig_def)

;;; Class fields — only direct children of class_body
(class_body
  (public_field_definition
    name: (property_identifier) @field_name) @field_def)

;;; Top-level lexical declarations (const/let) — dual anchor
;;; This REPLACES the is_top_level Rust post-filter for variables.
;;; Only lexical_declaration (const/let) is matched; var uses
;;; variable_declaration, which is intentionally NOT matched here (matches
;;; current behavior — the old query only had lexical_declaration).
(program
  (lexical_declaration
    (variable_declarator
      name: (identifier) @var_name)) @var_def)

(export_statement
  (lexical_declaration
    (variable_declarator
      name: (identifier) @var_name)) @var_def)
