// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A library's name is the namespace its declarations are in. The test
//! library is named `Std`, as the standard library is, so a FUNCTION at the
//! top of one of its files is `Std.F`.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics_with_library, test_library_diagnostics, with_db};

/// A library's top level is its root: reached under the library's name,
/// and bare, since every library is imported by default.
#[rstest]
fn a_librarys_top_level_is_under_its_name(mut with_db: RootDatabase) {
    let lib = "FUNCTION Seven : INT\n    Seven := 7;\nEND_FUNCTION\n";
    let source = "FUNCTION Run : INT\n    Run := Std.Seven() + Seven();\nEND_FUNCTION\n";
    assert_snapshot!(
        test_diagnostics_with_library(&mut with_db, &[lib], &[source]),
        @""
    );
}

/// A path written from the root is absolute: `NAMESPACE Std.Timers` in the
/// library `Std` is `Std.Timers`, as `NAMESPACE Timers` there is. The
/// standard library names its namespaces the first way.
#[rstest]
fn a_path_written_from_the_root_is_absolute(mut with_db: RootDatabase) {
    let lib = "NAMESPACE Std.Timers\n    FUNCTION Tick : INT\n        Tick := 1;\n    END_FUNCTION\nEND_NAMESPACE\n\
               NAMESPACE Edge\n    FUNCTION Rise : INT\n        Rise := 2;\n    END_FUNCTION\nEND_NAMESPACE\n";
    let source = "USING Std.Timers;\nUSING Std.Edge;\nFUNCTION Run : INT\n    Run := Tick() + Rise() + Std.Edge.Rise();\nEND_FUNCTION\n";
    assert_snapshot!(
        test_diagnostics_with_library(&mut with_db, &[lib], &[source]),
        @""
    );
}

/// A library names its own namespaces relative to its root, or from it:
/// `USING Motion`, `Motion.Twice` and `Std.Motion.Twice` all reach the
/// same declaration from inside the library.
#[rstest]
fn a_library_reaches_its_own_names_from_its_root(mut with_db: RootDatabase) {
    let lib = "NAMESPACE Motion\n    FUNCTION Twice : INT\n    VAR_INPUT n : INT; END_VAR\n        Twice := n * 2;\n    END_FUNCTION\nEND_NAMESPACE\n\
               NAMESPACE Line\n    USING Motion;\n    FUNCTION Run : INT\n        Run := Twice(1) + Motion.Twice(2) + Std.Motion.Twice(3);\n    END_FUNCTION\nEND_NAMESPACE\n\
               FUNCTION Go : INT\n    Go := Line.Run() + Motion.Twice(4);\nEND_FUNCTION\n";
    let source =
        "FUNCTION Main : INT\n    Main := Std.Go() + Go() + Std.Line.Run();\nEND_FUNCTION\n";
    assert_snapshot!(
        test_library_diagnostics(&mut with_db, &[lib], &[source]),
        @""
    );
}

/// The workspace reaches a library's nested namespaces under the root, and
/// its top level bare.
#[rstest]
fn a_workspace_reaches_a_librarys_names_under_its_root(mut with_db: RootDatabase) {
    let lib = "NAMESPACE Line
    FUNCTION Run : INT
        Run := 1;
    END_FUNCTION
END_NAMESPACE
               FUNCTION Go : INT
    Go := Line.Run();
END_FUNCTION
";
    let source = "USING Std.Line;
FUNCTION Main : INT
    Main := Std.Go() + Go() + Std.Line.Run() + Run();
END_FUNCTION
";
    assert_snapshot!(
        test_diagnostics_with_library(&mut with_db, &[lib], &[source]),
        @""
    );
}

/// A namespace a library names after itself is its root, not a namespace
/// of that name inside it: `NAMESPACE Std` in `Std` adds to the top level.
#[rstest]
fn a_namespace_named_after_the_library_is_its_root(mut with_db: RootDatabase) {
    let lib = "NAMESPACE Std\n    FUNCTION Seven : INT\n        Seven := 7;\n    END_FUNCTION\nEND_NAMESPACE\n\
               FUNCTION Eight : INT\n    Eight := Seven() + 1;\nEND_FUNCTION\n";
    let source =
        "USING Std;\nFUNCTION Run : INT\n    Run := Std.Seven() + Eight();\nEND_FUNCTION\n";
    assert_snapshot!(
        test_diagnostics_with_library(&mut with_db, &[lib], &[source]),
        @""
    );
}

/// The workspace's own declaration comes before a library's of the same
/// name: the two are different names now, `Clamp` and `Std.Clamp`, so
/// neither is a duplicate of the other, and the library's stays reachable
/// under its root.
#[rstest]
fn a_workspace_declaration_comes_before_a_librarys(mut with_db: RootDatabase) {
    let lib = "FUNCTION Clamp : INT\n    Clamp := 1;\nEND_FUNCTION\n";
    let source = "FUNCTION Clamp : STRING\n    Clamp := 'mine';\nEND_FUNCTION\n\
                  FUNCTION Run : INT\nVAR s : STRING; END_VAR\n    s := Clamp();\n    Run := Std.Clamp();\nEND_FUNCTION\n";
    assert_snapshot!(
        test_diagnostics_with_library(&mut with_db, &[lib], &[source]),
        @""
    );
}

/// Reopening a library's namespace from the workspace takes its full path:
/// `NAMESPACE Std.Timers`, not `NAMESPACE Timers`, which is the workspace's
/// own namespace of that name.
#[rstest]
fn reopening_a_librarys_namespace_names_its_root(mut with_db: RootDatabase) {
    let lib = "NAMESPACE Timers\n    FUNCTION_BLOCK TON\n    END_FUNCTION_BLOCK\nEND_NAMESPACE\n";
    let source = "NAMESPACE Timers\n    FUNCTION_BLOCK TON\n    END_FUNCTION_BLOCK\nEND_NAMESPACE\n\
                  NAMESPACE Std.Timers\n    FUNCTION_BLOCK TOF\n    END_FUNCTION_BLOCK\nEND_NAMESPACE\n\
                  PROGRAM P\nVAR a : Timers.TON; b : Std.Timers.TON; c : Std.Timers.TOF; END_VAR\nEND_PROGRAM\n";
    assert_snapshot!(
        test_diagnostics_with_library(&mut with_db, &[lib], &[source]),
        @""
    );
}

/// A library's symbols carry its root, in a test's name among them.
#[rstest]
fn a_librarys_test_is_named_under_its_root(mut with_db: RootDatabase) {
    use crate::tests::utils::add_library_sources;
    add_library_sources(
        &mut with_db,
        &[
            "{test}\nFUNCTION t_root : BOOL\n    t_root := TRUE;\nEND_FUNCTION\n\
           NAMESPACE Inner\n{test}\nFUNCTION t_inner : BOOL\n    t_inner := TRUE;\nEND_FUNCTION\nEND_NAMESPACE\n",
        ],
    );
    let mut names: Vec<String> = hir::hir_ty::index_graphs::discover_all_tests(&with_db)
        .into_iter()
        .map(|test| match test {
            hir::hir_ty::index_graphs::TestItem::Function(_, name) => name,
        })
        .collect();
    names.sort();
    assert_eq!(names, ["Std.Inner.t_inner", "Std.t_root"]);
}
