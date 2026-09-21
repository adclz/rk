; Highlighting for Structured Text.
;
; Keywords are matched by their token text: the grammar folds every
; case-insensitive spelling into the upper-case alias, so a lower-case
; keyword lands on the same anonymous node as the upper-case one.

[
  "PROGRAM" "END_PROGRAM" "CONFIGURATION" "END_CONFIGURATION"
  "RESOURCE" "END_RESOURCE" "TASK" "NAMESPACE" "END_NAMESPACE" "USING"
  "CLASS" "END_CLASS" "INTERFACE" "END_INTERFACE" "METHOD" "END_METHOD"
  "FUNCTION" "END_FUNCTION" "FUNCTION_BLOCK" "END_FUNCTION_BLOCK"
  "TYPE" "END_TYPE" "STRUCT" "END_STRUCT" "ACTION" "END_ACTION"
  "STEP" "END_STEP" "INITIAL_STEP" "TRANSITION" "END_TRANSITION"
  "IMPLEMENTS" "EXTENDS" "ABSTRACT" "FINAL" "OVERRIDE"
  "INTERNAL" "VAR_ACCESS" "VAR_CONFIG"
  "ON" "WITH" "FROM" "SINGLE" "INTERVAL" "PRIORITY" "OVERLAP"
] @keyword
(read_only) @keyword
(read_write) @keyword
(private) @keyword
(public) @keyword
(protected) @keyword

[
  "VAR" "END_VAR" "VAR_INPUT" "VAR_OUTPUT" "VAR_IN_OUT" "VAR_TEMP"
  "VAR_EXTERNAL" "VAR_GLOBAL" "RETAIN" "NON_RETAIN" "CONSTANT" "AT"
  "REF_TO" "REF" "R_EDGE" "F_EDGE"
] @keyword.storage

[
  "IF" "THEN" "ELSE" "ELSIF" "END_IF" "CASE" "OF" "END_CASE"
  "FOR" "TO" "BY" "DO" "END_FOR" "REPEAT" "UNTIL" "END_REPEAT"
  "WHILE" "END_WHILE" "EXIT" "RETURN" "CONTINUE" "__RAISE"
] @keyword.control

[ "AND" "OR" "XOR" "NOT" ] @keyword.control
[ "MOD" ] @operator
[ "THIS" "SUPER" ] @variable.builtin
[ "TRUE" "FALSE" ] @constant.builtin
(null) @constant.builtin

[ "ARRAY" "STRING" "BOOL" "CHAR" ] @type.builtin
(int_type_name) @type.builtin
(real_type_name) @type.builtin
(bit_str_type_name) @type.builtin
(string_type_name) @type.builtin
(time_type_name) @type.builtin
(date_type_name) @type.builtin
(tod_type_name) @type.builtin
(dt_type_name) @type.builtin
(l_time_type_name) @type.builtin
(l_date_type_name) @type.builtin
(ltod_type_name) @type.builtin
(l_dt_type_name) @type.builtin
(multibits_type_name) @type.builtin

(line_comment) @comment
(c_style_comment) @comment
(pascal_style_comment) @comment
(pragma) @attribute
; The pragmas an author actually writes never appear as `pragma`. Above a POU
; they arrive wrapped in `pou_pragma`; on a statement they arrive bare, so each
; kind is captured itself rather than through the wrapper.
(allow_pragma) @attribute
(export_pragma) @attribute
(extern_pragma) @attribute
(once_pragma) @attribute
(test_pragma) @attribute
(warn_pragma) @attribute
(wasm_pragma) @attribute
; …and inside one, each part is what it is: the rule or module names are
; strings, the parameters name variables, the result names a type.
(pragma_string) @string
(warn_pragma_level) @keyword
(extern_param_list var: (identifier) @variable)
(extern_result var: (identifier) @variable)
(wasm_pragma type_ref: (identifier) @type)

; `Color#Red`: the type reads as a type, the variant and the `#` as a member.
(enum_value) @constant
(enum_value enum_path: (_) @type)
(enum_value_spec value: (identifier) @constant)

(int_literal) @number
(unsigned_int) @number
(real_literal) @number
(numeric_literal) @number
; A literal with a type prefix (`INT#5`, `T#1s`) is `variable.other.constant`
; in the editor, the same as an enum value; a bare `5` is a number.
(int_literal kind: (_)) @constant
(real_literal type: (_)) @constant
(time_literal) @constant
(date_literal) @constant
(time_literal) @number
(date_literal) @number
(bool_literal) @constant.builtin
(char_literal) @string

(func_decl name: (identifier) @function)
(fb_decl name: (identifier) @function)
(method_decl name: (identifier) @function.method)
(method_prototype name: (identifier) @function.method)
(class_decl name: (identifier) @type)
(interface_decl name: (identifier) @type)
(prog_decl name: (identifier) @function)
(type_decl name: (identifier) @type)
(namespace_decl name: (namespace_h_name) @namespace)
(config_decl name: (identifier) @type)
(resource_decl name: (identifier) @type)
(task_config name: (identifier) @function)
(fb_task task: (identifier) @function.call)
(prog_config name: (identifier) @function)
(prog_config task: (identifier) @function.call)
(prog_config access: (namespace_access) @type)
(func_call function: (_) @function.call)
(param_assign_input param: (identifier) @variable)
(param_assign_output param: (identifier) @variable)

[ ":=" "=>" "=" "<>" "<" "<=" ">" ">=" "+" "-" "*" "/" "**" "&" ] @operator
(deref_sign) @operator
[ ";" "," "." ":" ".." ] @punctuation.delimiter
[ "(" ")" "[" "]" "{" "}" ] @punctuation.bracket
