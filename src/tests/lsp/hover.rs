use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::HoverContents;
use auto_lsp::lsp_types::MarkedString;
use db::RootDatabase;
use hir::HasName;
use hir::hir_def::hir_node::HirNode;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::handlers::HoverHandler;
use ide_proto::walk::WalkHir;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::add_sources;
use crate::tests::utils::with_db;

/// Walk the HIR and collect hover text using the provided extraction closure.
fn collect_hovers(
    db: &mut RootDatabase,
    source: &str,
    extract: impl Fn(&RootDatabase, &HirNode) -> Option<String>,
) -> String {
    add_sources(db, &[source]);
    let sema = semantic_index(db, *db.get_files().iter().last().unwrap());

    let mut nodes = vec![];
    let _ = sema.walk_hir(db, &mut |node| {
        nodes.push(node);
        ControlFlow::Continue(())
    });

    nodes
        .iter()
        .filter_map(|node| extract(db, node))
        .collect::<Vec<String>>()
        .join("\n")
}

fn hover_markup(contents: HoverContents) -> Option<String> {
    match contents {
        HoverContents::Scalar(marked) => Some(marker_string_to_string(marked)),
        HoverContents::Array(arr) => Some(
            arr.into_iter()
                .map(|marked| marker_string_to_string(marked))
                .collect::<Vec<String>>()
                .join("\n"),
        ),
        HoverContents::Markup(markup) => Some(markup.value.to_string()),
    }
}

fn marker_string_to_string(marker: MarkedString) -> String {
    match marker {
        MarkedString::String(s) => s,
        MarkedString::LanguageString(l) => {
            let language = l.language;
            let value = l.value;
            format!("```{language}\n{value}\n```")
        }
    }
}

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

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::PouDecl(ty) = node else { return None };
        hover_markup(ty.hover(db, ty.get_name_span(db).start_byte)?.contents)
    }), @r"
    # fn1 comment
    ```iecst
    FUNCTION fn1
    ```
                    

    # fb1 comment
    ```iecst
    FUNCTION_BLOCK fb1
    ```
                    

    # class1 comment
    ```iecst
    CLASS class1
    ```
    ");
}

