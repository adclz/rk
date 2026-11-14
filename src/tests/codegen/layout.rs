// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A value takes its own size, aligned to it: a BOOL and an 8-bit integer
//! one byte, a 16-bit one two. A write covers its value's bytes and no
//! more, and a STRUCT lays its values out as the canonical ABI lays out a
//! record.

use debug_format::{DEBUG_SYMBOLS_SECTION, DebugInfo, DebugSymbols, TypeDesc, VarValue};
use insta::assert_snapshot;
use rstest::rstest;
use wit_parser::{Resolve, SizeAlign, Type, TypeDefKind};

use crate::tests::codegen::{TestPlc, compile_to_mir_and_wasm, expect_section, run, with_db};

/// Values side by side in a STRUCT, written from the last to the first:
/// each write leaves the values after it as they were, and a FALSE BOOL
/// before other values reads FALSE, its byte alone. The result has a bit
/// for each value that is not what was written.
#[rstest]
fn a_write_leaves_the_value_after_it(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Packed : STRUCT
            a : BOOL; b : SINT; c : BYTE; d : BOOL; e : INT; f : WORD; g : USINT;
        END_STRUCT
        END_TYPE

        FUNCTION check : DINT
        VAR p : Packed; END_VAR
            p.g := 16#5A;
            p.f := 16#1234;
            p.e := -2;
            p.d := TRUE;
            p.c := 16#C3;
            p.b := -7;
            p.a := FALSE;
            check := 0;
            IF p.a THEN check := check + 1; END_IF;
            IF p.b <> -7 THEN check := check + 2; END_IF;
            IF p.c <> 16#C3 THEN check := check + 4; END_IF;
            IF NOT p.d THEN check := check + 8; END_IF;
            IF p.e <> -2 THEN check := check + 16; END_IF;
            IF p.f <> 16#1234 THEN check := check + 32; END_IF;
            IF p.g <> 16#5A THEN check := check + 64; END_IF;
        END_FUNCTION
    "#;
    let overwritten: i32 = run(&mut with_db, source, "check", ());
    assert_eq!(overwritten, 0, "a bit for each value overwritten");
}

/// An enum or a subrange reached through a VAR_IN_OUT, a VAR_OUTPUT or a
/// REF_TO is written at its storage's width: the BYTE or the INT after it
/// keeps its value. A write through the pointer took four bytes, whatever
/// the value's own.
#[rstest]
fn an_enum_or_a_subrange_written_through_a_pointer_leaves_the_value_after_it(
    mut with_db: db::RootDatabase,
) {
    let source = r#"
        TYPE Color : USINT (Red, Green, Blue); END_TYPE
        TYPE Small : INT (0..100); END_TYPE
        TYPE Holder : STRUCT
            c : Color; after_c : BYTE;
            s : Small; after_s : INT;
        END_STRUCT
        END_TYPE

        FUNCTION paint
        VAR_IN_OUT c : Color; END_VAR
            c := Color#Blue;
        END_FUNCTION

        FUNCTION pick
        VAR_OUTPUT c : Color; END_VAR
            c := Color#Green;
        END_FUNCTION

        FUNCTION bump
        VAR_IN_OUT s : Small; END_VAR
            s := 42;
        END_FUNCTION

        FUNCTION fill
        VAR_OUTPUT s : Small; END_VAR
            s := 7;
        END_FUNCTION

        FUNCTION check : DINT
        VAR h : Holder; r : REF_TO Color; q : REF_TO Small; END_VAR
            h.after_c := 16#AA;
            h.after_s := -3;
            check := 0;
            paint(c := h.c);
            IF h.c <> Color#Blue OR h.after_c <> 16#AA THEN check := check + 1; END_IF;
            pick(c => h.c);
            IF h.c <> Color#Green OR h.after_c <> 16#AA THEN check := check + 2; END_IF;
            bump(s := h.s);
            IF h.s <> 42 OR h.after_s <> -3 THEN check := check + 4; END_IF;
            fill(s => h.s);
            IF h.s <> 7 OR h.after_s <> -3 THEN check := check + 8; END_IF;
            r := REF(h.c);
            r^ := Color#Red;
            IF h.c <> Color#Red OR h.after_c <> 16#AA THEN check := check + 16; END_IF;
            q := REF(h.s);
            q^ := 99;
            IF h.s <> 99 OR h.after_s <> -3 THEN check := check + 32; END_IF;
        END_FUNCTION
    "#;
    let overwritten: i32 = run(&mut with_db, source, "check", ());
    assert_eq!(
        overwritten, 0,
        "a bit for each write that went past its value"
    );
}

