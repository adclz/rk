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

// valid implicit cast case with addition and comparison
#[rstest]
fn valid_infer_implicit_cast(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1
	IF 1.0 + 5 = 6.0 THEN

	END_IF
        END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// variables takes priority over unknown types
#[rstest]
fn invalid_infer_with_variable(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
	VAR
		test: BOOL;
	END_VAR

	IF 0.0 + test = 6 THEN

	END_IF;

END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0305] Error: type mismatch
       ,-[ file:///test0.st:7:5 ]
       |
     4 |        test: BOOL;
       |        ^^|^
       |          `--- type is declared by variable 'test' here
       |
     7 |     IF 0.0 + test = 6 THEN
       |        ^^^^^|^^^^
       |             `------ operator '+' cannot be applied to type 'BOOL'
    ---'
    [E0308] Error: invalid literal
       ,-[ file:///test0.st:7:5 ]
       |
     7 |     IF 0.0 + test = 6 THEN
       |        ^|^   ^^|^
       |         `---------- cannot infer '<float>' to 'BOOL': invalid boolean literal; BOOL is TRUE or FALSE
       |                |
       |                `--- 'BOOL' is expected due to this
    ---'
    ");
}
