use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn unknown_struct_field(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Engine:
            STRUCT
                power : INT;
                oil : REAL;
            END_STRUCT
        END_TYPE

        FUNCTION StartEngine
            VAR
                // fuel is not a member of engine
                Base : Engine := (power := 100, fuel := 10.0);
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
        ,-[ file:///test0.st:12:49 ]
        |
      2 |         TYPE Engine:
        |              ^^^|^^  
        |                 `---- 'Engine' is declared here
        | 
     12 |                 Base : Engine := (power := 100, fuel := 10.0);
        |                                                 ^^|^  
        |                                                   `--- no field 'fuel' in STRUCT
        | 
    ----'
    ");
}

#[rstest]
fn invalid_struct_value(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Engine:
            STRUCT
                power : INT;
                oil : REAL;
            END_STRUCT
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := (power := 10.5, fuel := 10.0);
            END_VAR

        END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
        ,-[ file:///test0.st:11:44 ]
        |
      4 |                 power : INT;
        |                 ^^|^^   ^|^  
        |                   `---------- 'power' is declared here
        |                          |   
        |                          `--- type defined here
        | 
     11 |                 Base : Engine := (power := 10.5, fuel := 10.0);
        |                                            ^^|^  
        |                                              `--- invalid value initializer: invalid INT literal
    ----'
    Error: 
        ,-[ file:///test0.st:11:50 ]
        |
      2 |         TYPE Engine:
        |              ^^^|^^  
        |                 `---- 'Engine' is declared here
        | 
     11 |                 Base : Engine := (power := 10.5, fuel := 10.0);
        |                                                  ^^|^  
        |                                                    `--- no field 'fuel' in STRUCT
        | 
    ----'
    ");
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
    Error: 
       ,-[ file:///test0.st:8:35 ]
       |
     3 |             Engine: ARRAY[0..3] OF INT;
       |             ^^^|^^  
       |                `---- 'Engine' is declared here
       | 
     8 |                 Base : Engine := [5(10)];
       |                                   ^^|^^  
       |                                     `---- too many array elements: provided 5, but array capacity is 4
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
    Error: 
       ,-[ file:///test0.st:9:45 ]
       |
     3 |             Engine: ARRAY[0..3] OF INT;
       |             ^^^|^^  
       |                `---- 'Engine' is declared here
       | 
     9 |                 Base : Engine := [3(10), 5, 6, 4];
       |                                             |  
       |                                             `-- too many array elements: provided 5, but array capacity is 4
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
    Error: 
       ,-[ file:///test0.st:8:37 ]
       |
     3 |             Engine: ARRAY[0..3, 0..6] OF INT;
       |             ^^^|^^  
       |                `---- 'Engine' is declared here
       | 
     8 |                 Base : Engine := [3(10(10))];
       |                                     ^^^|^^  
       |                                        `---- too many array elements: provided 10, but array capacity is 7
    ---'
    ");
}

#[rstest]
fn invalid_array_value(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: ARRAY[0..3] OF INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [3(10.5)];
            END_VAR

        END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:8:37 ]
       |
     3 |             Engine: ARRAY[0..3] OF INT;
       |             ^^^|^^                 ^|^  
       |                `------------------------ 'Engine' is declared here
       |                                     |   
       |                                     `--- type defined here
       | 
     8 |                 Base : Engine := [3(10.5)];
       |                                     ^^|^  
       |                                       `--- invalid value initializer: invalid INT literal
    ---'
    ");
}

#[rstest]
fn multi_dimensional_invalid_array_value(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: ARRAY[0..3, 0..6] OF INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [3(5(10.5))];
            END_VAR

        END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:8:39 ]
       |
     3 |             Engine: ARRAY[0..3, 0..6] OF INT;
       |             ^^^|^^                       ^|^  
       |                `------------------------------ 'Engine' is declared here
       |                                           |   
       |                                           `--- type defined here
       | 
     8 |                 Base : Engine := [3(5(10.5))];
       |                                       ^^|^  
       |                                         `--- invalid value initializer: invalid INT literal
    ---'
    ");
}

#[rstest]
fn invalid_value_in_array_of_struct(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: STRUCT
                Power: INT;
                Torque: INT;
            END_STRUCT;
            EngineArray: ARRAY[0..3] OF Engine;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : EngineArray := [(Power := 10, Torque := 10.0)];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
        ,-[ file:///test0.st:12:64 ]
        |
      5 |                 Torque: INT;
        |                 ^^^|^^  ^|^  
        |                    `--------- 'Torque' is declared here
        |                          |   
        |                          `--- type defined here
        | 
     12 |                 Base : EngineArray := [(Power := 10, Torque := 10.0)];
        |                                                                ^^|^  
        |                                                                  `--- invalid value initializer: invalid INT literal
    ----'
    ");
}

#[rstest]
fn invalid_value_in_struct_with_array(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: STRUCT
                Power: ARRAY[0..2] OF INT;
                Torque: INT;
            END_STRUCT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [(Power := [10, 5.3], Torque := 10.0)];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
        ,-[ file:///test0.st:11:50 ]
        |
      4 |                 Power: ARRAY[0..2] OF INT;
        |                 ^^|^^                 ^|^  
        |                   `------------------------ 'Power' is declared here
        |                                        |   
        |                                        `--- type defined here
        | 
     11 |                 Base : Engine := [(Power := [10, 5.3], Torque := 10.0)];
        |                                                  ^|^  
        |                                                   `--- invalid value initializer: invalid INT literal
    ----'
    Error: 
        ,-[ file:///test0.st:11:66 ]
        |
      5 |                 Torque: INT;
        |                 ^^^|^^  ^|^  
        |                    `--------- 'Torque' is declared here
        |                          |   
        |                          `--- type defined here
        | 
     11 |                 Base : Engine := [(Power := [10, 5.3], Torque := 10.0)];
        |                                                                  ^^|^  
        |                                                                    `--- invalid value initializer: invalid INT literal
    ----'
    ");
}
