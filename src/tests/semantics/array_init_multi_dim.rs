use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn non_zero_based_array_exact(mut with_db: RootDatabase) {
    // Array[1..3] has 3 elements - should pass
    let source = r#"
        TYPE
            OneBasedArray: ARRAY[1..3] OF INT;
        END_TYPE

        FUNCTION Test
            VAR
                Data : OneBasedArray := [3(10)];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn array_initializer_out_of_bounds(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: ARRAY[0..3] OF INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [5(10)];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0605] Error: invalid array access
       ,-[ file:///test0.st:8:35 ]
       |
     8 |                 Base : Engine := [5(10)];
       |                                   ^^|^^
       |                                     `---- too many elements in array initializer (expected at most 4)
    ---'
    ");
}

#[rstest]
fn array_initializer_out_of_bounds_with_single_values(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: ARRAY[0..3] OF INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                // (3) + 4 + 5 + 6 (3 * n + 1) = limit
                Base : Engine := [3(10), 5, 6, 4];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0605] Error: invalid array access
       ,-[ file:///test0.st:9:45 ]
       |
     9 |                 Base : Engine := [3(10), 5, 6, 4];
       |                                             |
       |                                             `-- too many elements in array initializer (expected at most 4)
    ---'
    ");
}

#[rstest]
fn multi_dimensional_array_initializer_out_of_bounds(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: ARRAY[0..3, 0..6] OF INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [3(10(10))];
            END_VAR

        END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0605] Error: invalid array access
       ,-[ file:///test0.st:8:37 ]
       |
     8 |                 Base : Engine := [3(10(10))];
       |                                     ^^^|^^
       |                                        `---- too many elements in array initializer (expected at most 7)
       |
       | Note: this error occurred in array dimension 2
    ---'
    ");
}

#[rstest]
fn type_check_multi_dimensional_array(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: ARRAY[0..3, 0..6] OF BOOL;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [3(5(10.5))];
            END_VAR

        END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:8:39 ]
       |
     8 |                 Base : Engine := [3(5(10.5))];
       |                                       ^^|^
       |                                         `--- cannot infer '<float>' to 'BOOL': invalid boolean literal
    ---'
    ");
}

#[rstest]
fn array_initializer_exact_bounds(mut with_db: RootDatabase) {
    // Should pass - exactly 4 elements for ARRAY[0..3]
    let source = r#"
        TYPE
            Engine: ARRAY[0..3] OF INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [4(10)];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn array_initializer_mixed_indexed_and_single(mut with_db: RootDatabase) {
    // 2 + 1 + 1 = 4 elements, should pass
    let source = r#"
        TYPE
            Engine: ARRAY[0..3] OF INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [2(10), 20, 30];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn multi_dimensional_exact_bounds(mut with_db: RootDatabase) {
    // 4 elements in dim 0, 7 elements in dim 1 - should pass
    let source = r#"
        TYPE
            Engine: ARRAY[0..3, 0..6] OF INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [4(7(1))];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn multi_dimensional_first_dim_overflow(mut with_db: RootDatabase) {
    // 5 elements in dim 0, but only 4 allowed
    let source = r#"
        TYPE
            Engine: ARRAY[0..3, 0..6] OF INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [5(7(1))];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0605] Error: invalid array access
       ,-[ file:///test0.st:8:35 ]
       |
     8 |                 Base : Engine := [5(7(1))];
       |                                   ^^^|^^^
       |                                      `----- too many elements in array initializer (expected at most 4)
    ---'
    ");
}

#[rstest]
fn multi_dimensional_both_dims_overflow(mut with_db: RootDatabase) {
    // Both dimensions overflow
    let source = r#"
        TYPE
            Engine: ARRAY[0..3, 0..6] OF INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [5(10(1))];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0605] Error: invalid array access
       ,-[ file:///test0.st:8:35 ]
       |
     8 |                 Base : Engine := [5(10(1))];
       |                                   ^^^^|^^^
       |                                       `----- too many elements in array initializer (expected at most 4)
    ---'
    [E0605] Error: invalid array access
       ,-[ file:///test0.st:8:37 ]
       |
     8 |                 Base : Engine := [5(10(1))];
       |                                     ^^|^^
       |                                       `---- too many elements in array initializer (expected at most 7)
       |
       | Note: this error occurred in array dimension 2
    ---'
    ");
}

#[rstest]
fn three_dimensional_array(mut with_db: RootDatabase) {
    // 3D array with overflow in third dimension
    let source = r#"
        TYPE
            Cube: ARRAY[0..1, 0..2, 0..3] OF INT;
        END_TYPE

        FUNCTION Test
            VAR
                Data : Cube := [2(3(5(1)))];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0605] Error: invalid array access
       ,-[ file:///test0.st:8:37 ]
       |
     8 |                 Data : Cube := [2(3(5(1)))];
       |                                     ^^|^
       |                                       `--- too many elements in array initializer (expected at most 4)
       |
       | Note: this error occurred in array dimension 3
    ---'
    ");
}

#[rstest]
fn array_in_struct_overflow(mut with_db: RootDatabase) {
    // Array inside struct with overflow
    let source = r#"
        TYPE
            Engine: STRUCT
                Power: ARRAY[0..2] OF INT;
                Torque: INT;
            END_STRUCT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := (Power := [5(10)], Torque := 100);
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0605] Error: invalid array access
        ,-[ file:///test0.st:11:45 ]
        |
     11 |                 Base : Engine := (Power := [5(10)], Torque := 100);
        |                                             ^^|^^
        |                                               `---- too many elements in array initializer (expected at most 3)
    ----'
    ");
}

#[rstest]
fn array_of_struct_overflow(mut with_db: RootDatabase) {
    // Array of structs with too many elements
    let source = r#"
        TYPE
            Engine: STRUCT
                Power: INT;
            END_STRUCT;
            EngineArray: ARRAY[0..1] OF Engine;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : EngineArray := [(Power := 10), (Power := 20), (Power := 30)];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0605] Error: invalid array access
        ,-[ file:///test0.st:11:80 ]
        |
     11 |                 Base : EngineArray := [(Power := 10), (Power := 20), (Power := 30)];
        |                                                                                ^|
        |                                                                                 `-- too many elements in array initializer (expected at most 2)
    ----'
    ");
}

#[rstest]
fn nested_array_in_struct_in_array(mut with_db: RootDatabase) {
    // Complex nesting: array of structs containing arrays
    let source = r#"
        TYPE
            Engine: STRUCT
                Values: ARRAY[0..1] OF INT;
            END_STRUCT;
            EngineArray: ARRAY[0..1] OF Engine;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : EngineArray := [(Values := [5(1)])];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0605] Error: invalid array access
        ,-[ file:///test0.st:11:52 ]
        |
     11 |                 Base : EngineArray := [(Values := [5(1)])];
        |                                                    ^^|^
        |                                                      `--- too many elements in array initializer (expected at most 2)
    ----'
    ");
}

#[rstest]
fn single_element_array_overflow(mut with_db: RootDatabase) {
    // Single element array with 2 elements
    let source = r#"
        TYPE
            Single: ARRAY[0..0] OF INT;
        END_TYPE

        FUNCTION Test
            VAR
                Data : Single := [1, 2];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0605] Error: invalid array access
       ,-[ file:///test0.st:8:38 ]
       |
     8 |                 Data : Single := [1, 2];
       |                                      |
       |                                      `-- too many elements in array initializer (expected at most 1)
    ---'
    ");
}

#[rstest]
fn non_zero_based_array_overflow(mut with_db: RootDatabase) {
    // Array starting at 1, not 0
    let source = r#"
        TYPE
            OneBasedArray: ARRAY[1..3] OF INT;
        END_TYPE

        FUNCTION Test
            VAR
                Data : OneBasedArray := [4(10)];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0605] Error: invalid array access
       ,-[ file:///test0.st:8:42 ]
       |
     8 |                 Data : OneBasedArray := [4(10)];
       |                                          ^^|^^
       |                                            `---- too many elements in array initializer (expected at most 3)
    ---'
    ");
}
