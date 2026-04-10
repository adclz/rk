use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn once_called_twice(mut with_db: RootDatabase) {
    let source = r#"
{once}
FUNCTION setup : INT
    setup := 1;
END_FUNCTION

FUNCTION main : INT
    setup();
    setup();
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "once-violation"), @r"
    [L0402] Info: calling a {once} function more than once
       ,-[ file:///test0.st:9:5 ]
       |
     2 | ,-> {once}
       : :
     5 | |-> END_FUNCTION
       | |
       | `------------------ {once} pragma declared here
       |
     8 |         setup();
       |         ^^|^^
       |           `---- first call here
     9 |         setup();
       |         ^^|^^
       |           `---- 'setup' is marked {once} but is called more than once in this body
       | |
       | |   Note: lint rule: once-violation
    ---'
    ");
}

#[rstest]
fn once_called_once_no_warning(mut with_db: RootDatabase) {
    let source = r#"
{once}
FUNCTION setup : INT
    setup := 1;
END_FUNCTION

FUNCTION main : INT
    setup();
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "once-violation"), @r"");
}

#[rstest]
fn non_once_called_twice_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION helper : INT
    helper := 1;
END_FUNCTION

FUNCTION main : INT
    helper();
    helper();
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "once-violation"), @r"");
}

#[rstest]
fn once_method_called_twice(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
    {once}
    METHOD PUBLIC init : INT
        init := 1;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION main : INT
VAR fb : MyFB; END_VAR
    fb.init();
    fb.init();
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "once-violation"), @r"
    [L0402] Info: calling a {once} function more than once
        ,-[ file:///test0.st:11:8 ]
        |
      3 | ,->     {once}
        : :
      6 | |->     END_METHOD
        | |
        | `-------------------- {once} pragma declared here
        |
     11 |         fb.init();
        |            ^^|^
        |              `--- 'init' is marked {once} but is called more than once in this body
     12 |         fb.init();
        |            ^^|^
        |              `--- first call here
        |
        |     Note: lint rule: once-violation
    ----'
    ");
}

#[rstest]
fn once_method_called_once_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
    {once}
    METHOD PUBLIC init : INT
        init := 1;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION main : INT
VAR fb : MyFB; END_VAR
    fb.init();
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "once-violation"), @r"");
}
