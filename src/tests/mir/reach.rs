// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What a build keeps of a module (`MirModule::keep_reached`): the
//! functions its exports reach, and the imports those call, numbered again
//! from zero, imports first.

use db::RootDatabase;
use insta::assert_snapshot;
use mir::function::MirLinkage;
use rstest::rstest;

use crate::tests::utils::{add_sources, assert_workspace_is_clean, lower_workspace, with_db};

/// What `sources` keep once lowered: each import by its module and name,
/// each function by its name, `export` for a root; the numbers checked on
/// the way, imports first, each function's name finding its own.
fn kept(db: &mut RootDatabase, sources: &[&str]) -> String {
    add_sources(db, sources);
    assert_workspace_is_clean(db);
    let mut module = lower_workspace(db);
    module.keep_reached();

    let imports = module.extern_functions.len();
    for (at, ext) in module.extern_functions.iter().enumerate() {
        assert_eq!(ext.index as usize, at, "{}", ext.name.text(db));
        assert_eq!(module.function_indices.get(&ext.name), Some(&ext.index));
    }
    for (at, func) in module.functions.iter().enumerate() {
        assert_eq!(func.index as usize, imports + at, "{}", func.name.text(db));
        assert_eq!(module.function_indices.get(&func.name), Some(&func.index));
    }
    let count = (imports + module.functions.len()) as u32;
    assert!(
        module.function_indices.values().all(|index| *index < count),
        "a name left for a dropped function"
    );
    assert_eq!(module.longest_lists.len(), imports);

    let mut lines: Vec<String> = module
        .extern_functions
        .iter()
        .map(|ext| format!("import {}.{}", ext.module, ext.import_name))
        .chain(module.functions.iter().map(|func| {
            let linkage = match func.linkage {
                MirLinkage::Export => "export",
                MirLinkage::Internal => "func",
            };
            format!("{linkage} {}", func.name.text(db))
        }))
        .collect();
    lines.sort();
    lines.join("\n")
}

/// A FUNCTION, a METHOD, a block's body and an import no export reaches are
/// dropped; what the PROGRAM calls stays, its block's method included.
#[rstest]
fn what_no_export_reaches_is_dropped(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION Used : INT
            Used := 1;
        END_FUNCTION

        FUNCTION Unused : INT
            Unused := 2;
        END_FUNCTION

        FUNCTION_BLOCK Motor
        METHOD Start : BOOL
            Start := TRUE;
        END_METHOD
        METHOD Stop : BOOL
            Stop := FALSE;
        END_METHOD
        END_FUNCTION_BLOCK

        {extern 'acme:drive/axis@1.0.0' 'position'}
        FUNCTION Position : UDINT
        END_FUNCTION

        {extern 'acme:drive/axis@1.0.0' 'speed'}
        FUNCTION Speed : UDINT
        END_FUNCTION

        PROGRAM P
        VAR m : Motor; x : INT; p : UDINT; ok : BOOL; END_VAR
            x := Used();
            ok := m.Start();
            p := Position();
        END_PROGRAM
    "#;
    assert_snapshot!(kept(&mut with_db, &[source]), @r"
    export P$__body__
    func Motor#Start
    func Used
    import acme:drive/axis@1.0.0.position
    ");
}

/// A FUNCTION only a test calls stays, since a test is a root: exported in
/// a debug build, and left to Binaryen in a release one, as before.
#[rstest]
fn a_function_only_a_test_calls_stays(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION Helper : INT
            Helper := 1;
        END_FUNCTION

        FUNCTION Other : INT
            Other := 2;
        END_FUNCTION

        {test}
        FUNCTION test_helper
        VAR x : INT; END_VAR
            x := Helper();
        END_FUNCTION
    "#;
    assert_snapshot!(kept(&mut with_db, &[source]), @r"
    export test_helper
    func Helper
    ");
}

/// A PROGRAM holding an async block, which calls it with `call`.
fn moves(call: &str) -> String {
    format!(
        r#"
        {{extern async 'acme:drive/axis@1.0.0' 'move'}}
        FUNCTION_BLOCK MOVE
        VAR_INPUT target : DINT; END_VAR
        VAR_OUTPUT DONE : BOOL; END_VAR
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR mv : MOVE; x : INT; END_VAR
            x := x + 1;
            {call}
        END_PROGRAM
    "#
    )
}

/// An async block's import, and the component model's built-ins that check
/// its call, stay with the block's body.
#[rstest]
fn a_called_async_block_brings_its_import_and_the_built_ins(mut with_db: RootDatabase) {
    assert_snapshot!(kept(&mut with_db, &[&moves("mv(target := 5);")]), @r"
    export P$__body__
    func MOVE$__body__
    import $root.[subtask-drop]
    import $root.[waitable-join]
    import $root.[waitable-set-new]
    import $root.[waitable-set-poll]
    import acme:drive/axis@1.0.0.[async-lower]move
    ");
}

/// An async block nothing calls brings neither its import nor the
/// built-ins, though an instance of it is declared.
#[rstest]
fn an_async_block_nothing_calls_brings_nothing(mut with_db: RootDatabase) {
    assert_snapshot!(kept(&mut with_db, &[&moves("")]), @"export P$__body__");
}
