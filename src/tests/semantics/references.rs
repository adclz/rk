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
    Error: 
       ,-[ file:///test0.st:5:31 ]
       |
     5 |             test2: REF_TO INT := REF(test); // Reference to INT, but test is UINT
       |                               ^^^^^^|^^^^^  
       |                                     `------- expected 'REF TO INT', got 'REF TO UINT'
    ---'
    ");
}

#[rstest]
fn valid_deref_int(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
	VAR
		test: REF_TO INT;
	END_VAR

	test^ := 0;

END_FUNCTION_BLOCK

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
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
    Error: 
       ,-[ file:///test0.st:8:31 ]
       |
     8 |             test2: REF_TO fb1 := REF(test); // Reference to fb1, but test is fb2
       |                               ^^^^^^|^^^^^  
       |                                     `------- expected 'REF TO FUNCTION_BLOCK: fb1', got 'REF TO FUNCTION_BLOCK: fb2'
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
    Error: 
       ,-[ file:///test0.st:7:17 ]
       |
     4 |             test: REF_TO INT;
       |             ^^|^  
       |               `--- type is declared by variable 'test' here
       | 
     7 |         test := 0;
       |                 |  
       |                 `-- expected 'REF TO INT', got 'INT'
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
fn valid_assign_value_to_deref_type(mut with_db: RootDatabase) {
    let source = r#"
    FUNCTION_BLOCK fn1
        VAR
            test: REF_TO INT;
        END_VAR

        test^ := 0;

    END_FUNCTION_BLOCK
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
    Error: 
       ,-[ file:///test0.st:9:11 ]
       |
     6 |        myInt: INT;
       |        ^^|^^  
       |          `---- type is declared by variable 'myInt' here
       | 
     9 |     myInt := REF(myA1[2][9]);
       |              ^^^^^^^|^^^^^^^  
       |                     `--------- expected 'INT', got 'REF TO INT'
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
