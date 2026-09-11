//! Tests for WASM import generation from extern pragmas.

use rstest::rstest;

use super::{compile_to_wasm, execute_wasm_with_imports, validate_wasm, with_db};

#[rstest]
fn test_extern_generates_valid_wasm(mut with_db: db::RootDatabase) {
    let source = r#"
{extern 'math' 'abs'}
FUNCTION my_abs : INT
VAR_INPUT x : INT; END_VAR
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    // Validation will fail because the import is not satisfied,
    // but the WASM binary itself should be structurally valid
    assert!(validate_wasm(&wasm_bytes).is_ok());
}

#[rstest]
fn test_extern_import_executes(mut with_db: db::RootDatabase) {
    let source = r#"
{extern 'math' 'abs'}
FUNCTION my_abs : INT
VAR_INPUT x : INT; END_VAR
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let result: i32 = execute_wasm_with_imports(&wasm_bytes, "my_abs", (-42i32,), |linker| {
        linker
            .func_wrap("math", "abs", |x: i32| -> i32 { x.abs() })
            .unwrap();
    });

    assert_eq!(result, 42);
}

#[rstest]
fn test_extern_with_real_type(mut with_db: db::RootDatabase) {
    let source = r#"
{extern 'math' 'sqrt'}
FUNCTION my_sqrt : REAL
VAR_INPUT x : REAL; END_VAR
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let result: f32 = execute_wasm_with_imports(&wasm_bytes, "my_sqrt", (9.0f32,), |linker| {
        linker
            .func_wrap("math", "sqrt", |x: f32| -> f32 { x.sqrt() })
            .unwrap();
    });

    assert!((result - 3.0).abs() < f32::EPSILON);
}

#[rstest]
fn test_extern_and_local_functions_coexist(mut with_db: db::RootDatabase) {
    let source = r#"
{extern 'math' 'add'}
FUNCTION ext_add : INT
VAR_INPUT a : INT; b : INT; END_VAR
END_FUNCTION

FUNCTION double : INT
VAR_INPUT x : INT; END_VAR
    double := x + x;
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Execute the local function (should work without providing imports for it)
    let result: i32 = execute_wasm_with_imports(&wasm_bytes, "double", (21i32,), |linker| {
        linker
            .func_wrap("math", "add", |a: i32, b: i32| -> i32 { a + b })
            .unwrap();
    });

    assert_eq!(result, 42);

    // Execute the extern function
    let result: i32 = execute_wasm_with_imports(&wasm_bytes, "ext_add", (10i32, 32i32), |linker| {
        linker
            .func_wrap("math", "add", |a: i32, b: i32| -> i32 { a + b })
            .unwrap();
    });

    assert_eq!(result, 42);
}

#[rstest]
fn test_local_function_calls_after_extern(mut with_db: db::RootDatabase) {
    // Ensure that local function indices are correct even when imports exist
    let source = r#"
{extern 'math' 'negate'}
FUNCTION ext_negate : INT
VAR_INPUT x : INT; END_VAR
END_FUNCTION

FUNCTION add_one : INT
VAR_INPUT x : INT; END_VAR
    add_one := x + 1;
END_FUNCTION

FUNCTION add_two : INT
VAR_INPUT x : INT; END_VAR
    add_two := x + 2;
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let result: i32 = execute_wasm_with_imports(&wasm_bytes, "add_one", (10i32,), |linker| {
        linker
            .func_wrap("math", "negate", |x: i32| -> i32 { -x })
            .unwrap();
    });
    assert_eq!(result, 11);

    let result: i32 = execute_wasm_with_imports(&wasm_bytes, "add_two", (10i32,), |linker| {
        linker
            .func_wrap("math", "negate", |x: i32| -> i32 { -x })
            .unwrap();
    });
    assert_eq!(result, 12);
}

#[rstest]
fn extern_struct_input_arrives_as_slot_layout_snapshot(mut with_db: db::RootDatabase) {
    // An aggregate VAR_INPUT reaches the import as a POINTER to a call-entry
    // snapshot, laid out in this compiler's 4-byte slots: every field of 32
    // bits or less occupies one i32 slot, so Pt{x: INT; y: INT} is x at +0
    // and y at +4 (NOT the IEC-packed 2-byte offsets a host might assume).
    // This test IS the host-side contract for struct params.
    let source = r#"
TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE
{extern 'host' 'send-pt'}
FUNCTION send_pt : INT
VAR_INPUT p : Pt; END_VAR
END_FUNCTION
FUNCTION drive : INT
VAR q : Pt; END_VAR
    q.x := 7; q.y := 9;
    drive := send_pt(p := q);
END_FUNCTION
    "#;
    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let r: i32 = execute_wasm_with_imports(&wasm_bytes, "drive", (), |linker| {
        linker
            .func_wrap(
                "host",
                "send-pt",
                |mut caller: wasmtime::Caller<'_, ()>, p: i32| -> i32 {
                    let m = caller.get_export("memory").and_then(|e| e.into_memory()).unwrap();
                    let mut b = [0u8; 8];
                    m.read(&caller, p as usize, &mut b).unwrap();
                    let x = i32::from_le_bytes(b[0..4].try_into().unwrap());
                    let y = i32::from_le_bytes(b[4..8].try_into().unwrap());
                    x * 100 + y
                },
            )
            .unwrap();
    });
    assert_eq!(r, 709, "x at slot +0, y at slot +4, both fields present");
}

