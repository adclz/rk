//! What a LIBRARY contributes to the module, and what it does not.
//!
//! A library is code, not a PLC. Its POUs are there to be used, but the
//! CONFIGURATION it declares describes the machine its own author was
//! building. Counted as this workspace's, it spent the one configuration a
//! workspace may have (E1402) before the user wrote a line, and its programs
//! were allocated memory and SCHEDULED into their scan.

use db::RootDatabase;
use rstest::rstest;

use crate::tests::utils::{
    add_library_sources, add_sources, lower_workspace, test_diagnostics_with_library, with_db,
};

/// A library with a complete PLC of its own: a program, a resource, a task.
const LIBRARY: &str = r#"
FUNCTION_BLOCK LibFb
VAR n : DINT; END_VAR
    n := n + 1;
END_FUNCTION_BLOCK

PROGRAM LibProg
VAR b : LibFb; END_VAR
    b();
END_PROGRAM

CONFIGURATION LibCfg
    RESOURCE LibRes ON CPU
        TASK LibT(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM LP WITH LibT : LibProg;
    END_RESOURCE
END_CONFIGURATION
"#;

/// The workspace's own, using the library's block.
const WORKSPACE: &str = r#"
PROGRAM Main
VAR m : LibFb; x : DINT; END_VAR
    m();
    x := x + 1;
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Res ON CPU
        TASK Cycle(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH Cycle : Main;
    END_RESOURCE
END_CONFIGURATION
"#;

/// The workspace may declare its own CONFIGURATION even when the library
/// already declares one.
#[rstest]
fn a_librarys_configuration_does_not_spend_the_workspaces_one(mut with_db: RootDatabase) {
    let reported = test_diagnostics_with_library(&mut with_db, &[LIBRARY], &[WORKSPACE]);
    assert_eq!(
        reported, "",
        "the library's CONFIGURATION is not this workspace's second one"
    );
}

/// And it is not scheduled into this workspace's scan.
///
/// The diagnostic was only half of it: the fragments MIR lowers came from
/// every index it was handed, library files included, so the library's program
/// was given an instance in linear memory and a slot in a task. The PLC ran
/// someone else's program every cycle.
#[rstest]
fn a_librarys_programs_are_not_scheduled(mut with_db: RootDatabase) {
    add_library_sources(&mut with_db, &[LIBRARY]);
    add_sources(&mut with_db, &[WORKSPACE]);

    // Every file, in the order the CLI hands them over: the workspace's and
    // the library's together.
    let module = lower_workspace(&with_db);

    let schedule = module.schedule.as_ref().expect("a schedule");
    let scheduled: Vec<String> = schedule
        .tasks
        .iter()
        .flat_map(|t| {
            t.programs
                .iter()
                .map(|p| format!("{}:{}", t.name.text(&with_db), p.prog_name.text(&with_db)))
        })
        .collect();
    assert_eq!(
        scheduled,
        vec!["Cycle:Main".to_string()],
        "only the workspace's own program is scanned"
    );
}

/// A PROGRAM the library declares does not make the workspace's own a
/// duplicate, and the workspace's is the one that resolves.
#[rstest]
fn a_librarys_program_does_not_collide_with_the_workspaces(mut with_db: RootDatabase) {
    let lib = r#"
PROGRAM Shared
VAR n : DINT; END_VAR
    n := n + 1;
END_PROGRAM
"#;
    let workspace = r#"
PROGRAM Shared
VAR x : DINT; END_VAR
    x := x + 2;
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Res ON CPU
        TASK Cycle(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH Cycle : Shared;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_eq!(
        test_diagnostics_with_library(&mut with_db, &[lib], &[workspace]),
        "",
        "the library's PROGRAM is not this workspace's duplicate"
    );
}

// ── What a module EXPORTS ─────────────────────────────────────────────────
//
// An export is a root: an optimizer keeps whatever a host could call. With
// every POU exported, and the standard library's 341 tests lowered into every
// module, a one-line program compiled to 128 KB and Binaryen could remove none
// of it. A FUNCTION is exported when it says so, with `{export}`.

/// A library with a helper the workspace calls, a FUNCTION of its own it
/// wants a host to reach, and a test.
const LIBRARY_WITH_A_TEST: &str = r#"
FUNCTION_BLOCK LibFb
VAR n : DINT; END_VAR
    n := n + 1;
END_FUNCTION_BLOCK

FUNCTION LibHelper : DINT
VAR_INPUT a : DINT; END_VAR
    LibHelper := a + 1;
END_FUNCTION

{export}
FUNCTION LibEntry : DINT
    LibEntry := 7;
END_FUNCTION

{test}
FUNCTION lib_test
VAR f : LibFb; END_VAR
    f();
END_FUNCTION
"#;

const WORKSPACE_WITH_A_TEST: &str = r#"
{export}
FUNCTION Twice : DINT
VAR_INPUT a : DINT; END_VAR
    Twice := Once(a := a) + Once(a := a);
END_FUNCTION

FUNCTION Once : DINT
VAR_INPUT a : DINT; END_VAR
    Once := LibHelper(a := a);
END_FUNCTION

FUNCTION_BLOCK Counter
VAR n : DINT; END_VAR
METHOD Reset
    n := 0;
END_METHOD
    n := n + 1;
END_FUNCTION_BLOCK

PROGRAM Main
VAR c : Counter; END_VAR
    c();
END_PROGRAM

{test}
FUNCTION ws_test
VAR x : DINT; END_VAR
    x := Twice(a := 1);
END_FUNCTION
"#;

/// The workspace's files and the library's, lowered together as the CLI does.
fn lower_with_library(db: &mut RootDatabase, library: &str, workspace: &str) -> mir::MirModule {
    add_library_sources(db, &[library]);
    add_sources(db, &[workspace]);
    lower_workspace(db)
}

/// The function exports and the custom sections of a module.
fn exports_and_sections(wasm: &[u8]) -> (Vec<String>, Vec<String>) {
    let (mut exports, mut sections) = (Vec::new(), Vec::new());
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        match payload.expect("a valid module") {
            wasmparser::Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.expect("an export");
                    if export.kind == wasmparser::ExternalKind::Func {
                        exports.push(export.name.to_string());
                    }
                }
            }
            wasmparser::Payload::CustomSection(c) => sections.push(c.name().to_string()),
            _ => {}
        }
    }
    exports.sort();
    (exports, sections)
}

