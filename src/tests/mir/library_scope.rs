//! What a LIBRARY contributes to the module, and what it does not.
//!
//! A library is code, not a PLC. Its POUs are there to be used, but the
//! CONFIGURATION it declares describes the machine its own author was
//! building. Counted as this workspace's, it spent the one configuration a
//! workspace may have (E0242) before the user wrote a line, and its programs
//! were allocated memory and SCHEDULED into their scan.

use auto_lsp::default::db::BaseDatabase;
use db::{RootDatabase, WorkspaceDataBase};
use hir::hir_def::semantic_index::semantic_index;
use rstest::rstest;

use crate::tests::utils::{
    add_library_sources, add_sources, test_diagnostics_with_library, with_db,
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
    let files: Vec<_> = with_db
        .get_files()
        .iter()
        .map(|e| *e.value())
        .chain(with_db.get_library_files().iter().map(|e| *e.value()))
        .collect();
    let indices: Vec<_> = files
        .iter()
        .map(|file| semantic_index(&with_db, *file))
        .collect();
    let module =
        mir::lower::lower_module::lower_modules(&with_db, &indices).expect("the module lowers");

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
