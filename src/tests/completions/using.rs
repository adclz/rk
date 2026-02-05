use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::{hir_def::{semantic_index::semantic_index}};
use ide_proto::handlers::{CompletionHandler};
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

    let completions =  using.completion(&with_db, 6).unwrap();

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

    let completions =  using.completion(&with_db, 6).unwrap();

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

    let completions =  using.completion(&with_db, 6).unwrap();

    assert_eq!(completions.len(), 2);
    assert!(format!("{completions:?}").contains("subsystem1"));
    assert!(format!("{completions:?}").contains("subsystem2"));
}