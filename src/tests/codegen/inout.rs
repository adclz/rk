// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! VAR_IN_OUT at FB call sites. A scalar one is a member of the instance:
//! the call copies the argument in before the body and back after it. A
//! STRING, a reference or an aggregate is bound by reference: a pointer
//! field in the instance, stored once per call and auto-dereferenced in the
//! body. Aliasing, nesting and passthrough, the E0806 variable requirement,
//! and the split between the two models.

use crate::tests::codegen::TestPlc;
use crate::tests::codegen::{compile_to_mir_and_wasm, with_db};
use rstest::*;

/// Decode the string at the start of the retain band: `[len:i32]` + bytes.
fn read_retain_string(plc: &TestPlc) -> String {
    let r = plc.read_retain();
    let len = i32::from_le_bytes(r[0..4].try_into().unwrap()) as usize;
    String::from_utf8_lossy(&r[4..4 + len]).to_string()
}

/// The caller's variable receives what the body wrote: `d(v := x)` with
/// x=21 and `v := v * 2` leaves x = 42, copied back after the body. (A
/// member once copied in and never back, so x silently kept 21.)
#[rstest]
fn fb_inout_basic(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK doubler
            VAR_IN_OUT v : INT; END_VAR
            v := v * 2;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR d : doubler; x : INT := 21; END_VAR
            d(v := x);
            test := x;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 42, "the member is copied back: x becomes 42");
}

/// The positional (non-formal) arg form `d(x)` binds the inout too, and the
/// copy in and back round-trips across two calls (42 -> 84).
#[rstest]
fn fb_inout_positional(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK doubler
            VAR_IN_OUT v : INT; END_VAR
            v := v * 2;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR d : doubler; x : INT := 21; END_VAR
            d(x);
            d(x);
            test := x;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 84, "positional inout round-trips: 21->42->84");
}

/// A VAR_IN_OUT bound to a struct field as the caller l-value: the write must
/// land in `p.n` specifically, leaving the sibling field untouched.
#[rstest]
fn fb_inout_field_target(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Pair : STRUCT n : INT; other : INT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK doubler
            VAR_IN_OUT v : INT; END_VAR
            v := v * 2;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR d : doubler; p : Pair; END_VAR
            p.n := 21;
            p.other := 5;
            d(v := p.n);
            test := p.n + p.other;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 47, "writes p.n=42, leaves p.other=5 -> 47");
}

/// Two scalar VAR_IN_OUTs of a block bound to one variable are two members,
/// each a copy: a write to `a` is not seen through `b` in the body, and the
/// variable keeps what the member declared last copies back. A FUNCTION's
/// two alias it: the test after this one.
#[rstest]
fn two_members_bound_to_one_variable_are_copies(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK aliaser
            VAR_IN_OUT a : INT; b : INT; END_VAR
            VAR_OUTPUT seen : INT; END_VAR
            a := 100;
            seen := b;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR fb : aliaser; x : INT := 7; END_VAR
            fb(a := x, b := x);
            test := fb.seen * 1000 + x;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 7007,
        "`b` copied 7 in before the body wrote `a`, and `b` copied 7 back after `a` copied 100"
    );
}

/// A call that leaves a scalar VAR_IN_OUT out runs the body on the member:
/// its start value first, then what the last call left. A call that binds
/// it copies the argument in and the result back.
#[rstest]
fn an_omitted_member_keeps_its_value(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK acc
            VAR_IN_OUT total : INT := 5; END_VAR
            total := total + 1;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR a : acc; x : INT := 20; END_VAR
            a();
            a();
            a(total := x);
            a();
            test := a.total * 100 + x;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 2221,
        "5 + 1 + 1, then 20 + 1 back into x, then 21 + 1 in the member"
    );
}

/// An enum and a subrange are scalars too: members, copied in and back,
/// with a start value.
#[rstest]
fn an_enum_and_a_subrange_in_out_are_members(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Gear : (Low, Mid, High); END_TYPE

        FUNCTION_BLOCK shifter
            VAR_IN_OUT gear : Gear := Gear#Mid; pct : INT (0..100) := 50; END_VAR
            gear := Gear#High;
            pct := pct + 1;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR s : shifter; g : Gear; p : INT (0..100) := 10; END_VAR
            s();
            s(gear := g, pct := p);
            IF g = Gear#High AND s.gear = Gear#High THEN
                test := p * 100 + s.pct;
            END_IF;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 1111,
        "p is 11 back from the member, which holds 11 too"
    );
}

