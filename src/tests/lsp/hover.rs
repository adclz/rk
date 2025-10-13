use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::HoverContents;
use db::RootDatabase;
use hir::hir_def::pous::pou::Pou;
use hir::hir_def::semantic_index::HirNode;
use hir::hir_def::semantic_index::semantic_index;
use hir::hir_ty::ty::ty_for_variable;
use hir::hir_ty::ty::TyDef;
use hir::walk::WalkHir;
use ide_proto::ty::TyHover;
use ide_proto::AsProtocol;
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
        if let HirNode::Ty(ty) = node
            && let TyDef::Pou(pou) = ty.def(&with_db)
        {
            if let HoverContents::Markup(d) = ty.force_hover(&with_db)?.contents {
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
    (FUNCTION) fn1
    ```


    # fb1 comment
    ```iecst
    (FUNCTION_BLOCK) fb1
    ```


    # class1 comment
    ```iecst
    (CLASS) class1
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
        if let HirNode::Ty(ty) = node
            && let TyDef::Pou(pou) = ty.def(&with_db)
            && let Pou::FunctionBlock(fb) = pou.pou(&with_db)
        {
            for var in fb.variables(&with_db) {
                nodes.push(ty_for_variable(&with_db, *var));
            }
        }
        ControlFlow::Continue(())
    });

    assert_snapshot!(nodes.iter().filter_map(|n|{
        if let HoverContents::Markup(d) = n.force_hover(&with_db)?.contents {
            Some(d.value)
        } else {
            None
        }})
        .collect::<Vec<String>>()
        .join("\n"), @r"
    # var1 comment
    ```iecst
    (VAR) var1
    ```


    # var2 comment
    ```iecst
    (VAR) var2
    ```


    # var3 comment
    ```iecst
    (VAR) var3
    ```
    ");
}
