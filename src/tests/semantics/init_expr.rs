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
    Advice: 
        ,-[ file:///test0.st:12:44 ]
        |
      4 |                 power : INT;
        |                 ^^|^^  
        |                   `---- type is defined by struct field 'power' here
        | 
     12 |                 Base : Engine := (power := 100, fuel := 10.0);
        |                                            ^|^  
        |                                             `--- expected 'INT', got '(INT) 100'
    ----'
    Error: 
        ,-[ file:///test0.st:12:49 ]
        |
     12 |                 Base : Engine := (power := 100, fuel := 10.0);
        |                                                 ^^^^^^|^^^^^  
        |                                                       `------- no field 'fuel' in type 'STRUCT'
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
    Advice: 
        ,-[ file:///test0.st:11:44 ]
        |
      4 |                 power : INT;
        |                 ^^|^^  
        |                   `---- type is defined by struct field 'power' here
        | 
     11 |                 Base : Engine := (power := 10.5, fuel := 10.0);
        |                                            ^^|^  
        |                                              `--- expected 'INT', got '(REAL) 10.5'
    ----'
    Error: 
        ,-[ file:///test0.st:11:50 ]
        |
     11 |                 Base : Engine := (power := 10.5, fuel := 10.0);
        |                                                  ^^^^^^|^^^^^  
        |                                                        `------- no field 'fuel' in type 'STRUCT'
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
    Advice: 
       ,-[ file:///test0.st:8:37 ]
       |
     8 |                 Base : Engine := [5(10)];
       |                                     ^|  
       |                                      `-- expected 'INT', got '(INT) 10'
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
    Advice: 
       ,-[ file:///test0.st:9:37 ]
       |
     9 |                 Base : Engine := [3(10), 5, 6, 4];
       |                                     ^|  
       |                                      `-- expected 'INT', got '(INT) 10'
    ---'
    Advice: 
       ,-[ file:///test0.st:9:42 ]
       |
     9 |                 Base : Engine := [3(10), 5, 6, 4];
       |                                          |  
       |                                          `-- expected 'INT', got '(INT) 5'
    ---'
    Advice: 
       ,-[ file:///test0.st:9:45 ]
       |
     9 |                 Base : Engine := [3(10), 5, 6, 4];
       |                                             |  
       |                                             `-- expected 'INT', got '(INT) 6'
    ---'
    Advice: 
       ,-[ file:///test0.st:9:48 ]
       |
     9 |                 Base : Engine := [3(10), 5, 6, 4];
       |                                                |  
       |                                                `-- expected 'INT', got '(INT) 4'
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
    Advice: 
       ,-[ file:///test0.st:8:40 ]
       |
     8 |                 Base : Engine := [3(10(10))];
       |                                        ^|  
       |                                         `-- expected 'INT', got '(INT) 10'
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
    Advice: 
       ,-[ file:///test0.st:8:37 ]
       |
     8 |                 Base : Engine := [3(10.5)];
       |                                     ^^|^  
       |                                       `--- expected 'INT', got '(REAL) 10.5'
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
    Advice: 
       ,-[ file:///test0.st:8:39 ]
       |
     8 |                 Base : Engine := [3(5(10.5))];
       |                                       ^^|^  
       |                                         `--- expected 'INT', got '(REAL) 10.5'
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
    Advice: 
        ,-[ file:///test0.st:12:50 ]
        |
      4 |                 Power: INT;
        |                 ^^|^^  
        |                   `---- type is defined by struct field 'Power' here
        | 
     12 |                 Base : EngineArray := [(Power := 10, Torque := 10.0)];
        |                                                  ^|  
        |                                                   `-- expected 'INT', got '(INT) 10'
    ----'
    Advice: 
        ,-[ file:///test0.st:12:64 ]
        |
      5 |                 Torque: INT;
        |                 ^^^|^^  
        |                    `---- type is defined by struct field 'Torque' here
        | 
     12 |                 Base : EngineArray := [(Power := 10, Torque := 10.0)];
        |                                                                ^^|^  
        |                                                                  `--- expected 'INT', got '(REAL) 10.0'
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
                Base : Engine := (Power := [10, 5.3], Torque := 10.0);
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Advice: 
        ,-[ file:///test0.st:11:45 ]
        |
     11 |                 Base : Engine := (Power := [10, 5.3], Torque := 10.0);
        |                                             ^|  
        |                                              `-- expected 'INT', got '(INT) 10'
    ----'
    Advice: 
        ,-[ file:///test0.st:11:49 ]
        |
     11 |                 Base : Engine := (Power := [10, 5.3], Torque := 10.0);
        |                                                 ^|^  
        |                                                  `--- expected 'INT', got '(REAL) 5.3'
    ----'
    Advice: 
        ,-[ file:///test0.st:11:65 ]
        |
      5 |                 Torque: INT;
        |                 ^^^|^^  
        |                    `---- type is defined by struct field 'Torque' here
        | 
     11 |                 Base : Engine := (Power := [10, 5.3], Torque := 10.0);
        |                                                                 ^^|^  
        |                                                                   `--- expected 'INT', got '(REAL) 10.0'
    ----'
    ");
}

#[rstest]
fn unexpected_struct_field(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: ARRAY[0..3] OF INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [2(param1 := 0)];
            END_VAR

        END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:8:37 ]
       |
     8 |                 Base : Engine := [2(param1 := 0)];
       |                                     ^^^^^|^^^^^  
       |                                          `------- no field 'param1' in type 'INT'
    ---'
    ");
}

#[rstest]
fn unexpected_array(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [2];
            END_VAR

        END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:8:31 ]
       |
     8 |                 Base : Engine := [2];
       |                               ^^^|^^  
       |                                  `---- cannot index into type 'INT'
    ---'
    ");
}
