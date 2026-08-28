//! STRING codegen, end to end: the passing MATRIX (every `{source} x
//! {operation}` combination, validated with `wasmparser`) and the STORAGE
//! semantics (instance fields, globals, defaults, FB call I/O — compiled AND
//! executed, so the values are pinned, not just the encoding).
//!
//! ## Matrix: source kinds (where a STRING value comes from)
//!
//! | Tag  | LocalInfo / shape                                 | Notes |
//! |------|---------------------------------------------------|-------|
//! | `L`  | `MirExpr::StringLiteral { offset, len }`          | `'lit'` or `"lit"` |
//! | `IP` | `LocalInfo::StringParam { ptr_idx, len_idx }`     | `VAR_INPUT s : STRING` — 2 wasm i32 params |
//! | `IO` | `LocalInfo::StringInOutParam { addr_idx, cap_idx }` | `VAR_IN_OUT s : STRING` — 2 wasm i32 params |
//! | `MM` | `LocalInfo::StringMemory { address, capacity }`   | `VAR s : STRING` or function return slot |
//! | `RT` | call returning `STRING`                           | (ptr, len) left on stack by the callee |
//!
//! Owned strings that are NOT function-locals — instance fields, globals,
//! array elements — address uniformly through `emit_addr_of` and are covered
//! by the executed-semantics half below rather than extra matrix rows.
//!
//! ## Matrix: operations
//!
//! | Tag      | Site                                                  |
//! |----------|--------------------------------------------------------|
//! | `=MM`    | assign INTO a memory-resident STRING (local or return) |
//! | `=IP`    | assign INTO a `VAR_INPUT STRING` param (in-place mutation) |
//! | `=IO`    | assign INTO a `VAR_IN_OUT STRING` param                |
//! | `->VAL`  | pass as `VAR_INPUT STRING` arg                         |
//! | `->REF`  | pass as `VAR_IN_OUT STRING` arg                        |
//! | `->RAISE`| use as `__RAISE` payload                               |
//! | `->NEST` | pass as a STRING arg where the source is itself a STRING-returning call (triggers snapshot dance) |
//!
//! |        | `=MM` | `=IP` | `=IO` | `->VAL` | `->REF` | `->RAISE` | `->NEST` |
//! |--------|-------|-------|-------|---------|---------|-----------|----------|
//! | `L`    |   x   |   x   |   x   |    x    |    .    |     x     |    .     |
//! | `IP`   |   x   |   x   |   x   |    x    |    .    |     x     |    x     |
//! | `IO`   |   x   |   x   |   x   |    x    |    x    |     x     |    .     |
//! | `MM`   |   x   |   x   |   x   |    x    |    x    |     x     |    x     |
//! | `RT`   |   x   |   x   |   x   |    x    |    .    |     x     |    x     |
//!
//! `.` cells: `L/RT -> REF` and `IO -> NEST` need an lvalue; `L -> NEST` has no
//! nested STRING-returning call; `IP -> REF` is grammatically allowed but its
//! behaviour is not strictly defined, skipped.
//!
//! A combination that regresses to invalid wasm gets `#[ignore = "BROKEN: ..."]`
//! until fixed; none currently. Two audit-era holes are fixed and pinned below
//! as `regression_*`: an empty-body FUNCTION whose call sites emitted index 0,
//! and STRING params consuming two wasm slots while local indexing counted one.
//! Also fixed and pinned here (2026-08-28): a STRING default on an instance's
//! first field emitted a scalar store over the literal pool (invalid wasm), and
//! an FB call writing a STRING input never armed the `rk.str_assign` graft.

use rstest::rstest;
use runtime::{Config, Plc};

use super::{add_source, compile_to_mir_and_wasm, compile_to_wasm, execute_wasm, with_db};

// =========================================================================
// The matrix: compile + validate
// =========================================================================


/// Compile a source string to core wasm and validate it. Returns Ok on
/// successful validation. On failure, dumps the wasm to a per-test
/// `/tmp/str_audit_<name>.wasm` so the developer can `wasm-tools print`
/// the malformed module — passing tests don't dump anything.
fn validate(db: &mut db::RootDatabase, source: &str, dump_name: &str) -> Result<(), String> {
    let file = add_source(db, source);
    let sem_idx = hir::hir_def::semantic_index::semantic_index(db, file);
    let mir_module = match mir::lower::lower_module::lower_module(db, sem_idx) {
        Ok(m) => m,
        Err(e) => return Err(format!("MIR lowering failed: {:?}", e)),
    };
    let core_bytes = wasm_codegen::generate_wasm(db, &mir_module).finish();
    let mut features = wasmparser::WasmFeatures::default();
    features.insert(wasmparser::WasmFeatures::EXCEPTIONS);
    match wasmparser::Validator::new_with_features(features).validate_all(&core_bytes) {
        Ok(_) => Ok(()),
        Err(e) => {
            let dump_path = format!("/tmp/str_audit_{}.wasm", dump_name);
            std::fs::write(&dump_path, &core_bytes).ok();
            Err(format!("validation failed ({}): {}", dump_path, e))
        }
    }
}

