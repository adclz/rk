use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn valid_case_int(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
    END_VAR

    CASE x OF
        1: y := 10;
        2: y := 20;
        3: y := 30;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_case_with_subrange(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
    END_VAR

    CASE x OF
        1..5: y := 10;
        6..10: y := 20;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_case_with_multiple_labels(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
    END_VAR

    CASE x OF
        1, 2, 3: y := 10;
        4, 5, 6: y := 20;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_case_label_type_mismatch(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
    END_VAR

    CASE x OF
        'hello': y := 10;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0308] Error: invalid literal
       ,-[ file:///test0.st:9:9 ]
       |
     9 |         'hello': y := 10;
       |         ^^^|^^^
       |            `----- cannot infer '<string>' to 'INT': cannot use string literal as INT
    ---'
    ");
}

#[rstest]
fn valid_case_string_label(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : STRING;
        y : INT;
    END_VAR

    CASE x OF
        'a': y := 1;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    // A string literal is a constant expression, so it is a legal label —
    // and it compares by content, like `=` on STRINGs. (See
    // codegen::control_flow::case_string_labels_compare_by_content for the
    // execution side; this only pins that it checks clean.)
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_case_subrange_type_mismatch(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
    END_VAR

    CASE x OF
        'a'..'z': y := 10;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1205] Error: control flow violation
       ,-[ file:///test0.st:9:9 ]
       |
     9 |         'a'..'z': y := 10;
       |         ^|^
       |          `--- a CASE range bound must be an integer constant
    ---'
    [E1205] Error: control flow violation
       ,-[ file:///test0.st:9:14 ]
       |
     9 |         'a'..'z': y := 10;
       |              ^|^
       |               `--- a CASE range bound must be an integer constant
    ---'
    [E0308] Error: invalid literal
       ,-[ file:///test0.st:9:9 ]
       |
     9 |         'a'..'z': y := 10;
       |         ^|^
       |          `--- cannot infer '<string>' to 'INT': cannot use string literal as INT
    ---'
    [E0308] Error: invalid literal
       ,-[ file:///test0.st:9:14 ]
       |
     9 |         'a'..'z': y := 10;
       |              ^|^
       |               `--- cannot infer '<string>' to 'INT': cannot use string literal as INT
    ---'
    ");
}

#[rstest]
fn valid_case_with_body_type_check(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
    END_VAR

    CASE x OF
        1: y := 10;
        2: y := 'bad';
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0308] Error: invalid literal
        ,-[ file:///test0.st:10:17 ]
        |
      5 |         y : INT;
        |         |
        |         `-- type is declared by variable 'y' here
        |
     10 |         2: y := 'bad';
        |                 ^^|^^
        |                   `---- cannot infer '<string>' to 'INT': cannot use string literal as INT
    ----'
    ");
}

#[rstest]
fn valid_case_nested(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
        z : INT;
    END_VAR

    CASE x OF
        1:
            CASE y OF
                10: z := 100;
                20: z := 200;
            ELSE
                z := 0;
            END_CASE;
        2: z := 2;
    ELSE
        z := -1;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_case_mixed_labels_and_subranges(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
    END_VAR

    CASE x OF
        1, 2, 10..20: y := 10;
        3: y := 30;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A CASE label picks a branch at compile time, so a VARIABLE cannot be one —
/// IEC's `Case_List_Element` is a signed integer, a subrange, or an enum
/// value. This used to check clean and then abort `rk compile` with an
/// internal compiler error.
#[rstest]
fn invalid_case_with_variable_label(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
        LIMIT : INT := 100;
    END_VAR

    CASE x OF
        LIMIT: y := 1;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1205] Error: control flow violation
        ,-[ file:///test0.st:10:9 ]
        |
     10 |         LIMIT: y := 1;
        |         ^^|^^
        |           `---- a CASE label must evaluate to a constant at compile time
    ----'
    ");
}

#[rstest]
fn invalid_case_else_body_type_mismatch(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
    END_VAR

    CASE x OF
        1: y := 10;
    ELSE
        y := 'wrong';
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0308] Error: invalid literal
        ,-[ file:///test0.st:11:14 ]
        |
      5 |         y : INT;
        |         |
        |         `-- type is declared by variable 'y' here
        |
     11 |         y := 'wrong';
        |              ^^^|^^^
        |                 `----- cannot infer '<string>' to 'INT': cannot use string literal as INT
    ----'
    ");
}

/// A range is an ordering, so its bounds have to be orderable numbers. A
/// string can be a label on its own but not the end of a range: `'a'..'z'`
/// denotes a lexicographic set the compiler has no representation for, and it
/// used to check clean and then abort `rk compile`.
#[rstest]
fn invalid_case_string_range_bounds(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        s : STRING;
        y : INT;
    END_VAR

    CASE s OF
        'a'..'z': y := 1;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1205] Error: control flow violation
       ,-[ file:///test0.st:9:9 ]
       |
     9 |         'a'..'z': y := 1;
       |         ^|^
       |          `--- a CASE range bound must be an integer constant
    ---'
    [E1205] Error: control flow violation
       ,-[ file:///test0.st:9:14 ]
       |
     9 |         'a'..'z': y := 1;
       |              ^|^
       |               `--- a CASE range bound must be an integer constant
    ---'
    ");
}
