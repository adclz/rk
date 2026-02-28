use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::check::diagnostics_for_file;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::add_sources;
use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn duplicate_variables(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT;
        test: REAL;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0102] Error: duplicate definitions
       ,-[ file:///test0.st:5:9 ]
       |
     4 |         test: INT;
       |         ^^|^
       |           `--- variable 'test' is already defined here
     5 |         test: REAL;
       |         ^^|^
       |           `--- duplicate variable 'test'
    ---'
    ");
}

#[rstest]
fn duplicate_inline_variables(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test, test: INT;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0102] Error: duplicate definitions
       ,-[ file:///test0.st:4:15 ]
       |
     4 |         test, test: INT;
       |         ^^|^  ^^|^
       |           `--------- variable 'test' is already defined here
       |                 |
       |                 `--- duplicate variable 'test'
    ---'
    ");
}

#[rstest]
fn duplicate_struct_fields(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    T1 : STRUCT
        test: INT;
        test: REAL;
    END_STRUCT;
END_TYPE"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0103] Error: duplicate definitions
       ,-[ file:///test0.st:5:9 ]
       |
     4 |         test: INT;
       |         ^^|^
       |           `--- field 'test' is already defined here
     5 |         test: REAL;
       |         ^^|^
       |           `--- duplicate field 'test'
    ---'
    ");
}

#[rstest]
fn duplicate_pous(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1

END_FUNCTION_BLOCK

FUNCTION_BLOCK fb1

END_FUNCTION_BLOCK
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0101] Error: duplicate definitions
       ,-[ file:///test0.st:2:16 ]
       |
     2 | FUNCTION_BLOCK fb1
       |                ^|^
       |                 `--- duplicate POU 'fb1'
       |
     6 | FUNCTION_BLOCK fb1
       |                ^|^
       |                 `--- POU 'fb1' is already defined here
    ---'
    ");
}

#[rstest]
fn duplicate_enum_variants(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    E1 : (A, B, A);
END_TYPE
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0104] Error: duplicate definitions
       ,-[ file:///test0.st:3:11 ]
       |
     3 |     E1 : (A, B, A);
       |           |     |
       |           `-------- duplicate enum variant 'A'
       |                 |
       |                 `-- enum variant 'A' is already defined here
    ---'
    ");
}

#[rstest]
fn duplicate_pous_in_namespace(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE ns1
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK
END_NAMESPACE
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0101] Error: duplicate definitions
       ,-[ file:///test0.st:3:20 ]
       |
     3 |     FUNCTION_BLOCK fb1
       |                    ^|^
       |                     `--- duplicate POU 'fb1'
       |
     7 |     FUNCTION_BLOCK fb1
       |                    ^|^
       |                     `--- POU 'fb1' is already defined here
    ---'
    ");
}

// DashMap keys are not ordered
// This means the snapshot might not be in the order we expect.
// So instead we just check the number of errors

#[rstest]
fn cross_file_global_duplicates(mut with_db: RootDatabase) {
    let source1 = r#"
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK
"#;

    let source2 = r#"
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK
"#;

    let source3 = r#"
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK
"#;
    add_sources(&mut with_db, &[source1, source2, source3]);

    let diagnostics = with_db
        .get_files()
        .iter()
        .map(|file| diagnostics_for_file(&with_db, *file))
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 3);
}

#[rstest]
fn cross_file_namespace_duplicates(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns1
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK
END_NAMESPACE"#;

    let source2 = r#"
NAMESPACE ns1
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK
END_NAMESPACE"#;

    add_sources(&mut with_db, &[source1, source2]);

    let diagnostics = with_db
        .get_files()
        .iter()
        .map(|file| diagnostics_for_file(&with_db, *file))
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 2);
}

#[rstest]
fn duplicate_methods_in_interface(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE it1
    METHOD m1 END_METHOD
    METHOD m1 END_METHOD
END_INTERFACE
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0106] Error: duplicate definitions
       ,-[ file:///test0.st:4:12 ]
       |
     3 |     METHOD m1 END_METHOD
       |            ^|
       |             `-- method 'm1' is already defined here
     4 |     METHOD m1 END_METHOD
       |            ^|
       |             `-- duplicate method 'm1'
    ---'
    ");
}