// Tiny shared prelude that every fixture pulls in. Provides STRING
// utilities so we can build realistic source/operation combinations
// without dragging in the full stdlib.
const PRELUDE: &str = r#"
FUNCTION str_concat : STRING
VAR_INPUT a : STRING; b : STRING; END_VAR
    {wasm 'str.concat' (params a b) (result str_concat)}
END_FUNCTION

FUNCTION len_of : UDINT
VAR_INPUT s : STRING; END_VAR
    {wasm 'str.byte_len' (params s) (result len_of)}
END_FUNCTION

FUNCTION push_str
VAR_INPUT a : STRING; END_VAR
VAR_IN_OUT b : STRING; END_VAR
    b := str_concat(b, a);
END_FUNCTION

// Body is non-empty on purpose: `lower_module` skips functions with
// zero statements (treats them as stubs), and an unresolved call to a
// dropped function silently falls back to func index 0 — a *separate*
// codegen bug worth a fix of its own, but one that would mask the
// STRING-passing behaviour the audit is trying to measure.
FUNCTION sink_string
VAR_INPUT s : STRING; END_VAR
VAR n : UDINT; END_VAR
    n := len_of(s);
END_FUNCTION

FUNCTION returns_string : STRING
VAR_INPUT v : INT; END_VAR
    returns_string := 'fixed';
END_FUNCTION
"#;

fn full_source(body: &str) -> String {
    format!("{}\n{}", PRELUDE, body)
}

// =============================================================================
// L (literal) sources
// =============================================================================

/// `L → =MM`: literal assigned to a local STRING var.
#[rstest]
fn l_assign_mm(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR s : STRING; END_VAR
    s := 'hi';
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "l_assign_mm").unwrap();
}

/// `L → =IP`: literal assigned to a VAR_INPUT STRING (mutating an input).
#[rstest]
fn l_assign_ip(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_INPUT s : STRING; END_VAR
    s := 'hi';
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "l_assign_ip").unwrap();
}

/// `L → =IO`: literal assigned to a VAR_IN_OUT STRING.
#[rstest]
fn l_assign_io(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_IN_OUT s : STRING; END_VAR
    s := 'hi';
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "l_assign_io").unwrap();
}

/// `L → →VAL`: literal passed as VAR_INPUT STRING arg.
#[rstest]
fn l_pass_val(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
    sink_string('hi');
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "l_pass_val").unwrap();
}

/// `L → →RAISE`: literal used as `__RAISE` payload.
#[rstest]
fn l_raise(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
    __RAISE('boom');
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "l_raise").unwrap();
}

// =============================================================================
// IP (VAR_INPUT STRING param) sources
// =============================================================================

/// `IP → =MM`: input STRING param assigned to a local memory STRING.
#[rstest]
fn ip_assign_mm(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_INPUT inp : STRING; END_VAR
VAR local : STRING; END_VAR
    local := inp;
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "ip_assign_mm").unwrap();
}

/// `IP → =IP`: copy one VAR_INPUT STRING into another (mutating the second).
#[rstest]
fn ip_assign_ip(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_INPUT a : STRING; b : STRING; END_VAR
    b := a;
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "ip_assign_ip").unwrap();
}

/// `IP → =IO`: input STRING assigned to a VAR_IN_OUT STRING param.
#[rstest]
fn ip_assign_io(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_INPUT src : STRING; END_VAR
VAR_IN_OUT dst : STRING; END_VAR
    dst := src;
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "ip_assign_io").unwrap();
}

/// `IP → →VAL`: forward a VAR_INPUT STRING as a value arg to another function.
#[rstest]
fn ip_pass_val(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_INPUT s : STRING; END_VAR
    sink_string(s);
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "ip_pass_val").unwrap();
}

/// `IP → →RAISE`: raise carrying the input STRING.
#[rstest]
fn ip_raise(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_INPUT msg : STRING; END_VAR
    __RAISE(msg);
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "ip_raise").unwrap();
}

