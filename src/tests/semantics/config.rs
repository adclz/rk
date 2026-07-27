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
use rstest::rstest;

use crate::tests::utils::{add_sources, test_diagnostics, with_db};

/// A valid CONFIGURATION with a single bare TASK and PROGRAM.
#[rstest]
fn valid_config_single_resource(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(INTERVAL := T#10ms, PRIORITY := 5);
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
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
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
    TASK t1(INTERVAL := T#10ms, PRIORITY := 10);
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// Duplicate CONFIGURATION names across files should be reported.
#[rstest]
fn duplicate_config_cross_file(mut with_db: RootDatabase) {
    let source1 = r#"
CONFIGURATION MyCfg
    TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
END_CONFIGURATION
"#;
    let source2 = r#"
CONFIGURATION MyCfg
    TASK t1(INTERVAL := T#10ms, PRIORITY := 2);
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
    TASK t1(INTERVAL := T#10ms, PRIORITY := 5);
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
    TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
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
    TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
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
    TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
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
    TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
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
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
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
    TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
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
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
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
fn config_inst_init_resolves_but_is_unapplied(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR
        x : INT;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM inst1 WITH t1 : MyProg;

    VAR_CONFIG
        inst1.x : INT := 42;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0240] Error: unsupported configuration element
        ,-[ file:///test0.st:13:15 ]
        |
     13 |         inst1.x : INT := 42;
        |               |
        |               `-- VAR_CONFIG is checked but not applied yet, so this value never reaches the instance
        |
        | Note: set the value in the program's own VAR declaration instead
    ----'
    ");
}

/// A valid VAR_CONFIG overriding a variable inside a nested function block.
#[rstest]
fn config_inst_init_nested_fb_resolves_but_is_unapplied(mut with_db: RootDatabase) {
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
    TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM inst1 WITH t1 : MyProg;

    VAR_CONFIG
        inst1.fb1.param : BOOL := TRUE;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0240] Error: unsupported configuration element
        ,-[ file:///test0.st:19:19 ]
        |
     19 |         inst1.fb1.param : BOOL := TRUE;
        |                   ^^|^^
        |                     `---- VAR_CONFIG is checked but not applied yet, so this value never reaches the instance
        |
        | Note: set the value in the program's own VAR declaration instead
    ----'
    ");
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
    TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM inst1 WITH t1 : MyProg;

    VAR_CONFIG
        noSuchInst.x : INT := 42;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0240] Error: unsupported configuration element
        ,-[ file:///test0.st:13:20 ]
        |
     13 |         noSuchInst.x : INT := 42;
        |                    |
        |                    `-- VAR_CONFIG is checked but not applied yet, so this value never reaches the instance
        |
        | Note: set the value in the program's own VAR declaration instead
    ----'
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
    TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM inst1 WITH t1 : MyProg;

    VAR_CONFIG
        inst1.nonexistent : INT := 42;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0240] Error: unsupported configuration element
        ,-[ file:///test0.st:13:15 ]
        |
     13 |         inst1.nonexistent : INT := 42;
        |               ^^^^^|^^^^^
        |                    `------- VAR_CONFIG is checked but not applied yet, so this value never reaches the instance
        |
        | Note: set the value in the program's own VAR declaration instead
    ----'
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
    TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM inst1 WITH t1 : MyProg;

    VAR_CONFIG
        inst1.x : INT := 'hello';
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0240] Error: unsupported configuration element
        ,-[ file:///test0.st:13:15 ]
        |
     13 |         inst1.x : INT := 'hello';
        |               |
        |               `-- VAR_CONFIG is checked but not applied yet, so this value never reaches the instance
        |
        | Note: set the value in the program's own VAR declaration instead
    ----'
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
    TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM inst1 WITH t1 : MyProg;

    VAR_CONFIG
        inst1.x.deeper : INT := 42;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0240] Error: unsupported configuration element
        ,-[ file:///test0.st:13:17 ]
        |
     13 |         inst1.x.deeper : INT := 42;
        |                 ^^^|^^
        |                    `---- VAR_CONFIG is checked but not applied yet, so this value never reaches the instance
        |
        | Note: set the value in the program's own VAR declaration instead
    ----'
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
fn config_inst_init_in_resource_resolves_but_is_unapplied(mut with_db: RootDatabase) {
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
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE

    VAR_CONFIG
        inst1.fb1.value : REAL := 3.14;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0240] Error: unsupported configuration element
        ,-[ file:///test0.st:21:19 ]
        |
     21 |         inst1.fb1.value : REAL := 3.14;
        |                   ^^|^^
        |                     `---- VAR_CONFIG is checked but not applied yet, so this value never reaches the instance
        |
        | Note: set the value in the program's own VAR declaration instead
    ----'
    ");
}

/// Spec::infer() on a ProgConfig's prog_type should resolve to Type::Program.
#[rstest]
fn config_prog_type_resolves_to_type_program(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(INTERVAL := T#10ms, PRIORITY := 5);
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
            assert!(
                matches!(ty, Type::Program(_)),
                "expected Type::Program, got {ty:?}"
            );
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
    TASK t1(INTERVAL := T#10ms, PRIORITY := 5);
    PROGRAM inst1 WITH t1 : MyProg;
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source]);

    let file = *with_db.get_files().iter().last().unwrap();
    let sema = semantic_index(&with_db, file);
    let config = sema.configs[0];

    let result = infer_config_result(&with_db, config);
    assert_eq!(
        result.task_of_prog.len(),
        1,
        "should have one task_of_prog entry"
    );

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
    TASK t1(INTERVAL := T#10ms, PRIORITY := 5);
    PROGRAM inst1 WITH t1 : MyProg;
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source]);

    let file = *with_db.get_files().iter().last().unwrap();
    let sema = semantic_index(&with_db, file);
    let config = sema.configs[0];

    let result = infer_config_result(&with_db, config);
    assert_eq!(
        result.prog_instance.len(),
        1,
        "should have one prog_instance entry"
    );

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
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source]);

    let file = *with_db.get_files().iter().last().unwrap();
    let sema = semantic_index(&with_db, file);
    let config = sema.configs[0];

    let result = infer_config_result(&with_db, config);
    assert_eq!(
        result.task_of_prog.len(),
        1,
        "should have one task_of_prog entry from resource"
    );
    assert_eq!(
        result.prog_instance.len(),
        1,
        "should have one prog_instance entry from resource"
    );
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

/// A RESOURCE shares the CONFIGURATION's scope, so its `VAR_GLOBAL`s must be
/// checked exactly like the configuration's own: a bad initializer is a type
/// error and a repeated name is a duplicate. Both were silently skipped, since
/// the scope only ever enumerated the configuration's own variables.
#[rstest]
fn resource_global_bad_initializer_is_a_type_error(mut with_db: db::RootDatabase) {
    let source = r#"
        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                VAR_GLOBAL
                    bad : INT := 'oops';
                END_VAR
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:5:31 ]
       |
     5 |                     bad : INT := 'oops';
       |                               ^^^^|^^^^
       |                                   `------ expected 'INT', got 'STRING'
    ---'
    ");
}

#[rstest]
fn duplicate_resource_globals_are_reported(mut with_db: db::RootDatabase) {
    let source = r#"
        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                VAR_GLOBAL
                    g : INT;
                    g : INT;
                END_VAR
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0102] Error: duplicate definitions
       ,-[ file:///test0.st:6:21 ]
       |
     5 |                     g : INT;
       |                     |
       |                     `-- variable 'g' is already defined here
     6 |                     g : INT;
       |                     |
       |                     `-- duplicate variable 'g'
    ---'
    ");
}

/// A PROGRAM bound to a TASK the scheduler cannot honour must be rejected at
/// check time, not compiled into a core with no schedule.
///
/// Each of these used to produce 0 errors and 0 warnings, compile
/// successfully, and then fail at `rk sim` with an unrelated complaint about
/// the program's body export — the program simply never ran and nothing said
/// why. The scheduler dropped them with a bare `continue`.
#[rstest]
fn event_driven_task_is_rejected(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            RESOURCE R ON CPU
                TASK T(SINGLE := go, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0239] Error: task cannot be scheduled
       ,-[ file:///test0.st:5:22 ]
       |
     5 |                 TASK T(SINGLE := go, PRIORITY := 1);
       |                      |
       |                      `-- task 'T' cannot be scheduled: event-driven tasks (SINGLE) are not supported yet; only cyclic tasks run
       |
       | Note: use a cyclic period, e.g. `INTERVAL := T#10ms`
    ---'
    ");
}

#[rstest]
fn zero_interval_task_is_rejected(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            RESOURCE R ON CPU
                TASK T(INTERVAL := T#0ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0239] Error: task cannot be scheduled
       ,-[ file:///test0.st:5:22 ]
       |
     5 |                 TASK T(INTERVAL := T#0ms, PRIORITY := 1);
       |                      |
       |                      `-- task 'T' cannot be scheduled: INTERVAL must be greater than zero
    ---'
    ");
}

/// A name that resolves to nothing cannot supply a period.
#[rstest]
fn non_literal_interval_is_rejected(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            RESOURCE R ON CPU
                TASK T(INTERVAL := someName, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0239] Error: task cannot be scheduled
       ,-[ file:///test0.st:5:22 ]
       |
     5 |                 TASK T(INTERVAL := someName, PRIORITY := 1);
       |                      |
       |                      `-- task 'T' cannot be scheduled: INTERVAL must be a TIME literal or a CONSTANT global holding one
       |
       | Note: declare the global `VAR_GLOBAL CONSTANT`
    ---'
    ");
}

/// A CONSTANT global holding a TIME literal IS a compile-time period, and must
/// be accepted — "known at compile time" is wider than "written as a literal",
/// and rejecting it would state a language rule to cover a missing resolution.
#[rstest]
fn constant_global_interval_is_accepted(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            VAR_GLOBAL CONSTANT period : TIME := T#10ms; END_VAR
            RESOURCE R ON CPU
                TASK T(INTERVAL := period, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// ...but a global WITHOUT `CONSTANT` may be written while the PLC runs, and
/// the emitted schedule cannot follow it.
#[rstest]
fn mutable_global_interval_is_rejected(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            VAR_GLOBAL period : TIME := T#10ms; END_VAR
            RESOURCE R ON CPU
                TASK T(INTERVAL := period, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0239] Error: task cannot be scheduled
       ,-[ file:///test0.st:6:22 ]
       |
     6 |                 TASK T(INTERVAL := period, PRIORITY := 1);
       |                      |
       |                      `-- task 'T' cannot be scheduled: INTERVAL must be a TIME literal or a CONSTANT global holding one
       |
       | Note: declare the global `VAR_GLOBAL CONSTANT`
    ---'
    ");
}

#[rstest]
fn program_instance_without_a_task_is_rejected(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            RESOURCE R ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0238] Error: program instance never runs
       ,-[ file:///test0.st:6:25 ]
       |
     6 |                 PROGRAM P1 : P;
       |                         ^|
       |                          `-- program instance 'P1' has no WITH <task>, so it will never run
    ---'
    ");
}

/// A TASK with neither SINGLE nor INTERVAL triggers nothing, so a PROGRAM
/// bound to it never runs — the same silent outcome as the other unschedulable
/// shapes, and reported the same way.
#[rstest]
fn trigger_less_task_is_rejected(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            RESOURCE R ON CPU
                TASK T(PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0239] Error: task cannot be scheduled
       ,-[ file:///test0.st:5:22 ]
       |
     5 |                 TASK T(PRIORITY := 1);
       |                      |
       |                      `-- task 'T' cannot be scheduled: a TASK needs an INTERVAL to run its programs
       |
       | Note: add `INTERVAL := T#10ms`
    ---'
    ");
}

/// ...but the same TASK with nothing bound to it stays clean. Most of this
/// suite's LSP fixtures declare exactly this shape because it is the shortest
/// thing that parses, and none of them care whether it could run.
#[rstest]
fn an_unused_trigger_less_task_is_accepted(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            RESOURCE R ON CPU
                TASK Spare(PRIORITY := 9);
                TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH Fast : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// An unschedulable TASK that nothing is bound to is NOT an error: it costs
/// nobody anything, and a workspace may declare tasks ahead of using them.
/// What must never be silent is a PROGRAM that cannot run.
#[rstest]
fn an_unused_unschedulable_task_is_accepted(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            RESOURCE R ON CPU
                TASK Later(SINGLE := go, PRIORITY := 9);
                TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH Fast : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// The ordinary shape stays clean — these checks reject what cannot run, not
/// what merely looks unusual.
#[rstest]
fn a_schedulable_configuration_is_accepted(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            RESOURCE R ON CPU
                TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
                TASK Slow(INTERVAL := LTIME#1s, PRIORITY := 5);
                PROGRAM P1 WITH Fast : P;
                PROGRAM P2 WITH Slow : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A `PROGRAM ... (...)` connection list is parsed and then discarded: no copy
/// is emitted around the scan and the names are never resolved, so `ghost` and
/// `nosuch` — neither of which exists — used to compile clean.
#[rstest]
fn program_connection_elements_are_reported(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM A
        VAR_INPUT inp : INT; END_VAR
        VAR_OUTPUT outp : INT; END_VAR
            outp := inp * 2;
        END_PROGRAM

        CONFIGURATION Cfg
            VAR_GLOBAL src : INT := 7; snk : INT; END_VAR
            RESOURCE R ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM PA WITH T : A (inp := src, outp => snk, ghost := nosuch);
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0240] Error: unsupported configuration element
        ,-[ file:///test0.st:12:40 ]
        |
     12 |                 PROGRAM PA WITH T : A (inp := src, outp => snk, ghost := nosuch);
        |                                        ^|^
        |                                         `--- program connection lists are parsed but not wired up yet, so this has no effect
        |
        | Note: assign it in the program body instead
    ----'
    [E0240] Error: unsupported configuration element
        ,-[ file:///test0.st:12:52 ]
        |
     12 |                 PROGRAM PA WITH T : A (inp := src, outp => snk, ghost := nosuch);
        |                                                    ^^|^
        |                                                      `--- program connection lists are parsed but not wired up yet, so this has no effect
        |
        | Note: assign it in the program body instead
    ----'
    [E0240] Error: unsupported configuration element
        ,-[ file:///test0.st:12:65 ]
        |
     12 |                 PROGRAM PA WITH T : A (inp := src, outp => snk, ghost := nosuch);
        |                                                                 ^^|^^
        |                                                                   `---- program connection lists are parsed but not wired up yet, so this has no effect
        |
        | Note: assign it in the program body instead
    ----'
    ");
}

/// Two RESOURCEs each declaring `g` used to alias to ONE address — the earlier
/// allocation was dead and the later one won for every body, silently. They are
/// now rejected: this implementation flattens resources into one memory, so it
/// cannot give them separate storage, and refusing beats aliasing.
///
/// Note this rejects something IEC permits, since resource globals are meant to
/// be resource-scoped. That is a limitation of the flattened model, not a rule.
#[rstest]
fn same_named_globals_in_two_resources_are_rejected(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM P VAR_EXTERNAL g : INT; END_VAR VAR n : INT; END_VAR n := g; END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE R1 ON CPU
                VAR_GLOBAL g : INT := 10; END_VAR
                TASK T1(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM A1 WITH T1 : P;
            END_RESOURCE
            RESOURCE R2 ON CPU
                VAR_GLOBAL g : INT := 99; END_VAR
                TASK T2(INTERVAL := T#10ms, PRIORITY := 2);
                PROGRAM A2 WITH T2 : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0102] Error: duplicate definitions
        ,-[ file:///test0.st:11:28 ]
        |
      6 |                 VAR_GLOBAL g : INT := 10; END_VAR
        |                            |
        |                            `-- variable 'g' is already defined here
        |
     11 |                 VAR_GLOBAL g : INT := 99; END_VAR
        |                            |
        |                            `-- duplicate variable 'g'
    ----'
    ");
}
