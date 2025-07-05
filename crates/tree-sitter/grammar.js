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
 * @file Structured Text grammar for tree-sitter
 * @author CLAUZEL Adrien <clauzeladrien@mail.com>
 * @license AGPL-3.0-only
 */

function commaSep1(rule) {
    return seq(rule, repeat(seq(',', rule)))
}

function commaSep(rule) {
    return optional(commaSep1(rule))
}

/// Create rules for a new type with a spec and an init
function createSpecInit(name, spec, init = null) {
    let result = {
        [`${name}_type_spec`]: $ => spec($)
    };
    if (init) {
        result[`${name}_type_init`] = $ => init($)
    };
    return result;
}

/// Use a list of specs and inits to complete a new rule
function useSpecInit(specs, inits) {
    return $ => seq(
        field("spec", choice(...specs.map(rule => $[`${rule}_type_spec`]))),
        field("init", optional(choice(...inits.map(rule => $[`${rule}_type_init`]))))
    )

}

// Variable declarations
const io_var_decls = $ => [
    $.input_decls,
    $.output_decls,
    $.in_out_decls
]

const func_var_decls = $ => [
    $.external_var_decls,
    $.var_decls
]

const other_var_decls = $ => [
    $.retain_var_decls,
    $.no_retain_var_decls,
    $.loc_partly_var_decl
]

// Precedences from the standard
const PREC = {
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
    boolean_or: 1 // OR
}

