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
    [E0115] Error: duplicate definitions
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
    [E0115] Error: duplicate definitions
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
    [E0101] Error: duplicate definitions
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
    [E0101] Error: duplicate definitions
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

/// PROGRAM entry referencing a type that has not been declared should report E0203.
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
    [E0203] Error: no namespace item found
       ,-[ file:///test0.st:5:33 ]
       |
     5 |         PROGRAM inst1 WITH t1 : UnknownProg;
       |                                 ^^^^^|^^^^^
       |                                      `------- no item found for path 'UnknownProg'
    ---'
    ");
}

/// PROGRAM WITH referencing a task that has not been declared should report E1411.
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
    [E1411] Error: configuration error
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

/// A VAR_GLOBAL variable referencing an unknown type should report E0203.
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
    [E0203] Error: no namespace item found
       ,-[ file:///test0.st:4:13 ]
       |
     4 |         x : UnknownType;
       |             ^^^^^|^^^^^
       |                  `------- no item found for path 'UnknownType'
    ---'
    ");
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

/// PROGRAM inside a RESOURCE block with an unknown task reference should report E1411.
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
    [E1411] Error: configuration error
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

/// A VAR_ACCESS declaration with an unknown spec type should report E0203.
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
    [E0203] Error: no namespace item found
       ,-[ file:///test0.st:7:20 ]
       |
     7 |         ABLE : x : UnknownType READ_ONLY;
       |                    ^^^^^|^^^^^
       |                         `------- no item found for path 'UnknownType'
    ---'
    ");
}

/// A VAR_ACCESS declaration referencing a nonexistent variable should report E0201.
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
    [E0201] Error: no item found in scope
       ,-[ file:///test0.st:7:16 ]
       |
     7 |         ABLE : nonexistent : INT READ_ONLY;
       |                ^^^^^|^^^^^
       |                     `------- no item "nonexistent" found in scope
    ---'
    "#);
}

/// A VAR_ACCESS declared type differs from the actual variable type should report E1415.
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
    [E1415] Error: access declaration type mismatch
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
/// A VAR_CONFIG value is refused where it cannot be the variable's starting
/// value: a second one for the same variable, or for an instance holding it
/// (E1426), a member with no value of its own (E0405), and, by the rules a
/// declaration's initial value follows, an input the host overwrites (E1419),
/// a part of a wider address (E1423) and a channel a declaration names, whose
/// value stands (E1426).
#[rstest]
fn invalid_var_config_values(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Drive
VAR out AT %Q* : INT; level AT %I* : INT; b AT %Q* : BYTE; END_VAR
VAR_IN_OUT io : INT; END_VAR
VAR CONSTANT c : INT := 1; END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Pump
VAR k : INT; END_VAR
END_FUNCTION_BLOCK

PROGRAM P
VAR d : Drive; pump : Pump; n : INT; lamp AT %QW8 : INT; END_VAR
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL panel AT %QW4 : INT := 3; END_VAR
VAR_CONFIG
    Res.P1.n : INT := 1;
    Res.P1.n : INT := 2;
    Res.P1.d.io : INT := 3;
    Res.P1.d.c : INT := 4;
    Res.P1.d.level AT %IW0 : INT := 5;
    Res.P1.d.out AT %QW4 : INT := 6;
    Res.P1.lamp : INT := 7;
    Res.P1.d.b AT %QB9 : BYTE := 8;
    Res.P1.pump : Pump := (k := 9);
    Res.P1.pump.k : INT := 10;
END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1426] Error: configuration entry refused
        ,-[ file:///test0.st:19:12 ]
        |
     19 |     Res.P1.n : INT := 1;
        |            |
        |            `-- 'n' is given a value here and by another entry
        |
        | Note: an instance's variable has one starting value; keep one of the entries
    ----'
    [E1426] Error: configuration entry refused
        ,-[ file:///test0.st:20:12 ]
        |
     20 |     Res.P1.n : INT := 2;
        |            |
        |            `-- 'n' is given a value here and by another entry
        |
        | Note: an instance's variable has one starting value; keep one of the entries
    ----'
    [E1419] Error: write to an input location
        ,-[ file:///test0.st:23:14 ]
        |
     23 |     Res.P1.d.level AT %IW0 : INT := 5;
        |              ^^|^^
        |                `---- '%IW0' is an input, so an initial value is overwritten before anything reads it
        |
        | Note: the host writes the input image before every scan, the first one included
    ----'
    [E1426] Error: configuration entry refused
        ,-[ file:///test0.st:24:14 ]
        |
     24 |     Res.P1.d.out AT %QW4 : INT := 6;
        |              ^|^
        |               `--- 'out' is at '%QW4', whose declaration gives its starting value
        |
        | Note: give the value in that declaration instead
    ----'
    [E1426] Error: configuration entry refused
        ,-[ file:///test0.st:25:12 ]
        |
     25 |     Res.P1.lamp : INT := 7;
        |            ^^|^
        |              `--- 'lamp' is at '%QW8', whose declaration gives its starting value
        |
        | Note: give the value in that declaration instead
    ----'
    [E1423] Error: part of a wider address
        ,-[ file:///test0.st:26:14 ]
        |
     26 |     Res.P1.d.b AT %QB9 : BYTE := 8;
        |              |
        |              `-- '%QB9' is part of '%QW4' and cannot have an initial value of its own
        |
        | Note: give the variable located at '%QW4' an initial value with this part set in it
    ----'
    [E1426] Error: configuration entry refused
        ,-[ file:///test0.st:27:12 ]
        |
     27 |     Res.P1.pump : Pump := (k := 9);
        |            ^^|^
        |              `--- 'pump' is given a value here and by another entry
        |
        | Note: an instance's variable has one starting value; keep one of the entries
    ----'
    [E1426] Error: configuration entry refused
        ,-[ file:///test0.st:28:17 ]
        |
     28 |     Res.P1.pump.k : INT := 10;
        |                 |
        |                 `-- 'k' is given a value here and by another entry
        |
        | Note: an instance's variable has one starting value; keep one of the entries
    ----'
    [E0405] Error: member cannot be initialized
        ,-[ file:///test0.st:21:23 ]
        |
      4 | VAR_IN_OUT io : INT; END_VAR
        |            ^|
        |             `-- 'io' is declared here
        |
     21 |     Res.P1.d.io : INT := 3;
        |                       ^^|^
        |                         `--- 'io' is a VAR_IN_OUT, which each call binds to its argument, so an initializer cannot give it a value
        |
        | Note: pass the variable in the call instead, as 'io := x'
    ----'
    [E0405] Error: member cannot be initialized
        ,-[ file:///test0.st:22:22 ]
        |
      5 | VAR CONSTANT c : INT := 1; END_VAR
        |              |
        |              `-- 'c' is declared here
        |
     22 |     Res.P1.d.c : INT := 4;
        |                      ^^|^
        |                        `--- 'c' is CONSTANT, whose value is its declaration's, so an initializer cannot give it a value
        |
        | Note: declare it without CONSTANT to let each instance start at its own value
    ----'
    ");
}

#[rstest]
fn config_inst_init_resolves(mut with_db: RootDatabase) {
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
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A location-only entry is the standard's own form (`STATION_2.P4.FB1.C2 AT
/// %QB25: BYTE;`), in our corpus, and it locates a variable declared `AT %Q*`
/// (see `config_inst_init_location_locates_a_partly_located_variable`). One
/// whose variable has no partial address is refused as such (E1424), not
/// with a syntax code.
#[rstest]
fn config_inst_init_location_of_an_unlocated_variable_is_refused(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR
        x : BYTE;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE

    VAR_CONFIG
        inst1.x AT %QB25 : BYTE;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1424] Error: location refused
        ,-[ file:///test0.st:15:15 ]
        |
     15 |         inst1.x AT %QB25 : BYTE;
        |               |
        |               `-- 'x' is not declared AT %I*, %Q* or %M*, so its address is not VAR_CONFIG's to give
        |
        | Note: declare it AT %I*, %Q* or %M* in its POU to leave its address to the configuration
    ----'
    ");
}

/// The same entry, on a variable whose declaration leaves its address to the
/// configuration: nothing to report.
#[rstest]
fn config_inst_init_location_locates_a_partly_located_variable(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
    VAR
        x AT %Q* : BYTE;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE

    VAR_CONFIG
        inst1.x AT %QB25 : BYTE;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// The standard writes the path RESOURCE.PROGRAM.VARIABLE (Table 62,
/// `STATION_1.P1.COUNT`). Only the two-segment form resolved; the standard's
/// own form was answered with "no program instance 'STATION_1'".
#[rstest]
fn config_inst_init_resource_qualified_path_resolves(mut with_db: RootDatabase) {
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
        Res.inst1.x : INT := 42;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A leading segment that names neither a resource nor an instance is still
/// the instance error it always was.
#[rstest]
fn config_inst_init_unknown_leading_segment_is_reported(mut with_db: RootDatabase) {
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
        Nope.inst1.x : INT := 42;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1413] Error: configuration error
        ,-[ file:///test0.st:15:9 ]
        |
     15 |         Nope.inst1.x : INT := 42;
        |         ^^|^
        |           `--- no program instance 'Nope' found in this configuration
    ----'
    ");
}

/// A valid VAR_CONFIG overriding a variable inside a nested function block.
#[rstest]
fn config_inst_init_nested_fb_resolves(mut with_db: RootDatabase) {
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
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// VAR_CONFIG with an unknown program instance should report E1413.
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
    [E1413] Error: configuration error
        ,-[ file:///test0.st:15:9 ]
        |
     15 |         noSuchInst.x : INT := 42;
        |         ^^^^^|^^^^
        |              `------ no program instance 'noSuchInst' found in this configuration
    ----'
    ");
}

/// VAR_CONFIG referencing a nonexistent field on a program should report E1414.
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
    [E1414] Error: configuration error
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
    [E0308] Error: invalid literal
        ,-[ file:///test0.st:15:26 ]
        |
     15 |         inst1.x : INT := 'hello';
        |                          ^^^|^^^
        |                             `----- cannot infer '<string>' to 'INT': cannot use string literal as INT
    ----'
    ");
}

/// VAR_CONFIG trying to walk through a non-composite type should report E1414.
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
    [E1414] Error: configuration error
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
fn config_inst_init_in_resource_resolves(mut with_db: RootDatabase) {
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
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
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
    [E0203] Error: no namespace item found
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
    [E0308] Error: invalid literal
       ,-[ file:///test0.st:4:30 ]
       |
     4 |                 bad : INT := 'oops';
       |                              ^^^|^^
       |                                 `---- cannot infer '<string>' to 'INT': cannot use string literal as INT
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
    [E0101] Error: duplicate definitions
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
    [E1410] Error: task cannot be scheduled
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
    [E1410] Error: task cannot be scheduled
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
    [E1410] Error: task cannot be scheduled
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
    [E1410] Error: task cannot be scheduled
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
    [E1412] Error: program instance never runs
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
    [E1410] Error: task cannot be scheduled
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
    [E1416] Error: unsupported configuration element
        ,-[ file:///test0.st:12:40 ]
        |
     12 |                 PROGRAM PA WITH T : A (inp := src, outp => snk, ghost := nosuch);
        |                                        ^|^
        |                                         `--- program connection lists are parsed but not wired up yet, so this has no effect
        |
        | Note: assign it in the program body instead
    ----'
    [E1416] Error: unsupported configuration element
        ,-[ file:///test0.st:12:52 ]
        |
     12 |                 PROGRAM PA WITH T : A (inp := src, outp => snk, ghost := nosuch);
        |                                                    ^^|^
        |                                                      `--- program connection lists are parsed but not wired up yet, so this has no effect
        |
        | Note: assign it in the program body instead
    ----'
    [E1416] Error: unsupported configuration element
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
    [E1406] Error: configuration error
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
/// CONFIGURATION they used to fail as stray tokens — one E0001 quoting the
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
    [E1404] Error: syntax
       ,-[ file:///test0.st:5:13 ]
       |
     5 |             TASK T(INTERVAL := T#10ms, PRIORITY := 1);
       |             ^^^^^^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^^^^^
       |                                  `---------------------- TASK and PROGRAM must be declared inside a RESOURCE
       |
       | Note: wrap them in a RESOURCE <name> ON <cpu> ... END_RESOURCE block
    ---'
    [E1404] Error: syntax
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
    [E0021] Error: syntax
       ,-[ file:///test0.st:6:17 ]
       |
     6 |                 VAR_GLOBAL g : INT := 10; END_VAR
       |                 ^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^
       |                                 `------------------ VAR_GLOBAL is not allowed in this context
       |
       | Note: VAR_GLOBAL can only be used inside CONFIGURATION
    ---'
    [E0021] Error: syntax
        ,-[ file:///test0.st:11:17 ]
        |
     11 |                 VAR_GLOBAL g : INT := 99; END_VAR
        |                 ^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^
        |                                 `------------------ VAR_GLOBAL is not allowed in this context
        |
        | Note: VAR_GLOBAL can only be used inside CONFIGURATION
    ----'
    [E0206] Error: external variable not found
       ,-[ file:///test0.st:2:32 ]
       |
     2 |         PROGRAM P VAR_EXTERNAL g : INT; END_VAR VAR n : INT; END_VAR n := g; END_PROGRAM
       |                                ^^^|^^^
       |                                   `----- external variable 'g' not found in any accessible VAR_GLOBAL
    ---'
    [E1403] Error: configuration error
       ,-[ file:///test0.st:5:22 ]
       |
     5 |             RESOURCE R1 ON CPU
       |                      ^|
       |                       `-- a deployment drives one RESOURCE; this configuration declares 2 (R1, R2); deploy one RESOURCE per runtime
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
    [E1402] Error: configuration error
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
    [E1402] Error: configuration error
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

/// PRIORITY is optional in the grammar (its absence is E1405). The schedule
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

/// A global spec is a NAME LIST or a single located name — never one bare
/// identifier
#[rstest]
fn a_global_name_list_declares_every_name(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL ga : INT; gb : INT; gsolo : INT; END_VAR
    ga := 1; gb := 2; gsolo := 3;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL ga, gb : INT := 5; gsolo : INT; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(crate::tests::utils::test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A located global is reachable by its own name — the `AT` clause is not
/// part of it — and the address binds it to the input band, so the whole
/// declaration checks clean. The location itself is exercised in
/// `semantics::direct_variables`.
#[rstest]
fn a_located_global_is_named_by_its_identifier(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P
VAR_EXTERNAL sensor : BOOL; END_VAR
VAR t : BOOL; END_VAR
    t := sensor;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL sensor AT %IX0.0 : BOOL; END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(crate::tests::utils::test_diagnostics(&mut with_db, &[source]), @"");
}

// E1403: a deployment drives one RESOURCE, so a second one is refused where
// it can be fixed instead of at deploy, where the same rule used to surface
// after a compile that exited 0.

#[rstest]
fn a_second_resource_is_rejected_at_check(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P VAR n : INT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Core0 ON CPU
        TASK T1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM A1 WITH T1 : P;
    END_RESOURCE
    RESOURCE Core1 ON CPU
        TASK T2(INTERVAL := T#20ms, PRIORITY := 2);
        PROGRAM A2 WITH T2 : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1403] Error: configuration error
       ,-[ file:///test0.st:5:14 ]
       |
     5 |     RESOURCE Core0 ON CPU
       |              ^^|^^
       |                `---- a deployment drives one RESOURCE; this configuration declares 2 (Core0, Core1); deploy one RESOURCE per runtime
    ---'
    ");
}

#[rstest]
fn resources_split_across_fragments_are_counted_together(mut with_db: RootDatabase) {
    let source1 = r#"
PROGRAM P VAR n : INT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Core0 ON CPU
        TASK T1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM A1 WITH T1 : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    let source2 = r#"
CONFIGURATION Cfg
    RESOURCE Core1 ON CPU
        TASK T2(INTERVAL := T#20ms, PRIORITY := 2);
        PROGRAM A2 WITH T2 : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source1, source2]), @r"
    [E1403] Error: configuration error
       ,-[ file:///test0.st:5:14 ]
       |
     5 |     RESOURCE Core0 ON CPU
       |              ^^|^^
       |                `---- a deployment drives one RESOURCE; this configuration declares 2 (Core0, Core1); deploy one RESOURCE per runtime
    ---'
    [E1403] Error: configuration error
       ,-[ file:///test1.st:3:14 ]
       |
     3 |     RESOURCE Core1 ON CPU
       |              ^^|^^
       |                `---- a deployment drives one RESOURCE; this configuration declares 2 (Core0, Core1); deploy one RESOURCE per runtime
    ---'
    ");
}

// One RESOURCE with several TASKS is the supported shape and stays clean.
#[rstest]
fn one_resource_many_tasks_is_clean(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P VAR n : INT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Res ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        TASK Slow(INTERVAL := T#50ms, PRIORITY := 2);
        PROGRAM A1 WITH Fast : P;
        PROGRAM A2 WITH Slow : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}
