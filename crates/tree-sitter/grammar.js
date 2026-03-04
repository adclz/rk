/*
This file is part of tree-sitter-rk.
Copyright (C) 2025 CLAUZEL Adrien

auto-lsp is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>
*/

/**
 * @file Iec61131-3 grammar for tree-sitter
 * @author CLAUZEL Adrien <clauzeladrien@gmail.com>
 * @license AGPL-3.0-only
 */

function commaSep1(rule) {
  return seq(rule, repeat(seq(",", rule)));
}

function commaSep(rule) {
  return optional(commaSep1(rule));
}

function dotSep1(rule) {
  return seq(rule, repeat(seq(".", rule)));
}

function dotSep(rule) {
  return optional(commaSep1(rule));
}

/// Create rules for a new type with a spec and an init
/// This will create two rules, one for the spec and one for the init,
/// with the given name as a prefix 
/// $NAME_type_spec and $NAME_type_init
function createSpecInit(name, spec, init = null) {
  let result = {
    [`${name}_type_spec`]: ($) => spec($),
  };
  if (init) {
    result[`${name}_type_init`] = ($) => init($);
  }
  return result;
}

/// Use a list of specs and inits to complete a new rule
function useSpecInit(specs, inits) {
  return ($) =>
    seq(
      ":",
      field("spec", choice(...specs.map((rule) => $[`${rule}_type_spec`]))),
      field(
        "init",
        optional(choice(...inits.map((rule) => $[`${rule}_type_init`]))),
      ),
    );
}

/// Use a list of specs without the leading ':' (e.g. for array element types after OF)
function useSpec(specs) {
  return ($) =>
    field("spec", choice(...specs.map((rule) => $[`${rule}_type_spec`])));
}

// Variable declarations
const io_var_decls = ($) => [$.input_decls, $.output_decls, $.in_out_decls];

const func_var_decls = ($) => [$.external_var_decls, $.var_decls];

const other_var_decls = ($) => [
  $.retain_var_decls,
  $.no_retain_var_decls,
  $.loc_partly_var_decl,
];

// Precedences from the standard
const RK_PREC = {
  expression: 11,
  parameter_list: 10, // _ (parameter_list)
  dereference: 9, // ^
  unary: 8, // + - NOT
  exponentiation: 7, // **
  multiply: 6, // *
  divide: 6, // /
  modulo: 6, // MOD
  add: 5, // +
  substract: 5, // -
  comparison: 4, // < > <= >=
  equality: 4, // = <>
  boolean_and: 3, // & AND
  boolean_xor: 2, // XOR
  boolean_or: 1, // OR
};

