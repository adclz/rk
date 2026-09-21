//! Struct execution tests - testing struct field access and manipulation.

use crate::tests::codegen::{compile_to_wasm, with_db};
use rstest::*;

#[rstest]
fn test_struct_field_access(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point :
        STRUCT
            x : INT;
            y : INT;
        END_STRUCT
        END_TYPE

        FUNCTION get_x : INT
        VAR
            p : Point;
        END_VAR
            p.x := 10;
            p.y := 20;
            get_x := p.x;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let result = super::execute_wasm::<(), i32>(&wasm_bytes, "get_x", ());
    assert_eq!(result, 10, "Should read p.x value");
}

#[rstest]
fn test_struct_computation(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Rectangle :
        STRUCT
            width : INT;
            height : INT;
        END_STRUCT
        END_TYPE

        FUNCTION area : INT
        VAR
            rect : Rectangle;
        END_VAR
            rect.width := 5;
            rect.height := 3;
            area := rect.width * rect.height;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let result = super::execute_wasm::<(), i32>(&wasm_bytes, "area", ());
    assert_eq!(result, 15, "5 * 3 = 15");
}

#[rstest]
fn test_nested_struct(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point :
        STRUCT
            x : INT;
            y : INT;
        END_STRUCT
        END_TYPE

        TYPE Line :
        STRUCT
            start : Point;
            end_point : Point;
        END_STRUCT
        END_TYPE

        FUNCTION test_nested : INT
        VAR
            line : Line;
        END_VAR
            line.start.x := 1;
            line.start.y := 2;
            line.end_point.x := 10;
            line.end_point.y := 20;

            test_nested := line.end_point.x + line.start.x;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let result = super::execute_wasm::<(), i32>(&wasm_bytes, "test_nested", ());
    assert_eq!(result, 11, "10 + 1 = 11");
}

/// Regression: a `STRING[n]` reached through a struct or FB *field* was written
/// with capacity 80 instead of `n`, so `rk_str_assign` copied up to 80 bytes
/// into a 4+n byte slot and clobbered whatever followed it — and wrote a length
/// past the field's own buffer, so the field then read back longer than it is.
///
/// `MirPlace::Field.field_type` was re-derived from the HIR type via
/// `lower_type_resolved(path_expr.infer(db))`, and `Type::normalize` collapses
/// `STRING[n]` and plain `STRING` to the same type — losing `n`. The struct
/// layout already held the right type; it just was not read.
#[rstest]
fn sized_string_struct_field_does_not_overrun_its_slot(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Rec :
        STRUCT
            f : STRING[4];
            g : DINT;
        END_STRUCT;
        END_TYPE

        FUNCTION run : DINT
        VAR
            r : Rec;
            src : STRING[16];
        END_VAR
            r.g := 111;
            (* From a variable: an over-long LITERAL is refused at the
               assignment (E0306); truncating is what a variable does. *)
            src := 'ABCDEFGHIJKLMNOP';
            r.f := src;
            run := r.g;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 111, "the sibling field must not be clobbered");
}

/// The same overrun seen from the string's own side: the value the field holds
/// afterwards is the truncation, not the whole source string.
///
/// (Checked by comparison rather than `LEN(r.f)` only because this suite
/// compiles without the stdlib, so `LEN` is out of scope here. `LEN` on a
/// struct STRING field is exercised against the real stdlib instead.)
#[rstest]
fn sized_string_struct_field_truncates_to_its_capacity(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Rec :
        STRUCT
            f : STRING[4];
            g : DINT;
        END_STRUCT;
        END_TYPE

        FUNCTION run : DINT
        VAR
            r : Rec;
            src : STRING[16];
        END_VAR
            (* From a variable: an over-long LITERAL is refused at the
               assignment (E0306); truncating is what a variable does. *)
            src := 'ABCDEFGHIJKLMNOP';
            r.f := src;
            IF r.f = 'ABCD' THEN run := 1; ELSE run := 0; END_IF;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 1, "STRING[4] holds exactly its first 4 characters");
}

/// An FB member is the other `Field` shape with the same defect.
#[rstest]
fn sized_string_fb_member_does_not_overrun_its_slot(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Holder
        VAR
            s : STRING[4];
            guard : DINT := 222;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR
            h : Holder;
            src : STRING[16];
        END_VAR
            (* From a variable: an over-long LITERAL is refused at the
               assignment (E0306); truncating is what a variable does. *)
            src := 'ABCDEFGHIJKLMNOP';
            h.s := src;
            run := h.guard;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 222, "the FB's next member must not be clobbered");
}
