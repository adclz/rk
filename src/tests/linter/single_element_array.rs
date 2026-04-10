use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn equal_bounds(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    arr : ARRAY[5..5] OF INT;
END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "single-element-array"), @r"
    [L0110] Info: single-element array
       ,-[ file:///test0.st:4:17 ]
       |
     4 |     arr : ARRAY[5..5] OF INT;
       |                 |
       |                 `-- array dimension 1 has equal bounds (5..5), contains only one element
       |
       | Note: lint rule: single-element-array
    ---'
    ");
}

#[rstest]
fn equal_bounds_zero(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    arr : ARRAY[0..0] OF INT;
END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "single-element-array"), @r"
    [L0110] Info: single-element array
       ,-[ file:///test0.st:4:17 ]
       |
     4 |     arr : ARRAY[0..0] OF INT;
       |                 |
       |                 `-- array dimension 1 has equal bounds (0..0), contains only one element
       |
       | Note: lint rule: single-element-array
    ---'
    ");
}

#[rstest]
fn normal_array_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    arr : ARRAY[0..10] OF INT;
END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "single-element-array"), @r"");
}

#[rstest]
fn multi_dim_one_equal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    arr : ARRAY[0..5, 3..3] OF INT;
END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "single-element-array"), @r"
    [L0110] Info: single-element array
       ,-[ file:///test0.st:4:23 ]
       |
     4 |     arr : ARRAY[0..5, 3..3] OF INT;
       |                       |
       |                       `-- array dimension 2 has equal bounds (3..3), contains only one element
       |
       | Note: lint rule: single-element-array
    ---'
    ");
}

#[rstest]
fn one_based_normal_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    arr : ARRAY[1..10] OF INT;
END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "single-element-array"), @r"");
}
