//! FUNCTION aggregate (struct/array) `VAR_INPUT` args: copy semantics, like
//! FUNCTION_BLOCK inputs. The caller copies the arg into a scratch local at
//! call entry (`MirExpr::CopyIntoScratch`) and the callee receives a pointer
//! to that snapshot — so callee-side writes and later caller-side mutations
//! never leak across the call boundary. (Regression: aggregates previously
//! lowered as a bogus 1-slot "value" and the module failed wasm validation.)

use crate::tests::codegen::with_db;
use rstest::*;

/// The audit probe: a STRUCT passed by value into a FUNCTION.
#[rstest]
fn fn_struct_input(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION take_pt : INT
        VAR_INPUT p : Point; END_VAR
            take_pt := p.x + p.y;
        END_FUNCTION

        FUNCTION test : INT
        VAR s : Point; END_VAR
            s.x := 10;
            s.y := 20;
            test := take_pt(p := s);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 30, "struct input by value: 10 + 20");
}

/// An ARRAY passed by value into a FUNCTION.
#[rstest]
fn fn_array_input(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION sum4 : INT
        VAR_INPUT a : ARRAY[0..3] OF INT; END_VAR
            sum4 := a[0] + a[1] + a[2] + a[3];
        END_FUNCTION

        FUNCTION test : INT
        VAR arr : ARRAY[0..3] OF INT; END_VAR
            arr[0] := 1;
            arr[1] := 2;
            arr[2] := 3;
            arr[3] := 4;
            test := sum4(a := arr);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 10, "array input by value: 1+2+3+4");
}

/// Value semantics: the callee writes to its input (L0113 lint, legal) — the
/// mutation lands in the call-entry snapshot, never in the caller's struct.
#[rstest]
fn fn_struct_input_callee_write_invisible(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION clobber : INT
        VAR_INPUT p : Point; END_VAR
            p.x := 999;
            clobber := p.x;
        END_FUNCTION

        FUNCTION test : INT
        VAR s : Point; END_VAR
            s.x := 10;
            s.y := 20;
            clobber(p := s);
            test := s.x + s.y;
        END_FUNCTION
    "#;
    // Unchecked compile: writing to a VAR_INPUT raises the L0113 lint.
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 30,
        "callee wrote its snapshot, caller's s unchanged"
    );
}

/// Snapshot semantics under aliasing: the SAME struct is bound to a VAR_INPUT
/// and a VAR_IN_OUT. The body mutates through the inout reference first, then
/// reads the input — which must still show the call-entry values (the copy
/// was taken before the call, like an FB's copy-in).
#[rstest]
fn fn_struct_input_snapshot_aliasing(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION probe : INT
        VAR_INPUT snap : Point; END_VAR
        VAR_IN_OUT live : Point; END_VAR
            live.x := 100;
            probe := snap.x;
        END_FUNCTION

        FUNCTION test : INT
        VAR s : Point; END_VAR
            s.x := 7;
            test := probe(snap := s, live := s);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 7,
        "input is a call-entry snapshot: sees 7, not the inout's 100"
    );
}

/// Forwarding: a function passes its own aggregate input on to another
/// function — each hop takes a fresh snapshot.
#[rstest]
fn fn_struct_input_forwarding(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION inner_sum : INT
        VAR_INPUT q : Point; END_VAR
            inner_sum := q.x + q.y;
        END_FUNCTION

        FUNCTION outer_fwd : INT
        VAR_INPUT p : Point; END_VAR
            outer_fwd := inner_sum(q := p);
        END_FUNCTION

        FUNCTION test : INT
        VAR s : Point; END_VAR
            s.x := 4;
            s.y := 5;
            test := outer_fwd(p := s);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 9, "forwarded struct input: 4 + 5");
}

/// Aggregate input combined with a scalar input and a discarded output in the
/// same call — arg positions must all line up.
#[rstest]
fn fn_mixed_aggregate_scalar_discard(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION mixed : INT
        VAR_INPUT p : Point; k : INT; END_VAR
        VAR_OUTPUT dbg : INT; END_VAR
            dbg := p.x;
            mixed := p.x * k + p.y;
        END_FUNCTION

        FUNCTION test : INT
        VAR s : Point; END_VAR
            s.x := 3;
            s.y := 1;
            test := mixed(p := s, k := 10);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 31, "3*10 + 1, dbg discarded");
}

