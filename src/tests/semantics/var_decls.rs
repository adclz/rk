use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn function_as_var_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn

END_FUNCTION

FUNCTION_BLOCK fb1
    VAR
        test: fn;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0215] Error: invalid variable type
       ,-[ file:///test0.st:8:15 ]
       |
     8 |         test: fn;
       |               ^|  
       |                `-- 'FUNCTION: fn' is a function and cannot be used as a variable type
    ---'
    ");
}
