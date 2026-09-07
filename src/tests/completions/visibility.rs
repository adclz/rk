use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::HirNodeInfo;
use ide_proto::{
    handlers::{
        CompletionHandler, CompletionRequest,
        completions_utils::{CompletionCtx, QueryMode},
    },
    walk::completion_descendant_at,
};
use rstest::rstest;

use crate::tests::utils::{add_sources, find_pou_with_name, with_db};

/// What the checker would refuse is not offered: a PRIVATE FUNCTION outside
/// its namespace, and anything inside an INTERNAL namespace from outside the
/// namespace enclosing it. Both are the checker's own boundary, asked before
/// the name is written rather than reported after.
const SRC: &str = r#"
NAMESPACE Lib
    NAMESPACE INTERNAL Impl
        FUNCTION hidden : INT
            hidden := 1;
        END_FUNCTION
    END_NAMESPACE

    FUNCTION PRIVATE helper : INT
        helper := 1;
    END_FUNCTION

    FUNCTION api : INT
        api := 1;
    END_FUNCTION

    FUNCTION inside : INT
        inside := 1;
    END_FUNCTION
END_NAMESPACE

FUNCTION outside : INT
    outside := 1;
END_FUNCTION
"#;

fn offered(db: &RootDatabase, pou_name: &str) -> String {
    let file = *db.get_files().iter().last().unwrap();
    let pou = find_pou_with_name(db, file, pou_name).unwrap();
    let mut ctx = CompletionCtx::new(0, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(db), "", db);
    format!("{:?}", ctx.take_items())
}

#[rstest]
fn a_private_function_is_offered_only_inside_its_namespace(mut with_db: RootDatabase) {
    add_sources(&mut with_db, &[SRC]);

    let from_outside = offered(&with_db, "outside");
    assert!(from_outside.contains("api"), "{from_outside}");
    assert!(
        !from_outside.contains("helper"),
        "PRIVATE stays inside Lib: {from_outside}"
    );

    let from_inside = offered(&with_db, "inside");
    assert!(from_inside.contains("helper"), "{from_inside}");
}

#[rstest]
fn an_internal_namespace_is_offered_only_inside_its_enclosing_one(mut with_db: RootDatabase) {
    add_sources(&mut with_db, &[SRC]);

    let from_outside = offered(&with_db, "outside");
    assert!(
        !from_outside.contains("hidden"),
        "INTERNAL Impl stays inside Lib: {from_outside}"
    );

    let from_inside = offered(&with_db, "inside");
    assert!(from_inside.contains("hidden"), "{from_inside}");
}

/// A method is offered where the checker would accept the call: a PRIVATE
/// one only from its own class, a PROTECTED one from the class and what
/// derives from it, a PUBLIC one anywhere.
#[rstest]
fn a_method_is_offered_where_it_may_be_called(mut with_db: RootDatabase) {
    use auto_lsp::lsp_types::Url;

    let source = r#"
CLASS Motor
    METHOD PUBLIC start : BOOL
    END_METHOD
    METHOD PRIVATE arm : BOOL
    END_METHOD
    METHOD PROTECTED spin : BOOL
    END_METHOD
END_CLASS

FUNCTION outside : INT
VAR
    m : Motor;
END_VAR
    m.
END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);
    let file = with_db
        .get_file(&Url::parse("file:///test0.st").unwrap())
        .unwrap();
    let offset = source.find("    m.\n").unwrap() + "    m.".len();
    let (node, node_key, is_last_before) =
        completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: Some(".".into()),
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let offered = format!("{:?}", node.completion(&with_db, &req).unwrap());
    assert!(offered.contains("start"), "{offered}");
    assert!(
        !offered.contains("arm"),
        "PRIVATE is not offered outside the class: {offered}"
    );
    assert!(
        !offered.contains("spin"),
        "PROTECTED is not offered outside the class: {offered}"
    );
}
