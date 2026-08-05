//! # STRING-passing audit
//!
//! Exercises every combination of `{STRING source}` × `{operation on it}`
//! that the codegen has to handle, with the smallest IEC snippet that
//! triggers each, validating the emitted core wasm with
//! `wasmparser::Validator`. Tests that compile to valid wasm = working
//! path. Tests marked `#[ignore = "BROKEN: ..."]` = recorded holes for
//! the upcoming surgical refactor to address.
//!
//! ## Source types (where a STRING value comes from)
//!
//! | Tag  | LocalInfo / shape                                 | Notes |
//! |------|---------------------------------------------------|-------|
//! | `L`  | `MirExpr::StringLiteral { offset, len }`          | `'lit'` or `"lit"` |
//! | `IP` | `LocalInfo::StringParam { ptr_idx, len_idx }`     | `VAR_INPUT s : STRING` — 2 wasm i32 params |
//! | `IO` | `LocalInfo::StringInOutParam { addr_idx, cap_idx }` | `VAR_IN_OUT s : STRING` — 2 wasm i32 params |
//! | `MM` | `LocalInfo::StringMemory { address, capacity }`   | `VAR s : STRING` or function return slot |
//! | `RT` | call returning `STRING`                           | (ptr, len) left on stack by the callee |
//!
//! ## Operations (what we do with it)
//!
//! | Tag      | Site                                                  |
//! |----------|--------------------------------------------------------|
//! | `=MM`    | assign INTO a memory-resident STRING (local or return) |
//! | `=IP`    | assign INTO a `VAR_INPUT STRING` param (in-place mutation) |
//! | `=IO`    | assign INTO a `VAR_IN_OUT STRING` param                |
//! | `→VAL`   | pass as `VAR_INPUT STRING` arg                         |
//! | `→REF`   | pass as `VAR_IN_OUT STRING` arg                        |
//! | `→RAISE` | use as `__RAISE` payload                               |
//! | `→NEST`  | pass as a STRING arg where the source is itself a STRING-returning call (triggers snapshot dance) |
//!
//! ## Matrix
//!
//! Each row is a source kind, each column an operation. Cell value:
//! - ✓  = test exists in this file and validates (compile + wasm-validate)
//! - ·  = combination is meaningless / impossible
//!
//! |        | `=MM` | `=IP` | `=IO` | `→VAL` | `→REF` | `→RAISE` | `→NEST` |
//! |--------|-------|-------|-------|--------|--------|----------|---------|
//! | `L`    |   ✓   |   ✓   |   ✓   |   ✓    |   ·    |    ✓     |   ·     |
//! | `IP`   |   ✓   |   ✓   |   ✓   |   ✓    |   ·    |    ✓     |   ✓     |
//! | `IO`   |   ✓   |   ✓   |   ✓   |   ✓    |   ✓    |    ✓     |   ·     |
//! | `MM`   |   ✓   |   ✓   |   ✓   |   ✓    |   ✓    |    ✓     |   ✓     |
//! | `RT`   |   ✓   |   ✓   |   ✓   |   ✓    |   ·    |    ✓     |   ✓     |
//!
//! `·` cells:
//! - `L → REF`, `RT → REF`, `IO → NEST`: VAR_IN_OUT requires an lvalue;
//!   literals, call-results, and another VAR_IN_OUT passed to a
//!   STRING-returning callsite are all non-lvalue in practice.
//! - `L → NEST`: a literal as the inner-call arg doesn't trigger the
//!   snapshot dance (no nested *STRING-returning* call).
//! - `IP → REF`: VAR_INPUT as VAR_IN_OUT source — currently allowed by
//!   grammar via implicit conversion; behaviour isn't strictly defined,
//!   skipped here.
//!
//! ## Surgical-refactor punch list (from the audit)
//!
//! Both surface as `#[ignore]` tests below with `BROKEN:` messages:
//!
//! 1. **`known_bug_call_to_empty_body_fn`** — `lower_module` skips
//!    functions whose body is empty, but call sites to the dropped
//!    function still emit `call` with index `0` (the `unwrap_or`
//!    fallback in `emit_call`). Produces invalid wasm whenever a
//!    user writes an empty `FUNCTION foo … END_FUNCTION` and calls it.
//!    *Fix:* either lower empty-body fns as no-op stubs, or have HIR
//!    reject them as "no observable behaviour".
//!
//! 2. **`known_bug_string_param_clobbered_by_scalar_var`** — every MIR
//!    lowering path bumps `next_local_idx` by 1 per param, but STRING
//!    params flatten to *two* wasm i32 slots. The function-return slot
//!    and subsequent scalar Vars get MIR `local_index` values that
//!    correspond to wasm locals **already used** by the STRING param's
//!    `(ptr, len)`. The wasm validates (i32==i32) but the function
//!    silently mutates the input param's slots. Test: a function that
//!    writes its return slot then re-reads the STRING returns the
//!    *written* value instead of the original `len`.
//!    *Fix:* count wasm-slots, not logical params, in `next_local_idx`
//!    bookkeeping across `lower_func.rs`, `lower_module.rs`, and
//!    `monomorphize.rs` (this is the surgical-refactor target).

use rstest::rstest;

use super::{add_source, with_db};

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
