use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::HoverContents;
use auto_lsp::lsp_types::MarkedString;
use db::RootDatabase;
use hir::HasName;
use hir::HirNodeInfo;
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
                .map(marker_string_to_string)
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
    ```iecst
    FUNCTION fn1
    ```

    ```iecst
    FUNCTION_BLOCK fb1
    ```

    ---
    # fb1 comment
    ```iecst
    CLASS class1
    ```

    ---
    # class1 comment
    ");
}

#[rstest]
pub fn multiline_block_comment_hover(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE System

    /*
    ## SR - Set-Dominant Bistable

    - `S1` = **TRUE** sets output `Q1` to TRUE
    - `R` = **TRUE** resets output `Q1` to FALSE
    - If both are TRUE, **set wins**
    */
    FUNCTION_BLOCK SR
    END_FUNCTION_BLOCK

END_NAMESPACE
"#;

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::PouDecl(ty) = node else { return None };
        hover_markup(ty.hover(db, ty.get_name_span(db).start_byte)?.contents)
    }), @r"
    ```iecst
    System
    FUNCTION_BLOCK SR
    ```

    ---
    ## SR - Set-Dominant Bistable

    - `S1` = **TRUE** sets output `Q1` to TRUE
    - `R` = **TRUE** resets output `Q1` to FALSE
    - If both are TRUE, **set wins**
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
    ```iecst
    (VAR) var1: INT
    ```

    ```iecst
    (VAR) var2: INT
    ```

    ---
    # var2 comment
    ```iecst
    (VAR) var3: INT
    ```

    ---
    # var3 comment
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
TYPE Arr10: ARRAY[1..10] OF INT; END_TYPE
"#;

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::PouDecl(ty) = node else { return None };
        hover_markup(ty.hover(db, ty.get_name_span(db).start_byte)?.contents)
    }), @r"
    ```iecst
    TYPE Arr10: ARRAY [1..10] OF INT
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
    TYPE AnEnum: ENUM A, B, C, D 
    ```

    ```iecst
    TYPE ABiggerEnum: ENUM A, B, C, D, E, F, G, H, I, J, ... (2 more) 
    ```
    ");
}

#[rstest]
pub fn hover_type_specs(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    Arr10: ARRAY[1..10] OF INT;
    ASubrange: INT (0..6);
    Alias: Arr10;
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
    TYPE Arr10: ARRAY [1..10] OF INT
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
    TYPE Engine: STRUCT 
        oil: INT,
        fuel: BOOL

    ```

    ```iecst
    TYPE Engine: STRUCT 
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

#[rstest]
pub fn hover_namespace_fragments_in_spec(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE System
    FUNCTION_BLOCK Controller
    END_FUNCTION_BLOCK
END_NAMESPACE

FUNCTION_BLOCK fb1
    VAR
        x : System.Controller;
    END_VAR
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let mut specs = vec![];
    let _ = sema.walk_hir(&with_db, &mut |node| {
        if let HirNode::Spec(s) = node {
            specs.push(s);
        }
        ControlFlow::Continue(())
    });

    // Find the Spec for System.Controller (the one with a namespace)
    let ns_spec = specs
        .iter()
        .find(|s| {
            let kind = s.kind(&with_db);
            matches!(kind, hir::hir_def::expressions::spec::SpecKind::Target(t) if t.path.namespace.is_some())
        })
        .expect("should find a namespace-qualified Spec");

    // Hover on "System" fragment -> should show namespace hover
    let system_offset = source.find("System.Controller;").unwrap();
    let hover_ns = ns_spec.hover(&with_db, system_offset).unwrap();
    assert_snapshot!(hover_markup(hover_ns.contents).unwrap(), @r"
    ```iecst
    NAMESPACE System
    ```
    ");

    // Hover on "Controller" target -> should show resolved type hover
    let ctrl_offset = source.find("System.Controller;").unwrap() + "System.".len();
    let hover_target = ns_spec.hover(&with_db, ctrl_offset).unwrap();
    assert_snapshot!(hover_markup(hover_target.contents).unwrap(), @r"

    ```iecst
    System
    FUNCTION_BLOCK Controller
    ```

    ");
}

#[rstest]
pub fn hover_namespace_path_expr_in_body(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE System
    FUNCTION Sin : REAL
    VAR_INPUT
        x : REAL;
    END_VAR
    END_FUNCTION
END_NAMESPACE

FUNCTION fn1 : REAL
VAR
    x : REAL;
END_VAR
    x := System.Sin(x := x);
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // Find the PathExpr nodes for "System" (the namespace fragment in the body)
    let mut path_exprs = vec![];
    let _ = sema.walk_hir(&with_db, &mut |node| {
        if let HirNode::PathExpr(p) = node {
            path_exprs.push(p);
        }
        ControlFlow::Continue(())
    });

    // The PathExpr for "System" in the body should show namespace hover
    let system_offset = source.find("System.Sin").unwrap();
    let system_path = path_exprs
        .iter()
        .find(|p| {
            let span = p.get_span(&with_db);
            system_offset >= span.start_byte && system_offset <= span.end_byte
        })
        .expect("should find PathExpr at System position");

    let hover = system_path.hover(&with_db, system_offset).unwrap();
    assert_snapshot!(hover_markup(hover.contents).unwrap(), @r"
    ```iecst
    NAMESPACE System
    ```
    ");
}

#[rstest]
pub fn hover_config_task(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(PRIORITY := 5);
    END_RESOURCE
END_CONFIGURATION
"#;

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::Task(t) = node else { return None };
        hover_markup(t.hover(db, t.name(db).get_span(db).start_byte)?.contents)
    }), @r"
    ```iecst
    TASK t1 (PRIORITY := 5)
    ```
    ");
}

#[rstest]
pub fn hover_config_prog_instance(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(PRIORITY := 5);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::ProgConfig(p) = node else { return None };
        hover_markup(p.hover(db, p.name(db).get_span(db).start_byte)?.contents)
    }), @r"
    ```iecst
    PROGRAM inst1 WITH t1 : MyProg
    ```
    ");
}

