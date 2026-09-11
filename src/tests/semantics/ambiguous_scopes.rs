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
    [E0205] Error: multiple items in scope
        ,-[ file:///test0.st:17:21 ]
        |
     17 |             test := SharedName();
        |                     ^^^^^|^^^^
        |                          `------ multiple items named 'SharedName' available in scope
        |
        | Note: qualify the name to resolve the ambiguity: ns1.SharedName or ns2.SharedName
    ----'
    ");
}

/// Same-name FUNCTIONs reachable through ONE namespace path are an overload
/// set, not an ambiguity — so the call resolves and only the genuine defect
/// (identical signatures = duplicate definitions) is reported. Contrast with
/// `ambiguous_using_same_name`, where the matches come from DIFFERENT paths.
#[rstest]
fn duplicate_in_same_namespace_is_one_error_not_ambiguity(mut with_db: RootDatabase) {
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
    [E0102] Error: duplicate definitions
       ,-[ file:///test0.st:9:22 ]
       |
     3 |             FUNCTION SharedName : INT
       |                      ^^^^^|^^^^
       |                           `------ POU 'SharedName' is already defined here
       |
     9 |             FUNCTION SharedName : INT
       |                      ^^^^^|^^^^
       |                           `------ duplicate POU 'SharedName'
    ---'
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

#[rstest]
fn ambiguous_using_same_name_in_spec(mut with_db: RootDatabase) {
    let source = r#"
        NAMESPACE ns1
            FUNCTION_BLOCK SharedFB
            END_FUNCTION_BLOCK
        END_NAMESPACE

        NAMESPACE ns2
            FUNCTION_BLOCK SharedFB
            END_FUNCTION_BLOCK
        END_NAMESPACE

        FUNCTION test : INT
            USING ns1;
            USING ns2;
        VAR
            fb : SharedFB;
        END_VAR
            test := 0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0205] Error: multiple items in scope
        ,-[ file:///test0.st:16:18 ]
        |
     16 |             fb : SharedFB;
        |                  ^^^^|^^^
        |                      `----- multiple items named 'SharedFB' available in scope
        |
        | Note: qualify the name to resolve the ambiguity: ns1.SharedFB or ns2.SharedFB
    ----'
    ");
}
