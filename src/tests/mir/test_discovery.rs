use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::with_db;
use super::utils::mir_exports;

#[rstest]
fn namespace_qualified_export_names(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Std.Math
    FUNCTION ADD : INT
    VAR_INPUT a : INT; b : INT; END_VAR
        ADD := a + b;
    END_FUNCTION
END_NAMESPACE
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"export Std.Math.ADD(Int, Int) -> Int");
}

#[rstest]
fn test_pragma_qualified_name(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Std.Math
    NAMESPACE Test
        {test}
        FUNCTION test_add
        VAR x : INT; END_VAR
            x := 1;
        END_FUNCTION
    END_NAMESPACE
END_NAMESPACE
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"export Std.Math.Test.test_add()");
}

#[rstest]
fn nested_namespace_path(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE A
    NAMESPACE B
        NAMESPACE C
            FUNCTION deep_fn : INT
            VAR_INPUT x : INT; END_VAR
                deep_fn := x;
            END_FUNCTION
        END_NAMESPACE
    END_NAMESPACE
END_NAMESPACE
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"export A.B.C.deep_fn(Int) -> Int");
}

#[rstest]
fn global_function_no_namespace_prefix(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION global_fn : INT
VAR_INPUT x : INT; END_VAR
    global_fn := x;
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"export global_fn(Int) -> Int");
}

#[rstest]
fn multiple_test_functions_in_namespace(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Std.Math
    FUNCTION ABS : INT
    VAR_INPUT IN : INT; END_VAR
        IF IN < 0 THEN ABS := -IN; ELSE ABS := IN; END_IF;
    END_FUNCTION

    NAMESPACE Test
        {test}
        FUNCTION test_abs_positive
        VAR x : INT; END_VAR
            x := ABS(IN := 5);
        END_FUNCTION

        {test}
        FUNCTION test_abs_negative
        VAR x : INT; END_VAR
            x := ABS(IN := -5);
        END_FUNCTION
    END_NAMESPACE
END_NAMESPACE
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export Std.Math.ABS(Int) -> Int
    export Std.Math.Test.test_abs_negative()
    export Std.Math.Test.test_abs_positive()
    ");
}

#[rstest]
fn test_functions_across_files(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE Std.Math
    FUNCTION ADD : INT
    VAR_INPUT a : INT; b : INT; END_VAR
        ADD := a + b;
    END_FUNCTION
END_NAMESPACE
    "#;
    let source2 = r#"
NAMESPACE Std.Math.Test
    USING Std.Math;

    {test}
    FUNCTION test_add
    VAR x : INT; END_VAR
        x := ADD(a := 1, b := 2);
    END_FUNCTION
END_NAMESPACE
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source1, source2]), @r"
    export Std.Math.ADD(Int, Int) -> Int
    export Std.Math.Test.test_add()
    ");
}

#[rstest]
fn test_pragma_on_program(mut with_db: RootDatabase) {
    let source = r#"
{test}
PROGRAM test_something
VAR x : INT; END_VAR
    x := 42;
END_PROGRAM
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"export test_something()");
}
