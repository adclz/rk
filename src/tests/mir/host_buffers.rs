// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What MIR says of the buffers a host writes into, which codegen sizes the
//! scratch by: the largest STRING the module declares, wherever it is, and
//! for each import that gives a list the longest array a call passes it.

use db::RootDatabase;
use rstest::rstest;

use crate::tests::utils::{add_sources, assert_workspace_is_clean, lower_workspace, with_db};

fn lowered(db: &mut RootDatabase, sources: &[&str]) -> mir::MirModule {
    add_sources(db, sources);
    assert_workspace_is_clean(db);
    lower_workspace(db)
}

/// A STRING of 12345 bytes in each place one is declared, beside smaller
/// ones: a FUNCTION's local, input and result, a PROGRAM's variable, a
/// FUNCTION_BLOCK's, a configuration's global, a STRUCT's field, an
/// array's element. A string the host gives may land in any of them.
#[rstest]
#[case::a_local(
    "FUNCTION f : INT VAR s : STRING[12345]; t : STRING; END_VAR s := 'x'; f := 1; END_FUNCTION"
)]
#[case::an_input(
    "FUNCTION f : INT VAR_INPUT s : STRING[12345]; END_VAR VAR t : STRING; END_VAR f := 1; END_FUNCTION"
)]
#[case::a_result("FUNCTION f : STRING[12345] VAR t : STRING; END_VAR f := t; END_FUNCTION")]
#[case::a_program_s_variable(
    "PROGRAM P VAR s : STRING[12345]; t : STRING; END_VAR s := t; END_PROGRAM
     CONFIGURATION C RESOURCE R ON CPU TASK T(INTERVAL := T#10ms, PRIORITY := 1);
     PROGRAM Main WITH T : P; END_RESOURCE END_CONFIGURATION"
)]
#[case::a_block_s_variable(
    "FUNCTION_BLOCK B VAR s : STRING[12345]; END_VAR s := 'x'; END_FUNCTION_BLOCK
     PROGRAM P VAR b : B; t : STRING; END_VAR b(); END_PROGRAM
     CONFIGURATION C RESOURCE R ON CPU TASK T(INTERVAL := T#10ms, PRIORITY := 1);
     PROGRAM Main WITH T : P; END_RESOURCE END_CONFIGURATION"
)]
#[case::a_global(
    "PROGRAM P VAR_EXTERNAL g : STRING[12345]; END_VAR VAR t : STRING; END_VAR t := g; END_PROGRAM
     CONFIGURATION C VAR_GLOBAL g : STRING[12345]; END_VAR
     RESOURCE R ON CPU TASK T(INTERVAL := T#10ms, PRIORITY := 1);
     PROGRAM Main WITH T : P; END_RESOURCE END_CONFIGURATION"
)]
#[case::a_field(
    "TYPE Named : STRUCT id : INT; s : STRING[12345]; END_STRUCT END_TYPE
     FUNCTION f : INT VAR n : Named; t : STRING; END_VAR n.id := 1; f := n.id; END_FUNCTION"
)]
#[case::an_element(
    "FUNCTION f : INT VAR a : ARRAY[0..1] OF STRING[12345]; t : STRING; END_VAR a[0] := t; f := 1;
     END_FUNCTION"
)]
fn the_largest_string_is_found_where_it_is_declared(
    mut with_db: RootDatabase,
    #[case] source: &str,
) {
    let module = lowered(&mut with_db, &[source]);
    assert_eq!(module.largest_string, 12345);
}

/// A module that declares no STRING has none to write into.
#[rstest]
fn a_module_with_no_string_has_none_to_write_into(mut with_db: RootDatabase) {
    let module = lowered(
        &mut with_db,
        &["FUNCTION f : INT VAR a : ARRAY[0..9] OF INT; END_VAR f := a[0]; END_FUNCTION"],
    );
    assert_eq!(module.largest_string, 0);
}

/// Each import that gives a list keeps the longest array any of its calls
/// passes, the elements of one array or of the other, through a FUNCTION
/// that passes its own `ARRAY[*]` on too; one declared with its arrays
/// before its input reads its calls the same way. An import that gives no
/// list keeps none.
#[rstest]
fn the_longest_list_is_the_longest_array_a_call_passes(mut with_db: RootDatabase) {
    let source = r#"
{extern 'wasi:test/rt@0.1.0' 'entries'}
FUNCTION entries
VAR_INPUT limit : UDINT; END_VAR
VAR_IN_OUT ids : ARRAY[*] OF UDINT; names : ARRAY[*] OF STRING; END_VAR
VAR_OUTPUT w0 : LWORD; w1 : LWORD; END_VAR
END_FUNCTION

{extern 'wasi:test/rt@0.1.0' 'readings'}
FUNCTION readings
VAR_IN_OUT values : ARRAY[*] OF REAL; END_VAR
VAR_INPUT limit : UDINT; END_VAR
VAR_OUTPUT w0 : LWORD; w1 : LWORD; END_VAR
END_FUNCTION

{extern 'wasi:test/rt@0.1.0' 'tick'}
FUNCTION tick : UDINT
END_FUNCTION

FUNCTION forward : UDINT
VAR_IN_OUT ids : ARRAY[*] OF UDINT; names : ARRAY[*] OF STRING[16]; END_VAR
    entries(limit := 1, ids := ids, names := names);
    forward := 0;
END_FUNCTION

FUNCTION Use : UDINT
VAR
    ids : ARRAY[1..4] OF UDINT;
    names : ARRAY[0..3] OF STRING[16];
    more : ARRAY[0..9] OF UDINT;
    long : ARRAY[0..6] OF STRING[200];
    many : ARRAY[0..99] OF UDINT;
    labels : ARRAY[0..99] OF STRING[16];
    values : ARRAY[0..41] OF REAL;
END_VAR
    entries(limit := 4, ids := ids, names := names);
    entries(limit := 10, ids := more, names := long);
    forward(ids := many, names := labels);
    readings(values := values, limit := 42);
    Use := ids[1] + more[0] + many[0] + tick();
END_FUNCTION"#;
    let module = lowered(&mut with_db, &[source]);
    let longest = |name: &str| {
        let at = module
            .extern_functions
            .iter()
            .position(|ext| ext.import_name == name)
            .unwrap_or_else(|| panic!("the module imports `{name}`"));
        module.longest_lists[at]
    };
    assert_eq!(longest("entries"), 100);
    assert_eq!(longest("readings"), 42);
    assert_eq!(longest("tick"), 0);
}
