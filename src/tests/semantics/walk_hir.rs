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
    ResolvedInitExpr(ResolvedInitExpr { [salsa id]: Id(3400) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(3000) })
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
    PouDecl(PouDecl { [salsa id]: Id(2800) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4400) })
    ResolvedAccess(ResolvedAccess { [salsa id]: Id(3c00) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4000) })
    ResolvedAccess(ResolvedAccess { [salsa id]: Id(3c01) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4401) })
    ResolvedAccess(ResolvedAccess { [salsa id]: Id(3c02) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4402) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4003) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4001) })
    ResolvedAccess(ResolvedAccess { [salsa id]: Id(3c03) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4002) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4403) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4004) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4005) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4006) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4404) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4009) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4007) })
    ResolvedAccess(ResolvedAccess { [salsa id]: Id(3c05) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4008) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4405) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(400c) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(400a) })
    ResolvedAccess(ResolvedAccess { [salsa id]: Id(3c06) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(400b) })
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
    PouDecl(PouDecl { [salsa id]: Id(3400) })
    SpanNamespaceAccess(SpanNamespaceAccess { id: AstId(3), scope_id: FileScopeId { [salsa id]: Id(402) }, path: NamespaceAccess(Id(c00)) })
    MethodRef(Declared(MethodDecl { [salsa id]: Id(2c00) }))
    VariableDecl(VariableDecl { [salsa id]: Id(2800) })
    Spec(Spec { [salsa id]: Id(2400) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4c00) })
    ResolvedAccess(ResolvedAccess { [salsa id]: Id(4000) })
    MethodRef(Declared(MethodDecl { [salsa id]: Id(2c00) }))
    VariableDecl(VariableDecl { [salsa id]: Id(2800) })
    Spec(Spec { [salsa id]: Id(2400) })
    ResolvedParam(ResolvedParam { [salsa id]: Id(4800) })
    ResolvedAccess(ResolvedAccess { [salsa id]: Id(4001) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4400) })
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
        5,
        10,
        12,
        15,
        16,
        19,
        37,
        39,
        40,
        45,
        47,
        52,
        59,
        64,
        69,
        74,
        76,
        77,
        82,
        84,
        89,
        91,
        92,
        97,
        99,
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
        25,
        28,
        30,
        31,
        31,
        38,
        39,
        39,
        45,
        46,
    ]
    ");
}
