// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Tests for WASM import generation from extern pragmas.

use rstest::rstest;

use super::{compile_to_wasm, execute_wasm_with_imports, validate_wasm, with_db};

/// A module defines its own memory, growable and as large as its layout,
/// and exports it as `memory`: a host gives it nothing, and a module that
/// declares no `{extern}` imports nothing at all.
#[rstest]
fn a_module_owns_its_memory(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION run : DINT
    run := 1;
END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let module = wasmtime::Module::new(&super::test_engine(), &wasm).expect("a valid module");
    assert_eq!(module.imports().count(), 0, "the module imports nothing");
    let memory = module
        .exports()
        .find(|export| export.name() == "memory")
        .expect("the module exports its memory");
    match memory.ty() {
        wasmtime::ExternType::Memory(memory) => {
            assert!(memory.minimum() >= 1, "{memory:?}");
            assert_eq!(memory.maximum(), None, "it can grow");
        }
        other => panic!("`memory` is a {other:?}"),
    }
}

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
    // This test IS the host-side contract for struct params, which only a
    // WASI import keeps: any other is typed in WIT, and refuses them (E1502).
    let source = r#"
TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE
{extern 'wasi:test/host@0.1.0' 'send-pt'}
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
                "wasi:test/host@0.1.0",
                "send-pt",
                |mut caller: wasmtime::Caller<'_, ()>, p: i32| -> i32 {
                    let m = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .unwrap();
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
    // A WASI import only: any other is typed in WIT, and refuses it (E1502).
    let source = r#"
{extern 'wasi:test/host@0.1.0' 'pick'}
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
                "wasi:test/host@0.1.0",
                "pick",
                |mut caller: wasmtime::Caller<'_, ()>, p: i32| -> i32 {
                    let m = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .unwrap();
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
    assert_eq!(
        recv_params,
        Some(2),
        "a STRING VAR_INPUT is (ptr, len) on the wire"
    );
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
        linker
            .func_wrap("host", "shared", |x: i32| -> i32 { x * 2 })
            .unwrap();
    });
    assert_eq!(r, 24, "both callers reach the one host function");
}

/// The memory a host function writes its record in.
fn memory_of(caller: &mut wasmtime::Caller<'_, ()>) -> wasmtime::Memory {
    caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
        .expect("the module exports its memory")
}

/// The whole result contract in one call: an import returns the record of
/// its scalar VAR_OUTPUTs in declaration order, whatever order a call binds
/// them in, the return value LAST, so an output's place never shifts when a
/// return type is added. With two fields or more, the canonical ABI has the
/// call pass the address the host writes the record at, last, as aligned as
/// the record: here `value` at 0, `status` at 8, the return value at 10.
/// The caller sees the return value as the call's value and each `=>`
/// binding filled, a discarded output included, and a variable wider than
/// its output takes the field with its sign.
#[rstest]
fn extern_outputs_return_as_a_record_return_last(mut with_db: db::RootDatabase) {
    let source = r#"
{extern 'rt' 'sample'}
FUNCTION sample : INT
VAR_INPUT channel : INT; END_VAR
VAR_OUTPUT value : LINT; status : INT; END_VAR
END_FUNCTION
FUNCTION drive4 : LINT
VAR v : LINT; s : INT; r : INT; END_VAR
    r := sample(status => s, channel := 7, value => v);
    drive4 := v * 1000 + TO_L(s) * 10 + TO_L(r);
END_FUNCTION
FUNCTION drive8 : LINT
VAR wide : LINT; r : INT; END_VAR
    r := sample(channel := 7, status => wide);
    drive8 := wide;
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
            .func_wrap(
                "rt",
                "sample",
                |mut caller: wasmtime::Caller<'_, ()>, ch: i32, at: i32| {
                    assert_eq!(at % 8, 0, "the record is as aligned as its LINT");
                    let mut record = [0u8; 16];
                    record[0..8].copy_from_slice(&i64::from(ch).to_le_bytes());
                    record[8..10].copy_from_slice(&(-2i16).to_le_bytes());
                    record[10..12].copy_from_slice(&3i16.to_le_bytes());
                    memory_of(&mut caller)
                        .write(&mut caller, at as usize, &record)
                        .unwrap();
                },
            )
            .unwrap();
    };
    let r: i64 = execute_wasm_with_imports(&wasm_bytes, "drive4", (), host);
    assert_eq!(r, 6983, "value 7, status -2, return 3");
    let r: i64 = execute_wasm_with_imports(&wasm_bytes, "drive5", (), host);
    assert_eq!(r, 703, "value 7 and return 3 survive a discarded status");
    let r: i64 = execute_wasm_with_imports(&wasm_bytes, "drive8", (), host);
    assert_eq!(r, -2, "an INT field read into a LINT keeps its sign");
}

