// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The conversions `Std.Convert` names for what an assignment widens on its own,
//! each at the edge of the type it starts from, where a wrong sign extension or
//! a wrong lane shows. That every such pair has a function is checked by
//! `crates/doc/src/casts.rs`.

use rstest::rstest;

use super::{compile_with_libraries, execute_wasm, with_db};

const CALLS: [(&str, &str); 39] = [
    ("BOOL_TO_BYTE(TRUE)", "BYTE#1"),
    ("BOOL_TO_WORD(TRUE)", "WORD#1"),
    ("BOOL_TO_DWORD(TRUE)", "DWORD#1"),
    ("BOOL_TO_LWORD(TRUE)", "LWORD#1"),
    ("BYTE_TO_WORD(BYTE#255)", "WORD#255"),
    ("BYTE_TO_DWORD(BYTE#255)", "DWORD#255"),
    ("BYTE_TO_LWORD(BYTE#255)", "LWORD#255"),
    ("WORD_TO_DWORD(WORD#65535)", "DWORD#65535"),
    ("WORD_TO_LWORD(WORD#65535)", "LWORD#65535"),
    ("DWORD_TO_LWORD(DWORD#4294967295)", "LWORD#4294967295"),
    ("SINT_TO_INT(SINT#-128)", "INT#-128"),
    ("SINT_TO_DINT(SINT#-128)", "DINT#-128"),
    ("SINT_TO_LINT(SINT#-128)", "LINT#-128"),
    ("SINT_TO_REAL(SINT#-128)", "REAL#-128.0"),
    ("SINT_TO_LREAL(SINT#-128)", "LREAL#-128.0"),
    ("INT_TO_DINT(INT#-32768)", "DINT#-32768"),
    ("INT_TO_LINT(INT#-32768)", "LINT#-32768"),
    ("INT_TO_REAL(INT#-32768)", "REAL#-32768.0"),
    ("INT_TO_LREAL(INT#-32768)", "LREAL#-32768.0"),
    ("DINT_TO_LINT(DINT#-2147483648)", "LINT#-2147483648"),
    ("DINT_TO_LREAL(DINT#-2147483648)", "LREAL#-2147483648.0"),
    ("USINT_TO_INT(USINT#255)", "INT#255"),
    ("USINT_TO_DINT(USINT#255)", "DINT#255"),
    ("USINT_TO_LINT(USINT#255)", "LINT#255"),
    ("USINT_TO_UINT(USINT#255)", "UINT#255"),
    ("USINT_TO_UDINT(USINT#255)", "UDINT#255"),
    ("USINT_TO_ULINT(USINT#255)", "ULINT#255"),
    ("USINT_TO_REAL(USINT#255)", "REAL#255.0"),
    ("USINT_TO_LREAL(USINT#255)", "LREAL#255.0"),
    ("UINT_TO_DINT(UINT#65535)", "DINT#65535"),
    ("UINT_TO_LINT(UINT#65535)", "LINT#65535"),
    ("UINT_TO_UDINT(UINT#65535)", "UDINT#65535"),
    ("UINT_TO_ULINT(UINT#65535)", "ULINT#65535"),
    ("UINT_TO_REAL(UINT#65535)", "REAL#65535.0"),
    ("UINT_TO_LREAL(UINT#65535)", "LREAL#65535.0"),
    ("UDINT_TO_LINT(UDINT#4294967295)", "LINT#4294967295"),
    ("UDINT_TO_ULINT(UDINT#4294967295)", "ULINT#4294967295"),
    ("UDINT_TO_LREAL(UDINT#4294967295)", "LREAL#4294967295.0"),
    ("REAL_TO_LREAL(REAL#1.5)", "LREAL#1.5"),
];

#[rstest]
fn each_widening_keeps_the_value(mut with_db: db::RootDatabase) {
    let mut source = String::from("USING Std.Convert;\nFUNCTION test : DINT\n");
    for (i, (call, value)) in CALLS.iter().enumerate() {
        source += &format!(
            "    IF {call} <> {value} THEN test := {}; RETURN; END_IF;\n",
            i + 1
        );
    }
    source += "END_FUNCTION\n";
    let wasm = compile_with_libraries(
        &mut with_db,
        &[
            include_str!("../../../stdlib/Convert.st"),
            include_str!("../../../stdlib/Strings.st"),
        ],
        &source,
    );
    let failed: i32 = execute_wasm(&wasm, "test", ());
    if failed != 0 {
        panic!("{} is wrong", CALLS[failed as usize - 1].0);
    }
}
