use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

#[rstest]
fn ambiguous_using_same_name(mut with_db: RootDatabase) {
    let source = r#"
        NAMESPACE ns1
            FUNCTION SharedName : INT
                SharedName := 0;
            END_FUNCTION
        END_NAMESPACE

        NAMESPACE ns2
            FUNCTION SharedName : INT
                SharedName := 0;
            END_FUNCTION
        END_NAMESPACE

        FUNCTION test : INT
            USING ns1;
            USING ns2;
            test := SharedName();
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0225] Error: multiple items in scope
        ,-[ file:///test0.st:17:21 ]
        |
     17 |             test := SharedName();
        |                     ^^^^^|^^^^
        |                          `------ multiple items named 'SharedName' available in scope:
        |
        | Note: qualify the name to resolve the ambiguity: ns1.SharedName or ns2.SharedName
    ----'
    ");
}

#[rstest]
fn ambiguous_using_duplicate_in_same_namespace(mut with_db: RootDatabase) {
    let source = r#"
        NAMESPACE ns
            FUNCTION SharedName : INT
                SharedName := 0;
            END_FUNCTION
        END_NAMESPACE

        NAMESPACE ns
            FUNCTION SharedName : INT
                SharedName := 0; // will not trigger an error because it is a self reference
            END_FUNCTION
        END_NAMESPACE

        FUNCTION test : INT
            USING ns;
            test := SharedName();
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0101] Error: duplicate definitions
       ,-[ file:///test0.st:3:22 ]
       |
     3 |             FUNCTION SharedName : INT
       |                      ^^^^^|^^^^
       |                           `------ duplicate POU 'SharedName'
       |
     9 |             FUNCTION SharedName : INT
       |                      ^^^^^|^^^^
       |                           `------ POU 'SharedName' is already defined here
    ---'
    [E0225] Error: multiple items in scope
        ,-[ file:///test0.st:16:21 ]
        |
     16 |             test := SharedName();
        |                     ^^^^^|^^^^
        |                          `------ multiple items named 'SharedName' available in scope:
        |
        | Note: 'SharedName' is declared multiple times in namespace 'ns', fix the duplicate declaration first
    ----'
    ");
}

#[rstest]
fn ambiguous_using_qualified_no_error(mut with_db: RootDatabase) {
    let source = r#"
        NAMESPACE ns1
            FUNCTION SharedName : INT
                SharedName := 0;
            END_FUNCTION
        END_NAMESPACE

        NAMESPACE ns2
            FUNCTION SharedName : INT
                SharedName := 0;
            END_FUNCTION
        END_NAMESPACE

        FUNCTION test : INT
            USING ns1;
            USING ns2;
            test := ns1.SharedName();
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn ambiguous_using_not_used_no_error(mut with_db: RootDatabase) {
    let source = r#"
        NAMESPACE ns1
            FUNCTION SharedName : INT
                SharedName := 0;
            END_FUNCTION
        END_NAMESPACE

        NAMESPACE ns2
            FUNCTION SharedName : INT
                SharedName := 0;
            END_FUNCTION
        END_NAMESPACE

        FUNCTION test : INT
            USING ns1;
            USING ns2;
            test := 0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn single_using_no_ambiguity(mut with_db: RootDatabase) {
    let source = r#"
        NAMESPACE ns1
            FUNCTION SharedName : INT
                SharedName := 0;
            END_FUNCTION
        END_NAMESPACE

        FUNCTION test : INT
            USING ns1;
            test := SharedName();
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn local_pou_wins_over_using(mut with_db: RootDatabase) {
    let source = r#"
        NAMESPACE ns1
            FUNCTION SharedName : INT
                SharedName := 0;
            END_FUNCTION
        END_NAMESPACE

        NAMESPACE ns2
            FUNCTION SharedName : INT
                SharedName := 0;
            END_FUNCTION

            FUNCTION test : INT
                USING ns1;
                test := SharedName();
            END_FUNCTION
        END_NAMESPACE
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}
