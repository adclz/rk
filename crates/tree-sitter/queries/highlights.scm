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

[ "AND" "OR" "XOR" "NOT" "MOD" ] @keyword.operator
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
(bool_name) @type.builtin
(char_name) @type.builtin
(int_name) @type.builtin
(real_name) @type.builtin
(string_name) @type.builtin

(line_comment) @comment
(c_style_comment) @comment
(pascal_style_comment) @comment
(pragma) @attribute

(int_literal) @number
(real_literal) @number
(numeric_literal) @number
(signed_int) @number
(unsigned_int) @number
(binary_int) @number
(octal_int) @number
(hex_int) @number
(time_literal) @number
(date_literal) @number
(duration) @number
(bool_literal) @constant.builtin
(char_literal) @string
(char_str) @string
(s_byte_char_str) @string
(d_byte_char_str) @string

(func_decl name: (identifier) @function)
(fb_decl name: (identifier) @function)
(method_decl name: (identifier) @function.method)
(method_prototype name: (identifier) @function.method)
(class_decl name: (identifier) @type)
(interface_decl name: (identifier) @type)
(prog_decl name: (identifier) @function)
(type_decl name: (identifier) @type)
(namespace_decl name: (namespace_h_name) @namespace)
(func_call function: (_) @function.call)

[ ":=" "=>" "=" "<>" "<" "<=" ">" ">=" "+" "-" "*" "/" "**" "&" ] @operator
(deref_sign) @operator
[ ";" "," "." ":" ".." ] @punctuation.delimiter
[ "(" ")" "[" "]" "{" "}" ] @punctuation.bracket
