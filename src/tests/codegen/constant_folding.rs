// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A constant expression folds to what the program computes: each operation
//! at its type, wrapping at that type's width, wherever it is written — an
//! initializer, a CASE label, a FOR step, a bound.

use crate::tests::codegen::with_db;
use rstest::*;

/// `200 * 200` multiplies two INTs, so it is -25536 in an initializer as in
/// an assignment. The initializer used to fold it in 64 bits to 40000, and
/// the parenthesized one did not fold at all, so the three disagreed.
#[rstest]
fn an_initializer_folds_as_the_assignment_computes(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR
            a : DINT := 200 * 200;
            b : DINT := (200 * 200);
            c : DINT;
            d : DINT := DINT#200 * 200;
        END_VAR
            c := 200 * 200;
            IF a = c AND b = c AND d = 40000 THEN
                run := a;
            END_IF;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "run", ());
    assert_eq!(
        result, -25536,
        "INT arithmetic wraps in all three; DINT#200 * 200 is 40000"
    );
}

/// A label computes at its type like the selector's value: `K + 1` on a
/// SINT K = 127 is -128, and `K * 3` is 125. They were 128 and 381, which no
/// SINT selector holds, so the branches never ran.
#[rstest]
fn a_case_label_wraps_like_the_value_it_matches(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION pick : DINT
        VAR_INPUT s : SINT; END_VAR
        VAR CONSTANT K : SINT := 127; END_VAR
            CASE s OF
                K + 1: pick := 1;
                K * 3: pick := 2;
            ELSE
                pick := 0;
            END_CASE;
        END_FUNCTION

        FUNCTION run : DINT
        VAR CONSTANT K : SINT := 127; END_VAR
        VAR s : SINT; END_VAR
            s := K + 1;
            run := pick(s := s) * 10;
            s := K * 3;
            run := run + pick(s := s);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "run", ());
    assert_eq!(result, 12, "K + 1 picks 1, K * 3 picks 2");
}

/// A ULINT label above `i64::MAX` is a constant like any other.
#[rstest]
fn a_ulint_label_above_i64_max_matches(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION pick : DINT
        VAR_INPUT v : ULINT; END_VAR
            CASE v OF
                18446744073709551615: pick := 1;
                9223372036854775808: pick := 2;
            ELSE
                pick := 0;
            END_CASE;
        END_FUNCTION

        FUNCTION run : DINT
            run := pick(v := ULINT#18446744073709551615) * 10
                + pick(v := ULINT#9223372036854775808);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "run", ());
    assert_eq!(result, 12, "the maximum picks 1, 2**63 picks 2");
}

/// Parentheses and a sign fold around a CONSTANT, not only around a literal:
/// in a bound, a subrange, a label and a step.
#[rstest]
fn parentheses_and_a_sign_fold_around_a_constant(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR CONSTANT K : INT := 5; END_VAR
        VAR
            a : ARRAY[-K..(K + 1) * 2] OF INT;
            s : INT(-K..K);
            x : INT := 12;
            y : INT := -5;
            i : INT;
        END_VAR
            a[-5] := 1;
            a[12] := 2;
            s := y;
            CASE x OF (K + 1) * 2: run := run + 10; END_CASE;
            CASE y OF -K: run := run + 100; END_CASE;
            FOR i := 10 TO 0 BY -K DO
                run := run + 1000;
            END_FOR;
            run := run + a[-5] + a[12] + s;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "run", ());
    assert_eq!(
        result, 3108,
        "both labels match, the step runs 10, 5, 0, and a[-5] + a[12] + s is -2"
    );
}

/// A CONSTANT defined from CONSTANTs is a label and a step: its initializer
/// folds where it is declared, not in the body that names it.
#[rstest]
fn a_constant_from_constants_is_a_label_and_a_step(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION pick : DINT
        VAR_INPUT x : INT; END_VAR
        VAR CONSTANT A : INT := 2; B : INT := A * 2; END_VAR
            CASE x OF
                A: pick := 1;
                B: pick := 2;
            ELSE
                pick := 0;
            END_CASE;
        END_FUNCTION

        FUNCTION run : DINT
        VAR CONSTANT ONE : INT := 1; TWO : INT := ONE + ONE; END_VAR
        VAR i : INT; END_VAR
            FOR i := 0 TO 6 BY TWO DO
                run := run + i;
            END_FOR;
            run := run + pick(x := 4) * 100;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "run", ());
    assert_eq!(result, 212, "B is 4, and the step 2 sums 0 + 2 + 4 + 6");
}

