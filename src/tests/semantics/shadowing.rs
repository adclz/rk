use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, test_diagnostics_with_library, with_db};

// Workspace vs library, and file scope vs USING — the two rulings:
// * a workspace declaration in the SAME namespace as a library's is E0102,
//   reported on the WORKSPACE side (the library is never the one flagged);
// * a file-scope declaration SHADOWS a USING import (local wins, silently),
//   while the qualified path still reaches the import.

const LIB: &str = r#"
NAMESPACE Std.Timers
    FUNCTION_BLOCK TON
        VAR_INPUT IN : BOOL; PT : TIME; END_VAR
        VAR_OUTPUT Q : BOOL; END_VAR
    END_FUNCTION_BLOCK
END_NAMESPACE
"#;

#[rstest]
fn a_workspace_duplicate_of_a_library_pou_is_refused(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Std.Timers
    FUNCTION_BLOCK TON
        VAR_INPUT IN : BOOL; PT : TIME; END_VAR
        VAR_OUTPUT Q : BOOL; END_VAR
        Q := TRUE;
    END_FUNCTION_BLOCK
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics_with_library(&mut with_db, &[LIB], &[source]), @r"
    [E0102] Error: duplicate definitions
       ,-[ file:///test0.st:3:20 ]
       |
     3 |     FUNCTION_BLOCK TON
       |                    ^|^
       |                     `--- duplicate POU 'TON'
       |
       |-[ file:///lib0.st:3:20 ]
       |
     3 |     FUNCTION_BLOCK TON
       |                    ^|^
       |                     `--- POU 'TON' is already defined here
    ---'
    ");
}

#[rstest]
fn reopening_a_library_namespace_with_new_names_is_fine(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Std.Timers
    FUNCTION_BLOCK MY_EXTRA
        VAR_OUTPUT Q : BOOL; END_VAR
        Q := TRUE;
    END_FUNCTION_BLOCK
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics_with_library(&mut with_db, &[LIB], &[source]), @r"");
}

#[rstest]
fn a_file_scope_declaration_shadows_a_using_import(mut with_db: RootDatabase) {
    // Local wins, silently (user-ruled): no diagnostic, and the execution
    // side is pinned by codegen::namespaces. The import stays reachable
    // qualified.
    let source = r#"
NAMESPACE N
    FUNCTION_BLOCK X
        VAR_OUTPUT Q : INT; END_VAR
        Q := 1;
    END_FUNCTION_BLOCK
END_NAMESPACE

FUNCTION_BLOCK X
    VAR_OUTPUT Q : INT; END_VAR
    Q := 2;
END_FUNCTION_BLOCK

USING N;

FUNCTION f : INT
    VAR a : X; b : N.X; END_VAR
    a();
    b();
    f := a.Q * 10 + b.Q;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}