#[rstest]
pub fn hover_config_with_task_ref(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(PRIORITY := 5);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let mut prog_configs = vec![];
    let _ = sema.walk_hir(&with_db, &mut |node| {
        if let HirNode::ProgConfig(p) = node {
            prog_configs.push(p);
        }
        ControlFlow::Continue(())
    });

    assert_eq!(prog_configs.len(), 1);
    let prog = prog_configs[0];

    // Hover on "t1" in "WITH t1" should show the task hover
    let task_ref = prog.task(&with_db).unwrap();
    let task_offset = task_ref.get_span(&with_db).start_byte;
    let hover = prog.hover(&with_db, task_offset).unwrap();
    assert_snapshot!(hover_markup(hover.contents).unwrap(), @r"
    ```iecst
    TASK t1 (PRIORITY := 5)
    ```
    ");
}

#[rstest]
pub fn hover_config_prog_type_spec(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(PRIORITY := 5);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE
END_CONFIGURATION
"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // Find Spec nodes — the prog_type spec should show PROGRAM hover
    let mut specs = vec![];
    let _ = sema.walk_hir(&with_db, &mut |node| {
        if let HirNode::Spec(s) = node {
            specs.push(s);
        }
        ControlFlow::Continue(())
    });

    // Find the Spec for MyProg (the one with a Target kind matching "MyProg")
    let prog_spec = specs
        .iter()
        .find(|s| {
            let kind = s.kind(&with_db);
            matches!(kind, hir::hir_def::expressions::spec::SpecKind::Target(t) if t.path.target.ident.text(&with_db) == "MyProg")
        })
        .expect("should find MyProg Spec");

    let hover = prog_spec.hover(&with_db, 0).unwrap();
    assert_snapshot!(hover_markup(hover.contents).unwrap(), @r"

    ```iecst
    PROGRAM MyProg
    ```

    ");
}

#[rstest]
pub fn hover_comment_bracket_ref_link(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
END_FUNCTION_BLOCK

(* Uses [MyFB] internally *)
FUNCTION fn1
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let mut pous = vec![];
    let _ = sema.walk_hir(&with_db, &mut |node| {
        if let HirNode::PouDecl(p) = node {
            pous.push(p);
        }
        ControlFlow::Continue(())
    });

    // fn1 has a comment with [MyFB] → should become a markdown link
    let fn1 = pous
        .iter()
        .find(|p| p.get_name_ident(&with_db).text(&with_db) == "fn1")
        .unwrap();
    let hover = fn1
        .hover(&with_db, fn1.get_name_span(&with_db).start_byte)
        .unwrap();
    assert_snapshot!(hover_markup(hover.contents).unwrap(), @r"
    ```iecst
    FUNCTION fn1
    ```

    ---
    Uses [MyFB](file:///test0.st) internally
    ");
}

#[rstest]
pub fn hover_comment_bracket_ref_unresolved(mut with_db: RootDatabase) {
    let source = r#"
(* References [NonExistent] type *)
FUNCTION fn1
END_FUNCTION
"#;

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::PouDecl(ty) = node else { return None };
        hover_markup(ty.hover(db, ty.get_name_span(db).start_byte)?.contents)
    }), @r"
    ```iecst
    FUNCTION fn1
    ```

    ---
    References [NonExistent] type
    ");
}

#[rstest]
pub fn hover_comment_bracket_ref_variable(mut with_db: RootDatabase) {
    let source = r#"
CLASS Sensor
END_CLASS

FUNCTION fn1
VAR
    (* Controls a [Sensor] *)
    x : INT;
END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let mut vars = vec![];
    let _ = sema.walk_hir(&with_db, &mut |node| {
        if let HirNode::VariableDecl(v) = node {
            vars.push(v);
        }
        ControlFlow::Continue(())
    });

    let x_var = vars
        .iter()
        .find(|v| v.name(&with_db).text(&with_db) == "x")
        .unwrap();
    let hover = x_var.hover(&with_db, 0).unwrap();
    assert_snapshot!(hover_markup(hover.contents).unwrap(), @r"
    ```iecst
    (VAR) x: INT
    ```

    ---
    Controls a [Sensor](file:///test0.st)
    ");
}

/// Array bound expressions should show inferred type on hover (not {unknown})
#[rstest]
fn array_bound_expr_hover(mut with_db: RootDatabase) {
    let source = "TYPE MyArr : ARRAY[0..10] OF INT; END_TYPE\n";

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::Expr(e) = node else { return None };
        hover_markup(e.hover(db, e.get_span(db).start_byte)?.contents)
    }), @r"
    ```iecst
    {integer} 0
    ```

    ```iecst
    {integer} 10
    ```
    ");
}

/// Array bound expressions in multiline TYPE block should show inferred type on hover
#[rstest]
fn array_bound_expr_hover_type_block(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    test0: ARRAY[0..10] OF INT;
END_TYPE
"#;

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::Expr(e) = node else { return None };
        hover_markup(e.hover(db, e.get_span(db).start_byte)?.contents)
    }), @r"
    ```iecst
    {integer} 0
    ```

    ```iecst
    {integer} 10
    ```
    ");
}