/// A debugger forcing a value by name writes its bytes and no more: the
/// SINT after a BOOL and the INT after an INT keep their values.
#[rstest]
fn a_debugger_forcing_a_value_writes_its_bytes_alone(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR
            flag : BOOL; after_flag : SINT := -5;
            level : INT; after_level : INT := 1234;
        END_VAR
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let info = DebugInfo::from_wasm(&wasm);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.write_var(&info, "P1.flag", VarValue::Bool(true))
        .expect("force the BOOL");
    plc.write_var(&info, "P1.level", VarValue::I16(-2))
        .expect("force the INT");

    let read = |path: &str| plc.read_var(&info, path).expect(path);
    assert_eq!(read("P1.flag"), VarValue::Bool(true));
    assert_eq!(
        read("P1.after_flag"),
        VarValue::I8(-5),
        "the SINT after the BOOL"
    );
    assert_eq!(read("P1.level"), VarValue::I16(-2));
    assert_eq!(
        read("P1.after_level"),
        VarValue::I16(1234),
        "the INT after the INT"
    );
}

/// A STRUCT of scalars has the layout of the WIT record of the same fields:
/// each field where the canonical ABI puts it, and the same size, which
/// wit-parser computes. A BOOL is a `bool`, an integer the one of its
/// width, an enum on USINT an `enum`, a CHAR a `char`.
#[rstest]
fn a_struct_is_laid_out_as_the_canonical_abi_lays_out_a_record(mut with_db: db::RootDatabase) {
    let wit = r#"
        package rk:layout;

        interface shapes {
            enum color { red, green, blue }
            record small { a: bool, b: s8, c: u8 }
            record mixed { a: bool, b: s64, c: s16, d: s8, e: s32, f: u8 }
            record halves { a: u8, b: u16, c: u8, d: f32, e: u16 }
            record nested { a: bool, inner: mixed, b: u16, c: color, d: f64, e: char }
            record wide { a: u8, b: u64, c: u16, d: f32, e: bool, f: u32 }
        }
    "#;
    let source = r#"
        TYPE Color : USINT (Red, Green, Blue); END_TYPE
        TYPE Small : STRUCT a : BOOL; b : SINT; c : USINT; END_STRUCT END_TYPE
        TYPE Mixed : STRUCT
            a : BOOL; b : LINT; c : INT; d : SINT; e : DINT; f : BYTE;
        END_STRUCT
        END_TYPE
        TYPE Halves : STRUCT a : USINT; b : UINT; c : BYTE; d : REAL; e : WORD; END_STRUCT END_TYPE
        TYPE Nested : STRUCT
            a : BOOL; inner : Mixed; b : UINT; c : Color; d : LREAL; e : CHAR;
        END_STRUCT
        END_TYPE
        TYPE Wide : STRUCT
            a : USINT; b : ULINT; c : UINT; d : REAL; e : BOOL; f : UDINT;
        END_STRUCT
        END_TYPE

        PROGRAM P
        VAR
            s : ARRAY[0..1] OF Small;
            m : ARRAY[0..1] OF Mixed;
            h : ARRAY[0..1] OF Halves;
            n : ARRAY[0..1] OF Nested;
            w : ARRAY[0..1] OF Wide;
        END_VAR
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;

    let mut resolve = Resolve::default();
    resolve.push_str("layout.wit", wit).expect("the WIT parses");
    let mut sizes = SizeAlign::default();
    sizes.fill(&resolve).expect("sizes");
    let mut canonical: Vec<String> = resolve
        .types
        .iter()
        .filter_map(|(id, def)| {
            let TypeDefKind::Record(record) = &def.kind else {
                return None;
            };
            let types: Vec<Type> = record.fields.iter().map(|field| field.ty).collect();
            let offsets = sizes.field_offsets(&types);
            let fields: Vec<String> = record
                .fields
                .iter()
                .zip(offsets)
                .map(|(field, (offset, _))| format!("{} {}", field.name, offset.size_wasm32()))
                .collect();
            Some(format!(
                "{}: {}; {}",
                def.name.as_deref().unwrap_or_default(),
                fields.join(", "),
                sizes.size(&Type::Id(id)).size_wasm32()
            ))
        })
        .collect();
    canonical.sort();

    // The table describes a STRUCT where an array's elements need it, which
    // is why the program holds arrays of them.
    let (_, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let table = DebugSymbols::from_msgpack(expect_section(&wasm, DEBUG_SYMBOLS_SECTION))
        .expect("a debug symbol table");
    let mut laid_out: Vec<String> = table
        .types
        .iter()
        .filter_map(|ty| {
            let TypeDesc::Struct { name, size, fields } = ty else {
                return None;
            };
            let fields: Vec<String> = fields
                .iter()
                .map(|field| format!("{} {}", field.name.to_lowercase(), field.offset))
                .collect();
            Some(format!(
                "{}: {}; {size}",
                name.to_lowercase(),
                fields.join(", ")
            ))
        })
        .collect();
    laid_out.sort();
    laid_out.dedup();

    assert_eq!(
        laid_out, canonical,
        "rk's STRUCT against the canonical record"
    );
    assert_snapshot!(laid_out.join("\n"), @r"
    halves: a 0, b 2, c 4, d 8, e 12; 16
    mixed: a 0, b 8, c 16, d 18, e 20, f 24; 32
    nested: a 0, inner 8, b 40, c 42, d 48, e 56; 64
    small: a 0, b 1, c 2; 3
    wide: a 0, b 8, c 16, d 20, e 24, f 28; 32
    ");
}
