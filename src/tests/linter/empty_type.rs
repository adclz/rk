use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn empty_struct(mut with_db: RootDatabase) {
    let source = r#"
TYPE EmptyStruct : STRUCT
END_STRUCT;
END_TYPE
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-type"), @r"
    [L0214] Hint: empty type declaration
       ,-[ file:///test0.st:2:6 ]
       |
     2 | TYPE EmptyStruct : STRUCT
       |      ^^^^^|^^^^^
       |           `------- STRUCT 'EmptyStruct' has no fields
       |
       | Note: lint rule: empty-type
    ---'
    ");
}

#[rstest]
fn empty_enum(mut with_db: RootDatabase) {
    let source = r#"
TYPE EmptyEnum : ();
END_TYPE
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-type"), @r"
    [L0214] Hint: empty type declaration
       ,-[ file:///test0.st:2:6 ]
       |
     2 | TYPE EmptyEnum : ();
       |      ^^^^|^^^^
       |          `------ ENUM 'EmptyEnum' has no variants
       |
       | Note: lint rule: empty-type
    ---'
    ");
}

#[rstest]
fn non_empty_struct(mut with_db: RootDatabase) {
    let source = r#"
TYPE MyStruct : STRUCT
    x : INT;
END_STRUCT;
END_TYPE
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-type"), @r"");
}

#[rstest]
fn non_empty_enum(mut with_db: RootDatabase) {
    let source = r#"
TYPE Color : (Red, Green, Blue);
END_TYPE
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-type"), @r"");
}
