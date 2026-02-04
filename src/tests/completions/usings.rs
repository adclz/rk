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

    assert_eq!(completions.len(), 2); // fn1 and fn2
    assert!(format!("{completions:?}").contains("fn1"));
    assert!(format!("{completions:?}").contains("fn2"));

    // fn1 should have an additional namespace prefix (check for the escaped newline in debug output)
    assert!(format!("{completions:?}").contains(r#""USING ns;\n""#));
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

    assert_eq!(completions.len(), 2); // fn1 and fn2
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

    assert_eq!(completions.len(), 1); // fn1
    assert!(format!("{completions:?}").contains("fn1"));

    // fn1 should *not* have an additional namespace prefix
    assert!(!format!("{completions:?}").contains("USING ns;\n"));
}
