use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn unused_using_directive(mut with_db: RootDatabase) {
    let sources = &[
        r#"
NAMESPACE Tools
    FUNCTION_BLOCK Logger
    END_FUNCTION_BLOCK
END_NAMESPACE
"#,
        r#"
FUNCTION_BLOCK fb1
    USING Tools;
END_FUNCTION_BLOCK
"#,
    ];
    assert_snapshot!(test_single_lint(&mut with_db, sources, "unused-import"), @r"
    [L0301] Hint: unused import
       ,-[ file:///test1.st:3:11 ]
       |
     3 |     USING Tools;
       |           ^^|^^
       |             `---- unused import 'Tools'
       |
       | Note: lint rule: unused-import
    ---'
    ");
}

#[rstest]
fn used_using_directive_in_body(mut with_db: RootDatabase) {
    let sources = &[
        r#"
NAMESPACE Tools
    FUNCTION helper : INT
        helper := 0;
    END_FUNCTION
END_NAMESPACE
"#,
        r#"
FUNCTION test : INT
    USING Tools;
    test := helper();
END_FUNCTION
"#,
    ];
    assert_snapshot!(test_single_lint(&mut with_db, sources, "unused-import"), @r"");
}

#[rstest]
fn multiple_usings_one_unused(mut with_db: RootDatabase) {
    let sources = &[
        r#"
NAMESPACE Tools
    FUNCTION helper : INT
        helper := 0;
    END_FUNCTION
END_NAMESPACE

NAMESPACE Utils
    FUNCTION_BLOCK Helper
    END_FUNCTION_BLOCK
END_NAMESPACE
"#,
        r#"
FUNCTION test : INT
    USING Tools;
    USING Utils;
    test := helper();
END_FUNCTION
"#,
    ];
    assert_snapshot!(test_single_lint(&mut with_db, sources, "unused-import"), @r"
    [E0205] Error: multiple items in scope
       ,-[ file:///test1.st:5:13 ]
       |
     5 |     test := helper();
       |             ^^^|^^
       |                `---- multiple items named 'helper' available in scope
       |
       | Note: qualify the name to resolve the ambiguity: Tools.helper or Utils.helper
    ---'
    [L0301] Hint: unused import
       ,-[ file:///test1.st:3:11 ]
       |
     3 |     USING Tools;
       |           ^^|^^
       |             `---- unused import 'Tools'
       |
       | Note: lint rule: unused-import
    ---'
    [L0301] Hint: unused import
       ,-[ file:///test1.st:4:11 ]
       |
     4 |     USING Utils;
       |           ^^|^^
       |             `---- unused import 'Utils'
       |
       | Note: lint rule: unused-import
    ---'
    ");
}

#[rstest]
fn no_usings_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-import"), @r"");
}

#[rstest]
fn used_using_directive_in_head_only(mut with_db: RootDatabase) {
    let sources = &[
        r#"
NAMESPACE Tools
    FUNCTION_BLOCK Logger
    END_FUNCTION_BLOCK
END_NAMESPACE
"#,
        r#"
FUNCTION_BLOCK fb1
    USING Tools;
VAR
    _log : Logger;
END_VAR
END_FUNCTION_BLOCK
"#,
    ];
    // USING is used only in variable type spec (head-level), should not warn
    assert_snapshot!(test_single_lint(&mut with_db, sources, "unused-import"), @"");
}

#[rstest]
fn global_using_used_in_head(mut with_db: RootDatabase) {
    let sources = &[
        r#"
NAMESPACE Tools
    FUNCTION_BLOCK Logger
    END_FUNCTION_BLOCK
END_NAMESPACE
"#,
        r#"
USING Tools;

FUNCTION_BLOCK fb1
VAR
    _log : Logger;
END_VAR
END_FUNCTION_BLOCK
"#,
    ];
    // Global USING used in a type spec (head-level) should not warn
    assert_snapshot!(test_single_lint(&mut with_db, sources, "unused-import"), @"");
}

#[rstest]
fn global_using_used_by_pou_in_namespace(mut with_db: RootDatabase) {
    let sources = &[
        r#"
NAMESPACE Tools
    FUNCTION_BLOCK Logger
    END_FUNCTION_BLOCK
END_NAMESPACE
"#,
        r#"
USING Tools;

NAMESPACE MyApp
    FUNCTION_BLOCK Controller
    VAR
        _log : Logger;
    END_VAR
    END_FUNCTION_BLOCK
END_NAMESPACE
"#,
    ];
    // Global USING used by a POU inside a namespace should not warn
    assert_snapshot!(test_single_lint(&mut with_db, sources, "unused-import"), @"");
}

#[rstest]
fn global_using_unused(mut with_db: RootDatabase) {
    let sources = &[
        r#"
NAMESPACE Tools
    FUNCTION_BLOCK Logger
    END_FUNCTION_BLOCK
END_NAMESPACE
"#,
        r#"
USING Tools;

FUNCTION test : INT
    test := 0;
END_FUNCTION
"#,
    ];
    assert_snapshot!(test_single_lint(&mut with_db, sources, "unused-import"), @r"
    [L0301] Hint: unused import
       ,-[ file:///test1.st:2:7 ]
       |
     2 | USING Tools;
       |       ^^|^^
       |         `---- unused import 'Tools'
       |
       | Note: lint rule: unused-import
    ---'
    ");
}

/// A `USING` written directly under `NAMESPACE`, where the namespace's own
/// POUs share it.
///
/// Only the POUs' own usings were collected, so one written at the namespace
/// level was never a candidate and could never be reported, however unused.
#[rstest]
fn unused_using_directive_in_a_namespace(mut with_db: RootDatabase) {
    let sources = &[
        r#"
NAMESPACE Tools
    FUNCTION_BLOCK Logger
    END_FUNCTION_BLOCK
END_NAMESPACE
"#,
        r#"
NAMESPACE App
    USING Tools;

    FUNCTION test : INT
        test := 0;
    END_FUNCTION
END_NAMESPACE
"#,
    ];
    assert_snapshot!(test_single_lint(&mut with_db, sources, "unused-import"), @r"
    [L0301] Hint: unused import
       ,-[ file:///test1.st:3:11 ]
       |
     3 |     USING Tools;
       |           ^^|^^
       |             `---- unused import 'Tools'
       |
       | Note: lint rule: unused-import
    ---'
    ");
}

/// The namespace's `USING` is what lets its POUs name `helper`, so it is
/// used, whichever POU does the naming.
#[rstest]
fn used_using_directive_in_a_namespace(mut with_db: RootDatabase) {
    let sources = &[
        r#"
NAMESPACE Tools
    FUNCTION helper : INT
        helper := 0;
    END_FUNCTION
END_NAMESPACE
"#,
        r#"
NAMESPACE App
    USING Tools;

    FUNCTION test : INT
        test := helper();
    END_FUNCTION
END_NAMESPACE
"#,
    ];
    assert_snapshot!(test_single_lint(&mut with_db, sources, "unused-import"), @"");
}

/// A namespace holding nested ones is visited once, so its unused `USING` is
/// reported once. `SemanticIndex::namespaces` is flat and already holds every
/// nested namespace; collecting from both would report each one twice.
#[rstest]
fn an_unused_namespace_using_is_reported_once(mut with_db: RootDatabase) {
    let sources = &[
        r#"
NAMESPACE Tools
    FUNCTION_BLOCK Logger
    END_FUNCTION_BLOCK
END_NAMESPACE
"#,
        r#"
NAMESPACE Outer
    USING Tools;

    NAMESPACE Inner
        FUNCTION test : INT
            test := 0;
        END_FUNCTION
    END_NAMESPACE
END_NAMESPACE
"#,
    ];
    assert_snapshot!(test_single_lint(&mut with_db, sources, "unused-import"), @r"
    [L0301] Hint: unused import
       ,-[ file:///test1.st:3:11 ]
       |
     3 |     USING Tools;
       |           ^^|^^
       |             `---- unused import 'Tools'
       |
       | Note: lint rule: unused-import
    ---'
    ");
}
