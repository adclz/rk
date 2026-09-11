use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// Reading a config VAR_GLOBAL directly by name (type-faithful), without a
/// VAR_EXTERNAL declaration, resolves but warns.
#[rstest]
fn direct_global_access_warns(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Main
VAR
    local : INT;
END_VAR
    local := g_counter;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL
    g_counter : INT;
END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "global-without-external"), @r"
    [L0118] Warning: global accessed without VAR_EXTERNAL
        ,-[ file:///test0.st:6:14 ]
        |
      6 |     local := g_counter;
        |              ^^^^|^^^^
        |                  `------ global 'g_counter' is accessed without a VAR_EXTERNAL declaration
        |
     11 |     g_counter : INT;
        |     ^^^^|^^^^
        |         `------ global 'g_counter' is declared here
        |
        | Note: lint rule: global-without-external
    ----'
    ");
}

/// Importing the global via VAR_EXTERNAL is the IEC-compliant way: no warning.
#[rstest]
fn external_declaration_no_warn(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Main
VAR_EXTERNAL
    g_counter : INT;
END_VAR
VAR
    local : INT;
END_VAR
    local := g_counter;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL
    g_counter : INT;
END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "global-without-external"), @r"");
}