// =============================================================================
// IO (VAR_IN_OUT STRING param) sources
// =============================================================================

/// `IO → =MM`: VAR_IN_OUT STRING read into a local memory STRING.
#[rstest]
fn io_assign_mm(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_IN_OUT src : STRING; END_VAR
VAR local : STRING; END_VAR
    local := src;
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "io_assign_mm").unwrap();
}

/// `IO → →VAL`: forward a VAR_IN_OUT STRING by value to another function.
#[rstest]
fn io_pass_val(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_IN_OUT s : STRING; END_VAR
    sink_string(s);
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "io_pass_val").unwrap();
}

// =============================================================================
// MM (local memory STRING) sources
// =============================================================================

/// `MM → =MM`: copy one local STRING into another.
#[rstest]
fn mm_assign_mm(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR a : STRING; b : STRING; END_VAR
    a := 'seed';
    b := a;
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "mm_assign_mm").unwrap();
}

/// `MM → →VAL`: pass a local STRING as a value arg.
#[rstest]
fn mm_pass_val(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR s : STRING; END_VAR
    s := 'seed';
    sink_string(s);
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "mm_pass_val").unwrap();
}

/// `MM → →REF`: pass a local STRING as VAR_IN_OUT.
#[rstest]
fn mm_pass_ref(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR s : STRING; END_VAR
    s := 'seed';
    push_str('!', s);
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "mm_pass_ref").unwrap();
}

/// `MM → →RAISE`: raise the contents of a local STRING.
#[rstest]
fn mm_raise(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR msg : STRING; END_VAR
    msg := 'whoops';
    __RAISE(msg);
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "mm_raise").unwrap();
}

// =============================================================================
// RT (STRING-returning call result) sources
// =============================================================================

/// `RT → =MM`: a STRING-returning call assigned to a local memory STRING.
#[rstest]
fn rt_assign_mm(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR s : STRING; END_VAR
    s := returns_string(0);
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "rt_assign_mm").unwrap();
}

/// `RT → →VAL`: pass a STRING-returning call result as VAR_INPUT.
#[rstest]
fn rt_pass_val(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
    sink_string(returns_string(0));
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "rt_pass_val").unwrap();
}

/// `RT → →RAISE`: raise the result of a STRING-returning call.
#[rstest]
fn rt_raise(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
    __RAISE(returns_string(0));
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "rt_raise").unwrap();
}

/// `RT → →NEST`: a STRING-returning call inside another STRING-arg call
/// position. Triggers the snapshot-dance path so the inner call's result
/// survives the outer call's argument evaluation.
#[rstest]
fn rt_nest(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR s : STRING; END_VAR
    s := str_concat(returns_string(0), returns_string(1));
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "rt_nest").unwrap();
}

// =============================================================================
// Remaining cells: IO/MM/RT into IP/IO targets, and *_NEST variants
// =============================================================================

/// `IO → =IP`: VAR_IN_OUT STRING assigned to a VAR_INPUT STRING param.
#[rstest]
fn io_assign_ip(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_INPUT dst : STRING; END_VAR
VAR_IN_OUT src : STRING; END_VAR
    dst := src;
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "io_assign_ip").unwrap();
}

/// `IO → =IO`: VAR_IN_OUT STRING assigned to another VAR_IN_OUT STRING.
#[rstest]
fn io_assign_io(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_IN_OUT a : STRING; b : STRING; END_VAR
    a := b;
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "io_assign_io").unwrap();
}

/// `IO → →REF`: forward a VAR_IN_OUT to another function as VAR_IN_OUT.
#[rstest]
fn io_pass_ref(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_IN_OUT s : STRING; END_VAR
    push_str('!', s);
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "io_pass_ref").unwrap();
}

/// `IO → →RAISE`: raise the contents of a VAR_IN_OUT STRING.
#[rstest]
fn io_raise(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_IN_OUT msg : STRING; END_VAR
    __RAISE(msg);
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "io_raise").unwrap();
}

/// `MM → =IP`: assign local STRING to a VAR_INPUT STRING param.
#[rstest]
fn mm_assign_ip(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_INPUT inp : STRING; END_VAR
VAR local : STRING; END_VAR
    local := 'seed';
    inp := local;
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "mm_assign_ip").unwrap();
}

/// `MM → =IO`: assign local STRING to a VAR_IN_OUT STRING param.
#[rstest]
fn mm_assign_io(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_IN_OUT dst : STRING; END_VAR
VAR local : STRING; END_VAR
    local := 'seed';
    dst := local;
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "mm_assign_io").unwrap();
}

