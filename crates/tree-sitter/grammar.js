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

const RESERVED_NAMES = [
    "PROGRAM", "END_PROGRAM",
    "CONFIGURATION", "END_CONFIGURATION",
    "RESOURCE", "END_RESOURCE",
    "NAMESPACE", "END_NAMESPACE",
    "USING",
    "CLASS", "END_CLASS",
    "INTERFACE", "END_INTERFACE",
    "FUNCTION", "END_FUNCTION",
    "FUNCTION_BLOCK", "END_FUNCTION_BLOCK",
    "TYPE", "END_TYPE",
    "VAR", "END_VAR",
    "VAR_INPUT",
    "VAR_OUTPUT",
    "VAR_IN_OUT",
    "VAR_TEMP",
    "VAR_EXTERNAL",
    "VAR_GLOBAL"
];

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

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check
module.exports = grammar({
    name: "rk",

    extras: $ => [
        /\s/, // Whitespace
        $.comment,
        token(choice('\t', '\r', '\n')),
    ],

    //@ts-ignore (update cli to 0.25)
    reserved: {
        global: $ => RESERVED_NAMES,
    },

    supertypes: $ => [
        $._func_variables,
        $._fb_variables,
        $._class_variables,
        $._method_variables
    ],

    precedences: $ => [
        // string type name > byte string access
        [$.string_type_name, $.s_byte_str_spec, $.d_byte_str_spec],
        [$.unsigned_int, $.signed_int, $.int_literal, $.bit_str_literal],

        [$.global_ref_deref, $.enum_value]
    ],

    conflicts: $ => [
        [$.symbolic_variable, $.func_access, $.instance_name],
        [$.func_access, $.instance_name, $.invocation],
        [$.func_access, $.instance_name],
        [$.func_access, $.enum_value],
        [$.variable_list, $.fb_name],
        [$.instance_name],
        [$.ref_name, $.param_assign],

        [$.signed_int],
        [$.signed_int, $.bit_str_literal],

        // ambiguity in PROGRAM declaration
        [$.array_elem_init_value, $.primary_expr],
        [$.struct_elem_init, $.primary_expr],
        [$.string_type_name, $.var_spec],

        // local variable declarations
        [$.var_decls, $.loc_var_decls],
        [$.var_decls, $.loc_var_decls, $.loc_partly_var_decl],
        [$.var_decls, $.loc_partly_var_decl],

        [$.retain_var_decls, $.loc_var_decls, $.loc_partly_var_decl],
        [$.retain_var_decls, $.loc_partly_var_decl],
        [$.no_retain_var_decls, $.loc_var_decls, $.loc_partly_var_decl],
        [$.no_retain_var_decls, $.loc_partly_var_decl],

        // conflicts in type declarations
        [$.enum_spec_init, $.enum_value_spec],
        [$.numeric_literal, $.enum_value_spec],
        [$.numeric_type_name, $.subrange_spec],
        [$.subrange_spec, $.enum_spec_init],

        [$.global_ref_deref],
        [$.global_ref_deref, $.symbolic_variable],
    ],

    word: $ => $.identifier,

    rules: {
        // Source file declaration
        source_file: $ => repeat(
            choice(
                $.config_decl, // Declaration of CONFIGURATION and RESOURCE
                $.prog_decl, // Declaration of PROGRAM
                $.namespace_decl, // Declaration of NAMESPACE (including all other declarations)
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
            $.bit_str_literal,
            $.bool_literal
        ),

        numeric_literal: $ => choice(
            $.int_literal,
            $.real_literal
        ),

        int_literal: $ => seq(
            optional(seq($.int_type_name, '#')),
            choice(
                $.signed_int,
                $.binary_int,
                $.octal_int,
                $.hex_int
            )
        ),

        unsigned_int: $ => prec.left(seq(
            $.digit,
            repeat(seq(optional('_',), $.digit))
        )),

        signed_int: $ => seq(
            optional(choice('+', '-')),
            $.unsigned_int
        ),

        binary_int: $ => seq(
            '2#',
            $.bit,
            repeat1(seq(optional('_'), $.bit))
        ),

        octal_int: $ => seq(
            '8#',
            $.octal_digit,
            repeat1(seq(optional('_'), $.octal_digit))
        ),

        hex_int: $ => seq(
            '16#',
            $.hex_digit,
            repeat1(seq(optional('_'), $.hex_digit))
        ),

        real_literal: $ => seq(
            optional(seq($.real_type_name, '#')),
            $.signed_int,
            '.',
            $.unsigned_int,
            optional(seq('E', $.signed_int))
        ),

        bit_str_literal: $ => seq(
            optional(seq($.multibits_type_name, '#')),
            choice(
                $.unsigned_int,
                $.binary_int,
                $.octal_int,
                $.hex_int
            )
        ),

        bool_literal: $ => seq(
            optional(seq($.bool_type_name, '#')),
            token(choice('0', '1', 'TRUE', 'FALSE'))
        ),

        // Table 6 - Character String literals
        // Table 7 - Two-character combinations in character strings

        char_literal: $ => seq(
            optional('STRING#'),
            $.char_str
        ),

        char_str: $ => choice(
            $.s_byte_char_str,
            $.d_byte_char_str
        ),

        s_byte_char_str: $ => seq(
            "'",
            $.s_byte_char_value,
            "'"
        ),

        d_byte_char_str: $ => seq(
            '"',
            $.d_byte_char_value,
            '"'
        ),

        s_byte_char_value: $ => choice(
            $.common_char_value,
            '$\'',
            '"',
            seq('$', $.hex_digit, $.hex_digit)
        ),

        d_byte_char_value: $ => choice(
            $.common_char_value,
            "'",
            '$"',
            seq('$', $.hex_digit, $.hex_digit, $.hex_digit, $.hex_digit)
        ),

        common_char_value: $ => choice(
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

        duration: $ => seq(
            choice($.time_type_name, 'T', 'LT'),
            '#',
            optional(choice('+', '-')),
            $.interval
        ),

        fix_point: $ => prec.left(seq(
            $.unsigned_int,
            optional(seq('.', $.unsigned_int))
        )),

        interval: $ => choice(
            $.days,
            $.hours,
            $.minutes,
            $.seconds,
            $.milliseconds,
            $.microseconds,
            $.nanoseconds
        ),

        days: $ => prec.left(choice(
            seq($.fix_point, 'd'),
            seq(
                $.unsigned_int, 'd', optional('_'),
                optional($.hours)
            )
        )),

        hours: $ => prec.left(choice(
            seq($.fix_point, 'h'),
            seq(
                $.unsigned_int, 'h', optional('_'),
                optional($.minutes)
            )
        )),

        minutes: $ => prec.left(choice(
            seq($.fix_point, 'm'),
            seq(
                $.unsigned_int, 'm', optional('_'),
                optional($.seconds)
            )
        )),

        seconds: $ => prec.left(choice(
            seq($.fix_point, 's'),
            seq(
                $.unsigned_int, 's', optional('_'),
                optional($.milliseconds)
            )
        )),

        milliseconds: $ => prec.left(choice(
            seq($.fix_point, 'ms'),
            seq(
                $.unsigned_int, 'ms', optional('_'),
                optional($.microseconds)
            )
        )),

        microseconds: $ => prec.left(choice(
            seq($.fix_point, 'us'),
            seq(
                $.unsigned_int, 'us', optional('_'),
                optional($.nanoseconds)
            )
        )),

        nanoseconds: $ => seq(
            $.fix_point,
            'ns'
        ),

        time_of_day: $ => seq(
            choice($.tod_type_name, 'LTIME_OF_DAY'),
            '#',
            $.daytime
        ),

        daytime: $ => seq(
            $.day_hour,
            ':',
            $.day_minute,
            ':',
            $.day_second
        ),

        day_hour: $ => $.unsigned_int,

        day_minute: $ => $.unsigned_int,

        day_second: $ => $.fix_point,

        date: $ => seq(
            choice($.date_type_name, 'D', 'LD'),
            '#',
            $.date_literal
        ),

        date_literal: $ => seq(
            $.year,
            '-',
            $.month,
            '-',
            $.day
        ),

        year: $ => $.unsigned_int,

        month: $ => $.unsigned_int,

        day: $ => $.unsigned_int,

        date_and_time: $ => seq(
            choice($.dt_type_name, 'LDATE_AND_TIME'),
            '#',
            $.date_literal,
            '-',
            $.daytime
        ),

        // Table 10 - Elementary data types

        data_type_access: $ => choice(
            $.elem_type_name,
            $.derived_type_access
        ),

        elem_type_name: $ => choice(
            $.numeric_type_name,
            $.bit_str_type_name,
            $.string_type_name,
            $.date_type_name,
            $.time_type_name,
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
            'SINT',
            'INT',
            'DINT',
            'LINT'
        ),

        unsign_int_type_name: $ => choice(
            'USINT',
            'UINT',
            'UDINT',
            'ULINT'
        ),

        real_type_name: $ => choice(
            'REAL',
            'LREAL'
        ),

        string_type_name: $ => choice(
            seq('STRING', optional(seq('[', $.unsigned_int, ']'))),
            seq('WSTRING', optional(seq('[', $.unsigned_int, ']')))
        ),

        time_type_name: $ => choice(
            'TIME',
            'LTIME'
        ),

        date_type_name: $ => choice(
            'DATE',
            'LDATE'
        ),

        tod_type_name: $ => choice(
            'TIME_OF_DAY',
            'TOD',
            'LTOD'
        ),

        dt_type_name: $ => choice(
            'DATE_AND_TIME',
            'DT',
            'LDT'
        ),

        bit_str_type_name: $ => choice(
            $.bool_type_name,
            $.multibits_type_name
        ),

        bool_type_name: $ => 'BOOL',

        multibits_type_name: $ => choice(
            'BYTE',
            'WORD',
            'DWORD',
            'LWORD'
        ),

        // Table 11 - Declaration of user-defined data types and initialization

        derived_type_access: $ => $.type_access,

        type_access: $ => seq(
            repeat(seq(field("ressource_or_namespace", $.identifier), '.')),
            field("access", $.identifier)
        ),

        data_type_decl: $ => seq(
            'TYPE',
            repeat(seq($.type_decl, ';')),
            'END_TYPE'
        ),

        type_decl: $ => seq(
            field("name", $.identifier),
            field("declaration", choice(
                $.simple_type_decl,
                $.subrange_type_decl,
                $.enum_type_decl,
                $.array_type_decl,
                $.struct_type_decl,
                $.str_type_decl,
                $.ref_type_decl,
            )),
            field("spec", optional(
                choice(
                    $.array_spec,
                    $.simple_spec,
                    $.struct_spec_init,
                    $.subrange_spec_init
                )
            ))
        ),

        spec: $ => seq(
            $.type_access,
            choice(
                $.array_spec,
                $.simple_spec,
                $.struct_spec_init,
                $.subrange_spec_init
            )
        ),

        simple_type_decl: $ => seq(':', $.simple_spec_init),

        simple_spec_init: $ => prec.left(seq(
            $.simple_spec,
            optional(seq(':=', $.constant_expr))
        )),

        simple_spec: $ => $.elem_type_name,

        subrange_type_decl: $ => seq(':', $.subrange_spec_init),

        subrange_spec_init: $ => prec.left(seq(
            $.subrange_spec,
            optional(seq(':=', $.signed_int))
        )),

        subrange_spec: $ => choice(
            seq($.int_type_name, '(', $.subrange, ')'),
            $.type_access
        ),

        subrange: $ => seq(
            $.constant_expr,
            '..',
            $.constant_expr
        ),

        enum_type_decl: $ => seq(
            ':',
            choice(seq(optional($.elem_type_name), $.named_spec_init), $.enum_spec_init)
        ),

        named_spec_init: $ => prec.left(seq(
            '(', commaSep1($.enum_value_spec), ')',
            optional(seq(':=', $.enum_value))
        )),

        enum_spec_init: $ => prec.left(seq(
            choice(
                seq('(', commaSep($.identifier), ')'),
                $.type_access
            ),
            optional(seq(':=', $.enum_value))
        )),

        enum_value_spec: $ => seq(
            $.identifier,
            optional(seq(
                ':=',
                choice($.int_literal, $.constant_expr)
            ))
        ),

        enum_value: $ => seq(
            optional(seq($.identifier, '#')),
            $.identifier
        ),

        array_type_decl: $ => seq(':', $.array_spec_init),

        array_spec_init: $ => prec.left(seq(
            $.array_spec,
            optional(seq(':=', $.array_init))
        )),

        array_spec: $ => seq('ARRAY', '[', commaSep1($.subrange), ']', 'OF', $.data_type_access),

        array_init: $ => seq('[', commaSep($.array_elem_init), ']'),

        array_elem_init: $ => choice(
            $.array_elem_init_value,
            seq($.unsigned_int, '(', optional($.array_elem_init_value), ')')
        ),

        array_elem_init_value: $ => choice(
            $.constant_expr,
            $.enum_value,
            $.struct_init,
            $.array_init
        ),

        struct_type_decl: $ => seq(':', $.struct_decl),

        struct_spec_init: $ => seq(':=', $.struct_init),

        struct_decl: $ => seq(
            'STRUCT',
            optional('OVERLAP'),
            repeat1(seq($.struct_elem_decl, ';')),
            'END_STRUCT'
        ),

        struct_elem_decl: $ => seq(
            field("name", $.identifier),
            optional(seq(
                $.located_at,
                optional($.multibit_part_access)
            )),
            ':',
            choice(
                $.simple_spec_init,
                $.subrange_spec_init,
                $.enum_spec_init,
                $.array_spec_init,
                $.struct_spec_init
            )
        ),

        struct_init: $ => seq('(', commaSep($.struct_elem_init), ')'),

        struct_elem_init: $ => seq(
            field("name", $.identifier),
            ':=',
            choice($.constant_expr, $.enum_value, $.array_init, $.struct_init, $.ref_value)
        ),

        str_type_decl: $ => prec.left(seq(
            $.string_type_name,
            ':',
            $.string_type_name,
            optional(seq(':=', $.char_str))
        )),

        // Table 16 - Directly represented variables 

        direct_variable: $ => prec.left(seq(
            '%',
            choice('I', 'Q', 'M'),
            optional(choice('X', 'B', 'W', 'D', 'L')),
            $.unsigned_int,
            repeat(seq('.', $.unsigned_int))
        )),

        // Table 12 - Reference operations 

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
            $.data_type_access
        ),

        ref_name: $ => $.identifier,

        ref_value: $ => choice(
            $.ref_addr,
            'NULL'
        ),

        ref_addr: $ => seq(
            'REF',
            '(',
            choice($.symbolic_variable, $.instance_name),
            ')'
        ),

        ref_assign: $ => seq(
            $.ref_name,
            ':=',
            choice($.ref_name, $.ref_deref, $.ref_value)
        ),

        ref_deref: $ => (
            $.ref_name,
            '^'
        ),

        // Table 13 - Declaration of variables/Table 14 – Initialization of variables 

        variable: $ => choice($.direct_variable, $.symbolic_variable),

        symbolic_variable: $ => prec(1, seq(
            optional(choice(
                seq('THIS', '.'),
                prec.left(repeat1(seq(field("namespace", $.identifier), '.')))
            )),
            choice($.ref_deref, $.multi_elem_var)
        )),

        multi_elem_var: $ => seq(
            $.ref_deref,
            prec.left(repeat1(choice($.subscript_list, $.struct_variable)))
        ),

        subscript_list: $ => seq('[', commaSep($.subscript), ']'),

        subscript: $ => $.expression,

        struct_variable: $ => seq(
            '.',
            $.struct_elem_select
        ),

        struct_elem_select: $ => $.ref_deref,

        input_decls: $ => seq(
            'VAR_INPUT',
            field("retain", optional(choice('RETAIN', 'NON_RETAIN'))),
            repeat(seq($.input_decl, optional(';'))),
            'END_VAR'
        ),

        input_decl: $ => choice(
            $.var_decl_init,
            $.edge_decl,
            $.array_conform_decl
        ),

        edge_decl: $ => seq(
            field("variables", $.variable_list),
            ':',
            'BOOL',
            field("edge", optional(choice('R_EDGE', 'F_EDGE')))
        ),

        var_decl_init: $ => seq(
            field("variables", $.variable_list),
            ':',
            field("init",
                choice(
                    $.array_spec_init,
                    $.str_var_decl,
                    seq(
                        field("type", $.identifier),
                        optional(choice(
                            seq(":=", field("default", $.identifier)),
                            seq(":=", $.struct_init),
                        )
                        )
                    )),
            )
        ),


        ref_var_decl: $ => seq(
            field("variables", $.variable_list),
            ':',
            $.ref_spec
        ),

        interface_var_decl: $ => seq(
            field("variables", $.variable_list),
            ':',
            $.type_access
        ),

        variable_list: $ => commaSep1($.identifier),

        array_conformand: $ => seq(
            'ARRAY',
            '[',
            commaSep1('*'),
            ']',
            'OF',
            $.data_type_access
        ),

        array_conform_decl: $ => seq(
            field("variables", $.variable_list),
            ':',
            $.array_conformand
        ),

        fb_decl_no_init: $ => seq(
            commaSep1($.fb_name),
            ':',
            $.type_access
        ),

        fb_decl_init: $ => seq(':=', $.struct_init),

        fb_name: $ => $.identifier,

        output_decls: $ => seq(
            'VAR_OUTPUT',
            optional(choice('RETAIN', 'NON_RETAIN')),
            repeat(seq($.output_decl, optional(';'))),
            'END_VAR'
        ),

        output_decl: $ => choice(
            $.var_decl_init,
            $.array_conform_decl
        ),

        in_out_decls: $ => seq(
            'VAR_IN_OUT',
            repeat(seq($.in_out_var_decl, optional(';'))),
            'END_VAR'
        ),

        in_out_var_decl: $ => choice(
            $.var_decl,
            $.array_conform_decl,
            $.fb_decl_no_init
        ),

        var_decl: $ => seq(
            field("variables", $.variable_list),
            ':',
            choice($.simple_spec, $.str_var_decl, $.array_var_decl, $.struct_var_decl)
        ),

        array_var_decl: $ => seq(
            field("variables", $.variable_list),

            ':',
            $.array_spec
        ),

        struct_var_decl: $ => seq(
            field("variables", $.variable_list),
            ':',
            $.type_access
        ),

        var_decls: $ => seq(
            'VAR',
            field("constant", optional('CONSTANT')),
            field("access", optional($.access_spec)),
            repeat(seq($.var_decl_init, optional(';'))),
            'END_VAR'
        ),

        retain_var_decls: $ => seq(
            'VAR',
            field("retain", 'RETAIN'),
            field("access", optional($.access_spec)),
            repeat(seq($.var_decl_init, optional(';'))),
            'END_VAR'
        ),

        loc_var_decls: $ => seq(
            'VAR',
            field("constant_or_retain", optional(choice('CONSTANT', 'RETAIN', 'NON_RETAIN'))),
            repeat(seq($.loc_var_decl, optional(';'))),
            'END_VAR'
        ),

        loc_var_decl: $ => seq(
            optional(field("variable_name", $.identifier)),
            $.located_at,
            ':',
            $.loc_var_spec_init
        ),

        temp_var_decls: $ => seq(
            'VAR_TEMP',
            repeat(seq(choice($.var_decl, $.ref_var_decl, $.interface_var_decl), optional(';'))),
            'END_VAR'
        ),

        external_var_decls: $ => seq(
            'VAR_EXTERNAL',
            field("constant", optional('CONSTANT')),
            repeat(seq($.external_decl, optional(';'))),
            'END_VAR'
        ),

        external_decl: $ => seq(
            field("global_var_name", $.identifier),
            ':',
            choice($.simple_spec, $.array_spec, $.type_access),
        ),

        global_var_decls: $ => seq(
            'VAR_GLOBAL',
            field("constant_or_retain", optional(choice('CONSTANT', 'RETAIN'))),
            repeat(seq($.global_var_decl, optional(';'))),
            'END_VAR'
        ),

        global_var_decl: $ => seq(
            field("spec", $.global_var_spec),
            ':',
            choice($.loc_var_spec_init, $.type_access)
        ),

        global_var_spec: $ => choice(
            seq(commaSep1($.identifier)),
            seq(
                $.identifier,
                $.located_at
            )
        ),

        loc_var_spec_init: $ => choice(
            $.simple_spec_init,
            $.array_spec_init,
            $.struct_spec_init,
            $.s_byte_str_spec,
            $.d_byte_str_spec,
        ),

        located_at: $ => seq(
            'AT',
            $.direct_variable
        ),

        str_var_decl: $ => choice(
            $.s_byte_str_spec,
            $.d_byte_str_spec
        ),

        s_byte_str_spec: $ => seq(
            'STRING',
            optional(seq('[', $.unsigned_int, ']')),
            optional(seq(':=', $.s_byte_char_str))
        ),

        d_byte_str_spec: $ => seq(
            'WSTRING',
            optional(seq('[', $.unsigned_int, ']')),
            optional(seq(':=', $.d_byte_char_str))
        ),

        loc_partly_var_decl: $ => seq(
            'VAR',
            optional(choice('RETAIN', 'NON_RETAIN')),
            repeat($.loc_partly_var),
            'END_VAR'
        ),

        loc_partly_var: $ => seq(
            field("variable_name", $.identifier),
            'AT',
            '%',
            choice('I', 'Q', 'M'),
            '*',
            ':',
            $.var_spec,
            optional(';')
        ),

        var_spec: $ => choice(
            $.simple_spec,
            $.array_spec,
            $.type_access,
            seq(choice('STRING', 'WSTRING'), optional(seq('[', $.unsigned_int, ']'))),
        ),

        // Table 19 - Function declaration

        func_access: $ => seq(
            repeat(seq(field("namespace", $.identifier), '.')),
            $.identifier
        ),

        func_decl: $ => seq(
            'FUNCTION',
            field("name", $.identifier),
            field("access", optional(seq(':', $.data_type_access))),
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
            field("extends", optional(seq("EXTENDS", choice($.type_access)))),
            field("implements", optional(seq("IMPLEMENTS", $.interface_name_list))),
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
            repeat(seq($.fb_input_decl, optional(';'))),
            'END_VAR'
        ),

        fb_input_decl: $ => choice(
            $.var_decl_init,
            $.edge_decl,
            $.array_conform_decl
        ),

        fb_output_decls: $ => seq(
            'VAR_OUTPUT',
            optional(choice('RETAIN', 'NON_RETAIN')),
            repeat(seq($.fb_output_decl, optional(';'))),
            'END_VAR'
        ),

        fb_output_decl: $ => choice(
            $.var_decl_init,
            $.array_conform_decl
        ),

        no_retain_var_decls: $ => seq(
            'VAR',
            'NON_RETAIN',
            optional($.access_spec),
            repeat(seq($.var_decl_init, optional(';'))),
            'END_VAR'
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
            optional(seq(':', $.data_type_access)),
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
            field("extends", optional(seq("EXTENDS", $.type_access))),
            field("implements", optional(seq("IMPLEMENTS", $.interface_name_list))),
            field("declarations", repeat($._class_variables)),
            field("method", repeat($.method_decl)),
            'END_CLASS'
        ),

        _class_variables: $ => choice(
            ...func_var_decls($),
            ...other_var_decls($)
        ),

        class_type_name: $ => $.identifier,

        instance_name: $ => seq(
            repeat(seq(field("namespace", $.identifier), '.')),
            field("name", $.identifier),
            repeat('^') //todo: check
        ),

        interface_decl: $ => seq(
            'INTERFACE',
            field("name", $.identifier),
            field("directives", repeat($.using_directive)),
            field("extends", optional(seq('EXTENDS', $.interface_name_list))),
            field("prototype", repeat($.method_prototype)),
            'END_INTERFACE'
        ),

        method_prototype: $ => seq(
            'METHOD',
            field("name", $.identifier),
            field("data_type", optional(seq(':', $.data_type_access))),
            field("variables", repeat($._method_variables)),
            'END_METHOD'
        ),

        _method_variables: $ => choice(
            ...io_var_decls($)
        ),

        interface_spec_init: $ => seq(':=', $.interface_value),

        interface_value: $ => choice(
            $.symbolic_variable,
            $.instance_name,
            'NULL'
        ),

        interface_name_list: $ => commaSep1($.type_access),

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

        prog_type_access: $ => seq(
            repeat(seq(field("namespace", $.identifier), '.')),
            $.identifier
        ),

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
            $.data_type_access,
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
            seq(':=', $.expression, ';'),
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
            repeat(seq($.access_decl, ';')),
            'END_VAR'
        ),

        access_decl: $ => seq(
            $.access_name,
            ':',
            $.access_path,
            ':',
            optional(seq($.data_type_access, $.access_direction)),
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
            optional(seq($.identifier, '.')),
            field("name", $.identifier),
            optional(seq('.', field("struct_name", $.identifier)))
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
            'END_VAR'
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
                    $.type_access,
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

        expression: $ => seq(
            $.xor_expr,
            repeat(seq('OR', $.xor_expr))
        ),

        constant_expr: $ => $.expression,
        //todo: a constant expression must evaluate to a constant value at compile time 

        xor_expr: $ => seq(
            $.and_expr,
            repeat(seq('XOR', $.and_expr))
        ),

        and_expr: $ => seq(
            $.compare_expr,
            repeat(seq(choice('&', 'AND'), $.compare_expr))
        ),

        compare_expr: $ => seq(
            $.equ_expr,
            repeat(seq(choice('=', '<>'), $.equ_expr))
        ),

        equ_expr: $ => seq(
            $.add_expr,
            repeat(seq(choice('<', '>', '<=', '>='), $.add_expr))
        ),

        add_expr: $ => prec.left(seq(
            $.term,
            repeat(seq(choice('+', '-'), $.term))
        )),

        term: $ => seq(
            $.power_expr,
            repeat(seq(choice('*', '/', seq('MOD', $.power_expr))))
        ),

        power_expr: $ => seq(
            $.unary_expr,
            repeat(seq('**', $.unary_expr))
        ),

        unary_expr: $ => seq(
            optional(choice('-', '+', 'NOT')),
            $.primary_expr
        ),

        primary_expr: $ => choice(
            $.constant,
            $.enum_value,
            $.variable_access,
            $.func_call,
            $.ref_value,
            seq('(', $.expression, ')')
        ),

        variable_access: $ => seq(
            $.variable,
            $.multibit_part_access
        ),

        multibit_part_access: $ => seq(
            '.',
            choice(
                $.unsigned_int,
                seq('%', optional(choice('X', 'B', 'W', 'D', 'L')), $.unsigned_int)
            )
        ),

        func_call: $ => seq(
            $.func_access,
            '(', commaSep($.param_assign), ')'
        ),

        stmt_list: $ => prec.right(repeat1(seq($.stmt, optional(";")))),

        stmt: $ => choice(
            $.assign_stmt,
            $.subprog_ctrl_stmt,
            $.selection_stmt,
            $.iteration_stmt,
        ),

        assign_stmt: $ => choice(
            seq($.variable, ':=', $.expression),
            $.ref_assign,
            $.assignment_attempt,
        ),

        assignment_attempt: $ => seq(
            choice($.ref_name, $.ref_deref),
            '?=',
            choice($.ref_name, $.ref_deref, $.ref_value)
        ),

        invocation: $ => seq(
            choice(
                $.instance_name,
                $.identifier,
                'THIS',
                seq(
                    optional(seq('THIS', '.')),
                    repeat1(seq(choice($.instance_name), '.')),
                    $.identifier
                )
            ),
            '(', commaSep($.param_assign), ')'
        ),

        subprog_ctrl_stmt: $ => choice(
            $.func_call,
            $.invocation,
            seq('SUPER', '(', ')'),
            'RETURN'
        ),

        param_assign: $ => choice(
            seq(optional(seq(field("variable_name", $.identifier), ':=')), $.expression),
            $.ref_assign,
            seq(optional('NOT'), field("variable_name", $.identifier), '=>', $.variable)
        ),

        selection_stmt: $ => choice(
            $.if_stmt,
            $.case_stmt
        ),

        if_stmt: $ => seq(
            'IF',
            $.expression,
            'THEN',
            $.stmt_list,
            repeat(seq('ELSE IF', $.expression, 'THEN', $.stmt_list)),
            optional(seq('ELSE', $.stmt_list)),
            'END_IF'
        ),

        case_stmt: $ => seq(
            'CASE',
            $.expression,
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

        iteration_stmt: $ => choice(
            $.for_stmt,
            $.while_stmt,
            $.repeat_stmt,
            'EXIT',
            'CONTINUE'
        ),

        for_stmt: $ => seq(
            'FOR',
            $.control_variable,
            ':=',
            $.for_list,
            'DO',
            $.stmt_list,
            'END_FOR'
        ),

        control_variable: $ => $.identifier,

        for_list: $ => seq(
            $.expression,
            'TO',
            $.expression,
            optional(seq('BY', $.expression))
        ),

        while_stmt: $ => seq(
            'WHILE',
            $.expression,
            'DO',
            $.stmt_list,
            'END_WHILE'
        ),

        repeat_stmt: $ => seq(
            'REPEAT',
            $.stmt_list,
            'UNTIL',
            $.expression,
            'END_REPEAT'
        ),

        // Table 73 - 76 - Graphic languages elements 

        // note: IEC does not specify at least one occurence of a statement in a ladder diagram
        // but since tree sitter does not support empty string, we have to add at least one rung.
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

        letter: $ => /[a-zA-Z_]/,
        digit: $ => /[0-9]/,
        bit: $ => /[01]/,
        octal_digit: $ => /[0-7]/,
        hex_digit: $ => /[0-9a-fA-F]/,
        identifier: $ => /[0-9a-zA-Z_]+/,
    }
});
