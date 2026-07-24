;;; ============================================================================
;;; Kotlin symbol extraction queries (constrained)
;;; See c5t note 49f662d3 for the audit and 7d1244e0 for the spec.
;;;
;;; Strategy:
;;;   - class_declaration / object_declaration / companion_object / type_alias:
;;;     BARE queries (no source_file anchor). Existing tests expect nested/
;;;     inner classes to be extracted (test_nested_class_extraction), so we do
;;;     NOT add a top-level-only constraint to these.
;;;   - function_declaration / property_declaration: SPLIT by context.
;;;     Top-level (source_file child) -> @fn_name/@fn_def, @prop_name/@prop_def.
;;;     Class/object body member (class_body child) -> @method_name/@method_def,
;;;     @member_prop_name/@member_prop_def.
;;;     This eliminates local functions/properties inside function bodies
;;;     (the Kotlin equivalent of the TS local-variable bug) while preserving
;;;     member extraction.
;;;   - enum_entry: anchored to enum_class_body (structural).
;;;   - class_parameter: anchored to class_declaration > primary_constructor.
;;; Kotlin has NO export wrapper (unlike TypeScript) — visibility is a
;;; modifier child, so single patterns suffice (Go-style).
;;; ============================================================================

;;; Classes (regular, data, sealed, abstract, inner, value, enum, interface)
;;; BARE — intentionally NOT anchored to source_file, so nested/inner classes
;;; inside other classes are still extracted (existing test contract).
(class_declaration
  (type_identifier) @class_name) @class_def

;;; Object declarations (singletons) — BARE, same rationale.
(object_declaration
  (type_identifier) @object_name) @object_def

;;; Companion objects — BARE. Captured via @companion_def only (no name
;;; capture in the query; process_match extracts the name from the node, like
;;; the current code does).
(companion_object) @companion_def

;;; Type aliases — BARE, so nested type aliases inside class bodies extract.
(type_alias
  (type_identifier) @typealias_name) @typealias_def

;;; ----------------------------------------------------------------------------
;;; Functions — split by syntactic context
;;; ----------------------------------------------------------------------------

;;; Top-level functions (direct child of source_file).
;;; Eliminates local functions declared inside other function bodies.
(source_file
  (function_declaration
    (simple_identifier) @fn_name) @fn_def)

;;; Class/object member functions (direct child of class_body).
;;; Capture name differs (@method_name) so process_match knows the context
;;; without walking parents. The is_inside_interface helper is STILL used on
;;; these to split interface_method vs method (see Step 2d).
(class_body
  (function_declaration
    (simple_identifier) @method_name) @method_def)

;;; ----------------------------------------------------------------------------
;;; Properties — split by syntactic context
;;; ----------------------------------------------------------------------------

;;; Top-level properties (direct child of source_file).
;;; Eliminates local properties inside function bodies.
(source_file
  (property_declaration
    (variable_declaration
      (simple_identifier) @prop_name)) @prop_def)

;;; Class/object member properties (direct child of class_body).
(class_body
  (property_declaration
    (variable_declaration
      (simple_identifier) @member_prop_name)) @member_prop_def)

;;; ----------------------------------------------------------------------------
;;; Enum entries — anchored to enum_class_body (structural)
;;; ----------------------------------------------------------------------------
(enum_class_body
  (enum_entry
    (simple_identifier) @enum_entry_name) @enum_entry_def)

;;; ----------------------------------------------------------------------------
;;; Class parameters (constructor properties -> fields)
;;; Anchored to class_declaration > primary_constructor > class_parameter.
;;; This is structural: class_parameter only appears in primary_constructor,
;;; which only appears in class_declaration. Anchoring makes it explicit and
;;; excludes any other simple_identifier match.
;;; ----------------------------------------------------------------------------
(class_declaration
  (primary_constructor
    (class_parameter
      (simple_identifier) @class_param_name) @class_param_def))