/// `MM → →NEST`: a local STRING passed to a STRING-returning call which
/// is itself nested as an arg of an outer STRING-arg call. Exercises the
/// snapshot path *with a local STRING source* feeding the inner call.
#[rstest]
fn mm_nest(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR a : STRING; b : STRING; END_VAR
    a := 'left';
    b := str_concat(a, str_concat(a, '!'));
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "mm_nest").unwrap();
}

/// `RT → =IP`: STRING-returning call assigned to a VAR_INPUT STRING param.
#[rstest]
fn rt_assign_ip(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_INPUT inp : STRING; END_VAR
    inp := returns_string(0);
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "rt_assign_ip").unwrap();
}

/// `RT → =IO`: STRING-returning call assigned to a VAR_IN_OUT STRING param.
#[rstest]
fn rt_assign_io(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_IN_OUT dst : STRING; END_VAR
    dst := returns_string(0);
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "rt_assign_io").unwrap();
}

/// `IP → →NEST`: a VAR_INPUT STRING used as inner-call source whose
/// result is nested as another STRING arg.
#[rstest]
fn ip_nest(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_INPUT inp : STRING; END_VAR
VAR out : STRING; END_VAR
    out := str_concat(str_concat(inp, '!'), inp);
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "ip_nest").unwrap();
}

// =============================================================================
// Side-cases revealed by writing the audit
// =============================================================================

/// Regression: an empty-body POU used to be silently dropped by
/// `lower_module` ("skip stub functions"). Call sites to the dropped
/// function would still emit `call <some-idx>` — but since the callee
/// wasn't in `function_indices`, the lookup fell back to `0`, producing
/// invalid wasm. Fixed by lowering empty bodies as no-op MIR functions.
#[rstest]
fn regression_empty_body_function_is_callable(mut with_db: db::RootDatabase) {
    let src = format!(
        "{}\n{}",
        PRELUDE,
        r#"
FUNCTION empty_stub
VAR_INPUT s : STRING; END_VAR
END_FUNCTION

FUNCTION user
    empty_stub('hi');
END_FUNCTION
"#
    );
    validate(&mut with_db, &src, "known_bug_call_to_empty_body_fn").unwrap();
}

/// `next_local_idx` accounting: a STRING input param consumes **two**
/// wasm i32 slots (ptr + len), but every lowering path's variable loop
/// increments `next_local_idx` by 1 per param regardless of width.
/// Subsequent scalar `VAR` locals therefore get wasm indices that
/// collide with the second slot of the STRING param.
///
/// Triggers when: at least one STRING param exists *and* at least one
/// scalar Var is declared after it and is actually used. Reproducer
/// below: one STRING input + one scalar Var + reading the Var.
#[rstest]
fn string_param_then_scalar_var_index_accounting(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user : INT
VAR_INPUT s : STRING; END_VAR
VAR count : INT; END_VAR
    count := 42;
    user := count;
END_FUNCTION
"#,
    );
    validate(
        &mut with_db,
        &src,
        "string_param_then_scalar_var_index_accounting",
    )
    .unwrap();
}

/// Variant of the index-accounting test with **two** STRING inputs
/// followed by a scalar Var — bumps the expected offset by another
/// slot, so an off-by-one bug here would manifest differently from
/// the single-param case.
#[rstest]
fn two_string_params_then_scalar_var(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user : INT
VAR_INPUT a : STRING; b : STRING; END_VAR
VAR count : INT; END_VAR
    count := 7;
    user := count;
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "two_string_params_then_scalar_var").unwrap();
}

/// VAR_IN_OUT STRING param followed by a scalar Var — symmetric to the
/// `VAR_INPUT` case since both flatten to two wasm i32s.
#[rstest]
fn string_inout_param_then_scalar_var(mut with_db: db::RootDatabase) {
    let src = full_source(
        r#"
FUNCTION user
VAR_IN_OUT s : STRING; END_VAR
VAR n : INT; END_VAR
    n := 1;
    s := 'updated';
END_FUNCTION
"#,
    );
    validate(&mut with_db, &src, "string_inout_param_then_scalar_var").unwrap();
}

