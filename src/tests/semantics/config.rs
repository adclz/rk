use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::check::diagnostics_for_file;
use hir::hir_def::semantic_index::semantic_index;
use hir::hir_ty::name_res::config_index;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_sources, test_diagnostics, with_db};

/// A valid CONFIGURATION with a single bare TASK and PROGRAM.
#[rstest]
fn valid_config_single_resource(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(PRIORITY := 5);
    PROGRAM RETAIN inst1 WITH t1 : MyProg;
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A valid CONFIGURATION containing an explicit RESOURCE block.
#[rstest]
fn valid_config_with_resource(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    RESOURCE res1 ON CPU_TYPE
        TASK t1(PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A configuration with VAR_GLOBAL variables is valid.
#[rstest]
fn valid_config_global_vars(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    VAR_GLOBAL
        counter : INT;
    END_VAR
    TASK t1(PRIORITY := 10);
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// Duplicate CONFIGURATION names across files should be reported.
#[rstest]
fn duplicate_config_cross_file(mut with_db: RootDatabase) {
    let source1 = r#"
CONFIGURATION MyCfg
    TASK t1(PRIORITY := 1);
END_CONFIGURATION
"#;
    let source2 = r#"
CONFIGURATION MyCfg
    TASK t1(PRIORITY := 2);
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source1, source2]);

    let diagnostics: Vec<_> = with_db
        .get_files()
        .iter()
        .map(|file| diagnostics_for_file(&with_db, *file))
        .collect();

    // One file will have the duplicate error; at least one diagnostic total.
    let total: usize = diagnostics.iter().map(|d| d.len()).sum();
    assert!(total > 0, "expected duplicate config diagnostic");
}

/// config_index should find a configuration by name across the workspace.
#[rstest]
fn config_index_lookup(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    TASK t1(PRIORITY := 5);
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source]);

    let files: Vec<_> = with_db.get_files().iter().map(|f| *f).collect();
    let sema = semantic_index(&with_db, files[0]);

    assert_eq!(sema.configs.len(), 1);

    let name = sema.configs[0].name(&with_db);
    let found = config_index(&with_db, name);
    assert!(found.is_some(), "config_index should find MyCfg by name");
}

