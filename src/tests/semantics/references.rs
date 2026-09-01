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
    ");
}