/// A derived block's inherited CONSTANT is constant there too: in a bound, a
/// member's initializer, and another CONSTANT used as a label.
#[rstest]
fn an_inherited_constant_folds_in_the_derived_block(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Base
        VAR CONSTANT N : INT := 3; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Derived EXTENDS Base
        VAR CONSTANT K : INT := N + 1; END_VAR
        VAR
            arr : ARRAY[0..N] OF INT;
            x : INT := N;
            y : INT := N + 1;
        END_VAR
        VAR_INPUT sel : INT; END_VAR
        VAR_OUTPUT out : DINT; END_VAR
            arr[N] := 40;
            CASE sel OF
                K: out := arr[3] + x + y;
            ELSE
                out := 0;
            END_CASE;
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR d : Derived; END_VAR
            d(sel := 4);
            run := d.out;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "run", ());
    assert_eq!(result, 47, "K is 4, and arr[3] + x + y is 40 + 3 + 4");
}

/// `AND`, `OR`, `XOR` and `NOT` fold bit by bit at the type their operands meet
/// at, to what the program computes, wherever a constant is needed. Each row is a
/// program whose `test` holds. They did not fold: each was refused.
#[rstest]
#[case::a_constant(
    "FUNCTION test : BOOL VAR CONSTANT k : WORD := WORD#16#FF00 AND 16#0FF0; END_VAR test := k = 16#0F00; END_FUNCTION"
)]
#[case::every_operator(
    "FUNCTION test : BOOL
     VAR CONSTANT m : BYTE := 16#F0; k : BYTE := (m OR 16#0F) XOR NOT m; END_VAR
     VAR x : BYTE := 16#F0; END_VAR
         x := (x OR 16#0F) XOR NOT x;
         test := k = x AND k = 16#F0;
     END_FUNCTION"
)]
#[case::at_a_signed_type(
    "FUNCTION test : BOOL
     VAR CONSTANT k : INT := -1 XOR 16#00FF; END_VAR
     VAR x : INT := -1; END_VAR
         x := x XOR 16#00FF;
         test := k = x AND k = -256;
     END_FUNCTION"
)]
#[case::at_64_bits(
    "FUNCTION test : BOOL
     VAR CONSTANT k : LWORD := LWORD#16#FFFF_FFFF_0000_0000 OR 16#FFFF; END_VAR
     VAR x : LWORD := LWORD#16#FFFF_FFFF_0000_0000; END_VAR
         x := x OR 16#FFFF;
         test := k = x AND k = LWORD#16#FFFF_FFFF_0000_FFFF;
     END_FUNCTION"
)]
#[case::a_bound(
    "FUNCTION test : BOOL VAR CONSTANT k : INT := 3 OR 4; END_VAR VAR a : ARRAY[0..k] OF INT; END_VAR
         a[7] := 1; test := a[7] = 1;
     END_FUNCTION"
)]
#[case::a_label(
    "FUNCTION test : BOOL VAR x : INT := 7; END_VAR CASE x OF 3 OR 4: test := TRUE; END_CASE; END_FUNCTION"
)]
#[case::a_label_flipped_at_the_selectors_type(
    "FUNCTION test : BOOL VAR w : WORD := 15; END_VAR CASE w OF NOT 16#FFF0: test := TRUE; END_CASE; END_FUNCTION"
)]
#[case::a_label_at_64_bits(
    "FUNCTION test : BOOL VAR w : LWORD := LWORD#16#FFFF_FFFF_0000_FFFF; END_VAR
         CASE w OF LWORD#16#FFFF_FFFF_0000_0000 OR 16#FFFF: test := TRUE; END_CASE;
     END_FUNCTION"
)]
#[case::a_strings_length(
    "FUNCTION test : BOOL VAR s : STRING[16#FF AND 16#0F]; long : STRING := 'abcdefghijklmnopqrs'; END_VAR
         s := long; test := s = 'abcdefghijklmno';
     END_FUNCTION"
)]
#[case::a_subranges_bounds(
    "FUNCTION test : BOOL VAR r : INT (0..(16#F0 OR 16#0F)); END_VAR r := 255; test := r = 255; END_FUNCTION"
)]
#[case::a_step(
    "FUNCTION test : BOOL VAR i, n : INT; END_VAR
         FOR i := 0 TO 9 BY 7 AND 3 DO n := n + 1; END_FOR;
         test := n = 4;
     END_FUNCTION"
)]
#[case::an_initial_index(
    "FUNCTION test : BOOL VAR a : ARRAY[0..9] OF INT := [[5 XOR 3] := 7]; END_VAR test := a[6] = 7; END_FUNCTION"
)]
#[case::a_members_initial_value(
    "FUNCTION_BLOCK Fb
     VAR CONSTANT m : WORD := 16#00F0; END_VAR
     VAR x : WORD := m XOR 16#00FF; END_VAR
     END_FUNCTION_BLOCK
     FUNCTION test : BOOL VAR f : Fb; END_VAR test := f.x = 16#000F; END_FUNCTION"
)]
#[case::a_fields_initial_value(
    "TYPE Pt : STRUCT x : DWORD := DWORD#16#FF00 OR 16#00FF; END_STRUCT; END_TYPE
     FUNCTION test : BOOL VAR p : Pt; END_VAR test := p.x = 16#FFFF; END_FUNCTION"
)]
#[case::a_types_initial_value(
    "TYPE Mask : WORD := WORD#16#FF00 AND 16#0FF0; END_TYPE
     FUNCTION test : BOOL VAR m : Mask; END_VAR test := m = 16#0F00; END_FUNCTION"
)]
#[case::an_omitted_inputs_value(
    "FUNCTION f : WORD VAR_INPUT w : WORD := WORD#16#FF00 OR 16#00FF; END_VAR f := w; END_FUNCTION
     FUNCTION test : BOOL test := f() = 16#FFFF; END_FUNCTION"
)]
fn bit_operations_are_constants(mut with_db: db::RootDatabase, #[case] program: &str) {
    let result: i32 = super::run(&mut with_db, program, "test", ());
    assert_eq!(result, 1, "{program}");
}