/// Aggregate input on a call made from inside an FB body — the scratch lands
/// in the `$__body__` function's locals.
#[rstest]
fn fn_struct_input_from_fb_body(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION take_pt : INT
        VAR_INPUT p : Point; END_VAR
            take_pt := p.x + p.y;
        END_FUNCTION

        FUNCTION_BLOCK caller
        VAR pt : Point; END_VAR
        VAR_OUTPUT res : INT; END_VAR
            pt.x := 8;
            pt.y := 9;
            res := take_pt(p := pt);
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR c : caller; END_VAR
            c();
            test := c.res;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 17, "struct input from an FB body: 8 + 9");
}

/// An FB instance passed as a VAR_INPUT is copied like a STRUCT: the callee
/// reads the caller's members and its writes stay its own. It used to arrive
/// as a garbage value, read as an address.
#[rstest]
fn fn_fb_instance_input(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR_OUTPUT n : INT; m : INT; END_VAR
            n := n + 1;
            m := m + 10;
        END_FUNCTION_BLOCK

        FUNCTION read_m : INT
        VAR_INPUT c : Counter; END_VAR
            read_m := c.m;
        END_FUNCTION

        FUNCTION test : INT
        VAR k : Counter; END_VAR
            k();
            k();
            test := read_m(k) * 100 + k.n;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 2002, "the callee read m = 20");
}

/// A CLASS instance, the same way.
#[rstest]
fn fn_class_instance_input(mut with_db: db::RootDatabase) {
    let source = r#"
        CLASS Box
        VAR PUBLIC v : INT; w : INT; END_VAR
        END_CLASS

        FUNCTION read_w : INT
        VAR_INPUT b : Box; END_VAR
            read_w := b.w;
            b.w := 0;
        END_FUNCTION

        FUNCTION test : INT
        VAR b : Box; END_VAR
            b.v := 3;
            b.w := 4;
            test := read_w(b) * 10 + b.w;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 44, "read w = 4; the caller's w is untouched");
}

/// An input whose address the body takes is a wasm parameter, which has
/// none: it is copied into memory at entry. Each form used to panic codegen.
#[rstest]
fn fn_input_whose_address_is_taken(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION inc
        VAR_IN_OUT io : INT; END_VAR
            io := io + 1;
        END_FUNCTION

        FUNCTION put
        VAR_OUTPUT o : INT; END_VAR
            o := 9;
        END_FUNCTION

        FUNCTION_BLOCK Bumper
        VAR_IN_OUT io : INT; END_VAR
            io := io + 1;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK M
            METHOD PUBLIC twice : INT
            VAR_INPUT x : INT; END_VAR
                inc(io := x);
                inc(io := x);
                twice := x;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION as_in_out : INT
        VAR_INPUT x : INT; END_VAR
            inc(io := x);
            as_in_out := x;
        END_FUNCTION

        FUNCTION via_ref : INT
        VAR_INPUT x : INT; END_VAR
        VAR r : REF_TO INT; END_VAR
            r := REF(x);
            r^ := x + 2;
            via_ref := x;
        END_FUNCTION

        FUNCTION as_output : INT
        VAR_INPUT x : INT; END_VAR
            put(o => x);
            as_output := x;
        END_FUNCTION

        FUNCTION as_fb_in_out : INT
        VAR_INPUT x : INT; END_VAR
        VAR b : Bumper; END_VAR
            b(io := x);
            as_fb_in_out := x;
        END_FUNCTION

        // One bit per form that went wrong.
        FUNCTION test : INT
        VAR v : INT := 5; m : M; END_VAR
            IF as_in_out(v) <> 6 THEN test := test + 1; END_IF;
            IF v <> 5 THEN test := test + 2; END_IF;
            IF via_ref(5) <> 7 THEN test := test + 4; END_IF;
            IF as_output(5) <> 9 THEN test := test + 8; END_IF;
            IF as_fb_in_out(5) <> 6 THEN test := test + 16; END_IF;
            IF m.twice(5) <> 7 THEN test := test + 32; END_IF;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 0, "each form wrote the callee's copy only");
}

