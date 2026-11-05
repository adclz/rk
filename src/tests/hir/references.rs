// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn valid_null_initializer(mut with_db: RootDatabase) {
    let source = r#"
    FUNCTION_BLOCK fn1
        VAR
            test: INT;
            test2: REF_TO INT := NULL;
        END_VAR
    END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_ref_to_elementary_type(mut with_db: RootDatabase) {
    let source = r#"
    FUNCTION_BLOCK fn1
        VAR
            test: UINT;
            test2: REF_TO INT := REF(test); // Reference to INT, but test is UINT
        END_VAR
    END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:5:31 ]
       |
     5 |             test2: REF_TO INT := REF(test); // Reference to INT, but test is UINT
       |                               ^^^^^^|^^^^^
       |                                     `------- expected 'REF_TO INT', got 'REF_TO UINT'
    ---'
    ");
}

#[rstest]
fn valid_ref_to_pou_type(mut with_db: RootDatabase) {
    let source = r#"
    FUNCTION_BLOCK fb1 END_FUNCTION_BLOCK

    FUNCTION_BLOCK fn1
        VAR
            test: fb1;
            test2: REF_TO fb1 := REF(test);
        END_VAR

    END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_ref_to_pou_type(mut with_db: RootDatabase) {
    let source = r#"
    FUNCTION_BLOCK fb1 END_FUNCTION_BLOCK
    FUNCTION_BLOCK fb2 END_FUNCTION_BLOCK

    FUNCTION_BLOCK fn1
        VAR
            test: fb2;
            test2: REF_TO fb1 := REF(test); // Reference to fb1, but test is fb2
        END_VAR

    END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:31 ]
       |
     8 |             test2: REF_TO fb1 := REF(test); // Reference to fb1, but test is fb2
       |                               ^^^^^^|^^^^^
       |                                     `------- expected 'REF_TO fb1', got 'REF_TO fb2'
    ---'
    ");
}

#[rstest]
fn assign_non_ref_type(mut with_db: RootDatabase) {
    let source = r#"
    FUNCTION_BLOCK fn1
        VAR
            test: REF_TO INT;
        END_VAR

        test := 0;

    END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:17 ]
       |
     4 |             test: REF_TO INT;
       |             ^^|^
       |               `--- 'test' is declared here
       |
     7 |         test := 0;
       |                 |
       |                 `-- expected 'REF_TO INT', got 'INT'
    ---'
    ");
}