/// Array bound expressions in variable declarations should show inferred type on hover
#[rstest]
fn array_bound_expr_hover_variable(mut with_db: RootDatabase) {
    let source =
        "FUNCTION test\n    VAR\n        x : ARRAY[0..10] OF INT;\n    END_VAR\nEND_FUNCTION\n";

    assert_snapshot!(collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::Expr(e) = node else { return None };
        hover_markup(e.hover(db, e.get_span(db).start_byte)?.contents)
    }), @r"
    ```iecst
    {integer} 0
    ```

    ```iecst
    {integer} 10
    ```
    ");
}

#[rstest]
fn hover_range_same_file_highlights_declaration(mut with_db: RootDatabase) {
    // The declaration and the call live in the same file: the hover range is allowed to
    // point at the declaration site (this is the intentional decl-site highlight).
    let source =
        "FUNCTION helper : INT\nEND_FUNCTION\n\nFUNCTION main : INT\n    helper();\nEND_FUNCTION\n";
    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let usage = source.rfind("helper").unwrap();
    let node = ide_proto::walk::descendant_at(&with_db, file, usage).unwrap();
    let hover = node.hover(&with_db, usage).unwrap();

    let range = hover
        .range
        .expect("same-file hover should carry a decl-site range");
    // `helper` is declared on line 0 after "FUNCTION ".
    assert_eq!(range.start.line, 0);
    assert_eq!(range.start.character, "FUNCTION ".len() as u32);
}

