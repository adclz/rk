use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use hir::hir_ty::config::infer_config_result;
use hir::hir_ty::index_graphs::config_fragments;
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
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 5);
        PROGRAM RETAIN inst1 WITH t1 : MyProg;
    END_RESOURCE
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
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 10);
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// The same RESOURCE name in two fragments of one CONFIGURATION: the name a
/// deployment binds to, claimed twice. Reported symmetrically at each
/// fragment, the sibling as related.
#[rstest]
fn duplicate_resource_across_fragments_is_rejected(mut with_db: RootDatabase) {
    let source1 = r#"
CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
    END_RESOURCE
END_CONFIGURATION
"#;
    let source2 = r#"
CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 2);
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source1, source2]), @r"
    [E0116] Error: duplicate definitions
       ,-[ file:///test0.st:3:14 ]
       |
     3 |     RESOURCE Res ON CPU
       |              ^|^
       |               `--- duplicate resource 'Res'
       |
       |-[ file:///test1.st:3:14 ]
       |
     3 |     RESOURCE Res ON CPU
       |              ^|^
       |               `--- resource 'Res' is already defined here
    ---'
    [E0116] Error: duplicate definitions
       ,-[ file:///test1.st:3:14 ]
       |
     3 |     RESOURCE Res ON CPU
       |              ^|^
       |               `--- duplicate resource 'Res'
       |
       |-[ file:///test0.st:3:14 ]
       |
     3 |     RESOURCE Res ON CPU
       |              ^|^
       |               `--- resource 'Res' is already defined here
    ---'
    ");
}

/// The same VAR_GLOBAL in two fragments: two memory slots for one name, and
/// resolution would pick one nondeterministically. Reported symmetrically.
#[rstest]
fn duplicate_global_across_fragments_is_rejected(mut with_db: RootDatabase) {
    let source1 = r#"
CONFIGURATION MyCfg
    VAR_GLOBAL
        shared : INT;
    END_VAR
END_CONFIGURATION
"#;
    let source2 = r#"
CONFIGURATION MyCfg
    VAR_GLOBAL
        shared : DINT;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source1, source2]), @r"
    [E0102] Error: duplicate definitions
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         shared : INT;
       |         ^^^|^^
       |            `---- duplicate variable 'shared'
       |
       |-[ file:///test1.st:4:9 ]
       |
     4 |         shared : DINT;
       |         ^^^|^^
       |            `---- variable 'shared' is already defined here
    ---'
    [E0102] Error: duplicate definitions
       ,-[ file:///test1.st:4:9 ]
       |
     4 |         shared : DINT;
       |         ^^^|^^
       |            `---- duplicate variable 'shared'
       |
       |-[ file:///test0.st:4:9 ]
       |
     4 |         shared : INT;
       |         ^^^|^^
       |            `---- variable 'shared' is already defined here
    ---'
    ");
}

/// The GVL use-case end to end: one fragment holds only VAR_GLOBALs, another
/// holds the resources, and a POU reaches the global through VAR_EXTERNAL.
#[rstest]
fn fragments_split_globals_and_resources_merge(mut with_db: RootDatabase) {
    let globals = r#"
CONFIGURATION Plant
    VAR_GLOBAL
        line_speed : INT;
    END_VAR
END_CONFIGURATION
"#;
    let machine = r#"
PROGRAM Conveyor
VAR_EXTERNAL
    line_speed : INT;
END_VAR
    line_speed := line_speed + 1;
END_PROGRAM

CONFIGURATION Plant
    RESOURCE Main ON CPU
        TASK Cyclic(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH Cyclic : Conveyor;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[globals, machine]), @r"");
}

/// config_fragments should find a configuration's blocks by name across the
/// workspace.
#[rstest]
fn config_fragments_lookup(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 5);
    END_RESOURCE
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source]);

    let files: Vec<_> = with_db.get_files().iter().map(|f| *f).collect();
    let sema = semantic_index(&with_db, files[0]);

    assert_eq!(sema.configs.len(), 1);

    let name = sema.configs[0].name(&with_db);
    let found = config_fragments(&with_db, name);
    assert_eq!(found.len(), 1, "config_fragments should find MyCfg by name");
}