/// A STRING input passed on as a VAR_IN_OUT is the callee's copy too.
#[rstest]
fn fn_string_input_as_in_out(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION shout
        VAR_IN_OUT s : STRING; END_VAR
            s := 'bye';
        END_FUNCTION

        FUNCTION louder : STRING
        VAR_INPUT s : STRING; END_VAR
            shout(s := s);
            louder := s;
        END_FUNCTION

        FUNCTION test : INT
        VAR v : STRING := 'hi'; w : STRING; END_VAR
            w := louder(v);
            IF w <> 'bye' THEN test := test + 1; END_IF;
            IF v <> 'hi' THEN test := test + 2; END_IF;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 0, "'bye' in the callee, 'hi' in the caller");
}

/// A METHOD's inputs are laid out before its locals, whatever order the
/// sections are declared in: a VAR written first used to take the index of
/// the first input.
#[rstest]
fn method_locals_declared_before_inputs(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK F
            METHOD PUBLIC m : INT
            VAR l : INT; END_VAR
            VAR_INPUT x : INT; END_VAR
                l := 100;
                m := x + l;
            END_METHOD
        END_FUNCTION_BLOCK

        CLASS C
            METHOD PUBLIC m : INT
            VAR l : INT; END_VAR
            VAR_INPUT x : INT; END_VAR
                l := 100;
                m := x + l;
            END_METHOD
        END_CLASS

        FUNCTION test : INT
        VAR f : F; c : C; END_VAR
            IF f.m(5) <> 105 THEN test := test + 1; END_IF;
            IF c.m(7) <> 107 THEN test := test + 2; END_IF;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 0);
}

/// An omitted input takes the callee's default, folded: a name in it is the
/// callee's, never a caller variable of that name. It used to be read in the
/// caller, where a local `K` or `L` hid the constant, or codegen panicked.
#[rstest]
fn fn_default_is_the_callee_constant(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Color : (Red, Green, Blue); END_TYPE

        CONFIGURATION Cfg
        VAR_GLOBAL CONSTANT K : INT := 7; KL : LREAL := 2.5; END_VAR
        VAR_GLOBAL G : INT := 3; END_VAR
        END_CONFIGURATION

        FUNCTION with_k : INT
        VAR_INPUT x : INT := K; END_VAR
        VAR_EXTERNAL CONSTANT K : INT; END_VAR
            with_k := x;
        END_FUNCTION

        FUNCTION with_l : INT
        VAR_INPUT x : INT := L * 2 + 1; END_VAR
        VAR CONSTANT L : INT := 9; END_VAR
            with_l := x;
        END_FUNCTION

        FUNCTION wide : LINT
        VAR_INPUT x : LINT := 5000000000; END_VAR
            wide := x;
        END_FUNCTION

        FUNCTION real_k : LREAL
        VAR_INPUT r : LREAL := KL; END_VAR
        VAR_EXTERNAL CONSTANT KL : LREAL; END_VAR
            real_k := r;
        END_FUNCTION

        FUNCTION pick : Color
        VAR_INPUT c : Color := Color#Blue; END_VAR
            pick := c;
        END_FUNCTION

        FUNCTION through : INT
        VAR_INPUT r : REF_TO INT := REF(G); END_VAR
        VAR_EXTERNAL G : INT; END_VAR
            through := r^;
        END_FUNCTION

        FUNCTION set_g
        VAR_EXTERNAL G : INT; END_VAR
            G := 3;
        END_FUNCTION

        FUNCTION_BLOCK Fb
            METHOD PUBLIC get : INT
            VAR_INPUT x : INT := L; END_VAR
            VAR CONSTANT L : INT := 4; END_VAR
                get := x;
            END_METHOD
        END_FUNCTION_BLOCK

        // The caller's K, L and G are someone else's: one bit per default
        // that read one.
        FUNCTION test : INT
        VAR K : INT := 100; L : INT := 1000; G : INT := 50; fb : Fb; END_VAR
            set_g();
            IF with_k() <> 7 THEN test := test + 1; END_IF;
            IF with_l() <> 19 THEN test := test + 2; END_IF;
            IF wide() <> 5000000000 THEN test := test + 4; END_IF;
            IF real_k() <> 2.5 THEN test := test + 8; END_IF;
            IF pick() <> Color#Blue THEN test := test + 16; END_IF;
            IF through() <> 3 THEN test := test + 32; END_IF;
            IF fb.get() <> 4 THEN test := test + 64; END_IF;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 0);
}
