-- Code Analysis Schema for SurrealDB
-- Translated from NanoGraph schema.pg
--
-- Design philosophy:
-- - Simple: 2 node types (File, Symbol) instead of 30+
-- - Generic: Symbol.kind field handles all code entities
-- - Language-agnostic: Same schema for TypeScript, Rust, Python, etc.
-- - Confidence tracking: Tree-sitter can be uncertain about cross-file refs

-- ============================================================================
-- Node Tables
-- ============================================================================

-- Symbol Table: Represents any code symbol (function, class, struct, etc.)
DEFINE TABLE symbol SCHEMAFULL;

DEFINE FIELD symbol_id ON symbol TYPE string
    ASSERT string::len($value) > 0;

DEFINE FIELD repo_id ON symbol TYPE string
    ASSERT string::len($value) > 0;

DEFINE FIELD name ON symbol TYPE string
    ASSERT string::len($value) > 0;

DEFINE FIELD kind ON symbol TYPE string
    ASSERT string::len($value) > 0;

DEFINE FIELD language ON symbol TYPE string
    ASSERT string::len($value) > 0;

DEFINE FIELD file_path ON symbol TYPE string
    ASSERT string::len($value) > 0;

DEFINE FIELD start_line ON symbol TYPE int
    ASSERT $value >= 0;

DEFINE FIELD end_line ON symbol TYPE int
    ASSERT $value >= 0;

DEFINE FIELD visibility ON symbol TYPE option<string>;

DEFINE FIELD entry_type ON symbol TYPE option<string>;

DEFINE FIELD signature ON symbol TYPE option<string>;

DEFINE FIELD content ON symbol TYPE option<string>;

DEFINE FIELD module_path ON symbol TYPE option<string>;

-- Indexes for Symbol table
-- Compound unique index: symbol_id is unique WITHIN each repo_id
DEFINE INDEX idx_symbol_unique ON symbol FIELDS repo_id, symbol_id UNIQUE;
DEFINE INDEX idx_symbol_repo ON symbol FIELDS repo_id;
DEFINE INDEX idx_symbol_name ON symbol FIELDS name;
DEFINE INDEX idx_symbol_kind ON symbol FIELDS kind;
DEFINE INDEX idx_symbol_language ON symbol FIELDS language;
DEFINE INDEX idx_symbol_file ON symbol FIELDS file_path;
DEFINE INDEX idx_symbol_entry_type ON symbol FIELDS entry_type;

-- File Table: Represents source code files
DEFINE TABLE file SCHEMAFULL;

DEFINE FIELD file_id ON file TYPE string
    ASSERT string::len($value) > 0;

DEFINE FIELD repo_id ON file TYPE string
    ASSERT string::len($value) > 0;

DEFINE FIELD path ON file TYPE string
    ASSERT string::len($value) > 0;

DEFINE FIELD language ON file TYPE string
    ASSERT string::len($value) > 0;

DEFINE FIELD hash ON file TYPE string
    ASSERT string::len($value) > 0;

-- Indexes for File table
-- Compound unique index: file_id is unique WITHIN each repo_id
DEFINE INDEX idx_file_unique ON file FIELDS repo_id, file_id UNIQUE;
DEFINE INDEX idx_file_repo ON file FIELDS repo_id;
DEFINE INDEX idx_file_path ON file FIELDS path;
DEFINE INDEX idx_file_language ON file FIELDS language;

-- ============================================================================
-- Edge Tables (Relationships)
-- ============================================================================

-- FileContains: File contains Symbol
DEFINE TABLE file_contains TYPE RELATION IN file OUT symbol ENFORCED;

DEFINE FIELD confidence ON file_contains TYPE float DEFAULT 1.0
    ASSERT $value >= 0.0 AND $value <= 1.0;

DEFINE INDEX idx_file_contains_in ON file_contains FIELDS in;
DEFINE INDEX idx_file_contains_out ON file_contains FIELDS out;

-- HasField: Struct/Class has field
DEFINE TABLE has_field TYPE RELATION IN symbol OUT symbol ENFORCED;

DEFINE FIELD confidence ON has_field TYPE float DEFAULT 1.0
    ASSERT $value >= 0.0 AND $value <= 1.0;

DEFINE INDEX idx_has_field_in ON has_field FIELDS in;
DEFINE INDEX idx_has_field_out ON has_field FIELDS out;

-- HasMethod: Type has method (impl block or trait impl)
DEFINE TABLE has_method TYPE RELATION IN symbol OUT symbol ENFORCED;

DEFINE FIELD confidence ON has_method TYPE float DEFAULT 1.0
    ASSERT $value >= 0.0 AND $value <= 1.0;

DEFINE INDEX idx_has_method_in ON has_method FIELDS in;
DEFINE INDEX idx_has_method_out ON has_method FIELDS out;