/// TASK with SINGLE and INTERVAL data sources.
#[rstest]
fn valid_task_with_single_interval(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(SINGLE := %IX0.0, INTERVAL := T#20ms, PRIORITY := 3);
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// PROGRAM entry referencing a type that has not been declared should report E0218.
#[rstest]
fn invalid_config_unknown_prog_type(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : UnknownProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0210] Error: no namespace item found
       ,-[ file:///test0.st:5:33 ]
       |
     5 |         PROGRAM inst1 WITH t1 : UnknownProg;
       |                                 ^^^^^|^^^^^
       |                                      `------- no item found for path 'UnknownProg'
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
    RESOURCE Res ON CPU
        PROGRAM inst1 WITH unknownTask : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0219] Error: configuration error
       ,-[ file:///test0.st:7:28 ]
       |
     7 |         PROGRAM inst1 WITH unknownTask : MyProg;
       |                            ^^^^^|^^^^^
       |                                 `------- task 'unknownTask' not found in this configuration
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
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
    END_RESOURCE
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
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
    END_RESOURCE
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
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM RETAIN inst1 WITH t1 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A CONFIGURATION's VAR_GLOBAL reaches a PROGRAM instantiated inside a
/// RESOURCE: globals are application-scoped, and a RESOURCE — being only a
/// named group of tasks and programs — does not narrow what its programs see.
#[rstest]
fn config_global_reaches_a_program_inside_a_resource(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR_EXTERNAL
        flag : BOOL;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    VAR_GLOBAL
        flag : BOOL;
    END_VAR
    RESOURCE res1 ON CPU_TYPE
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
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM RETAIN inst1 WITH t1 : MyProg;
    END_RESOURCE
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
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE

    VAR_CONFIG
        inst1.x : INT := 42;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0240] Error: unsupported configuration element
        ,-[ file:///test0.st:15:15 ]
        |
     15 |         inst1.x : INT := 42;
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
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE

    VAR_CONFIG
        inst1.fb1.param : BOOL := TRUE;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0240] Error: unsupported configuration element
        ,-[ file:///test0.st:21:19 ]
        |
     21 |         inst1.fb1.param : BOOL := TRUE;
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
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE

    VAR_CONFIG
        noSuchInst.x : INT := 42;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0240] Error: unsupported configuration element
        ,-[ file:///test0.st:15:20 ]
        |
     15 |         noSuchInst.x : INT := 42;
        |                    |
        |                    `-- VAR_CONFIG is checked but not applied yet, so this value never reaches the instance
        |
        | Note: set the value in the program's own VAR declaration instead
    ----'
    [E0222] Error: configuration error
        ,-[ file:///test0.st:15:9 ]
        |
     15 |         noSuchInst.x : INT := 42;
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
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE

    VAR_CONFIG
        inst1.nonexistent : INT := 42;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0240] Error: unsupported configuration element
        ,-[ file:///test0.st:15:15 ]
        |
     15 |         inst1.nonexistent : INT := 42;
        |               ^^^^^|^^^^^
        |                    `------- VAR_CONFIG is checked but not applied yet, so this value never reaches the instance
        |
        | Note: set the value in the program's own VAR declaration instead
    ----'
    [E0223] Error: configuration error
        ,-[ file:///test0.st:15:15 ]
        |
     15 |         inst1.nonexistent : INT := 42;
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
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE

    VAR_CONFIG
        inst1.x : INT := 'hello';
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0240] Error: unsupported configuration element
        ,-[ file:///test0.st:15:15 ]
        |
     15 |         inst1.x : INT := 'hello';
        |               |
        |               `-- VAR_CONFIG is checked but not applied yet, so this value never reaches the instance
        |
        | Note: set the value in the program's own VAR declaration instead
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:15:23 ]
        |
     15 |         inst1.x : INT := 'hello';
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
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE

    VAR_CONFIG
        inst1.x.deeper : INT := 42;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0240] Error: unsupported configuration element
        ,-[ file:///test0.st:15:17 ]
        |
     15 |         inst1.x.deeper : INT := 42;
        |                 ^^^|^^
        |                    `---- VAR_CONFIG is checked but not applied yet, so this value never reaches the instance
        |
        | Note: set the value in the program's own VAR declaration instead
    ----'
    [E0223] Error: configuration error
        ,-[ file:///test0.st:15:17 ]
        |
     15 |         inst1.x.deeper : INT := 42;
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
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 5);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source]);

    let file = *with_db.get_files().iter().last().unwrap();
    let sema = semantic_index(&with_db, file);
    let config = sema.configs[0];

    // Find the ProgConfig's prog_type Spec and check it infers to Type::Program
    let mut found = false;
    for r in config.resources(&with_db).iter() {
        for p in r.programs(&with_db).iter() {
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
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 5);
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
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 5);
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

/// A CONFIGURATION's `VAR_GLOBAL`s are checked like any other declaration: a
/// bad initializer is a type error.
#[rstest]
fn config_global_bad_initializer_is_a_type_error(mut with_db: db::RootDatabase) {
    let source = r#"
        CONFIGURATION Cfg
            VAR_GLOBAL
                bad : INT := 'oops';
            END_VAR
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:27 ]
       |
     4 |                 bad : INT := 'oops';
       |                           ^^^^|^^^^
       |                               `------ expected 'INT', got 'STRING'
    ---'
    ");
}

#[rstest]
fn duplicate_config_globals_are_reported(mut with_db: db::RootDatabase) {
    let source = r#"
        CONFIGURATION Cfg
            VAR_GLOBAL
                g : INT;
                g : INT;
            END_VAR
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0102] Error: duplicate definitions
       ,-[ file:///test0.st:5:17 ]
       |
     4 |                 g : INT;
       |                 |
       |                 `-- variable 'g' is already defined here
     5 |                 g : INT;
       |                 |
       |                 `-- duplicate variable 'g'
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

/// PRIORITY is held as source text by the declaration, so an unusable value
/// used to reach the scheduler as `None` — indistinguishable from "no
/// PRIORITY given" — and sort last with nothing said. HIR resolves it now.
#[rstest]
fn unusable_task_priority_is_rejected(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM P VAR n : INT; END_VAR n := n + 1; END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE R ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 99999999999);
                PROGRAM A WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0241] Error: configuration error
       ,-[ file:///test0.st:6:22 ]
       |
     6 |                 TASK T(INTERVAL := T#10ms, PRIORITY := 99999999999);
       |                      |
       |                      `-- task 'T' has an unusable PRIORITY '99999999999'
       |
       | Note: PRIORITY must fit in a 32-bit unsigned integer; 0 is the most urgent
    ---'
    ");
}

/// Tasks and programs belong to a RESOURCE. Written straight into the
/// CONFIGURATION they used to fail as stray tokens — one E0050 quoting the
/// whole tokenized configuration — which said nothing about what to do.
#[rstest]
fn task_or_program_outside_a_resource_is_rejected(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM P VAR n : INT; END_VAR n := n + 1; END_PROGRAM

        CONFIGURATION Cfg
            TASK T(INTERVAL := T#10ms, PRIORITY := 1);
            PROGRAM A WITH T : P;
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0039] Error: syntax
       ,-[ file:///test0.st:5:13 ]
       |
     5 |             TASK T(INTERVAL := T#10ms, PRIORITY := 1);
       |             ^^^^^^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^^^^^
       |                                  `---------------------- TASK and PROGRAM must be declared inside a RESOURCE
       |
       | Note: wrap them in a RESOURCE <name> ON <cpu> ... END_RESOURCE block
    ---'
    [E0039] Error: syntax
       ,-[ file:///test0.st:6:13 ]
       |
     6 |             PROGRAM A WITH T : P;
       |             ^^^^^^^^^^|^^^^^^^^^^
       |                       `------------ TASK and PROGRAM must be declared inside a RESOURCE
       |
       | Note: wrap them in a RESOURCE <name> ON <cpu> ... END_RESOURCE block
    ---'
    ");
}

/// `VAR_GLOBAL` is application-scoped: it belongs to the CONFIGURATION, never
/// to a RESOURCE. A RESOURCE is a named group of tasks and programs and holds
/// no variables of its own, so a `VAR_GLOBAL` inside one is rejected.
///
/// This removes a whole class of ambiguity with it: two RESOURCEs declaring the
/// same name can no longer exist, so nothing has to decide which one a POU's
/// VAR_EXTERNAL binds to — a question the standard leaves open and which no
/// implementation answers consistently.
#[rstest]
fn var_global_in_a_resource_is_rejected(mut with_db: RootDatabase) {
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
    [E0030] Error: syntax
       ,-[ file:///test0.st:6:17 ]
       |
     6 |                 VAR_GLOBAL g : INT := 10; END_VAR
       |                 ^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^
       |                                 `------------------ VAR_GLOBAL is not allowed in this context
       |
       | Note: VAR_GLOBAL can only be used inside CONFIGURATION
    ---'
    [E0030] Error: syntax
        ,-[ file:///test0.st:11:17 ]
        |
     11 |                 VAR_GLOBAL g : INT := 99; END_VAR
        |                 ^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^
        |                                 `------------------ VAR_GLOBAL is not allowed in this context
        |
        | Note: VAR_GLOBAL can only be used inside CONFIGURATION
    ----'
    [E0220] Error: external variable not found
       ,-[ file:///test0.st:2:32 ]
       |
     2 |         PROGRAM P VAR_EXTERNAL g : INT; END_VAR VAR n : INT; END_VAR n := g; END_PROGRAM
       |                                ^^^|^^^
       |                                   `----- external variable 'g' not found in any accessible VAR_GLOBAL
    ---'
    ");
}

/// A second CONFIGURATION used to be discarded silently — the artifact held
/// part of what was written. It is rejected now, and not only for that: a POU
/// is a type any configuration may use, so a second one leaves no answer to
/// which globals are in scope inside a POU.
#[rstest]
fn more_than_one_configuration_is_rejected(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM P VAR n : INT; END_VAR n := n + 1; END_PROGRAM

        CONFIGURATION First
            RESOURCE R ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM A WITH T : P;
            END_RESOURCE
        END_CONFIGURATION

        CONFIGURATION Second
            RESOURCE R ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM B WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0242] Error: configuration error
        ,-[ file:///test0.st:4:23 ]
        |
      4 |         CONFIGURATION First
        |                       ^^|^^
        |                         `---- a workspace can only have one CONFIGURATION; this one declares 2
        |
     11 |         CONFIGURATION Second
        |                       ^^^|^^
        |                          `---- 'Second' is declared here
    ----'
    [E0242] Error: configuration error
        ,-[ file:///test0.st:11:23 ]
        |
      4 |         CONFIGURATION First
        |                       ^^|^^
        |                         `---- 'First' is declared here
        |
     11 |         CONFIGURATION Second
        |                       ^^^|^^
        |                          `---- a workspace can only have one CONFIGURATION; this one declares 2
    ----'
    ");
}

// ---------------------------------------------------------------------------
// The resolved schedule
//
// `ConfigInferenceResult::schedule` is what lowering consumes: the whole
// execution model, already resolved. These assert its CONTENT directly rather
// than through a compiled artifact — a wrong shape here is a wrong PLC, and
// the failure should name the shape, not a wasm symbol.
// ---------------------------------------------------------------------------

/// Renders the single CONFIGURATION's resolved schedule — what lowering
/// consumes — as text, so a test states the whole execution model in the shape
/// a reader can check against the source above it.
fn resolved_schedule(db: &RootDatabase) -> String {
    use std::fmt::Write;

    /// Nanoseconds back in the units the source wrote them in.
    fn duration(ns: u64) -> String {
        for (unit, per) in [
            ("s", 1_000_000_000u64),
            ("ms", 1_000_000),
            ("us", 1_000),
        ] {
            if ns.is_multiple_of(per) {
                return format!("{}{unit}", ns / per);
            }
        }
        format!("{ns}ns")
    }

    let file = *db.get_files().iter().last().unwrap();
    let config = semantic_index(db, file).configs[0];
    let schedule = &infer_config_result(db, config).schedule;

    if schedule.resources.is_empty() {
        return "(nothing runs)".to_string();
    }

    let mut out = String::new();
    for r in &schedule.resources {
        let _ = writeln!(
            out,
            "RESOURCE {} ON {}",
            r.name.text(db),
            r.cpu_type.text(db)
        );
        for t in &r.tasks {
            let priority = match t.priority {
                Some(p) => format!("priority {p}"),
                None => "no priority".to_string(),
            };
            let _ = writeln!(
                out,
                "  TASK {} every {}, {priority}",
                t.name.text(db),
                duration(t.interval_ns)
            );
            for p in &t.programs {
                let _ = writeln!(
                    out,
                    "    PROGRAM {} : {}",
                    p.instance_name.text(db),
                    p.program.name(db).text(db)
                );
            }
        }
    }
    out
}

/// Every value lowering needs is resolved here: the resource and its `ON` type,
/// the interval in nanoseconds, the priority as a number, and the instances
/// bound to each task. Nothing downstream re-derives or re-parses any of it.
#[rstest]
fn resolved_schedule_carries_resources_tasks_and_instances(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P VAR n : INT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Core0 ON CPU_A
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM PA WITH Fast : P;
        PROGRAM PB WITH Fast : P;
    END_RESOURCE
    RESOURCE Core1 ON CPU_B
        TASK Slow(INTERVAL := T#1s, PRIORITY := 7);
        PROGRAM PC WITH Slow : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(resolved_schedule(&with_db), @r"
    RESOURCE Core0 ON CPU_A
      TASK Fast every 10ms, priority 1
        PROGRAM PA : P
        PROGRAM PB : P
    RESOURCE Core1 ON CPU_B
      TASK Slow every 1s, priority 7
        PROGRAM PC : P
    ");
}

/// Tasks come out most-urgent-first, so a consumer dispatching in slice order
/// is correct without remembering to sort. Declaration order is deliberately
/// the reverse of priority order here.
#[rstest]
fn resolved_schedule_orders_tasks_by_priority(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P VAR n : INT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Cfg
    RESOURCE R ON CPU
        TASK Third(INTERVAL := T#10ms, PRIORITY := 9);
        TASK First(INTERVAL := T#10ms, PRIORITY := 0);
        TASK Second(INTERVAL := T#10ms, PRIORITY := 4);
        PROGRAM P3 WITH Third : P;
        PROGRAM P1 WITH First : P;
        PROGRAM P2 WITH Second : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(resolved_schedule(&with_db), @r"
    RESOURCE R ON CPU
      TASK First every 10ms, priority 0
        PROGRAM P1 : P
      TASK Second every 10ms, priority 4
        PROGRAM P2 : P
      TASK Third every 10ms, priority 9
        PROGRAM P3 : P
    ");
}

/// A task nothing is bound to runs nothing, so it is absent from the model —
/// and a program whose task cannot be scheduled takes its task with it. Both
/// are reported elsewhere; the schedule only describes what runs.
#[rstest]
fn resolved_schedule_omits_what_cannot_run(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P VAR n : INT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Cfg
    RESOURCE R ON CPU
        TASK Bound(INTERVAL := T#10ms, PRIORITY := 1);
        TASK Orphan(INTERVAL := T#10ms, PRIORITY := 2);
        PROGRAM PA WITH Bound : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(resolved_schedule(&with_db), @r"
    RESOURCE R ON CPU
      TASK Bound every 10ms, priority 1
        PROGRAM PA : P
    ");
}

/// A RESOURCE whose tasks all fail to resolve contributes nothing rather than
/// an empty group, so a consumer never has to skip blanks.
#[rstest]
fn resolved_schedule_drops_empty_resources(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P VAR n : INT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Empty ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
    END_RESOURCE
    RESOURCE Real ON CPU
        TASK U(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM PA WITH U : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(resolved_schedule(&with_db), @r"
    RESOURCE Real ON CPU
      TASK U every 10ms, priority 1
        PROGRAM PA : P
    ");
}

/// An INTERVAL need not be a literal: a CONSTANT global is just as fixed, and
/// the schedule stores the resolved nanoseconds either way.
#[rstest]
fn resolved_schedule_resolves_a_constant_interval(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P VAR n : INT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Cfg
    VAR_GLOBAL CONSTANT period : TIME := T#25ms; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := period, PRIORITY := 1);
        PROGRAM PA WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(resolved_schedule(&with_db), @r"
    RESOURCE R ON CPU
      TASK T every 25ms, priority 1
        PROGRAM PA : P
    ");
}

/// PRIORITY is optional in the grammar (its absence is E0035). The schedule
/// still describes what runs, with no priority rather than a guessed one.
#[rstest]
fn resolved_schedule_tolerates_a_missing_priority(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P VAR n : INT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Cfg
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms);
        PROGRAM PA WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(resolved_schedule(&with_db), @r"
    RESOURCE R ON CPU
      TASK T every 10ms, no priority
        PROGRAM PA : P
    ");
}
