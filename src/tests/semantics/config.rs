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
