use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::HirNodeInfo;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::walk::WalkHir;
use insta::assert_debug_snapshot;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::add_sources;
use crate::tests::utils::with_db;

#[rstest]
pub fn walk_init_expr(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT := INT#5;
    END_VAR

END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let mut nodes = vec![];

    let _ = sema.walk_hir(&with_db, &mut |node| {
        nodes.push(format!("{node:?}"));
        ControlFlow::Continue(())
    });

    // fb1 = Ty (PouDecl)
    // test = Ty (VariabeDecl)
    // test := ** INT#5; ** = InitExpr (ResolvedInitExpr)

    assert_snapshot!(nodes.join("\n"), @r"
    PouDecl(FunctionBlock(FunctionBlock { [salsa id]: Id(2000) }))
    VariableDecl(VariableDecl { [salsa id]: Id(1c00) })
    Spec(Spec { [salsa id]: Id(c00) })
    InitExprWithType(InitExprWithTypeContext { init_expr: InitExpr { [salsa id]: Id(1800) }, ty: Elementary(Int) })
    ");
}

#[rstest]
pub fn walk_class_methods(mut with_db: RootDatabase) {
    let source = r#"
CLASS C2
   METHOD INTERNAL myInternalMethod: INT  END_METHOD
   METHOD PUBLIC myPublicMethod: INT  END_METHOD
END_CLASS"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let mut nodes = vec![];

    let _ = sema.walk_hir(&with_db, &mut |node| {
        nodes.push(format!("{node:?}"));
        ControlFlow::Continue(())
    });

    // C2 = Ty (PouDecl)
    // myInternalMethod = Ty (MethodDecl)
    // myPublicMethod = Ty (MethodDecl)

    assert_snapshot!(nodes.join("\n"), @r"
    PouDecl(Class(Class { [salsa id]: Id(1400) }))
    MethodRef(Declared(MethodDecl { [salsa id]: Id(1000) }))
    Spec(Spec { [salsa id]: Id(c00) })
    MethodRef(Declared(MethodDecl { [salsa id]: Id(1001) }))
    Spec(Spec { [salsa id]: Id(c01) })
    ");
}

#[rstest]
pub fn walk_interface_methods(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ROOM
    METHOD DAYTIME END_METHOD // Called in day-time
    METHOD NIGHTTIME END_METHOD // in night-time
END_INTERFACE "#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let mut nodes = vec![];

    let _ = sema.walk_hir(&with_db, &mut |node| {
        nodes.push(format!("{node:?}"));
        ControlFlow::Continue(())
    });

    // ROOM = Ty (PouDecl)
    // DAYTIME = Ty (MethodProt)
    // NIGHTTIME = Ty (MethodProt)

    assert_snapshot!(nodes.join("\n"), @r"
    PouDecl(Interface(Interface { [salsa id]: Id(1000) }))
    MethodRef(Prototype(MethodPrototype { [salsa id]: Id(c00) }))
    MethodRef(Prototype(MethodPrototype { [salsa id]: Id(c01) }))
    ");
}

#[rstest]
pub fn walk_stmts(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    test := fn_vall();

    fn_call(input := 5, output => test);

    IF test > 5 THEN
    END_IF;

    FOR i := 1 TO 10 BY 1 DO
    END_FOR;

    REPEAT UNTIL test = 100
    END_REPEAT;

    WHILE test < 100 DO
    END_WHILE;
END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let mut nodes = vec![];

    let _ = sema.walk_hir(&with_db, &mut |node| {
        nodes.push(format!("{node:?}"));
        ControlFlow::Continue(())
    });

    assert_snapshot!(nodes.join("\n"), @r"
    PouDecl(FunctionBlock(FunctionBlock { [salsa id]: Id(2c00) }))
    VariableAccess(VariableAccess { [salsa id]: Id(1400) })
    Expr(Expr { [salsa id]: Id(1c00) })
    BeginPathExpr(BeginPathExpr { [salsa id]: Id(1002) })
    Param(ParamAssign { [salsa id]: Id(2800) })
    Param(ParamAssign { [salsa id]: Id(2801) })
    Expr(Expr { [salsa id]: Id(1c04) })
    Expr(Expr { [salsa id]: Id(1c05) })
    Expr(Expr { [salsa id]: Id(1c06) })
    Expr(Expr { [salsa id]: Id(1c07) })
    Expr(Expr { [salsa id]: Id(1c0a) })
    Expr(Expr { [salsa id]: Id(1c0d) })
    ");
}

#[rstest]
pub fn walk_function_block_method(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1 EXTENDS base
	METHOD decl
        VAR_INPUT input1 : INT; END_VAR
	END_METHOD

	THIS.decl(0.5);

END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let mut nodes = vec![];

    let _ = sema.walk_hir(&with_db, &mut |node| {
        nodes.push(format!("{node:?}"));
        ControlFlow::Continue(())
    });

    assert_snapshot!(nodes.join("\n"), @r"
    PouDecl(FunctionBlock(FunctionBlock { [salsa id]: Id(3400) }))
    SpanNamespaceAccess(Extends(SpanNamespaceAccess { id: AstId(3), scope_id: ScopeId { [salsa id]: Id(402) }, path: NamespaceAccess { namespace: None, target: SpanIdent { id: AstId(4), scope_id: ScopeId { [salsa id]: Id(402) }, ident: Ident(Id(800)) } } }))
    MethodRef(Declared(MethodDecl { [salsa id]: Id(3000) }))
    VariableDecl(VariableDecl { [salsa id]: Id(2c00) })
    Spec(Spec { [salsa id]: Id(2800) })
    BeginPathExpr(BeginPathExpr { [salsa id]: Id(1400) })
    Param(ParamAssign { [salsa id]: Id(1c00) })
    ");
}

// Walking the HIR should preserve the order of AST nodes.
#[rstest]
pub fn sorted_ast_ids_in_function_block(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    test := fn_vall();

    fn_call(input := 5, output => test);

    IF test > 5 THEN
    END_IF;

    FOR i := 1 TO 10 BY 1 DO
    END_FOR;

    REPEAT UNTIL test = 100
    END_REPEAT;

    WHILE test < 100 DO
    END_WHILE;
END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let mut nodes = vec![];

    let _ = sema.walk_hir(&with_db, &mut |node| {
        nodes.push(node.get_id(&with_db).id());
        ControlFlow::Continue(())
    });

    assert_debug_snapshot!(nodes, @r"
    [
        1,
        6,
        12,
        18,
        23,
        32,
        41,
        61,
        66,
        71,
        78,
        93,
    ]
    ");
}

#[rstest]
pub fn sorted_ast_ids_in_non_formal_func_call(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
	VAR_INPUT
		param1: INT;
		param2: REAL;
	END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1

	fn(0, 1.5, 5);
END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let mut nodes = vec![];

    let _ = sema.walk_hir(&with_db, &mut |node| {
        nodes.push(node.get_id(&with_db).id());
        ControlFlow::Continue(())
    });

    assert_debug_snapshot!(nodes, @r"
    [
        1,
        4,
        10,
        13,
        18,
        21,
        26,
        31,
        39,
        46,
    ]
    ");
}
