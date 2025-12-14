use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// valid case of all "untyped" integers with addition and comparison
#[rstest]
fn valid_infer_int_comp(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1
	IF 1 + 5 = 6 THEN

	END_IF
        END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// valid case of all "untyped" reals with addition and comparison
#[rstest]
fn valid_infer_real_comp(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1
	IF 1.0 + 5.0 = 6.0 THEN

	END_IF
        END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// invalid case of int and real
// 1 emits the inference type (int)
#[rstest]
fn invalid_infer_int_real_comp(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1
	IF 1 + 5.0 = 6 THEN

	END_IF
        END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Advice: 
       ,-[ file:///test0.st:3:9 ]
       |
     3 |     IF 1 + 5.0 = 6 THEN
       |        |   ^|^  
       |        `-------- type is inferred from here
       |             |   
       |             `--- cannot infer type '(REAL) 5.0' to 'LINT'
    ---'
    ");
}

// variables takes priority over unknown types
#[rstest]
fn invalid_infer_with_variable(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
	VAR
		test: LINT;
	END_VAR

	IF 0.0 + test = 6 THEN

	END_IF;

END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Advice: 
       ,-[ file:///test0.st:7:5 ]
       |
     4 |        test: LINT;
       |        ^^|^  
       |          `--- type is declared by variable 'test' here
       | 
     7 |     IF 0.0 + test = 6 THEN
       |        ^|^   ^^|^  
       |         `---------- cannot infer type '(REAL) 0.0' to 'LINT'
       |                |   
       |                `--- type is inferred from here
    ---'
    ");
}