#[rstest]
fn duplicate_methods_in_class(mut with_db: RootDatabase) {
    let source = r#"
CLASS it1
    METHOD m1 END_METHOD
    METHOD m1 END_METHOD
END_CLASS
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0105] Error: duplicate definitions
       ,-[ file:///test0.st:4:12 ]
       |
     3 |     METHOD m1 END_METHOD
       |            ^|
       |             `-- method 'm1' is already defined here
     4 |     METHOD m1 END_METHOD
       |            ^|
       |             `-- duplicate method 'm1'
    ---'
    ");
}

#[rstest]
fn duplicate_methods_in_fb(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK it1
    METHOD m1 END_METHOD
    METHOD m1 END_METHOD
END_FUNCTION_BLOCK
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0105] Error: duplicate definitions
       ,-[ file:///test0.st:4:12 ]
       |
     3 |     METHOD m1 END_METHOD
       |            ^|
       |             `-- method 'm1' is already defined here
     4 |     METHOD m1 END_METHOD
       |            ^|
       |             `-- duplicate method 'm1'
    ---'
    ");
}

#[rstest]
fn duplicate_methods_in_inherited_methods(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE I1
    METHOD m1 END_METHOD
END_INTERFACE

INTERFACE I2
    METHOD m1 END_METHOD
END_INTERFACE

CLASS it1 IMPLEMENTS I1, I2
    METHOD OVERRIDE m1 END_METHOD
END_CLASS
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0107] Error: duplicate definitions
       ,-[ file:///test0.st:3:12 ]
       |
     3 |     METHOD m1 END_METHOD
       |            ^|
       |             `-- duplicate method 'm1'
       |
     7 |     METHOD m1 END_METHOD
       |            ^|
       |             `-- method 'm1' is already defined here
       |
       | Note: this error happens because both interfaces 'I1' and 'I2' define a method 'm1'
    ---'
    ");
}

#[rstest]
fn duplicate_init_expr(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Engine:
            STRUCT
                power : INT;
                oil : REAL;
            END_STRUCT
        END_TYPE

        FUNCTION fn
            VAR
                Base : Engine := (power := 100, power := 100);
            END_VAR

        END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0110] Error: duplicate definitions
        ,-[ file:///test0.st:11:35 ]
        |
     11 |                 Base : Engine := (power := 100, power := 100);
        |                                   ^^^^^^|^^^^^  ^^^^^^|^^^^^
        |                                         `--------------------- duplicate field 'power' in initializer expression
        |                                                       |
        |                                                       `------- field 'power' is already initialized here
    ----'
    ");
}

#[rstest]
fn duplicate_usings(mut with_db: RootDatabase) {
    let source = r#"
        NAMESPACE ns1
        END_NAMESPACE

        FUNCTION fn
            USING ns1;
            USING ns1;
            
        END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0109] Error: duplicate definitions
       ,-[ file:///test0.st:7:19 ]
       |
     6 |             USING ns1;
       |                   ^|^
       |                    `--- namespace 'ns1' is already imported here
     7 |             USING ns1;
       |                   ^|^
       |                    `--- duplicate `USING` for namespace 'ns1'
    ---'
    ");
}

#[rstest]
fn duplicate_prorams(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM prog1
        END_PROGRAM

        PROGRAM prog1
        END_PROGRAM
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0111] Error: duplicate definitions
       ,-[ file:///test0.st:2:17 ]
       |
     2 |         PROGRAM prog1
       |                 ^^|^^
       |                   `---- duplicate program 'prog1'
       |
     5 |         PROGRAM prog1
       |                 ^^|^^
       |                   `---- program 'prog1' is already defined here
    ---'
    ");
}

