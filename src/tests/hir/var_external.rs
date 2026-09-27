//! VAR_EXTERNAL: a declaration that aliases a CONFIGURATION's VAR_GLOBAL by
//! name. Existence is E0206; the type agreement the alias demands is E0207.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

/// A PROGRAM with VAR_EXTERNAL referencing a VAR_GLOBAL declared in the instantiating config.
#[rstest]
fn valid_var_external_from_config(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR_EXTERNAL
        counter : INT;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    VAR_GLOBAL
        counter : INT;
    END_VAR
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM RETAIN inst1 WITH t1 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// VAR_EXTERNAL referencing a name absent from the config's VAR_GLOBAL should report E0206.
#[rstest]
fn invalid_var_external_not_in_config(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR_EXTERNAL
        missing : INT;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    VAR_GLOBAL
        counter : INT;
    END_VAR
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM RETAIN inst1 WITH t1 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0206] Error: external variable not found
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         missing : INT;
       |         ^^^^^^|^^^^^^
       |               `-------- external variable 'missing' not found in any accessible VAR_GLOBAL
    ---'
    ");
}

/// VAR_EXTERNAL in a program that is not instantiated by any config should report E0206.
#[rstest]
fn invalid_var_external_no_config(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM StandaloneProgram
    VAR_EXTERNAL
        orphan : INT;
    END_VAR
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0206] Error: external variable not found
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         orphan : INT;
       |         ^^^^^^|^^^^^
       |               `------- external variable 'orphan' not found in any accessible VAR_GLOBAL
    ---'
    ");
}

#[rstest]
fn invalid_var_external_wrong_base_type(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR_EXTERNAL
        g : REAL;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    VAR_GLOBAL
        g : INT;
    END_VAR
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0207] Error: external variable type mismatch
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         g : REAL;
       |         ^^^^|^^^
       |             `----- 'g' is declared 'REAL' here but its VAR_GLOBAL is 'INT': an external must repeat the global's type exactly
    ---'
    ");
}

#[rstest]
fn invalid_var_external_erases_subrange(mut with_db: RootDatabase) {
    let source = r#"
TYPE Small : INT (0..10); END_TYPE

PROGRAM MyProg
    VAR_EXTERNAL
        g : INT;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    VAR_GLOBAL
        g : Small;
    END_VAR
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0207] Error: external variable type mismatch
       ,-[ file:///test0.st:6:9 ]
       |
     6 |         g : INT;
       |         ^^^|^^^
       |            `----- 'g' is declared 'INT' here but its VAR_GLOBAL is 'Small (0..10)': an external must repeat the global's type exactly
    ---'
    ");
}

#[rstest]
fn valid_var_external_repeats_the_subrange(mut with_db: RootDatabase) {
    let source = r#"
TYPE Small : INT (0..10); END_TYPE

PROGRAM MyProg
    VAR_EXTERNAL
        g : Small;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    VAR_GLOBAL
        g : Small;
    END_VAR
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// The rule is about STORAGE, so it covers every type, and inline composites
// compare structurally: two `ARRAY[0..2] OF INT` specs are distinct nodes
// but the same type.
#[rstest]
fn valid_var_external_composite_types(mut with_db: RootDatabase) {
    let source = r#"
TYPE Rec : STRUCT a : INT; END_STRUCT END_TYPE

PROGRAM MyProg
    VAR_EXTERNAL
        arr : ARRAY[0..2] OF INT;
        r : Rec;
        s : STRING;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    VAR_GLOBAL
        arr : ARRAY[0..2] OF INT;
        r : Rec;
        s : STRING;
    END_VAR
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_var_external_array_dims_differ(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR_EXTERNAL
        arr : ARRAY[0..3] OF INT;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    VAR_GLOBAL
        arr : ARRAY[0..2] OF INT;
    END_VAR
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0207] Error: external variable type mismatch
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         arr : ARRAY[0..3] OF INT;
       |         ^^^^^^^^^^^^|^^^^^^^^^^^
       |                     `------------- 'arr' is declared 'ARRAY [0..3] OF INT' here but its VAR_GLOBAL is 'ARRAY [0..2] OF INT': an external must repeat the global's type exactly
    ---'
    ");
}
