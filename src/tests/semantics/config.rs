use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::check::diagnostics_for_file;
use hir::hir_def::config::ConfigResource;
use hir::hir_def::semantic_index::semantic_index;
use hir::hir_ty::config::infer_config_result;
use hir::hir_ty::index_graphs::config_index;
use hir::hir_ty::infer::Infer;
use hir::hir_ty::ty::Type;
use insta::assert_snapshot;
use ide_proto::walk::WalkHir;
use ide_proto::hir_node::HirNode;
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
    [E0210] Error: no namespace item found
       ,-[ file:///test0.st:4:29 ]
       |
     4 |     PROGRAM inst1 WITH t1 : UnknownProg;
       |                             ^^^^^|^^^^^
       |                                  `------- no item found for path 'UnknownProg'
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

/// Spec::infer() on a ProgConfig's prog_type should resolve to Type::Program.
#[rstest]
fn config_prog_type_resolves_to_type_program(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(PRIORITY := 5);
    PROGRAM inst1 WITH t1 : MyProg;
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source]);

    let file = *with_db.get_files().iter().last().unwrap();
    let sema = semantic_index(&with_db, file);
    let config = sema.configs[0];

    // Find the ProgConfig's prog_type Spec and check it infers to Type::Program
    let mut found = false;
    for res in config.resources(&with_db).iter() {
        if let ConfigResource::Program(p) = res {
            let ty = p.prog_type(&with_db).infer(&with_db);
            assert!(matches!(ty, Type::Program(_)), "expected Type::Program, got {ty:?}");
            found = true;
        }
    }
    assert!(found, "should have found a ProgConfig");
}

/// infer_config_result should populate task_of_prog for valid WITH references.
#[rstest]
fn config_infer_result_resolves_task_of_prog(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(PRIORITY := 5);
    PROGRAM inst1 WITH t1 : MyProg;
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source]);

    let file = *with_db.get_files().iter().last().unwrap();
    let sema = semantic_index(&with_db, file);
    let config = sema.configs[0];

    let result = infer_config_result(&with_db, config);
    assert_eq!(result.task_of_prog.len(), 1, "should have one task_of_prog entry");

    let (prog, task) = result.task_of_prog.iter().next().unwrap();
    assert_eq!(task.name(&with_db).ident.text(&with_db), "t1");
    assert_eq!(prog.name(&with_db).ident.text(&with_db), "inst1");
}

/// infer_config_result should populate prog_instance for valid program references.
#[rstest]
fn config_infer_result_resolves_prog_instance(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(PRIORITY := 5);
    PROGRAM inst1 WITH t1 : MyProg;
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source]);

    let file = *with_db.get_files().iter().last().unwrap();
    let sema = semantic_index(&with_db, file);
    let config = sema.configs[0];

    let result = infer_config_result(&with_db, config);
    assert_eq!(result.prog_instance.len(), 1, "should have one prog_instance entry");

    let (_, prog_decl) = result.prog_instance.iter().next().unwrap();
    assert_eq!(prog_decl.name(&with_db).text(&with_db), "MyProg");
}

/// infer_config_result should resolve tasks scoped within a RESOURCE block.
#[rstest]
fn config_infer_result_resource_scoped_task(mut with_db: RootDatabase) {
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
    add_sources(&mut with_db, &[source]);

    let file = *with_db.get_files().iter().last().unwrap();
    let sema = semantic_index(&with_db, file);
    let config = sema.configs[0];

    let result = infer_config_result(&with_db, config);
    assert_eq!(result.task_of_prog.len(), 1, "should have one task_of_prog entry from resource");
    assert_eq!(result.prog_instance.len(), 1, "should have one prog_instance entry from resource");
    assert!(result.errors.is_empty(), "should have no errors");
}

/// Programs should NOT be resolvable as types from non-config scopes.
#[rstest]
fn program_not_resolvable_from_non_config_scope(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

FUNCTION_BLOCK fb1
    VAR
        x : MyProg;
    END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0210] Error: no namespace item found
       ,-[ file:///test0.st:7:13 ]
       |
     7 |         x : MyProg;
       |             ^^^|^^
       |                `---- no item found for path 'MyProg'
    ---'
    ");
}
