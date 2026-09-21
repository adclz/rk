use db::RootDatabase;
use ide_proto::{handlers::call_hierarchy, walk::descendant_at};
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_source, with_db};

/// The hierarchy at the first occurrence of `needle`, rendered as
/// `<name> <kind>` lines with one indented line per call site.
fn hierarchy(db: &mut RootDatabase, source: &str, needle: &str, incoming: bool) -> String {
    let file = add_source(db, source);
    let offset = source
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in the source"));
    let node = descendant_at(db, file, offset).expect("a node at the cursor");

    let mut out = String::new();
    match incoming {
        true => {
            for call in call_hierarchy::incoming(db, &node).expect("a callable at the cursor") {
                out.push_str(&format!("{} {:?}\n", call.from.name, call.from.kind));
                for range in &call.from_ranges {
                    out.push_str(&format!("    calls at line {}\n", range.start.line));
                }
            }
        }
        false => {
            for call in call_hierarchy::outgoing(db, &node).expect("a callable at the cursor") {
                out.push_str(&format!("{} {:?}\n", call.to.name, call.to.kind));
                for range in &call.from_ranges {
                    out.push_str(&format!("    called at line {}\n", range.start.line));
                }
            }
        }
    }
    match out.is_empty() {
        true => "<none>".to_string(),
        false => out,
    }
}

const WORKSPACE: &str = r#"
NAMESPACE App
    FUNCTION leaf : INT
        leaf := 1;
    END_FUNCTION

    FUNCTION_BLOCK Timer
    VAR_INPUT pt : INT; END_VAR
    VAR_OUTPUT q : INT; END_VAR
        q := pt;
    END_FUNCTION_BLOCK

    CLASS Motor
    VAR t : Timer; END_VAR
        METHOD PUBLIC Spin : INT
            Spin := leaf();
            t(pt := 1);
        END_METHOD
    END_CLASS

    FUNCTION middle : INT
    VAR m : Motor; END_VAR
        middle := leaf() + leaf();
        middle := middle + m.Spin();
    END_FUNCTION

    FUNCTION top : INT
        top := middle();
    END_FUNCTION
END_NAMESPACE"#;

/// Two calls to one callee are one entry with two sites, not two entries.
#[rstest]
fn outgoing_groups_the_sites_of_one_callee(mut with_db: RootDatabase) {
    assert_snapshot!(hierarchy(&mut with_db, WORKSPACE, "FUNCTION middle", false), @r"
    Spin Method
        called at line 23
    leaf Function
        called at line 22
        called at line 22
    ");
}

/// Every body that names it, across the workspace: a FUNCTION and a METHOD
/// answer the same way, because a method's body is a scope like any other.
#[rstest]
fn incoming_finds_every_caller(mut with_db: RootDatabase) {
    assert_snapshot!(hierarchy(&mut with_db, WORKSPACE, "FUNCTION leaf", true), @r"
    Spin Method
        calls at line 15
    middle Function
        calls at line 22
        calls at line 22
    ");
}

/// Invoking an FB instance is a call to the BLOCK: the hierarchy is what runs,
/// not what the instance is named.
#[rstest]
fn an_instance_invocation_is_a_call_to_its_block(mut with_db: RootDatabase) {
    assert_snapshot!(hierarchy(&mut with_db, WORKSPACE, "FUNCTION_BLOCK Timer", true), @r"
    Spin Method
        calls at line 16
    ");
}

/// The cursor may sit on a CALL rather than a declaration; the hierarchy is
/// the callee's either way.
#[rstest]
fn a_call_site_opens_the_callee_hierarchy(mut with_db: RootDatabase) {
    assert_snapshot!(hierarchy(&mut with_db, WORKSPACE, "middle()", true), @r"
    top Function
        calls at line 27
    ");
}

/// A leaf calls nothing, and says so rather than failing.
#[rstest]
fn a_leaf_has_no_outgoing_calls(mut with_db: RootDatabase) {
    assert_snapshot!(hierarchy(&mut with_db, WORKSPACE, "FUNCTION leaf", false), @"<none>");
}