-- HasMember: Module has member symbol
DEFINE TABLE has_member TYPE RELATION IN symbol OUT symbol ENFORCED;

DEFINE FIELD confidence ON has_member TYPE float DEFAULT 1.0
    ASSERT $value >= 0.0 AND $value <= 1.0;

DEFINE INDEX idx_has_member_in ON has_member FIELDS in;
DEFINE INDEX idx_has_member_out ON has_member FIELDS out;

-- Calls: Function/method calls another function/method
DEFINE TABLE calls TYPE RELATION IN symbol OUT symbol ENFORCED;

DEFINE FIELD confidence ON calls TYPE float DEFAULT 1.0
    ASSERT $value >= 0.0 AND $value <= 1.0;

DEFINE FIELD call_site_line ON calls TYPE int
    ASSERT $value >= 0;

DEFINE INDEX idx_calls_in ON calls FIELDS in;
DEFINE INDEX idx_calls_out ON calls FIELDS out;

-- FileImports: File-level imports
DEFINE TABLE file_imports TYPE RELATION IN file OUT symbol ENFORCED;

DEFINE FIELD confidence ON file_imports TYPE float DEFAULT 1.0
    ASSERT $value >= 0.0 AND $value <= 1.0;

DEFINE INDEX idx_file_imports_in ON file_imports FIELDS in;
DEFINE INDEX idx_file_imports_out ON file_imports FIELDS out;

-- Import: Scoped imports (Rust use inside fn/impl, Nushell use inside command)
DEFINE TABLE import TYPE RELATION IN symbol OUT symbol ENFORCED;

DEFINE FIELD confidence ON import TYPE float DEFAULT 1.0
    ASSERT $value >= 0.0 AND $value <= 1.0;

DEFINE INDEX idx_import_in ON import FIELDS in;
DEFINE INDEX idx_import_out ON import FIELDS out;

-- TypeAnnotation: Type annotations in function signatures
DEFINE TABLE type_annotation TYPE RELATION IN symbol OUT symbol ENFORCED;

DEFINE FIELD confidence ON type_annotation TYPE float DEFAULT 1.0
    ASSERT $value >= 0.0 AND $value <= 1.0;

DEFINE INDEX idx_type_annotation_in ON type_annotation FIELDS in;
DEFINE INDEX idx_type_annotation_out ON type_annotation FIELDS out;

-- FieldType: Field type references in structs
DEFINE TABLE field_type TYPE RELATION IN symbol OUT symbol ENFORCED;

DEFINE FIELD confidence ON field_type TYPE float DEFAULT 1.0
    ASSERT $value >= 0.0 AND $value <= 1.0;

DEFINE INDEX idx_field_type_in ON field_type FIELDS in;
DEFINE INDEX idx_field_type_out ON field_type FIELDS out;

-- Implements: Trait/interface implementation
DEFINE TABLE implements TYPE RELATION IN symbol OUT symbol ENFORCED;

DEFINE FIELD confidence ON implements TYPE float DEFAULT 1.0
    ASSERT $value >= 0.0 AND $value <= 1.0;

DEFINE INDEX idx_implements_in ON implements FIELDS in;
DEFINE INDEX idx_implements_out ON implements FIELDS out;

-- Extends: Class/type extension
DEFINE TABLE extends TYPE RELATION IN symbol OUT symbol ENFORCED;

DEFINE FIELD confidence ON extends TYPE float DEFAULT 1.0
    ASSERT $value >= 0.0 AND $value <= 1.0;

DEFINE INDEX idx_extends_in ON extends FIELDS in;
DEFINE INDEX idx_extends_out ON extends FIELDS out;

-- Returns: Function/method returns a type
DEFINE TABLE returns TYPE RELATION IN symbol OUT symbol ENFORCED;

DEFINE FIELD confidence ON returns TYPE float DEFAULT 1.0
    ASSERT $value >= 0.0 AND $value <= 1.0;

DEFINE INDEX idx_returns_in ON returns FIELDS in;
DEFINE INDEX idx_returns_out ON returns FIELDS out;

-- Accepts: Function/method accepts a type as parameter
DEFINE TABLE accepts TYPE RELATION IN symbol OUT symbol ENFORCED;

DEFINE FIELD confidence ON accepts TYPE float DEFAULT 1.0
    ASSERT $value >= 0.0 AND $value <= 1.0;

DEFINE INDEX idx_accepts_in ON accepts FIELDS in;
DEFINE INDEX idx_accepts_out ON accepts FIELDS out;

-- Uses: Function/method body references a variable/constant
DEFINE TABLE uses TYPE RELATION IN symbol OUT symbol ENFORCED;

DEFINE FIELD confidence ON uses TYPE float DEFAULT 1.0
    ASSERT $value >= 0.0 AND $value <= 1.0;

DEFINE INDEX idx_uses_in ON uses FIELDS in;
DEFINE INDEX idx_uses_out ON uses FIELDS out;
