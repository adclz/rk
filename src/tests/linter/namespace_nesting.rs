use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// Every lint runs once per POU, however deeply the POU is nested.
///
/// `SemanticIndex::namespaces` is flat and already holds every nested
/// namespace, and the walk descended into them as well, so a POU was linted
/// once per ancestor: the standard library reported 156 of its 329
/// diagnostics twice, all of them inside `NAMESPACE Std.X / NAMESPACE Test`.
#[rstest]
fn a_lint_inside_a_nested_namespace_is_reported_once(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Outer
    NAMESPACE Inner
        FUNCTION deep : INT
        VAR
            n : INT;
        END_VAR
            n := n;
            deep := n;
        END_FUNCTION
    END_NAMESPACE
END_NAMESPACE
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-assignment"), @r"
    [L0309] Warning: self-assignment
       ,-[ file:///test0.st:8:13 ]
       |
     8 |             n := n;
       |             ^^^|^^
       |                `---- variable 'n' is assigned to itself
       |
       | Note: lint rule: self-assignment
    ---'
    ");
}

/// Three levels deep is still once, not three times.
#[rstest]
fn nesting_depth_does_not_multiply_a_lint(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE A
    NAMESPACE B
        NAMESPACE C
            FUNCTION deeper : INT
            VAR
                n : INT;
            END_VAR
                n := n;
                deeper := n;
            END_FUNCTION
        END_NAMESPACE
    END_NAMESPACE
END_NAMESPACE
"#;
    let out = test_single_lint(&mut with_db, &[source], "self-assignment");
    assert_eq!(out.matches("[L0309]").count(), 1, "reported once:\n{out}");
}