/// TASK with SINGLE and INTERVAL data sources.
#[rstest]
fn valid_task_with_single_interval(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    TASK t1(SINGLE := %IX0.0, INTERVAL := T#20ms, PRIORITY := 3);
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// PROGRAM entry referencing a type that has not been declared should report E0218.
#[rstest]
fn invalid_config_unknown_prog_type(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    TASK t1(PRIORITY := 1);
    PROGRAM inst1 WITH t1 : UnknownProg;
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0218] Error: configuration error
       ,-[ file:///test0.st:4:29 ]
       |
     4 |     PROGRAM inst1 WITH t1 : UnknownProg;
       |                             ^^^^^|^^^^^  
       |                                  `------- program type 'UnknownProg' not found
    ---'
    ");
}

/// PROGRAM WITH referencing a task that has not been declared should report E0219.
#[rstest]
fn invalid_config_unknown_task_ref(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    PROGRAM inst1 WITH unknownTask : MyProg;
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0219] Error: configuration error
       ,-[ file:///test0.st:6:24 ]
       |
     6 |     PROGRAM inst1 WITH unknownTask : MyProg;
       |                        ^^^^^|^^^^^  
       |                             `------- task 'unknownTask' not found in this configuration
    ---'
    ");
}

/// A VAR_GLOBAL variable with an elementary type inside CONFIGURATION is valid.
#[rstest]
fn valid_config_global_var_basic_type(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    VAR_GLOBAL
        counter : INT;
        flag : BOOL;
        ratio : REAL;
    END_VAR
    TASK t1(PRIORITY := 1);
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A VAR_GLOBAL variable referencing an unknown type should report E0210.
#[rstest]
fn invalid_config_global_var_unknown_type(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    VAR_GLOBAL
        x : UnknownType;
    END_VAR
    TASK t1(PRIORITY := 1);
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0210] Error: no namespace item found
       ,-[ file:///test0.st:4:13 ]
       |
     4 |         x : UnknownType;
       |             ^^^^^|^^^^^  
       |                  `------- no item found for path 'UnknownType'
    ---'
    ");
}

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
    TASK t1(PRIORITY := 1);
    PROGRAM RETAIN inst1 WITH t1 : MyProg;
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A PROGRAM with VAR_EXTERNAL referencing a VAR_GLOBAL from a RESOURCE block.
#[rstest]
fn valid_var_external_from_resource(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR_EXTERNAL
        flag : BOOL;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    RESOURCE res1 ON CPU_TYPE
        VAR_GLOBAL
            flag : BOOL;
        END_VAR
        TASK t1(PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// VAR_EXTERNAL referencing a name absent from the config's VAR_GLOBAL should report E0220.
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
    TASK t1(PRIORITY := 1);
    PROGRAM RETAIN inst1 WITH t1 : MyProg;
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0220] Error: external variable not found
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         missing : INT;
       |         ^^^^^^|^^^^^^  
       |               `-------- external variable 'missing' not found in any accessible VAR_GLOBAL
    ---'
    ");
}

/// VAR_EXTERNAL in a program that is not instantiated by any config should report E0220.
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
    [E0220] Error: external variable not found
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         orphan : INT;
       |         ^^^^^^|^^^^^  
       |               `------- external variable 'orphan' not found in any accessible VAR_GLOBAL
    ---'
    ");
}

/// PROGRAM inside a RESOURCE block with an unknown task reference should report E0219.
#[rstest]
fn invalid_resource_unknown_task_ref(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    RESOURCE res1 ON CPU_TYPE
        TASK t1(PRIORITY := 1);
        PROGRAM inst1 WITH noSuchTask : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0219] Error: configuration error
       ,-[ file:///test0.st:8:28 ]
       |
     8 |         PROGRAM inst1 WITH noSuchTask : MyProg;
       |                            ^^^^^|^^^^  
       |                                 `------ task 'noSuchTask' not found in this configuration
    ---'
    ");
}

// ── VAR_ACCESS tests ──────────────────────────────────────────────────

/// A valid PROGRAM with VAR_ACCESS referencing an existing variable with matching type.
#[rstest]
fn valid_prog_access_decl(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR
        x : INT;
    END_VAR
    VAR_ACCESS
        ABLE : x : INT READ_ONLY;
    END_VAR
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A VAR_ACCESS declaration with an unknown spec type should report E0210.
#[rstest]
fn invalid_prog_access_unknown_type(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR
        x : INT;
    END_VAR
    VAR_ACCESS
        ABLE : x : UnknownType READ_ONLY;
    END_VAR
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0210] Error: no namespace item found
       ,-[ file:///test0.st:7:20 ]
       |
     7 |         ABLE : x : UnknownType READ_ONLY;
       |                    ^^^^^|^^^^^  
       |                         `------- no item found for path 'UnknownType'
    ---'
    ");
}

/// A VAR_ACCESS declaration referencing a nonexistent variable should report E0204.
#[rstest]
fn invalid_prog_access_unknown_var(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR
        x : INT;
    END_VAR
    VAR_ACCESS
        ABLE : nonexistent : INT READ_ONLY;
    END_VAR
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r#"
    [E0204] Error: no item found in scope
       ,-[ file:///test0.st:7:16 ]
       |
     7 |         ABLE : nonexistent : INT READ_ONLY;
       |                ^^^^^|^^^^^  
       |                     `------- no item "nonexistent" found in scope
    ---'
    "#);
}

/// A VAR_ACCESS declared type differs from the actual variable type should report E0221.
#[rstest]
fn invalid_prog_access_type_mismatch(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR
        x : INT;
    END_VAR
    VAR_ACCESS
        ABLE : x : REAL READ_ONLY;
    END_VAR
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0221] Error: access declaration type mismatch
       ,-[ file:///test0.st:7:20 ]
       |
     4 |         x : INT;
       |         |  
       |         `-- variable 'x' is declared here
       | 
     7 |         ABLE : x : REAL READ_ONLY;
       |                    ^^|^  
       |                      `--- access declaration expects 'REAL', but variable has type 'INT'
    ---'
    ");
}

// ── VAR_CONFIG tests ────────────────────────────────────────────────────

/// A valid VAR_CONFIG overriding an INT variable in a program instance.
#[rstest]
fn valid_config_inst_init(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR
        x : INT;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(PRIORITY := 1);
    PROGRAM inst1 WITH t1 : MyProg;

    VAR_CONFIG
        inst1.x : INT := 42;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A valid VAR_CONFIG overriding a variable inside a nested function block.
#[rstest]
fn valid_config_inst_init_nested_fb(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK InnerFB
    VAR
        param : BOOL;
    END_VAR
END_FUNCTION_BLOCK

PROGRAM MyProg
    VAR
        fb1 : InnerFB;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(PRIORITY := 1);
    PROGRAM inst1 WITH t1 : MyProg;

    VAR_CONFIG
        inst1.fb1.param : BOOL := TRUE;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// VAR_CONFIG with an unknown program instance should report E0222.
#[rstest]
fn invalid_config_inst_init_unknown_instance(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR
        x : INT;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(PRIORITY := 1);
    PROGRAM inst1 WITH t1 : MyProg;

    VAR_CONFIG
        noSuchInst.x : INT := 42;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0222] Error: configuration error
        ,-[ file:///test0.st:13:9 ]
        |
     13 |         noSuchInst.x : INT := 42;
        |         ^^^^^|^^^^  
        |              `------ no program instance 'noSuchInst' found in this configuration
    ----'
    ");
}

/// VAR_CONFIG referencing a nonexistent field on a program should report E0223.
#[rstest]
fn invalid_config_inst_init_unknown_field(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR
        x : INT;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(PRIORITY := 1);
    PROGRAM inst1 WITH t1 : MyProg;

    VAR_CONFIG
        inst1.nonexistent : INT := 42;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0223] Error: configuration error
        ,-[ file:///test0.st:13:15 ]
        |
     13 |         inst1.nonexistent : INT := 42;
        |               ^^^^^|^^^^^  
        |                    `------- 'MyProg' has no field named 'nonexistent'
    ----'
    ");
}

/// VAR_CONFIG init value type mismatch should report an error from init inference.
#[rstest]
fn invalid_config_inst_init_type_mismatch(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR
        x : INT;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(PRIORITY := 1);
    PROGRAM inst1 WITH t1 : MyProg;

    VAR_CONFIG
        inst1.x : INT := 'hello';
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:13:23 ]
        |
     13 |         inst1.x : INT := 'hello';
        |                       ^^^^^|^^^^  
        |                            `------ expected 'INT', got 'STRING'
    ----'
    ");
}

/// VAR_CONFIG trying to walk through a non-composite type should report E0223.
#[rstest]
fn invalid_config_inst_init_not_walkable(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR
        x : INT;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(PRIORITY := 1);
    PROGRAM inst1 WITH t1 : MyProg;

    VAR_CONFIG
        inst1.x.deeper : INT := 42;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0223] Error: configuration error
        ,-[ file:///test0.st:13:17 ]
        |
     13 |         inst1.x.deeper : INT := 42;
        |                 ^^^|^^  
        |                    `---- 'INT' has no field named 'deeper'
    ----'
    ");
}

/// VAR_CONFIG inside a RESOURCE block with a nested FB path.
#[rstest]
fn valid_config_inst_init_in_resource(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK InnerFB
    VAR
        value : REAL;
    END_VAR
END_FUNCTION_BLOCK

PROGRAM MyProg
    VAR
        fb1 : InnerFB;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    RESOURCE res1 ON CPU_TYPE
        TASK t1(PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE

    VAR_CONFIG
        inst1.fb1.value : REAL := 3.14;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}
