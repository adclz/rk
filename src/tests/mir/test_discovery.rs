use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::with_db;
use super::utils::{mir_exports, mir_test_manifest};

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

// --- Test manifest tests ---

#[rstest]
fn manifest_contains_test_functions(mut with_db: RootDatabase) {
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
    assert_snapshot!(mir_test_manifest(&mut with_db, &[source]), @r"
    test Std.Math.Test.test_abs_negative
    test Std.Math.Test.test_abs_positive
    ");
}

#[rstest]
fn manifest_excludes_non_test_functions(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION helper : INT
VAR_INPUT x : INT; END_VAR
    helper := x + 1;
END_FUNCTION

FUNCTION not_a_test : INT
    not_a_test := helper(x := 1);
END_FUNCTION
    "#;
    // No {test} pragmas — manifest should be empty
    assert_snapshot!(mir_test_manifest(&mut with_db, &[source]), @"");
}

#[rstest]
fn manifest_with_case_pragmas(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION ABS : INT
VAR_INPUT IN : INT; END_VAR
    IF IN < 0 THEN ABS := -IN; ELSE ABS := IN; END_IF;
END_FUNCTION

{test}
{case(5, 5)}
{case(-42, 42)}
{case(0, 0)}
FUNCTION test_abs
VAR_INPUT x : INT; expected : INT; END_VAR
    IF ABS(IN := x) <> expected THEN END_IF;
END_FUNCTION
    "#;
    assert_snapshot!(mir_test_manifest(&mut with_db, &[source]), @r"
    test test_abs[0](5, 5)
    test test_abs[1](-42, 42)
    test test_abs[2](0, 0)
    ");
}

#[rstest]
fn manifest_test_program(mut with_db: RootDatabase) {
    let source = r#"
{test}
PROGRAM test_something
VAR x : INT; END_VAR
    x := 42;
END_PROGRAM
    "#;
    assert_snapshot!(mir_test_manifest(&mut with_db, &[source]), @"test test_something");
}

#[rstest]
fn manifest_roundtrip_msgpack(mut with_db: RootDatabase) {
    // Verify the manifest can be serialized and deserialized via MessagePack
    use auto_lsp::default::db::BaseDatabase;
    use hir::hir_def::semantic_index::semantic_index;
    use crate::tests::utils::add_sources;

    let source = r#"
{test}
FUNCTION test_simple
VAR x : INT; END_VAR
    x := 1;
END_FUNCTION
    "#;
    add_sources(&mut with_db, &[source]);
    let files: Vec<_> = with_db.get_files().iter().map(|e| *e.value()).collect();
    let sem_indices: Vec<_> = files.iter().map(|f| semantic_index(&with_db, *f)).collect();
    let module = mir::lower::lower_module::lower_modules(&with_db, &sem_indices).unwrap();

    let bytes = module.test_manifest.to_msgpack();
    let decoded = mir::test_manifest::TestManifest::from_msgpack(&bytes).unwrap();
    assert_eq!(module.test_manifest, decoded);
    assert_eq!(decoded.tests.len(), 1);
    assert_eq!(decoded.tests[0].path, "test_simple");
}
