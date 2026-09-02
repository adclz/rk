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

#[rstest]
fn an_unknown_relative_path_is_still_reported(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Lib
    FUNCTION api : INT
        api := Nope.hidden();
    END_FUNCTION
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r#"
    [E0204] Error: no item found in scope
       ,-[ file:///test0.st:4:16 ]
       |
     4 |         api := Nope.hidden();
       |                ^^|^
       |                  `--- no item "Nope" found in scope
    ---'
    "#);
}
