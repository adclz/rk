use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use hir::walk::WalkHir;
use ide_proto::AsProtocol;
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
    PouDecl(PouDecl { [salsa id]: Id(2400) })
    VariableDecl(VariableDecl { [salsa id]: Id(1c00) })
    Spec(Spec { [salsa id]: Id(c00) })
    ResolvedInitExpr(ResolvedInitExpr { [salsa id]: Id(3000) })
    Expr(Expr { [salsa id]: Id(1400) })
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
    PouDecl(PouDecl { [salsa id]: Id(1800) })
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
    PouDecl(PouDecl { [salsa id]: Id(1400) })
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
    PouDecl(PouDecl { [salsa id]: Id(3000) })
    ResolvedAccess(ResolvedAccess { kind: Err(NoLocalItemInScope { expr: PathExpr { [salsa id]: Id(c00) } }), call_site: CallSite { scope: ScopeId { [salsa id]: Id(402) }, id: AstId(10) }, elements: [] })
    Expr(Expr { [salsa id]: Id(1c00) })
    ResolvedAccess(ResolvedAccess { kind: Err(NoLocalItemInScope { expr: PathExpr { [salsa id]: Id(c01) } }), call_site: CallSite { scope: ScopeId { [salsa id]: Id(402) }, id: AstId(16) }, elements: [] })
    ResolvedAccess(ResolvedAccess { kind: Err(NoLocalItemInScope { expr: PathExpr { [salsa id]: Id(c02) } }), call_site: CallSite { scope: ScopeId { [salsa id]: Id(402) }, id: AstId(21) }, elements: [] })
    Expr(Expr { [salsa id]: Id(1c04) })
    Expr(Expr { [salsa id]: Id(1c02) })
    ResolvedAccess(ResolvedAccess { kind: Err(NoLocalItemInScope { expr: PathExpr { [salsa id]: Id(c05) } }), call_site: CallSite { scope: ScopeId { [salsa id]: Id(402) }, id: AstId(47) }, elements: [] })
    Expr(Expr { [salsa id]: Id(1c03) })
    Expr(Expr { [salsa id]: Id(1c05) })
    Expr(Expr { [salsa id]: Id(1c06) })
    Expr(Expr { [salsa id]: Id(1c07) })
    Expr(Expr { [salsa id]: Id(1c0a) })
    Expr(Expr { [salsa id]: Id(1c08) })
    ResolvedAccess(ResolvedAccess { kind: Err(NoLocalItemInScope { expr: PathExpr { [salsa id]: Id(c08) } }), call_site: CallSite { scope: ScopeId { [salsa id]: Id(402) }, id: AstId(84) }, elements: [] })
    Expr(Expr { [salsa id]: Id(1c09) })
    Expr(Expr { [salsa id]: Id(1c0d) })
    Expr(Expr { [salsa id]: Id(1c0b) })
    ResolvedAccess(ResolvedAccess { kind: Err(NoLocalItemInScope { expr: PathExpr { [salsa id]: Id(c0a) } }), call_site: CallSite { scope: ScopeId { [salsa id]: Id(402) }, id: AstId(99) }, elements: [] })
    Expr(Expr { [salsa id]: Id(1c0c) })
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
    PouDecl(PouDecl { [salsa id]: Id(3800) })
    SpanNamespaceAccess(Extends(SpanNamespaceAccess { id: AstId(3), scope_id: ScopeId { [salsa id]: Id(402) }, path: NamespaceAccess { namespace: None, target: SpanIdent { id: AstId(4), scope_id: ScopeId { [salsa id]: Id(402) }, ident: Ident(Id(800)) } } }))
    MethodRef(Declared(MethodDecl { [salsa id]: Id(3000) }))
    VariableDecl(VariableDecl { [salsa id]: Id(2c00) })
    Spec(Spec { [salsa id]: Id(2800) })
    ResolvedAccess(ResolvedAccess { kind: Ok(ResolvedPath { expr: CallSite { scope: ScopeId { [salsa id]: Id(402) }, id: AstId(24) }, adjustement: None, kind: This(PouDecl { [salsa id]: Id(3800) }) }), call_site: CallSite { scope: ScopeId { [salsa id]: Id(402) }, id: AstId(20) }, elements: [Ok(ResolvedPath { expr: CallSite { scope: ScopeId { [salsa id]: Id(402) }, id: AstId(26) }, adjustement: None, kind: Method(Declared(MethodDecl { [salsa id]: Id(3000) })) })] })
    ResolvedPath(ResolvedPath { expr: CallSite { scope: ScopeId { [salsa id]: Id(402) }, id: AstId(26) }, adjustement: None, kind: Method(Declared(MethodDecl { [salsa id]: Id(3000) })) })
    ResolvedParam(ResolvedParam { param_assign: ParamAssign { [salsa id]: Id(1c00) }, kind: NonFormal { resolved_param: Some(VariableDecl { [salsa id]: Id(2c00) }), value: Expr { [salsa id]: Id(1800) } } })
    VariableDecl(VariableDecl { [salsa id]: Id(2c00) })
    Spec(Spec { [salsa id]: Id(2800) })
    Expr(Expr { [salsa id]: Id(1800) })
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
        nodes.push(node.as_proto().get_id(&with_db).id());
        ControlFlow::Continue(())
    });

    assert_debug_snapshot!(nodes, @r"
    [
        1,
        10,
        12,
        16,
        21,
        41,
        42,
        47,
        49,
        61,
        66,
        71,
        78,
        79,
        84,
        86,
        93,
        94,
        99,
        101,
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
        nodes.push(node.as_proto().get_id(&with_db).id());
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
        29,
        31,
        4,
        10,
        32,
        39,
        13,
        18,
        40,
        46,
        47,
    ]
    ");
}