#[rstest]
fn hover_range_cross_file_has_no_range(mut with_db: RootDatabase) {
    // The definition lives in another file. LSP `Hover.range` cannot point across files,
    // so the range is dropped (otherwise it would land on unrelated text in this file).
    let lib = "FUNCTION helper : INT\nEND_FUNCTION\n";
    let user = "FUNCTION main : INT\n    helper();\nEND_FUNCTION\n";
    add_sources(&mut with_db, &[lib, user]);
    let file = with_db
        .get_file(&auto_lsp::lsp_types::Url::parse("file:///test1.st").unwrap())
        .unwrap();

    let usage = user.find("helper").unwrap();
    let node = ide_proto::walk::descendant_at(&with_db, file, usage).unwrap();
    let hover = node.hover(&with_db, usage).unwrap();

    assert!(
        hover.range.is_none(),
        "cross-file hover must not carry a range, got {:?}",
        hover.range
    );
}

/// Hovering a relative namespace fragment names the namespace it resolves
/// to, with the enclosing path the user did not have to write.
#[rstest]
pub fn relative_namespace_fragment_hover(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Lib
    NAMESPACE Impl
        FUNCTION hidden : INT
            hidden := 1;
        END_FUNCTION
    END_NAMESPACE
    FUNCTION api : INT
        api := Impl.hidden();
    END_FUNCTION
END_NAMESPACE
"#;
    let impl_offset = source.find("Impl.hidden").unwrap();
    let out = collect_hovers(&mut with_db, source, |db, node| {
        let HirNode::PathExpr(p) = node else {
            return None;
        };
        let span = p.get_span(db);
        if impl_offset < span.start_byte || impl_offset > span.end_byte {
            return None;
        }
        hover_markup(node.hover(db, impl_offset)?.contents)
    });
    assert!(out.contains("NAMESPACE Lib.Impl"), "{out}");
}

/// A length or a bound written in a spec is an expression like any other,
/// and hovering it used to say unknown. Nothing recorded a STRING length's
/// type at all, and a name resolved in a spec was only ever looked for in a
/// body, where it never appears.
#[rstest]
pub fn hover_on_a_spec_bound(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb
VAR CONSTANT
    SIZE : INT := 30;
END_VAR
VAR
    a : STRING[20];
    b : STRING[SIZE];
    c : ARRAY[0..9] OF INT;
    d : ARRAY[0..SIZE] OF INT;
END_VAR
END_FUNCTION_BLOCK
"#;
    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let hovered: Vec<String> = ["20]", "SIZE]", "9]", "SIZE] OF"]
        .iter()
        .map(|needle| {
            let offset = source.find(needle).expect("the bound");
            let node = ide_proto::walk::descendant_at(&with_db, file, offset).expect("a node");
            let text = ide_proto::handlers::HoverHandler::hover(&node, &with_db, offset)
                .and_then(|h| hover_markup(h.contents))
                .expect("hover text");
            format!("{needle} -> {}", text.replace('\n', " ").trim())
        })
        .collect();

    assert_snapshot!(hovered.join("\n"), @r"
    20] -> ```iecst {integer} 20 ```
    SIZE] -> ```iecst (VAR) SIZE: INT ```
    9] -> ```iecst {integer} 9 ```
    SIZE] OF -> ```iecst (VAR) SIZE: INT ```
    ");
}

/// A name in a program configuration shows what it names.
#[rstest]
#[case::input("x1 := %IX", "(INPUT) x1: BOOL")]
#[case::source("w, y1", "(GLOBAL) w: UINT")]
#[case::function_block("fb1 WITH", "Counter")]
#[case::task("FAST);", "TASK FAST")]
#[case::var_config_member("out AT %QW0", "out: INT")]
pub fn hover_in_a_program_configuration(
    mut with_db: RootDatabase,
    #[case] at: &str,
    #[case] shown: &str,
) {
    use crate::tests::lsp::{CONNECTED, connected_at};
    add_sources(&mut with_db, &[CONNECTED]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = connected_at(at, true);
    let node = ide_proto::walk::descendant_at(&with_db, file, offset).expect("a node");
    let hover = node.hover(&with_db, offset).expect("a hover");
    let markup = hover_markup(hover.contents).unwrap();
    assert!(markup.contains(shown), "{markup}");
}