/// A struct VAR_IN_OUT is passed by reference (one address stored, no value
/// copy of the aggregate). The body mutates fields through the reference and
/// the caller observes it — the old value model silently dropped aggregates.
#[rstest]
fn fb_inout_aggregate(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Vec2 : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK bump
            VAR_IN_OUT v : Vec2; END_VAR
            v.x := v.x + 10;
            v.y := v.y + 20;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR fb : bump; p : Vec2; END_VAR
            p.x := 1;
            p.y := 2;
            fb(v := p);
            test := p.x + p.y;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 33, "struct inout by-ref: (1+10) + (2+20) = 33");
}

/// A nested FB (an instance that is itself a member of the calling FB) bound
/// to an inout arg that is another member of the outer FB: the call site runs
/// through the dynamic `this + offset` FbCall branch, and the copy back
/// lands in the outer's member.
#[rstest]
fn fb_inout_nested(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK doubler
            VAR_IN_OUT v : INT; END_VAR
            v := v * 2;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK outer
            VAR d : doubler; x : INT; END_VAR
            VAR_OUTPUT res : INT; END_VAR
            x := 21;
            d(v := x);
            res := x;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR o : outer; END_VAR
            o();
            test := o.res;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 42, "nested FB inout on an outer member: 21 -> 42");
}

/// Inout PASSTHROUGH: an FB forwards its OWN VAR_IN_OUT param to a nested FB's
/// VAR_IN_OUT. Two members, two copies: the inner copies the outer's member
/// in and back, and the outer copies its own back into the original
/// variable two frames up, so the innermost write reaches it.
#[rstest]
fn fb_inout_passthrough(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK doubler
            VAR_IN_OUT v : INT; END_VAR
            v := v * 2;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK forwarder
            VAR_IN_OUT w : INT; END_VAR
            VAR d : doubler; END_VAR
            d(v := w);
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR f : forwarder; x : INT := 21; END_VAR
            f(w := x);
            test := x;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 42, "forwarded inout mutates the original variable");
}

/// STRING VAR_IN_OUT on an FB: the callee writes through the reference into
/// the caller's buffer.
#[rstest]
fn fb_inout_string(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK renamer
            VAR_IN_OUT s : STRING; END_VAR
            s := 'hi-fb-inout';
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR RETAIN s : STRING; END_VAR
        VAR fb : renamer; END_VAR
            s := 'before';
            fb(s := s);
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    assert_eq!(read_retain_string(&plc), "hi-fb-inout");
}

/// E0806: a VAR_IN_OUT argument must be a variable. A literal or an
/// expression has nowhere to receive the copy back — previously this
/// compiled silently and the callee dereferenced a garbage address.
#[rstest]
fn fb_inout_non_lvalue_rejected(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK doubler
            VAR_IN_OUT v : INT; END_VAR
            v := v * 2;
        END_FUNCTION_BLOCK

        FUNCTION fn2 : INT
            VAR_IN_OUT io : INT; END_VAR
            io := io + 1;
            fn2 := io;
        END_FUNCTION

        FUNCTION test : INT
        VAR d : doubler; x : INT; END_VAR
            d(v := 5);
            test := fn2(io := x + 1);
        END_FUNCTION
    "#;
    let file = super::add_source(&mut with_db, source);
    let diags = hir::check::diagnostics_for_file(&with_db, file);
    let e0806_count = diags
        .iter()
        .filter(|d| crate::tests::utils::diagnostic_code(d) == "E0806")
        .count();
    assert_eq!(
        e0806_count, 2,
        "both the FB literal and the FUNCTION expression inout args must be \
         rejected with E0806, got: {diags:?}"
    );
    assert_eq!(diags.len(), 2, "no other diagnostics expected: {diags:?}");
}

/// FUNCTION VAR_IN_OUT bound to a plain scalar FUNCTION-local: the arg must be
/// forced into linear memory so its address exists. (Regression: the
/// address-taken pass only covered FB calls, so `fn2(x)` emitted an
/// addressless AddrOf -> invalid wasm.)
#[rstest]
fn fn_inout_scalar_local(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION fn2 : INT
        VAR_IN_OUT io : INT; END_VAR
            io := io + 1;
            fn2 := 0;
        END_FUNCTION

        FUNCTION test : INT
        VAR x : INT := 10; END_VAR
            fn2(x);
            fn2(io := x);
            test := x;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 12, "two inout calls increment x: 10 -> 12");
}

