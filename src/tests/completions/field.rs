use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use ide_proto::{handlers::{CompletionHandler, CompletionRequest}, walk::{descendant_at, completion_descendant_at}};
use rstest::rstest;

use crate::tests::utils::{add_sources, with_db};

#[rstest]
pub fn struct_field_completion(mut with_db: RootDatabase) {
    let source = r#"
TYPE Engine :
    STRUCT
        oil : REAL;
        fuel: INT;
    END_STRUCT
END_TYPE


FUNCTION_BLOCK fb
    VAR
        my_var: Engine;
    END_VAR

    my_var.
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let path_expr =
        descendant_at(&with_db, *with_db.get_files().iter().last().unwrap(), 162).unwrap();
    let req = CompletionRequest { offset: 162, trigger_character: None, query: "".into(), node_index_pos: None, is_last_before: false };
    let completions = path_expr
        .completion(&with_db, &req)
        .unwrap();

    assert_eq!(completions.len(), 2);
    assert!(format!("{completions:?}").contains("oil"));
    assert!(format!("{completions:?}").contains("fuel"));
}

#[rstest]
pub fn nested_struct_field_completion(mut with_db: RootDatabase) {
    let source = r#"
TYPE Inner :
    STRUCT
        depth : INT;
        width : REAL;
    END_STRUCT
END_TYPE

TYPE Outer :
    STRUCT
        inner : Inner;
        name : INT;
    END_STRUCT
END_TYPE

FUNCTION_BLOCK fb
    VAR
        my_var: Outer;
    END_VAR

    my_var.inner.
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    // Offset right after the trailing dot — no node contains this position,
    // so completion_descendant_at falls back to the closest preceding node.
    let offset = source.find("my_var.inner.").unwrap() + "my_var.inner.".len();
    let file = *with_db.get_files().iter().last().unwrap();
    let (path_expr, node_key, is_last_before) = completion_descendant_at(&with_db, file, offset).unwrap();
    let req = CompletionRequest { offset, trigger_character: Some(".".into()), query: "".into(), node_index_pos: Some(node_key), is_last_before };
    let completions = path_expr
        .completion(&with_db, &req)
        .unwrap();

    // Should show fields of Inner (depth, width), NOT fields of Outer (inner, name)
    assert!(format!("{completions:?}").contains("depth"), "expected 'depth' in completions: {completions:?}");
    assert!(format!("{completions:?}").contains("width"), "expected 'width' in completions: {completions:?}");
    assert!(!format!("{completions:?}").contains("name"), "should NOT contain 'name' from Outer: {completions:?}");
}

#[rstest]
pub fn class_var_and_methods_completion(mut with_db: RootDatabase) {
    let source = r#"
CLASS Engine
    VAR
        oil : REAL;
        fuel: INT;
    END_VAR

    METHOD Start
    END_METHOD

END_CLASS

FUNCTION_BLOCK fb
    VAR
        my_var: Engine;
    END_VAR

    my_var.
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let path_expr =
        descendant_at(&with_db, *with_db.get_files().iter().last().unwrap(), 191).unwrap();
    let req = CompletionRequest { offset: 191, trigger_character: None, query: "".into(), node_index_pos: None, is_last_before: false };
    let completions = path_expr
        .completion(&with_db, &req)
        .unwrap();

    assert_eq!(completions.len(), 3);
    assert!(format!("{completions:?}").contains("oil"));
    assert!(format!("{completions:?}").contains("fuel"));
    assert!(format!("{completions:?}").contains("Start()"));
}

#[rstest]
pub fn enum_variants_completion(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    List: UINT (A, B, C);
END_TYPE

FUNCTION fn1
    VAR
        test: List;
    END_VAR

    test := List#

END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let expr = descendant_at(&with_db, *with_db.get_files().iter().last().unwrap(), 112).unwrap();
    let req = CompletionRequest { offset: 112, trigger_character: None, query: "".into(), node_index_pos: None, is_last_before: false };
    let completions = expr.completion(&with_db, &req).unwrap();

    assert_eq!(completions.len(), 3);
    assert!(format!("{completions:?}").contains("A"));
    assert!(format!("{completions:?}").contains("B"));
    assert!(format!("{completions:?}").contains("C"));
}

#[rstest]
pub fn invocation_completion(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    METHOD method 
    
    END_METHOD

    THIS.

END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let expr = descendant_at(&with_db, *with_db.get_files().iter().last().unwrap(), 108).unwrap();
    let req = CompletionRequest { offset: 108, trigger_character: None, query: "".into(), node_index_pos: None, is_last_before: false };
    let completions = expr.completion(&with_db, &req).unwrap();

    assert_eq!(completions.len(), 2);
    assert!(format!("{completions:?}").contains("test"));
    assert!(format!("{completions:?}").contains("method()"));
}

/// Typing `my_var := 0.` should NOT trigger field completions.
/// The dot after a numeric literal is part of a REAL literal (e.g. `0.0`),
/// not a field access.
#[rstest]
pub fn no_completion_after_numeric_literal_dot(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb
    VAR
        my_var: INT;
    END_VAR

    my_var := 0.
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let offset = source.find("0.").unwrap() + "0.".len();
    let file = *with_db.get_files().iter().last().unwrap();
    let result = completion_descendant_at(&with_db, file, offset);

    if let Some((node, node_key, is_last_before)) = result {
        let req = CompletionRequest {
            offset,
            trigger_character: Some(".".into()),
            query: "".into(),
            node_index_pos: Some(node_key),
            is_last_before,
        };
        let completions = node.completion(&with_db, &req).unwrap_or_default();
        // Should be empty or at least not contain all scope items
        assert!(
            completions.len() <= 1,
            "dot after numeric literal should not trigger completions, got {} items: {:?}",
            completions.len(),
            completions.iter().map(|c| &c.label).collect::<Vec<_>>()
        );
    }
}
