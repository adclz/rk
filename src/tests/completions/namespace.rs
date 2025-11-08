use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::AsProtocol;
use rstest::rstest;
use auto_lsp::default::db::BaseDatabase;

use crate::tests::utils::{add_sources, with_db};

// Namespace completions
#[rstest]
pub fn cursor_before_class_variables(mut with_db: RootDatabase) {
    let source = r#"NAMESPACE ns    
    
END_NAMESPACE"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // 21 = middle
    let in_variables = sema.descendant_at(&with_db, 21).unwrap();
    let completions = in_variables.as_proto().completion(&with_db, 21).unwrap();

    assert!(format!("{completions:?}").contains("NAMESPACE"));
    assert!(format!("{completions:?}").contains("INTERFACE"));
    assert!(format!("{completions:?}").contains("FUNCTION"));
    assert!(format!("{completions:?}").contains("FUNCTION_BLOCK"));
    assert!(format!("{completions:?}").contains("TYPE"));
    assert!(format!("{completions:?}").contains("CLASS"));
}