/// FUNCTION VAR_IN_OUT with a STRUCT: by-reference, the callee mutates the
/// caller's fields through the pointer.
#[rstest]
fn fn_inout_struct(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Vec2 : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION bump : INT
        VAR_IN_OUT v : Vec2; END_VAR
            v.x := v.x + 10;
            v.y := v.y + 20;
            bump := 0;
        END_FUNCTION

        FUNCTION test : INT
        VAR p : Vec2; END_VAR
            p.x := 1;
            p.y := 2;
            bump(v := p);
            test := p.x + p.y;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 33, "struct inout on a FUNCTION: (1+10) + (2+20)");
}

/// FUNCTION VAR_IN_OUT with an ARRAY: element writes through the reference.
#[rstest]
fn fn_inout_array(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION dbl : INT
        VAR_IN_OUT arr : ARRAY[0..1] OF INT; END_VAR
            arr[0] := arr[0] * 2;
            arr[1] := arr[1] * 2;
            dbl := 0;
        END_FUNCTION

        FUNCTION test : INT
        VAR a : ARRAY[0..1] OF INT; END_VAR
            a[0] := 3;
            a[1] := 4;
            dbl(arr := a);
            test := a[0] + a[1];
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 14, "array inout on a FUNCTION: 6 + 8");
}

/// E0807: binding a VAR_IN_OUT with `=>` is rejected — a member would run
/// the body on what it holds and copy nothing in, and a reference would be
/// left unbound (the body would deref address 0).
#[rstest]
fn fb_inout_arrow_binding_rejected(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK doubler
            VAR_IN_OUT v : INT; END_VAR
            v := v * 2;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR d : doubler; x : INT := 21; END_VAR
            d(v => x);
            test := x;
        END_FUNCTION
    "#;
    let file = super::add_source(&mut with_db, source);
    let diags = hir::check::diagnostics_for_file(&with_db, file);
    let e0807 = diags
        .iter()
        .filter(|d| crate::tests::utils::diagnostic_code(d) == "E0807")
        .count();
    assert_eq!(e0807, 1, "inout bound via => must be rejected: {diags:?}");
}

/// A METHOD accessing the FB's scalar VAR_IN_OUT, as bare `v` and as
/// `THIS.v`: it reads and writes the member. The call of the block copies
/// the argument in and back; a method call binds nothing, so what the
/// method writes stays in the member until a call of the block copies it
/// back.
#[rstest]
fn fb_inout_method_this_access(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK counter
            VAR_IN_OUT v : INT; END_VAR

            METHOD PUBLIC bump : INT
                v := v + 1;
                THIS.v := THIS.v + 10;
                bump := THIS.v;
            END_METHOD

            v := v + 1;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR d : counter; x : INT := 5; END_VAR
            d(v := x);
            d.bump();
            test := x * 100 + d.v;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 617,
        "the call copies 6 back into x; the method's +1 and +10 stay in the member"
    );
}

/// The by_ref discriminator: an FB with BOTH a REF_TO input (a Pointer field
/// the user derefs explicitly with `^`) and a REF_TO VAR_IN_OUT (a Pointer
/// field to the caller's reference, auto-dereferenced once, then `^` for
/// the INT). Only `MirStructField.by_ref` tells them apart — a regression
/// here would auto-deref REF members or stop auto-dereffing inouts. The
/// body also re-points the in-out, which re-points the caller's reference.
#[rstest]
fn fb_ref_to_member_not_auto_dereffed(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE PInt : REF_TO INT; END_TYPE

        FUNCTION_BLOCK mixer
            VAR_IN_OUT io : PInt; END_VAR
            VAR_INPUT target : REF_TO INT; END_VAR
            io^ := io^ + 1;
            target^ := target^ + 100;
            io := target;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR m : mixer; a : INT := 1; b : INT := 2; r : PInt; END_VAR
            r := REF(a);
            m(io := r, target := REF(b));
            r^ := 7;
            test := a * 1000 + b;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 2007,
        "io auto-deref then ^: a=2; REF explicit deref: b=102; r re-pointed at b, which the 7 overwrites"
    );
}

