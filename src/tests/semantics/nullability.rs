use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn deref_uninitialized_ref(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        ptr: REF_TO INT;
        result: INT;
    END_VAR

    result := ptr^;
END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1007] Warning: possibly null dereference
       ,-[ file:///test0.st:8:15 ]
       |
     4 |         ptr: REF_TO INT;
       |         ^^^^^^^|^^^^^^^
       |                `--------- 'ptr' declared without initializer here
       |
     8 |     result := ptr^;
       |               ^|^
       |                `--- dereference of reference 'ptr' which is never initialized
    ---'
    ");
}

#[rstest]
fn deref_after_null_assignment(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        x: INT := 5;
        ptr: REF_TO INT;
        result: INT;
    END_VAR

    ptr := REF(x);
    ptr := NULL;
    result := ptr^;
END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1007] Warning: possibly null dereference
        ,-[ file:///test0.st:11:15 ]
        |
     10 |     ptr := NULL;
        |     ^^^^^|^^^^^
        |          `------- 'ptr' set to NULL here
     11 |     result := ptr^;
        |               ^|^
        |                `--- dereference of reference 'ptr' which is null
    ----'
    ");
}

#[rstest]
fn deref_after_valid_assignment(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        x: INT := 5;
        ptr: REF_TO INT;
        result: INT;
    END_VAR

    ptr := REF(x);
    result := ptr^;
END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn deref_initialized_ref_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        x: INT := 5;
        ptr: REF_TO INT := REF(x);
        result: INT;
    END_VAR

    result := ptr^;
END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn deref_null_initialized_ref(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        ptr: REF_TO INT := NULL;
        result: INT;
    END_VAR

    result := ptr^;
END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1007] Warning: possibly null dereference
       ,-[ file:///test0.st:8:15 ]
       |
     4 |         ptr: REF_TO INT := NULL;
       |                         ^^^|^^^
       |                            `----- 'ptr' set to NULL here
       |
     8 |     result := ptr^;
       |               ^|^
       |                `--- dereference of reference 'ptr' which is null
    ---'
    ");
}

#[rstest]
fn no_null_tracking_for_var_input(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR_INPUT
        ptr: REF_TO INT;
    END_VAR
    VAR
        result: INT;
    END_VAR

    result := ptr^;
END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn no_null_tracking_for_var_in_out(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
    VAR_IN_OUT
        x: INT;
    END_VAR

    fn1 := x;
END_FUNCTION
    "#;

    // VAR_IN_OUT is the caller's responsibility, no null tracking needed
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn deref_propagated_null_from_uninitialized_ref(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        x: INT := 5;
        ptr: REF_TO INT;
        ptr_2: REF_TO INT;
        result: INT;
    END_VAR

    ptr := ptr_2;
    result := ptr^;
END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1007] Warning: possibly null dereference
        ,-[ file:///test0.st:11:15 ]
        |
      6 |         ptr_2: REF_TO INT;
        |         ^^^^^^^^|^^^^^^^^
        |                 `---------- 'ptr' declared without initializer here
        |
     11 |     result := ptr^;
        |               ^|^
        |                `--- dereference of reference 'ptr' which is never initialized
    ----'
    ");
}

#[rstest]
fn deref_propagated_null_from_null_ref(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        ptr: REF_TO INT;
        ptr_2: REF_TO INT := NULL;
        result: INT;
    END_VAR

    ptr := ptr_2;
    result := ptr^;
END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1007] Warning: possibly null dereference
        ,-[ file:///test0.st:10:15 ]
        |
      5 |         ptr_2: REF_TO INT := NULL;
        |                           ^^^|^^^
        |                              `----- 'ptr' set to NULL here
        |
     10 |     result := ptr^;
        |               ^|^
        |                `--- dereference of reference 'ptr' which is null
    ----'
    ");
}

#[rstest]
fn no_warning_after_propagated_non_null(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        x: INT := 5;
        ptr: REF_TO INT;
        ptr_2: REF_TO INT := REF(x);
        result: INT;
    END_VAR

    ptr := ptr_2;
    result := ptr^;
END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn deref_after_reassignment_null_then_valid(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        x: INT := 5;
        ptr: REF_TO INT;
        result: INT;
    END_VAR

    ptr := NULL;
    ptr := REF(x);
    result := ptr^;
END_FUNCTION_BLOCK
    "#;

    // ptr was NULL but then reassigned to REF(x), so no warning
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn deref_after_reassignment_valid_then_null(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        x: INT := 5;
        ptr: REF_TO INT := REF(x);
        result: INT;
    END_VAR

    ptr := NULL;
    result := ptr^;
END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1007] Warning: possibly null dereference
        ,-[ file:///test0.st:10:15 ]
        |
      9 |     ptr := NULL;
        |     ^^^^^|^^^^^
        |          `------- 'ptr' set to NULL here
     10 |     result := ptr^;
        |               ^|^
        |                `--- dereference of reference 'ptr' which is null
    ----'
    ");
}

#[rstest]
fn deref_through_value_write_no_false_positive(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        x: INT := 5;
        ptr: REF_TO INT := REF(x);
    END_VAR

    ptr^ := 42;
END_FUNCTION_BLOCK
    "#;

    // Writing through a non-null ref should not trigger a warning
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn null_state_independent_per_variable(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        x: INT := 5;
        ptr1: REF_TO INT := REF(x);
        ptr2: REF_TO INT;
        result1: INT;
        result2: INT;
    END_VAR

    result1 := ptr1^;
    result2 := ptr2^;
END_FUNCTION_BLOCK
    "#;

    // Only ptr2 should warn, ptr1 is initialized
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1007] Warning: possibly null dereference
        ,-[ file:///test0.st:12:16 ]
        |
      6 |         ptr2: REF_TO INT;
        |         ^^^^^^^^|^^^^^^^
        |                 `--------- 'ptr2' declared without initializer here
        |
     12 |     result2 := ptr2^;
        |                ^^|^
        |                  `--- dereference of reference 'ptr2' which is never initialized
    ----'
    ");
}

#[rstest]
fn double_deref_null_ref_to_ref(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        ptr: REF_TO REF_TO INT;
        result: INT;
    END_VAR

    result := ptr^^;
END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1007] Warning: possibly null dereference
       ,-[ file:///test0.st:8:15 ]
       |
     4 |         ptr: REF_TO REF_TO INT;
       |         ^^^^^^^^^^^|^^^^^^^^^^
       |                    `------------ 'ptr' declared without initializer here
       |
     8 |     result := ptr^^;
       |               ^|^
       |                `--- dereference of reference 'ptr' which is never initialized
    ---'
    [E1007] Warning: possibly null dereference
       ,-[ file:///test0.st:8:15 ]
       |
     4 |         ptr: REF_TO REF_TO INT;
       |         ^^^^^^^^^^^|^^^^^^^^^^
       |                    `------------ 'ptr' declared without initializer here
       |
     8 |     result := ptr^^;
       |               ^|^
       |                `--- dereference of reference 'ptr' which is never initialized
    ---'
    ");
}

#[rstest]
fn deref_in_function(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
    VAR
        ptr: REF_TO INT;
    END_VAR

    fn1 := ptr^;
END_FUNCTION
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1007] Warning: possibly null dereference
       ,-[ file:///test0.st:7:12 ]
       |
     4 |         ptr: REF_TO INT;
       |         ^^^^^^^|^^^^^^^
       |                `--------- 'ptr' declared without initializer here
       |
     7 |     fn1 := ptr^;
       |            ^|^
       |             `--- dereference of reference 'ptr' which is never initialized
    ---'
    ");
}

#[rstest]
fn deref_in_program(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM main
    VAR
        ptr: REF_TO INT;
        result: INT;
    END_VAR

    result := ptr^;
END_PROGRAM
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1007] Warning: possibly null dereference
       ,-[ file:///test0.st:8:15 ]
       |
     4 |         ptr: REF_TO INT;
       |         ^^^^^^^|^^^^^^^
       |                `--------- 'ptr' declared without initializer here
       |
     8 |     result := ptr^;
       |               ^|^
       |                `--- dereference of reference 'ptr' which is never initialized
    ---'
    ");
}

#[rstest]
fn array_of_ref_no_tracking(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
    VAR
        ptrs: ARRAY[1..3] OF REF_TO INT;
        result: INT;
    END_VAR

    result := ptrs[1]^;
END_FUNCTION_BLOCK
    "#;

    // Array elements are not individually tracked - no warning expected
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}