/// A record of one field is that field: an import with one output and no
/// return value returns the output, with no address to write it at.
#[rstest]
fn an_import_with_one_output_returns_it(mut with_db: db::RootDatabase) {
    let source = r#"
{extern 'rt' 'count'}
FUNCTION count
VAR_INPUT channel : INT; END_VAR
VAR_OUTPUT seen : LINT; END_VAR
END_FUNCTION
FUNCTION drive7 : LINT
VAR n : LINT; END_VAR
    count(channel := 4, seen => n);
    drive7 := n;
END_FUNCTION
    "#;
    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let r: i64 = execute_wasm_with_imports(&wasm_bytes, "drive7", (), |linker| {
        linker
            .func_wrap("rt", "count", |channel: i32| -> i64 {
                i64::from(channel) << 40
            })
            .unwrap();
    });
    assert_eq!(r, 4 << 40);
}

/// `%MX0.3` and `%MB1` are bits of `%MW0`, so they have no address of their
/// own: each field read from the record pops into its scratch, and the word
/// is rebuilt from it once every field is off the stack. A WASI import
/// returns its record as any other does: `seen` at 0, `level` at 1, the
/// return value at 2.
#[rstest]
fn extern_outputs_land_in_parts_of_a_wider_address(mut with_db: db::RootDatabase) {
    let source = r#"
{extern 'wasi:test/rt@0.1.0' 'probe'}
FUNCTION probe : INT
VAR_INPUT channel : INT; END_VAR
VAR_OUTPUT seen : BOOL; level : BYTE; END_VAR
END_FUNCTION
FUNCTION drive6 : WORD
VAR r : INT; END_VAR
    %MW0 := 16#F000;
    r := probe(channel := 1, seen => %MX0.3, level => %MB1);
    drive6 := %MW0;
END_FUNCTION
    "#;
    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let r: i32 = execute_wasm_with_imports(&wasm_bytes, "drive6", (), |linker| {
        linker
            .func_wrap(
                "wasi:test/rt@0.1.0",
                "probe",
                |mut caller: wasmtime::Caller<'_, ()>, _ch: i32, at: i32| {
                    let record = [1u8, 0xFD, 9, 0];
                    memory_of(&mut caller)
                        .write(&mut caller, at as usize, &record)
                        .unwrap();
                },
            )
            .unwrap();
    });
    assert_eq!(r, 0xFD08, "bit 3 set, the high byte replaced");
}

/// A STRING passed to an import is checked for UTF-8 before the call: the
/// component model traps on any other bytes, a trap that stops the whole
/// PLC. The check raises instead, a `__TRY` catches it, and the host is not
/// called. Valid text reaches the host as its bytes and their count.
#[rstest]
fn a_string_passed_to_an_import_is_checked_for_utf8(mut with_db: db::RootDatabase) {
    let source = r#"
{extern 'host' 'recv'}
FUNCTION Recv : DINT
VAR_INPUT s : STRING; END_VAR
END_FUNCTION

FUNCTION Send : DINT
VAR_INPUT valid : BOOL; END_VAR
VAR e : STRING; END_VAR
    __TRY
        IF valid THEN
            Send := Recv('café');
        ELSE
            Send := Recv('caf$E9');
        END_IF;
    __CATCH(e)
        IF e = 'a STRING passed to the host is not UTF-8' THEN
            Send := -1;
        END_IF;
    __END_TRY;
END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<u8>>::new()));
    let host = |linker: &mut wasmtime::Linker<()>| {
        let received = received.clone();
        linker
            .func_wrap(
                "host",
                "recv",
                move |mut caller: wasmtime::Caller<'_, ()>, ptr: i32, len: i32| -> i32 {
                    let memory = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .unwrap();
                    let mut bytes = vec![0; len as usize];
                    memory.read(&caller, ptr as usize, &mut bytes).unwrap();
                    received.lock().unwrap().push(bytes);
                    len
                },
            )
            .unwrap();
    };
    let sent: i32 = execute_wasm_with_imports(&wasm, "Send", (1i32,), host);
    assert_eq!(sent, 5, "'café' is 5 bytes");
    let refused: i32 = execute_wasm_with_imports(&wasm, "Send", (0i32,), host);
    assert_eq!(refused, -1, "the check raised, with its message");
    assert_eq!(
        *received.lock().unwrap(),
        ["café".as_bytes()],
        "the host was called with the valid text only"
    );
}