#[rstest]
fn valid_assign_null_to_ref_type(mut with_db: RootDatabase) {
    let source = r#"
    FUNCTION_BLOCK fn1
        VAR
            test: REF_TO INT;
        END_VAR

        test := NULL;

    END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_assign_ref_to_ref_type(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
	S1: STRUCT
		SC1: INT;
		SC2: REAL;
	END_STRUCT;
	A1: ARRAY[1..99] OF INT;
        END_TYPE

        FUNCTION fn: INT

	VAR
		myS1: S1;
		myA1: A1;
		myRefS1: REF_TO S1 := REF(myS1);
		myRefA1: REF_TO A1 := REF(myA1);
	END_VAR

	myRefS1^.SC1 := myRefA1^[12]; // in this case, equivalent to S1.SC1:= A1[12];


        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_using_array_as_ref_type(mut with_db: RootDatabase) {
    let source = r#"
TYPE
	A1: ARRAY[1..99] OF INT;
END_TYPE

FUNCTION fn: BOOL

	VAR
		myA1: A1;
		myRefInt: REF_TO INT := REF(myA1[1]);
	END_VAR

	myRefInt := REF(myA1[11]);

END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_using_array_as_ref_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn: BOOL

	VAR
		myA1: ARRAY[1..10, 1..10] OF INT;
		myInt: INT;
	END_VAR

	myInt := REF(myA1[2][9]);

END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:9:11 ]
       |
     6 |        myInt: INT;
       |        ^^|^^
       |          `---- 'myInt' is declared here
       |
     9 |     myInt := REF(myA1[2][9]);
       |              ^^^^^^^|^^^^^^^
       |                     `--------- expected 'INT', got 'REF_TO INT'
    ---'
    ");
}

#[rstest]
fn valid_using_deref_array_as_ref_type(mut with_db: RootDatabase) {
    let source = r#"
TYPE
	S1: STRUCT
		SC1: INT;
		SC2: REAL;
	END_STRUCT;
	A1: ARRAY[1..99] OF INT;
END_TYPE

FUNCTION fn: INT

	VAR
		myS1: S1;
		myA1: A1;
		myRefS1: REF_TO S1 := REF(myS1);
		myRefA1: REF_TO A1 := REF(myA1);
	END_VAR

	myRefS1^.SC1 := myRefA1^[12]; // in this case, equivalent to S1.SC1:= A1[12];
	myS1.SC1:= myA1[12];

END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_multiple_deref(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test_double_deref: INT
	VAR
		x: INT := 5;
		ptr: REF_TO INT;
		ptrptr: REF_TO REF_TO INT;
	END_VAR

	ptr := REF(x);
	ptrptr := REF(ptr);
	test_double_deref := ptrptr^^;

END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_array_of_ref_to_elementary(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        x: INT := 10;
        arr: ARRAY[0..2] OF REF_TO INT;
    END_VAR

    arr[0] := REF(x);

END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_array_of_ref_to_pou(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        value: INT;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK fn1
    VAR
        inst: fb1;
        arr: ARRAY[0..2] OF REF_TO fb1;
    END_VAR

    arr[0] := REF(inst);

END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_array_of_ref_to_type_mismatch(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        x: REAL;
        arr: ARRAY[0..2] OF REF_TO INT;
    END_VAR

    arr[0] := REF(x);

END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:15 ]
       |
     5 |         arr: ARRAY[0..2] OF REF_TO INT;
       |         ^|^
       |          `--- 'arr' is declared here
       |
     8 |     arr[0] := REF(x);
       |               ^^^|^^
       |                  `---- expected 'REF_TO INT', got 'REF_TO REAL'
    ---'
    ");
}

#[rstest]
fn valid_array_of_ref_to_null_assign(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        arr: ARRAY[0..2] OF REF_TO INT;
    END_VAR

    arr[0] := NULL;

END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_struct_with_ref_field(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    MyStruct: STRUCT
        ptr: REF_TO INT;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK fn1
    VAR
        x: INT := 10;
        s: MyStruct;
    END_VAR

    s.ptr := REF(x);

END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_deref_array_element(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        x: INT := 42;
        arr: ARRAY[0..2] OF REF_TO INT;
        result: INT;
    END_VAR

    arr[0] := REF(x);
    result := arr[0]^;

END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_deref_struct_field(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    PtrHolder: STRUCT
        ptr: REF_TO INT;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK fn1
    VAR
        x: INT := 10;
        s: PtrHolder;
        result: INT;
    END_VAR

    s.ptr := REF(x);
    result := s.ptr^;

END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_double_deref(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        x: INT := 5;
        ptr: REF_TO INT;
        ptrptr: REF_TO REF_TO INT;
        result: INT;
    END_VAR

    ptr := REF(x);
    ptrptr := REF(ptr);
    result := ptrptr^^;

END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_ref_to_array(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        arr: ARRAY[0..10] OF INT;
        ptr: REF_TO ARRAY[0..10] OF INT;
    END_VAR

    ptr := REF(arr);
    ptr^[0] := 42;

END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_ref_to_array_in_type_decl(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    ArrRef: REF_TO ARRAY[0..10] OF INT;
END_TYPE

FUNCTION_BLOCK fb1
    VAR
        arr: ARRAY[0..10] OF INT;
        ptr: ArrRef;
    END_VAR

    ptr := REF(arr);

END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn returning_a_reference_of_the_wrong_target_type(mut with_db: RootDatabase) {
    // The return slot still type-checks: only the mismatch is refused, and the
    // message names the declared return type rather than the slot.
    let source = r#"
TYPE PInt : REF_TO INT; END_TYPE

FUNCTION borrow : PInt
VAR r : REAL; END_VAR
    borrow := REF(r);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:6:15 ]
       |
     4 | FUNCTION borrow : PInt
       |          ^^^|^^
       |             `---- FUNCTION 'borrow' is declared here, with return type 'PInt'
       |
     6 |     borrow := REF(r);
       |               ^^^|^^
       |                  `---- expected 'PInt', got 'REF_TO REAL'
    ---'
    [E0903] Error: reference outlives its storage
       ,-[ file:///test0.st:6:15 ]
       |
     5 | VAR r : REAL; END_VAR
       |     |
       |     `-- 'r' is per-call storage, declared here
     6 |     borrow := REF(r);
       |               ^^^|^^
       |                  `---- reference to 'r' outlives the call that owns it
       |
       | Help: return a reference to instance state, or to storage the caller owns (a VAR_IN_OUT)
    ---'
    ");
}

// A reference to the POU's own per-call storage, handed back to the caller.
// It does not fault: an address-taken local sits at a fixed address, so the
// reference quietly reads whatever the NEXT call leaves in that slot — which
// is exactly why it is worth refusing at compile time.

#[rstest]
fn returning_a_reference_to_a_local_is_refused(mut with_db: RootDatabase) {
    let source = r#"
TYPE PInt : REF_TO INT; END_TYPE

FUNCTION borrow : PInt
VAR local : INT := 1; END_VAR
    borrow := REF(local);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0903] Error: reference outlives its storage
       ,-[ file:///test0.st:6:15 ]
       |
     5 | VAR local : INT := 1; END_VAR
       |     ^^|^^
       |       `---- 'local' is per-call storage, declared here
     6 |     borrow := REF(local);
       |               ^^^^^|^^^^
       |                    `------ reference to 'local' outlives the call that owns it
       |
       | Help: return a reference to instance state, or to storage the caller owns (a VAR_IN_OUT)
    ---'
    ");
}

#[rstest]
fn returning_a_reference_to_a_method_local_is_refused(mut with_db: RootDatabase) {
    let source = r#"
TYPE PInt : REF_TO INT; END_TYPE

FUNCTION_BLOCK holder
    METHOD PUBLIC slot : PInt
    VAR local : INT; END_VAR
        slot := REF(local);
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0903] Error: reference outlives its storage
       ,-[ file:///test0.st:7:17 ]
       |
     6 |     VAR local : INT; END_VAR
       |         ^^|^^
       |           `---- 'local' is per-call storage, declared here
     7 |         slot := REF(local);
       |                 ^^^^^|^^^^
       |                      `------ reference to 'local' outlives the call that owns it
       |
       | Help: return a reference to instance state, or to storage the caller owns (a VAR_IN_OUT)
    ---'
    ");
}

#[rstest]
fn returning_a_reference_to_caller_storage_is_allowed(mut with_db: RootDatabase) {
    // A VAR_IN_OUT names the caller's storage, which outlives the call.
    let source = r#"
TYPE PInt : REF_TO INT; END_TYPE

FUNCTION borrow : PInt
VAR_IN_OUT t : INT; END_VAR
    borrow := REF(t);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn returning_a_reference_to_instance_state_is_allowed(mut with_db: RootDatabase) {
    let source = r#"
TYPE PInt : REF_TO INT; END_TYPE

FUNCTION_BLOCK holder
VAR total : INT; END_VAR
    METHOD PUBLIC slot : PInt
        slot := REF(total);
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// The same reference kept where it outlives the call: an output and an
// in-out, which the caller owns, an instance member, a global. A block's
// VAR_TEMP is per-call storage too.

#[rstest]
fn storing_a_reference_to_a_local_where_it_outlives_the_call_is_refused(mut with_db: RootDatabase) {
    let source = r#"
TYPE PInt : REF_TO INT; END_TYPE

FUNCTION lend
VAR_OUTPUT o : PInt; END_VAR
VAR_IN_OUT io : PInt; END_VAR
VAR_EXTERNAL g : PInt; END_VAR
VAR tmp : INT; END_VAR
    o := REF(tmp);
    io := REF(tmp);
    g := REF(tmp);
END_FUNCTION

FUNCTION_BLOCK holder
VAR keep : PInt; END_VAR
VAR_TEMP scratch : INT; END_VAR
    METHOD PUBLIC grab
    VAR local : INT; END_VAR
        keep := REF(local);
        THIS.keep := REF(local);
    END_METHOD
    keep := REF(scratch);
END_FUNCTION_BLOCK

CONFIGURATION cfg
VAR_GLOBAL g : PInt; END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0903] Error: reference outlives its storage
       ,-[ file:///test0.st:9:10 ]
       |
     8 | VAR tmp : INT; END_VAR
       |     ^|^
       |      `--- 'tmp' is per-call storage, declared here
     9 |     o := REF(tmp);
       |     |    ^^^^|^^^
       |     `-------------- 'o' outlives the call
       |              |
       |              `----- reference to 'tmp' outlives the call that owns it
       |
       | Help: keep the reference in a local of the call, or point it at storage that outlives it
    ---'
    [E0903] Error: reference outlives its storage
        ,-[ file:///test0.st:10:11 ]
        |
      8 | VAR tmp : INT; END_VAR
        |     ^|^
        |      `--- 'tmp' is per-call storage, declared here
        |
     10 |     io := REF(tmp);
        |     ^|    ^^^^|^^^
        |      `-------------- 'io' outlives the call
        |               |
        |               `----- reference to 'tmp' outlives the call that owns it
        |
        | Help: keep the reference in a local of the call, or point it at storage that outlives it
    ----'
    [E0903] Error: reference outlives its storage
        ,-[ file:///test0.st:11:10 ]
        |
      8 | VAR tmp : INT; END_VAR
        |     ^|^
        |      `--- 'tmp' is per-call storage, declared here
        |
     11 |     g := REF(tmp);
        |     |    ^^^^|^^^
        |     `-------------- 'g' outlives the call
        |              |
        |              `----- reference to 'tmp' outlives the call that owns it
        |
        | Help: keep the reference in a local of the call, or point it at storage that outlives it
    ----'
    [E0903] Error: reference outlives its storage
        ,-[ file:///test0.st:19:17 ]
        |
     18 |     VAR local : INT; END_VAR
        |         ^^|^^
        |           `---- 'local' is per-call storage, declared here
     19 |         keep := REF(local);
        |         ^^|^    ^^^^^|^^^^
        |           `----------------- 'keep' outlives the call
        |                      |
        |                      `------ reference to 'local' outlives the call that owns it
        |
        | Help: keep the reference in a local of the call, or point it at storage that outlives it
    ----'
    [E0903] Error: reference outlives its storage
        ,-[ file:///test0.st:20:22 ]
        |
     18 |     VAR local : INT; END_VAR
        |         ^^|^^
        |           `---- 'local' is per-call storage, declared here
        |
     20 |         THIS.keep := REF(local);
        |         ^^^^|^^^^    ^^^^^|^^^^
        |             `-------------------- 'THIS.keep' outlives the call
        |                           |
        |                           `------ reference to 'local' outlives the call that owns it
        |
        | Help: keep the reference in a local of the call, or point it at storage that outlives it
    ----'
    [E0903] Error: reference outlives its storage
        ,-[ file:///test0.st:22:13 ]
        |
     16 | VAR_TEMP scratch : INT; END_VAR
        |          ^^^|^^^
        |             `----- 'scratch' is per-call storage, declared here
        |
     22 |     keep := REF(scratch);
        |     ^^|^    ^^^^^^|^^^^^
        |       `------------------- 'keep' outlives the call
        |                   |
        |                   `------- reference to 'scratch' outlives the call that owns it
        |
        | Help: keep the reference in a local of the call, or point it at storage that outlives it
    ----'
    ");
}

#[rstest]
fn storing_a_reference_where_it_lives_no_longer_is_allowed(mut with_db: RootDatabase) {
    // A local holding a reference to another local dies with it; a member
    // holding a reference to a member lives as long; a place written through
    // a reference may be anywhere, and is not judged.
    let source = r#"
TYPE PInt : REF_TO INT; END_TYPE
TYPE PPInt : REF_TO PInt; END_TYPE

FUNCTION local_only : INT
VAR tmp : INT; p : PInt; hold : PInt; pp : PPInt; END_VAR
    p := REF(tmp);
    pp := REF(hold);
    pp^ := REF(tmp);
    local_only := p^;
END_FUNCTION

FUNCTION_BLOCK holder
VAR count : INT; keep : PInt; END_VAR
    METHOD PUBLIC grab
        keep := REF(count);
    END_METHOD
    keep := REF(count);
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// Reference binding is INVARIANT: the pointee must be exactly the declared
// type. Before, the value-coercion table was consulted for the pointee, so
// every implicitly-widenable pair checked clean and typed the load wrong —
// invalid wasm at exit 0 for REAL/LREAL, a 2-byte slot read as 4 for DINT.

#[rstest]
fn a_reference_does_not_widen_its_pointee(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : REAL
    VAR x : INT; p : REF_TO REAL; q : REF_TO INT; r : REF_TO REAL := REF(x); END_VAR
    p := REF(x);
    q := REF(x);
    p := q;
    f := p^;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:3:67 ]
       |
     3 |     VAR x : INT; p : REF_TO REAL; q : REF_TO INT; r : REF_TO REAL := REF(x); END_VAR
       |                                                                   ^^^^|^^^^
       |                                                                       `------ expected 'REF_TO REAL', got 'REF_TO INT'
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:10 ]
       |
     3 |     VAR x : INT; p : REF_TO REAL; q : REF_TO INT; r : REF_TO REAL := REF(x); END_VAR
       |                  |
       |                  `-- 'p' is declared here
     4 |     p := REF(x);
       |          ^^^|^^
       |             `---- expected 'REF_TO REAL', got 'REF_TO INT'
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:6:10 ]
       |
     3 |     VAR x : INT; p : REF_TO REAL; q : REF_TO INT; r : REF_TO REAL := REF(x); END_VAR
       |                  |
       |                  `-- 'p' is declared here
       |
     6 |     p := q;
       |          |
       |          `-- expected 'REF_TO REAL', got 'REF_TO INT'
    ---'
    ");
}

#[rstest]
fn a_reference_parameter_does_not_widen_its_pointee(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION g : REAL
    VAR_INPUT p : REF_TO REAL; END_VAR
    g := p^;
END_FUNCTION

FUNCTION_BLOCK fb
    VAR_INPUT p : REF_TO REAL; END_VAR
END_FUNCTION_BLOCK

FUNCTION t : REAL
    VAR x : INT; i : fb; END_VAR
    i(p := REF(x));
    t := g(p := REF(x));
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:13:12 ]
        |
      8 |     VAR_INPUT p : REF_TO REAL; END_VAR
        |               |
        |               `-- 'p' is declared here
        |
     13 |     i(p := REF(x));
        |            ^^^|^^
        |               `---- expected 'REF_TO REAL', got 'REF_TO INT'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:14:17 ]
        |
      3 |     VAR_INPUT p : REF_TO REAL; END_VAR
        |               |
        |               `-- 'p' is declared here
        |
     14 |     t := g(p := REF(x));
        |                 ^^^|^^
        |                    `---- expected 'REF_TO REAL', got 'REF_TO INT'
    ----'
    ");
}

#[rstest]
fn a_reference_binds_the_same_type_by_shape(mut with_db: RootDatabase) {
    // Invariance is structural, not identity: an alias, a same-shape array
    // and a same struct are the same type even as distinct specs.
    let source = r#"
TYPE MyInt : INT; END_TYPE
TYPE Pt : STRUCT x : INT; END_STRUCT; END_TYPE

FUNCTION f : INT
    VAR
        m : MyInt; a : ARRAY[0..2] OF INT; s : Pt; x : INT;
        pi : REF_TO INT; pm : REF_TO MyInt; pa : REF_TO ARRAY[0..2] OF INT; ps : REF_TO Pt;
        pp : REF_TO REF_TO INT;
    END_VAR
    pi := REF(m);
    pm := REF(x);
    pa := REF(a);
    ps := REF(s);
    pp := REF(pi);
    pi := NULL;
    f := pp^^;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn a_var_in_out_does_not_widen_the_callers_slot(mut with_db: RootDatabase) {
    // A FUNCTION's VAR_IN_OUT aliases the caller's storage: a REAL parameter
    // over an INT variable wrote four bytes over two and read garbage back
    // (executed: x was not 4 after `io := io + 1.0`). A block's scalar one is
    // a member, whose copy back would narrow a REAL to the INT, so the type
    // has to match there too. Interface-typed parameters are exempt — pinned
    // by the two tests that follow.
    let source = r#"
FUNCTION g : REAL
    VAR_IN_OUT io : REAL; END_VAR
    g := io;
END_FUNCTION

FUNCTION_BLOCK fb
    VAR_IN_OUT io : DINT; END_VAR
END_FUNCTION_BLOCK

FUNCTION t : REAL
    VAR x : INT; i : fb; r : REAL; END_VAR
    t := g(io := x);
    t := g(x);
    i(io := x);
    t := g(io := r);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:13:18 ]
        |
      3 |     VAR_IN_OUT io : REAL; END_VAR
        |                ^|
        |                 `-- 'io' is declared here
        |
     13 |     t := g(io := x);
        |                  |
        |                  `-- expected 'REAL', got 'INT'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:14:12 ]
        |
      3 |     VAR_IN_OUT io : REAL; END_VAR
        |                ^|
        |                 `-- 'io' is declared here
        |
     14 |     t := g(x);
        |            |
        |            `-- expected 'REAL', got 'INT'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:15:13 ]
        |
      8 |     VAR_IN_OUT io : DINT; END_VAR
        |                ^|
        |                 `-- 'io' is declared here
        |
     15 |     i(io := x);
        |             |
        |             `-- expected 'DINT', got 'INT'
    ----'
    ");
}

#[rstest]
fn an_interface_parameter_takes_any_implementer_by_reference(mut with_db: RootDatabase) {
    // The invariance rule must not reach interface-typed parameters: binding
    // an implementer is dispatch, not a reinterpretation of the caller's
    // slot. Named and positional, VAR_IN_OUT and VAR_INPUT.
    let source = r#"
INTERFACE I
    METHOD M : INT
    END_METHOD
END_INTERFACE

CLASS C IMPLEMENTS I
    METHOD M : INT
        M := 1;
    END_METHOD
END_CLASS

FUNCTION by_in_out : INT
    VAR_IN_OUT i : I; END_VAR
    by_in_out := i.M();
END_FUNCTION

FUNCTION by_input : INT
    VAR_INPUT i : I; END_VAR
    by_input := i.M();
END_FUNCTION

FUNCTION t : INT
    VAR c : C; END_VAR
    t := by_in_out(i := c);
    t := by_in_out(c);
    t := by_input(i := c);
    t := by_input(c);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn an_interface_parameter_still_requires_implements(mut with_db: RootDatabase) {
    // The exemption skips invariance, not the IMPLEMENTS check.
    let source = r#"
INTERFACE I
    METHOD M : INT
    END_METHOD
END_INTERFACE

CLASS NotC
    METHOD M : INT
        M := 1;
    END_METHOD
END_CLASS

FUNCTION by_in_out : INT
    VAR_IN_OUT i : I; END_VAR
    by_in_out := i.M();
END_FUNCTION

FUNCTION t : INT
    VAR n : NotC; END_VAR
    t := by_in_out(i := n);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:20:25 ]
        |
      2 | INTERFACE I
        |           |
        |           `-- INTERFACE 'I' is declared here
        |
     20 |     t := by_in_out(i := n);
        |                         |
        |                         `-- expected 'I', got 'NotC'
    ----'
    ");
}

/// A reference points at a STRING of one capacity: a write through a
/// `REF_TO STRING` reaching a `STRING[4]` clamped at 80, past it. Binding
/// one to the other is refused, like any other pointee type.
#[rstest]
fn a_string_reference_keeps_its_capacity(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : INT
VAR s4 : STRING[4]; r : REF_TO STRING; r4 : REF_TO STRING[4]; END_VAR
    r := REF(s4);
    r := r4;
    r4 := REF(s4);
    f := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:10 ]
       |
     3 | VAR s4 : STRING[4]; r : REF_TO STRING; r4 : REF_TO STRING[4]; END_VAR
       |                     |
       |                     `-- 'r' is declared here
     4 |     r := REF(s4);
       |          ^^^|^^^
       |             `----- expected 'REF_TO STRING', got 'REF_TO STRING[4]'
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:5:10 ]
       |
     3 | VAR s4 : STRING[4]; r : REF_TO STRING; r4 : REF_TO STRING[4]; END_VAR
       |                     |
       |                     `-- 'r' is declared here
       |
     5 |     r := r4;
       |          ^|
       |           `-- expected 'REF_TO STRING', got 'REF_TO STRING[4]'
    ---'
    ");
}

/// A reference in RETAIN storage is refused: a warm start restores its
/// address, and a new build may have moved what it pointed at. Whether a
/// member is kept follows the retain map: NON_RETAIN, VAR_TEMP and a
/// VAR_IN_OUT bound by reference are not, and a block's scalar VAR_IN_OUT,
/// a member, is kept as a VAR is.
#[rstest]
fn a_reference_in_retain_storage_is_refused(mut with_db: RootDatabase) {
    let source = r#"
TYPE PInt : REF_TO INT; END_TYPE
TYPE S : STRUCT a : INT; p : PInt; END_STRUCT; END_TYPE

FUNCTION_BLOCK H
VAR x : INT; p : PInt; END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Kept
VAR RETAIN q : PInt; END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Pruned
VAR x : INT; END_VAR
VAR NON_RETAIN p : PInt; END_VAR
VAR_IN_OUT io : INT; END_VAR
END_FUNCTION_BLOCK

PROGRAM P
VAR RETAIN
    r : PInt;
    s : S;
    arr : ARRAY[0..1] OF H;
    pruned : Pruned;
    n : INT;
END_VAR
END_PROGRAM

PROGRAM Q
VAR h : H; n : INT; END_VAR
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL RETAIN g : PInt; END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
        PROGRAM RETAIN Q1 WITH T : Q;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0904] Error: reference in RETAIN storage
        ,-[ file:///test0.st:10:12 ]
        |
     10 | VAR RETAIN q : PInt; END_VAR
        |            |
        |            `-- 'q' is a reference in RETAIN storage: its address does not survive a new build
        |
        | Help: move 'q' out of RETAIN and set it in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    [E0904] Error: reference in RETAIN storage
        ,-[ file:///test0.st:21:5 ]
        |
     21 |     r : PInt;
        |     |
        |     `-- 'r' is a reference in RETAIN storage: its address does not survive a new build
        |
        | Help: move 'r' out of RETAIN and set it in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    [E0904] Error: reference in RETAIN storage
        ,-[ file:///test0.st:22:5 ]
        |
     22 |     s : S;
        |     |
        |     `-- 's.p' is a reference in RETAIN storage: its address does not survive a new build
        |
        | Help: mark 'p' NON_RETAIN in 'S' and set it in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    [E0904] Error: reference in RETAIN storage
        ,-[ file:///test0.st:23:5 ]
        |
     23 |     arr : ARRAY[0..1] OF H;
        |     ^|^
        |      `--- 'arr[i].p' is a reference in RETAIN storage: its address does not survive a new build
        |
        | Help: mark 'p' NON_RETAIN in 'H' and set it in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    [E0904] Error: reference in RETAIN storage
        ,-[ file:///test0.st:34:19 ]
        |
     34 | VAR_GLOBAL RETAIN g : PInt; END_VAR
        |                   |
        |                   `-- 'g' is a reference in RETAIN storage: its address does not survive a new build
        |
        | Help: move 'g' out of RETAIN and set it in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    [E0904] Error: reference in RETAIN storage
        ,-[ file:///test0.st:38:24 ]
        |
     30 | VAR h : H; n : INT; END_VAR
        |     |
        |     `-- 'h' is declared here
        |
     38 |         PROGRAM RETAIN Q1 WITH T : Q;
        |                        ^|
        |                         `-- PROGRAM RETAIN 'Q1' keeps 'h.p', a reference: its address does not survive a new build
        |
        | Help: mark 'p' NON_RETAIN in 'H' and set it in the first scan
        |
        | Note: a warm start restores the address even where a new build moved its target
    ----'
    ");
}

/// A reference points at a named variable or instance: not at a literal, an
/// expression, a call's result, a direct address or a bit, in a body or an
/// initial value.
#[rstest]
fn invalid_reference_to_what_names_no_variable(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION G : INT
    G := 1;
END_FUNCTION

FUNCTION_BLOCK fb1
VAR
    i : INT;
    w : WORD;
    r : REF_TO INT;
    s : REF_TO STRING;
    b : REF_TO BOOL;
    pb : REF_TO BYTE;
    first : REF_TO INT := REF(5);
END_VAR
    s := REF('ab');
    r := REF(i + 1);
    r := REF(G());
    r := REF(%MW10);
    b := REF(w.%X3);
    pb := REF(w.%B1);
    r := REF(i);
END_FUNCTION_BLOCK
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0905] Error: REF() argument not a variable
        ,-[ file:///test0.st:14:31 ]
        |
     14 |     first : REF_TO INT := REF(5);
        |                               |
        |                               `-- REF() takes a variable, not a value
        |
        | Help: store the value in a variable, and take REF() of the variable
        |
        | Note: a literal, an expression or a call result has no storage to point at
    ----'
    [E0905] Error: REF() argument not a variable
        ,-[ file:///test0.st:16:14 ]
        |
     16 |     s := REF('ab');
        |              ^^|^
        |                `--- REF() takes a variable, not a value
        |
        | Help: store the value in a variable, and take REF() of the variable
        |
        | Note: a literal, an expression or a call result has no storage to point at
    ----'
    [E0905] Error: REF() argument not a variable
        ,-[ file:///test0.st:17:14 ]
        |
     17 |     r := REF(i + 1);
        |              ^^|^^
        |                `---- REF() takes a variable, not a value
        |
        | Help: store the value in a variable, and take REF() of the variable
        |
        | Note: a literal, an expression or a call result has no storage to point at
    ----'
    [E0905] Error: REF() argument not a variable
        ,-[ file:///test0.st:18:14 ]
        |
     18 |     r := REF(G());
        |              ^|^
        |               `--- REF() takes a variable, not a value
        |
        | Help: store the value in a variable, and take REF() of the variable
        |
        | Note: a literal, an expression or a call result has no storage to point at
    ----'
    [E0905] Error: REF() argument not a variable
        ,-[ file:///test0.st:19:14 ]
        |
     19 |     r := REF(%MW10);
        |              ^^|^^
        |                `---- REF() takes a variable, not a direct address
        |
        | Help: declare a variable AT the address, and take REF() of it
        |
        | Note: a reference points at a named variable or instance
    ----'
    [E0905] Error: REF() argument not a variable
        ,-[ file:///test0.st:20:14 ]
        |
     20 |     b := REF(w.%X3);
        |              ^^|^^
        |                `---- REF() takes a variable, not a slice of one
        |
        | Note: a slice of a variable has no address of its own, where a variable declared at whole bytes of an address has
    ----'
    [E0905] Error: REF() argument not a variable
        ,-[ file:///test0.st:21:15 ]
        |
     21 |     pb := REF(w.%B1);
        |               ^^|^^
        |                 `---- REF() takes a variable, not a slice of one
        |
        | Note: a slice of a variable has no address of its own, where a variable declared at whole bytes of an address has
    ----'
    ");
}

/// A reference is ordered by no operator: `<`, `<=`, `>` and `>=` on two
/// references, or on one and NULL, are E0305, which names the reference.
#[rstest]
fn a_reference_is_not_ordered(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Ordered : BOOL
VAR
    a, b : DINT;
    r1, r2 : REF_TO DINT;
END_VAR
    r1 := REF(a);
    r2 := REF(b);
    Ordered := r1 < r2;
    Ordered := r1 >= NULL;
    Ordered := NULL > r2;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0305] Error: operator not supported by the type
       ,-[ file:///test0.st:9:16 ]
       |
     5 |     r1, r2 : REF_TO DINT;
       |     ^|
       |      `-- 'r1' is declared here
       |
     9 |     Ordered := r1 < r2;
       |                ^^^|^^^
       |                   `----- operator '<' cannot be applied to type 'REF_TO DINT'
       |
       | Note: references can only be compared with '=' or '<>'
    ---'
    [E0305] Error: operator not supported by the type
        ,-[ file:///test0.st:10:16 ]
        |
      5 |     r1, r2 : REF_TO DINT;
        |     ^|
        |      `-- 'r1' is declared here
        |
     10 |     Ordered := r1 >= NULL;
        |                ^^^^^|^^^^
        |                     `------ operator '>=' cannot be applied to type 'REF_TO DINT'
        |
        | Note: references can only be compared with '=' or '<>'
    ----'
    [E0305] Error: operator not supported by the type
        ,-[ file:///test0.st:11:16 ]
        |
      5 |     r1, r2 : REF_TO DINT;
        |         ^|
        |          `-- 'r2' is declared here
        |
     11 |     Ordered := NULL > r2;
        |                ^^^^|^^^^
        |                    `------ operator '>' cannot be applied to type 'REF_TO DINT'
        |
        | Note: references can only be compared with '=' or '<>'
    ----'
    ");
}

/// `=` and `<>` are what a reference has, against another or NULL.
#[rstest]
fn a_reference_is_compared_for_equality(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Same : BOOL
VAR
    a : DINT;
    r1, r2 : REF_TO DINT;
END_VAR
    r1 := REF(a);
    r2 := r1;
    Same := r1 = r2 AND r1 <> NULL AND NULL <> r2;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A reference to what has an address of its own: a variable located at a
/// word, one located at a bit with no wider variable over it, and a place
/// reached through a reference.
#[rstest]
fn valid_a_reference_to_a_located_variable_or_through_a_reference(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pt : STRUCT x : REAL; y : REAL; END_STRUCT; END_TYPE

PROGRAM P
VAR
    m AT %MW10 : INT;
    flag AT %MX0.3 : BOOL;
    point : Pt;
    pm : REF_TO INT;
    pf : REF_TO BOOL;
    pp : REF_TO Pt;
    px : REF_TO REAL;
END_VAR
    pm := REF(m);
    pf := REF(flag);
    pp := REF(point);
    px := REF(pp^.x);
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// Two references are compared only when they point at the same type, and
/// a reference is not compared with a number.
#[rstest]
fn invalid_references_to_different_types_are_not_comparable(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pt : STRUCT x : REAL; y : REAL; END_STRUCT; END_TYPE

FUNCTION Same : BOOL
VAR
    point : Pt;
    n : INT;
    rp : REF_TO Pt;
    rx : REF_TO REAL;
    ri : REF_TO INT;
END_VAR
    rp := REF(point);
    rx := REF(point.x);
    ri := REF(n);
    Same := rp = rx;
    Same := ri = 0;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0302] Error: types not comparable
        ,-[ file:///test0.st:15:18 ]
        |
      8 |     rp : REF_TO Pt;
        |     ^|
        |      `-- 'rp' is declared here
        |
     15 |     Same := rp = rx;
        |                  ^|
        |                   `-- cannot compare 'REF_TO Pt' with 'REF_TO REAL'
    ----'
    [E0302] Error: types not comparable
        ,-[ file:///test0.st:16:18 ]
        |
     10 |     ri : REF_TO INT;
        |     ^|
        |      `-- 'ri' is declared here
        |
     16 |     Same := ri = 0;
        |                  |
        |                  `-- cannot compare 'REF_TO INT' with 'INT'
    ----'
    ");
}

/// A `REF()` of another type is refused on either side, and a reference has no order.
#[rstest]
fn invalid_comparisons_of_a_ref(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION F
VAR
    i : INT;
    x : REAL;
    r : REF_TO INT;
    b : BOOL;
END_VAR
    b := r = REF(x);
    b := REF(x) = r;
    b := REF(i) = REF(x);
    b := i = REF(i);
    b := r < REF(i);
    b := REF(i) < r;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0302] Error: types not comparable
       ,-[ file:///test0.st:9:14 ]
       |
     6 |     r : REF_TO INT;
       |     |
       |     `-- 'r' is declared here
       |
     9 |     b := r = REF(x);
       |              ^^^|^^
       |                 `---- cannot compare 'REF_TO INT' with 'REF_TO REAL'
    ---'
    [E0302] Error: types not comparable
        ,-[ file:///test0.st:10:19 ]
        |
      5 |     x : REAL;
        |     |
        |     `-- 'x' is declared here
        |
     10 |     b := REF(x) = r;
        |                   |
        |                   `-- cannot compare 'REF_TO INT' with 'REF_TO REAL'
    ----'
    [E0302] Error: types not comparable
        ,-[ file:///test0.st:11:19 ]
        |
      4 |     i : INT;
        |     |
        |     `-- 'i' is declared here
        |
     11 |     b := REF(i) = REF(x);
        |                   ^^^|^^
        |                      `---- cannot compare 'REF_TO INT' with 'REF_TO REAL'
    ----'
    [E0302] Error: types not comparable
        ,-[ file:///test0.st:12:14 ]
        |
      4 |     i : INT;
        |     |
        |     `-- 'i' is declared here
        |
     12 |     b := i = REF(i);
        |              ^^^|^^
        |                 `---- cannot compare 'INT' with 'REF_TO INT'
    ----'
    [E0305] Error: operator not supported by the type
        ,-[ file:///test0.st:13:10 ]
        |
      6 |     r : REF_TO INT;
        |     |
        |     `-- 'r' is declared here
        |
     13 |     b := r < REF(i);
        |          ^^^^^|^^^^
        |               `------ operator '<' cannot be applied to type 'REF_TO INT'
        |
        | Note: references can only be compared with '=' or '<>'
    ----'
    [E0305] Error: operator not supported by the type
        ,-[ file:///test0.st:14:10 ]
        |
      6 |     r : REF_TO INT;
        |     |
        |     `-- 'r' is declared here
        |
     14 |     b := REF(i) < r;
        |          ^^^^^|^^^^
        |               `------ operator '<' cannot be applied to type 'REF_TO INT'
        |
        | Note: references can only be compared with '=' or '<>'
    ----'
    ");
}

/// A `REF()` is typed as what it points at, and it is no value of that type. These
/// passed the check: address arithmetic for an INT, and for a REAL an invalid module.
#[rstest]
#[case::added_on_the_left("i := REF(i) + 1;", "REF_TO INT")]
#[case::added_on_the_right("i := 1 + REF(i);", "REF_TO INT")]
#[case::added_to_a_variable("i := i + REF(j);", "REF_TO INT")]
#[case::subtracted("i := REF(i) - 1;", "REF_TO INT")]
#[case::multiplied("i := REF(i) * 2;", "REF_TO INT")]
#[case::divided("x := REF(x) / 2.0;", "REF_TO REAL")]
#[case::modulo("i := REF(i) MOD 2;", "REF_TO INT")]
#[case::an_exponent("x := 2.0 ** REF(i);", "REF_TO INT")]
#[case::a_base("x := REF(x) ** 2;", "REF_TO REAL")]
#[case::negated("i := -REF(i);", "REF_TO INT")]
#[case::signed("i := +REF(i);", "REF_TO INT")]
#[case::inverted("b := NOT REF(b);", "REF_TO BOOL")]
#[case::anded("b := REF(b) AND TRUE;", "REF_TO BOOL")]
#[case::ored("b := TRUE OR REF(b);", "REF_TO BOOL")]
#[case::xored("b := REF(b) XOR b;", "REF_TO BOOL")]
#[case::a_selector("CASE REF(i) OF 1: j := 1; END_CASE;", "REF_TO INT")]
#[case::a_subscript("i := a[REF(j)];", "REF_TO INT")]
#[case::in_parentheses("i := (REF(i)) + 1;", "REF_TO INT")]
#[case::a_bit_string("w := REF(w) AND 16#FF;", "REF_TO WORD")]
#[case::inverted_in_a_condition("IF NOT REF(b) THEN i := 1; END_IF;", "REF_TO BOOL")]
#[case::a_written_subscript("a[REF(j)] := 1;", "REF_TO INT")]
#[case::beside_a_read("r := REF(j); i := r^ + REF(i);", "REF_TO INT")]
#[case::an_enum_selector("CASE REF(c) OF Color#Red: j := 1; END_CASE;", "REF_TO Color")]
#[case::a_type_with_named_values("i := REF(k) + 1;", "REF_TO Light")]
fn invalid_a_ref_as_a_value(mut with_db: RootDatabase, #[case] body: &str, #[case] got: &str) {
    let source = format!(
        "TYPE Color : (Red, Green); Light : INT (Stop := 1); END_TYPE\nFUNCTION F\nVAR i, j : INT; x : REAL; b : BOOL; a : ARRAY[0..1] OF INT; w : WORD; r : REF_TO INT; c : Color; k : Light; END_VAR\n{body}\nEND_FUNCTION\n"
    );
    let report = test_diagnostics(&mut with_db, &[&source]);
    assert_eq!(report.matches(" Error: ").count(), 1, "{report}");
    assert!(
        report.contains(&format!("expected a value, got '{got}'")),
        "{report}"
    );
}

/// In parentheses it is a reference all the same, and nothing else.
#[rstest]
fn a_ref_in_parentheses_is_a_reference(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION F
VAR
    i : INT;
    r : REF_TO INT;
    q : REF_TO INT := (REF(i));
END_VAR
    r := (REF(i));
    r := ((REF(i)));
    i := (REF(i));
    r := (REF(r));
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:10:10 ]
        |
      4 |     i : INT;
        |     |
        |     `-- 'i' is declared here
        |
     10 |     i := (REF(i));
        |          ^^^^|^^^
        |              `----- expected 'INT', got 'REF_TO INT'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:11:10 ]
        |
      5 |     r : REF_TO INT;
        |     |
        |     `-- 'r' is declared here
        |
     11 |     r := (REF(r));
        |          ^^^^|^^^
        |              `----- expected 'REF_TO INT', got 'REF_TO REF_TO INT'
    ----'
    ");
}

/// A `REF()` in an initial value is held to the same rule as in a body.
#[rstest]
#[case::added("k : INT := REF(i) + 1;", "expected a value, got 'REF_TO INT'")]
#[case::negated("k : INT := -REF(i);", "expected a value, got 'REF_TO INT'")]
#[case::inverted("k : BOOL := NOT REF(b);", "expected a value, got 'REF_TO BOOL'")]
#[case::in_a_struct("p : Pt := (a := REF(i) + 1);", "expected a value, got 'REF_TO INT'")]
#[case::in_an_array(
    "l : ARRAY[0..1] OF INT := [REF(i) + 1];",
    "expected a value, got 'REF_TO INT'"
)]
#[case::in_parentheses("k : INT := (REF(i));", "expected 'INT', got 'REF_TO INT'")]
#[case::against_a_value("k : BOOL := REF(i) = i;", "cannot compare 'INT' with 'REF_TO INT'")]
fn invalid_a_ref_in_an_initial_value(
    mut with_db: RootDatabase,
    #[case] decl: &str,
    #[case] message: &str,
) {
    let source = format!(
        "TYPE Pt : STRUCT a : INT; END_STRUCT; END_TYPE\nFUNCTION F\nVAR i : INT; b : BOOL; {decl} END_VAR\nEND_FUNCTION\n"
    );
    let report = test_diagnostics(&mut with_db, &[&source]);
    assert!(report.contains(message), "{report}");
}

/// A pack is read by an integer, as an array is.
#[rstest]
fn invalid_a_ref_reading_a_pack(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION F : INT
VAR_INPUT v : INT...; END_VAR
VAR i : INT; END_VAR
    F := ...v[REF(i)];
END_FUNCTION
"#;
    let report = test_diagnostics(&mut with_db, &[source]);
    assert!(
        report.contains("expected a value, got 'REF_TO INT'"),
        "{report}"
    );
}

/// What each place says of a `REF()` it has no use for, and two `REF()`s that do
/// not compare: named as references, with no order between them.
#[rstest]
#[case::a_variable("i := REF(j);", "[E0301]", "expected 'INT', got 'REF_TO INT'")]
#[case::a_variable_in_parentheses("i := (REF(j));", "[E0301]", "expected 'INT', got 'REF_TO INT'")]
#[case::a_struct("pt := REF(pt);", "[E0301]", "expected 'Pt', got 'REF_TO Pt'")]
#[case::a_condition(
    "IF REF(b) THEN i := 1; END_IF;",
    "[E0301]",
    "expected 'BOOL', got 'REF_TO BOOL'"
)]
#[case::a_while(
    "WHILE REF(b) DO EXIT; END_WHILE;",
    "[E0301]",
    "expected 'BOOL', got 'REF_TO BOOL'"
)]
#[case::a_repeat(
    "REPEAT i := 1; UNTIL REF(b) END_REPEAT;",
    "[E0301]",
    "expected 'BOOL', got 'REF_TO BOOL'"
)]
#[case::a_start(
    "FOR i := REF(j) TO 3 DO j := 1; END_FOR;",
    "[E0301]",
    "expected 'INT', got 'REF_TO INT'"
)]
#[case::a_final_value(
    "FOR i := 1 TO REF(j) DO j := 1; END_FOR;",
    "[E1214]",
    "the final value is 'REF_TO INT'"
)]
#[case::a_step(
    "FOR i := 1 TO 3 BY REF(j) DO j := 1; END_FOR;",
    "[E1214]",
    "the step is 'REF_TO INT'"
)]
#[case::a_label(
    "CASE i OF REF(j): j := 1; END_CASE;",
    "[E1205]",
    "the label is not a compile-time constant"
)]
#[case::an_argument(
    "i := takes_int(REF(i));",
    "[E0301]",
    "expected 'INT', got 'REF_TO INT'"
)]
#[case::a_named_argument(
    "i := takes_int(p := REF(i));",
    "[E0301]",
    "expected 'INT', got 'REF_TO INT'"
)]
#[case::a_variadic("i := sum(REF(i), 1);", "[E0301]", "expected 'INT', got 'REF_TO INT'")]
#[case::a_blocks_input("fb(in := REF(i));", "[E0301]", "expected 'INT', got 'REF_TO INT'")]
#[case::an_in_out("io_int(REF(i));", "[E0806]", "requires a variable, not a value")]
#[case::a_message(
    "__RAISE(REF(s));",
    "[E0301]",
    "expected 'STRING', got 'REF_TO STRING'"
)]
#[case::written_through("r^ := REF(i);", "[E0301]", "expected 'INT', got 'REF_TO INT'")]
#[case::a_value_on_the_left(
    "b := i = REF(i);",
    "[E0302]",
    "cannot compare 'INT' with 'REF_TO INT'"
)]
#[case::a_value_on_the_right(
    "b := REF(i) = i;",
    "[E0302]",
    "cannot compare 'INT' with 'REF_TO INT'"
)]
#[case::a_number(
    "b := REF(i) = 5;",
    "[E0302]",
    "cannot compare 'INT' with 'REF_TO INT'"
)]
#[case::two_types(
    "b := REF(i) = REF(x);",
    "[E0302]",
    "cannot compare 'REF_TO INT' with 'REF_TO REAL'"
)]
#[case::two_types_ordered(
    "b := REF(i) < REF(x);",
    "[E0302]",
    "cannot compare 'REF_TO INT' with 'REF_TO REAL'"
)]
#[case::two_capacities(
    "b := REF(s) = REF(s4);",
    "[E0302]",
    "cannot compare 'REF_TO STRING' with 'REF_TO STRING[4]'"
)]
#[case::a_capacity_and_a_reference("b := rs4 = REF(s);", "[E0302]", "with 'REF_TO STRING'")]
#[case::named_values_and_a_number(
    "b := REF(k) = REF(i);",
    "[E0302]",
    "cannot compare 'REF_TO Light' with 'REF_TO INT'"
)]
#[case::in_order(
    "b := REF(i) < REF(j);",
    "[E0305]",
    "operator '<' cannot be applied to type 'REF_TO INT'"
)]
#[case::in_order_of_or_equal(
    "b := REF(i) >= REF(j);",
    "[E0305]",
    "operator '>=' cannot be applied to type 'REF_TO INT'"
)]
#[case::in_order_in_parentheses(
    "b := (REF(i)) < (REF(j));",
    "[E0305]",
    "operator '<' cannot be applied to type 'REF_TO INT'"
)]
#[case::in_order_of_elements(
    "b := REF(a[0]) < REF(a[1]);",
    "[E0305]",
    "operator '<' cannot be applied to type 'REF_TO INT'"
)]
#[case::of_a_ref("r := REF(REF(i));", "[E0905]", "")]
#[case::of_a_literal("r := REF(1);", "[E0905]", "")]
#[case::of_a_calculation("r := REF(i + 1);", "[E0905]", "")]
fn invalid_a_ref_where_it_has_no_use(
    mut with_db: RootDatabase,
    #[case] body: &str,
    #[case] code: &str,
    #[case] message: &str,
) {
    let source = format!(
        r#"
TYPE
    Pt : STRUCT a : INT; END_STRUCT;
    Light : INT (Stop := 1);
END_TYPE

FUNCTION_BLOCK Fb
VAR_INPUT in : INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION takes_int : INT
VAR_INPUT p : INT; END_VAR
    takes_int := p;
END_FUNCTION

FUNCTION io_int
VAR_IN_OUT p : INT; END_VAR
    p := 1;
END_FUNCTION

FUNCTION sum : INT
VAR_INPUT v : INT...; END_VAR
    sum := ...v +;
END_FUNCTION

FUNCTION F
VAR
    i, j : INT;
    k : Light;
    x : REAL;
    b : BOOL;
    s : STRING;
    s4 : STRING[4];
    a : ARRAY[0..1] OF INT;
    pt : Pt;
    fb : Fb;
    r : REF_TO INT;
    rs4 : REF_TO STRING[4];
END_VAR
    {body}
END_FUNCTION
"#
    );
    let report = test_diagnostics(&mut with_db, &[&source]);
    assert!(report.contains(code), "{report}");
    assert!(report.contains(message), "{report}");
}
