//! Date and time literals through codegen: the integer encodings, the unit
//! conversions of the cast emitter, and the implicit widenings whose
//! containment the `const` assertions in hir's literals.rs declare.

use crate::tests::codegen::{compile_to_wasm, with_db};
use rstest::*;
use wasmtime::{Module, Store};

/// Sanity-check the date/time integer encoding end-to-end. The cast emitter
/// is responsible for the unit conversions documented in `stdlib/Convert.st`;
/// these round-trips would silently misbehave if the i32/i64 split or the
/// `* / 1_000_000` (and friends) scaling regressed.
#[rstest]
fn test_execute_datetime_round_trip(mut with_db: db::RootDatabase) {
    let source = r#"
        // TIME (i32 ms) -> LTIME (i64 ns): widening with * 1_000_000
        FUNCTION time_to_ltime : LTIME
        VAR_INPUT t : TIME; END_VAR
            time_to_ltime := t;
        END_FUNCTION

        // LTIME (i64 ns) -> TIME (i32 ms): explicit narrow with / 1_000_000
        FUNCTION ltime_to_time_inline : TIME
        VAR_INPUT lt : LTIME; END_VAR
        VAR result : TIME; END_VAR
            {wasm 'cast' (params lt) (result result)}
            ltime_to_time_inline := result;
        END_FUNCTION

        // DT (i32 secs) -> DATE (i32 days): div by 86400
        FUNCTION dt_to_date_inline : DATE
        VAR_INPUT d : DATE_AND_TIME; END_VAR
        VAR result : DATE; END_VAR
            {wasm 'cast' (params d) (result result)}
            dt_to_date_inline := result;
        END_FUNCTION

        // DT -> TOD (ms-of-day): (secs % 86400) * 1000
        FUNCTION dt_to_tod_inline : TOD
        VAR_INPUT d : DATE_AND_TIME; END_VAR
        VAR result : TOD; END_VAR
            {wasm 'cast' (params d) (result result)}
            dt_to_tod_inline := result;
        END_FUNCTION

        // LDT (i64 ns) -> LTOD (i64 ns-of-day): mod by 86_400_000_000_000
        FUNCTION ldt_to_ltod_inline : LTOD
        VAR_INPUT l : LDATE_AND_TIME; END_VAR
        VAR result : LTOD; END_VAR
            {wasm 'cast' (params l) (result result)}
            ldt_to_ltod_inline := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let engine = crate::tests::codegen::test_engine();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    // 5 ms -> 5_000_000 ns
    let f = instance
        .get_typed_func::<i32, i64>(&mut store, "time_to_ltime")
        .unwrap();
    assert_eq!(f.call(&mut store, 5).unwrap(), 5_000_000);

    // 1500 ns -> 0 ms (truncating). 5_000_000 ns -> 5 ms.
    let f = instance
        .get_typed_func::<i64, i32>(&mut store, "ltime_to_time_inline")
        .unwrap();
    assert_eq!(f.call(&mut store, 1500).unwrap(), 0);
    assert_eq!(f.call(&mut store, 5_000_000).unwrap(), 5);

    // 86_399 secs (just under one day) -> 0 days. 86_400 secs -> 1 day.
    // 1973-01-01 = 1096 days from 1970-01-01 (1970 + 1971 + 1972 leap = 365+365+366).
    let f = instance
        .get_typed_func::<i32, i32>(&mut store, "dt_to_date_inline")
        .unwrap();
    assert_eq!(f.call(&mut store, 86_399).unwrap(), 0);
    assert_eq!(f.call(&mut store, 86_400).unwrap(), 1);
    assert_eq!(f.call(&mut store, 1_096 * 86_400).unwrap(), 1_096);

    // DT 12:34:56 (= 12*3600 + 34*60 + 56 = 45_296 secs into the day)
    // -> TOD 45_296_000 ms-of-day.
    let f = instance
        .get_typed_func::<i32, i32>(&mut store, "dt_to_tod_inline")
        .unwrap();
    let secs_into_day = 12 * 3600 + 34 * 60 + 56;
    assert_eq!(
        f.call(&mut store, secs_into_day).unwrap(),
        secs_into_day * 1_000
    );
    // 1 day + 1 sec since epoch -> 1 sec-of-day -> 1000 ms-of-day.
    assert_eq!(f.call(&mut store, 86_400 + 1).unwrap(), 1_000);

    // LDT 1 day + 500 ns -> LTOD 500 ns.
    let f = instance
        .get_typed_func::<i64, i64>(&mut store, "ldt_to_ltod_inline")
        .unwrap();
    assert_eq!(f.call(&mut store, 86_400_000_000_000 + 500).unwrap(), 500);
    assert_eq!(f.call(&mut store, 86_400_000_000_000 * 2 + 1).unwrap(), 1);
}