/// A CHAR passed to an import is checked for a character before the call:
/// WIT's `char` is a Unicode scalar value, and the component model traps on
/// a surrogate or on a code past U+10FFFF.
#[rstest]
#[case::a_letter(0x78, 0x78)]
#[case::the_last_one(0x10FFFF, 0x10FFFF)]
#[case::a_surrogate(0xD800, -1)]
#[case::past_the_last_one(0x110000, -1)]
fn a_char_passed_to_an_import_is_checked(
    mut with_db: db::RootDatabase,
    #[case] code: u32,
    #[case] expected: i32,
) {
    let source = r#"
{extern 'host' 'put'}
FUNCTION Put : DINT
VAR_INPUT c : CHAR; END_VAR
END_FUNCTION

FUNCTION AS_CHAR : CHAR
VAR_INPUT IN : DWORD; END_VAR
    {wasm 'nop' (params IN) (result AS_CHAR)}
END_FUNCTION

FUNCTION Send : DINT
VAR_INPUT code : DWORD; END_VAR
VAR e : STRING; END_VAR
    __TRY
        Send := Put(AS_CHAR(code));
    __CATCH(e)
        IF e = 'a CHAR passed to the host is not a character' THEN
            Send := -1;
        END_IF;
    __END_TRY;
END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let sent: i32 = execute_wasm_with_imports(&wasm, "Send", (code as i32,), |linker| {
        linker
            .func_wrap("host", "put", |c: i32| -> i32 { c })
            .unwrap();
    });
    assert_eq!(sent, expected);
}

/// Only an import passed a STRING or a CHAR is called through a wrapper,
/// named after it in the `name` section; any other is called directly.
#[rstest]
fn only_an_import_passed_a_string_or_a_char_has_a_wrapper(mut with_db: db::RootDatabase) {
    let source = r#"
{extern 'host' 'recv'}
FUNCTION Recv : DINT
VAR_INPUT n : INT; s : STRING; END_VAR
END_FUNCTION

{extern 'host' 'put'}
FUNCTION Put : DINT
VAR_INPUT c : CHAR; END_VAR
END_FUNCTION

{extern 'math' 'abs'}
FUNCTION Abs : INT
VAR_INPUT x : INT; END_VAR
END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let mut names = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
        if let Ok(wasmparser::Payload::CustomSection(section)) = payload
            && let wasmparser::KnownCustom::Name(reader) = section.as_known()
        {
            for name in reader {
                if let Ok(wasmparser::Name::Function(map)) = name {
                    for naming in map {
                        let naming = naming.unwrap();
                        names.push((naming.index, naming.name.to_string()));
                    }
                }
            }
        }
    }
    let wrappers: Vec<&str> = names
        .iter()
        .map(|(_, name)| name.as_str())
        .filter(|name| name.ends_with("$checked"))
        .collect();
    assert_eq!(wrappers, ["Recv$checked", "Put$checked"]);
    let imports: Vec<(u32, &str)> = names
        .iter()
        .filter(|(index, _)| *index < 3)
        .map(|(index, name)| (*index, name.as_str()))
        .collect();
    assert_eq!(imports, [(0, "Recv"), (1, "Put"), (2, "Abs")]);
}