/// Regression: STRING params consume two wasm i32 slots, but every
/// MIR lowering path used to bump `next_local_idx` by one. The return
/// slot and subsequent Var locals were assigned to wasm-local indices
/// that aliased the STRING param's `len` slot, silently corrupting it.
/// Validated fine at the wasm level (i32 == i32) but produced wrong
/// runtime values.
///
/// Fixed by `param_wasm_width` (see `mir::lower::lower_func`) used in
/// every lowering path. Without that fix, `probe(0, 4)` returns 999
/// instead of 4 because `probe := 999` aliases the STRING param's len.
#[rstest]
fn regression_string_param_not_clobbered_by_return_write(mut with_db: db::RootDatabase) {
    use wasmtime::{Module, Store};

    // Write to `probe` (the return slot) BEFORE reading `s` again.
    // If `probe` aliases the wasm slot holding `s.len`, the second
    // `len_of(s)` call will receive the corrupted length.
    let src = full_source(
        r#"
FUNCTION probe : UDINT
VAR_INPUT s : STRING; END_VAR
    probe := 999;
    probe := len_of(s);
END_FUNCTION
"#,
    );
    let file = add_source(&mut with_db, &src);
    let sem_idx = hir::hir_def::semantic_index::semantic_index(&with_db, file);
    let mir_module = mir::lower::lower_module::lower_module(&with_db, sem_idx).unwrap();
    let bytes = wasm_codegen::generate_wasm(&with_db, &mir_module).finish();

    let engine = crate::tests::codegen::test_engine();
    let module = Module::new(&engine, &bytes).expect("wasm should validate");
    let mut store = Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let probe = instance
        .get_typed_func::<(i32, i32), i32>(&mut store, "probe")
        .unwrap();
    // Pass a "STRING" with ptr=0, len=4. Whatever bytes live at 0..4
    // don't matter; len_of just returns the second arg.
    let result = probe.call(&mut store, (0, 4)).unwrap();
    assert_eq!(
        result, 4,
        "len_of(s) should return s.len=4 but got {} \
         (`probe := 999` aliased the STRING param's len slot)",
        result
    );
}

// --- STRING comparison operators (=, <>, <, <=, >, >=) lower to the grafted
// `str.byte_cmp` builtin applied against 0. Executed, not just validated. ---

#[rstest]
fn string_equality_executes(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION str_eq : BOOL
        VAR_INPUT a : STRING; b : STRING; END_VAR
            str_eq := a = b;
        END_FUNCTION

        FUNCTION check : INT
        VAR s : STRING := 'START'; n : INT; END_VAR
            IF s = 'START' THEN n := n + 1; END_IF;      // literal RHS
            IF 'START' = s THEN n := n + 10; END_IF;     // literal LHS
            IF s <> 'STOP' THEN n := n + 100; END_IF;    // not-equal
            IF '' = '' THEN n := n + 1000; END_IF;       // empty = empty
            check := n;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "check", ());
    assert_eq!(r, 1111, "all four equality forms hold");
}

#[rstest]
fn string_ordering_executes(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION check : DINT
        VAR n : DINT; END_VAR
            IF 'abc' < 'abd' THEN n := n + 1; END_IF;     // lexicographic
            IF 'ab' < 'abc' THEN n := n + 10; END_IF;     // shared prefix: shorter is smaller
            IF 'abd' > 'abc' THEN n := n + 100; END_IF;
            IF 'abc' <= 'abc' THEN n := n + 1000; END_IF;
            IF 'abc' >= 'abc' THEN n := n + 10000; END_IF;
            IF NOT ('abc' < 'abc') THEN n := n + 100000; END_IF;
            check := n;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "check", ());
    assert_eq!(r, 111111, "IEC lexicographic ordering with length tiebreak");
}

#[rstest]
fn string_comparison_of_producer_results_executes(mut with_db: db::RootDatabase) {
    // Both operands are STRING-returning calls — the per-call-site snapshot
    // must keep the first result alive while the second producer runs.
    let source = r#"
        FUNCTION tag : STRING
        VAR_INPUT which : INT; END_VAR
            IF which = 1 THEN tag := 'one'; ELSE tag := 'two'; END_IF;
        END_FUNCTION

        FUNCTION check : INT
        VAR n : INT; END_VAR
            IF tag(1) = tag(1) THEN n := n + 1; END_IF;
            IF tag(1) <> tag(2) THEN n := n + 10; END_IF;
            check := n;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "check", ());
    assert_eq!(r, 11, "producer-vs-producer comparison uses snapshotted operands");
}

/// A literal that exactly FILLS the destination still passes: the check is
/// about overflow, not about discouraging full strings.
#[rstest]
fn literal_exactly_at_capacity_is_fine(mut with_db: db::RootDatabase) {
    let source = full_source(
        r#"
FUNCTION get : UDINT
VAR s : STRING[5]; END_VAR
    s := 'five!';
    get := len_of(s);
END_FUNCTION
"#,
    );
    let wasm = compile_to_wasm(&mut with_db, &source);
    let result: i32 = execute_wasm(&wasm, "get", ());
    assert_eq!(result, 5);
}

