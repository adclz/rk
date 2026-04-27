// Compile-time preprocessor tests.
//
// `{#if x is T}` blocks dispatch on a parameter's concrete type at
// codegen time. The compiler validates each cond and the pragmas inside:
//
// - **E0325** `{#if x is T}` where `x` doesn't resolve in scope.
// - **E0326** `{#if x is T}` where `x` is concrete (the branch can never
//   narrow it).
// - **E0327** `{#if x is T}` where `T` isn't a variant of `x`'s `ANY_*`
//   bound (the branch can never fire).
// - **E0328** `{wasm}` pragma references a parameter whose anchor isn't
//   pinned by the enclosing `{#if}` chain. Wasm intrinsics emit a single
//   concrete instruction, so they need a pinned type.
//
// `{extern}` pragmas are intentionally untouched: those are host imports
// and the host can dispatch lazily per variant (the `generic-extern`
// lint flags those informationally as L0112).

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

// -- Valid cases: no errors ------------------------------------------

#[rstest]
fn extern_outside_if_lazy_no_error(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION abs : ANY_NUM
VAR_INPUT IN : INTO(abs); END_VAR
{extern 'math' 'abs' (params IN) (result abs)}
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn wasm_inside_if_pinned(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION SHL : ANY_BIT
VAR_INPUT
    IN : INTO(SHL);
    N : INT;
END_VAR
{#if IN is BYTE}
    {wasm IN 'shl' (params IN N) (result SHL)}
{#elif IN is WORD}
    {wasm IN 'shl' (params IN N) (result SHL)}
{#elif IN is DWORD}
    {wasm IN 'shl' (params IN N) (result SHL)}
{#elif IN is LWORD}
    {wasm IN 'shl' (params IN N) (result SHL)}
{#endif}
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn extern_inside_if_no_check(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION abs : ANY_NUM
VAR_INPUT IN : INTO(abs); END_VAR
{#if IN is REAL}
    {extern 'math' 'abs_f32' (params IN) (result abs)}
{#endif}
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn into_chain_pinned_via_anchor(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION foo : ANY_INT
VAR_INPUT
    a : INTO(foo);
    b : INTO(a);
END_VAR
{#if foo is INT}
    {wasm 'i32.add' (params a b) (result foo)}
{#endif}
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// -- E0325: ident in cond doesn't resolve ----------------------------

#[rstest]
fn e0325_ident_not_in_scope(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION foo : INT
VAR_INPUT x : INT; END_VAR
{#if y is INT}
    foo := x;
{#endif}
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0325] Error: preprocess condition: unknown identifier
       ,-[ file:///test0.st:4:6 ]
       |
     4 | {#if y is INT}
       |      |
       |      `-- 'y' is not in scope
    ---'
    ");
}

// -- E0326: ident is concrete, branch can never narrow ---------------

#[rstest]
fn e0326_ident_concrete(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION foo : INT
VAR_INPUT x : INT; END_VAR
{#if x is DINT}
    foo := x;
{#endif}
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0326] Error: preprocess condition: identifier is not generic
       ,-[ file:///test0.st:4:6 ]
       |
     4 | {#if x is DINT}
       |      |
       |      `-- 'x' has concrete type 'INT', so this branch can never narrow it
    ---'
    ");
}

// -- E0327: type after `is` isn't in the bound -----------------------

#[rstest]
fn e0327_type_not_in_bound(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION foo : ANY_INT
VAR_INPUT x : INTO(foo); END_VAR
{#if x is STRING}
    foo := x;
{#endif}
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0327] Error: preprocess condition: type not in generic bound
       ,-[ file:///test0.st:4:11 ]
       |
     4 | {#if x is STRING}
       |           ^^^|^^
       |              `---- 'STRING' is not a variant of 'ANY_INT'
    ---'
    ");
}

#[rstest]
fn e0327_real_not_in_any_int(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION foo : ANY_INT
VAR_INPUT x : INTO(foo); END_VAR
{#if x is REAL}
    foo := x;
{#endif}
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0327] Error: preprocess condition: type not in generic bound
       ,-[ file:///test0.st:4:11 ]
       |
     4 | {#if x is REAL}
       |           ^^|^
       |             `--- 'REAL' is not a variant of 'ANY_INT'
    ---'
    ");
}

// -- E0328: wasm pragma references unpinned anchor -------------------

#[rstest]
fn e0328_wasm_unpinned_other_param(mut with_db: RootDatabase) {
    // `a` gets pinned to REAL via its INTO(foo) chain; `b` is an
    // independent ANY_INT and the wasm pragma references it, so E0328
    // fires on `b` only.
    let source = r#"
FUNCTION foo : ANY_NUM
VAR_INPUT
    a : INTO(foo);
    b : ANY_INT;
END_VAR
{#if a is REAL}
    {wasm 'f32.add' (params a b) (result foo)}
{#endif}
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0328] Error: wasm pragma: generic parameter is not pinned
       ,-[ file:///test0.st:8:31 ]
       |
     8 |     {wasm 'f32.add' (params a b) (result foo)}
       |                               |
       |                               `-- 'b' is bound by 'ANY_INT' and not pinned by the enclosing '{#if}' chain
       |
       | Note: wasm pragmas need a concrete type; add a '{#if b is …}' branch
    ---'
    ");
}

#[rstest]
fn e0328_extern_silent_in_partial_pin(mut with_db: RootDatabase) {
    // Same shape as `e0328_wasm_unpinned_other_param`, but with
    // `{extern}` instead of `{wasm}`: no error, since the host
    // dispatches lazily per `b` variant.
    let source = r#"
FUNCTION foo : ANY_NUM
VAR_INPUT
    a : INTO(foo);
    b : ANY_INT;
END_VAR
{#if a is REAL}
    {extern 'm' 'fn' (params a b) (result foo)}
{#endif}
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}
