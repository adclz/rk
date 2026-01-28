use auto_lsp::core::semantic_tokens_builder::SemanticTokensBuilder;
use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::CLASS;
use ide_proto::FUNCTION;
use ide_proto::INTERFACE;
use ide_proto::SUPPORTED_TYPES;
use ide_proto::walk::WalkHir;
use rstest::rstest;

use crate::tests::utils::add_sources;
use crate::tests::utils::with_db;

#[rstest]
pub fn pou_tokens_in_variable_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb0

END_FUNCTION_BLOCK

CLASS cl0

END_CLASS

FUNCTION_BLOCK fb1
    VAR
        test: fb0; // highlight fb0
        test2: cl0; // highlight cl0
    END_VAR

END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut builder = SemanticTokensBuilder::new("".into());

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut builder);
        std::ops::ControlFlow::Continue(())
    });

    let result = builder.build();

    // fb0 and cl0 should be highlighted in variable types
    assert_eq!(result.data[0].token_type, SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32);
    assert_eq!(result.data[1].token_type, SUPPORTED_TYPES.iter().position(|x| *x == CLASS).unwrap() as u32);
}

#[rstest]
pub fn pou_tokens_resolved_path(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb0

END_FUNCTION_BLOCK

CLASS cl0

END_CLASS

FUNCTION_BLOCK fb1
    VAR
        test: fb0; // highlight fb0
        test2: cl0; // highlight cl0
    END_VAR


    test; // highlight fb0
    test2;  // highlight cl0

END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut builder = SemanticTokensBuilder::new("".into());

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut builder);
        std::ops::ControlFlow::Continue(())
    });

    let result = builder.build();

    // fb0 and cl0 should be highlighted in variable types (twice each)
    assert_eq!(result.data[0].token_type, SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32);
    assert_eq!(result.data[1].token_type, SUPPORTED_TYPES.iter().position(|x| *x == CLASS).unwrap() as u32);
    assert_eq!(result.data[2].token_type, SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32);
    assert_eq!(result.data[3].token_type, SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32);
    assert_eq!(result.data[4].token_type, SUPPORTED_TYPES.iter().position(|x| *x == CLASS).unwrap() as u32);
    assert_eq!(result.data[5].token_type, SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32);
    assert_eq!(result.data[6].token_type, SUPPORTED_TYPES.iter().position(|x| *x == CLASS).unwrap() as u32);

}


#[rstest]
pub fn class_and_fb_tokens_in_extends(mut with_db: RootDatabase) {
    let source = r#"
CLASS cl0

END_CLASS

FUNCTION_BLOCK fb0 EXTENDS cl0 // highlight cl0

END_FUNCTION_BLOCK

FUNCTION_BLOCK fb1 EXTENDS fb1 // highlight fb1

END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut builder = SemanticTokensBuilder::new("".into());

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut builder);
        std::ops::ControlFlow::Continue(())
    });

    let result = builder.build();

    // fb0 and cl0 should be highlighted in variable types
    assert_eq!(result.data[0].token_type, SUPPORTED_TYPES.iter().position(|x| *x == CLASS).unwrap() as u32);
    assert_eq!(result.data[1].token_type, SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32);
}

#[rstest]
pub fn class_and_fb_tokens_in_implements(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE in0 END_INTERFACE
INTERFACE in1 END_INTERFACE
INTERFACE in2 END_INTERFACE
INTERFACE in3 END_INTERFACE
INTERFACE in4 END_INTERFACE
INTERFACE in5 END_INTERFACE

FUNCTION_BLOCK fb0 MPLEMENTS in0, in1, in2 // highlight in0, in1, in2

END_FUNCTION_BLOCK

FUNCTION_BLOCK fb1 IMPLEMENTS in3, in4, in5 // highlight in3, in4, in5

END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut builder = SemanticTokensBuilder::new("".into());

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut builder);
        std::ops::ControlFlow::Continue(())
    });

    let result = builder.build();

    assert_eq!(result.data[0].token_type, SUPPORTED_TYPES.iter().position(|x| *x == INTERFACE).unwrap() as u32);
    assert_eq!(result.data[1].token_type, SUPPORTED_TYPES.iter().position(|x| *x == INTERFACE).unwrap() as u32);
    assert_eq!(result.data[2].token_type, SUPPORTED_TYPES.iter().position(|x| *x == INTERFACE).unwrap() as u32);
    assert_eq!(result.data[3].token_type, SUPPORTED_TYPES.iter().position(|x| *x == INTERFACE).unwrap() as u32);
    assert_eq!(result.data[4].token_type, SUPPORTED_TYPES.iter().position(|x| *x == INTERFACE).unwrap() as u32);
    assert_eq!(result.data[5].token_type, SUPPORTED_TYPES.iter().position(|x| *x == INTERFACE).unwrap() as u32);
}

