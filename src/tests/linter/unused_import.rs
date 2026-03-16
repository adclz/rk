use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_lint_diagnostics, with_db};

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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, sources), @r"
    [W0109] Warning: unused import
       ,-[ file:///test1.st:3:11 ]
       |
     3 |     USING Tools;
       |           ^^|^^
       |             `---- unused import 'Tools'
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, sources), @r"");
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, sources), @r"
    [W0109] Warning: unused import
       ,-[ file:///test1.st:4:11 ]
       |
     4 |     USING Utils;
       |           ^^|^^
       |             `---- unused import 'Utils'
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, sources), @r"");
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, sources), @r"");
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, sources), @r"");
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, sources), @r"
    [W0109] Warning: unused import
       ,-[ file:///test1.st:2:7 ]
       |
     2 | USING Tools;
       |       ^^|^^
       |         `---- unused import 'Tools'
    ---'
    ");
}