/// A CONSTANT folds once however often it is named. Each of these names the
/// two before it: folded again at each name, 26 of them took 15 s to check, and
/// these 60 would not have finished.
#[test]
fn a_constant_named_twice_folds_once() {
    let chain: String = (2..=60)
        .map(|k| format!("f{k} : LINT := f{} + f{};\n", k - 1, k - 2))
        .collect();
    let source = format!(
        "FUNCTION test : BOOL
         VAR CONSTANT f0 : LINT := 1; f1 : LINT := 2; {chain} END_VAR
         VAR a : ARRAY[0..(f60 MOD 1000)] OF LINT; END_VAR
             a[881] := f60;
             test := a[f60 MOD 1000] = 4052739537881;
         END_FUNCTION"
    );
    let result: i32 = crate::tests::utils::within(std::time::Duration::from_secs(30), move || {
        super::run(&mut db::RootDatabase::default(), &source, "test", ())
    });
    assert_eq!(result, 1);
}

/// A REAL CONSTANT computed from another is an initial value `__init` stores,
/// each CONSTANT in it its value: a function block's member's and a global's.
/// It stopped the compiler: the value was taken for a chain of CONSTANTs, and
/// the chain's end, `k0 * 2.0`, lowered as a read of `k0`.
#[rstest]
fn a_real_constant_from_another_is_an_initial_value(mut with_db: db::RootDatabase) {
    let source = r#"
        CONFIGURATION Cfg
        VAR_GLOBAL CONSTANT g0 : REAL := 1.5; g1 : REAL := g0 * 2.0; END_VAR
        VAR_GLOBAL g : REAL := g1; END_VAR
        END_CONFIGURATION

        FUNCTION_BLOCK Fb
        VAR CONSTANT k0 : REAL := 1.5; k1 : REAL := k0 * 2.0; END_VAR
        VAR m : REAL := k1; END_VAR
        END_FUNCTION_BLOCK

        {test}
        FUNCTION a_members
        VAR f : Fb; END_VAR
            IF f.m <> 3.0 THEN
                __RAISE('the member is not 3.0');
            END_IF;
        END_FUNCTION

        {test}
        FUNCTION a_globals
        VAR_EXTERNAL g : REAL; END_VAR
            IF g <> 3.0 THEN
                __RAISE('the global is not 3.0');
            END_IF;
        END_FUNCTION
    "#;
    let build = super::compile_tests(&mut with_db, source);
    let results = super::run_tests(&build.component, None).expect("run tests");
    assert_eq!(results.len(), 2);
    for result in &results {
        assert!(result.passed(), "{}: {:?}", result.name, result.reason);
    }
}