/// `{export}` is the only thing that exports a FUNCTION, wherever it is
/// declared. An FB body and a method never are: the host has no instance to
/// call them on. A PROGRAM body is, for the schedule.
#[rstest]
fn a_function_is_exported_when_it_says_so(mut with_db: RootDatabase) {
    use mir::function::MirLinkage;
    let module = lower_with_library(&mut with_db, LIBRARY_WITH_A_TEST, WORKSPACE_WITH_A_TEST);
    let linkage = |name: &str| {
        module
            .functions
            .iter()
            .find(|f| f.name.text(&with_db) == name)
            .unwrap_or_else(|| panic!("`{name}` was not lowered"))
            .linkage
            .clone()
    };
    assert_eq!(linkage("Twice"), MirLinkage::Export, "{{export}}");
    assert_eq!(
        linkage("LibEntry"),
        MirLinkage::Export,
        "{{export}}, in a library"
    );
    assert_eq!(
        linkage("Once"),
        MirLinkage::Internal,
        "the workspace's, unmarked"
    );
    assert_eq!(
        linkage("LibHelper"),
        MirLinkage::Internal,
        "called, never exported"
    );
    assert_eq!(linkage("Counter$__body__"), MirLinkage::Internal);
    assert_eq!(linkage("Counter#Reset"), MirLinkage::Internal);
    assert_eq!(linkage("LibFb$__body__"), MirLinkage::Internal);
    assert_eq!(
        linkage("Main$__body__"),
        MirLinkage::Export,
        "the schedule names it"
    );
    assert_eq!(
        linkage("ws_test"),
        MirLinkage::Export,
        "the runner calls it"
    );
}

/// The manifest lists the workspace's tests alone, so a library's could never
/// run from here. They are not lowered either.
#[rstest]
fn a_librarys_tests_are_not_lowered(mut with_db: RootDatabase) {
    let module = lower_with_library(&mut with_db, LIBRARY_WITH_A_TEST, WORKSPACE_WITH_A_TEST);
    let names: Vec<&str> = module
        .functions
        .iter()
        .map(|f| f.name.text(&with_db).as_str())
        .collect();
    assert!(!names.contains(&"lib_test"), "lowered anyway: {names:?}");
    assert!(
        names.contains(&"ws_test"),
        "the workspace's own test stays: {names:?}"
    );
    let listed: Vec<&str> = module
        .test_manifest
        .tests
        .iter()
        .map(|t| t.path.as_str())
        .collect();
    assert_eq!(listed, vec!["ws_test"]);
}

/// What the module exports, in each profile. A release is what a plant runs:
/// its tests are still lowered, which keeps the memory layout equal to the
/// debug build's, but none is exported and none is listed, so the optimizer
/// is free to remove them.
#[rstest]
fn a_release_exports_no_test_and_lists_none(mut with_db: RootDatabase) {
    let module = lower_with_library(&mut with_db, LIBRARY_WITH_A_TEST, WORKSPACE_WITH_A_TEST);
    let debug = wasm_codegen::generate_wasm(&with_db, &module).finish();
    let release =
        wasm_codegen::generate_wasm_profile(&with_db, &module, wasm_codegen::Profile::Release)
            .finish();

    let (exports, sections) = exports_and_sections(&debug);
    assert_eq!(exports, ["LibEntry", "Main$__body__", "Twice", "ws_test"]);
    assert!(
        sections.contains(&"test-manifest".to_string()),
        "{sections:?}"
    );

    let (exports, sections) = exports_and_sections(&release);
    assert_eq!(exports, ["LibEntry", "Main$__body__", "Twice"]);
    assert!(
        !sections.contains(&"test-manifest".to_string()),
        "{sections:?}"
    );
}

/// An `{extern}` import is not re-exported: exported, it could never be
/// dropped, and every module asked its host for a clock whether it timed
/// anything or not.
#[rstest]
fn an_import_is_not_re_exported(mut with_db: RootDatabase) {
    let library = r#"
{extern 'host' 'now'}
FUNCTION HostNow : LINT
END_FUNCTION
"#;
    let workspace = r#"
{export}
FUNCTION Stamp : LINT
    Stamp := HostNow();
END_FUNCTION
"#;
    let module = lower_with_library(&mut with_db, library, workspace);
    let wasm = wasm_codegen::generate_wasm(&with_db, &module).finish();
    let (exports, _) = exports_and_sections(&wasm);
    assert_eq!(exports, ["Stamp"]);
}
