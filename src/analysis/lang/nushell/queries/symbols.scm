;;; def command
(decl_def
    (cmd_identifier) @cmd_name) @cmd_def

;;; module
(decl_module
    (cmd_identifier) @module_name) @module_def

;;; alias
(decl_alias
    (cmd_identifier) @alias_name) @alias_def

;;; extern
(decl_extern
    (cmd_identifier) @extern_name) @extern_def

;;; const
(stmt_const
    (identifier) @const_name) @const_def

;;; command call
(command
    (cmd_identifier) @command_call_name) @command_call

;;; use statement
(decl_use) @use_decl