// Reserved keywords that cannot be used as identifiers
const RESERVED_NAMES = [
  "PROGRAM",
  "END_PROGRAM",
  "CONFIGURATION",
  "END_CONFIGURATION",
  "RESOURCE",
  "END_RESOURCE",
  "NAMESPACE",
  "END_NAMESPACE",
  "USING",
  "CLASS",
  "END_CLASS",
  "INTERFACE",
  "END_INTERFACE",
  "METHOD",
  "END_METHOD",
  "FUNCTION",
  "END_FUNCTION",
  "FUNCTION_BLOCK",
  "END_FUNCTION_BLOCK",
  "TYPE",
  "END_TYPE",
  "IMPLEMENTS",
  "EXTENDS",
  "STRUCT",
  "END_STRUCT",
  "VAR",
  "END_VAR",
  "VAR_INPUT",
  "VAR_OUTPUT",
  "VAR_IN_OUT",
  "VAR_TEMP",
  "VAR_EXTERNAL",
  "VAR_GLOBAL",
  "VAR_LOCATED",
  "RETAIN",
  "NON_RETAIN",
  "IF",
  "THEN",
  "ELSE",
  "ELSIF",
  "END_IF",
  "CASE",
  "OF",
  "END_CASE",
  "FOR",
  "TO",
  "BY",
  "DO",
  "END_FOR",
  "REPEAT",
  "UNTIL",
  "END_REPEAT",
  "WHILE",
  "END_WHILE",
  "EXIT",
  "RETURN",
  // References
  "AT",
  "%",
  "REF_TO",
  "REF",
];

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check
module.exports = grammar({
  name: "rk",

  extras: ($) => [
    /\s/, // Whitespace
    $.line_comment,
    $.c_style_comment,
    $.pascal_style_comment,
    $.pragma,
  ],

  reserved: {
    global: ($) => RESERVED_NAMES,
  },

  supertypes: ($) => [
    $._func_variables,
    $._fb_variables,
    $._class_variables,
    $._method_prot_variables,
    $._method_decl_variables,
    $._config_variables,

    $._input_var_kind,
    $._fb_input_var_kind,
    $._output_var_kind,
    $._fb_output_var_kind,
    $._temp_var_kind,
    $._in_out_var_kind,
    $._external_var_kind,
    $._global_var_kind,

    $.data_type_access,
    $._elem_type_name,

    $._expression,
    $._primary_expression,

    // operators
    $.unary,
    $.add,
    $.mult,
    $.eq,
    $.ord,

    $._stmt,

    // literals
    $.any_time_type_name,
    $.any_date_type_name,
    $.any_tod_type_name,
    $.any_dt_type_name,
  ],

  conflicts: ($) => [
    [$.constant_expr, $.parenthesized_expression],
    [$.begin_path_expression, $.field_expression],
    [$.any_invocation_kind, $.field_expression],
    [$.any_invocation],
    [$.any_invocation_kind],
    [$.variable, $.func_call],
    [$.func_call, $._stmt],
    [$.case_selection],
  ],

  word: ($) => $.identifier,

  rules: {
    // Source file declaration
    source_file: ($) =>
      repeat(
        choice(
          $.config_decl, // Declaration of CONFIGURATION and RESOURCE
          $.prog_decl, // Declaration of PROGRAM
          $.namespace_decl, // Declaration of NAMESPACE (including all other declarations)
          // Global POUs declarations
          $.data_type_decl,
          $.func_decl,
          $.fb_decl,
          $.class_decl,
          $.interface_decl,
          // Global using
          $.using_directive,
          $.ERR_invalid_pou_keyword,
        ),
      ),

    // Other - Errors

    // Pou declaration errors
    // Note that this error is rather consuming and i'm unsure if it should be kept
    ERR_invalid_pou_keyword: ($) => $.identifier,

    // Qualifier errors
    ERR_implements_before_extends: ($) =>
      prec(-1, seq("IMPLEMENTS", $.interface_name_list)),
    ERR_implements_multiple_times: ($) =>
      prec(-1, seq("IMPLEMENTS", $.interface_name_list)),
    ERR_extends_multiple_times: ($) =>
      prec(-1, seq("EXTENDS", $.namespace_access)),

    // Variable declaration errors
    ERR_variable_with_no_spec: ($) => prec(-1, $.identifier),
    ERR_invalid_edge_qualifier: ($) => prec(-1, /[FR](_(E(D(G)?)?)?)?/),

    // Statements
    ERR_empty_right_hand_assignment: ($) => prec(-1, ":="), // a := ?

    // Expressions
    ERR_assign_func_call: ($) => prec(-1, $.func_call), // A function call cannot be assigned

    // Missing ':' before =
    ERR_missing_dot_in_assignment: ($) => seq("=", $._expression),
    // Missing '=' after ':'
    ERR_missing_equal_in_assignment: ($) => prec(-1, seq(":", $._expression)),

    // Missing ':' before =
    ERR_missing_dot_in_for_control: ($) => "=",
    // Missing '=' after ':'
    ERR_missing_equal_in_for_control: ($) => ":",

    // Using '=>' (output assign) instead of ':='
    ERR_output_assign_in_assignment: ($) => seq("=>", $._expression),
    ERR_output_assign_in_for_control: ($) => "=>",

    ERR_unexpected_this_in_path: ($) => prec(-1, "THIS"),
    ERR_unexpected_super_in_path: ($) => prec(-1, "SUPER"),

    // Initalisations of arrays and structs are highly permissive,
    // so permissive that it is fine to call functions inside.
    // for now we will forbid function calls, until the day we implement compile time evaluation
    ERR_func_call_in_init: ($) => prec(2, $.func_call),

    ERR_class_variables_after_method: ($) =>
      prec(-1, repeat1($._class_variables)),
    ERR_fb_variables_after_method: ($) => prec(-1, repeat1($._fb_variables)),

    ERR_program_not_allowed_in_namespace: ($) => prec(-1, $.prog_decl),
    ERR_config_not_allowed_in_namespace: ($) => prec(-1, $.config_decl),

    ERR_single_after_interval: ($) => prec(-1, seq("SINGLE", ":=", field("single", $.data_source), ",")),
    ERR_interval_before_single: ($) => prec(-1, seq("INTERVAL", ":=", field("interval", $.data_source), ",")),
    ERR_interval_after_priority: ($) => prec(-1, seq(",", "INTERVAL", ":=", field("interval", $.data_source))),
    ERR_single_after_priorty: ($) => prec(-1, seq(",", "SINGLE", ":=", field("single", $.data_source))),

    // Invalid variable sections
    ERR_var_in_out_not_allowed: ($) => prec(-1, $.in_out_decls), // VAR_IN_OUT
    ERR_var_temp_not_allowed: ($) => prec(-1, $.temp_var_decls), // VAR_TEMP
    ERR_var_access_not_allowed: ($) => prec(-1, $.prog_access_decls), // VAR_ACCESS
    ERR_var_config_not_allowed: ($) => prec(-1, $.config_init), // VAR_CONFIG
    ERR_var_located_not_allowed: ($) => prec(-1, $.loc_var_decls), // VAR_LOCATED
    ERR_var_external_not_allowed: ($) => prec(-1, $.external_var_decls), // VAR_EXTERNAL
    ERR_var_global_not_allowed: ($) => prec(-1, $.global_var_decls), // VAR_GLOBAL
    ERR_var_not_allowed: ($) => prec(-1, $.var_decls), // VAR

    // Table 3 - Comments

    line_comment: ($) => token(seq("//", /.*/)),

    c_style_comment: ($) => seq("/*", optional($.comment_text), "*/"),

    pascal_style_comment: ($) => seq("(*", optional($.comment_text), "*)"),

    comment_text: ($) => repeat1(/.|\n|\r/),

    // Table 4 - Pragma

    pragma: ($) => seq("{", repeat(choice(/[^*]/, /\*[^)]/)), "}"),

    // Table 5 - Numeric literal

    constant: ($) =>
      choice($.numeric_literal, $.char_literal, $.time_literal, $.bool_literal),

    numeric_literal: ($) => choice($.real_literal, $.int_literal),

    int_literal: ($) =>
      seq(
        optional(seq(field("kind", $.int_kind), "#")),
        field(
          "int",
          choice($.signed_int, $.binary_int, $.octal_int, $.hex_int),
        ),
      ),

    int_kind: ($) => choice($.multibits_type_name, $.int_type_name),

    unsigned_int: ($) => token(/[0-9][0-9_]*/),

    // The precedence here is necessary to avoid conflicts with the unary minus operator
    signed_int: ($) =>
      prec(RK_PREC.unary + 1, seq(optional(choice("+", "-")), $.unsigned_int)),

    binary_int: ($) => seq("2#", field("value", $._bit_value)),

    octal_int: ($) => seq("8#", field("value", $._octal_value)),

    hex_int: ($) => seq("16#", field("value", $._hex_value)),

    real_literal: ($) =>
      seq(
        optional(seq(field("type", $.real_type_name), "#")),
        // Same as Unsigned int, but with a dot and optional exponent
        prec(
          RK_PREC.unary + 1,
          seq(optional(choice("+", "-")), field("value", $.real_value)),
        ),
      ),

    real_value: ($) =>
      token(/[0-9][0-9_]*\.[0-9][0-9_]*([eE][-+]?[0-9][0-9_]*)?/),

    bool_literal: ($) =>
      choice($.bool_literal_with_string, $.bool_literal_with_numeric),

    bool_literal_with_string: ($) =>
      seq(optional("BOOL#"), field("value", choice("TRUE", "FALSE"))),
    bool_literal_with_numeric: ($) =>
      seq("BOOL#", field("value", choice("0", "1"))),

    // Table 6 - Character String literals
    // Table 7 - Two-character combinations in character strings

    char_literal: ($) =>
      seq(
        optional(seq(field("kind", $.string_type_name), "#")),
        field("value", $.char_str),
      ),

    char_str: ($) =>
      choice(prec(-1, $.hex_int), $.s_byte_char_str, $.d_byte_char_str),

    s_byte_char_str: ($) => seq("'", repeat($._s_byte_char_value), "'"),

    d_byte_char_str: ($) => seq('"', repeat($._d_byte_char_value), '"'),

    _s_byte_char_value: ($) =>
      choice(
        $._common_char_value,
        token("$'"),
        token('"'),
        seq("$", $._hex_digit, $._hex_digit),
      ),

    _d_byte_char_value: ($) =>
      choice(
        $._common_char_value,
        token("'"),
        token('$"'),
        seq("$", repeat1($._hex_digit)),
      ),

    _common_char_value: ($) =>
      choice(
        /[ !#%&]/,
        /[\(\)\*\+,\-\.\/]/,
        /[0-9]/,
        /[:;<=>?@]/,
        /[A-Z]/,
        /[\[\]\\\^_`]/,
        /[a-z]/,
        /[{\|}~]/,
        "$$",
        "$L",
        "$N",
        "$P",
        "$R",
        "$T",
      ),

    // Table 8 - Duration literals
    // Table 9 – Date and time of day literals

    time_literal: ($) =>
      choice($.duration, $.time_of_day, $.date, $.date_and_time),

    duration: ($) => choice($.time, $.ltime),

    time: ($) =>
      seq(
        alias(/(TIME|T|time|t)#/, $.time_type_name),
        field("sign", optional(choice("+", "-"))),
        field("value", $.time_value),
      ),

    ltime: ($) =>
      seq(
        alias(/(LTIME|LT|ltime|lt)#/, $.l_time_type_name),
        field("sign", optional(choice("+", "-"))),
        field("value", $.time_value),
      ),

    time_value: ($) => /([0-9._]+(d|h|ms|us|ns|m|s))+/,

    fix_point: ($) =>
      seq(field("real", $.unsigned_int), ".", field("frac", $.unsigned_int)),

    time_of_day: ($) => choice($.tod, $.ltod),

    tod: ($) =>
      seq(
        alias(/(TOD|TIME_OF_DAY|tod)#/, $.tod_type_name),
        field("value", $.daytime),
      ),

    ltod: ($) =>
      seq(
        alias(/(LTOD|LTIME_OF_DAY|ltod)#/, $.ltod_type_name),
        field("value", $.daytime),
      ),

    daytime: ($) => /[0-9a-zA-Z_.:]+/,

    date: ($) => choice($.short_date, $.long_date),

    short_date: ($) =>
      seq(
        alias(/(DATE|D|date|d)#/, $.date_type_name),
        field("value", $.date_literal),
      ),

    long_date: ($) =>
      seq(
        alias(/(LDATE|LD|ldate|ld)#/, $.date_type_name),
        field("value", $.date_literal),
      ),

    date_literal: ($) => /[0-9a-zA-Z_.:-]+/,

    date_and_time: ($) => choice($.short_date_and_time, $.long_date_and_time),

    short_date_and_time: ($) =>
      seq(
        alias(/(DATE_AND_TIME|DT)#/, $.date_and_time_type_name),
        field("value", $.date_and_daytime),
      ),

    long_date_and_time: ($) =>
      seq(
        alias(/(LDATE_AND_TIME|LDT)#/, $.l_date_and_time_type_name),
        field("value", $.date_and_daytime),
      ),

    any_date_and_time_type_name: ($) =>
      choice($.date_and_time_type_name, $.l_date_and_time_type_name),

    date_and_time_type_name: ($) => /DATE_AND_TIME|DT/,
    l_date_and_time_type_name: ($) => /LDATE_AND_TIME|LDT/,

    date_and_daytime: ($) => /[0-9dhmsDHMS_.:-]+/,

    // Table 10 - Elementary data types

    data_type_access: ($) =>
      choice($.namespace_access, $._elem_type_name),

    _elem_type_name: ($) =>
      choice(
        $.numeric_type_name,
        $.bit_str_type_name,
        $.any_date_type_name,
        $.any_time_type_name,
        $.any_tod_type_name,
        $.any_dt_type_name,
        $.string_type_name
      ),

    numeric_type_name: ($) => choice($.int_type_name, $.real_type_name),

    int_type_name: ($) => choice($.sign_int_type_name, $.unsign_int_type_name),

    sign_int_type_name: ($) =>
      choice(
        alias("SINT", $.sint_name),
        alias("INT", $.int_name),
        alias("DINT", $.dint_name),
        alias("LINT", $.lint_name),
      ),

    unsign_int_type_name: ($) =>
      choice(
        alias("USINT", $.usint_name),
        alias("UINT", $.uint_name),
        alias("UDINT", $.udint_name),
        alias("ULINT", $.ulint_name),
      ),

    real_type_name: ($) =>
      choice(alias("REAL", $.real_name), alias("LREAL", $.lreal_name)),

    string_type_name: ($) =>
      choice(
        $.string_name,
        $.wstring_name,
        $.char_name,
        $.wchar_name,
      ),

    string_name: ($) => seq("STRING", optional(seq("[", $.unsigned_int, "]"))),
    wstring_name: ($) => seq("WSTRING", optional(seq("[", $.unsigned_int, "]"))),
    char_name: ($) => "CHAR",
    wchar_name: ($) => "WCHAR",

    any_time_type_name: ($) => choice($.time_type_name, $.l_time_type_name),

    time_type_name: ($) => /TIME|time/,
    l_time_type_name: ($) => /LTIME|ltime/,

    any_date_type_name: ($) => choice($.date_type_name, $.l_date_type_name),

    date_type_name: ($) => /DATE|date/,
    l_date_type_name: ($) => /LDATE|ldate/,

    any_tod_type_name: ($) => choice($.tod_type_name, $.ltod_type_name),

    tod_type_name: ($) => /TOD|TIME_OF_DAY|tod/,
    ltod_type_name: ($) => /LTOD|LTIME_OF_DAY|ltod/,

    any_dt_type_name: ($) => choice($.dt_type_name, $.l_dt_type_name),

    dt_type_name: ($) => /DATE_AND_TIME|DT/,
    l_dt_type_name: ($) => /LDATE_AND_TIME|LDT/,

    bit_str_type_name: ($) =>
      choice(alias("BOOL", $.bool_name), $.multibits_type_name),

    multibits_type_name: ($) =>
      choice(
        alias("BYTE", $.byte_name),
        alias("WORD", $.word_name),
        alias("DWORD", $.dword_name),
        alias("LWORD", $.lword_name),
      ),

    // Table 11 - Declaration of user-defined data types and initialization

    data_type_decl: ($) =>
      seq("TYPE", repeat(seq($.type_decl, optional(";"))), "END_TYPE"),

    // Type_Decl : Simple_Type_Decl | Subrange_Type_Decl | Enum_Type_Decl | Array_Type_Decl | Struct_Type_Decl |
    //  Str_Type_Decl | Ref_Type_Decl;
    type_decl: ($) =>
      seq(
        field("name", $.identifier),
        useSpecInit(
          ["simple", "subrange", "enum", "array", "struct", "ref"],
          [
            "simple",
            //"subrange", handled by simple
            //"enum", handled by simple
            "array",
            "struct",
            // "ref", handled in primary_expression
          ],
        )($),
      ),

    ...createSpecInit(
      "simple",
      ($) => $.data_type_access,
      ($) => seq(":=", $.constant_expr),
    ),

    ...createSpecInit(
      "subrange",
      ($) =>
        seq(
          field("type", $._elem_type_name),
          "(",
          field("range", $.subrange),
          ")",
        ),
      ($) => $.simple_type_init,
    ),

    subrange: ($) =>
      seq(
        field("lower", $.constant_expr),
        "..",
        field("upper", $.constant_expr),
      ),

    // Enum_Type_Decl : Enum_Type_Name ':' ( ( Elem_Type_Name ? Named_Spec_Init ) | Enum_Spec_Init );
    // Enum_Spec_Init : ( ( '(' Identifier ( ',' Identifier )* ')' ) | Enum_Type_Access ) ( ':=' Enum_Value )?;
    ...createSpecInit(
      "enum",
      ($) => $.enum_spec,
      ($) => seq(":=", $.namespace_access),
    ),

    //enum_spec: $ => seq('(', commaSep($.identifier), ')'),

    // Named_Spec_Init : '(' Enum_Value_Spec ( ',' Enum_Value_Spec )* ')' ( ':=' Enum_Value )?;
    enum_spec: ($) =>
      seq(
        field("elem_type", optional($._elem_type_name)),
        "(",
        commaSep1($.enum_value_spec),
        ")",
      ),

    enum_value_spec: ($) =>
      seq(field("value", $.identifier), optional(seq(":=", $._expression))),

    enum_value: ($) =>
      seq(field("enum_path", $.begin_path_expression), "#", $.identifier),

    ...createSpecInit(
      "array",
      ($) =>
        seq(
          "ARRAY",
          "[",
          field("ranges", $.ranges),
          "]",
          "OF",
          useSpec(["simple", "ref"])($),
        ),
      ($) => seq(":=", "[", commaSep($.init_elem), "]"),
    ),

    ranges: ($) => commaSep1($.subrange),

    ...createSpecInit(
      "struct",
      ($) =>
        seq(
          "STRUCT",
          optional(";"),
          field("overlap", optional("OVERLAP")),
          repeat(seq($.struct_elem_decl, optional(";"))),
          "END_STRUCT",
        ),
      ($) => seq(":=", "(", commaSep($.init_elem), ")"),
    ),

    init_elem: ($) =>
      choice(
        $.struct_elem,
        $.struct_init,
        $.array_init,
        $.array_index_elem,
        $.ERR_func_call_in_init,
        $.constant_expr,
      ),

    array_init: ($) => seq("[", field("values", $.array_values), "]"),

    array_index_elem: ($) =>
      seq(
        field("index", $.unsigned_int),
        "(",
        field("values", $.array_values),
        ")",
      ),

    // Placeholder for array values
    array_values: ($) => commaSep1($.init_elem),

    struct_init: ($) => seq("(", commaSep($.struct_elem), ")"),

    struct_elem: ($) =>
      seq(field("name", $.identifier), ":=", field("value", $.init_elem)),

    // Struct_Elem_Decl : Struct_Elem_Name ( Located_At Multibit_Part_Access ? )? ':'
    // ( Simple_Spec_Init | Subrange_Spec_Init | Enum_Spec_Init | Array_Spec_Init | Struct_Spec_Init );
    struct_elem_decl: ($) =>
      seq(
        field("name", $.identifier),
        field("attributes", optional($.struct_elem_decl_attributes)),
        useSpecInit(
          ["simple", "subrange", "enum", "array", "struct", "ref"],
          [
            "simple",
            // "subrange", handled by simple
            "array",
            "struct",
            // "enum" handled by simple
          ],
        )($),
      ),

    struct_elem_decl_attributes: ($) =>
      seq(
        field("located", $.located_at),
        optional(field("multibits", $.multibit_part_access)),
      ),

    // Table 16 - Directly represented variables

    direct_variable: ($) =>
      seq(
        "%",
        // Parsed in the HIR
        field("adress", $.adress_identifier),
        field("offset", choice(alias("*", $.partly), $.offset)),
      ),

    offset: ($) => prec.left(dotSep1($.unsigned_int)),

    // Table 12 - Reference operations

    ...createSpecInit(
      "ref",
      ($) => seq(field("ref_count", repeat1(alias("REF_TO", $.ref_to))), $.data_type_access),
      ($) => seq(":=", $.ref_value),
    ),

    ref_type_decl: ($) =>
      seq(field("name", $.identifier), ":", $.ref_spec_init),

    ref_spec_init: ($) =>
      prec.left(seq($.ref_spec, optional(seq(":=", $.ref_value)))),

    ref_spec: ($) => seq("REF_TO", $.data_type_access),

    ref_value: ($) => choice($.ref_addr, alias("NULL", $.null)),

    ref_addr: ($) => seq("REF", "(", $.begin_path_expression, ")"),

    deref_expression: ($) =>
      prec(
        RK_PREC.dereference,
        seq($.path_expression, repeat1(alias("^", $.deref_sign))),
      ),

    // Table 13 - Declaration of variables/Table 14 – Initialization of variables

    variable: ($) => choice($.begin_path_expression, $.direct_variable),

    // Var_Access : Variable_Name;
    var_access: ($) =>
      choice(
        $.ERR_unexpected_this_in_path,
        $.ERR_unexpected_super_in_path,
        alias($.identifier, $.field),
      ),

    input_decls: ($) =>
      seq(
        "VAR_INPUT",
        field("retain", optional(choice("RETAIN", "NON_RETAIN"))),
        repeat(
          seq(choice($.input_var, $.ERR_variable_with_no_spec), optional(";")),
        ),
        "END_VAR",
        optional(";"),
      ),

    input_var: ($) =>
      seq(
        field("variables", $.variable_list),
        field("type", $._input_var_kind),
      ),

    _input_var_kind: ($) =>
      choice($.var_decl_init, $.edge_decl, $.array_conformand, $.variadic_decl),

    variadic_decl: ($) =>
      seq(":", field("type", $.data_type_access), "..."),

    edge_decl: ($) =>
      seq(
        ":",
        "BOOL",
        field("edge", choice("R_EDGE", "F_EDGE", $.ERR_invalid_edge_qualifier)),
      ),

    // : Variable_List ':' ( Simple_Spec_Init | Str_Var_Decl | Ref_Spec_Init )
    //| Array_Var_Decl_Init | Struct_Var_Decl_Init | FB_Decl_Init | Interface_Spec_Init;

    // INPUTS
    // OUTPUTS
    // VARS
    // RETAIN
    var_decl_init: ($) =>
      useSpecInit(
        ["simple", "ref", "array", "struct"],
        [
          "simple",
          "array",
          "struct",
        ],
      )($),

    // ( Simple_Spec | Str_Var_Decl | Array_Var_Decl | Struct_Var_Decl )

    // INOUT
    // TEMP
    var_decl: ($) =>
      useSpecInit(
        ["simple", "array", "struct"],
        [
          "simple",
          "array",
          "struct",
        ],
      )($),

    variable_list: ($) => commaSep1($.identifier),

    array_conformand: ($) =>
      seq(":", "ARRAY", "[", commaSep1("*"), "]", "OF", $.data_type_access),

    output_decls: ($) =>
      seq(
        "VAR_OUTPUT",
        optional(choice("RETAIN", "NON_RETAIN")),
        repeat(
          seq(choice($.output_var, $.ERR_variable_with_no_spec), optional(";")),
        ),
        "END_VAR",
        optional(";"),
      ),

    output_var: ($) =>
      seq(
        field("variables", $.variable_list),
        field("type", $._output_var_kind),
      ),

    _output_var_kind: ($) => choice($.var_decl_init, $.array_conformand),

    in_out_decls: ($) =>
      seq(
        "VAR_IN_OUT",
        repeat(
          seq(choice($.in_out_var, $.ERR_variable_with_no_spec), optional(";")),
        ),
        "END_VAR",
        optional(";"),
      ),

    in_out_var: ($) =>
      seq(
        field("variables", $.variable_list),
        field("type", $._in_out_var_kind),
      ),

    _in_out_var_kind: ($) => choice($.var_decl, $.array_conformand),

    var_decls: ($) =>
      seq(
        "VAR",
        field("constant", optional("CONSTANT")),
        field("access", optional($.access_spec)),
        repeat(
          seq(
            choice($.var_decl_init_list, $.ERR_variable_with_no_spec),
            optional(";"),
          ),
        ),
        "END_VAR",
        optional(";"),
      ),

    retain_var_decls: ($) =>
      seq(
        "VAR",
        field("retain", "RETAIN"),
        field("access", optional($.access_spec)),
        repeat(
          seq(
            choice($.var_decl_init_list, $.ERR_variable_with_no_spec),
            optional(";"),
          ),
        ),
        "END_VAR",
        optional(";"),
      ),

    var_decl_init_list: ($) =>
      seq(field("variables", $.variable_list), field("type", $.var_decl_init)),

    loc_var_decls: ($) =>
      seq(
        "VAR_LOCATED",
        field(
          "constant_or_retain",
          optional(choice("CONSTANT", "RETAIN", "NON_RETAIN")),
        ),
        repeat(seq($.loc_var_decl, optional(";"))),
        "END_VAR",
        optional(";"),
      ),

    loc_var_decl: ($) =>
      seq(
        optional(field("variable_name", $.identifier)),
        field("located_at", $.located_at),
        field("spec_init", $.loc_var_spec_init),
      ),

    temp_var_decls: ($) =>
      seq(
        "VAR_TEMP",
        repeat(
          seq(choice($.temp_var, $.ERR_variable_with_no_spec), optional(";")),
        ),
        "END_VAR",
        optional(";"),
      ),

    temp_var: ($) =>
      seq(field("variables", $.variable_list), field("type", $._temp_var_kind)),

    _temp_var_kind: ($) => choice($.var_decl, $.ref_spec),

    external_var_decls: ($) =>
      seq(
        "VAR_EXTERNAL",
        field("constant", optional("CONSTANT")),
        repeat(
          seq(
            choice($.external_decl, $.ERR_variable_with_no_spec),
            optional(";"),
          ),
        ),
        "END_VAR",
        optional(";"),
      ),

    external_decl: ($) =>
      seq(field("name", $.identifier), field("type", $._external_var_kind)),

    _external_var_kind: ($) => choice($.var_decl, $.array_conformand),

    // Global_Var_Decls : 'VAR_GLOBAL' ( 'CONSTANT' | 'RETAIN' )? ( Global_Var_Decl ';' )* 'END_VAR';
    global_var_decls: ($) =>
      seq(
        "VAR_GLOBAL",
        field("constant_or_retain", optional(choice("CONSTANT", "RETAIN"))),
        repeat(seq($.global_var_decl, optional(";"))),
        "END_VAR",
        optional(";"),
      ),

    // Global_Var_Decl : Global_Var_Spec ':' ( Loc_Var_Spec_Init | FB_Type_Access );
    global_var_decl: ($) =>
      seq(
        field("spec", $.global_var_spec),
        field("type", $._global_var_kind),
      ),
    _global_var_kind: ($) => choice($.loc_var_spec_init, $.namespace_access),

    // Global_Var_Spec : ( Global_Var_Name ( ',' Global_Var_Name )* ) | ( Global_Var_Name Located_At );
    global_var_spec: ($) =>
      choice(
        seq(commaSep1(field("name", $.identifier))),
        seq(field("name", $.identifier), $.located_at),
      ),

    // Loc_Var_Spec_Init : Simple_Spec_Init | Array_Spec_Init | Struct_Spec_Init | S_Byte_Str_Spec | D_Byte_Str_Spec;
    loc_var_spec_init: ($) =>
      useSpecInit(
        ["simple", "array", "struct"],
        [
          "simple",
          "array",
          "struct",
        ],
      )($),

    located_at: ($) => seq("AT", $.direct_variable),

    loc_partly_var_decl: ($) =>
      seq(
        "VAR",
        field("retain", optional(choice("RETAIN", "NON_RETAIN"))),
        repeat1(seq($.loc_partly_var, optional(";"))),
        "END_VAR",
        optional(";"),
      ),

    loc_partly_var: ($) =>
      seq(
        field("variable_name", $.identifier),
        "AT",
        "%",
        field("IQM", $.IQM),
        "*",
        ":",
        field("spec", $.var_spec),
      ),

    // Var_Spec : Simple_Spec | Array_Spec | Struct_Type_Access
    // | ( 'STRING' | 'WSTRING' ) ( '[' Unsigned_Int ']' )?;
    var_spec: ($) => $.data_type_access,

    // Table 19 - Function declaration

    func_decl: ($) =>
      seq(
        "FUNCTION",
        field("spec", optional($.access_spec)),
        field("name", $.identifier),
        field("generic_spec", optional($.generic_spec)),
        optional(seq(":", field("return_type", $.data_type_access))),
        field("directives", repeat($.using_directive)),
        field("variables", repeat($._func_variables)),
        field("body", optional($.func_body)),
        "END_FUNCTION",
      ),

    _func_variables: ($) =>
      choice(...io_var_decls($), ...func_var_decls($),
        $.temp_var_decls,
        $.ERR_var_access_not_allowed,
        $.ERR_var_config_not_allowed,
        $.ERR_var_located_not_allowed,
        $.ERR_var_external_not_allowed,
        $.ERR_var_global_not_allowed
      ),

    func_body: ($) => choice($.ladder_diagram, $.fb_diagram, $.stmt_list),

    // Table 40 – Function block type declaration
    // Table 41 - Function block instance declaration

    fb_decl: ($) =>
      seq(
        "FUNCTION_BLOCK",
        field("qualifier", optional(choice("FINAL", "ABSTRACT"))),
        field("name", $.identifier),
        field("generic_spec", optional($.generic_spec)),
        optional($.ERR_implements_before_extends),
        optional(seq("EXTENDS", field("extends", $.namespace_access))),
        optional(repeat($.ERR_extends_multiple_times)),
        optional(seq("IMPLEMENTS", field("implements", $.interface_name_list))),
        optional(repeat($.ERR_implements_multiple_times)),
        field("directives", repeat($.using_directive)),
        field("variables", repeat($._fb_variables)),
        field("method", repeat($.method_decl)),
        optional($.ERR_fb_variables_after_method),
        field("body", optional($.fb_body)),
        "END_FUNCTION_BLOCK",
      ),

    _fb_variables: ($) =>
      choice(
        $.fb_input_decls,
        $.fb_output_decls,
        $.in_out_decls,
        $.temp_var_decls,
        ...func_var_decls($),
        ...other_var_decls($),
        $.ERR_var_access_not_allowed,
        $.ERR_var_config_not_allowed,
        $.ERR_var_located_not_allowed,
        $.ERR_var_global_not_allowed,
      ),

    fb_input_decls: ($) =>
      seq(
        "VAR_INPUT",
        optional(choice("RETAIN", "NON_RETAIN")),
        repeat(
          seq(
            choice($.fb_input_var, $.ERR_variable_with_no_spec),
            optional(";"),
          ),
        ),
        "END_VAR",
        optional(";"),
      ),

    fb_input_var: ($) =>
      seq(
        field("variables", $.variable_list),
        field("type", $._fb_input_var_kind),
      ),

    _fb_input_var_kind: ($) =>
      choice($.var_decl_init, $.edge_decl, $.array_conformand),

    fb_output_decls: ($) =>
      seq(
        "VAR_OUTPUT",
        optional(choice("RETAIN", "NON_RETAIN")),
        repeat(
          seq(
            choice($.fb_output_var, $.ERR_variable_with_no_spec),
            optional(";"),
          ),
        ),
        "END_VAR",
        optional(";"),
      ),

    fb_output_var: ($) =>
      seq(
        field("variables", $.variable_list),
        field("type", $._fb_output_var_kind),
      ),

    _fb_output_var_kind: ($) => choice($.var_decl_init, $.array_conformand),

    no_retain_var_decls: ($) =>
      seq(
        "VAR",
        "NON_RETAIN",
        field("spec", optional($.access_spec)),
        repeat(
          seq(
            choice($.var_decl_init_list, $.ERR_variable_with_no_spec),
            optional(";"),
          ),
        ),
        "END_VAR",
        optional(";"),
      ),

    fb_body: ($) => choice($.SFC, $.ladder_diagram, $.fb_diagram, $.stmt_list),

    method_decl: ($) =>
      seq(
        "METHOD",
        field("access", optional($.access_spec)),
        field("modifier", optional(choice("FINAL", "ABSTRACT"))),
        field("_override", optional("OVERRIDE")),
        field("name", $.identifier),
        optional(seq(":", field("return_type", $.data_type_access))),
        field(
          "variables",
          repeat(
            $._method_decl_variables
          ),
        ),
        field("body", optional($.func_body)),
        "END_METHOD",
      ),

    _method_decl_variables: ($) =>
      choice(
        ...io_var_decls($),
        ...func_var_decls($),
        $.temp_var_decls,
        $.ERR_var_access_not_allowed,
        $.ERR_var_config_not_allowed,
        $.ERR_var_located_not_allowed,
        $.ERR_var_external_not_allowed,
        $.ERR_var_global_not_allowed,
      ),

    // Table 48 - Class
    // Table 50 Textual call of methods – Formal and non-formal parameter list

    class_decl: ($) =>
      seq(
        "CLASS",
        field("modifier", optional(choice("FINAL", "ABSTRACT"))),
        field("name", $.identifier),
        field("generic_spec", optional($.generic_spec)),
        field("directives", repeat($.using_directive)),
        optional($.ERR_implements_before_extends),
        optional(seq("EXTENDS", field("extends", $.namespace_access))),
        optional(repeat($.ERR_extends_multiple_times)),
        optional(seq("IMPLEMENTS", field("implements", $.interface_name_list))),
        optional(repeat($.ERR_implements_multiple_times)),
        field("variables", repeat($._class_variables)),
        field("methods", repeat($.method_decl)),
        optional($.ERR_class_variables_after_method),
        "END_CLASS",
      ),

    _class_variables: ($) =>
      choice(...func_var_decls($), ...other_var_decls($),
        $.ERR_var_in_out_not_allowed,
        $.ERR_var_temp_not_allowed,
        $.ERR_var_access_not_allowed,
        $.ERR_var_config_not_allowed,
        $.ERR_var_located_not_allowed,
        $.ERR_var_external_not_allowed,
        $.ERR_var_global_not_allowed
      ),

    interface_decl: ($) =>
      seq(
        "INTERFACE",
        field("name", $.identifier),
        field("directives", repeat($.using_directive)),
        optional(seq("EXTENDS", field("extends", $.interface_name_list))),
        optional(repeat($.ERR_extends_multiple_times)),
        field("prototype", repeat($.method_prototype)),
        "END_INTERFACE",
      ),

    method_prototype: ($) =>
      seq(
        "METHOD",
        field("name", $.identifier),
        optional(seq(":", field("data_type", $.data_type_access))),
        field("variables", repeat($._method_prot_variables)),
        "END_METHOD",
      ),

    _method_prot_variables: ($) =>
      choice(...io_var_decls($),
        $.ERR_var_access_not_allowed,
        $.ERR_var_config_not_allowed,
        $.ERR_var_located_not_allowed,
        $.ERR_var_external_not_allowed,
        $.ERR_var_global_not_allowed,
        // temp and in_out are not allowed too
        $.ERR_var_temp_not_allowed,
        $.ERR_var_in_out_not_allowed
      ),

    interface_spec_init: ($) => seq(":=", $.interface_value),

    interface_value: ($) => choice($.begin_path_expression, "NULL"),

    interface_name_list: ($) => commaSep1($.namespace_access),

    interface_name: ($) => $.identifier,

    access_spec: ($) =>
      choice(
        alias("PUBLIC", $.public),
        alias("PROTECTED", $.protected),
        alias("PRIVATE", $.private),
        alias("INTERNAL", $.internal),
      ),

    // Table 47 - Program declaration

    prog_decl: ($) =>
      seq(
        "PROGRAM",
        field("name", $.identifier),
        field(
          "declarations",
          repeat(
            choice(
              ...io_var_decls($),
              ...func_var_decls($),
              $.temp_var_decls,
              ...other_var_decls($),
              $.loc_var_decls,
              $.prog_access_decls,
              $.global_var_decls,
            ),
          ),
        ),
        field("body", optional($.fb_body)),
        "END_PROGRAM",
      ),

    prog_access_decls: ($) =>
      seq(
        "VAR_ACCESS",
        repeat(seq($.prog_access_decl, optional(";"))),
        "END_VAR",
      ),

    // Prog_Access_Decl : Access_Name ':' Symbolic_Variable Multibit_Part_Access ?
    // ':' Data_Type_Access Access_Direction ?;
    prog_access_decl: ($) =>
      seq(
        field("name", $.identifier),
        ":",
        field("variable", $.path_expression),
        optional(seq(".", $.direct_variable)),
        ":",
        field("access", $.data_type_access),
        field("direction", optional($.access_direction)),
      ),

    // Table 54 - 61 - Sequential Function Chart (SFC)

    SFC: ($) => repeat1($.SFC_network),

    SFC_network: ($) =>
      seq($.initial_step, repeat(choice($.step, $.transition, $.action))),

    initial_step: ($) =>
      seq(
        "INITIAL_STEP",
        $.step_name,
        ":",
        repeat(seq($.action_association, optional(";"))),
        "END_STEP",
      ),

    step: ($) =>
      seq(
        "STEP",
        $.step_name,
        ":",
        repeat(seq($.action_association, optional(";"))),
        "END_STEP",
      ),

    step_name: ($) => $.identifier,

    action_association: ($) =>
      seq(
        $.action_name,
        "(",
        optional($.action_qualifier),
        repeat(seq(field("variable_name", $.identifier), optional(";"))),
        ")",
      ),

    action_name: ($) => $.identifier,

    action_qualifier: ($) =>
      choice(
        "N",
        "R",
        "S",
        "P",
        seq(choice("L", "D", "SD", "DS", "SL"), ",", $.action_time),
      ),

    action_time: ($) =>
      choice($.duration, field("variable_name", $.identifier)),

    transition: ($) =>
      seq(
        "TRANSITION",
        optional($.transition_name),
        ":",
        optional(seq("(", "PRIORITY", ":=", $.unsigned_int, ")")),
        "FROM",
        $.steps,
        "TO",
        $.steps,
        $.transition_cond,
        "END_TRANSITION",
      ),

    transition_name: ($) => $.identifier,

    steps: ($) =>
      choice(
        $.step_name,
        seq("(", $.step_name, repeat1(seq(",", $.step_name)), ")"),
      ),

    transition_cond: ($) =>
      choice(
        seq(":=", $._expression, optional(";")),
        seq(":", choice($.fbd_network, $.ld_rung)),
      ),

    action: ($) => seq("ACTION", $.action_name, ":", $.fb_body, "END_ACTION"),

    // Table 62 - Configuration and resource declaration

    config_decl: ($) =>
      seq(
        "CONFIGURATION",
        field("name", $.identifier),
        field("global_variables", repeat($._config_variables)),
        field(
          "resources",
          repeat(choice($.single_resource_decl, $.resource_decl)),
        ),
        field("access_decls", optional($.access_decls)),
        field("config_init", optional($.config_init)),
        "END_CONFIGURATION",
      ),

    _config_variables: ($) =>
      choice(
        $.global_var_decls,
        $.ERR_var_not_allowed,
        $.ERR_var_in_out_not_allowed,
        $.ERR_var_temp_not_allowed,
        $.ERR_var_config_not_allowed,
        $.ERR_var_located_not_allowed,
        $.ERR_var_external_not_allowed,
      ),

    resource_decl: ($) =>
      seq(
        "RESOURCE",
        field("name", $.identifier),
        "ON",
        field("resource_type_name", $.identifier),
        field("global_variables", optional($.global_var_decls)),
        field("resource", repeat($.single_resource_decl)),
        "END_RESOURCE",
      ),

    single_resource_decl: ($) =>
      seq(choice($.task_config, $.prog_config), optional(";")),

    access_decls: ($) =>
      seq(
        "VAR_ACCESS",
        repeat(seq($.access_decl, optional(";"))),
        "END_VAR",
        optional(";"),
      ),

    // Access_Decl : Access_Name ':' Access_Path ':' Data_Type_Access Access_Direction ?;
    access_decl: ($) =>
      seq(
        field("name", $.identifier),
        ":",
        field("path", $.access_path),
        ":",
        field("access", $.data_type_access),
        field("direction", optional($.access_direction)),
      ),

    // Access_Path : ( Resource_Name '.' )? Direct_Variable | ( Resource_Name '.' )? ( Prog_Name '.' )?
    // ( ( FB_Instance_Name | Class_Instance_Name ) '.' )* Symbolic_Variable;
    access_path: ($) =>
      seq(
        field("path", $.path_expression),
        optional(seq(".", field("direct", $.direct_variable))),
      ),

    access_direction: ($) => choice(alias("READ_WRITE", $.read_write), alias("READ_ONLY", $.read_only)),

    task_config: ($) =>
      seq("TASK", field("name", $.identifier), field("init", $.task_init)),

    task_init: ($) =>
      seq(
        "(",
        optional(seq("SINGLE", ":=", field("single", $.data_source), ",")),
        optional(seq("INTERVAL", ":=", field("interval", $.data_source), ",")),
        optional($.ERR_single_after_interval),
        "PRIORITY", ":=", field("priority", $.unsigned_int),
        optional($.ERR_interval_after_priority),
        optional($.ERR_single_after_priorty),
        ")",

      ),

    data_source: ($) =>
      choice($.constant, $.path_expression, $.direct_variable),

    prog_config: ($) =>
      seq(
        "PROGRAM",
        field("retain", optional(choice("RETAIN", "NON_RETAIN"))),
        field("name", $.identifier),
        field("task", optional(seq("WITH", $.identifier))),
        ":",
        field("access", $.namespace_access),
        field("configuration_elements", optional($.prog_conf_elems)),
      ),

    prog_conf_elems: ($) => seq("(", commaSep($.prog_conf_elem), ")"),

    prog_conf_elem: ($) => choice($.fb_task, $.prog_cnxn),

    fb_task: ($) => seq($.path_expression, "WITH", field("task", $.identifier)),

    prog_cnxn: ($) =>
      choice(
        seq($.path_expression, ":=", $.prog_data_source),
        seq($.path_expression, "=>", $.data_sink),
      ),

    prog_data_source: ($) =>
      choice($.constant, $.path_expression, $.direct_variable),

    data_sink: ($) => choice($.path_expression, $.direct_variable),

    config_init: ($) =>
      seq(
        "VAR_CONFIG",
        repeat(seq($.config_inst_init, optional(";"))),
        "END_VAR",
        optional(";"),
      ),

    // Config_Inst_Init : Resource_Name '.' Prog_Name '.' ( ( FB_Instance_Name | Class_Instance_Name ) '.' )*
    // ( Variable_Name Located_At ? ':' Loc_Var_Spec_Init
    // | ( ( FB_Instance_Name ':' FB_Type_Access )
    // | ( Class_Instance_Name ':' Class_Type_Access ) ) ':=' Struct_Init );
    config_inst_init: ($) =>
      seq(
        field("path", $.path_expression),
        seq(optional($.located_at), $.loc_var_spec_init),
      ),

    // Table 64 - Namespace

    namespace_decl: ($) =>
      seq(
        "NAMESPACE",
        field("internal", optional("INTERNAL")),
        field("name", $.namespace_h_name),
        field("directives", repeat($.using_directive)),
        field("elements", optional($.namespace_elements)),
        "END_NAMESPACE",
      ),

    namespace_elements: ($) =>
      repeat1(
        choice(
          $.data_type_decl,
          $.func_decl,
          $.fb_decl,
          $.class_decl,
          $.interface_decl,
          $.namespace_decl,
          $.ERR_invalid_pou_keyword,
          $.ERR_program_not_allowed_in_namespace,
          $.ERR_config_not_allowed_in_namespace,
        ),
      ),

    namespace_h_name: ($) => dotSep1($.identifier),

    using_directive: ($) =>
      seq("USING", commaSep1($.namespace_h_name), optional(";")),

    // Table 71 - 72 - Language Structured Text (ST)

    // Expressions are merged into a single _expression rule
    // to allow for better precedence handling and operator overloading.
    _expression: ($) =>
      choice(
        $.fold_expression,
        $.boolean_operator,
        $.comparison_operator,
        $.add_operator,
        $.mult_operator,
        $.power_operator,
        $.unary_operator,
        $._primary_expression,
      ),

    // Fold expression: variadic parameter folded with a binary operator
    // e.g. ...IN+ (left fold: IN[0] + IN[1] + ... + IN[N-1])
    fold_expression: ($) =>
      seq(
        "...",
        field("param", $.identifier),
        field("operator", $.fold_operator),
      ),

    fold_operator: ($) =>
      choice(
        // Arithmetic
        alias("+", $.fold_plus),
        alias("-", $.fold_minus),
        alias("*", $.fold_mul),
        alias("/", $.fold_div),
        alias("%", $.fold_mod),
        alias("**", $.fold_power),
        // Boolean / Bitwise
        alias("&", $.fold_and),
        alias("|", $.fold_or),
        alias("^", $.fold_xor),
        // Comparison
        alias("=", $.fold_eq),
        alias("<>", $.fold_ne),
        alias("<", $.fold_lt),
        alias(">", $.fold_gt),
        alias("<=", $.fold_le),
        alias(">=", $.fold_ge),
      ),

    // Primary_Expr : Constant | Enum_Value | Variable_Access | Func_Call | Ref_Value| '(' Expression ')';
    _primary_expression: ($) =>
      choice(
        $.constant,
        $.enum_value,
        $.variable_access,
        $.func_call,
        $.ref_value,
        $.parenthesized_expression,
      ),

    parenthesized_expression: ($) => seq("(", $._expression, ")"),

    boolean_operator: ($) =>
      choice($.or_operator, $.xor_operator, $.and_operator),

    or_operator: ($) =>
      prec.left(
        RK_PREC.boolean_or,
        seq(field("left", $._expression), "OR", field("right", $._expression)),
      ),
    xor_operator: ($) =>
      prec.left(
        RK_PREC.boolean_xor,
        seq(field("left", $._expression), "XOR", field("right", $._expression)),
      ),
    and_operator: ($) =>
      prec.left(
        RK_PREC.boolean_and,
        seq(
          field("left", $._expression),
          choice("&", "AND"),
          field("right", $._expression),
        ),
      ),

    comparison_operator: ($) => choice($.eq_operator, $.ord_operator),

    eq_operator: ($) =>
      prec.left(
        RK_PREC.equality,
        seq(
          field("left", $._expression),
          field("operator", $.eq),
          field("right", $._expression),
        ),
      ),
    ord_operator: ($) =>
      prec.left(
        RK_PREC.comparison,
        seq(
          field("left", $._expression),
          field("operator", $.ord),
          field("right", $._expression),
        ),
      ),

    eq: ($) => prec(RK_PREC.equality, choice("=", "<>")),
    ord: ($) => prec(RK_PREC.comparison, choice("<", ">", "<=", ">=")),

    add_operator: ($) =>
      prec.left(
        RK_PREC.add,
        seq(
          field("left", $._expression),
          field("operator", $.add),
          field("right", $._expression),
        ),
      ),

    add: ($) => prec(RK_PREC.add, choice("+", "-")),

    mult_operator: ($) =>
      prec.left(
        RK_PREC.modulo,
        seq(
          field("left", $._expression),
          field("operator", $.mult),
          field("right", $._expression),
        ),
      ),

    mult: ($) => prec(RK_PREC.modulo, choice("*", "/", "MOD")),

    power_operator: ($) =>
      prec.left(
        RK_PREC.exponentiation,
        seq(field("left", $._expression), "**", field("right", $._expression)),
      ),

    unary_operator: ($) =>
      seq(field("operator", $.unary), field("expr", $._expression)),

    unary: ($) => prec(RK_PREC.unary, choice("-", "+", "NOT")),

    // A constant expression must evaluate to a constant value at compile time
    constant_expr: ($) => $._expression,

    variable_access: ($) =>
      seq(
        field("variable", $.variable),
        field("access", optional($.multibit_part_access)),
      ),

    multibit_part_access: ($) => seq(".", field("path", $.multibit)),

    multibit: ($) => // Bit access + Offset
      seq(
        optional(seq("%", field("access", $.adress_identifier))),
        $.unsigned_int
      ),

    func_call: ($) =>
      seq(
        field("function", $.begin_path_expression),
        optional(field("type_args", $.generic_type_args)),
        "(",
        prec(
          RK_PREC.parameter_list,
          field("params", commaSep($.param_assign)),
        ),
        ")",
      ),

    generic_type_args: ($) =>
      seq(
        "<",
        commaSep1(field("type_arg", $.data_type_access)),
        ">",
      ),

    stmt_list: ($) => prec.left(repeat1(seq($._stmt, optional(";")))),

    // Stmt : Assign_Stmt | Subprog_Ctrl_Stmt | Selection_Stmt | Iteration_Stmt;
    _stmt: ($) =>
      choice(
        // assignments
        $.assign,
        //$.empty_path_expression, // used for completions
        // subprog
        $.func_call,
        // invocations (THIS, SUPER)
        $.begin_path_expression,
        "RETURN",
        // selection
        $.if_stmt,
        $.case_stmt,
        // iteration
        $.for_stmt,
        $.while_stmt,
        $.repeat_stmt,
        "EXIT",
        "CONTINUE",
      ),

    // assignment: $ => seq(
    //    $.variable,
    //    ':=',
    //    $._expression
    //),
    // ref_assign: $ => seq(
    //    ':=',
    //    choice($.ref_name, $.ref_deref, $.ref_value)
    //),
    // assignment_attempt: $ => seq(
    //    field("value", choice($.identifier, $.ref_deref)),
    //    '?=',
    //    field("target", choice($.identifier, $.ref_deref, $.ref_value))
    //),
    assign: ($) =>
      seq(
        field("variable", choice($.variable, $.ERR_assign_func_call)),
        field(
          "target",
          choice(
            $.assignment_attempt,
            $.assignment,
            $.ERR_empty_right_hand_assignment,
            $.ERR_missing_dot_in_assignment,
            $.ERR_missing_equal_in_assignment,
            $.ERR_output_assign_in_assignment,
          ),
        ),
      ),

    assignment: ($) => seq(":=", $._expression),

    assignment_attempt: ($) => seq("?=", $._expression),

    param_assign: ($) => choice($.param_assign_input, $.param_assign_output),

    param_assign_input: ($) =>
      seq(
        optional(seq(field("param", $.identifier), ":=")),
        field("value", $._expression),
      ),

    param_assign_output: ($) =>
      seq(
        field("not", optional("NOT")),
        field("param", $.identifier),
        "=>",
        field("variable", $.variable),
      ),

    if_stmt: ($) =>
      seq(
        "IF",
        field("if_cond", $._expression),
        "THEN",
        field("if_body", optional($.stmt_list)),
        field("else_if", repeat($.else_if_stmt)),
        optional(seq("ELSE", field("else_body", $.stmt_list))),
        "END_IF",
      ),

    else_if_stmt: ($) =>
      seq(
        "ELSIF",
        field("else_if_cond", $._expression),
        "THEN",
        field("else_if_body", $.stmt_list),
      ),

    case_stmt: ($) =>
      seq(
        "CASE",
        field("case_cond", $._expression),
        "OF",
        field("case_selection", repeat($.case_selection)),
        optional(seq("ELSE", field("default", $.stmt_list))),
        "END_CASE",
      ),

    case_selection: ($) =>
      seq(
        field("case_of", $.case_list),
        ":",
        field("case_do", optional($.stmt_list)),
        optional(";"),
      ),

    //Case_List : Case_List_Elem ( ',' Case_List_Elem )*;
    case_list: ($) => commaSep1($.case_list_elem),

    case_list_elem: ($) => choice($.subrange, $.constant_expr),

    for_stmt: ($) =>
      seq(
        "FOR",
        field("control_variable", $.variable),
        field("control_list", $.for_list),
        optional("DO"),
        field("body", optional($.stmt_list)),
        "END_FOR",
      ),

    control_variable: ($) => $.identifier,

    for_list: ($) =>
      seq(
        choice(
          ":=",
          $.ERR_missing_dot_in_for_control,
          $.ERR_missing_equal_in_for_control,
          $.ERR_output_assign_in_for_control,
        ),
        field("initial_value", $._expression),
        "TO",
        field("end_value", $._expression),
        optional(seq("BY", field("step", $._expression))),
      ),

    while_stmt: ($) =>
      seq(
        "WHILE",
        field("while_cond", $._expression),
        "DO",
        field("while_body", optional($.stmt_list)),
        "END_WHILE",
      ),

    repeat_stmt: ($) =>
      seq(
        "REPEAT",
        field("repeat_body", optional($.stmt_list)),
        "UNTIL",
        field("repeat_cond", $._expression),
        "END_REPEAT",
      ),

    // Other

    // Accessing a namespace
    namespace_access: ($) => choice($.identifier, $.scoped_identifier),

    scoped_identifier: ($) =>
      seq(
        field("path", $._path),
        ".",
        field("target", alias($.identifier, $.target)),
      ),

    _path: ($) => choice($.identifier, $.scoped_identifier),

    begin_path_expression: ($) =>
      choice(
        // invocation (THIS / SUPER / SUPER()) with optional deref, optionally followed by a path_expression
        $.invocation,
        // or a plain path expression
        $.path_expression,
      ),

    invocation: ($) => choice($.super_body_invocation, $.any_invocation),

    any_invocation: ($) =>
      seq(
        field("invocation", choice($.this_invocation, $.super_invocation)),
        optional($.any_invocation_kind),
      ),

    any_invocation_kind: ($) =>
      choice(
        alias("^", $.deref_invocation),
        seq(
          alias("^", $.deref_invocation),
          ".",
          field("path", $.path_expression),
        ),
        seq(".", field("path", $.path_expression)),
      ),

    this_invocation: ($) => seq(field("THIS", "THIS")),

    super_invocation: ($) => seq(field("SUPER", "SUPER")),

    super_body_invocation: ($) => seq(field("SUPER", "SUPER"), "()"),

    path_expression: ($) =>
      choice($.var_access, $.field_expression, $.index_expression, $.deref_expression),

    field_expression: ($) =>
      seq(field("path", $.path_expression), ".", field("target", $.var_access)),

    index_expression: ($) =>
      seq($.path_expression, "[", field("index", $.index_value), "]"),

    index_value: ($) => commaSep1($.constant_expr),

    empty_path_expression: ($) => prec(-1, $.path_expression),

    generic_spec: ($) => seq("<", $.generic_params, ">"),

    generic_params: ($) => commaSep1($.generic_rule),

    generic_rule: ($) => seq(
      field("generic_name", $.identifier),
      ":",
      field("generic_type", $.identifier),
      repeat(
        seq(
          "+",
          "INTO",
          "<",
          field("constraint", $.data_type_access),
          ">"
        )
      )
    ),

    IQM: ($) => choice("I", "Q", "M"),

    // Table 73 - 76 - Graphic languages elements

    ladder_diagram: ($) => repeat1($.ld_rung),

    ld_rung: ($) => "todo_lad",

    // same
    fb_diagram: ($) => repeat1($.fbd_network),

    fbd_network: ($) => "todo_fbd",

    // Table 1 - Character sets
    // Table 2 - Identifiers

    _bit_value: ($) => /[?:_01]*/,
    _octal_value: ($) => /[?:_0-7]*/,
    _hex_digit: ($) => /[?:_0-9a-fA-F]/,
    _hex_value: ($) => /[?:_0-9a-fA-F]*/,
    identifier: (_) => /[_\p{XID_Start}][_\p{XID_Continue}]*/,
    adress_identifier: (_) => /[A-Za-z]*/,
  },
});