#[rstest]
fn extern_array_input_uses_slot_stride(mut with_db: db::RootDatabase) {
    // An aggregate VAR_INPUT arrives as a POINTER to the call-entry snapshot,
    // and ARRAY[0..3] OF BYTE is FOUR 4-byte slots (stride 4), not four
    // packed bytes: a host reads one i32 per element. (VAR_IN_OUT does not
    // exist on externs at all — E1502 — so a copy in is the only direction.)
    let source = r#"
{extern 'host' 'pick'}
FUNCTION pick : BYTE
VAR_INPUT buf : ARRAY[0..3] OF BYTE; END_VAR
END_FUNCTION
FUNCTION drive2 : BYTE
VAR a : ARRAY[0..3] OF BYTE; END_VAR
    a[0] := BYTE#41; a[1] := BYTE#42; a[2] := BYTE#43; a[3] := BYTE#44;
    drive2 := pick(buf := a);
END_FUNCTION
    "#;
    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let r: i32 = execute_wasm_with_imports(&wasm_bytes, "drive2", (), |linker| {
        linker
            .func_wrap(
                "host",
                "pick",
                |mut caller: wasmtime::Caller<'_, ()>, p: i32| -> i32 {
                    let m = caller.get_export("memory").and_then(|e| e.into_memory()).unwrap();
                    let mut b = [0u8; 4];
                    m.read(&caller, p as usize + 4, &mut b).unwrap();
                    i32::from_le_bytes(b)
                },
            )
            .unwrap();
    });
    assert_eq!(r, 42, "element 1 lives one 4-byte slot in");
}

#[rstest]
fn extern_string_input_signature_is_ptr_and_len(mut with_db: db::RootDatabase) {
    // A STRING VAR_INPUT flattens to TWO i32 params on the import — the
    // host-side contract a foreign implementer needs to know. (STRING can
    // only ever flow IN: VAR_IN_OUT and STRING outputs are E1502.)
    let source = r#"
{extern 'host' 'recv'}
FUNCTION recv : INT
VAR_INPUT s : STRING; END_VAR
END_FUNCTION
    "#;
    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    assert!(validate_wasm(&wasm_bytes).is_ok());
    let mut recv_params: Option<usize> = None;
    let mut types: Vec<wasmparser::FuncType> = Vec::new();
    for p in wasmparser::Parser::new(0).parse_all(&wasm_bytes) {
        match p {
            Ok(wasmparser::Payload::TypeSection(r)) => {
                for t in r.into_iter_err_on_gc_types() {
                    types.push(t.unwrap());
                }
            }
            Ok(wasmparser::Payload::ImportSection(r)) => {
                for g in r {
                    if let wasmparser::Imports::Single(_, imp) = g.unwrap()
                        && imp.module == "host"
                        && imp.name == "recv"
                        && let wasmparser::TypeRef::Func(ti) = imp.ty
                    {
                        recv_params = Some(types[ti as usize].params().len());
                    }
                }
            }
            _ => {}
        }
    }
    assert_eq!(recv_params, Some(2), "a STRING VAR_INPUT is (ptr, len) on the wire");
}

#[rstest]
fn two_pous_may_share_one_import(mut with_db: db::RootDatabase) {
    // Two FUNCTIONs importing the same host symbol both bind and both run.
    let source = r#"
{extern 'host' 'shared'}
FUNCTION a1 : INT
VAR_INPUT x : INT; END_VAR
END_FUNCTION
{extern 'host' 'shared'}
FUNCTION a2 : INT
VAR_INPUT x : INT; END_VAR
END_FUNCTION
FUNCTION drive3 : INT
    drive3 := a1(x := 5) + a2(x := 7);
END_FUNCTION
    "#;
    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let r: i32 = execute_wasm_with_imports(&wasm_bytes, "drive3", (), |linker| {
        linker.func_wrap("host", "shared", |x: i32| -> i32 { x * 2 }).unwrap();
    });
    assert_eq!(r, 24, "both callers reach the one host function");
}

#[rstest]
fn extern_outputs_return_as_results_return_last(mut with_db: db::RootDatabase) {
    // The whole result contract in one call: scalar VAR_OUTPUTs come back as
    // wasm results in declaration order, the return value LAST — so output
    // positions never shift when a return type is added. The host returns a
    // tuple; the caller sees the return value as the call's value and each
    // `=>` binding filled, including a discarded output popping cleanly.
    let source = r#"
{extern 'rt' 'sample'}
FUNCTION sample : INT
VAR_INPUT channel : INT; END_VAR
VAR_OUTPUT value : LINT; status : INT; END_VAR
END_FUNCTION
FUNCTION drive4 : LINT
VAR v : LINT; s : INT; r : INT; END_VAR
    r := sample(channel := 7, value => v, status => s);
    drive4 := v * 1000 + TO_L(s) * 10 + TO_L(r);
END_FUNCTION
FUNCTION drive5 : LINT
VAR v : LINT; r : INT; END_VAR
    (* status discarded: still pops off the stack without corrupting it *)
    r := sample(channel := 7, value => v);
    drive5 := v * 100 + TO_L(r);
END_FUNCTION
FUNCTION TO_L : LINT
VAR_INPUT IN : INT; END_VAR
    TO_L := IN;
END_FUNCTION
    "#;
    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let host = |linker: &mut wasmtime::Linker<()>| {
        linker
            .func_wrap("rt", "sample", |ch: i32| -> (i64, i32, i32) {
                // (value, status, return): value=ch as i64, status=2, ret=3
                (ch as i64, 2, 3)
            })
            .unwrap();
    };
    let r: i64 = execute_wasm_with_imports(&wasm_bytes, "drive4", (), host);
    assert_eq!(r, 7023, "value 7, status 2, return 3");
    let r: i64 = execute_wasm_with_imports(&wasm_bytes, "drive5", (), host);
    assert_eq!(r, 703, "value 7 and return 3 survive a discarded status");
}