#[rstest]
fn duplicate_generics(mut with_db: RootDatabase) {
    let source = r#"
    FUNCTION fn1<T: ANY_INT, T: ANY_INT>

    END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0112] Error: duplicate definitions
       ,-[ file:///test0.st:2:30 ]
       |
     2 |     FUNCTION fn1<T: ANY_INT, T: ANY_INT>
       |                  |           |
       |                  `-------------- generic parameter 'T' is already defined here
       |                              |
       |                              `-- duplicate generic parameter 'T'
    ---'
    ");
}

#[rstest]
fn duplicate_configurations(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION cfg

END_CONFIGURATION 

CONFIGURATION cfg

END_CONFIGURATION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0113] Error: duplicate definitions
       ,-[ file:///test0.st:2:15 ]
       |
     2 | CONFIGURATION cfg
       |               ^|^
       |                `--- duplicate configuration 'cfg'
       |
     6 | CONFIGURATION cfg
       |               ^|^
       |                `--- configuration 'cfg' is already defined here
    ---'
    ");
}

// ── Configuration internal duplicates ─────────────────────────────────────

#[rstest]
fn duplicate_tasks_in_config(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    TASK t1(PRIORITY := 1);
    TASK t1(PRIORITY := 2);
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0114] Error: duplicate definitions
       ,-[ file:///test0.st:4:10 ]
       |
     3 |     TASK t1(PRIORITY := 1);
       |          ^|
       |           `-- task 't1' is already defined here
     4 |     TASK t1(PRIORITY := 2);
       |          ^|
       |           `-- duplicate task 't1'
    ---'
    ");
}

#[rstest]
fn duplicate_prog_instances_in_config(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(PRIORITY := 1);
    PROGRAM inst1 WITH t1 : MyProg;
    PROGRAM inst1 WITH t1 : MyProg;
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0115] Error: duplicate definitions
       ,-[ file:///test0.st:8:13 ]
       |
     7 |     PROGRAM inst1 WITH t1 : MyProg;
       |             ^^|^^
       |               `---- program instance 'inst1' is already defined here
     8 |     PROGRAM inst1 WITH t1 : MyProg;
       |             ^^|^^
       |               `---- duplicate program instance 'inst1'
    ---'
    ");
}

#[rstest]
fn duplicate_resources_in_config(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    RESOURCE res1 ON CPU_TYPE
        TASK t1(PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE
    RESOURCE res1 ON CPU_TYPE
        TASK t2(PRIORITY := 2);
        PROGRAM inst2 WITH t2 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0116] Error: duplicate definitions
        ,-[ file:///test0.st:10:14 ]
        |
      6 |     RESOURCE res1 ON CPU_TYPE
        |              ^^|^
        |                `--- resource 'res1' is already defined here
        |
     10 |     RESOURCE res1 ON CPU_TYPE
        |              ^^|^
        |                `--- duplicate resource 'res1'
    ----'
    ");
}

#[rstest]
fn duplicate_tasks_in_resource(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    RESOURCE res1 ON CPU_TYPE
        TASK t1(PRIORITY := 1);
        TASK t1(PRIORITY := 2);
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0114] Error: duplicate definitions
       ,-[ file:///test0.st:5:14 ]
       |
     4 |         TASK t1(PRIORITY := 1);
       |              ^|
       |               `-- task 't1' is already defined here
     5 |         TASK t1(PRIORITY := 2);
       |              ^|
       |               `-- duplicate task 't1'
    ---'
    ");
}

#[rstest]
fn duplicate_prog_instances_in_resource(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    RESOURCE res1 ON CPU_TYPE
        TASK t1(PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0115] Error: duplicate definitions
       ,-[ file:///test0.st:9:17 ]
       |
     8 |         PROGRAM inst1 WITH t1 : MyProg;
       |                 ^^|^^
       |                   `---- program instance 'inst1' is already defined here
     9 |         PROGRAM inst1 WITH t1 : MyProg;
       |                 ^^|^^
       |                   `---- duplicate program instance 'inst1'
    ---'
    ");
}
