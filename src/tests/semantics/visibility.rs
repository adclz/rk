use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

/// An item that names no specifier is PUBLIC — the one place `rk` departs from
/// the standard's tables, which make PROTECTED the default. Pinned from the
/// outside, where the two answers differ: this call is legal only under PUBLIC.
/// Its counterpart is `access_protected_method_in_non_derived_pou`, the same
/// call with the specifier written out.
#[rstest]
fn valid_access_method_without_a_specifier(mut with_db: RootDatabase) {
    let source = r#"
    FUNCTION_BLOCK Counter
        VAR c : INT; END_VAR
        METHOD Inc : INT
            c := c + 1;
            Inc := c;
        END_METHOD
    END_FUNCTION_BLOCK

    FUNCTION caller : INT
        VAR a : Counter; END_VAR
        caller := a.Inc();
    END_FUNCTION
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// Same for a variable: table 11d also defaults to PROTECTED, and we also
/// don't, so an FB's member is readable from outside without an annotation.
#[rstest]
fn valid_access_variable_without_a_specifier(mut with_db: RootDatabase) {
    let source = r#"
    FUNCTION_BLOCK Counter
        VAR c : INT; END_VAR
    END_FUNCTION_BLOCK

    FUNCTION caller : INT
        VAR a : Counter; END_VAR
        caller := a.c;
    END_FUNCTION
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_access_private_method(mut with_db: RootDatabase) {
    let source = r#"
    CLASS Base
        METHOD PRIVATE myPrivateMethod: INT  END_METHOD

        METHOD testMethod: INT
            THIS.myPrivateMethod();
        END_METHOD
    END_CLASS
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_access_private_method(mut with_db: RootDatabase) {
    let source = r#"
    CLASS Base
        METHOD PRIVATE myPrivateMethod: INT  END_METHOD
    END_CLASS

    CLASS Mid EXTENDS Base
        METHOD testMethod: INT
            SUPER.myPrivateMethod();
        END_METHOD
    END_CLASS
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: access control violation
       ,-[ file:///test0.st:8:19 ]
       |
     8 |             SUPER.myPrivateMethod();
       |                   ^^^^^^^|^^^^^^^
       |                          `--------- can not access PRIVATE item 'myPrivateMethod'
       |
       | Note: variables and methods marked PRIVATE can only be accessed from within the same POU
    ---'
    ");
}

#[rstest]
fn access_internal_method_from_different_namespace(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE ns1
    CLASS Base
        METHOD INTERNAL myInternalMethod: INT  END_METHOD
    END_CLASS
END_NAMESPACE

NAMESPACE ns2
    USING ns1;

    CLASS Mid EXTENDS Base
        METHOD testMethod: INT
            SUPER.myInternalMethod();
        END_METHOD
    END_CLASS
END_NAMESPACE
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0402] Error: access control violation
        ,-[ file:///test0.st:13:19 ]
        |
     13 |             SUPER.myInternalMethod();
        |                   ^^^^^^^^|^^^^^^^
        |                           `--------- can not access INTERNAL item 'myInternalMethod'
        |
        | Note: calling scope is in NAMESPACE 'ns2', item is only available in NAMESPACE 'ns1'
    ----'
    ");
}

#[rstest]
fn access_internal_method_in_namespace_from_global_scope(mut with_db: RootDatabase) {
    let source = r#"
USING ns2;

CLASS Mid EXTENDS Base
    METHOD testMethod: INT
        SUPER.myInternalMethod();
    END_METHOD
END_CLASS

NAMESPACE ns2
    CLASS Base
        METHOD INTERNAL myInternalMethod: INT  END_METHOD
    END_CLASS
END_NAMESPACE
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0402] Error: access control violation
       ,-[ file:///test0.st:6:15 ]
       |
     6 |         SUPER.myInternalMethod();
       |               ^^^^^^^^|^^^^^^^
       |                       `--------- can not access INTERNAL item 'myInternalMethod'
       |
       | Note: calling scope is in the GLOBAL scope, item is only available in NAMESPACE 'ns2'
    ---'
    ");
}

#[rstest]
fn access_internal_method_in_global_scope_from_namespace(mut with_db: RootDatabase) {
    let source = r#"
CLASS Base
    METHOD INTERNAL myInternalMethod: INT  END_METHOD
END_CLASS

NAMESPACE ns2
    CLASS Mid EXTENDS Base
        METHOD testMethod: INT
            SUPER.myInternalMethod();
        END_METHOD
    END_CLASS
END_NAMESPACE
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0402] Error: access control violation
       ,-[ file:///test0.st:9:19 ]
       |
     9 |             SUPER.myInternalMethod();
       |                   ^^^^^^^^|^^^^^^^
       |                           `--------- can not access INTERNAL item 'myInternalMethod'
       |
       | Note: calling scope is in NAMESPACE 'ns2', item scope is only available the GLOBAL scope
    ---'
    ");
}

#[rstest]
fn valid_access_protected_method_in_derived_pou(mut with_db: RootDatabase) {
    let source = r#"
CLASS Base
    METHOD PROTECTED myProtectedMethod: INT  END_METHOD
END_CLASS

FUNCTION_BLOCK fn1 EXTENDS Base
    VAR
        obj: Base;
    END_VAR

    SUPER.myProtectedMethod();

END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn access_protected_method_in_non_derived_pou(mut with_db: RootDatabase) {
    let source = r#"
CLASS Base
    METHOD PROTECTED myProtectedMethod  END_METHOD
END_CLASS

FUNCTION_BLOCK fn1
    VAR
        obj: Base;
    END_VAR

    obj.myProtectedMethod();

END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0403] Error: access control violation
        ,-[ file:///test0.st:11:9 ]
        |
     11 |     obj.myProtectedMethod();
        |         ^^^^^^^^|^^^^^^^^
        |                 `---------- can not access PROTECTED item 'myProtectedMethod'
        |
        | Note: Variables and methods marked PROTECTED are only available within the same POU or derived POUs
    ----'
    ");
}

// `FUNCTION PRIVATE` — our extension; the standard gives a FUNCTION no
// specifier at all. A private function is callable from its own namespace,
// nested namespaces included, on its own side of the library line. The
// stdlib's extern shims are written this way, and used to be public: the
// builder dropped the specifier at lowering.

#[rstest]
fn valid_call_private_function_from_its_namespace(mut with_db: RootDatabase) {
    // Same namespace, a nested one, and the same namespace reopened in a
    // second workspace file; PUBLIC written out is the default made explicit.
    let lib = r#"
    NAMESPACE Lib
        FUNCTION PRIVATE helper : INT
            helper := 1;
        END_FUNCTION
        FUNCTION PUBLIC api : INT
            api := helper();
        END_FUNCTION
        NAMESPACE Test
            FUNCTION t : INT
                t := Lib.helper();
            END_FUNCTION
        END_NAMESPACE
    END_NAMESPACE
    "#;
    let reopened = r#"
    NAMESPACE Lib
        FUNCTION more : INT
            more := helper() + api();
        END_FUNCTION
    END_NAMESPACE
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[lib, reopened]), @r"");
}

#[rstest]
fn valid_call_private_function_declared_at_global_scope(mut with_db: RootDatabase) {
    // The global namespace holds every other one, so a PRIVATE at global
    // scope restricts nothing within the same origin.
    let source = r#"
    FUNCTION PRIVATE helper : INT
        helper := 1;
    END_FUNCTION
    NAMESPACE App
        FUNCTION use : INT
            use := helper();
        END_FUNCTION
    END_NAMESPACE
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_call_private_function_from_another_namespace(mut with_db: RootDatabase) {
    let source = r#"
    NAMESPACE Lib
        FUNCTION PRIVATE helper : INT
            helper := 1;
        END_FUNCTION
    END_NAMESPACE

    NAMESPACE App
        FUNCTION use : INT
            use := Lib.helper();
        END_FUNCTION
    END_NAMESPACE

    USING Lib;
    FUNCTION use_global : INT
        use_global := helper();
    END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0405] Error: access control violation
        ,-[ file:///test0.st:10:20 ]
        |
      3 | ,->         FUNCTION PRIVATE helper : INT
        : :
      5 | |->         END_FUNCTION
        | |
        | `-------------------------- declared PRIVATE here
        |
     10 |                 use := Lib.helper();
        |                        ^^^^^|^^^^
        |                             `------ can not call PRIVATE function 'Lib.helper'
    ----'
    [E0405] Error: access control violation
        ,-[ file:///test0.st:16:23 ]
        |
      3 | ,->         FUNCTION PRIVATE helper : INT
        : :
      5 | |->         END_FUNCTION
        | |
        | `-------------------------- declared PRIVATE here
        |
     16 |             use_global := helper();
        |                           ^^^|^^
        |                              `---- can not call PRIVATE function 'helper'
    ----'
    ");
}

#[rstest]
fn invalid_call_library_private_function_by_reopening_its_namespace(mut with_db: RootDatabase) {
    // Namespaces reopen from any file; the origin half of the rule is what
    // keeps a library's helpers out of workspace code that reopens the
    // library's namespace.
    let lib = r#"
    NAMESPACE Lib
        FUNCTION PRIVATE helper : INT
            helper := 1;
        END_FUNCTION
    END_NAMESPACE
    "#;
    let workspace = r#"
    NAMESPACE Lib
        FUNCTION sneak : INT
            sneak := helper();
        END_FUNCTION
    END_NAMESPACE
    "#;
    let rendered =
        crate::tests::utils::test_diagnostics_with_library(&mut with_db, &[lib], &[workspace]);
    assert_snapshot!(rendered, @r"
    [E0405] Error: access control violation
       ,-[ file:///test0.st:4:22 ]
       |
     4 |             sneak := helper();
       |                      ^^^|^^
       |                         `---- can not call PRIVATE function 'helper'
       |
       |-[ file:///lib0.st:3:9 ]
       |
     3 | ,->         FUNCTION PRIVATE helper : INT
       : :
     5 | |->         END_FUNCTION
       | |
       | `-------------------------- declared PRIVATE here
    ---'
    ");
}

#[rstest]
fn invalid_protected_or_internal_on_a_function(mut with_db: RootDatabase) {
    let source = r#"
    FUNCTION PROTECTED f : INT
        f := 1;
    END_FUNCTION
    FUNCTION INTERNAL g : INT
        g := 1;
    END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0406] Error: access control violation
       ,-[ file:///test0.st:2:14 ]
       |
     2 |     FUNCTION PROTECTED f : INT
       |              ^^^^|^^^^
       |                  `------ 'PROTECTED' does not apply to a FUNCTION: only PRIVATE does
    ---'
    [E0406] Error: access control violation
       ,-[ file:///test0.st:5:14 ]
       |
     5 |     FUNCTION INTERNAL g : INT
       |              ^^^^|^^^
       |                  `----- 'INTERNAL' does not apply to a FUNCTION: only PRIVATE does
    ---'
    ");
}

// `NAMESPACE INTERNAL` — the standard's module-level privacy: reachable only
// from inside the enclosing namespace, nested ones included, on its own side
// of the library line (the FUNCTION PRIVATE boundary). Checked on the target's
// namespace chain, so a qualified, relative, or USING-imported access meets one
// rule; a USING of one is refused at the USING and the uses behind it stand
// down. The keyword used to parse and restrict nothing.

#[rstest]
fn valid_access_internal_namespace_from_its_enclosing_namespace(mut with_db: RootDatabase) {
    let source = r#"
    NAMESPACE Lib
        NAMESPACE INTERNAL Impl
            FUNCTION hidden : INT
                hidden := 1;
            END_FUNCTION
        END_NAMESPACE
        FUNCTION api : INT
            api := Impl.hidden() + Lib.Impl.hidden();
        END_FUNCTION
        NAMESPACE Other
            FUNCTION sibling : INT
                sibling := Impl.hidden();
            END_FUNCTION
        END_NAMESPACE
    END_NAMESPACE

    NAMESPACE INTERNAL Priv
        FUNCTION hidden : INT
            hidden := 1;
        END_FUNCTION
    END_NAMESPACE
    FUNCTION top_level_internal_is_this_origin : INT
        top_level_internal_is_this_origin := Priv.hidden();
    END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_access_internal_namespace_from_outside(mut with_db: RootDatabase) {
    let source = r#"
    NAMESPACE Lib
        NAMESPACE INTERNAL Impl
            FUNCTION hidden : INT
                hidden := 1;
            END_FUNCTION
            TYPE T : INT; END_TYPE
        END_NAMESPACE
        NAMESPACE Mid
            NAMESPACE INTERNAL Deep
                FUNCTION f : INT
                    f := 1;
                END_FUNCTION
            END_NAMESPACE
        END_NAMESPACE
        FUNCTION outside_mid : INT
            outside_mid := Mid.Deep.f();
        END_FUNCTION
    END_NAMESPACE

    NAMESPACE App
        FUNCTION use : INT
            VAR x : Lib.Impl.T; END_VAR
            use := Lib.Impl.hidden() + x;
        END_FUNCTION
    END_NAMESPACE
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0407] Error: access control violation
        ,-[ file:///test0.st:17:37 ]
        |
     10 |             NAMESPACE INTERNAL Deep
        |                                ^^|^
        |                                  `--- namespace is declared INTERNAL here
        |
     17 |             outside_mid := Mid.Deep.f();
        |                                     |
        |                                     `-- can not access 'Lib.Mid.Deep' from INTERNAL namespace
    ----'
    [E0407] Error: access control violation
        ,-[ file:///test0.st:23:21 ]
        |
      3 |         NAMESPACE INTERNAL Impl
        |                            ^^|^
        |                              `--- namespace is declared INTERNAL here
        |
     23 |             VAR x : Lib.Impl.T; END_VAR
        |                     ^^^^^|^^^^
        |                          `------ can not access 'Lib.Impl' from INTERNAL namespace
    ----'
    [E0407] Error: access control violation
        ,-[ file:///test0.st:24:29 ]
        |
      3 |         NAMESPACE INTERNAL Impl
        |                            ^^|^
        |                              `--- namespace is declared INTERNAL here
        |
     24 |             use := Lib.Impl.hidden() + x;
        |                             ^^^|^^
        |                                `---- can not access 'Lib.Impl' from INTERNAL namespace
    ----'
    ");
}

#[rstest]
fn invalid_using_of_an_internal_namespace(mut with_db: RootDatabase) {
    // One report, at the USING; the call through it does not repeat it.
    let source = r#"
    NAMESPACE Lib
        NAMESPACE INTERNAL Impl
            FUNCTION hidden : INT
                hidden := 1;
            END_FUNCTION
        END_NAMESPACE
    END_NAMESPACE

    USING Lib.Impl;
    FUNCTION use : INT
        use := hidden();
    END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0407] Error: access control violation
        ,-[ file:///test0.st:10:11 ]
        |
      3 |         NAMESPACE INTERNAL Impl
        |                            ^^|^
        |                              `--- namespace is declared INTERNAL here
        |
     10 |     USING Lib.Impl;
        |           ^^^^|^^^
        |               `----- can not access 'Lib.Impl' from INTERNAL namespace
    ----'
    ");
}

#[rstest]
fn invalid_access_library_internal_namespace_from_workspace(mut with_db: RootDatabase) {
    // A top-level INTERNAL namespace is reachable across its own origin only:
    // "this library only".
    let lib = r#"
    NAMESPACE INTERNAL LibPriv
        FUNCTION hidden : INT
            hidden := 1;
        END_FUNCTION
    END_NAMESPACE
    "#;
    let workspace = r#"
    FUNCTION use : INT
        use := LibPriv.hidden();
    END_FUNCTION
    "#;
    let rendered =
        crate::tests::utils::test_diagnostics_with_library(&mut with_db, &[lib], &[workspace]);
    assert_snapshot!(rendered, @r"
    [E0407] Error: access control violation
       ,-[ file:///test0.st:3:24 ]
       |
     3 |         use := LibPriv.hidden();
       |                        ^^^|^^
       |                           `---- can not access 'LibPriv' from INTERNAL namespace
       |
       |-[ file:///lib0.st:2:24 ]
       |
     2 |     NAMESPACE INTERNAL LibPriv
       |                        ^^^|^^^
       |                           `----- namespace is declared INTERNAL here
    ---'
    ");
}