#[rstest]
pub fn namespace_hover(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE System

    NAMESPACE Subsystem1
        
    END_NAMESPACE
END_NAMESPACE
"#;

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::Namespace(ns) = node else { return None };
        hover_markup(node.hover(db, ns.name_span(db).start_byte).unwrap().contents)
    }), @r"
    ```iecst
    NAMESPACE System
    ```
                        

    ```iecst
    NAMESPACE System.Subsystem1
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

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::VariableDecl(var) = node else { return None };
        hover_markup(var.hover(db, 0)?.contents)
    }), @r"
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

#[rstest]
pub fn namespace_path(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE System.Subsystem1

    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    NAMESPACE Subsystem1

        FUNCTION_BLOCK fb2

        END_FUNCTION_BLOCK

    END_NAMESPACE

END_NAMESPACE
"#;

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::PouDecl(ty) = node else { return None };
        hover_markup(ty.hover(db, ty.get_name_span(db).start_byte)?.contents)
    }), @r"
    ```iecst
    System.Subsystem1
    FUNCTION_BLOCK fb1
    ```
                    


    ```iecst
    System.Subsystem1.Subsystem1
    FUNCTION_BLOCK fb2
    ```
    ");
}

#[rstest]
pub fn hover_array_type_decl(mut with_db: RootDatabase) {
    let source = r#"
TYPE Array: ARRAY[1..10] OF INT; END_TYPE
"#;

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::PouDecl(ty) = node else { return None };
        hover_markup(ty.hover(db, ty.get_name_span(db).start_byte)?.contents)
    }), @r"
    ```iecst
    TYPE Array: ARRAY [1..10] OF INT
    ```
    ");
}

#[rstest]
pub fn hover_subrange_type_decl(mut with_db: RootDatabase) {
    let source = r#"
TYPE ASubrange: INT (0..6) END_TYPE
"#;

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::PouDecl(ty) = node else { return None };
        hover_markup(ty.hover(db, ty.get_name_span(db).start_byte)?.contents)
    }), @r"
    ```iecst
    TYPE ASubrange: INT (0..6)
    ```
    ");
}

#[rstest]
pub fn hover_enum_type_decl(mut with_db: RootDatabase) {
    let source = r#"
TYPE AnEnum: (A, B, C, D) END_TYPE
TYPE ABiggerEnum: (
    A, 
    B, 
    C, 
    D,
    E,
    F,
    G,
    H,
    I,
    J, // should start collapsing here
    K,
    L) 
END_TYPE

"#;

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::PouDecl(ty) = node else { return None };
        hover_markup(ty.hover(db, ty.get_name_span(db).start_byte)?.contents)
    }), @r"
    ```iecst
    TYPE AnEnum: ENUM { A, B, C, D }
    ```
                    


    ```iecst
    TYPE ABiggerEnum: ENUM { A, B, C, D, E, F, G, H, I, J, ... (2 more) }
    ```
    ");
}

#[rstest]
pub fn hover_type_specs(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    Array: ARRAY[1..10] OF INT;
    ASubrange: INT (0..6);
    Alias: Array;
END_TYPE
"#;

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::Spec(ty) = node else { return None };
        hover_markup(ty.hover(db, 0)?.contents)
    }), @r"
    ```iecst
    ARRAY [1..10] OF INT
    ```
                    


    ```iecst
    INT
    ```
                    


    ```iecst
    INT (0..6)
    ```
                    


    ```iecst
    INT
    ```
                    


    ```iecst
    Array: ARRAY [1..10] OF INT
    ```
    ");
}

#[rstest]
pub fn hover_struct_type_with_fields_decl(mut with_db: RootDatabase) {
    let source = r#"
TYPE 
    Engine: STRUCT
        oil: INT;
        fuel: BOOL;
    END_STRUCT

    Engine: STRUCT
        oil1: INT;
        oil2: BOOL;
        oil3: INT;
        oil4: BOOL;
        oil5: INT; 
        oil6: BOOL;
        oil7: INT;
        oil8: BOOL;
        oil9: INT;
        oil10: BOOL; // should start collapsing here
        oil11: INT;
        oil12: BOOL;
    END_STRUCT
END_TYPE
"#;

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::PouDecl(ty) = node else { return None };
        hover_markup(ty.hover(db, ty.get_name_span(db).start_byte)?.contents)
    }), @r"
    ```iecst
    TYPE Engine: STRUCT {
        oil: INT,
        fuel: BOOL
    }
    ```
                    


    ```iecst
    TYPE Engine: STRUCT {
        oil1: INT,
        oil2: BOOL,
        oil3: INT,
        oil4: BOOL,
        oil5: INT,
        oil6: BOOL,
        oil7: INT,
        oil8: BOOL,
        oil9: INT,
        oil10: BOOL
        ... (2 more fields)
    }
    ```
    ");
}

#[rstest]
pub fn hover_struct_elements(mut with_db: RootDatabase) {
    let source = r#"
TYPE 
    Engine: STRUCT
        oil: INT;
        fuel: BOOL;
    END_STRUCT
END_TYPE
"#;

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::StructElement(ty) = node else { return None };
        hover_markup(ty.hover(db, ty.get_name_span(db).start_byte)?.contents)
    }), @r"
    ```iecst
    oil: INT
    ```
                    


    ```iecst
    fuel: BOOL
    ```
    ");
}

#[rstest]
pub fn hover_path_exprs(mut with_db: RootDatabase) {
    let source = r#"
TYPE 
    Engine: STRUCT
        oil: INT;
        fuel: BOOL;
    END_STRUCT
END_TYPE

FUNCTION_BLOCK fb
    VAR
        my_var: Engine;
    END_VAR

    my_var.oil := my_var.fuel;
END_FUNCTION_BLOCK
"#;

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::PathExpr(_) = node else { return None };
        assert!(node.hover(db, 0).is_some(), "Expected hover on path expression");
        hover_markup(node.hover(db, 0)?.contents)
    }), @r"
    ```iecst
    (VAR) my_var: Engine
    ```
                    


    ```iecst
    oil: INT
    ```
                    


    ```iecst
    (VAR) my_var: Engine
    ```
                    


    ```iecst
    fuel: BOOL
    ```
    ");
}
