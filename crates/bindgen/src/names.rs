// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! How a WIT name is written in ST: a namespace in words that start with a
//! capital (`monotonic-clock` is `MonotonicClock`), a POU, a type or an
//! enum's value in capitals joined by `_` (`read-via-stream` is
//! `READ_VIA_STREAM`), a variable in lowercase (`how-long` is `how_long`).
//! A name that is a keyword of ST takes a `_` after it: the interface
//! `exit` is the namespace `Exit_`.

/// The words ST reserves, which no generated name may be: the grammar's
/// keywords and the elementary types, compared without case, as ST
/// compares names. `src/tests/bindgen.rs` checks the list against the
/// grammar's.
pub const KEYWORDS: &[&str] = &[
    // The grammar's keywords (crates/tree-sitter/src/node-types.json).
    "ABSTRACT",
    "ACTION",
    "AND",
    "ARRAY",
    "AT",
    "BY",
    "CASE",
    "CHAR",
    "CLASS",
    "CONFIGURATION",
    "CONSTANT",
    "CONTINUE",
    "D",
    "DO",
    "DS",
    "ELSE",
    "ELSIF",
    "END_ACTION",
    "END_CASE",
    "END_CLASS",
    "END_CONFIGURATION",
    "END_FOR",
    "END_FUNCTION",
    "END_FUNCTION_BLOCK",
    "END_IF",
    "END_INTERFACE",
    "END_METHOD",
    "END_NAMESPACE",
    "END_PROGRAM",
    "END_REPEAT",
    "END_RESOURCE",
    "END_STEP",
    "END_STRUCT",
    "END_TRANSITION",
    "END_TYPE",
    "END_VAR",
    "END_WHILE",
    "EXIT",
    "EXTENDS",
    "FALSE",
    "FINAL",
    "FOR",
    "FROM",
    "FUNCTION",
    "FUNCTION_BLOCK",
    "F_EDGE",
    "I",
    "IF",
    "IMPLEMENTS",
    "INITIAL_STEP",
    "INTERFACE",
    "INTERNAL",
    "INTERVAL",
    "L",
    "M",
    "METHOD",
    "MOD",
    "N",
    "NAMESPACE",
    "NON_RETAIN",
    "NOT",
    "OF",
    "ON",
    "OR",
    "OVERLAP",
    "OVERRIDE",
    "P",
    "PRIORITY",
    "PROGRAM",
    "Q",
    "R",
    "REF",
    "REF_TO",
    "REPEAT",
    "RESOURCE",
    "RETAIN",
    "RETURN",
    "R_EDGE",
    "S",
    "SD",
    "SINGLE",
    "SL",
    "STEP",
    "STRING",
    "STRUCT",
    "SUPER",
    "TASK",
    "THEN",
    "THIS",
    "TO",
    "TRANSITION",
    "TRUE",
    "TYPE",
    "UNTIL",
    "USING",
    "VAR",
    "VAR_ACCESS",
    "VAR_CONFIG",
    "VAR_EXTERNAL",
    "VAR_GLOBAL",
    "VAR_INPUT",
    "VAR_IN_OUT",
    "VAR_OUTPUT",
    "VAR_TEMP",
    "WHILE",
    "WITH",
    "XOR",
    // The pragmas' words.
    "ALLOW",
    "EXPORT",
    "EXTERN",
    "PARAMS",
    "RESULT",
    "WASM",
    // The elementary types.
    "BOOL",
    "BYTE",
    "WORD",
    "DWORD",
    "LWORD",
    "SINT",
    "INT",
    "DINT",
    "LINT",
    "USINT",
    "UINT",
    "UDINT",
    "ULINT",
    "REAL",
    "LREAL",
    "TIME",
    "LTIME",
    "DATE",
    "LDATE",
    "TIME_OF_DAY",
    "TOD",
    "LTOD",
    "DATE_AND_TIME",
    "DT",
    "LDT",
    "WSTRING",
    "WCHAR",
];

/// `name` with a `_` after it when it is a keyword.
fn unreserved(name: String) -> String {
    match KEYWORDS
        .iter()
        .any(|keyword| keyword.eq_ignore_ascii_case(&name))
    {
        true => name + "_",
        false => name,
    }
}

/// A namespace: each word of `name` starting with a capital, the words
/// joined (`monotonic-clock` is `MonotonicClock`).
pub fn namespace(name: &str) -> String {
    let joined = name
        .split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect();
    unreserved(joined)
}

/// A POU, a type or an enum's value: `name` in capitals, its words joined
/// by `_` (`read-via-stream` is `READ_VIA_STREAM`).
pub fn upper(name: &str) -> String {
    unreserved(name.to_ascii_uppercase().replace('-', "_"))
}

/// A variable: `name` in lowercase, its words joined by `_` (`how-long` is
/// `how_long`).
pub fn lower(name: &str) -> String {
    unreserved(name.to_ascii_lowercase().replace('-', "_"))
}
