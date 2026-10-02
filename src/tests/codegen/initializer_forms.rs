// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Every form an initializer can take, computed once in the initializer and
//! once in a body and compared at run time. A FUNCTION local's runs at each
//! call and lowers as its body does, because both read the scope's inference
//! through one view (`ScopeId::inference`); a static's or a type default's
//! is a constant, folded at compile time, and the folder must agree with
//! the emitted code. A new form that read only half of the inference, or a
//! fold that computed differently, would show up here. Each source runs its
//! own `{test}` functions on the host, as `rk test` does, with a `check` of
//! its own: the test database holds no stdlib.

use rstest::rstest;

use super::with_db;

/// The types and the assertion every source below starts with.
const PRELUDE: &str = r#"
TYPE Pt : STRUCT x : INT := 7; y : INT; END_STRUCT; END_TYPE
TYPE Arr3 : ARRAY[1..3] OF INT; END_TYPE
TYPE Box : STRUCT p : Pt; arr : Arr3 := [1, 2, 3]; END_STRUCT; END_TYPE
TYPE Color : (Red, Green, Blue); END_TYPE
TYPE Pct : INT (0..100); END_TYPE

FUNCTION check
VAR_INPUT ok : BOOL; what : STRING; END_VAR
    IF NOT ok THEN __RAISE(what); END_IF;
END_FUNCTION
"#;

/// Compile the prelude and `source`, run every `{test}` in it, and expect
/// `count` passes: a failure says which check, by its message.
fn run_all(db: &mut db::RootDatabase, source: &str, count: usize) {
    let source = format!("{PRELUDE}\n{source}");
    let wasm = super::compile_to_wasm_as_built(db, &source);
    let results = super::run_tests(&wasm, None).expect("run tests");
    let failed: Vec<String> = results
        .iter()
        .filter(|r| !r.passed())
        .map(|r| format!("{}: {}", r.name, r.reason.as_deref().unwrap_or("no reason")))
        .collect();
    assert!(failed.is_empty(), "{}", failed.join("\n"));
    assert_eq!(results.len(), count, "every test ran");
}

