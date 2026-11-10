// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::utils::mir_exports;
use crate::tests::utils::with_db;

#[rstest]
fn extern_pragma_generates_import(mut with_db: RootDatabase) {
    let source = r#"
{extern 'math' 'sqrt-real'}
FUNCTION my_sqrt : REAL
VAR_INPUT IN : REAL; END_VAR
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"import math.sqrt-real(Real) -> Real");
}

#[rstest]
fn extern_pragma_no_params(mut with_db: RootDatabase) {
    let source = r#"
{extern 'assert' 'fail'}
FUNCTION __ASSERT_FAIL
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"import assert.fail()");
}

#[rstest]
fn extern_pragma_multiple_params(mut with_db: RootDatabase) {
    let source = r#"
{extern 'math' 'add'}
FUNCTION ext_add : INT
VAR_INPUT a : INT; b : INT; END_VAR
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"import math.add(Int, Int) -> Int");
}

#[rstest]
fn extern_pragma_result_only(mut with_db: RootDatabase) {
    // Extern with no params but a result (e.g., clock.now)
    let source = r#"
{extern 'wasi:clocks/monotonic-clock' 'now'}
FUNCTION get_time : LINT
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"import wasi:clocks/monotonic-clock.now() -> LInt");
}

/// A record comes back through an address: WASI's system clock `now`
/// writes its `instant` where the caller points, and the zone's
/// `utc-offset` its `option<s64>`, so each extern takes a `REF_TO` input
/// and the import is a function of that `i32` too.
#[rstest]
fn extern_pragma_record_by_address(mut with_db: RootDatabase) {
    let source = r#"
TYPE Instant : STRUCT
    seconds : LINT;
    nanoseconds : DINT;
END_STRUCT;
END_TYPE

TYPE ZoneOffset : STRUCT
    known : BOOL;
    nanoseconds : LINT;
END_STRUCT;
END_TYPE

{extern 'wasi:clocks/system-clock@0.3.0' 'now'}
FUNCTION system_now
VAR_INPUT ret : REF_TO Instant; END_VAR
END_FUNCTION

{extern 'wasi:clocks/timezone@0.3.0' 'utc-offset'}
FUNCTION utc_offset
VAR_INPUT seconds : LINT; nanoseconds : DINT; ret : REF_TO ZoneOffset; END_VAR
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    import wasi:clocks/system-clock@0.3.0.now(*struct(Instant))
    import wasi:clocks/timezone@0.3.0.utc-offset(LInt, DInt, *struct(ZoneOffset))
    ");
}

/// A module that writes to the host's standard output imports the stream it
/// writes through: WASI's `write-via-stream` and the component model's
/// built-ins for its stream and its future, which no declaration names. One
/// that writes to neither output imports none.
#[rstest]
fn a_module_that_writes_imports_its_output(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Say : BOOL
VAR_INPUT IN : STRING[4096]; END_VAR
    {wasm 'rk.stdout_write' (params IN) (result Say)}
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    func Say(String { capacity: 4096 }) -> Bool
    import wasi:cli/stdout@0.3.0.[async-lower][stream-write-0]write-via-stream(DInt, DInt, DInt) -> DInt
    import wasi:cli/stdout@0.3.0.[future-drop-readable-1]write-via-stream(DInt)
    import wasi:cli/stdout@0.3.0.[stream-new-0]write-via-stream() -> LWord
    import wasi:cli/stdout@0.3.0.write-via-stream(DInt) -> DInt
    ");
}