/// A VAR_IN_OUT argument inside a subscript, read or written, makes its
/// variable addressable like one anywhere else: the address-taken scan
/// skipped subscripts and assignment targets, and codegen panicked.
#[rstest]
fn in_out_argument_in_a_subscript(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION next : INT
        VAR_IN_OUT calls : INT; END_VAR
            next := calls;
            calls := calls + 1;
        END_FUNCTION

        FUNCTION test : INT
        VAR a : ARRAY[0..3] OF INT; n : INT; x : INT; END_VAR
            a[1] := 7;
            x := a[next(calls := n) + 1];
            a[next(calls := n)] := 5;
            test := x * 100 + a[1] * 10 + n;
        END_FUNCTION
    "#;
    let result: i32 = crate::tests::codegen::run(&mut with_db, source, "test", ());
    assert_eq!(result, 752, "read a[1] = 7, wrote a[1] := 5, two calls");
}

/// The copy back lands when the call ends: a body that raises writes
/// nothing back, and the variable keeps what it had.
#[rstest]
fn a_member_is_not_written_back_after_a_raise(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK thrower
            VAR_IN_OUT v : INT; END_VAR
            v := v + 1;
            __RAISE('boom');
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR t : thrower; x : INT := 1; e : STRING; END_VAR
            __TRY
                t(v := x);
            __CATCH(e)
                x := x * 10;
            __END_TRY;
            test := x * 100 + t.v;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 1002,
        "x stays 1 and is multiplied in the catch; the member holds 2"
    );
}

/// The split, on record: two VAR_IN_OUTs of a FUNCTION bound to one
/// variable alias it, so `b` sees the 20 written through `a`, where a
/// block's two members are copies.
#[rstest]
fn two_in_outs_of_a_function_bound_to_one_variable_alias_it(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION aliaser : INT
            VAR_IN_OUT a : INT; b : INT; END_VAR
            a := 20;
            aliaser := b;
        END_FUNCTION

        FUNCTION test : INT
        VAR x : INT := 7; seen : INT; END_VAR
            seen := aliaser(a := x, b := x);
            test := seen * 1000 + x;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 20020, "`b` is `x`, which `a` wrote 20 into");
}

/// Both models in one call: `s` is a reference to `box`, `io` a copy of
/// `box.v`. The body's write to `io` is not seen through `s`, and `io`'s
/// copy back, after the body, lands over what `s` wrote.
#[rstest]
fn a_member_and_a_reference_in_one_call(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Box : STRUCT v : INT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK bump
            VAR_IN_OUT s : Box; io : INT; END_VAR
            VAR_OUTPUT seen : INT; END_VAR
            io := io + 1;
            seen := s.v;
            s.v := s.v + 10;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR b : bump; box : Box := (v := 1); END_VAR
            b(s := box, io := box.v);
            test := b.seen * 100 + box.v;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 102,
        "seen is 1, and io's copy back puts 2 over the 11 that s wrote"
    );
}

/// The copy back lands where the call bound it: `v` was bound to `arr[0]`,
/// and stays bound to it after `k`'s copy back has put 1 into `i`.
#[rstest]
fn the_copy_back_lands_where_the_call_bound_it(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK pick
            VAR_IN_OUT k : INT; v : INT; END_VAR
            k := 1;
            v := 99;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR p : pick; i : INT; arr : ARRAY[0..1] OF INT := [10, 20]; END_VAR
            p(k := i, v := arr[i]);
            test := arr[0] * 100 + arr[1];
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 9920,
        "arr[0] took the 99; 1099 would mean arr[i] was read again after i became 1"
    );
}

/// A RETURN ends the call as the last statement does, and the copy back
/// follows it.
#[rstest]
fn an_early_return_copies_back(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK early
            VAR_IN_OUT v : INT; END_VAR
            v := v + 1;
            IF v > 0 THEN
                RETURN;
            END_IF;
            v := v + 100;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR e : early; x : INT := 1; END_VAR
            e(v := x);
            test := x;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 2,
        "the +1 came back; the +100 after the RETURN did not run"
    );
}

