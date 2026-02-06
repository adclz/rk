use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::HirNodeInfo;
use ide_proto::handlers::completions_utils::{CompletionCtx, QueryMode};
use rstest::rstest;

use crate::tests::utils::{add_sources, find_pou_with_name, with_db};

/// Test completion WITHIN a variable type declaration (head scope)
#[rstest]
pub fn head_scope_items(mut with_db: RootDatabase) {
    let source = r#"
TYPE Test: INT; END_TYPE

FUNCTION test_fn
    VAR
        my_var: INT;
        another_var: T
    END_VAR

    my_var := 10;
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou = find_pou_with_name(
        &with_db,
        *with_db.get_files().iter().last().unwrap(),
        "test_fn",
    )
    .unwrap();

    let offset = source.find("another_var").unwrap() + 1;
    let mut ctx = CompletionCtx::new(offset, QueryMode::Head);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
    let completions = ctx.take_items();

    // only Test should be suggested
    assert_eq!(completions.len(), 1);
    assert!(format!("{completions:?}").contains("Test"));
}
