// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

// A namespace path written inside a namespace is RELATIVE first: `Impl` inside
// `NAMESPACE Lib` means `Lib.Impl`, then `Impl` at the top level. Every
// qualified access — a call, a type, a USING — used to be looked up as written,
// so an enclosing namespace could not name its own nested one without the
// full path.

#[rstest]
fn a_nested_namespace_is_reachable_by_its_relative_path(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Lib
    USING Impl;
    NAMESPACE Impl
        FUNCTION hidden : INT
            hidden := 1;
        END_FUNCTION
        TYPE T : INT; END_TYPE
    END_NAMESPACE
    NAMESPACE A
        NAMESPACE B
            FUNCTION f : INT
                f := 1;
            END_FUNCTION
        END_NAMESPACE
    END_NAMESPACE

    FUNCTION api : INT
        VAR x : Impl.T; END_VAR
        api := Impl.hidden() + A.B.f() + hidden() + Lib.Impl.hidden() + x;
    END_FUNCTION

    NAMESPACE Other
        FUNCTION sibling : INT
            sibling := Impl.hidden();
        END_FUNCTION
    END_NAMESPACE
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A qualified name names what that namespace declares: `NsA.Helper` is not
/// the top-level `Helper`, and `NsB.Api` not the `Api` that `NsB` imports.
/// Both resolved, and ran an unrelated function.
#[rstest]
fn a_qualified_name_stays_inside_its_namespace(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Helper : INT Helper := 1; END_FUNCTION
NAMESPACE NsA
    FUNCTION Api : INT Api := 2; END_FUNCTION
END_NAMESPACE
NAMESPACE NsB
    USING NsA;
END_NAMESPACE

FUNCTION caller : INT
    caller := NsA.Helper() + NsB.Api();
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0201] Error: unknown name
        ,-[ file:///test0.st:11:19 ]
        |
     11 |     caller := NsA.Helper() + NsB.Api();
        |                   ^^^|^^
        |                      `---- no item 'Helper' found in scope
        |
        | Note: an item with a similar name is in scope:
        |       - Helper
    ----'
    [E0201] Error: unknown name
        ,-[ file:///test0.st:11:34 ]
        |
     11 |     caller := NsA.Helper() + NsB.Api();
        |                                  ^|^
        |                                   `--- no item 'Api' found in scope
        |
        | Note: an item with a similar name is available, but needs to be imported:
        |       - 'Api' via USING NsA
    ----'
    ");
}

#[rstest]
fn an_unknown_relative_path_is_still_reported(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Lib
    FUNCTION api : INT
        api := Nope.hidden();
    END_FUNCTION
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0201] Error: unknown name
       ,-[ file:///test0.st:4:16 ]
       |
     4 |         api := Nope.hidden();
       |                ^^|^
       |                  `--- no item 'Nope' found in scope
    ---'
    ");
}
