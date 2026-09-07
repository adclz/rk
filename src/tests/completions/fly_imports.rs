use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::HirNodeInfo;
use ide_proto::handlers::completions_utils::{CompletionCtx, QueryMode};
use rstest::rstest;

use crate::tests::utils::{add_sources, find_pou_with_name, with_db};

#[rstest]
pub fn using_for_item_not_in_scope(mut with_db: RootDatabase) {
    let source = r#"NAMESPACE ns
	FUNCTION fn1

	END_FUNCTION
END_NAMESPACE

FUNCTION fn2

END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou =
        find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "fn2").unwrap();

    let mut ctx = CompletionCtx::new(70, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
    let completions = ctx.take_items();

    assert_eq!(completions.len(), 3); // fn1, fn2, and ns namespace
    assert!(format!("{completions:?}").contains("fn1"));
    assert!(format!("{completions:?}").contains("fn2"));

    // fn1 should have an additional namespace prefix (check for the escaped newline in debug output)
    assert!(format!("{completions:?}").contains(r#""USING ns;\n\n""#));
}

#[rstest]
pub fn using_already_in_scope(mut with_db: RootDatabase) {
    let source = r#"USING ns;
NAMESPACE ns
	FUNCTION fn1

	END_FUNCTION
END_NAMESPACE

FUNCTION fn2

END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou =
        find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "fn2").unwrap();

    let mut ctx = CompletionCtx::new(80, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
    let completions = ctx.take_items();

    assert_eq!(completions.len(), 3); // fn1, fn2, and ns namespace
    assert!(format!("{completions:?}").contains("fn1"));
    assert!(format!("{completions:?}").contains("fn2"));

    // fn1 should *not* have an additional namespace prefix
    assert!(!format!("{completions:?}").contains("USING ns;\n"));
}

#[rstest]
pub fn deduplicate_using_with_multiple_candidates(mut with_db: RootDatabase) {
    let source = r#"USING ns;
NAMESPACE ns
	FUNCTION fn1

	END_FUNCTION

    FUNCTION fn1 // there should only be one USING ns;

	END_FUNCTION
END_NAMESPACE

FUNCTION_BLOCK fb2

END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let pou =
        find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "fb2").unwrap();

    let mut ctx = CompletionCtx::new(147, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
    let completions = ctx.take_items();

    assert_eq!(completions.len(), 2); // fn1 and ns namespace
    assert!(format!("{completions:?}").contains("fn1"));

    // fn1 should *not* have an additional namespace prefix
    assert!(!format!("{completions:?}").contains("USING ns;\n"));
}

/// A `{test}` FUNCTION is what the runner calls. Completing it inside
/// production code would suggest calling it by hand, so it is offered only
/// from another test; the plain function beside it is offered to both.
#[rstest]
pub fn a_test_is_offered_only_to_another_test(mut with_db: RootDatabase) {
    let source = r#"NAMESPACE ns
	{test}
	FUNCTION test_it
	END_FUNCTION

	FUNCTION helper
	END_FUNCTION
END_NAMESPACE

FUNCTION production
END_FUNCTION

{test}
FUNCTION test_other
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let production = find_pou_with_name(&with_db, file, "production").unwrap();
    let mut ctx = CompletionCtx::new(0, QueryMode::Body);
    ctx.scope_completion(production.get_scope_id(&with_db), "", &with_db);
    let offered = format!("{:?}", ctx.take_items());
    assert!(
        offered.contains("helper"),
        "the plain function is offered: {offered}"
    );
    assert!(
        !offered.contains("test_it") && !offered.contains("test_other"),
        "no test is offered to production code: {offered}"
    );

    let test = find_pou_with_name(&with_db, file, "test_other").unwrap();
    let mut ctx = CompletionCtx::new(0, QueryMode::Body);
    ctx.scope_completion(test.get_scope_id(&with_db), "", &with_db);
    let offered = format!("{:?}", ctx.take_items());
    assert!(
        offered.contains("test_it"),
        "a test is offered to another test: {offered}"
    );
    assert!(offered.contains("helper"), "{offered}");
}
