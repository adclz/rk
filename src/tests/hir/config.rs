// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use db::WorkspaceDataBase;
use hir::hir_def::semantic_index::semantic_index;
use hir::hir_ty::config::{SingleRead, SingleSource, SourceStep, infer_config_result};
use hir::hir_ty::index_graphs::config_fragments;
use hir::hir_ty::infer::Infer;
use hir::hir_ty::ty::Type;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_source, add_sources, test_diagnostics, with_db};

/// A `PROGRAM RETAIN` instance is accepted.
#[rstest]
fn valid_config_retain_instance(mut with_db: RootDatabase) {
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
    [E0115] Error: duplicate RESOURCE
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
       |               `--- resource 'Res' is already declared here
    ---'
    [E0115] Error: duplicate RESOURCE
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
       |               `--- resource 'Res' is already declared here
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
    [E0101] Error: duplicate variable
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
       |            `---- variable 'shared' is already declared here
    ---'
    [E0101] Error: duplicate variable
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
       |            `---- variable 'shared' is already declared here
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

    let files: Vec<_> = with_db.get_files().to_vec();
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
    [E0203] Error: unknown name in a namespace
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
    [E1411] Error: unknown TASK reference
       ,-[ file:///test0.st:7:28 ]
       |
     7 |         PROGRAM inst1 WITH unknownTask : MyProg;
       |                            ^^^^^|^^^^^
       |                                 `------- no TASK is named 'unknownTask' in this configuration
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
    [E0203] Error: unknown name in a namespace
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
    [E1411] Error: unknown TASK reference
       ,-[ file:///test0.st:8:28 ]
       |
     8 |         PROGRAM inst1 WITH noSuchTask : MyProg;
       |                            ^^^^^|^^^^
       |                                 `------ no TASK is named 'noSuchTask' in this configuration
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
    [E0203] Error: unknown name in a namespace
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
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0201] Error: unknown name
       ,-[ file:///test0.st:7:16 ]
       |
     7 |         ABLE : nonexistent : INT READ_ONLY;
       |                ^^^^^|^^^^^
       |                     `------- no item 'nonexistent' found in scope
    ---'
    ");
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
    [E1415] Error: VAR_ACCESS type mismatch
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
VAR_IN_OUT io : STRING; END_VAR
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
    Res.P1.d.io : STRING := 'x';
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
     20 |     Res.P1.n : INT := 2;
        |            |
        |            `-- 'n' is given a value here
        |
        | Help: keep one of the entries
        |
        | Note: an instance's variable has one starting value
    ----'
    [E0405] Error: initializer for a member that takes none
        ,-[ file:///test0.st:21:26 ]
        |
      4 | VAR_IN_OUT io : STRING; END_VAR
        |            ^|
        |             `-- 'io' is declared here
        |
     21 |     Res.P1.d.io : STRING := 'x';
        |                          ^^^|^^
        |                             `---- an initializer cannot set 'io', a VAR_IN_OUT
        |
        | Help: pass the variable in the call, as 'io := x'
        |
        | Note: a VAR_IN_OUT of a STRING, a reference or an aggregate is bound to its argument at each call, where a scalar one is a member and takes a value
    ----'
    [E0405] Error: initializer for a member that takes none
        ,-[ file:///test0.st:22:22 ]
        |
      5 | VAR CONSTANT c : INT := 1; END_VAR
        |              |
        |              `-- 'c' is declared here
        |
     22 |     Res.P1.d.c : INT := 4;
        |                      ^^|^
        |                        `--- an initializer cannot set 'c', a CONSTANT
        |
        | Help: declare it without CONSTANT to let each instance start at its own value
        |
        | Note: a CONSTANT has the value of its declaration
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
     17 | VAR_GLOBAL panel AT %QW4 : INT := 3; END_VAR
        |            ^^|^^
        |              `---- 'panel' is declared here
        |
     24 |     Res.P1.d.out AT %QW4 : INT := 6;
        |              ^|^
        |               `--- 'out' is at '%QW4', the cell 'panel' is declared at
        |
        | Help: give the value in that declaration
        |
        | Note: a declared cell starts at the value its declaration gives
    ----'
    [E1426] Error: configuration entry refused
        ,-[ file:///test0.st:25:12 ]
        |
     13 | VAR d : Drive; pump : Pump; n : INT; lamp AT %QW8 : INT; END_VAR
        |                                      ^^|^
        |                                        `--- 'lamp' is declared here
        |
     25 |     Res.P1.lamp : INT := 7;
        |            ^^|^
        |              `--- 'lamp' is at '%QW8', one cell for every instance of 'P'
        |
        | Help: give the value in its declaration
        |
        | Note: a PROGRAM's variable located in full is its channel, whatever the instance
    ----'
    [E1423] Error: part of a wider address
        ,-[ file:///test0.st:26:14 ]
        |
     17 | VAR_GLOBAL panel AT %QW4 : INT := 3; END_VAR
        |            ^^|^^
        |              `---- 'panel' is declared here
        |
     26 |     Res.P1.d.b AT %QB9 : BYTE := 8;
        |              |
        |              `-- '%QB9' is part of '%QW4' and cannot have an initial value of its own
        |
        | Help: give 'panel' an initial value with this part set in it
    ----'
    [E1426] Error: configuration entry refused
        ,-[ file:///test0.st:27:12 ]
        |
     27 |     Res.P1.pump : Pump := (k := 9);
        |            ^^|^
        |              `--- 'pump' is given a value here and by another entry
     28 |     Res.P1.pump.k : INT := 10;
        |                 |
        |                 `-- 'pump.k' is given a value here
        |
        | Help: keep one of the entries
        |
        | Note: an instance's variable has one starting value
    ----'
    ");
}

/// A task association names a TASK of the resource (E1411, as a program's
/// WITH), and one that can run: a function block bound to one that cannot
/// would never run, as a program would not (E1410).
#[rstest]
#[case::unknown_task("fb1 WITH Nope", "[E1411]")]
#[case::task_that_cannot_run("fb1 WITH Ev", "[E1410]")]
fn invalid_task_for_a_function_block(
    mut with_db: RootDatabase,
    #[case] element: &str,
    #[case] code: &str,
) {
    let source = format!(
        r#"
FUNCTION_BLOCK Counter
VAR n : INT; END_VAR
    n := n + 1;
END_FUNCTION_BLOCK

PROGRAM F
VAR fb1 : Counter; END_VAR
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        TASK Ev(PRIORITY := 2);
        PROGRAM P1 WITH T : F({element});
    END_RESOURCE
END_CONFIGURATION
"#
    );
    let diagnostics = test_diagnostics(&mut with_db, &[&source]);
    assert!(diagnostics.contains(code), "{diagnostics}");
}

/// A program instance's connections and task associations are refused
/// where they cannot hold (E1428): `:=` feeds an input and `=>` reads an
/// output, the two sides are one type, an input or a task association is
/// made once, a task association names a function block the program holds,
/// and the program does not call that function block itself. An output
/// cannot be copied into an input image (E1419).
#[rstest]
fn invalid_program_connections_and_task_associations(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Counter
VAR_OUTPUT n : INT; END_VAR
    n := n + 1;
END_FUNCTION_BLOCK

CLASS Box
VAR k : INT; END_VAR
END_CLASS

PROGRAM F
VAR_INPUT x1 : BOOL; x2 : UINT; END_VAR
VAR_OUTPUT y1 : UINT; END_VAR
VAR fb1 : Counter; fb2 : Counter; b : Box; END_VAR
    fb2();
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL w : INT; END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : F(y1 := 1, x1 => w, x2 := w, x1 := TRUE, x1 := FALSE, y1 => %IW0);
        PROGRAM P2 WITH T : F(b WITH T, fb2 WITH T, fb1 WITH T, fb1 WITH T, nosuch := 1);
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1428] Error: program configuration element refused
        ,-[ file:///test0.st:22:31 ]
        |
     22 |         PROGRAM P1 WITH T : F(y1 := 1, x1 => w, x2 := w, x1 := TRUE, x1 := FALSE, y1 => %IW0);
        |                               ^|
        |                                `-- 'y1' is not a VAR_INPUT of the program, so ':=' cannot feed it
        |
        | Note: ':=' connects a source to an input, '=>' an output to a sink
    ----'
    [E1428] Error: program configuration element refused
        ,-[ file:///test0.st:22:40 ]
        |
     22 |         PROGRAM P1 WITH T : F(y1 := 1, x1 => w, x2 := w, x1 := TRUE, x1 := FALSE, y1 => %IW0);
        |                                        ^|
        |                                         `-- 'x1' is not a VAR_OUTPUT of the program, so '=>' cannot read it
        |
        | Note: ':=' connects a source to an input, '=>' an output to a sink
    ----'
    [E1428] Error: program configuration element refused
        ,-[ file:///test0.st:22:49 ]
        |
     22 |         PROGRAM P1 WITH T : F(y1 := 1, x1 => w, x2 := w, x1 := TRUE, x1 := FALSE, y1 => %IW0);
        |                                                 ^|
        |                                                  `-- 'x2' is 'UINT', while 'w' is 'INT'
        |
        | Help: connect a variable of the same type
        |
        | Note: a connection copies the value as it is
    ----'
    [E1428] Error: program configuration element refused
        ,-[ file:///test0.st:22:58 ]
        |
     22 |         PROGRAM P1 WITH T : F(y1 := 1, x1 => w, x2 := w, x1 := TRUE, x1 := FALSE, y1 => %IW0);
        |                                                          ^|          ^|
        |                                                           `-------------- 'x1' is connected here and by another element
        |                                                                       |
        |                                                                       `-- 'x1' is connected here
        |
        | Help: keep one of the elements
        |
        | Note: an input has one source
    ----'
    [E1419] Error: write to an input location
        ,-[ file:///test0.st:22:83 ]
        |
     22 |         PROGRAM P1 WITH T : F(y1 := 1, x1 => w, x2 := w, x1 := TRUE, x1 := FALSE, y1 => %IW0);
        |                                                                                   ^|
        |                                                                                    `-- '%IW0' is an input: it is written by the host, not by the program
        |
        | Note: the host copies the input image in before each scan: the rest of this scan would read a value the input never had, and the next scan overwrites it
    ----'
    [E1428] Error: program configuration element refused
        ,-[ file:///test0.st:23:31 ]
        |
     23 |         PROGRAM P2 WITH T : F(b WITH T, fb2 WITH T, fb1 WITH T, fb1 WITH T, nosuch := 1);
        |                               |
        |                               `-- 'b' is not a FUNCTION_BLOCK instance, so no task can run it
        |
        | Note: a CLASS has no body for a task to run
    ----'
    [E1428] Error: program configuration element refused
        ,-[ file:///test0.st:23:41 ]
        |
     15 |     fb2();
        |     ^|^
        |      `--- 'fb2' is called here
        |
     23 |         PROGRAM P2 WITH T : F(b WITH T, fb2 WITH T, fb1 WITH T, fb1 WITH T, nosuch := 1);
        |                                         ^|^
        |                                          `--- 'fb2' is run by its task and called by 'F' too
        |
        | Help: remove the call from the program
        |
        | Note: the task runs the instance on its own
    ----'
    [E1428] Error: program configuration element refused
        ,-[ file:///test0.st:23:53 ]
        |
     23 |         PROGRAM P2 WITH T : F(b WITH T, fb2 WITH T, fb1 WITH T, fb1 WITH T, nosuch := 1);
        |                                                     ^|^         ^|^
        |                                                      `--------------- 'fb1' is associated with a task here and by another element
        |                                                                  |
        |                                                                  `--- 'fb1' is associated with a task here
        |
        | Help: keep one of the elements
        |
        | Note: a function block runs under one task
    ----'
    [E1428] Error: program configuration element refused
        ,-[ file:///test0.st:23:77 ]
        |
     23 |         PROGRAM P2 WITH T : F(b WITH T, fb2 WITH T, fb1 WITH T, fb1 WITH T, nosuch := 1);
        |                                                                             ^^^|^^
        |                                                                                `---- 'nosuch' is not a variable of 'F'
        |
        | Note: an element names a VAR_INPUT, a VAR_OUTPUT or a function block instance of the program
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
        | Help: declare it AT %I*, %Q* or %M* in its POU to leave its address to the configuration
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
    [E1413] Error: unknown instance in VAR_CONFIG
        ,-[ file:///test0.st:15:9 ]
        |
     15 |         Nope.inst1.x : INT := 42;
        |         ^^|^
        |           `--- no PROGRAM instance is named 'Nope' in this configuration
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
    [E1413] Error: unknown instance in VAR_CONFIG
        ,-[ file:///test0.st:15:9 ]
        |
     15 |         noSuchInst.x : INT := 42;
        |         ^^^^^|^^^^
        |              `------ no PROGRAM instance is named 'noSuchInst' in this configuration
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
    [E1414] Error: unknown field in VAR_CONFIG
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
    [E0308] Error: literal of the wrong kind
        ,-[ file:///test0.st:15:26 ]
        |
     15 |         inst1.x : INT := 'hello';
        |                          ^^^|^^^
        |                             `----- cannot use string literal as INT
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
    [E1414] Error: unknown field in VAR_CONFIG
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
    let file = add_source(&mut with_db, source);

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
    let file = add_source(&mut with_db, source);

    let sema = semantic_index(&with_db, file);
    let config = sema.configs[0];

    let result = infer_config_result(&with_db, config);
    assert_eq!(
        result.task_of_prog.len(),
        1,
        "should have one task_of_prog entry"
    );

    let (prog, task) = result.task_of_prog.iter().next().unwrap();
    assert_eq!(task.name(&with_db).with_case.text(&with_db), "t1");
    assert_eq!(prog.name(&with_db).with_case.text(&with_db), "inst1");
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
    let file = add_source(&mut with_db, source);

    let sema = semantic_index(&with_db, file);
    let config = sema.configs[0];

    let result = infer_config_result(&with_db, config);
    assert_eq!(
        result.prog_instance.len(),
        1,
        "should have one prog_instance entry"
    );

    let (_, prog_decl) = result.prog_instance.iter().next().unwrap();
    assert_eq!(prog_decl.name_with_case(&with_db).text(&with_db), "MyProg");
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
    let file = add_source(&mut with_db, source);

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
    [E0203] Error: unknown name in a namespace
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
    [E0308] Error: literal of the wrong kind
       ,-[ file:///test0.st:4:30 ]
       |
     4 |                 bad : INT := 'oops';
       |                              ^^^|^^
       |                                 `---- cannot use string literal as INT
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
    [E0101] Error: duplicate variable
       ,-[ file:///test0.st:5:17 ]
       |
     4 |                 g : INT;
       |                 |
       |                 `-- variable 'g' is already declared here
     5 |                 g : INT;
       |                 |
       |                 `-- duplicate variable 'g'
    ---'
    ");
}

/// A task with a SINGLE and no INTERVAL runs on the rising edges of its BOOL
/// alone: the standard's non-periodic task.
#[rstest]
fn valid_a_single_alone_schedules_a_task(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            VAR_GLOBAL go : BOOL; END_VAR
            RESOURCE R ON CPU
                TASK T(SINGLE := go, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A workspace whose task `T`, which a program runs under, is given `params`,
/// beside everything a task parameter can name.
fn task_given(params: &str) -> String {
    format!(
        r#"
TYPE Inner : STRUCT deep : BOOL; END_STRUCT; END_TYPE
TYPE Flags : STRUCT go : BOOL; inner : Inner; count : INT; END_STRUCT; END_TYPE
TYPE Switch : BOOL; END_TYPE
TYPE Row : ARRAY[0..1] OF BOOL; END_TYPE

FUNCTION_BLOCK Fb
VAR_OUTPUT q : BOOL; END_VAR
VAR inner : BOOL; END_VAR
    q := NOT inner;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Sub EXTENDS Fb
END_FUNCTION_BLOCK

CLASS Cl
VAR x : BOOL; END_VAR
END_CLASS

PROGRAM Source
VAR_INPUT start : BOOL; END_VAR
VAR_OUTPUT
    done : BOOL;
    status : Flags;
    bits : ARRAY[1..3] OF BOOL;
    grid : ARRAY[0..1, 0..1] OF BOOL;
    rows : ARRAY[0..1] OF Row;
    word : WORD;
    r : REF_TO BOOL;
    made : Fb;
    alias : Switch;
END_VAR
VAR
    hidden : BOOL;
    fb : Fb;
END_VAR
VAR_TEMP scratch : BOOL; END_VAR
    done := start;
END_PROGRAM

PROGRAM Run
VAR n : DINT; END_VAR
    n := n + 1;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL
    go : BOOL;
    flags : Flags;
    n : INT;
    gfb : Fb;
    gsub : Sub;
    gcl : Cl;
    plain : TIME;
    located AT %IX4.0 : BOOL;
    list : ARRAY[0..1] OF BOOL;
    status : WORD;
    level : INT;
    rate : REAL;
END_VAR
VAR_GLOBAL CONSTANT
    mask : WORD := 8;
    ratio : REAL := 1.5;
    off : BOOL := FALSE;
    on : BOOL := TRUE;
    unset : BOOL;
    computed : BOOL := NOT FALSE;
    settings : Flags := (go := FALSE);
    K : INT := 2;
    period : TIME := T#25ms;
END_VAR
    RESOURCE Res ON CPU
        TASK Base(INTERVAL := T#10ms, PRIORITY := 0);
        TASK T({params}, PRIORITY := 1);
        PROGRAM P1 WITH Base : Source;
        PROGRAM P2 WITH T : Run;
    END_RESOURCE
END_CONFIGURATION
"#
    )
}

/// A periodic task and a non-periodic one in one resource, a program under
/// each, the global the second waits for also fed to the first program's input:
/// the configuration the standard shows for its tasks, with the global on the
/// CONFIGURATION, where rk has them.
#[rstest]
fn valid_a_periodic_and_a_non_periodic_task_side_by_side(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Reader
VAR_INPUT trigger : BOOL; limit : UINT; END_VAR
VAR_OUTPUT seen : UINT; END_VAR
    IF trigger THEN seen := limit; END_IF;
END_PROGRAM

PROGRAM Writer
VAR_OUTPUT level : INT; END_VAR
    level := level + 1;
END_PROGRAM

CONFIGURATION Cell
VAR_GLOBAL limit : UINT; trigger : BOOL; END_VAR
    RESOURCE Station ON Cpu
        TASK Periodic(INTERVAL := t#50ms, PRIORITY := 2);
        TASK OnEvent(SINGLE := trigger, PRIORITY := 1);
        PROGRAM A WITH Periodic : Reader(trigger := trigger, limit := limit);
        PROGRAM B WITH OnEvent : Writer(level => %QW5);
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// What a SINGLE can be given, which is what the standard's `Data_Source` is:
/// a constant, a VAR_GLOBAL with or without its resource and a STRUCT element,
/// a program's output and a direct variable. A path may go on through the
/// elements and the constant subscripts of an output, which the standard's
/// `Symbolic_Variable` allows. It may end in a bit of what it reaches, which
/// is rk's: the standard's `Data_Source` has no partial access. Names are
/// caseless, and `test_diagnostics` also builds each one.
#[rstest]
#[case::global("SINGLE := go")]
#[case::global_caseless("SINGLE := GO")]
#[case::global_with_interval("SINGLE := go, INTERVAL := T#20ms")]
#[case::global_with_zero_interval("SINGLE := go, INTERVAL := T#0s")]
#[case::long_zero_interval("SINGLE := go, INTERVAL := LT#0s")]
#[case::global_with_a_constant_interval("SINGLE := go, INTERVAL := period")]
#[case::resource_prefix("SINGLE := Res.go")]
#[case::resource_prefix_caseless("SINGLE := RES.go")]
#[case::element("SINGLE := flags.go")]
#[case::element_with_resource("SINGLE := Res.flags.go")]
#[case::nested_element("SINGLE := flags.inner.deep")]
#[case::located_global("SINGLE := located")]
#[case::array_of_a_global("SINGLE := list[1]")]
#[case::output("SINGLE := P1.done")]
#[case::output_caseless("SINGLE := p1.DONE")]
#[case::output_with_resource("SINGLE := Res.P1.done")]
#[case::output_alias("SINGLE := P1.alias")]
#[case::output_element("SINGLE := P1.status.go")]
#[case::output_nested_element("SINGLE := P1.status.inner.deep")]
#[case::output_array("SINGLE := P1.bits[2]")]
#[case::output_array_constant_subscript("SINGLE := P1.bits[K]")]
#[case::output_array_computed_subscript("SINGLE := P1.bits[K - 1]")]
#[case::output_grid("SINGLE := P1.grid[1, 0]")]
#[case::output_grid_two_brackets("SINGLE := P1.grid[1][0]")]
#[case::output_array_of_arrays("SINGLE := P1.rows[1][0]")]
#[case::output_of_a_global_instance("SINGLE := gfb.q")]
#[case::inherited_output_of_a_global_instance("SINGLE := gsub.q")]
#[case::output_of_an_output_instance("SINGLE := P1.made.q")]
#[case::bit_of_a_global("SINGLE := status.%X3")]
#[case::bit_written_bare("SINGLE := status.3")]
#[case::bit_caseless("SINGLE := STATUS.%x3")]
#[case::bit_through_the_resource("SINGLE := Res.status.%X3")]
#[case::bit_of_an_integer("SINGLE := level.%X15")]
#[case::bit_of_an_element("SINGLE := flags.count.%X1")]
#[case::bit_of_a_bool("SINGLE := go.0")]
#[case::bit_of_an_output("SINGLE := P1.word.%X3")]
#[case::bit_of_an_output_written_bare("SINGLE := P1.word.3")]
#[case::bit_beside_a_period("SINGLE := status.%X3, INTERVAL := T#20ms")]
#[case::address("SINGLE := %IX2")]
#[case::address_bit("SINGLE := %IX2.1")]
#[case::address_caseless("SINGLE := %ix3.1")]
#[case::address_no_size_letter("SINGLE := %I1")]
#[case::address_of_an_output("SINGLE := %QX0.1")]
#[case::address_of_a_marker("SINGLE := %MX0.1")]
#[case::address_of_a_declaration("SINGLE := %IX4.0")]
#[case::false_beside_a_period("SINGLE := FALSE, INTERVAL := T#20ms")]
#[case::zero_beside_a_period("SINGLE := 0, INTERVAL := T#20ms")]
#[case::typed_zero_beside_a_period("SINGLE := BOOL#0, INTERVAL := T#20ms")]
#[case::typed_false_beside_a_period("SINGLE := BOOL#FALSE, INTERVAL := T#20ms")]
#[case::constant_false_beside_a_period("SINGLE := off, INTERVAL := T#20ms")]
#[case::constant_with_no_value_beside_a_period("SINGLE := unset, INTERVAL := T#20ms")]
fn valid_what_a_single_can_read(mut with_db: RootDatabase, #[case] params: &str) {
    let source = task_given(params);
    assert_eq!(test_diagnostics(&mut with_db, &[&source]), "", "{params}");
}

/// What a bound task cannot be given, each with its own reason: E1410 is the
/// task's, whatever is wrong with what it reads.
#[rstest]
#[case::true_alone("SINGLE := TRUE", "SINGLE is always TRUE, so the task would run once")]
#[case::true_beside_a_period(
    "SINGLE := TRUE, INTERVAL := T#20ms",
    "SINGLE is always TRUE, so the task would run once and its INTERVAL never"
)]
#[case::one("SINGLE := 1", "SINGLE is always TRUE, so the task would run once")]
#[case::typed_one(
    "SINGLE := BOOL#1",
    "SINGLE is always TRUE, so the task would run once"
)]
#[case::typed_true(
    "SINGLE := BOOL#TRUE",
    "SINGLE is always TRUE, so the task would run once"
)]
#[case::constant_true("SINGLE := on", "SINGLE is always TRUE, so the task would run once")]
#[case::constant_true_beside_a_period(
    "SINGLE := on, INTERVAL := T#20ms",
    "SINGLE is always TRUE, so the task would run once and its INTERVAL never"
)]
#[case::constant_computed("SINGLE := computed", "SINGLE is a constant, which never rises")]
#[case::constant_element("SINGLE := settings.go", "SINGLE is a constant, which never rises")]
#[case::bit_of_a_constant("SINGLE := mask.%X3", "SINGLE is a constant, which never rises")]
#[case::constant_with_no_value("SINGLE := unset", "SINGLE is a constant, which never rises")]
#[case::false_alone("SINGLE := FALSE", "SINGLE is a constant, which never rises")]
#[case::zero_alone("SINGLE := 0", "SINGLE is a constant, which never rises")]
#[case::constant_false_alone("SINGLE := off", "SINGLE is a constant, which never rises")]
#[case::false_beside_zero(
    "SINGLE := FALSE, INTERVAL := T#0s",
    "SINGLE is a constant, which never rises"
)]
#[case::integer("SINGLE := 5", "SINGLE must be a BOOL")]
#[case::typed_integer("SINGLE := INT#1", "SINGLE must be a BOOL")]
#[case::time("SINGLE := T#1s", "SINGLE must be a BOOL")]
#[case::integer_global("SINGLE := n", "SINGLE must be a BOOL")]
#[case::constant_integer("SINGLE := K", "SINGLE must be a BOOL")]
#[case::whole_struct("SINGLE := flags", "SINGLE must be a BOOL")]
#[case::integer_element("SINGLE := flags.count", "SINGLE must be a BOOL")]
#[case::whole_instance("SINGLE := P1", "SINGLE must be a BOOL")]
#[case::word_output("SINGLE := P1.word", "SINGLE must be a BOOL")]
#[case::reference_output("SINGLE := P1.r", "SINGLE must be a BOOL")]
#[case::whole_array("SINGLE := P1.bits", "SINGLE must be a BOOL")]
#[case::row_of_a_grid("SINGLE := P1.grid[1]", "SINGLE must be a BOOL")]
#[case::wide_input("SINGLE := %IW3", "SINGLE must be a BOOL")]
#[case::wide_output("SINGLE := %QB0", "SINGLE must be a BOOL")]
#[case::wide_marker("SINGLE := %MD0", "SINGLE must be a BOOL")]
#[case::wide_with_two_levels("SINGLE := %IW0.3", "SINGLE must be a BOOL")]
#[case::byte_of_a_global("SINGLE := status.%B0", "SINGLE must be a BOOL")]
#[case::word_of_an_output("SINGLE := P1.word.%W0", "SINGLE must be a BOOL")]
#[case::an_input("SINGLE := P1.start", "reads a VAR_OUTPUT")]
#[case::a_local("SINGLE := P1.hidden", "reads a VAR_OUTPUT")]
#[case::a_temp("SINGLE := P1.scratch", "reads a VAR_OUTPUT")]
#[case::through_a_local_instance("SINGLE := P1.fb.q", "reads a VAR_OUTPUT")]
#[case::a_local_of_an_instance("SINGLE := gfb.inner", "reads a VAR_OUTPUT")]
#[case::a_local_of_an_output_instance("SINGLE := P1.made.inner", "reads a VAR_OUTPUT")]
#[case::a_member_of_a_class("SINGLE := gcl.x", "reads a VAR_OUTPUT")]
#[case::bit_of_a_local("SINGLE := P1.hidden.0", "reads a VAR_OUTPUT")]
#[case::dereference("SINGLE := P1.r^", "must have a fixed place")]
#[case::subscript_not_constant("SINGLE := P1.bits[n]", "must have a fixed place")]
#[case::zero_interval_alone("INTERVAL := T#0s", "a zero INTERVAL schedules no periodic run")]
#[case::interval_not_constant("SINGLE := go, INTERVAL := plain", "INTERVAL must be a TIME literal")]
#[case::interval_of_an_address("SINGLE := go, INTERVAL := %MD0", "INTERVAL must be a TIME literal")]
#[case::interval_of_an_element("INTERVAL := flags.go", "INTERVAL must be a TIME literal")]
#[case::interval_of_an_output("INTERVAL := P1.done", "INTERVAL must be a TIME literal")]
#[case::interval_of_a_bit("INTERVAL := status.%X3", "INTERVAL must be a TIME literal")]
fn invalid_what_a_task_cannot_be_given(
    mut with_db: RootDatabase,
    #[case] params: &str,
    #[case] message: &str,
) {
    let source = task_given(params);
    let diagnostics = test_diagnostics(&mut with_db, &[&source]);
    assert!(diagnostics.starts_with("[E1410]"), "{diagnostics}");
    assert_eq!(diagnostics.matches("Error:").count(), 1, "{diagnostics}");
    assert!(diagnostics.contains(message), "{diagnostics}");
}

/// A report about what SINGLE is given points at it, with what to write
/// instead; one about the task as a whole points at its name.
#[rstest]
fn invalid_a_constant_single_says_why_and_where(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
CONFIGURATION Cfg
    RESOURCE R ON CPU
        TASK T(SINGLE := TRUE, INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1410] Error: unschedulable TASK
       ,-[ file:///test0.st:5:26 ]
       |
     5 |         TASK T(SINGLE := TRUE, INTERVAL := T#10ms, PRIORITY := 1);
       |                          ^^|^
       |                            `--- task 'T' cannot be scheduled: SINGLE is always TRUE, so the task would run once and its INTERVAL never
       |
       | Help: name a BOOL variable or a bit address
       |
       | Note: a SINGLE already TRUE at its first reading is a rising edge, and an INTERVAL runs while SINGLE stands at FALSE
    ---'
    ");
}

/// A SINGLE that is not a BOOL is refused where it is written.
#[rstest]
fn invalid_a_single_that_is_no_bool(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
CONFIGURATION Cfg
    VAR_GLOBAL count : INT; END_VAR
    RESOURCE R ON CPU
        TASK T(SINGLE := count, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1410] Error: unschedulable TASK
       ,-[ file:///test0.st:6:26 ]
       |
     6 |         TASK T(SINGLE := count, PRIORITY := 1);
       |                          ^^|^^
       |                            `---- task 'T' cannot be scheduled: SINGLE must be a BOOL
    ---'
    ");
}

/// A path through a program stops at what the program gives out.
#[rstest]
fn invalid_a_single_reading_inside_a_program(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P VAR n : BOOL; END_VAR n := NOT n; END_PROGRAM
CONFIGURATION Cfg
    RESOURCE R ON CPU
        TASK T(SINGLE := P1.n, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1410] Error: unschedulable TASK
       ,-[ file:///test0.st:5:29 ]
       |
     5 |         TASK T(SINGLE := P1.n, PRIORITY := 1);
       |                             |
       |                             `-- task 'T' cannot be scheduled: SINGLE reads a VAR_OUTPUT, not this variable
    ---'
    ");
}

/// A report about a name of the path is at that name, not at the path's last.
#[rstest]
fn invalid_a_single_through_what_a_program_keeps(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Fb VAR_OUTPUT q : BOOL; END_VAR q := NOT q; END_FUNCTION_BLOCK
PROGRAM P VAR fb : Fb; END_VAR fb(); END_PROGRAM
CONFIGURATION Cfg
    VAR_GLOBAL timer : Fb; END_VAR
    RESOURCE R ON CPU
        TASK T(SINGLE := P1.fb.q, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1410] Error: unschedulable TASK
       ,-[ file:///test0.st:7:29 ]
       |
     7 |         TASK T(SINGLE := P1.fb.q, PRIORITY := 1);
       |                             ^|
       |                              `-- task 'T' cannot be scheduled: SINGLE reads a VAR_OUTPUT, not this variable
    ---'
    ");
}

/// What has no fixed place cannot be read without running the program.
#[rstest]
fn invalid_a_single_with_no_fixed_place(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P VAR_OUTPUT r : REF_TO BOOL; END_VAR END_PROGRAM
CONFIGURATION Cfg
    RESOURCE R ON CPU
        TASK T(SINGLE := P1.r^, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1410] Error: unschedulable TASK
       ,-[ file:///test0.st:5:29 ]
       |
     5 |         TASK T(SINGLE := P1.r^, PRIORITY := 1);
       |                             |
       |                             `-- task 'T' cannot be scheduled: SINGLE must have a fixed place in memory
       |
       | Help: name a VAR_GLOBAL or a program's VAR_OUTPUT, or an element at constant subscripts
       |
       | Note: a host reads SINGLE from memory, without running the program
    ---'
    ");
}

/// A bit of an address is an address of its own, `%IX0.3`: a part of one is
/// not read here, and the syntax error says so where it is written.
#[rstest]
fn invalid_a_part_of_an_address_in_a_single(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
CONFIGURATION Cfg
    RESOURCE R ON CPU
        TASK T(SINGLE := %IW0.%X3, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    let diagnostics = test_diagnostics(&mut with_db, &[source]);
    assert!(diagnostics.contains("[E0001]"), "{diagnostics}");
}

/// Each name of a task parameter's path is looked up, the last one no more
/// than the first: `nothing.period` used to read the global `period` and go
/// unreported, whatever `nothing` was. A name that resolves to nothing is
/// reported at that name, and the task is not reported as well.
#[rstest]
#[case::root("SINGLE := nothing", "[E0201]", "no item 'nothing'")]
#[case::root_of_a_path("SINGLE := nothing.go", "[E0201]", "no item 'nothing'")]
#[case::after_the_resource("SINGLE := Res.nothing", "[E0201]", "no item 'nothing'")]
#[case::root_of_a_dotted_interval("INTERVAL := nothing.period", "[E0201]", "no item 'nothing'")]
#[case::unknown_element("SINGLE := flags.nope", "[E0202]", "'Flags' has no field named 'nope'")]
#[case::unknown_nested_element(
    "SINGLE := flags.inner.nope",
    "[E0202]",
    "has no field named 'nope'"
)]
#[case::element_of_a_bool("SINGLE := go.x", "[E0202]", "has no field named 'x'")]
#[case::unknown_output("SINGLE := P1.nope", "[E0202]", "'Source' has no field named 'nope'")]
#[case::unknown_output_after_the_resource(
    "SINGLE := Res.P1.nope",
    "[E0202]",
    "'Source' has no field named 'nope'"
)]
#[case::unknown_interval_element("INTERVAL := flags.nope", "[E0202]", "has no field named 'nope'")]
#[case::unknown_member_of_an_instance("SINGLE := gfb.nope", "[E0202]", "has no field named 'nope'")]
#[case::subscript_out_of_bounds("SINGLE := P1.bits[9]", "[E0506]", "index 9 is out of bounds")]
#[case::second_dimension_out_of_bounds(
    "SINGLE := P1.grid[0, 7]",
    "[E0506]",
    "index 7 is out of bounds"
)]
#[case::second_bracket_out_of_bounds(
    "SINGLE := P1.grid[0][7]",
    "[E0506]",
    "index 7 is out of bounds"
)]
#[case::subscript_of_a_bool("SINGLE := go[1]", "[E0508]", "cannot index into type 'BOOL'")]
#[case::subscript_of_an_instance("SINGLE := P1[1]", "[E0508]", "cannot index into type 'Source'")]
#[case::too_many_subscripts("SINGLE := P1.bits[1, 2]", "[E0508]", "cannot index into")]
#[case::too_many_brackets("SINGLE := P1.bits[1][2]", "[E0508]", "cannot index into")]
#[case::dereference_of_an_instance(
    "SINGLE := P1^",
    "[E0901]",
    "cannot dereference non-reference type 'Source'"
)]
#[case::dereference_of_a_bool("SINGLE := go^", "[E0901]", "non-reference type 'BOOL'")]
#[case::dereference_of_a_struct("SINGLE := flags^", "[E0901]", "non-reference type 'Flags'")]
#[case::dereference_of_an_output("SINGLE := P1.done^", "[E0901]", "non-reference type 'BOOL'")]
#[case::subscript_names_nothing("SINGLE := P1.bits[nothing]", "[E0201]", "no item 'nothing'")]
#[case::subscript_is_a_real("SINGLE := P1.bits[1.5]", "[E0504]", "the index is 'REAL'")]
#[case::subscript_is_a_bool("SINGLE := P1.bits[TRUE]", "[E0504]", "the index is 'BOOL'")]
#[case::subscript_is_a_real_global("SINGLE := P1.bits[rate]", "[E0504]", "the index is 'REAL'")]
#[case::subscript_is_a_real_constant("SINGLE := P1.bits[ratio]", "[E0504]", "the index is 'REAL'")]
#[case::incomplete_address("SINGLE := %I*", "[E1417]", "'%I*' is not a complete address")]
#[case::bit_past_the_end("SINGLE := status.%X16", "[E1429]", "out of range")]
#[case::bit_past_the_end_written_bare("SINGLE := status.16", "[E1429]", "out of range")]
#[case::bit_past_a_bool("SINGLE := go.1", "[E1429]", "out of range")]
#[case::bit_of_what_has_none("SINGLE := flags.%X0", "[E1431]", "")]
#[case::bit_of_a_real("SINGLE := rate.%X0", "[E1431]", "")]
#[case::part_of_no_size("SINGLE := status.%Z1", "[E1418]", "'%Z' names no access size")]
#[case::bit_of_nothing("SINGLE := nothing.%X3", "[E0201]", "no item 'nothing'")]
#[case::bit_of_a_dotted_interval("INTERVAL := nothing.%X3", "[E0201]", "no item 'nothing'")]
fn invalid_a_task_parameter_names_nothing(
    mut with_db: RootDatabase,
    #[case] params: &str,
    #[case] code: &str,
    #[case] message: &str,
) {
    let source = task_given(params);
    let diagnostics = test_diagnostics(&mut with_db, &[&source]);
    assert!(diagnostics.starts_with(code), "{diagnostics}");
    assert_eq!(diagnostics.matches("Error:").count(), 1, "{diagnostics}");
    assert!(diagnostics.contains(message), "{diagnostics}");
}

/// The report of a dotted name is at the name that resolves to nothing.
#[rstest]
fn invalid_a_dotted_parameter_points_at_the_unknown_name(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
CONFIGURATION Cfg
    VAR_GLOBAL CONSTANT period : TIME := T#10ms; END_VAR
    VAR_GLOBAL go : BOOL; END_VAR
    RESOURCE R ON CPU
        TASK A(INTERVAL := nothing.period, PRIORITY := 1);
        TASK B(SINGLE := nowhere.go, PRIORITY := 2);
        PROGRAM P1 WITH A : P;
        PROGRAM P2 WITH B : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0201] Error: unknown name
       ,-[ file:///test0.st:7:28 ]
       |
     7 |         TASK A(INTERVAL := nothing.period, PRIORITY := 1);
       |                            ^^^|^^^
       |                               `----- no item 'nothing' found in scope
    ---'
    [E0201] Error: unknown name
       ,-[ file:///test0.st:8:26 ]
       |
     8 |         TASK B(SINGLE := nowhere.go, PRIORITY := 2);
       |                          ^^^|^^^
       |                             `----- no item 'nowhere' found in scope
    ---'
    ");
}

/// A CONSTANT global reached through the resource's name is as fixed as one
/// reached by its own, and neither makes the task's INTERVAL need a SINGLE.
#[rstest]
fn valid_an_interval_through_the_resource(mut with_db: RootDatabase) {
    let source = task_given("INTERVAL := Res.period");
    add_sources(&mut with_db, &[&source]);
    let file = *with_db.get_files().iter().last().unwrap();
    let config = semantic_index(&with_db, file).configs[0];
    let schedule = &infer_config_result(&with_db, config).schedule;
    let task = schedule.resources[0]
        .tasks
        .iter()
        .find(|t| t.name.text(&with_db) == "T")
        .expect("T runs");
    assert_eq!(task.trigger.interval_ns, Some(25_000_000));
    assert_eq!(task.trigger.single, None);
}

/// A resource may share its name with a global. `flags.go` reads the global's
/// element when it has one, and the name is the resource's when it does not.
/// It used to be the resource's first, which silently read the global `go`,
/// and failed with no global `go` at all.
#[rstest]
fn resolved_schedule_reads_a_resource_named_like_a_global(mut with_db: RootDatabase) {
    let source = r#"
TYPE Flags : STRUCT go : BOOL; END_STRUCT; END_TYPE
PROGRAM Run VAR n : DINT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Cfg
    VAR_GLOBAL flags : Flags; go : BOOL; other : BOOL; END_VAR
    RESOURCE flags ON CPU
        TASK Element(SINGLE := flags.go, PRIORITY := 0);
        TASK Global(SINGLE := go, PRIORITY := 1);
        TASK WithTheResource(SINGLE := flags.flags.go, PRIORITY := 2);
        TASK GlobalOfTheResource(SINGLE := flags.other, PRIORITY := 3);
        PROGRAM A WITH Element : Run;
        PROGRAM B WITH Global : Run;
        PROGRAM C WITH WithTheResource : Run;
        PROGRAM D WITH GlobalOfTheResource : Run;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
    assert_snapshot!(resolved_schedule(&with_db), @r"
    RESOURCE flags ON CPU
      TASK Element on the rising edge of flags.go, priority 0
        PROGRAM A : Run
      TASK Global on the rising edge of go, priority 1
        PROGRAM B : Run
      TASK WithTheResource on the rising edge of flags.go, priority 2
        PROGRAM C : Run
      TASK GlobalOfTheResource on the rising edge of other, priority 3
        PROGRAM D : Run
    ");
}

/// A resource may share its name with a program instance, as `RESOURCE Main`
/// and `PROGRAM Main` do. `Main.done` reads the instance's output, and `Main.go`
/// the global, the name being the resource's there. Both used to depend on
/// which of the two was tried first.
#[rstest]
fn resolved_schedule_reads_a_resource_named_like_an_instance(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Source VAR_OUTPUT done : BOOL; END_VAR done := NOT done; END_PROGRAM
PROGRAM Run VAR n : DINT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Cfg
    VAR_GLOBAL go : BOOL; END_VAR
    VAR_GLOBAL CONSTANT period : TIME := T#25ms; END_VAR
    RESOURCE Main ON CPU
        TASK Base(INTERVAL := Main.period, PRIORITY := 0);
        TASK Output(SINGLE := Main.done, PRIORITY := 1);
        TASK WithTheResource(SINGLE := Main.Main.done, PRIORITY := 2);
        TASK Global(SINGLE := Main.go, PRIORITY := 3);
        PROGRAM Main WITH Base : Source;
        PROGRAM B WITH Output : Run;
        PROGRAM C WITH WithTheResource : Run;
        PROGRAM D WITH Global : Run;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
    assert_snapshot!(resolved_schedule(&with_db), @r"
    RESOURCE Main ON CPU
      TASK Base every 25ms, priority 0
        PROGRAM Main : Source
      TASK Output on the rising edge of Main.done, priority 1
        PROGRAM B : Run
      TASK WithTheResource on the rising edge of Main.done, priority 2
        PROGRAM C : Run
      TASK Global on the rising edge of go, priority 3
        PROGRAM D : Run
    ");
}

/// A name no reading of a path can use is reported at that name, and a variable
/// a shared name reaches keeps its own reports.
#[rstest]
#[case::nothing_after_a_shared_name("SINGLE := Main.nope", "[E0201]", "no item 'nope'")]
#[case::a_local_of_the_instance("SINGLE := Main.hidden", "[E1410]", "reads a VAR_OUTPUT")]
#[case::a_non_bool_output("SINGLE := Main.count", "[E1410]", "SINGLE must be a BOOL")]
fn invalid_a_path_through_a_resource_named_like_an_instance(
    mut with_db: RootDatabase,
    #[case] params: &str,
    #[case] code: &str,
    #[case] message: &str,
) {
    let source = format!(
        r#"
PROGRAM Source
VAR_OUTPUT done : BOOL; count : INT; END_VAR
VAR hidden : BOOL; END_VAR
    done := NOT hidden;
END_PROGRAM
PROGRAM Run VAR n : DINT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Main ON CPU
        TASK Base(INTERVAL := T#10ms, PRIORITY := 0);
        TASK T({params}, PRIORITY := 1);
        PROGRAM Main WITH Base : Source;
        PROGRAM B WITH T : Run;
    END_RESOURCE
END_CONFIGURATION
"#
    );
    let diagnostics = test_diagnostics(&mut with_db, &[&source]);
    assert!(diagnostics.starts_with(code), "{diagnostics}");
    assert_eq!(diagnostics.matches("Error:").count(), 1, "{diagnostics}");
    assert!(diagnostics.contains(message), "{diagnostics}");
}

/// A task's program in another resource is out of reach, as the standard's
/// `Prog_Output_Access` has no resource in it. Two RESOURCEs, which E1403
/// refuses: this is read on its own, for the day a configuration may have
/// more than one.
#[rstest]
fn invalid_a_single_reading_the_program_of_another_resource(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P VAR_OUTPUT done : BOOL; END_VAR done := NOT done; END_PROGRAM
CONFIGURATION Cfg
    RESOURCE A ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
    RESOURCE B ON CPU
        TASK Ev(SINGLE := P1.done, PRIORITY := 1);
        PROGRAM P2 WITH Ev : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    let diagnostics = test_diagnostics(&mut with_db, &[source]);
    assert!(diagnostics.contains("[E0201]"), "{diagnostics}");
    assert!(diagnostics.contains("no item 'P1'"), "{diagnostics}");
}

/// A VAR_GLOBAL another fragment of the configuration declares can be read,
/// as any other, and a CONSTANT of one can be a subscript.
#[rstest]
fn valid_a_single_reading_a_global_of_another_fragment(mut with_db: RootDatabase) {
    let tasks = r#"
PROGRAM P
VAR_OUTPUT bits : ARRAY[1..3] OF BOOL; END_VAR
    bits[1] := NOT bits[1];
END_PROGRAM
CONFIGURATION Cfg
    RESOURCE R ON CPU
        TASK T(SINGLE := go, PRIORITY := 1);
        TASK U(SINGLE := P1.bits[which], PRIORITY := 2);
        PROGRAM P1 WITH T : P;
        PROGRAM P2 WITH U : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    let globals = r#"
CONFIGURATION Cfg
    VAR_GLOBAL go : BOOL; END_VAR
    VAR_GLOBAL CONSTANT which : INT := 2; END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[tasks, globals]), @"");
}

/// A PROGRAM bound to a TASK the scheduler cannot honour is rejected at check
/// time, not compiled into a core with no schedule: a zero INTERVAL schedules
/// nothing, and no SINGLE runs this one instead.
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
    [E1410] Error: unschedulable TASK
       ,-[ file:///test0.st:5:22 ]
       |
     5 |                 TASK T(INTERVAL := T#0ms, PRIORITY := 1);
       |                      |
       |                      `-- task 'T' cannot be scheduled: a zero INTERVAL schedules no periodic run
       |
       | Help: set `INTERVAL := T#10ms` to run on a period, or add `SINGLE := go` to run on a rising edge
       |
       | Note: a task runs on each rising edge of SINGLE, and on its INTERVAL while SINGLE stands at FALSE
    ---'
    ");
}

/// A name that resolves to nothing is a mistake of its own, reported where
/// it is written (E0201) and not as a task that cannot be scheduled: the
/// likely cause is a typo, and the task is left out of the schedule.
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
    [E0201] Error: unknown name
       ,-[ file:///test0.st:5:36 ]
       |
     5 |                 TASK T(INTERVAL := someName, PRIORITY := 1);
       |                                    ^^^^|^^^
       |                                        `----- no item 'someName' found in scope
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
    [E1410] Error: unschedulable TASK
       ,-[ file:///test0.st:6:22 ]
       |
     6 |                 TASK T(INTERVAL := period, PRIORITY := 1);
       |                      |
       |                      `-- task 'T' cannot be scheduled: INTERVAL must be a TIME literal or a CONSTANT global holding one
       |
       | Help: declare the global `VAR_GLOBAL CONSTANT`
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
    [E1412] Error: program instance without a task
       ,-[ file:///test0.st:6:25 ]
       |
     6 |                 PROGRAM P1 : P;
       |                         ^|
       |                          `-- program instance 'P1' has no WITH <task>
       |
       | Note: rk runs a program only under a TASK
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
    [E1410] Error: unschedulable TASK
       ,-[ file:///test0.st:5:22 ]
       |
     5 |                 TASK T(PRIORITY := 1);
       |                      |
       |                      `-- task 'T' cannot be scheduled: a TASK needs an INTERVAL or a SINGLE to run its programs
       |
       | Help: add `INTERVAL := T#10ms` to run on a period, or `SINGLE := go` to run on a rising edge
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
                TASK Later(SINGLE := TRUE, PRIORITY := 9);
                TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH Fast : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A TASK's SINGLE or INTERVAL names a VAR_GLOBAL, used or not: a name that
/// resolves to nothing is E0201 at the name, whether or not a program is
/// bound to the task. The bound task is reported once, as that, and not as
/// unschedulable too.
#[rstest]
fn a_task_parameter_names_a_global(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
        CONFIGURATION Cfg
            RESOURCE R ON CPU
                TASK Later(SINGLE := nope, PRIORITY := 9);
                TASK Slow(INTERVAL := missing, PRIORITY := 8);
                TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH Fast : P;
                PROGRAM P2 WITH Slow : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0201] Error: unknown name
       ,-[ file:///test0.st:5:38 ]
       |
     5 |                 TASK Later(SINGLE := nope, PRIORITY := 9);
       |                                      ^^|^
       |                                        `--- no item 'nope' found in scope
    ---'
    [E0201] Error: unknown name
       ,-[ file:///test0.st:6:39 ]
       |
     6 |                 TASK Slow(INTERVAL := missing, PRIORITY := 8);
       |                                       ^^^|^^^
       |                                          `----- no item 'missing' found in scope
    ---'
    ");
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

/// A connection list names the program's variables. It used to be parsed and
/// then discarded, so `ghost`, which the program does not declare, compiled
/// clean.
#[rstest]
fn a_program_connection_names_a_variable_of_the_program(mut with_db: RootDatabase) {
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
    [E1428] Error: program configuration element refused
        ,-[ file:///test0.st:12:65 ]
        |
     12 |                 PROGRAM PA WITH T : A (inp := src, outp => snk, ghost := nosuch);
        |                                                                 ^^|^^
        |                                                                   `---- 'ghost' is not a variable of 'A'
        |
        | Note: an element names a VAR_INPUT, a VAR_OUTPUT or a function block instance of the program
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
    [E1406] Error: unusable TASK priority
       ,-[ file:///test0.st:6:22 ]
       |
     6 |                 TASK T(INTERVAL := T#10ms, PRIORITY := 99999999999);
       |                      |
       |                      `-- task 'T' has an unusable PRIORITY '99999999999'
       |
       | Note: PRIORITY is a 32-bit unsigned integer, 0 the most urgent
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
    [E1404] Error: TASK or PROGRAM outside a RESOURCE
       ,-[ file:///test0.st:5:13 ]
       |
     5 |             TASK T(INTERVAL := T#10ms, PRIORITY := 1);
       |             ^^|^
       |               `--- a TASK or a PROGRAM instance is declared inside a RESOURCE
       |
       | Help: wrap them in a RESOURCE <name> ON <cpu> ... END_RESOURCE block
    ---'
    [E1404] Error: TASK or PROGRAM outside a RESOURCE
       ,-[ file:///test0.st:6:13 ]
       |
     6 |             PROGRAM A WITH T : P;
       |             ^^^|^^^
       |                `----- a TASK or a PROGRAM instance is declared inside a RESOURCE
       |
       | Help: wrap them in a RESOURCE <name> ON <cpu> ... END_RESOURCE block
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
    [E0206] Error: VAR_EXTERNAL without a VAR_GLOBAL
       ,-[ file:///test0.st:2:32 ]
       |
     2 |         PROGRAM P VAR_EXTERNAL g : INT; END_VAR VAR n : INT; END_VAR n := g; END_PROGRAM
       |                                |
       |                                `-- no VAR_GLOBAL is named 'g'
    ---'
    [E1403] Error: more than one RESOURCE
        ,-[ file:///test0.st:5:22 ]
        |
      5 |             RESOURCE R1 ON CPU
        |                      ^|
        |                       `-- the configuration declares 2 RESOURCEs (R1, R2)
        |
     10 |             RESOURCE R2 ON CPU
        |                      ^|
        |                       `-- 'R2' is declared here
        |
        | Help: deploy one RESOURCE per runtime
        |
        | Note: a deployment drives one RESOURCE
    ----'
    [E0021] Error: VAR_GLOBAL not allowed in this context
       ,-[ file:///test0.st:6:17 ]
       |
     6 |                 VAR_GLOBAL g : INT := 10; END_VAR
       |                 ^^^^^|^^^^
       |                      `------ VAR_GLOBAL is not allowed here
       |
       | Note: VAR_GLOBAL goes in a CONFIGURATION
    ---'
    [E0021] Error: VAR_GLOBAL not allowed in this context
        ,-[ file:///test0.st:11:17 ]
        |
     11 |                 VAR_GLOBAL g : INT := 99; END_VAR
        |                 ^^^^^|^^^^
        |                      `------ VAR_GLOBAL is not allowed here
        |
        | Note: VAR_GLOBAL goes in a CONFIGURATION
    ----'
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
    [E1402] Error: more than one CONFIGURATION
        ,-[ file:///test0.st:4:23 ]
        |
      4 |         CONFIGURATION First
        |                       ^^|^^
        |                         `---- the workspace declares 2 CONFIGURATIONs
        |
     11 |         CONFIGURATION Second
        |                       ^^^|^^
        |                          `---- 'Second' is declared here
        |
        | Help: describe another PLC in its own workspace
        |
        | Note: a workspace has one CONFIGURATION
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
        for (unit, per) in [("s", 1_000_000_000u64), ("ms", 1_000_000), ("us", 1_000)] {
            if ns.is_multiple_of(per) {
                return format!("{}{unit}", ns / per);
            }
        }
        format!("{ns}ns")
    }

    /// What a task's SINGLE reads, as a reader would write it, with the bit
    /// it is when that is not the first.
    fn single_read(db: &RootDatabase, read: &SingleRead) -> String {
        let at = match read.shift {
            0 => String::new(),
            shift => format!(" bit {shift}"),
        };
        let (mut text, elements) = match &read.source {
            SingleSource::Address(address) => return format!("{}{at}", address.text),
            SingleSource::Global { global, elements } => {
                (global.name_with_case(db).text(db).to_string(), elements)
            }
            SingleSource::Output {
                instance,
                output,
                elements,
            } => (
                format!(
                    "{}.{}",
                    instance.text(db),
                    output.name_with_case(db).text(db)
                ),
                elements,
            ),
        };
        for step in elements {
            match step {
                SourceStep::Element(name) => {
                    let _ = write!(text, ".{}", name.text(db));
                }
                SourceStep::Index(subscripts) => {
                    let _ = write!(text, "{subscripts:?}");
                }
            }
        }
        text + &at
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
            let period = t
                .trigger
                .interval_ns
                .map(|ns| format!("every {}", duration(ns)));
            let edge = t
                .trigger
                .single
                .as_ref()
                .map(|single| format!("on the rising edge of {}", single_read(db, single)));
            let trigger = [period, edge].into_iter().flatten().collect::<Vec<_>>();
            let _ = writeln!(
                out,
                "  TASK {} {}, {priority}",
                t.name.text(db),
                trigger.join(" and ")
            );
            for p in &t.programs {
                let _ = writeln!(
                    out,
                    "    PROGRAM {} : {}",
                    p.instance_name.text(db),
                    p.program.name_with_case(db).text(db)
                );
            }
        }
    }
    out
}

/// Every value lowering needs is resolved here: the resource and its `ON` type,
/// the interval in nanoseconds, the priority as a number, and the instances
/// bound to each task. Nothing downstream re-derives or re-parses any of it.
/// Two RESOURCEs, which E1403 refuses: the schedule is read on its own here,
/// for the day a configuration may have more than one.
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
        TASK Ev(SINGLE := TRUE, PRIORITY := 3);
        PROGRAM PA WITH Bound : P;
        PROGRAM PB WITH Ev : P;
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

/// What runs each task is resolved here too: its period, and what its SINGLE
/// reads, all the way to the variable or the address. A located variable is
/// the address it is located at, and FALSE is no edge at all.
#[rstest]
fn resolved_schedule_carries_what_a_single_reads(mut with_db: RootDatabase) {
    let source = r#"
TYPE Flags : STRUCT go : BOOL; END_STRUCT; END_TYPE
PROGRAM P
VAR_OUTPUT done : BOOL; status : Flags; bits : ARRAY[1..3] OF BOOL; END_VAR
    done := NOT done;
END_PROGRAM

CONFIGURATION Cfg
    VAR_GLOBAL
        go : BOOL;
        flags : Flags;
        located AT %IX4.0 : BOOL;
        status : WORD;
        image AT %IW4 : WORD;
    END_VAR
    RESOURCE R ON CPU
        TASK Edge(SINGLE := go, PRIORITY := 0);
        TASK Both(SINGLE := go, INTERVAL := T#10ms, PRIORITY := 1);
        TASK NoPeriod(SINGLE := go, INTERVAL := T#0s, PRIORITY := 2);
        TASK Off(SINGLE := FALSE, INTERVAL := T#20ms, PRIORITY := 3);
        TASK Prefix(SINGLE := R.go, PRIORITY := 4);
        TASK Element(SINGLE := R.flags.go, PRIORITY := 5);
        TASK Output(SINGLE := P1.done, PRIORITY := 6);
        TASK OutputElement(SINGLE := p1.Status.GO, PRIORITY := 7);
        TASK OutputBit(SINGLE := P1.bits[2], PRIORITY := 8);
        TASK Bit(SINGLE := %IX2.1, PRIORITY := 9);
        TASK Located(SINGLE := located, PRIORITY := 10);
        TASK Part(SINGLE := status.%X3, PRIORITY := 11);
        TASK LocatedPart(SINGLE := image.3, PRIORITY := 12);
        PROGRAM P1 WITH Edge : P;
        PROGRAM P2 WITH Both : P;
        PROGRAM P3 WITH NoPeriod : P;
        PROGRAM P4 WITH Off : P;
        PROGRAM P5 WITH Prefix : P;
        PROGRAM P6 WITH Element : P;
        PROGRAM P7 WITH Output : P;
        PROGRAM P8 WITH OutputElement : P;
        PROGRAM P9 WITH OutputBit : P;
        PROGRAM P10 WITH Bit : P;
        PROGRAM P11 WITH Located : P;
        PROGRAM P12 WITH Part : P;
        PROGRAM P13 WITH LocatedPart : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    add_sources(&mut with_db, &[source]);
    assert_snapshot!(resolved_schedule(&with_db), @r"
    RESOURCE R ON CPU
      TASK Edge on the rising edge of go, priority 0
        PROGRAM P1 : P
      TASK Both every 10ms and on the rising edge of go, priority 1
        PROGRAM P2 : P
      TASK NoPeriod on the rising edge of go, priority 2
        PROGRAM P3 : P
      TASK Off every 20ms, priority 3
        PROGRAM P4 : P
      TASK Prefix on the rising edge of go, priority 4
        PROGRAM P5 : P
      TASK Element on the rising edge of flags.go, priority 5
        PROGRAM P6 : P
      TASK Output on the rising edge of P1.done, priority 6
        PROGRAM P7 : P
      TASK OutputElement on the rising edge of P1.status.go, priority 7
        PROGRAM P8 : P
      TASK OutputBit on the rising edge of P1.bits[2], priority 8
        PROGRAM P9 : P
      TASK Bit on the rising edge of %IX2.1, priority 9
        PROGRAM P10 : P
      TASK Located on the rising edge of %IX4.0, priority 10
        PROGRAM P11 : P
      TASK Part on the rising edge of status bit 3, priority 11
        PROGRAM P12 : P
      TASK LocatedPart on the rising edge of %IW4 bit 3, priority 12
        PROGRAM P13 : P
    ");
}

/// A RESOURCE whose tasks all fail to resolve contributes nothing rather than
/// an empty group, so a consumer never has to skip blanks. Two RESOURCEs,
/// which E1403 refuses: the schedule is read on its own here, for the day a
/// configuration may have more than one.
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

/// A global spec is a NAME LIST, `ga, gb : INT`, or one name, located or
/// not: every name declares a global.
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
/// `hir::direct_variables`.
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
    [E1403] Error: more than one RESOURCE
       ,-[ file:///test0.st:5:14 ]
       |
     5 |     RESOURCE Core0 ON CPU
       |              ^^|^^
       |                `---- the configuration declares 2 RESOURCEs (Core0, Core1)
       |
     9 |     RESOURCE Core1 ON CPU
       |              ^^|^^
       |                `---- 'Core1' is declared here
       |
       | Help: deploy one RESOURCE per runtime
       |
       | Note: a deployment drives one RESOURCE
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
    [E1403] Error: more than one RESOURCE
       ,-[ file:///test0.st:5:14 ]
       |
     5 |     RESOURCE Core0 ON CPU
       |              ^^|^^
       |                `---- the configuration declares 2 RESOURCEs (Core0, Core1)
       |
       |-[ file:///test1.st:3:14 ]
       |
     3 |     RESOURCE Core1 ON CPU
       |              ^^|^^
       |                `---- 'Core1' is declared here
       |
       | Help: deploy one RESOURCE per runtime
       |
       | Note: a deployment drives one RESOURCE
    ---'
    [E1403] Error: more than one RESOURCE
       ,-[ file:///test1.st:3:14 ]
       |
     3 |     RESOURCE Core1 ON CPU
       |              ^^|^^
       |                `---- the configuration declares 2 RESOURCEs (Core0, Core1)
       |
       |-[ file:///test0.st:5:14 ]
       |
     5 |     RESOURCE Core0 ON CPU
       |              ^^|^^
       |                `---- 'Core0' is declared here
       |
       | Help: deploy one RESOURCE per runtime
       |
       | Note: a deployment drives one RESOURCE
    ---'
    ");
}

/// A VAR_CONFIG entry repeats the variable's type, or an alias of the same
/// storage: `INT` for a `Tally` declared `INT := 5`. Another storage is
/// E1426, however wide.
#[rstest]
fn a_var_config_entry_repeats_the_variables_storage(mut with_db: RootDatabase) {
    let source = r#"
TYPE Tally : INT := 5; END_TYPE

FUNCTION_BLOCK Motor
VAR seen AT %M* : Tally; END_VAR
    seen := seen + 1;
END_FUNCTION_BLOCK

PROGRAM P
VAR a : Motor; b : Motor; c : Motor; END_VAR
    a(); b(); c();
END_PROGRAM

CONFIGURATION Cfg
VAR_CONFIG
    Res.P1.a.seen AT %MW0 : Tally;
    Res.P1.b.seen AT %MW1 : INT;
    Res.P1.c.seen AT %MW2 : UINT;
END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1426] Error: configuration entry refused
        ,-[ file:///test0.st:18:14 ]
        |
     18 |     Res.P1.c.seen AT %MW2 : UINT;
        |              ^^|^
        |                `--- the entry says 'UINT', but 'seen' is declared 'Tally'
        |
        | Help: write the declared type
        |
        | Note: the entry repeats the variable's type
    ----'
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

/// A VAR_CONFIG entry repeats its variable's type, so a path with nothing
/// after it, or with its location and nothing else, misses it (E0003). It was
/// a bare syntax error over the whole entry.
#[rstest]
fn invalid_var_config_entry_without_a_type(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM F
VAR k : INT; END_VAR
VAR x AT %Q* : INT; END_VAR
END_PROGRAM

CONFIGURATION Cfg
VAR_CONFIG
    Res.P1.k;
    Res.P1.x AT %QW0;
END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : F;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0003] Error: missing variable type
       ,-[ file:///test0.st:9:5 ]
       |
     9 |     Res.P1.k;
       |     ^^^^|^^^
       |         `----- the entry gives no type
    ---'
    [E0003] Error: missing variable type
        ,-[ file:///test0.st:10:5 ]
        |
     10 |     Res.P1.x AT %QW0;
        |     ^^^^^^^^|^^^^^^^
        |             `--------- the entry gives no type
    ----'
    [E1425] Error: variable not located
        ,-[ file:///test0.st:14:17 ]
        |
     14 |         PROGRAM P1 WITH T : F;
        |                 ^|
        |                  `-- no VAR_CONFIG entry locates 'P1.x', declared AT %Q*
        |
        | Help: give it its address in VAR_CONFIG, as in 'Res.P1.x AT %QW0 : INT;'
    ----'
    ");
}

/// VAR_CONFIG reaches an instance's members, and a VAR_TEMP or a
/// VAR_EXTERNAL is none: neither takes an address or a value there. The
/// report says what each is, and points at the VAR_GLOBAL an external
/// names.
#[rstest]
fn invalid_config_entry_on_a_temp_or_external(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION Cfg
VAR_GLOBAL g : INT := 5; END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Main;
    END_RESOURCE
    VAR_CONFIG
        Res.P1.f.g AT %QW0 : INT;
        Res.P1.f.scratch : INT := 5;
    END_VAR
END_CONFIGURATION

FUNCTION_BLOCK Fb
VAR_TEMP scratch : INT; END_VAR
VAR_EXTERNAL g : INT; END_VAR
    scratch := g;
END_FUNCTION_BLOCK

PROGRAM Main
VAR f : Fb; END_VAR
    f();
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1426] Error: configuration entry refused
        ,-[ file:///test0.st:9:18 ]
        |
      3 | VAR_GLOBAL g : INT := 5; END_VAR
        |            |
        |            `-- the VAR_GLOBAL 'g' is declared here
        |
      9 |         Res.P1.f.g AT %QW0 : INT;
        |                  |
        |                  `-- 'g' is a VAR_EXTERNAL of 'Fb', the VAR_GLOBAL 'g'
        |
     16 | VAR_EXTERNAL g : INT; END_VAR
        |              |
        |              `-- 'g' is declared here
        |
        | Help: give the VAR_GLOBAL 'g' its address or value where it is declared
        |
        | Note: a VAR_CONFIG entry names what an instance holds
    ----'
    [E1426] Error: configuration entry refused
        ,-[ file:///test0.st:10:18 ]
        |
     10 |         Res.P1.f.scratch : INT := 5;
        |                  ^^^|^^^
        |                     `----- 'scratch' is a VAR_TEMP of 'Fb', made for each call
        |
     15 | VAR_TEMP scratch : INT; END_VAR
        |          ^^^|^^^
        |             `----- 'scratch' is declared here
        |
        | Note: a VAR_CONFIG entry names what an instance holds
    ----'
    ");
}

/// Two parameters of a TASK are separated by a comma. The comma was
/// optional after SINGLE and after INTERVAL, so that either could end the
/// list, and `INTERVAL := T#10ms PRIORITY := 1` passed with no diagnostic.
/// What follows the missing comma is not read, so the TASK has no PRIORITY
/// either.
#[rstest]
fn invalid_task_parameters_without_a_comma(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Main
VAR n : INT; END_VAR
    n := n + 1;
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Res ON CPU
        TASK Fast(INTERVAL := T#10ms PRIORITY := 1);
        TASK Slow(INTERVAL := T#1s, PRIORITY := 2);
        PROGRAM P1 WITH Slow : Main;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1405] Error: missing PRIORITY in TASK
       ,-[ file:///test0.st:9:9 ]
       |
     9 |         TASK Fast(INTERVAL := T#10ms PRIORITY := 1);
       |         ^^^^^^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^^^^^^
       |                              `----------------------- the TASK has no PRIORITY
    ---'
    [E0001] Error: syntax error
       ,-[ file:///test0.st:9:38 ]
       |
     9 |         TASK Fast(INTERVAL := T#10ms PRIORITY := 1);
       |                                      ^^^^^^|^^^^^^
       |                                            `-------- unexpected token(s): 'PRIORITY := 1'
    ---'
    ");
}
