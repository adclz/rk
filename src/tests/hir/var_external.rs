// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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
    [E0206] Error: VAR_EXTERNAL without a VAR_GLOBAL
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         missing : INT;
       |         ^^^|^^^
       |            `----- no VAR_GLOBAL is named 'missing'
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
    [E0206] Error: VAR_EXTERNAL without a VAR_GLOBAL
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         orphan : INT;
       |         ^^^|^^
       |            `---- no VAR_GLOBAL is named 'orphan'
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
    [E0207] Error: VAR_EXTERNAL type mismatch
       ,-[ file:///test0.st:4:13 ]
       |
     4 |         g : REAL;
       |             ^^|^
       |               `--- 'g' is declared 'REAL' and its VAR_GLOBAL is 'INT'
       |
       | Note: a VAR_EXTERNAL repeats the type of its VAR_GLOBAL exactly
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
    [E0207] Error: VAR_EXTERNAL type mismatch
       ,-[ file:///test0.st:6:13 ]
       |
     6 |         g : INT;
       |             ^|^
       |              `--- 'g' is declared 'INT' and its VAR_GLOBAL is 'Small (0..10)'
       |
       | Note: a VAR_EXTERNAL repeats the type of its VAR_GLOBAL exactly
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
    [E0207] Error: VAR_EXTERNAL type mismatch
       ,-[ file:///test0.st:4:15 ]
       |
     4 |         arr : ARRAY[0..3] OF INT;
       |               ^^^^^^^^^|^^^^^^^^
       |                        `---------- 'arr' is declared 'ARRAY [0..3] OF INT' and its VAR_GLOBAL is 'ARRAY [0..2] OF INT'
       |
       | Note: a VAR_EXTERNAL repeats the type of its VAR_GLOBAL exactly
    ---'
    ");
}

/// An external aliases its global's storage, so a STRING's capacity must
/// agree too: a `STRING` external over a `STRING[4]` global writes 80 bytes.
#[rstest]
fn an_external_string_repeats_its_capacity(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION Cfg
VAR_GLOBAL g4 : STRING[4]; ga : ARRAY[0..1] OF STRING[4]; g : STRING; END_VAR
END_CONFIGURATION
FUNCTION f : INT
VAR_EXTERNAL g4 : STRING; ga : ARRAY[0..1] OF STRING; g : STRING[80]; END_VAR
    f := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0207] Error: VAR_EXTERNAL type mismatch
       ,-[ file:///test0.st:6:19 ]
       |
     6 | VAR_EXTERNAL g4 : STRING; ga : ARRAY[0..1] OF STRING; g : STRING[80]; END_VAR
       |                   ^^^|^^
       |                      `---- 'g4' is declared 'STRING[80]' and its VAR_GLOBAL is 'STRING[4]'
       |
       | Note: a VAR_EXTERNAL repeats the type of its VAR_GLOBAL exactly
    ---'
    [E0207] Error: VAR_EXTERNAL type mismatch
       ,-[ file:///test0.st:6:32 ]
       |
     6 | VAR_EXTERNAL g4 : STRING; ga : ARRAY[0..1] OF STRING; g : STRING[80]; END_VAR
       |                                ^^^^^^^^^^|^^^^^^^^^^
       |                                          `------------ 'ga' is declared 'ARRAY [0..1] OF STRING' and its VAR_GLOBAL is 'ARRAY [0..1] OF STRING[4]'
       |
       | Note: a VAR_EXTERNAL repeats the type of its VAR_GLOBAL exactly
    ---'
    ");
}