/// Indexing by a constant, a variable and two subscripts; a struct's field
/// and an element through it; every dereference shape (`p^`, `pp^^`,
/// `p^.x`, `p^.arr[i]`, `p^[i]`, a reference into a field or an element);
/// and the parts of a DWORD.
#[rstest]
fn access_forms(mut with_db: db::RootDatabase) {
    run_all(
        &mut with_db,
        r#"
{test}
FUNCTION t_index_and_field
VAR
    arr : ARRAY[1..3] OF INT := [10, 20, 30];
    i : INT := 2;
    e_const : INT := arr[3];
    e_dyn : INT := arr[i];
    b : Box;
    bx : INT := b.p.x;
    be : INT := b.arr[2];
    pts : ARRAY[1..2] OF Pt := [(x := 1, y := 2), (x := 3, y := 4)];
    py : INT := pts[2].y;
    pyi : INT := pts[i].x;
    e_constb, e_dynb, bxb, beb, pyb, pyib : INT;
END_VAR
    e_constb := arr[3]; e_dynb := arr[i]; bxb := b.p.x; beb := b.arr[2]; pyb := pts[2].y; pyib := pts[i].x;
    check(e_const = e_constb, 'arr[3]'); check(e_const = 30, 'arr[3] is 30');
    check(e_dyn = e_dynb, 'arr[i]'); check(e_dyn = 20, 'arr[i] is 20');
    check(bx = bxb, 'b.p.x'); check(bx = 7, 'the type default');
    check(be = beb, 'b.arr[2]'); check(be = 2, 'b.arr[2] is 2');
    check(py = pyb, 'pts[2].y'); check(py = 4, 'pts[2].y is 4');
    check(pyi = pyib, 'pts[i].x'); check(pyi = 3, 'pts[i].x is 3');
END_FUNCTION

{test}
FUNCTION t_matrix
VAR
    m : ARRAY[1..2, 1..2] OF INT := [1, 2, 3, 4];
    me : INT := m[2, 1];
    meb : INT;
END_VAR
    meb := m[2, 1];
    check(me = meb, 'm[2, 1]'); check(me = 3, 'm[2, 1] is 3');
END_FUNCTION

{test}
FUNCTION t_deref_forms
VAR
    v : DINT := 5;
    p : REF_TO DINT := REF(v);
    pp : REF_TO REF_TO DINT := REF(p);
    d1 : DINT := p^;
    d2 : DINT := pp^^;
    b : Box;
    pb : REF_TO Box := REF(b);
    f1 : INT := pb^.p.x;
    f2 : INT := pb^.arr[3];
    pa : REF_TO Arr3 := REF(b.arr);
    a1 : INT := pa^[1];
    px : REF_TO INT := REF(b.p.x);
    x1 : INT := px^;
    pe : REF_TO INT := REF(b.arr[2]);
    e1 : INT := pe^;
    d1b, d2b : DINT; f1b, f2b, a1b, x1b, e1b : INT;
END_VAR
    d1b := p^; d2b := pp^^; f1b := pb^.p.x; f2b := pb^.arr[3]; a1b := pa^[1]; x1b := px^; e1b := pe^;
    check(d1 = d1b, 'p^'); check(d2 = d2b, 'pp^^'); check(f1 = f1b, 'pb^.p.x'); check(f2 = f2b, 'pb^.arr[3]');
    check(a1 = a1b, 'pa^[1]'); check(x1 = x1b, 'px^'); check(e1 = e1b, 'pe^');
    check(d2 = 5, 'pp^^ is 5'); check(f1 = 7, 'pb^.p.x is 7'); check(e1 = 2, 'pe^ is 2');
    pe^ := 9;
    check(b.arr[2] = 9, 'written through pe');
END_FUNCTION

{test}
FUNCTION t_multibits
VAR
    d : DWORD := 16#11223344;
    b1 : BYTE := d.%B1;
    w0 : WORD := d.%W0;
    x3 : BOOL := d.%X3;
    n : BOOL := NOT d.%X0;
    b1b : BYTE; w0b : WORD; x3b, nb : BOOL;
END_VAR
    b1b := d.%B1; w0b := d.%W0; x3b := d.%X3; nb := NOT d.%X0;
    check(b1 = b1b, 'd.%B1'); check(b1 = BYTE#16#33, 'd.%B1 is 33');
    check(w0 = w0b, 'd.%W0'); check(w0 = WORD#16#3344, 'd.%W0 is 3344');
    check(x3 = x3b, 'd.%X3'); check(n = nb, 'NOT d.%X0');
END_FUNCTION
    "#,
        4,
    );
}

/// Every call form: positional, named, with defaults, with outputs, with a
/// VAR_IN_OUT, nested, overloaded on TIME and LTIME; returns of a struct, an
/// array and a STRING (truncated into a shorter one); and methods on an
/// instance, through a reference and on an array element.
#[rstest]
fn call_forms(mut with_db: db::RootDatabase) {
    run_all(
        &mut with_db,
        r#"
FUNCTION add3 : INT
VAR_INPUT a : INT; b : INT := 10; c : INT := 100; END_VAR
    add3 := a + b + c;
END_FUNCTION

FUNCTION split : INT
VAR_INPUT n : INT; END_VAR
VAR_OUTPUT lo : INT; hi : INT; END_VAR
    lo := n MOD 10; hi := n / 10;
    split := n;
END_FUNCTION

FUNCTION twice : INT
VAR_IN_OUT v : INT; END_VAR
    v := v * 2;
    twice := v;
END_FUNCTION

FUNCTION make_pt : Pt
VAR_INPUT x : INT; END_VAR
    make_pt.x := x; make_pt.y := x + 1;
END_FUNCTION

FUNCTION make_arr : Arr3
    make_arr[1] := 7; make_arr[2] := 8; make_arr[3] := 9;
END_FUNCTION

FUNCTION greet : STRING
VAR_INPUT who : STRING; END_VAR
    IF who = 'bob' THEN greet := 'hi bob'; ELSE greet := 'hi ?'; END_IF;
END_FUNCTION

FUNCTION big : TIME
VAR_INPUT a : TIME; b : TIME; END_VAR
    IF a > b THEN big := a; ELSE big := b; END_IF;
END_FUNCTION

FUNCTION big : LTIME
VAR_INPUT a : LTIME; b : LTIME; END_VAR
    IF a > b THEN big := a; ELSE big := b; END_IF;
END_FUNCTION

{test}
FUNCTION t_call_forms
VAR
    i : INT := 5;
    pos : INT := add3(1, 2, 3);
    named : INT := add3(c := 1, a := 2);
    dflt : INT := add3(i);
    lo : INT; hi : INT;
    whole : INT := split(42, lo => lo, hi => hi);
    v : INT := 3;
    tw : INT := twice(v);
    nested : INT := add3(add3(1), twice(v));
    t : TIME := T#2s;
    l : LTIME := LT#1s;
    m : LTIME := big(t, l);
    posb, namedb, dfltb, wholeb, twb, nestedb, lob, hib, vb : INT; mb : LTIME;
END_VAR
    posb := add3(1, 2, 3); namedb := add3(c := 1, a := 2); dfltb := add3(i);
    wholeb := split(42, lo => lob, hi => hib);
    vb := 3; twb := twice(vb); nestedb := add3(add3(1), twice(vb)); mb := big(t, l);
    check(pos = posb, 'positional'); check(named = namedb, 'named'); check(named = 13, 'named is 13');
    check(dflt = dfltb, 'defaults'); check(dflt = 115, 'defaults is 115');
    check(whole = wholeb, 'outputs'); check(lo = lob, 'lo =>'); check(lo = 2, 'lo is 2'); check(hi = hib, 'hi =>'); check(hi = 4, 'hi is 4');
    check(tw = twb, 'in-out'); check(v = vb, 'in-out written'); check(v = 12, 'v doubled twice');
    check(nested = nestedb, 'nested calls'); check(m = mb, 'overload on TIME and LTIME'); check(m = LT#2s, 'big is 2s');
END_FUNCTION

{test}
FUNCTION t_aggregate_and_string_returns
VAR
    i : INT := 9;
    p : Pt := make_pt(3);
    a : Arr3 := make_arr();
    s : STRING := greet('bob');
    s5 : STRING[5] := greet('bob');
    isbob : BOOL := greet('bob') = 'hi bob';
    pb : Pt; ab : Arr3; sb : STRING; s5b : STRING[5]; isbobb : BOOL;
END_VAR
    pb := make_pt(3); ab := make_arr(); sb := greet('bob'); s5b := greet('bob'); isbobb := greet('bob') = 'hi bob';
    check(p.x = pb.x, 'struct return x'); check(p.y = pb.y, 'struct return y'); check(p.y = 4, 'y is 4');
    check(a[3] = ab[3], 'array return'); check(a[3] = 9, 'a[3] is 9');
    check(s = sb, 'string return'); check(s = 'hi bob', 'hi bob');
    check(s5 = s5b, 'truncated string return'); check(s5 = 'hi bo', 'STRING[5]');
    check(isbob = isbobb, 'a call compared to a literal'); check(isbob, 'is bob');
END_FUNCTION

FUNCTION_BLOCK Counter
VAR_INPUT step : INT := 1; END_VAR
VAR n : INT := 4; END_VAR
    METHOD Get : INT
        Get := n;
    END_METHOD
    METHOD Scaled : INT
    VAR_INPUT k : INT; END_VAR
    VAR base : INT := n * k; END_VAR
        Scaled := base;
    END_METHOD
    n := n + step;
END_FUNCTION_BLOCK

{test}
FUNCTION t_methods_in_init
VAR
    c : Counter;
    pc : REF_TO Counter := REF(c);
    g0 : INT := c.Get();
    s0 : INT := pc^.Scaled(3);
    arr : ARRAY[1..2] OF Counter;
    ga : INT := arr[2].Get();
    g0b, s0b, gab : INT;
END_VAR
    g0b := c.Get(); s0b := pc^.Scaled(3); gab := arr[2].Get();
    check(g0 = g0b, 'c.Get()'); check(g0 = 4, 'member default');
    check(s0 = s0b, 'pc^.Scaled(3)'); check(s0 = 12, 'method local from input and member');
    check(ga = gab, 'arr[2].Get()'); check(ga = 4, 'element default');
END_FUNCTION
    "#,
        3,
    );
}

/// What a local's initializer may read and when it runs: an input, a
/// VAR_IN_OUT (written back), an output's default on every call, a local
/// starting over on every call, and an FB's VAR_TEMP on every invocation.
#[rstest]
fn scope_forms(mut with_db: db::RootDatabase) {
    run_all(
        &mut with_db,
        r#"
FUNCTION dbl : INT
VAR_INPUT n : INT; END_VAR
VAR d : INT := n * 2; END_VAR
    dbl := d;
END_FUNCTION

FUNCTION io_plus : INT
VAR_IN_OUT io : INT; END_VAR
VAR d : INT := io + 1; END_VAR
    io := d;
    io_plus := d;
END_FUNCTION

FUNCTION out_init : INT
VAR_OUTPUT o : INT := 7; END_VAR
    out_init := o;
END_FUNCTION

FUNCTION counter_local : INT
VAR c : INT := 0; END_VAR
    c := c + 1;
    counter_local := c;
END_FUNCTION

{test}
FUNCTION t_per_call
VAR a : INT := 5; o1 : INT; o2 : INT; END_VAR
    check(dbl(3) = 6, 'dbl(3)');
    check(dbl(4) = 8, 'dbl(4) recomputes');
    check(io_plus(a) = 6, 'io + 1'); check(a = 6, 'written back');
    check(io_plus(a) = 7, 'again');
    check(out_init(o => o1) = 7, 'output default'); check(o1 = 7, 'o1');
    o1 := 0;
    check(out_init(o => o2) = 7, 'output default again'); check(o2 = 7, 'o2');
    check(counter_local() = 1, 'local starts over'); check(counter_local() = 1, 'local starts over again');
END_FUNCTION

FUNCTION_BLOCK Temps
VAR_INPUT n : INT; END_VAR
VAR_OUTPUT out : INT; END_VAR
VAR_TEMP t : INT; END_VAR
    t := t + n;
    out := t;
END_FUNCTION_BLOCK

{test}
FUNCTION t_fb_temp_per_invocation
VAR f : Temps; END_VAR
    f(n := 1); check(f.out = 1, 'temp from n=1');
    f(n := 5); check(f.out = 5, 'temp starts over at 0');
END_FUNCTION
    "#,
        2,
    );
}

/// Values: enums, a subrange from an expression, implicit widenings,
/// negation, MOD, a power, boolean and bit-string operators;
/// nested array and struct initializers with non-constant parts and
/// references in them; strings; NULL and address comparisons.
#[rstest]
fn value_forms(mut with_db: db::RootDatabase) {
    run_all(
        &mut with_db,
        r#"
{test}
FUNCTION t_enums_subranges_conversions
VAR
    c : Color := Color#Green;
    c2 : Color := c;
    isg : BOOL := c = Color#Green;
    i : INT := 42;
    pc : Pct := i + 8;
    d : DINT := i * DINT#100000;
    r : REAL := 42.0 / 8.0;
    l : LINT := i;
    lr : LREAL := r;
    dt : DT := DT#2024-01-01-00:00:00;
    ldt : LDT := dt;
    neg : INT := -i;
    md : INT := i MOD 5;
    pw : REAL := 2.0 ** 3;
    bo : BOOL := isg AND NOT (i > 50) XOR FALSE;
    w : WORD := WORD#16#F0F0 AND WORD#16#FF00;
    c2b : Color; isgb, bob : BOOL; pcb : Pct; db : DINT; rb, pwb : REAL; lb : LINT; lrb : LREAL; ldtb : LDT; negb, mdb : INT; wb : WORD;
END_VAR
    c2b := c; isgb := c = Color#Green; pcb := i + 8; db := i * DINT#100000; rb := 42.0 / 8.0;
    lb := i; lrb := r; ldtb := dt; negb := -i; mdb := i MOD 5; pwb := 2.0 ** 3; bob := isg AND NOT (i > 50) XOR FALSE;
    wb := WORD#16#F0F0 AND WORD#16#FF00;
    check(c2 = c2b, 'enum copy'); check(isg = isgb, 'enum compare'); check(isg, 'is green');
    check(pc = pcb, 'subrange from expr'); check(pc = 50, 'pc 50');
    check(d = db, 'DINT conversion'); check(d = 4200000, 'd');
    check(r = rb, 'REAL division'); check(l = lb, 'widen to LINT'); check(lr = lrb, 'widen to LREAL');
    check(ldt = ldtb, 'DT to LDT'); check(neg = negb, 'negation'); check(md = mdb, 'MOD');
    check(pw = pwb, 'power'); check(bo = bob, 'bool ops'); check(w = wb, 'word AND'); check(w = WORD#16#F000, 'F000');
END_FUNCTION

{test}
FUNCTION t_nested_initializers
VAR
    i : INT := 3;
    a : ARRAY[1..5] OF INT := [i, i * 2, i + 3, 2(i)];
    p : Pt := (x := i, y := i * 2);
    pts : ARRAY[1..2] OF Pt := [(x := i), (y := i + 1)];
    refs : ARRAY[1..2] OF REF_TO INT := [REF(i), REF(a[1])];
    d0 : INT := refs[2]^;
END_VAR
    check(a[1] = 3, 'a[1]'); check(a[2] = 6, 'a[2]'); check(a[3] = 6, 'a[3]'); check(a[4] = 3, 'a[4]'); check(a[5] = 3, 'a[5]');
    check(p.x = 3, 'p.x'); check(p.y = 6, 'p.y');
    check(pts[1].x = 3, 'pts[1].x'); check(pts[1].y = 0, 'pts[1].y'); check(pts[2].x = 7, 'pts[2].x default'); check(pts[2].y = 4, 'pts[2].y');
    check(d0 = 3, 'refs[2]^');
    refs[1]^ := 9;
    check(i = 9, 'written through refs[1]');
END_FUNCTION

{test}
FUNCTION t_strings_in_init
VAR
    s : STRING[10] := 'abc';
    s2 : STRING := s;
    eq : BOOL := s = 'abc';
    lt : BOOL := 'abc' < 'abd';
    tr : STRING[2] := s;
    s2b : STRING; eqb, ltb : BOOL; trb : STRING[2];
END_VAR
    s2b := s; eqb := s = 'abc'; ltb := 'abc' < 'abd'; trb := s;
    check(s2 = s2b, 'string copy'); check(s2 = 'abc', 'abc');
    check(eq = eqb, 'string eq'); check(eq, 'eq'); check(lt = ltb, 'string lt'); check(lt, 'lt');
    check(tr = trb, 'truncated'); check(tr = 'ab', 'ab');
END_FUNCTION

{test}
FUNCTION t_null_refs
VAR
    p : REF_TO INT := NULL;
    isnull : BOOL := p = NULL;
    i : INT := 1;
    q : REF_TO INT := REF(i);
    notnull : BOOL := q <> NULL;
    same : BOOL := q = REF(i);
    isnullb, notnullb, sameb : BOOL;
END_VAR
    isnullb := p = NULL; notnullb := q <> NULL; sameb := q = REF(i);
    check(isnull = isnullb, 'p = NULL'); check(isnull, 'null'); check(notnull = notnullb, 'q <> NULL'); check(notnull, 'not null');
    check(same = sameb, 'q = REF(i)'); check(same, 'same address');
END_FUNCTION
    "#,
        4,
    );
}

/// What a static's or a type default's initializer folds to, against the same
/// expression computed at run time on variables, so that side is wasm's and
/// no folder's. Integers wrap at their declared width, as
/// `docs/math-operations.md` says, before any widening; `/` truncates and
/// MOD keeps the dividend's sign, as wasm does; a radix literal is its bit
/// pattern; a CONSTANT chain wraps where it is declared. REAL arithmetic
/// rounds in single precision at each step: one operation folded in double
/// and rounded once is the same number, a chain is not, so
/// `16777216.0 + 1.0 + 1.0` is 16777216.0 and the 2^53 chain the LREAL
/// counterpart.
#[rstest]
fn fold_forms(mut with_db: db::RootDatabase) {
    run_all(
        &mut with_db,
        r#"
FUNCTION_BLOCK Folded
VAR CONSTANT K1 : INT := 1000; K2 : INT := K1 * 40; END_VAR
VAR_OUTPUT
    o1 : INT := INT#32767 + 1;
    o2 : SINT := SINT#100 + SINT#100;
    o3 : DINT := SINT#100 + SINT#100;
    o4 : BYTE := BYTE#16#FF + 1;
    o5 : SINT := SINT#16#FF + 1;
    o6 : USINT := USINT#255 + USINT#1;
    o7 : INT := -7 / 2;
    o8 : INT := -7 MOD 3;
    o9 : INT := 7 MOD -3;
    o10 : INT := 200 * 200;
    o11 : REAL := REAL#0.1 + REAL#0.2;
    o12 : LREAL := REAL#0.1 + 0.2;
    o13 : LREAL := LREAL#0.1 + LREAL#0.2;
    o14 : REAL := REAL#1.0 / REAL#3.0;
    o15 : REAL := 2.0 ** -1;
    o16 : REAL := 4.0 ** 0.5;
    o18 : LINT := DINT#2147483647 + DINT#1;
    o19 : DINT := DINT#16#7FFFFFFF + DINT#1;
    o20 : INT := -(16#80);
    o22 : DINT := K2;
    o24 : LTIME := T#2s;
    o25 : REAL := REAL#16777216.0 + REAL#1.0;
    o26 : LREAL := 1 / 3;
    o27 : INT := 7 / -2;
    o28 : REAL := REAL#16777216.0 + REAL#1.0 + REAL#1.0;
    o29 : LREAL := LREAL#9007199254740992.0 + LREAL#1.0 + LREAL#1.0;
    o30 : REAL := REAL#0.1 * REAL#3.0 - REAL#0.3;
END_VAR
END_FUNCTION_BLOCK

{test}
FUNCTION t_folded_members
VAR CONSTANT K1 : INT := 1000; K2 : INT := K1 * 40; END_VAR
VAR f : Folded; END_VAR
VAR
    a1 : REAL := 0.1; a2 : REAL := 0.2; a3 : REAL := 0.3; one : REAL := 1.0; two : REAL := 2.0; three : REAL := 3.0;
    four : REAL := 4.0; half : REAL := 0.5; mone : REAL := -1.0; big : REAL := 16777216.0;
    la : LREAL := 0.1; lb : LREAL := 0.2; lone : LREAL := 1.0; lbig : LREAL := 9007199254740992.0;
END_VAR
    check(f.o1 = INT#32767 + 1, 'o1'); check(f.o1 = INT#-32768, 'o1 wraps');
    check(f.o2 = SINT#100 + SINT#100, 'o2'); check(f.o2 = SINT#-56, 'o2 wraps');
    check(f.o3 = SINT#100 + SINT#100, 'o3'); check(f.o3 = DINT#-56, 'o3 wraps before widening');
    check(f.o4 = BYTE#16#FF + 1, 'o4'); check(f.o4 = BYTE#0, 'o4 wraps');
    check(f.o5 = SINT#16#FF + 1, 'o5'); check(f.o5 = SINT#0, 'o5 is 0');
    check(f.o6 = USINT#255 + USINT#1, 'o6'); check(f.o6 = USINT#0, 'o6 wraps');
    check(f.o7 = -7 / 2, 'o7'); check(f.o7 = -3, 'o7 truncates');
    check(f.o8 = -7 MOD 3, 'o8'); check(f.o8 = -1, 'o8 sign of the dividend');
    check(f.o9 = 7 MOD -3, 'o9'); check(f.o9 = 1, 'o9');
    check(f.o10 = 200 * 200, 'o10'); check(f.o10 = -25536, 'o10 wraps');
    check(f.o11 = a1 + a2, 'o11');
    check(f.o12 = a1 + a2, 'o12');
    check(f.o13 = la + lb, 'o13');
    check(f.o14 = one / three, 'o14');
    check(f.o15 = two ** mone, 'o15'); check(f.o15 = 0.5, 'o15 is 0.5');
    check(f.o16 = four ** half, 'o16'); check(f.o16 = 2.0, 'o16 is 2');
    check(f.o18 = DINT#2147483647 + DINT#1, 'o18'); check(f.o18 = LINT#-2147483648, 'o18 wraps before widening');
    check(f.o19 = DINT#16#7FFFFFFF + DINT#1, 'o19'); check(f.o19 = DINT#-2147483648, 'o19 wraps');
    check(f.o20 = -(16#80), 'o20'); check(f.o20 = -128, 'o20');
    check(f.o22 = K2, 'o22'); check(f.o22 = DINT#-25536, 'o22 constant chain wraps');
    check(f.o24 = LT#2s, 'o24');
    check(f.o25 = big + one, 'o25');
    check(f.o26 = 1 / 3, 'o26'); check(f.o26 = LREAL#0.0, 'o26 integer division');
    check(f.o27 = 7 / -2, 'o27'); check(f.o27 = -3, 'o27');
    check(f.o28 = big + one + one, 'o28'); check(f.o28 = REAL#16777216.0, 'o28 rounds at each step');
    check(f.o29 = lbig + lone + lone, 'o29'); check(f.o29 = LREAL#9007199254740992.0, 'o29 rounds at each step');
    check(f.o30 = a1 * three - a3, 'o30');
END_FUNCTION

TYPE FoldedT : STRUCT
    w : INT := INT#32767 + 1;
    q : INT := -7 / 2;
    m : INT := -7 MOD 3;
    r : REAL := REAL#0.1 + REAL#0.2;
    x : LREAL := REAL#0.1 + 0.2;
    p : REAL := 2.0 ** -1;
    c : REAL := REAL#16777216.0 + REAL#1.0 + REAL#1.0;
END_STRUCT; END_TYPE

{test}
FUNCTION t_folded_type_defaults
VAR s : FoldedT; a1 : REAL := 0.1; a2 : REAL := 0.2; two : REAL := 2.0; mone : REAL := -1.0; one : REAL := 1.0; big : REAL := 16777216.0; END_VAR
    check(s.w = INT#32767 + 1, 'w'); check(s.w = INT#-32768, 'w wraps');
    check(s.q = -7 / 2, 'q'); check(s.m = -7 MOD 3, 'm');
    check(s.r = a1 + a2, 'r'); check(s.x = a1 + a2, 'x'); check(s.p = two ** mone, 'p'); check(s.p = 0.5, 'p is 0.5');
    check(s.c = big + one + one, 'c'); check(s.c = REAL#16777216.0, 'c rounds at each step');
END_FUNCTION
    "#,
        2,
    );
}