/// The capacity rule, both sides of it, in one place.
///
/// A length is enforced where it CAN be: a literal is measured at check
/// (E0309, `semantics::literals::strings`), because the compiler knows both
/// the capacity and the length. A variable is not — `s5 := s100` says nothing,
/// because the length is only known while running — so the store truncates to
/// the destination's capacity instead.
///
/// That split is deliberate, and this pins the runtime half of it: the write
/// keeps the first `capacity` bytes and the string stays well-formed, rather
/// than overrunning the slot or being refused. The compile-time half is a
/// snapshot in another file; this is the sentence that says they are one rule.
#[rstest]
fn a_variable_wider_than_its_destination_truncates(mut with_db: db::RootDatabase) {
    let source = full_source(
        r#"
FUNCTION get : UDINT
VAR
    wide : STRING[20];
    narrow : STRING[5];
END_VAR
    wide := 'twenty characters!!';
    narrow := wide;
    get := len_of(narrow);
END_FUNCTION
"#,
    );
    let wasm = compile_to_wasm(&mut with_db, &source);
    let result: i32 = execute_wasm(&wasm, "get", ());
    assert_eq!(
        result, 5,
        "the assignment kept the destination's capacity, and said nothing at check"
    );
}

// =========================================================================
// Storage semantics: instance fields, globals, defaults, FB call I/O —
// compiled and executed
// =========================================================================

/// Decode the string at the start of the retain band: `[len:i32]` + bytes.
fn read_retain_string(plc: &Plc) -> String {
    let r = plc.read_retain();
    let len = i32::from_le_bytes(r[0..4].try_into().unwrap()) as usize;
    String::from_utf8_lossy(&r[4..4 + len]).to_string()
}

/// Assigning a literal to a STRING instance field, then reading it back.
#[rstest]
fn string_field_assign_literal(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN s : STRING; END_VAR
            s := 'hello';
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(read_retain_string(&plc), "hello");
}

/// Copying one STRING instance field into another (`dst := src`).
#[rstest]
fn string_field_to_field_copy(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN dst : STRING; END_VAR
        VAR src : STRING; END_VAR
            src := 'world';
            dst := src;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(read_retain_string(&plc), "world");
}

/// A STRING instance-field initializer is applied at load (`__init`), before
/// any scan.
#[rstest]
fn string_field_initializer(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN s : STRING := 'init!'; END_VAR
            ; // no-op body; the initializer is what we check
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let plc = Plc::load(&wasm, Config::default()).expect("load");
    assert_eq!(
        read_retain_string(&plc),
        "init!",
        "initializer applied at load"
    );
}

/// A STRING VAR_GLOBAL initializer is applied at load and visible to programs.
#[rstest]
fn string_global_initializer(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Mirror
        VAR RETAIN seen : STRING; END_VAR
            seen := g;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL g : STRING := 'globinit'; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P WITH T : Mirror;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(read_retain_string(&plc), "globinit");
}

/// A struct initializer with a STRING field (aggregate + string init together).
#[rstest]
fn struct_with_string_initializer(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Person : STRUCT name : STRING; age : INT; END_STRUCT; END_TYPE

        PROGRAM P
        VAR RETAIN p : Person := (name := 'bob', age := 30); END_VAR
            ;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let plc = Plc::load(&wasm, Config::default()).expect("load");
    let r = plc.read_retain();
    // p.name (STRING) at offset 0; p.age (INT) at offset 4+80 = 84.
    assert_eq!(read_retain_string(&plc), "bob");
    assert_eq!(i32::from_le_bytes(r[84..88].try_into().unwrap()), 30);
}

/// Assigning a STRING-returning call's result into an instance field
/// (producer result → field via rk.str_assign).
#[rstest]
fn string_call_result_into_field(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION echo : STRING
        VAR_INPUT x : STRING; END_VAR
            echo := x;
        END_FUNCTION

        PROGRAM P
        VAR RETAIN s : STRING; END_VAR
            s := echo('hi there');
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(read_retain_string(&plc), "hi there");
}

/// Passing a STRING instance field as a by-value VAR_INPUT argument.
#[rstest]
fn string_field_as_argument(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION echo : STRING
        VAR_INPUT x : STRING; END_VAR
            echo := x;
        END_FUNCTION

        PROGRAM P
        VAR RETAIN out : STRING; END_VAR
        VAR src : STRING; END_VAR
            src := 'fieldarg';
            out := echo(src);
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(read_retain_string(&plc), "fieldarg");
}

