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
       |               `--- type is declared by variable 'test' here
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
       |          `---- type is declared by variable 'myInt' here
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
       |          `--- type is declared by variable 'arr' here
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
       |             `---- FUNCTION 'borrow' is defined here, with return type 'PInt'
       |
     6 |     borrow := REF(r);
       |               ^^^|^^
       |                  `---- expected 'PInt', got 'REF_TO REAL'
    ---'
    [E0251] Error: reference outlives its storage
       ,-[ file:///test0.st:6:15 ]
       |
     5 | VAR r : REAL; END_VAR
       |     ^^^^|^^^
       |         `----- 'r' is per-call storage, declared here
     6 |     borrow := REF(r);
       |               ^^^|^^
       |                  `---- reference to 'r' outlives the call that owns it
       |
       | Note: return a reference to instance state, or to storage the caller owns (a VAR_IN_OUT)
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
    [E0251] Error: reference outlives its storage
       ,-[ file:///test0.st:6:15 ]
       |
     5 | VAR local : INT := 1; END_VAR
       |     ^^^^^^^^|^^^^^^^
       |             `--------- 'local' is per-call storage, declared here
     6 |     borrow := REF(local);
       |               ^^^^^|^^^^
       |                    `------ reference to 'local' outlives the call that owns it
       |
       | Note: return a reference to instance state, or to storage the caller owns (a VAR_IN_OUT)
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
    [E0251] Error: reference outlives its storage
       ,-[ file:///test0.st:7:17 ]
       |
     6 |     VAR local : INT; END_VAR
       |         ^^^^^|^^^^^
       |              `------- 'local' is per-call storage, declared here
     7 |         slot := REF(local);
       |                 ^^^^^|^^^^
       |                      `------ reference to 'local' outlives the call that owns it
       |
       | Note: return a reference to instance state, or to storage the caller owns (a VAR_IN_OUT)
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
       |                  `-- type is declared by variable 'p' here
     4 |     p := REF(x);
       |          ^^^|^^
       |             `---- expected 'REF_TO REAL', got 'REF_TO INT'
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:6:10 ]
       |
     3 |     VAR x : INT; p : REF_TO REAL; q : REF_TO INT; r : REF_TO REAL := REF(x); END_VAR
       |                  |
       |                  `-- type is declared by variable 'p' here
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
        |               `-- type is declared by variable 'p' here
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
        |               `-- type is declared by variable 'p' here
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
    // A VAR_IN_OUT aliases the caller's storage; a REAL parameter over an INT
    // variable wrote four bytes over two and read garbage back (executed:
    // x was not 4 after `io := io + 1.0`). Interface-typed parameters are
    // exempt — pinned by the two tests that follow.
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
        |                 `-- type is declared by variable 'io' here
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
        |                 `-- type is declared by variable 'io' here
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
        |                 `-- type is declared by variable 'io' here
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
        |           `-- INTERFACE 'I' is defined here
        |
     20 |     t := by_in_out(i := n);
        |                         |
        |                         `-- expected 'I', got 'NotC'
    ----'
    ");
}
