use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::HoverContents;
use db::RootDatabase;
use hir::hir_def::semantic_index::HirNode;
use hir::hir_def::semantic_index::semantic_index;
use hir::walk::WalkHir;
use ide_proto::ToProtocol;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::add_sources;
use crate::tests::utils::with_db;

#[rstest]
pub fn pous_hover_comment(mut with_db: RootDatabase) {
    let source = r#"
// # fn1 comment
FUNCTION fn1
END_FUNCTION

FUNCTION_BLOCK fb1 /* # fb1 comment*/
END_FUNCTION_BLOCK

(* # class1 comment *)
CLASS class1
END_CLASS
"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let mut nodes = vec![];

    let _ = sema.walk_hir(&with_db, &mut |node| {
        nodes.push(node);
        ControlFlow::Continue(())
    });

    assert_snapshot!(nodes.iter().filter_map(|node|{
        if let HirNode::PouDecl(ty) = node
        {
            if let HoverContents::Markup(d) = ty.hover(&with_db, ty.name_span(&with_db).start_byte)?.contents {
                Some(d.value)
            } else {
                None
            }
        } else {
            None
        }})
        .collect::<Vec<String>>()
        .join("\n"), @r"
    # fn1 comment
    ```iecst
    [FUNCTION] fn1
    ```
                    

    # fb1 comment
    ```iecst
    [FUNCTION_BLOCK] fb1
    ```
                    

    # class1 comment
    ```iecst
    [CLASS] class1
    ```
    ");
}

#[rstest]
pub fn variables_hover_comment(mut with_db: RootDatabase) {
    let source = r#"
// # fn1 comment
FUNCTION_BLOCK fb1
    VAR
        // # var1 comment
        var1: INT; // inline comment
        (* # var2 comment *)
        var2: INT;

        var3: INT; /* # var3 comment */
    END_VAR

END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let mut nodes = vec![];

    let _ = sema.walk_hir(&with_db, &mut |node| {
        if let HirNode::VariableDecl(var) = node {
            nodes.push(var);
        }
        ControlFlow::Continue(())
    });

    assert_snapshot!(nodes.iter().filter_map(|n|{
        if let HoverContents::Markup(d) = n.hover(&with_db, 0)?.contents {
            Some(d.value)
        } else {
            None
        }})
        .collect::<Vec<String>>()
        .join("\n"), @r"
    # var1 comment
    ```iecst
    (VAR) var1: INT
    ```
                    

    # var2 comment
    ```iecst
    (VAR) var2: INT
    ```
                    

    # var3 comment
    ```iecst
    (VAR) var3: INT
    ```
    ");
}