/// Passing a STRING instance field as a `VAR_IN_OUT` argument (by reference):
/// the callee mutates the field in place. Exercises the (header_addr, cap)
/// flattening for a field place — previously only `StringMemory` locals worked.
#[rstest]
fn string_field_as_var_in_out(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION set_hi
        VAR_IN_OUT s : STRING; END_VAR
            s := 'hi-inout';
        END_FUNCTION

        PROGRAM P
        VAR RETAIN s : STRING; END_VAR
            set_hi(s);
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(read_retain_string(&plc), "hi-inout");
}

/// A declared `STRING[N]` capacity is honored for an instance FIELD: assigning a
/// longer string clamps to N (with the old bug, fields defaulted to capacity 80
/// and would store the whole string).
#[rstest]
fn sized_string_field_clamps_to_capacity(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN s : STRING[3]; END_VAR
        VAR src : STRING[8]; END_VAR
            src := 'hello';
            s := src;   (* from a variable: an over-long literal is E0309 *)
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(read_retain_string(&plc), "hel", "STRING[3] clamps 'hello'");
}

/// `STRING[N]` capacity is honored for a VAR_GLOBAL too.
#[rstest]
fn sized_string_global_clamps_to_capacity(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN seen : STRING[10]; END_VAR
        VAR src : STRING[8]; END_VAR
            src := 'abcdef';
            g := src;   (* from a variable: an over-long literal is E0309 *)
            seen := g;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL g : STRING[4]; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(
        read_retain_string(&plc),
        "abcd",
        "STRING[4] global clamps 'abcdef'"
    );
}

/// A STRING VAR_GLOBAL written by one program and read by another.
#[rstest]
fn string_global_shared(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM Setter
            g := 'shared';
        END_PROGRAM

        PROGRAM Mirror
        VAR RETAIN seen : STRING; END_VAR
            seen := g;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL g : STRING; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : Setter;
                PROGRAM P2 WITH T : Mirror;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(2).expect("scans"); // Setter writes g, Mirror copies it into seen
    assert_eq!(read_retain_string(&plc), "shared");
}

/// Regression: `ARRAY[..] OF STRING[n]` used to lay out 4+80-byte elements and
/// never truncate, because `lower_array_type` lowered the element through
/// `lower_type` alone. `Type::normalize` collapses `STRING[n]` and plain
/// `STRING` onto the same type, so the declared length only survives on the
/// SPEC — and this was the one call site that did not consult it.
///
/// Asserted through TRUNCATION, not through a neighbouring guard: the wrong
/// layout over-allocates (84 bytes per element instead of 8), so nothing is
/// ever clobbered and a guard variable passes either way. What actually
/// differs is how much of the source string the element keeps.
#[rstest]
fn array_of_sized_strings_truncates_at_the_declared_capacity(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR
            a : ARRAY[0..1] OF STRING[4];
            src : STRING[16];
        END_VAR
            src := 'ABCDEFGHIJKLMNOP';
            a[0] := src;   (* from a variable: an over-long literal is E0309 *)
            IF a[0] = 'ABCD' THEN run := 1; ELSE run := 0; END_IF;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 1, "an element of ARRAY OF STRING[4] holds 4 characters");
}

/// A `STRING[n]` reached through a `TYPE` alias keeps its length. The alias
/// carries a `Target` spec, so the `SizedString` sits on the data type's own
/// spec one hop away; not following that hop silently gave every aliased
/// string the 80-byte default.
#[rstest]
fn aliased_sized_string_truncates_at_the_declared_capacity(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : STRING[4]; END_TYPE

        FUNCTION run : DINT
        VAR
            s : Small;
            src : STRING[16];
        END_VAR
            src := 'ABCDEFGHIJKLMNOP';
            s := src;   (* from a variable: an over-long literal is E0309 *)
            IF s = 'ABCD' THEN run := 1; ELSE run := 0; END_IF;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 1, "an aliased STRING[4] holds 4 characters");
}

