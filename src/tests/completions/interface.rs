use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::AsProtocol;
use rstest::rstest;
use auto_lsp::default::db::BaseDatabase;

use crate::tests::utils::{add_sources, with_db};


// interfaces can only have methods
#[rstest]
pub fn interface_completions(mut with_db: RootDatabase) {
    let source = r#"INTERFACE in1 

END_INTERFACE"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // 15 = middle
    let in_variables = sema.descendant_at(&with_db, 15).unwrap();
    let completions = in_variables.as_proto().completion(&with_db, 19).unwrap();

    assert!(format!("{completions:?}").contains("METHOD"));
}