/// The containment invariant, exercised through the widening arm itself: a DT
/// at each encoding extreme, implicitly cast to LDT, must produce the exact
/// instant — not merely compile. The `const` assertion beside the bounds in
/// hir's literals.rs guarantees the multiply cannot overflow; this proves the
/// emitted arm (extend + `* 1e9`) computes the value those bounds promise.
#[rstest]
fn test_execute_dt_extremes_widen_to_exact_ldt(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION dt_widens : INT
        VAR
            hi  : DT := DT#2038-01-19-03:14:07;
            lo  : DT := DT#1901-12-13-20:45:52;
            lhi : LDT;
            llo : LDT;
        END_VAR
            lhi := hi;
            llo := lo;
            IF lhi = LDT#2038-01-19-03:14:07 AND llo = LDT#1901-12-13-20:45:52 THEN
                dt_widens := INT#1;
            ELSE
                dt_widens := INT#0;
            END_IF;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "dt_widens", ());
    assert_eq!(result, 1, "both DT extremes must widen to the exact LDT instant");
}

/// Every date/time literal bakes to its documented integer encoding, observed
/// as the raw lane value a typed return carries. One function per type; the
/// i32 lanes first, the i64 lanes after. LDATE had no codegen coverage at all
/// before this.
#[rstest]
fn test_literals_encode_as_documented(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION enc_time : TIME
            enc_time := T#1s500ms;              // 1500 ms
        END_FUNCTION
        FUNCTION enc_time_max : TIME
            enc_time_max := T#24d20h31m23s647ms; // i32::MAX ms
        END_FUNCTION
        FUNCTION enc_date : DATE
            enc_date := D#1970-01-02;           // 1 day
        END_FUNCTION
        FUNCTION enc_date_pre : DATE
            enc_date_pre := D#1969-12-31;       // -1 day
        END_FUNCTION
        FUNCTION enc_tod : TOD
            enc_tod := TOD#00:00:01.5;          // 1500 ms-of-day
        END_FUNCTION
        FUNCTION enc_dt : DT
            enc_dt := DT#1970-01-01-00:01:00;   // 60 s
        END_FUNCTION
        FUNCTION enc_dt_max : DT
            enc_dt_max := DT#2038-01-19-03:14:07; // i32::MAX s
        END_FUNCTION
        FUNCTION enc_ltime : LTIME
            enc_ltime := LT#1s;                 // 1_000_000_000 ns
        END_FUNCTION
        FUNCTION enc_ldate : LDATE
            enc_ldate := LD#1973-01-01;         // 1096 days (365+365+366)
        END_FUNCTION
        FUNCTION enc_ltod : LTOD
            enc_ltod := LTOD#00:00:00.000000042; // 42 ns-of-day
        END_FUNCTION
        FUNCTION enc_ldt : LDT
            enc_ldt := LDT#1970-01-01-00:00:01; // 1_000_000_000 ns
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let engine = crate::tests::codegen::test_engine();
    let module = Module::new(&engine, &wasm).unwrap();
    let mut store = Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let i32_of = |store: &mut Store<()>, name: &str| {
        instance
            .get_typed_func::<(), i32>(&mut *store, name)
            .unwrap()
            .call(&mut *store, ())
            .unwrap()
    };
    let i64_of = |store: &mut Store<()>, name: &str| {
        instance
            .get_typed_func::<(), i64>(&mut *store, name)
            .unwrap()
            .call(&mut *store, ())
            .unwrap()
    };

    assert_eq!(i32_of(&mut store, "enc_time"), 1_500);
    assert_eq!(i32_of(&mut store, "enc_time_max"), i32::MAX);
    assert_eq!(i32_of(&mut store, "enc_date"), 1);
    assert_eq!(i32_of(&mut store, "enc_date_pre"), -1, "pre-epoch DATE is a plain negative day count");
    assert_eq!(i32_of(&mut store, "enc_tod"), 1_500);
    assert_eq!(i32_of(&mut store, "enc_dt"), 60);
    assert_eq!(i32_of(&mut store, "enc_dt_max"), i32::MAX);
    assert_eq!(i64_of(&mut store, "enc_ltime"), 1_000_000_000);
    assert_eq!(i64_of(&mut store, "enc_ldate"), 1_096);
    assert_eq!(i64_of(&mut store, "enc_ltod"), 42);
    assert_eq!(i64_of(&mut store, "enc_ldt"), 1_000_000_000);
}

/// Widening a pre-epoch DATE must SIGN-extend: -1 day stays -1 in the i64
/// lane. A zero-extension bug would produce 4_294_967_295 and no test above
/// catches it, since every other widening case is positive.
#[rstest]
fn test_pre_epoch_date_widens_sign_extended(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION widen_pre : LDATE
        VAR d : DATE := D#1969-12-31; END_VAR
            widen_pre := d;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i64 = super::execute_wasm(&wasm, "widen_pre", ());
    assert_eq!(result, -1);
}