// Reserved keywords that cannot be used as identifiers
const RESERVED_NAMES = [
    "PROGRAM", "END_PROGRAM",
    "CONFIGURATION", "END_CONFIGURATION",
    "RESOURCE", "END_RESOURCE",
    "NAMESPACE", "END_NAMESPACE",
    "USING",
    "CLASS", "END_CLASS",
    "INTERFACE", "END_INTERFACE",
    "METHOD", "END_METHOD",
    "FUNCTION", "END_FUNCTION",
    "FUNCTION_BLOCK", "END_FUNCTION_BLOCK",
    "TYPE", "END_TYPE",
    "VAR", "END_VAR",
    "VAR_INPUT",
    "VAR_OUTPUT",
    "VAR_IN_OUT",
    "VAR_TEMP",
    "VAR_EXTERNAL",
    "VAR_GLOBAL",
    "IF", "THEN", "ELSE", "END_IF",
    "CASE", "OF", "END_CASE",
    "FOR", "TO", "BY", "END_FOR",
    "REPEAT", "UNTIL", "END_REPEAT",
    "WHILE", "END_WHILE",
    "DO",
    "EXIT", "RETURN",
    // Types
    "BOOL", "BYTE", "WORD", "DWORD", "LWORD",
    "SINT", "INT", "DINT", "LINT",
    "USINT", "UINT", "UDINT", "ULINT",
    "REAL", "LREAL",
    "CHAR", "WCHAR",
    "STRING", "WSTRING",
    "DATE", "TIME", "DT", "TOD", "LDATE", "LTIME", "LDT", "LTOD"
];

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check
module.exports = grammar({
    name: "rk",

    extras: $ => [
        /\s/, // Whitespace
        $.comment,
        token(choice('\t', '\r', '\n')),
    ],

    reserved: {
        global: $ => RESERVED_NAMES,
    },

    supertypes: $ => [
        $._func_variables,
        $._fb_variables,
        $._class_variables,

        $._input_var_kind,
        $._fb_input_var_kind,
        $._output_var_kind,
        $._fb_output_var_kind,
        $._temp_var_kind,
        $._in_out_var_kind,
        $._external_var_kind,
        $._global_var_kind,

        $._data_type_access,
        $._elem_type_name,

        $._expression,
        $._primary_expression,

        // operators
        $.unary,
        $.add,
        $.mult,
        $.eq,
        $.ord,

        // access and size
        $.IQM,
        $.XBWDL
    ],

    conflicts: $ => [
        // local variable declarations
        [$.var_decls, $.loc_var_decls],
        [$.var_decls, $.loc_var_decls, $.loc_partly_var_decl],
        [$.var_decls, $.loc_partly_var_decl],

        [$.retain_var_decls, $.loc_var_decls, $.loc_partly_var_decl],
        [$.retain_var_decls, $.loc_partly_var_decl],
        [$.no_retain_var_decls, $.loc_var_decls, $.loc_partly_var_decl],
        [$.no_retain_var_decls, $.loc_partly_var_decl],

        // conflicts in type declarations
        [$.numeric_literal, $.enum_value_spec],
        [$.subrange_type_spec, $.numeric_type_name],

        [$.symbolic_variable, $.multi_elem_var],

        [$.fq_name],
        [$.global_ref_deref, $.var_access],
    ],

    word: $ => $.identifier,

    rules: {
        // Source file declaration
        source_file: $ => repeat(
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
            )
        ),

        // Table 3 - Comments 

        comment: $ => choice(
            seq('//', /[^\r\n]*/),
            seq('(*', repeat(choice(/[^*]/, /\*[^)]/)), '*)'),
            seq('/*', repeat(choice(/[^*]/, /\*[^/]/)), '*/')
        ),

        // Table 4 - Pragma 

        pragma: $ => seq('{', repeat(choice(/[^}]/, /}[^}]/)), '}'),

        // Table 5 - Numeric literal

        constant: $ => choice(
            $.numeric_literal,
            $.char_literal,
            $.time_literal,
            $.bool_literal
        ),

        numeric_literal: $ => choice(
            $.int_literal,
            $.real_literal
        ),

        int_literal: $ => seq(
            optional(seq(field("kind", $.int_kind), "#")),
            field("int", choice(
                $.signed_int,
                $.binary_int,
                $.octal_int,
                $.hex_int
            ))
        ),

        int_kind: $ => choice(
            $.multibits_type_name,
            $.int_type_name
        ),

        unsigned_int: $ => /[0-9]+(_[0-9]+)*/,

        signed_int: $ => seq(
            optional(choice('+', '-')),
            $.unsigned_int
        ),

        binary_int: $ => seq(
            '2#',
            field("value", $._bit, repeat1(seq(optional('_'), $._bit)))
        ),

        octal_int: $ => seq(
            '8#',
            field("value", seq($._octal_digit, repeat1(seq(optional('_'), $._octal_digit)))),
        ),

        hex_int: $ => seq(
            '16#',
            field("value", seq($._hex_digit, repeat1(seq(optional('_'), $._hex_digit))))
        ),

        real_literal: $ => choice(
            $.real,
            $.l_real
        ),

        real: $ => seq(
            "REAL",
            field("value", $.real_value)
        ),

        l_real: $ => seq(
            "LREAL",
            field("value", $.real_value)
        ),

        real_value: $ => /[0-9]+(_[0-9]+)*/,

        bool_literal: $ => seq(
            field("type", optional('BOOL#')),
            // fixme! Add support for numbers in auto-lsp
            field("value", choice('TRUE', 'FALSE'))
        ),

        // Table 6 - Character String literals
        // Table 7 - Two-character combinations in character strings

        char_literal: $ => seq(
            optional('STRING#'),
            field("char", $.char_str)
        ),

        char_str: $ => choice(
            $._s_byte_char_str,
            $._d_byte_char_str
        ),

        _s_byte_char_str: $ => seq(
            "'",
            repeat($._s_byte_char_value),
            "'"
        ),

        _d_byte_char_str: $ => seq(
            '"',
            repeat($._d_byte_char_value),
            '"'
        ),

        _s_byte_char_value: $ => choice(
            $._common_char_value,
            '$\'',
            '"',
            seq('$', $._hex_digit, $._hex_digit)
        ),

        _d_byte_char_value: $ => choice(
            $._common_char_value,
            "'",
            '$"',
            seq('$', $._hex_digit, $._hex_digit, $._hex_digit, $._hex_digit)
        ),

        _common_char_value: $ => choice(
            ' ',
            '!',
            '#',
            '%',
            '&',
            ...Array.from({ length: 11 }, (_, i) => String.fromCharCode(40 + i)), // '('..'/'
            ...Array.from({ length: 10 }, (_, i) => String.fromCharCode(48 + i)), // '0'..'9'
            ...Array.from({ length: 7 }, (_, i) => String.fromCharCode(58 + i)),  // ':'..'@'
            ...Array.from({ length: 26 }, (_, i) => String.fromCharCode(65 + i)), // 'A'..'Z'
            ...Array.from({ length: 6 }, (_, i) => String.fromCharCode(91 + i)),  // '['..'`'
            ...Array.from({ length: 26 }, (_, i) => String.fromCharCode(97 + i)), // 'a'..'z'
            ...Array.from({ length: 4 }, (_, i) => String.fromCharCode(123 + i)), // '{'..'~'
            '$$',
            '$L',
            '$N',
            '$P',
            '$R',
            '$T'
        ),

        // Table 8 - Duration literals
        // Table 9 – Date and time of day literals 

        time_literal: $ => choice(
            $.duration,
            $.time_of_day,
            $.date,
            $.date_and_time
        ),

        duration: $ => choice(
            $.time,
            $.ltime
        ),

        time: $ => seq(
            choice('T', 'TIME'),
            '#',
            field("sign", optional(choice('+', '-'))),
            field("value", $.time_value)
        ),

        ltime: $ => seq(
            choice('LT', 'LTIME'),
            '#',
            field("sign", optional(choice('+', '-'))),
            field("value", $.time_value)
        ),

        time_value: $ => /([0-9._]+(d|h|ms|ns|m|s))+/,

        fix_point: $ => seq(
            field("real", $.unsigned_int),
            '.',
            field("frac", $.unsigned_int)
        ),

        time_of_day: $ => choice(
            $.tod,
            $.ltod
        ),

        tod: $ => seq(
            choice('TOD', 'TIME_OF_DAY'),
            '#',
            field("value", $.daytime)
        ),

        ltod: $ => seq(
            choice('LTOD', 'LTIME_OF_DAY'),
            '#',
            field("value", $.daytime)
        ),

        daytime: $ => /[0-9a-zA-Z_.:]+/,

        date: $ => choice(
            $.short_date,
            $.long_date
        ),

        short_date: $ => seq(
            choice('D', 'DATE'),
            '#',
            field("value", $.date_literal)
        ),

        long_date: $ => seq(
            choice('LD', 'LDATE'),
            '#',
            field("value", $.date_literal)
        ),

        date_literal: $ => /[0-9a-zA-Z_.:-]+/,

        date_and_time: $ => choice(
            $.short_date_and_time,
            $.long_date_and_time
        ),

        short_date_and_time: $ => seq(
            choice('DT', 'DATE_AND_TIME'),
            '#',
            field("value", $.date_and_daytime)
        ),

        long_date_and_time: $ => seq(
            choice('LDT', 'LDATE_AND_TIME'),
            '#',
            field("value", $.date_and_daytime)
        ),

        date_and_daytime: $ => /[0-9dhmsDHMS_.]+/,

        // Table 10 - Elementary data types

        _data_type_access: $ => choice(
            $.fq_name,
            $._elem_type_name,
        ),

        _elem_type_name: $ => choice(
            $.numeric_type_name,
            $.bit_str_type_name,
            $.date_type_name,
            $.time_type_name,
            $.tod_type_name,
            $.dt_type_name
        ),

        numeric_type_name: $ => choice(
            $.int_type_name,
            $.real_type_name
        ),

        int_type_name: $ => choice(
            $.sign_int_type_name,
            $.unsign_int_type_name
        ),

        sign_int_type_name: $ => choice(
            alias('SINT', $.sint_name),
            alias('INT', $.int_name),
            alias('DINT', $.dint_name),
            alias('LINT', $.lint_name)
        ),

        unsign_int_type_name: $ => choice(
            alias('USINT', $.usint_name),
            alias('UINT', $.uint_name),
            alias('UDINT', $.udint_name),
            alias('ULINT', $.ulint_name)
        ),

        real_type_name: $ => choice(
            alias('REAL', $.real_name),
            alias('LREAL', $.lreal_name)
        ),

        string_type_name: $ => choice(
            seq('STRING', optional(seq('[', $.unsigned_int, ']'))),
            seq('WSTRING', optional(seq('[', $.unsigned_int, ']'))),
            'CHAR',
            'WCHAR'
        ),

        time_type_name: $ => choice(
            alias('TIME', $.time_name),
            alias('LTIME', $.ltime_name)
        ),

        date_type_name: $ => choice(
            alias('DATE', $.date_name),
            alias('LDATE', $.ldate_name),
        ),

        tod_type_name: $ => choice(
            alias(choice('TOD', 'TIME_OF_DAY'), $.tod_name),
            alias(choice('LTOD', 'LTIME_OF_DAY'), $.ltod_name),
        ),

        dt_type_name: $ => choice(
            alias(choice('DT', 'DATE_AND_TIME'), $.dt_name),
            alias('LDT', $.ldt_name)
        ),

        bit_str_type_name: $ => choice(
            alias('BOOL', $.bool_name),
            $.multibits_type_name
        ),

        multibits_type_name: $ => choice(
            alias('BYTE', $.byte_name),
            alias('WORD', $.word_name),
            alias('DWORD', $.dword_name),
            alias('LWORD', $.lword_name)
        ),

        // Table 11 - Declaration of user-defined data types and initialization

        data_type_decl: $ => seq(
            'TYPE',
            repeat(seq($.type_decl, optional(';'))),
            'END_TYPE'
        ),


        // Type_Decl : Simple_Type_Decl | Subrange_Type_Decl | Enum_Type_Decl | Array_Type_Decl | Struct_Type_Decl |
        //  Str_Type_Decl | Ref_Type_Decl;
        type_decl: $ =>
            seq(
                field("name", $.identifier),
                useSpecInit([
                    "simple",
                    "subrange",
                    "enum",
                    "array",
                    "struct",
                    "str",
                    "ref"
                ], [
                    "simple",
                    //"subrange", handled by simple
                    //"enum", handled by simple
                    "array",
                    "struct",
                    // "str", handled by simple
                    // "ref"
                ])($),
            ),

        ...createSpecInit("simple",
            $ => seq(":", $._elem_type_name),
            $ => seq(':=', $.constant_expr)
        ),

        ...createSpecInit("subrange",
            $ => seq(":", field("type", $.int_type_name), '(', field("range", $.subrange), ')'),
            $ => $.simple_type_init
        ),

        subrange: $ => seq(
            field("lower", $.constant_expr),
            '..',
            field("upper", $.constant_expr)
        ),

        // Enum_Type_Decl : Enum_Type_Name ':' ( ( Elem_Type_Name ? Named_Spec_Init ) | Enum_Spec_Init ); 
        // Enum_Spec_Init : ( ( '(' Identifier ( ',' Identifier )* ')' ) | Enum_Type_Access ) ( ':=' Enum_Value )?; 
        ...createSpecInit("enum",
            $ => seq(":",
                choice(
                    seq(optional($._elem_type_name), $.named_spec),
                    $.enum_spec,
                )
            ),
            $ => seq(':=', $.enum_value)
        ),

        enum_spec: $ => prec.left(seq(
            choice(
                seq('(', commaSep($.identifier), ')'),
                $.fq_name
            ),
        )),
        // Named_Spec_Init : '(' Enum_Value_Spec ( ',' Enum_Value_Spec )* ')' ( ':=' Enum_Value )?; 
        named_spec: $ => prec.left(seq('(', commaSep1($.enum_value_spec), ')')),

        ...createSpecInit("array",
            $ => seq(
                ":",
                'ARRAY', '[', field("ranges", commaSep1($.subrange)), ']',
                'OF',
                field("type", $._data_type_access)
            ),
            $ => seq(':=', '[', commaSep($.array_elem_init), ']')
        ),

        ...createSpecInit("struct",
            $ => seq(
                ":",
                'STRUCT',
                optional('OVERLAP'),
                repeat1(seq($.struct_elem_decl, ';')),
                'END_STRUCT'
            ),
            $ => seq(':=', '(', commaSep($.struct_elem_init), ')')
        ),

        ...createSpecInit("str",
            $ => seq(":", choice(
                $.s_byte_str_spec,
                $.d_byte_str_spec,
                alias("CHAR", $.s_char),
                alias("WCHAR", $.d_char),
            )),
            $ => seq(':=', $.char_str)
        ),

        s_byte_str_spec: $ => seq(
            'STRING',
            optional(seq('[', field("size", $.unsigned_int), ']'))
        ),

        d_byte_str_spec: $ => seq(
            'WSTRING',
            optional(seq('[', field("size", $.unsigned_int), ']'))
        ),

        enum_value_spec: $ => prec.right(seq(
            $.identifier,
            optional(seq(
                ':=',
                choice($.int_literal, $._expression)
            ))
        )),

        enum_value: $ => seq(
            "#",
            $.identifier
        ),

        // Array_Elem_Init : Array_Elem_Init_Value | Unsigned_Int '(' Array_Elem_Init_Value ? ')'; 
        array_elem_init: $ => choice(
            $.array_elem_init_value,
            seq($.unsigned_int, '(', optional($.array_elem_init_value), ')')
        ),

        array_elem_init_value: $ => choice(
            $.constant_expr,
            $.struct_elem_init,
            $.array_type_init
        ),

        struct_decl: $ => seq(
            'STRUCT',
            optional('OVERLAP'),
            repeat1(seq($.struct_elem_decl, ';')),
            'END_STRUCT'
        ),

        // Struct_Elem_Decl : Struct_Elem_Name ( Located_At Multibit_Part_Access ? )? ':'
        // ( Simple_Spec_Init | Subrange_Spec_Init | Enum_Spec_Init | Array_Spec_Init | Struct_Spec_Init );
        struct_elem_decl: $ => seq(
            field("name", $.identifier),
            optional(seq(
                $.located_at,
                optional($.multibit_part_access)
            )),
            useSpecInit([
                "simple",
                "subrange",
                "enum",
                "array",
                "struct"
            ], [
                "simple",
                // "subrange", handled by simple
                "array",
                "struct",
                // "enum" handled by simple
            ])($),
        ),

        struct_init: $ => seq('(', commaSep($.struct_elem_init), ')'),

        struct_elem_init: $ => seq(
            field("name", $.identifier),
            ':=',
            choice($.constant_expr, $.array_type_init, $.struct_init)
        ),

        // Table 16 - Directly represented variables 

        direct_variable: $ => prec.left(seq(
            '%',
            field("kind", $.IQM),
            field("size", optional($.XBWDL)),
            field("offset", seq($.unsigned_int,
                repeat(seq('.', $.unsigned_int))))
        )),

        // Table 12 - Reference operations 

        ...createSpecInit("ref",
            $ => seq(":",
                'REF_TO',
                $._data_type_access
            ),
            $ => seq(':=', $.ref_value)
        ),

        ref_type_decl: $ => seq(
            field("name", $.identifier),
            ':',
            $.ref_spec_init
        ),

        ref_spec_init: $ => prec.left(seq(
            $.ref_spec,
            optional(seq(':=', $.ref_value))
        )),

        ref_spec: $ => seq(
            'REF_TO',
            $._data_type_access
        ),

        ref_value: $ => choice(
            $.ref_addr,
            alias('NULL', $.null)
        ),

        ref_addr: $ => seq(
            'REF',
            '(',
            choice($.symbolic_variable, $.instance_name),
            ')'
        ),

        ref_assign: $ => seq(
            ':=',
            choice($.ref_deref)
        ),

        ref_deref: $ => prec(PREC.dereference, seq(
            $.identifier,
            '^'
        )),

        // Table 13 - Declaration of variables/Table 14 – Initialization of variables 

        variable: $ => choice($.direct_variable, $.symbolic_variable),

        symbolic_variable: $ => seq(
            field("this", alias(optional(seq('THIS', '.')), $.this)),
            choice(
                $.var_access,
                $.multi_elem_var,
            )),

        // Var_Access : Variable_Name | Ref_Deref; 
        var_access: $ => choice($.identifier, $.ref_deref),

        multi_elem_var: $ => seq(
            field("access", $.var_access),
            prec.left(repeat1(choice($.subscript_list, $.struct_variable)))
        ),

        subscript_list: $ => seq('[', commaSep($._expression), ']'),

        struct_variable: $ => seq(
            '.',
            $.var_access
        ),

        input_decls: $ => seq(
            'VAR_INPUT',
            field("retain", optional(choice('RETAIN', 'NON_RETAIN'))),
            repeat(seq($.input_var, optional(';'))),
            'END_VAR'
        ),

        input_var: $ => seq(
            field("variables", $.variable_list),
            field("type", $._input_var_kind)
        ),

        _input_var_kind: $ => choice($.var_decl_init, $.edge_decl, $.array_conformand),

        edge_decl: $ => seq(
            field("variables", $.variable_list),
            ':',
            'BOOL',
            field("edge", optional(choice('R_EDGE', 'F_EDGE')))
        ),

        // : Variable_List ':' ( Simple_Spec_Init | Str_Var_Decl | Ref_Spec_Init )
        //| Array_Var_Decl_Init | Struct_Var_Decl_Init | FB_Decl_Init | Interface_Spec_Init; 

        // INPUTS
        // OUTPUTS
        // VARS
        // RETAIN
        var_decl_init: $ => useSpecInit([
            "simple",
            "str",
            "array",
            "struct"
        ], [
            "simple",
            // "str", handled by simple
            "array",
            "struct"

        ]
        )($),

        // ( Simple_Spec | Str_Var_Decl | Array_Var_Decl | Struct_Var_Decl )

        // INOUT
        // TEMP
        var_decl: $ => useSpecInit([
            "simple",
            "str",
            "array",
            "struct"
        ], [
            "simple",
            // "str", handled by simple
            "array",
            "struct"
        ]
        )($),

        variable_list: $ => commaSep1($.identifier),

        array_conformand: $ => seq(
            ":",
            'ARRAY',
            '[',
            commaSep1('*'),
            ']',
            'OF',
            $._data_type_access
        ),

        fb_decl_no_init: $ => seq(
            commaSep1($.fb_name),
            ':',
            $.fq_name
        ),

        fb_decl_init: $ => seq(':=', $.struct_init),

        fb_name: $ => $.identifier,

        output_decls: $ => seq(
            'VAR_OUTPUT',
            optional(choice('RETAIN', 'NON_RETAIN')),
            repeat(seq($.output_var, optional(';'))),
            'END_VAR',
            optional(';')
        ),

        output_var: $ => seq(
            field("variables", $.variable_list),
            field("type", $._output_var_kind)
        ),

        _output_var_kind: $ => choice($.var_decl_init, $.array_conformand),

        in_out_decls: $ => seq(
            'VAR_IN_OUT',
            repeat(seq($.in_out_var, optional(';'))),
            'END_VAR',
            optional(';')
        ),

        in_out_var: $ => seq(
            field("variables", $.variable_list),
            field("type", $._in_out_var_kind)
        ),

        _in_out_var_kind: $ => choice($.var_decl, $.array_conformand, $.fb_decl_no_init),

        var_decls: $ => seq(
            'VAR',
            field("constant", optional('CONSTANT')),
            field("access", optional($.access_spec)),
            repeat(seq($.var_decl_init_list, optional(';'))),
            'END_VAR',
            optional(';')
        ),

        retain_var_decls: $ => seq(
            'VAR',
            field("retain", 'RETAIN'),
            field("access", optional($.access_spec)),
            repeat(seq($.var_decl_init_list, optional(';'))),
            'END_VAR',
            optional(';')
        ),

        var_decl_init_list: $ => seq(
            field("variables", $.variable_list),
            field("type", $.var_decl)
        ),

        loc_var_decls: $ => seq(
            'VAR',
            field("constant_or_retain", optional(choice('CONSTANT', 'RETAIN', 'NON_RETAIN'))),
            repeat(seq($.loc_var_decl, optional(';'))),
            'END_VAR',
            optional(';')
        ),

        loc_var_decl: $ => seq(
            optional(field("variable_name", $.identifier)),
            $.located_at,
            ':',
            $.loc_var_spec_init
        ),

        temp_var_decls: $ => seq(
            'VAR_TEMP',
            repeat(seq($.temp_var, optional(';'))),
            'END_VAR',
            optional(';')
        ),

        temp_var: $ => seq(
            field("variables", $.variable_list),
            field("type", $._temp_var_kind)
        ),

        _temp_var_kind: $ => choice($.var_decl, $.ref_spec),

        external_var_decls: $ => seq(
            'VAR_EXTERNAL',
            field("constant", optional('CONSTANT')),
            repeat(seq($.external_decl, optional(';'))),
            'END_VAR',
            optional(';')
        ),

        external_decl: $ => seq(
            field("name", $.identifier),
            field("type", $._external_var_kind)
        ),

        _external_var_kind: $ => choice($.var_decl, $.array_conformand),

        global_var_decls: $ => seq(
            'VAR_GLOBAL',
            field("constant_or_retain", optional(choice('CONSTANT', 'RETAIN'))),
            repeat(seq($.global_var_decl, optional(';'))),
            'END_VAR',
            optional(';')
        ),

        global_var_decl: $ => seq(
            field("spec", $.global_var_spec),
            ':',
            field("type", $._global_var_kind)
        ),

        _global_var_kind: $ => choice($.loc_var_spec_init, $.fq_name),

        global_var_spec: $ => choice(
            seq(commaSep1($.identifier)),
            seq(
                $.identifier,
                $.located_at
            )
        ),

        // Loc_Var_Spec_Init : Simple_Spec_Init | Array_Spec_Init | Struct_Spec_Init | S_Byte_Str_Spec | D_Byte_Str_Spec; 
        loc_var_spec_init: $ => useSpecInit([
            "simple",
            "array",
            "struct",
            "str"
        ], [
            "simple",
            "array",
            "struct"
            // "str" handled by simple
        ])($),

        located_at: $ => seq(
            'AT',
            $.direct_variable
        ),

        loc_partly_var_decl: $ => seq(
            'VAR',
            field("retain", optional(choice('RETAIN', 'NON_RETAIN'))),
            repeat(seq($.loc_partly_var, optional(';'))),
            'END_VAR',
            optional(';')
        ),

        loc_partly_var: $ => seq(
            field("variable_name", $.identifier),
            'AT',
            '%',
            $.IQM,
            '*',
            ':',
            $.var_spec,
        ),

        var_spec: $ => choice(
            $.array_type_spec,
            $.fq_name,
            seq(choice('STRING', 'WSTRING'), optional(seq('[', $.unsigned_int, ']'))),
        ),

        // Table 19 - Function declaration

        func_decl: $ => seq(
            'FUNCTION',
            field("name", $.identifier),
            field("access", optional(seq(':', $._data_type_access))),
            field("directives", repeat($.using_directive)),
            field("variables", repeat($._func_variables)),
            field("body", optional($.func_body)),
            'END_FUNCTION'
        ),

        _func_variables: $ => choice(
            ...io_var_decls($),
            ...func_var_decls($),
            $.temp_var_decls,
        ),

        func_body: $ => choice(
            $.ladder_diagram,
            $.fb_diagram,
            $.stmt_list,
        ),

        // Table 40 – Function block type declaration
        // Table 41 - Function block instance declaration

        fb_decl: $ => seq(
            'FUNCTION_BLOCK',
            field("qualifier", optional(choice('FINAL', 'ABSTRACT'))),
            field("name", $.identifier),
            field("directives", repeat($.using_directive)),
            optional(seq("EXTENDS", field("extends", $.fq_name))),
            optional(seq("IMPLEMENTS", field("implements", $.interface_name_list))),
            field("variables", repeat($._fb_variables)),
            field("method", repeat($.method_decl)),
            field("body", optional($.fb_body)),
            "END_FUNCTION_BLOCK"
        ),

        _fb_variables: $ => choice(
            $.fb_input_decls,
            $.fb_output_decls,
            $.in_out_decls,
            $.temp_var_decls,
            ...func_var_decls($),
            ...other_var_decls($)
        ),

        fb_input_decls: $ => seq(
            'VAR_INPUT',
            optional(choice('RETAIN', 'NON_RETAIN')),
            repeat(seq($.fb_input_var, optional(';'))),
            'END_VAR',
            optional(';')
        ),

        fb_input_var: $ => seq(
            field("variables", $.variable_list),
            field("type", $._fb_input_var_kind)
        ),

        _fb_input_var_kind: $ => choice(
            $.var_decl_init,
            $.edge_decl,
            $.array_conformand
        ),

        fb_output_decls: $ => seq(
            'VAR_OUTPUT',
            optional(choice('RETAIN', 'NON_RETAIN')),
            repeat(seq($.fb_output_var, optional(';'))),
            'END_VAR',
            optional(';')
        ),

        fb_output_var: $ => seq(
            field("variables", $.variable_list),
            field("type", $._fb_output_var_kind)
        ),

        _fb_output_var_kind: $ => choice(
            $.var_decl_init,
            $.array_conformand
        ),

        no_retain_var_decls: $ => seq(
            'VAR', 'NON_RETAIN',
            field("spec", optional($.access_spec)),
            repeat(seq($.var_decl_init_list, optional(';'))),
            'END_VAR',
            optional(';')
        ),

        fb_body: $ => choice(
            $.SFC,
            $.ladder_diagram,
            $.fb_diagram,
            $.stmt_list,
        ),

        method_decl: $ => seq(
            'METHOD',
            $.access_spec,
            optional(choice('FINAL', 'ABSTRACT')),
            optional('OVERRIDE'),
            $.identifier,
            optional(seq(':', $._data_type_access)),
            repeat(choice(...io_var_decls($), ...func_var_decls($), $.temp_var_decls)),
            field("body", optional($.func_body)),
            'END_METHOD'
        ),

        // Table 48 - Class
        // Table 50 Textual call of methods – Formal and non-formal parameter list 

        class_decl: $ => seq(
            'CLASS',
            field("qualifier", optional(choice('FINAL', 'ABSTRACT'))),
            field("name", $.class_type_name),
            field("directives", repeat($.using_directive)),
            optional(seq("EXTENDS", field("extends", $.fq_name))),
            optional(seq("IMPLEMENTS", field("implements", $.interface_name_list))),
            field("declarations", repeat($._class_variables)),
            field("methods", repeat($.method_decl)),
            'END_CLASS'
        ),

        _class_variables: $ => choice(
            ...func_var_decls($),
            ...other_var_decls($)
        ),

        class_type_name: $ => $.identifier,

        instance_name: $ => prec(PREC.dereference, seq(
            field("name", $.identifier),
            repeat1('^')
        )),

        interface_decl: $ => seq(
            'INTERFACE',
            field("name", $.identifier),
            field("directives", repeat($.using_directive)),
            optional(seq('EXTENDS', field("extends", $.interface_name_list))),
            field("prototype", repeat($.method_prototype)),
            'END_INTERFACE'
        ),

        method_prototype: $ => seq(
            'METHOD',
            field("name", $.identifier),
            optional(seq(':', field("data_type", $._data_type_access))),
            field("variables", repeat(choice(...io_var_decls($)))),
            'END_METHOD'
        ),

        interface_spec_init: $ => seq(':=', $.interface_value),

        interface_value: $ => choice(
            $.symbolic_variable,
            $.instance_name,
            'NULL'
        ),

        interface_name_list: $ => commaSep1($.fq_name),

        interface_name: $ => $.identifier,

        access_spec: $ => choice('PUBLIC', 'PROTECTED', 'PRIVATE', 'INTERNAL'),

        // Table 47 - Program declaration 

        prog_decl: $ => seq(
            'PROGRAM',
            field("name", $.identifier),
            field("declarations", repeat(choice(
                ...io_var_decls($),
                ...func_var_decls($),
                $.temp_var_decls,
                ...other_var_decls($),
                $.loc_var_decls,
                $.prog_access_decl
            ))),
            field("body", optional($.fb_body)),
            'END_PROGRAM'
        ),

        prog_type_access: $ => $.fq_name,

        prog_access_decls: $ => seq(
            'ref_deref',
            repeat(seq($.prog_access_decl, ';')),
            $.identifier
        ),

        prog_access_decl: $ => seq(
            $.access_name,
            ':',
            $.symbolic_variable,
            optional($.multibit_part_access),
            ':',
            $._data_type_access,
            $.access_direction
        ),

        // Table 54 - 61 - Sequential Function Chart (SFC) 

        SFC: $ => repeat1($.SFC_network),

        SFC_network: $ => seq(
            $.initial_step,
            repeat(choice($.step, $.transition, $.action))
        ),

        initial_step: $ => seq(
            'INITIAL_STEP',
            $.step_name,
            ':',
            repeat(seq($.action_association, ';')),
            'END_STEP'
        ),

        step: $ => seq(
            'STEP',
            $.step_name,
            ':',
            repeat(seq($.action_association, ';')),
            'END_STEP'
        ),

        step_name: $ => $.identifier,

        action_association: $ => seq(
            $.action_name,
            '(',
            optional($.action_qualifier),
            repeat(seq(field("variable_name", $.identifier), ';')),
            ')'
        ),

        action_name: $ => $.identifier,

        action_qualifier: $ => choice(
            'N',
            'R',
            'S',
            'P',
            seq(
                choice('L', 'D', 'SD', 'DS', 'SL'),
                ',',
                $.action_time
            )
        ),

        action_time: $ => choice(
            $.duration,
            field("variable_name", $.identifier)
        ),

        transition: $ => seq(
            'TRANSITION',
            optional($.transition_name),
            ':',
            optional(seq('(', 'PRIORITY', ':=', $.unsigned_int, ')')),
            'FROM',
            $.steps,
            'TO',
            $.steps,
            $.transition_cond,
            'END_TRANSITION'
        ),

        transition_name: $ => $.identifier,

        steps: $ => choice(
            $.step_name,
            seq('(', $.step_name, repeat1(seq(',', $.step_name)), ')')
        ),

        transition_cond: $ => choice(
            seq(':=', $._expression, ';'),
            seq(':', choice($.fbd_network, $.ld_rung)
            )
        ),

        action: $ => seq(
            'ACTION',
            $.action_name,
            ':',
            $.fb_body,
            'END_ACTION'
        ),

        // Table 62 - Configuration and resource declaration 

        config_name: $ => $.identifier,

        resource_type_name: $ => $.identifier,

        config_decl: $ => seq(
            'CONFIGURATION',
            field("name", $.config_name),
            field("global_variables", optional($.global_var_decls)),
            field("ressources", choice($.single_resource_decl, repeat1($.resource_decl))),
            'END_CONFIGURATION'
        ),

        resource_decl: $ => seq(
            'RESOURCE',
            field("name", $.identifier),
            'ON',
            field("resource_type_name", $.resource_type_name),
            field("global_variables", optional($.global_var_decls)),
            field("ressource", $.single_resource_decl),
            'END_RESOURCE'
        ),

        single_resource_decl: $ => seq(
            repeat(seq($.task_config, ';')),
            repeat1(seq($.prog_config, ';'))
        ),

        access_decls: $ => seq(
            'ref_deref',
            repeat(seq($.access_decl, optional(';'))),
            'END_VAR',
            optional(';')
        ),

        access_decl: $ => seq(
            $.access_name,
            ':',
            $.access_path,
            ':',
            optional(seq($._data_type_access, $.access_direction)),
        ),

        access_path: $ => choice(
            seq(optional(seq($.identifier, '.')), $.direct_variable),
            seq(
                optional(seq($.identifier, '.')),
                repeat(seq(choice($.instance_name), '.')),
                $.symbolic_variable
            )
        ),

        global_ref_deref: $ => prec.left(seq(
            seq(field("ressource", $.identifier), '.'),
            field("name", $.identifier),
            optional(seq('.', field("struct", $.identifier)))
        )),

        access_name: $ => $.identifier,

        prog_output_access: $ => seq(
            field("prog_name", $.identifier),
            '.',
            $.symbolic_variable
        ),

        access_direction: $ => choice('READ_WRITE', 'READ_ONLY'),

        task_config: $ => seq(
            'TASK',
            field("name", $.identifier),
            field("init", $.task_init)
        ),

        task_init: $ => seq(
            '(',
            optional(seq('SINGLE', ':=', field("single", $.data_source), ',')),
            optional(seq('INTERVAL', ':=', field("interval", $.data_source), ',')),
            'PRIORITY', ':=', field("priority", $.unsigned_int),
            ')'
        ),

        data_source: $ => choice(
            $.constant,
            $.global_ref_deref,
            $.prog_output_access,
            $.direct_variable
        ),

        prog_config: $ => seq(
            'PROGRAM',
            field("retain", optional(choice('RETAIN', 'NON_RETAIN'))),
            field("name", $.identifier),
            field("task", optional(seq('WITH', $.identifier))),
            ':',
            field("access", $.prog_type_access),
            field("configuration_elements", optional(seq('(', $.prog_conf_elems, ')')))
        ),

        prog_conf_elems: $ => commaSep1($.prog_conf_elem),

        prog_conf_elem: $ => choice(
            $.fb_task,
            $.prog_cnxn
        ),

        fb_task: $ => seq(
            $.instance_name,
            'WITH',
            field("task", $.identifier)
        ),

        prog_cnxn: $ => choice(
            seq($.symbolic_variable, ':=', $.prog_data_source),
            seq($.symbolic_variable, '=>', $.data_sink),
        ),

        prog_data_source: $ => choice(
            $.constant,
            $.enum_value,
            $.global_ref_deref,
            $.direct_variable
        ),

        data_sink: $ => choice(
            $.global_ref_deref,
            $.direct_variable
        ),

        config_init: $ => seq(
            'VAR_CONFIG',
            repeat(seq($.config_inst_init, optional(';'))),
            'END_VAR',
            optional(';')
        ),

        config_inst_init: $ => seq(
            field("resource", $.identifier),
            '.',
            field("prog", $.identifier),
            '.',
            repeat(seq(choice($.instance_name), '.')),
            choice(
                seq($.identifier, optional($.located_at), ':', $.loc_var_spec_init),
                seq(
                    $.instance_name,
                    ':',
                    $.fq_name,
                    ':=',
                    $.struct_init
                )
            )
        ),

        // Table 64 - Namespace 

        namespace_decl: $ => seq(
            'NAMESPACE',
            field("internal", optional('INTERNAL')),
            field("name", $.namespace_h_name),
            field("directives", repeat($.using_directive)),
            field("elements", optional($.namespace_elements)),
            'END_NAMESPACE'
        ),

        namespace_elements: $ => repeat1(
            choice(
                $.data_type_decl,
                $.func_decl,
                $.fb_decl,
                $.class_decl,
                $.interface_decl,
                $.namespace_decl
            )
        ),

        namespace_h_name: $ => seq(
            $.identifier,
            repeat(seq('.', $.identifier))
        ),

        using_directive: $ => seq(
            'USING',
            commaSep1($.namespace_h_name),
            optional(';')
        ),

        pou_decl: $ => seq(
            repeat($.using_directive),
            repeat1(
                choice(
                    $.global_var_decls,
                    $.data_type_decl,
                    $.access_decls,
                    $.func_decl,
                    $.fb_decl,
                    $.class_decl,
                    $.interface_decl,
                    $.namespace_decl
                )
            )
        ),

        // Table 71 - 72 - Language Structured Text (ST) 

        _expression: $ => choice(
            $.boolean_operator,
            $.comparison_operator,
            $.add_operator,
            $.mult_operator,
            $.power_operator,
            $.unary_operator,
            $._primary_expression
        ),

        _primary_expression: $ => prec.left(choice(
            $.constant,
            $.fq_name,
            $.enum_value,
            $.variable_access,
            $.func_call,
            $.ref_value,
            $.parenthesized_expression
        )),

        parenthesized_expression: $ => seq('(', $._expression, ')'),

        boolean_operator: $ => choice(
            $.or_operator,
            $.xor_operator,
            $.and_operator
        ),

        or_operator: $ => prec.left(PREC.boolean_or, seq(field("left", $._expression), 'OR', field("right", $._expression))),
        xor_operator: $ => prec.left(PREC.boolean_xor, seq(field("left", $._expression), 'XOR', field("right", $._expression))),
        and_operator: $ => prec.left(PREC.boolean_and, seq(field("left", $._expression), choice('&', 'AND'), field("right", $._expression))),

        comparison_operator: $ => choice(
            $.eq_operator,
            $.ord_operator
        ),

        eq_operator: $ => prec.left(PREC.equality, seq(field("left", $._expression), field("operator", $.eq), field("right", $._expression))),
        ord_operator: $ => prec.left(PREC.comparison, seq(field("left", $._expression), field("operator", $.ord), field("right", $._expression))),

        eq: $ => prec(PREC.equality, choice('=', '<>')),
        ord: $ => prec(PREC.comparison, choice('<', '>', '<=', '>=')),


        add_operator: $ => prec.left(PREC.add,
            seq(
                field("left", $._expression),
                field("operator", $.add),
                field("right", $._expression)
            )
        ),

        add: $ => prec(PREC.add, choice('+', '-')),

        mult_operator: $ => prec.left(PREC.modulo,
            seq(
                field("left", $._expression),
                field("operator", $.mult),
                field("right", $._expression)
            )
        ),

        mult: $ => prec(PREC.modulo, choice('*', '/', 'MOD')),

        power_operator: $ => prec.left(PREC.exponentiation,
            seq(
                field("left", $._expression),
                '**',
                field("right", $._expression)
            )
        ),

        unary_operator: $ => seq(field("operator",  $.unary), field("expr", $._expression)),

        unary: $ => prec(PREC.unary,choice('-', '+', 'NOT')),

        // A constant expression must evaluate to a constant value at compile time 
        constant_expr: $ => $._expression,

        variable_access: $ => seq(
            field("variable", $.variable),
            field("access", $.multibit_part_access)
        ),

        multibit_part_access: $ => seq(
            '.',
            choice(
                $.unsigned_int,
                seq('%', field("size", optional($.XBWDL)), $.unsigned_int)
            )
        ),

        func_call: $ => seq(
            field("target", $.fq_name),
            '(', prec(PREC.parameter_list, field("params", commaSep($.param_assign))), ')'
        ),

        stmt_list: $ => prec.left(repeat1(seq($.stmt, optional(";")))),

        stmt: $ => choice(
            // assignments
            $.assign,
            // subprog
            $.func_call,
            $.invocation,
            alias(seq('SUPER', '(', ')'), $.super),
            'RETURN',
            // iteration
            $.if_stmt,
            $.case_stmt,
            $.for_stmt,
            $.while_stmt,
            $.repeat_stmt,
            // control flow
            'EXIT',
            'CONTINUE'
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
        assign: $ => seq(
            field("variable", $.variable),
            field("target", choice(
                $.assignment_attempt,
                $.assignment,
                $.ref_assign,
            )
            )),

        assignment: $ => seq(
            ':=',
            $._expression
        ),

        assignment_attempt: $ => seq(
            '?=',
            field("target", choice($.identifier, $.ref_deref, $.ref_value))
        ),

        invocation: $ => seq(
            choice(
                $.instance_name,
                alias('THIS', $.this),
                $.this_invocation
            ),
            '(', commaSep($.param_assign), ')'
        ),

        this_invocation: $ => seq(
            seq('THIS', '.'),
            repeat1(seq($.instance_name, '.')),
            alias($.identifier, $.method_name),
            '(', prec(PREC.parameter_list, field("params", commaSep($.param_assign))), ')'
        ),

        param_assign: $ => choice(
            $.param_assign_input,
            $.param_assign_output
        ),

        param_assign_input: $ => seq(
            optional(
                seq(
                    field("param", $.identifier),
                    ':=')
            ),
            field("value", $._expression)
        ),

        param_assign_output: $ => seq(
            field("not", optional('NOT')),
            field("param", $.identifier),
            '=>',
            field("variable", $.variable)
        ),

        if_stmt: $ => seq(
            'IF',
            $._expression,
            'THEN',
            optional($.stmt_list),
            repeat(seq('ELSE IF', $._expression, 'THEN', $.stmt_list)),
            optional(seq('ELSE', $.stmt_list)),
            'END_IF'
        ),

        case_stmt: $ => seq(
            'CASE',
            $._expression,
            'OF',
            repeat1($.case_selection),
            optional(seq('ELSE', $.stmt_list)),
            'END_CASE'
        ),

        case_selection: $ => seq(
            $.case_list,
            ':',
            $.stmt_list
        ),

        case_list: $ => commaSep1($.case_list_elem),

        case_list_elem: $ => choice(
            $.subrange,
            $.constant_expr
        ),

        for_stmt: $ => seq(
            "FOR",
            $.control_variable,
            ':=',
            $.for_list,
            'DO',
            $.stmt_list,
            'END_FOR'
        ),

        control_variable: $ => $.identifier,

        for_list: $ => seq(
            $._expression,
            'TO',
            $._expression,
            optional(seq('BY', $._expression))
        ),

        while_stmt: $ => seq(
            'WHILE',
            $._primary_expression,
            'DO',
            $.stmt_list,
            'END_WHILE'
        ),

        repeat_stmt: $ => seq(
            'REPEAT',
            $.stmt_list,
            'UNTIL',
            $._expression,
            'END_REPEAT'
        ),

        // Other

        // Fully qualified name (not present in the standard)
        // Inspired from rust and C++ qualified names
        fq_name: $ => prec.left(
            seq(
                optional(seq(repeat(seq("::", field("fragment", $.identifier))), "::")),
                field("target", $.identifier)
            ),
        ),

        IQM: $ => choice('I', 'Q', 'M'),
        XBWDL: $ => choice('X', 'B', 'W', 'D', 'L'),

        // Table 73 - 76 - Graphic languages elements 

        ladder_diagram: $ => repeat1(
            $.ld_rung,
        ),

        ld_rung: $ => "todo_lad",

        // same
        fb_diagram: $ => repeat1(
            $.fbd_network,
        ),

        fbd_network: $ => "todo_fbd",

        // Table 1 - Character sets
        // Table 2 - Identifiers

        _bit: $ => /[01]/,
        _octal_digit: $ => /[0-7]/,
        _hex_digit: $ => /[0-9a-fA-F]/,
        identifier: $ => /[0-9a-zA-Z_]+/,
    }
});
