use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// Two fragments in ONE file: they merge, but nothing is being separated, so
/// the split serves no purpose.
#[rstest]
fn same_configuration_twice_in_one_file(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION Plant
    VAR_GLOBAL
        a : INT;
    END_VAR
END_CONFIGURATION

CONFIGURATION Plant
    VAR_GLOBAL
        b : INT;
    END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(
        test_single_lint(&mut with_db, &[source], "duplicate-configuration"),
        @r"
    [L0205] Info: duplicate configuration in same file
       ,-[ file:///test0.st:8:15 ]
       |
     2 | CONFIGURATION Plant
       |               ^^|^^
       |                 `---- first declaration of 'Plant' here
       |
     8 | CONFIGURATION Plant
       |               ^^|^^
       |                 `---- CONFIGURATION 'Plant' is declared multiple times in this file, consider merging
       |
       | Note: lint rule: duplicate-configuration
    ---'
    "
    );
}

/// Fragments in SEPARATE files are the point of merging — the GVL model. No
/// lint: the lint is about a split with nothing on either side of it.
#[rstest]
fn fragments_in_separate_files_are_not_linted(mut with_db: RootDatabase) {
    let globals = r#"
CONFIGURATION Plant
    VAR_GLOBAL
        a : INT;
    END_VAR
END_CONFIGURATION
"#;
    let machine = r#"
PROGRAM P VAR n : INT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Plant
    RESOURCE Main ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(
        test_single_lint(&mut with_db, &[globals, machine], "duplicate-configuration"),
        @r""
    );
}

/// Fires whatever the fragments hold: globals in one and resources in the
/// other still separate nothing while they share a file.
#[rstest]
fn fragments_in_one_file_are_linted_whatever_they_hold(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM P VAR n : INT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Plant
    VAR_GLOBAL
        a : INT;
    END_VAR
END_CONFIGURATION

CONFIGURATION Plant
    RESOURCE Main ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(
        test_single_lint(&mut with_db, &[source], "duplicate-configuration"),
        @r"
    [L0205] Info: duplicate configuration in same file
        ,-[ file:///test0.st:10:15 ]
        |
      4 | CONFIGURATION Plant
        |               ^^|^^
        |                 `---- first declaration of 'Plant' here
        |
     10 | CONFIGURATION Plant
        |               ^^|^^
        |                 `---- CONFIGURATION 'Plant' is declared multiple times in this file, consider merging
        |
        | Note: lint rule: duplicate-configuration
    ----'
    "
    );
}
