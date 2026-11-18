// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What a build keeps of a module (`MirModule::keep_reached`): the
//! functions lowering marks for export, which are its roots, what those
//! reach, and the imports they call, numbered again from zero, imports
//! first.

use std::collections::HashMap;

use db::RootDatabase;
use insta::assert_snapshot;
use mir::function::MirLinkage;
use rstest::rstest;

use crate::tests::utils::{add_sources, assert_workspace_is_clean, lower_workspace, with_db};

/// What `sources` keep once lowered: each import by its module and name,
/// with the longest list a call keeps when it gives one, each function by
/// its name, `export` for a root, a function lowering marks for export (a
/// test is one in both profiles, which codegen leaves unexported in a
/// release build). The numbers are checked on the way, imports first, each
/// name finding its own function and none left over, and so is each kept
/// import's longest list, the one it had before the pass.
fn kept(db: &mut RootDatabase, sources: &[&str]) -> String {
    add_sources(db, sources);
    assert_workspace_is_clean(db);
    let mut module = lower_workspace(db);
    let longest_before: HashMap<_, _> = module
        .extern_functions
        .iter()
        .zip(&module.longest_lists)
        .map(|(ext, longest)| (ext.name, *longest))
        .collect();
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
    let count = imports + module.functions.len();
    assert_eq!(
        module.function_indices.len(),
        count,
        "a name left for a dropped function"
    );
    assert_eq!(module.longest_lists.len(), imports);
    for (ext, longest) in module.extern_functions.iter().zip(&module.longest_lists) {
        assert_eq!(
            Some(longest),
            longest_before.get(&ext.name),
            "{}",
            ext.name.text(db)
        );
    }

    let mut lines: Vec<String> = module
        .extern_functions
        .iter()
        .zip(&module.longest_lists)
        .map(|(ext, longest)| match longest {
            0 => format!("import {}.{}", ext.module, ext.import_name),
            _ => format!(
                "import {}.{} (longest list {longest})",
                ext.module, ext.import_name
            ),
        })
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
/// dropped; what the PROGRAM calls stays, its block's method included. The
/// import kept was declared after the one dropped, and takes its number.
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
            p := Speed();
        END_PROGRAM
    "#;
    assert_snapshot!(kept(&mut with_db, &[source]), @r"
    export P$__body__
    func Motor#Start
    func Used
    import acme:drive/axis@1.0.0.speed
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

/// A kept import that gives a list keeps the longest list its calls keep,
/// though an import declared before it, which a dropped FUNCTION called
/// with a longer one, goes.
#[rstest]
fn a_kept_import_keeps_its_longest_list(mut with_db: RootDatabase) {
    let source = r#"
        {extern 'acme:drive/axis@1.0.0' 'first'}
        FUNCTION first
        VAR_INPUT limit : UDINT; END_VAR
        VAR_IN_OUT values : ARRAY[*] OF UDINT; END_VAR
        VAR_OUTPUT w0 : LWORD; w1 : LWORD; END_VAR
        END_FUNCTION

        {extern 'acme:drive/axis@1.0.0' 'second'}
        FUNCTION second
        VAR_INPUT limit : UDINT; END_VAR
        VAR_IN_OUT values : ARRAY[*] OF UDINT; END_VAR
        VAR_OUTPUT w0 : LWORD; w1 : LWORD; END_VAR
        END_FUNCTION

        FUNCTION Unused : UDINT
        VAR many : ARRAY[0..98] OF UDINT; END_VAR
            first(limit := 99, values := many);
            Unused := many[0];
        END_FUNCTION

        PROGRAM P
        VAR few : ARRAY[0..6] OF UDINT; END_VAR
            second(limit := 7, values := few);
        END_PROGRAM
    "#;
    assert_snapshot!(kept(&mut with_db, &[source]), @r"
    export P$__body__
    import acme:drive/axis@1.0.0.second (longest list 7)
    ");
}

/// A call through an interface parameter is lowered to a copy of its
/// callee for the implementer bound, and a sibling call in a method's copy
/// to the owner's own override: each names one function, and the walk
/// keeps it. The base's methods, which no instance of the base calls, go.
#[rstest]
fn a_call_dispatch_resolves_keeps_its_callee(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE IRun
        METHOD Run : INT
        END_METHOD
        END_INTERFACE

        FUNCTION_BLOCK Impl IMPLEMENTS IRun
        METHOD Run : INT
            Run := 1;
        END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION Drive : INT
        VAR_INPUT x : IRun; END_VAR
            Drive := x.Run();
        END_FUNCTION

        FUNCTION_BLOCK Base
        METHOD Step : INT
            Step := Hook();
        END_METHOD
        METHOD Hook : INT
            Hook := 1;
        END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Derived EXTENDS Base
        METHOD OVERRIDE Hook : INT
            Hook := 2;
        END_METHOD
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR impl : Impl; d : Derived; a : INT; b : INT; END_VAR
            a := Drive(x := impl);
            b := d.Step();
        END_PROGRAM
    "#;
    assert_snapshot!(kept(&mut with_db, &[source]), @r"
    export P$__body__
    func Derived#Hook
    func Derived#Step
    func Drive$@Impl
    func Impl#Run
    ");
}

/// A FUNCTION called only from a starting value stays: a FUNCTION's and a
/// METHOD's variables start at each call, by code in its body, which the
/// walk sees. A block's or a PROGRAM's starting value is a constant (E0401)
/// and calls nothing.
#[rstest]
fn a_function_only_a_starting_value_calls_stays(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION ForFunction : INT
            ForFunction := 1;
        END_FUNCTION

        FUNCTION ForMethod : INT
            ForMethod := 2;
        END_FUNCTION

        FUNCTION Starts : INT
        VAR y : INT := ForFunction(); END_VAR
            Starts := y;
        END_FUNCTION

        FUNCTION_BLOCK Holder
        METHOD Get : INT
        VAR v : INT := ForMethod(); END_VAR
            Get := v;
        END_METHOD
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR h : Holder; z : INT; END_VAR
            z := Starts() + h.Get();
        END_PROGRAM
    "#;
    assert_snapshot!(kept(&mut with_db, &[source]), @r"
    export P$__body__
    func ForFunction
    func ForMethod
    func Holder#Get
    func Starts
    ");
}

/// A test FUNCTION_BLOCK runs through the function `rk test` calls for
/// each cycle, which calls its body: both stay, with what the body calls.
#[rstest]
fn a_test_block_keeps_its_body(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION Helper : INT
            Helper := 1;
        END_FUNCTION

        {test}
        FUNCTION_BLOCK test_counts
        VAR_OUTPUT DONE : BOOL; END_VAR
        VAR n : INT; END_VAR
            n := n + Helper();
            DONE := n > 2;
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(kept(&mut with_db, &[source]), @r"
    export test_counts$__test__
    func Helper
    func test_counts$__body__
    ");
}