/// `REF()` of a member points at the member, not at the caller's variable:
/// a write through it changes what the instance holds. The caller's
/// variable never sees it, since the next call that binds the member
/// copies the variable in first; only a write during a call, before its
/// copy back, reaches the variable.
#[rstest]
fn a_reference_to_a_member_points_at_the_member(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK keeper
            VAR_IN_OUT io : INT; END_VAR

            METHOD PUBLIC poke
            VAR p : REF_TO INT; END_VAR
                p := REF(io);
                p^ := 9;
            END_METHOD

            io := io + 1;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR k : keeper; x : INT := 4; END_VAR
            k(io := x);
            k.poke();
            test := x * 100 + k.io;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 509,
        "x took 5 at the copy back; the 9 went into the member"
    );
}

/// A bit of a bit string, `w.%X3`, bound to a member: the copy brings the
/// bit in, and the copy back changes that bit of `w` and no other. The word
/// is reset between the calls, so a copy in that skipped the bit (the
/// member's TRUE from the first call would be flipped back) fails the
/// second. A reference binds no bit (E1423 on a FUNCTION's VAR_IN_OUT).
#[rstest]
fn a_bit_of_a_word_bound_to_a_member(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK flip
            VAR_IN_OUT v : BOOL; END_VAR
            v := NOT v;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR f : flip; w : WORD := 16#00F0; END_VAR
            f(v := w.%X3);
            IF w = 16#00F8 THEN test := 1; END_IF;
            w := 16#00F0;
            f(v := w.%X3);
            IF w = 16#00F8 THEN test := test + 10; END_IF;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 11,
        "16#F8 after each call: bit 3 only, copied in from the reset word"
    );
}

/// A member of eight bytes, an LINT, is copied whole, in and back.
#[rstest]
fn an_eight_byte_member_is_copied_whole(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK bump
            VAR_IN_OUT v : LINT; END_VAR
            v := v + 1;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR b : bump; x : LINT := 16#1_0000_0001; END_VAR
            b(v := x);
            IF x = 16#1_0000_0002 AND b.v = 16#1_0000_0002 THEN
                test := 1;
            END_IF;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 1, "the high word came back with the low one");
}

/// The next call that binds the member copies the caller's variable in
/// first, so a write through `REF()` of the member between calls never
/// reaches that variable: the 9 is gone when `x` is copied in again.
#[rstest]
fn a_write_to_the_member_between_calls_is_copied_over(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK keeper
            VAR_IN_OUT io : INT; END_VAR

            METHOD PUBLIC poke
            VAR p : REF_TO INT; END_VAR
                p := REF(io);
                p^ := 9;
            END_METHOD

            io := io + 1;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR k : keeper; x : INT := 4; END_VAR
            k(io := x);
            k.poke();
            k(io := x);
            test := x * 100 + k.io;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 606, "x came in at 5 over the 9, and left at 6");
}

/// A write through `REF()` of the member during a call, before its copy
/// back, does reach the caller's variable.
#[rstest]
fn a_write_through_a_reference_during_the_call_comes_back(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK keeper
            VAR_IN_OUT io : INT; END_VAR

            METHOD PUBLIC poke
            VAR p : REF_TO INT; END_VAR
                p := REF(io);
                p^ := 9;
            END_METHOD

            io := io + 1;
            poke();
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR k : keeper; x : INT := 4; END_VAR
            k(io := x);
            test := x * 100 + k.io;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 909, "the 9 written during the call was copied back");
}

/// An inherited scalar VAR_IN_OUT is a member of the derived block, bound
/// and copied back by a call of it.
#[rstest]
fn an_inherited_member_is_copied_back(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Base
            VAR_IN_OUT io : INT; END_VAR
            io := io + 1;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Derived EXTENDS Base
            io := io + 10;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR d : Derived; x : INT := 1; END_VAR
            d(io := x);
            test := x * 100 + d.io;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 1111,
        "the derived body wrote the inherited member, and x got it back"
    );
}

/// `SUPER()` runs the base body on the same instance, with no copy of its
/// own: the derived body's write and the base body's both reach the
/// caller's variable at the one copy back.
#[rstest]
fn super_runs_the_base_body_on_the_same_member(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Base
            VAR_IN_OUT io : INT; END_VAR
            io := io + 1;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Derived EXTENDS Base
            io := io + 10;
            SUPER();
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR d : Derived; x : INT := 1; END_VAR
            d(io := x);
            test := x * 100 + d.io;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 1212,
        "1, plus 10 in the derived body, plus 1 in the base body"
    );
}
