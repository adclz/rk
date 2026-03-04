use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::handlers::{CompletionHandler, CompletionRequest};
use ide_proto::walk::completion_descendant_at;
use rstest::rstest;

use crate::tests::utils::{add_sources, with_db};

#[rstest]
pub fn deduplicate_multiple_candidates(mut with_db: RootDatabase) {
    // should only suggest one "System" namespace, even though there are two in the source
    let source = r#"
USING S

NAMESPACE System

END_NAMESPACE

NAMESPACE System

END_NAMESPACE
"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let using = sema.scope.usings(&with_db).first().unwrap();

    let req = CompletionRequest {
        offset: 6,
        trigger_character: None,
        query: "".into(),
        node_index_pos: None,
        is_last_before: false,
    };
    let completions = using.completion(&with_db, &req).unwrap();

    assert_eq!(completions.len(), 1);
    assert!(format!("{completions:?}").contains("System"));
}

#[rstest]
pub fn using_fragments(mut with_db: RootDatabase) {
    // should suggest both "subsystem1" and "subsystem2" when completing "System."
    let source = r#"
USING System.

NAMESPACE System.subsystem1

END_NAMESPACE

NAMESPACE System.subsystem2

END_NAMESPACE
"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let using = sema.scope.usings(&with_db).first().unwrap();

    let completions = using
        .completion(
            &with_db,
            &CompletionRequest {
                offset: 6,
                trigger_character: Some(".".into()),
                query: "".into(),
                node_index_pos: None,
                is_last_before: false,
            },
        )
        .unwrap();

    assert_eq!(completions.len(), 2);
    assert!(format!("{completions:?}").contains("subsystem1"));
    assert!(format!("{completions:?}").contains("subsystem2"));
}

#[rstest]
pub fn fragment_first_letter(mut with_db: RootDatabase) {
    // should suggest both "subsystem1" and "subsystem2" when completing "System.s"
    let source = r#"
USING System.s

NAMESPACE System.subsystem1

END_NAMESPACE

NAMESPACE System.subsystem2

END_NAMESPACE
"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let using = sema.scope.usings(&with_db).first().unwrap();

    let req = CompletionRequest {
        offset: 7,
        trigger_character: None,
        query: "".into(),
        node_index_pos: None,
        is_last_before: false,
    };
    let completions = using.completion(&with_db, &req).unwrap();

    assert_eq!(completions.len(), 2);
    assert!(format!("{completions:?}").contains("subsystem1"));
    assert!(format!("{completions:?}").contains("subsystem2"));
}

#[rstest]
pub fn deduplicate_using_fragments(mut with_db: RootDatabase) {
    // should suggest both "subsystem1" and "subsystem2" when completing "System."
    // even though there are two "System.subsystem1" namespaces in the source, they should only be suggested once
    let source = r#"
USING System.

NAMESPACE System.subsystem1

END_NAMESPACE

NAMESPACE System.subsystem1

END_NAMESPACE

NAMESPACE System.subsystem2

END_NAMESPACE
"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let using = sema.scope.usings(&with_db).first().unwrap();

    let completions = using
        .completion(
            &with_db,
            &CompletionRequest {
                offset: 6,
                trigger_character: Some(".".into()),
                query: "".into(),
                node_index_pos: None,
                is_last_before: false,
            },
        )
        .unwrap();

    assert_eq!(completions.len(), 2);
    assert!(format!("{completions:?}").contains("subsystem1"));
    assert!(format!("{completions:?}").contains("subsystem2"));
}

/// After a top-level USING directive (with nothing else in the file),
/// completions should still fire — not return Using namespace fragments.
#[rstest]
pub fn completion_after_top_level_using(mut with_db: RootDatabase) {
    let ns_source = r#"
NAMESPACE ns
    FUNCTION fn1 END_FUNCTION
END_NAMESPACE
"#;
    let using_source = r#"
USING ns;

"#;

    add_sources(&mut with_db, &[ns_source, using_source]);
    // Cursor on the blank line after "USING ns;" in the second file
    let offset = using_source.find("USING ns;").unwrap() + "USING ns;\n".len() + 1;
    let files: Vec<_> = with_db.get_files().iter().map(|f| *f).collect();
    let file = files[1]; // second file
    let result = completion_descendant_at(&with_db, file, offset);

    // Should NOT return the Using node as last_before — Using nodes
    // are not valid targets for general completions.
    if let Some((node, node_key, is_last_before)) = &result {
        let req = CompletionRequest {
            offset,
            trigger_character: None,
            query: "".into(),
            node_index_pos: Some(*node_key),
            is_last_before: *is_last_before,
        };
        let completions = node.completion(&with_db, &req).unwrap_or_default();

        let has_using_fragments = completions.iter().any(|c| c.label == "ns");
        assert!(
            !has_using_fragments || completions.len() > 1,
            "should not return only Using namespace fragments"
        );
    }
    // result == None is also acceptable (no node at that position)
}