/// Every date/time comparison is SIGNED, matching the signed integer
/// encodings. This was not always so: `is_signed` once admitted only the
/// SInt..LInt family, every date/time compare emitted `LtU`/`GtU`, and
/// `D#1969-12-31` sorted AFTER `D#1970-01-01` — this test found that.
#[rstest]
fn test_comparisons_are_signed_per_encoding(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION probe : INT
        VAR
            d1 : DATE := D#1969-12-31;           d2 : DATE := D#1970-01-01;
            g1 : LDATE := LD#1969-12-31;         g2 : LDATE := LD#1970-01-01;
            t1 : DT := DT#1969-12-31-23:59:59;   t2 : DT := DT#1970-01-01-00:00:00;
            l1 : LDT := LDT#1969-12-31-23:59:59; l2 : LDT := LDT#1970-01-01-00:00:00;
            m1 : TIME := T#-5s;                  m2 : TIME := T#0s;
            n1 : LTIME := LT#-5s;                n2 : LTIME := LT#0s;
            r : INT;
        END_VAR
            IF d1 < d2 THEN r := r + INT#1; END_IF;
            IF g1 < g2 THEN r := r + INT#2; END_IF;
            IF t1 < t2 THEN r := r + INT#4; END_IF;
            IF l1 < l2 THEN r := r + INT#8; END_IF;
            IF m1 < m2 THEN r := r + INT#16; END_IF;
            IF n1 < n2 THEN r := r + INT#32; END_IF;
            probe := r;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let bits: i32 = super::execute_wasm(&wasm, "probe", ());
    assert_eq!(
        bits, 63,
        "every family must order its pre-epoch/negative value below the epoch/zero (bitmask: DATE=1 LDATE=2 DT=4 LDT=8 TIME=16 LTIME=32)"
    );
}

/// TIME arithmetic wraps at the i32 lane, the project's defined overflow
/// behavior: the maximum TIME plus one millisecond is the minimum TIME.
#[rstest]
fn test_time_addition_wraps_at_lane(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION wraps : TIME
        VAR a : TIME := T#24d20h31m23s647ms; b : TIME := T#1ms; END_VAR
            wraps := a + b;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "wraps", ());
    assert_eq!(result, i32::MIN);
}

/// The recorded acceptance in `emit_datetime_cast` (truncating division,
/// pre-epoch dates off by one) has a compound effect it never named: the
/// TOD half of a pre-epoch decomposition `rem`s NEGATIVE — a value outside
/// TOD's declared domain (`TOD_MIN_MS..=TOD_MAX_MS`), sorting below
/// TOD#00:00:00 now that comparisons are signed. This test pins today's
/// behavior so the eventual ruling (floor division, clamp, or keep) flips it
/// DELIBERATELY. DT#1969-12-31-23:59:59 is -1 s; trunc-rem is -1000 ms.
#[rstest]
fn dt_pre_epoch_tod_escapes_its_domain(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION to_tod : TOD
        VAR_INPUT IN : DT; END_VAR
            {wasm 'cast' (params IN) (result to_tod)}
        END_FUNCTION

        FUNCTION raw_ms : TOD
            raw_ms := to_tod(DT#1969-12-31-23:59:59);
        END_FUNCTION

        FUNCTION escapes : INT
        VAR t : TOD; END_VAR
            t := to_tod(DT#1969-12-31-23:59:59);
            IF t < TOD#00:00:00 THEN escapes := INT#1; ELSE escapes := INT#0; END_IF;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let raw: i32 = super::execute_wasm(&wasm, "raw_ms", ());
    let escapes: i32 = super::execute_wasm(&wasm, "escapes", ());
    assert_eq!(raw, -1_000, "trunc-rem of -1 s is -1000 ms-of-day, below TOD_MIN_MS");
    assert_eq!(
        escapes, 1,
        "the escaped TOD sorts below TOD#00:00:00 (signed compare, out of domain)"
    );
}

/// The scaled widenings (`extend + multiply`) on a NEGATIVE value: sign
/// extension and the unit multiply compose, and only DT's extreme was
/// covered. `T#-5s` widened to LTIME must be -5_000_000_000 ns — a
/// zero-extend before the multiply would produce a huge positive instead.
#[rstest]
fn test_negative_time_widens_scaled_and_signed(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION widen_neg : LTIME
        VAR t : TIME := T#-5s; END_VAR
            widen_neg := t;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i64 = super::execute_wasm(&wasm, "widen_neg", ());
    assert_eq!(result, -5_000_000_000);
}

/// TOD's widening arm on a negative value. A legitimate TOD is non-negative,
/// so this only arises through the domain escape pinned above — which is
/// exactly why it needs the same sign-extension guarantee as the others: an
/// escaped -1000 ms TOD widened to LTOD must stay a coherent -1_000_000_000
/// ns, not zero-extend into garbage. Another input to the standing ruling in
/// `emit_datetime_cast`: as long as TOD can hold negative values in practice,
/// every arm consuming a TOD must handle them deliberately.
#[rstest]
fn escaped_tod_widens_sign_extended(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION to_tod : TOD
        VAR_INPUT IN : DT; END_VAR
            {wasm 'cast' (params IN) (result to_tod)}
        END_FUNCTION

        FUNCTION widen_escaped : LTOD
        VAR t : TOD; END_VAR
            t := to_tod(DT#1969-12-31-23:59:59);
            widen_escaped := t;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i64 = super::execute_wasm(&wasm, "widen_escaped", ());
    assert_eq!(
        result, -1_000_000_000,
        "the escaped TOD stays sign-coherent through the scaled widening"
    );
}
    