/// A declared `STRING[n]` keeps its length in EVERY container it can appear in.
///
/// The length is not part of type identity — `STRING[4] := STRING[80]` is legal
/// and truncates, and two lengths must not read as different overloads — so it
/// survives only on the spec, and every container has to lower through the
/// spec-aware path. Three separate bugs came from one container forgetting:
/// an `ARRAY OF STRING[4]` with 84-byte elements, a `TYPE` alias silently
/// widened to 80, and a struct field overrunning its slot.
///
/// One test over all of them, so a container that regresses is visible next to
/// the ones that do not.
#[rstest]
#[case::direct("s", "VAR s : STRING[4]; END_VAR")]
#[case::alias("s", "VAR s : Small; END_VAR")]
#[case::struct_field("r.f", "VAR r : Rec; END_VAR")]
#[case::fb_member("h.s", "VAR h : Holder; END_VAR")]
#[case::array_element("a[1]", "VAR a : ARRAY[0..1] OF STRING[4]; END_VAR")]
#[case::array_of_alias("b[1]", "VAR b : ARRAY[0..1] OF Small; END_VAR")]
#[case::struct_in_array("c[1].f", "VAR c : ARRAY[0..1] OF Rec; END_VAR")]
fn a_sized_string_keeps_its_length_in_every_container(
    mut with_db: db::RootDatabase,
    #[case] target: &str,
    #[case] decl: &str,
) {
    let source = format!(
        r#"
        TYPE Small : STRING[4]; END_TYPE
        TYPE Rec : STRUCT f : STRING[4]; g : DINT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK Holder
        VAR
            s : STRING[4];
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        {decl}
        VAR src : STRING[16]; END_VAR
            (* From a VARIABLE: an over-long LITERAL is refused at the
               assignment (E0309), and truncating is what a variable does. *)
            src := 'ABCDEFGHIJKLMNOP';
            {target} := src;
            IF {target} = 'ABCD' THEN run := 1; ELSE run := 0; END_IF;
        END_FUNCTION
    "#
    );
    let wasm = super::compile_to_wasm(&mut with_db, &source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 1, "`{target}` must hold exactly its declared 4 characters");
}

/// Escape sequences decode to the bytes they DENOTE, not the source text.
/// MIR used to intern the raw literal bytes while HIR validated the decoded
/// form, so `'A$0AB'` was checked as 3 characters and executed as 5 — and a
/// program's strings silently carried `$`-signs into production. One decoder
/// (`parse_single_byte_string`) now serves both.
#[rstest]
fn string_escapes_decode_to_denoted_bytes(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN s : STRING; END_VAR
            (* $41='A', $$ = one dollar, $N = LF, $T = tab, $'= quote *)
            s := '$41$$$N$T$'';
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(read_retain_string(&plc), "A$\n\t'");
}

/// A caller writing a literal into an FB's STRING input, with NO other string
/// activity in the module. The graft trigger walked Assign targets but not
/// FbCall input_writes/output_reads, so `rk.str_assign` was never grafted and
/// codegen died on its `.expect` — the exact shape of a Modbus-style
/// `client(host := '10.0.0.7')`.
#[rstest]
fn fb_string_input_written_at_the_call(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Echo
        VAR_INPUT host : STRING[16]; END_VAR
        VAR_OUTPUT got : STRING[16]; END_VAR
            got := host;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR e : Echo; ok : INT := 0; END_VAR
            e(host := '10.0.0.7');
            IF e.got = '10.0.0.7' THEN ok := 1; END_IF;
            test := ok;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 1, "the literal written at the call reaches the field");
}

/// A STRING default on an instance's FIRST field. `lower_init_leaves`'s
/// offset-0 shortcut addressed the whole struct local, hiding the leaf's
/// STRING type from codegen: the scalar path stored the LENGTH over the
/// literal's own pool bytes and left an operand on the stack — invalid wasm,
/// and a silently corrupted string pool.
#[rstest]
fn fb_string_default_on_first_field(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Cfg
        VAR_INPUT host : STRING[16] := '192.168.0.1'; END_VAR
        VAR_OUTPUT got : STRING[16]; END_VAR
            got := host;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR c : Cfg; ok : INT := 0; END_VAR
            c();
            IF c.got = '192.168.0.1' THEN ok := 1; END_IF;
            test := ok;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 1, "the default survives in the instance AND the pool");
}

/// The Modbus shape end to end: a default the first instance keeps and the
/// second overrides, in one module, so the pool literal is shared and must
/// stay intact after both inits.
#[rstest]
fn fb_string_default_and_override_coexist(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Cfg
        VAR_INPUT host : STRING[16] := '192.168.0.1'; END_VAR
        VAR_OUTPUT got : STRING[16]; END_VAR
            got := host;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR a : Cfg; b : Cfg; ok : INT := 0; END_VAR
            a();
            b(host := '10.0.0.7');
            IF a.got = '192.168.0.1' AND b.got = '10.0.0.7' THEN ok := 1; END_IF;
            test := ok;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 1, "default and override are independent instances");
}