/// `AND`, `&`, `OR`, `XOR` and `NOT` over BOOL literals and CONSTANTs are an
/// initial value `__init` stores, each CONSTANT in it its value: a TYPE's
/// fields, a function block's members, a global, and an omitted input's
/// default. They were refused, `NOT TRUE` aside.
#[rstest]
fn bool_logic_over_constants_is_an_initial_value(mut with_db: db::RootDatabase) {
    let source = r#"
        CONFIGURATION Cfg
        VAR_GLOBAL CONSTANT on : BOOL := TRUE; off : BOOL := NOT on; END_VAR
        VAR_GLOBAL g : BOOL := on AND NOT off; END_VAR
        END_CONFIGURATION

        TYPE Flags : STRUCT
            a : BOOL := TRUE AND FALSE;
            b : BOOL := TRUE XOR FALSE;
            c : BOOL := (FALSE OR TRUE) & NOT FALSE;
        END_STRUCT; END_TYPE

        FUNCTION_BLOCK Fb
        VAR CONSTANT k0 : BOOL := TRUE; k1 : BOOL := k0 AND TRUE; END_VAR
        VAR m : BOOL := k1 AND NOT FALSE; n : BOOL := NOT (k1 XOR k0); END_VAR
        END_FUNCTION_BLOCK

        FUNCTION pick : BOOL
        VAR_INPUT x : BOOL := TRUE AND NOT FALSE; END_VAR
            pick := x;
        END_FUNCTION

        {test}
        FUNCTION a_types_fields
        VAR f : Flags; END_VAR
            IF f.a OR NOT f.b OR NOT f.c THEN
                __RAISE('the fields are not FALSE, TRUE, TRUE');
            END_IF;
        END_FUNCTION

        {test}
        FUNCTION a_members
        VAR f : Fb; END_VAR
            IF NOT f.m OR NOT f.n THEN
                __RAISE('the members are not TRUE, TRUE');
            END_IF;
        END_FUNCTION

        {test}
        FUNCTION a_globals
        VAR_EXTERNAL g : BOOL; END_VAR
            IF NOT g THEN
                __RAISE('the global is not TRUE');
            END_IF;
        END_FUNCTION

        {test}
        FUNCTION an_inputs_default
            IF NOT pick() THEN
                __RAISE('the default is not TRUE');
            END_IF;
        END_FUNCTION
    "#;
    let build = super::compile_tests(&mut with_db, source);
    let results = super::run_tests(&build.component, None).expect("run tests");
    assert_eq!(results.len(), 4);
    for result in &results {
        assert!(result.passed(), "{}: {:?}", result.name, result.reason);
    }
}

/// A REAL CONSTANT that an initial value names is lowered once in it, however
/// often the value reaches it. Each of these names the two before it: lowered
/// again at each name, 16 of them made a module of 26 KB, and these 60 would
/// not have finished. The test computes the same chain at run time.
#[test]
fn a_real_constant_named_twice_is_lowered_once() {
    let chain: String = (2..=60)
        .map(|k| format!("f{k} : REAL := f{} + f{};\n", k - 1, k - 2))
        .collect();
    let source = format!(
        "CONFIGURATION Cfg
         VAR_GLOBAL CONSTANT f0 : REAL := 0.5; f1 : REAL := 1.5; {chain} END_VAR
         VAR_GLOBAL g : REAL := f60; END_VAR
         END_CONFIGURATION

         FUNCTION_BLOCK Fb
         VAR_EXTERNAL CONSTANT f60 : REAL; END_VAR
         VAR m : REAL := f60; END_VAR
         END_FUNCTION_BLOCK

         {{test}}
         FUNCTION the_chain
         VAR_EXTERNAL g : REAL; END_VAR
         VAR f : Fb; a : REAL := 0.5; b : REAL := 1.5; c : REAL; i : INT; END_VAR
             FOR i := 2 TO 60 DO
                 c := b + a;
                 a := b;
                 b := c;
             END_FOR;
             IF g <> b THEN
                 __RAISE('the global is not the chain');
             END_IF;
             IF f.m <> b THEN
                 __RAISE('the member is not the chain');
             END_IF;
         END_FUNCTION"
    );
    let results = crate::tests::utils::within(std::time::Duration::from_secs(30), move || {
        let build = super::compile_tests(&mut db::RootDatabase::default(), &source);
        super::run_tests(&build.component, None).expect("run tests")
    });
    assert_eq!(results.len(), 1);
    assert!(results[0].passed(), "{:?}", results[0].reason);
}

/// A CONSTANT lowered once is computed where its value is first reached, in
/// the function being built. An omitted input's default is lowered where the
/// call is: inside a test, inside a loop, it still is the default.
#[rstest]
fn a_constant_lowered_once_in_a_default(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION scaled : REAL
        VAR_INPUT x : REAL := k3; END_VAR
        VAR CONSTANT
            k0 : REAL := 0.5; k1 : REAL := 1.5; k2 : REAL := k1 + k0; k3 : REAL := k2 + k1;
        END_VAR
            scaled := x;
        END_FUNCTION

        {test}
        FUNCTION in_a_loop
        VAR i : INT; n : INT; END_VAR
            FOR i := 1 TO 3 DO
                IF scaled() = 3.5 THEN
                    n := n + 1;
                END_IF;
            END_FOR;
            IF n <> 3 THEN
                __RAISE('the default is not 3.5');
            END_IF;
        END_FUNCTION
    "#;
    let build = super::compile_tests(&mut with_db, source);
    let results = super::run_tests(&build.component, None).expect("run tests");
    assert_eq!(results.len(), 1);
    assert!(results[0].passed(), "{:?}", results[0].reason);
}
