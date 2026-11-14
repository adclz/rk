// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Flags: a typed enum on the bit string the canonical ABI holds them in,
//! each flag a bit of it by name.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;
use wasmtime::component::{Linker, Val};

use super::{checked, hosted};
use crate::tests::cli::component_host::Host;
use crate::tests::utils::with_db;

/// Flags of 8 at most are named values of a BYTE, of 16 of a WORD, of 32
/// of a DWORD, the first flag the lowest bit. They pass and come back as
/// one value, and a record's is one of its outputs.
#[rstest]
fn flags_are_named_bits(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface files {
            flags open-flags { create, directory, exclusive, truncate }
            flags modes { m0, m1, m2, m3, m4, m5, m6, m7, m8 }
            flags rights {
                r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15, r16,
            }
            record entry { how: open-flags, size: u64, modes: modes }
            open: func(how: open-flags, modes: modes) -> rights;
            last: func() -> entry;
        }

        world example {
            import files;
        }
    "#), @r"
    // wasi-example-files.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Files
    			TYPE OPEN_FLAGS : BYTE (
    				CREATE := 16#01,
    				DIRECTORY := 16#02,
    				EXCLUSIVE := 16#04,
    				TRUNCATE := 16#08
    			); END_TYPE

    			TYPE MODES : WORD (
    				M0 := 16#0001,
    				M1 := 16#0002,
    				M2 := 16#0004,
    				M3 := 16#0008,
    				M4 := 16#0010,
    				M5 := 16#0020,
    				M6 := 16#0040,
    				M7 := 16#0080,
    				M8 := 16#0100
    			); END_TYPE

    			TYPE RIGHTS : DWORD (
    				R0 := 16#00000001,
    				R1 := 16#00000002,
    				R2 := 16#00000004,
    				R3 := 16#00000008,
    				R4 := 16#00000010,
    				R5 := 16#00000020,
    				R6 := 16#00000040,
    				R7 := 16#00000080,
    				R8 := 16#00000100,
    				R9 := 16#00000200,
    				R10 := 16#00000400,
    				R11 := 16#00000800,
    				R12 := 16#00001000,
    				R13 := 16#00002000,
    				R14 := 16#00004000,
    				R15 := 16#00008000,
    				R16 := 16#00010000
    			); END_TYPE

    			{extern 'wasi:example/files@0.3.0' 'open'}
    			FUNCTION OPEN : RIGHTS
    			VAR_INPUT
    				how : OPEN_FLAGS;
    				modes : MODES;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/files@0.3.0' 'last'}
    			FUNCTION LAST
    			VAR_OUTPUT
    				how : OPEN_FLAGS;
    				size : ULINT;
    				modes : MODES;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// Flags a host takes and gives, and a record's.
const MARKS: &str = r#"
    package wasi:example@0.3.0;

    interface marks {
        flags style { bold, dashed, filled }
        flags wide { w0, w1, w2, w3, w4, w5, w6, w7, w8, w9 }
        record pen { style: style, size: u32, wide: wide }
        invert: func(style: style) -> style;
        pen-of: func(given: wide) -> pen;
    }

    world example {
        import marks;
    }
"#;

/// The host's side of [`MARKS`]: `invert` gives the flags it was not
/// given, `pen-of` a pen with the flags it was given and `dashed`.
fn marks_host(linker: &mut Linker<Host>) {
    let mut marks = linker
        .instance("wasi:example/marks@0.3.0")
        .expect("the interface");
    marks
        .func_new("invert", |_, _, params, results| {
            let [Val::Flags(given)] = params else {
                panic!("flags: {params:?}");
            };
            let others = ["bold", "dashed", "filled"]
                .iter()
                .filter(|flag| !given.iter().any(|given| given == *flag))
                .map(|flag| flag.to_string())
                .collect();
            results[0] = Val::Flags(others);
            Ok(())
        })
        .expect("a function of the host");
    marks
        .func_new("pen-of", |_, _, params, results| {
            let [given] = params else {
                panic!("flags: {params:?}");
            };
            results[0] = Val::Record(vec![
                ("style".to_string(), Val::Flags(vec!["dashed".to_string()])),
                ("size".to_string(), Val::U32(7)),
                ("wide".to_string(), given.clone()),
            ]);
            Ok(())
        })
        .expect("a function of the host");
}

/// Flags passed to a host and given back by it, by name on both sides: a
/// flag is the same bit for the generated code and for wasmtime. 1 when
/// each came back as the host gave it.
#[rstest]
fn flags_pass_and_come_back_bit_for_bit(mut with_db: RootDatabase) {
    let program = r#"
        PROGRAM P
        VAR_EXTERNAL output : DINT; END_VAR
        VAR
            style : Wasi.Example.Marks.STYLE;
            wide : Wasi.Example.Marks.WIDE;
            size : UDINT;
        END_VAR
            output := 1;
            style := Wasi.Example.Marks.INVERT(style := Wasi.Example.Marks.STYLE#DASHED);
            IF style <> (Wasi.Example.Marks.STYLE#BOLD OR Wasi.Example.Marks.STYLE#FILLED) THEN
                output := -1;
            END_IF;
            style := Wasi.Example.Marks.INVERT(style := 0);
            IF style <> 16#07 THEN output := -2; END_IF;
            Wasi.Example.Marks.PEN_OF(
                given := Wasi.Example.Marks.WIDE#W0 OR Wasi.Example.Marks.WIDE#W9,
                style => style, size => size, wide => wide);
            IF style <> Wasi.Example.Marks.STYLE#DASHED OR size <> 7 OR wide <> 16#0201 THEN
                output := -3;
            END_IF;
        END_PROGRAM
    "#;
    let mut plc = hosted(&mut with_db, MARKS, program, marks_host);
    assert_eq!(
        plc.scan(0),
        1,
        "the number of the check that failed, negative"
    );
}
