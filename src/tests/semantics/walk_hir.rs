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
    Ty(Ty { [salsa id]: Id(2800) })
    Ty(Ty { [salsa id]: Id(2801) })
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
    Ty(Ty { [salsa id]: Id(1c00) })
    Ty(Ty { [salsa id]: Id(1c01) })
    Ty(Ty { [salsa id]: Id(1c02) })
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
    Ty(Ty { [salsa id]: Id(1800) })
    Ty(Ty { [salsa id]: Id(1801) })
    Ty(Ty { [salsa id]: Id(1802) })
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
    Ty(Ty { [salsa id]: Id(2c00) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4800) })
    ResolvedVarResult(ResolvedVarResult { [salsa id]: Id(3400) })
    ResolvedPathElementResult(ResolvedPathElement { expr: PathExpr { [salsa id]: Id(c00) }, kind: Error(NoItemInScope { expr: PathExpr { [salsa id]: Id(c00) }, scope: FileScopeId { [salsa id]: Id(402) } }) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4400) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4801) })
    ResolvedVarResult(ResolvedVarResult { [salsa id]: Id(3402) })
    ResolvedPathElementResult(ResolvedPathElement { expr: PathExpr { [salsa id]: Id(c02) }, kind: Error(NoItemInScope { expr: PathExpr { [salsa id]: Id(c02) }, scope: FileScopeId { [salsa id]: Id(402) } }) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4802) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4403) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4803) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4404) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4405) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4406) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4804) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4409) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4805) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(440c) })
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
    Ty(Ty { [salsa id]: Id(3c01) })
    Ty(Ty { [salsa id]: Id(3c03) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4c00) })
    ResolvedVarResult(ResolvedVarResult { [salsa id]: Id(4000) })
    ResolvedVarResult(ResolvedVarResult { [salsa id]: Id(4001) })
    ResolvedParam(ResolvedParam { [salsa id]: Id(4800) })
    ResolvedVarResult(ResolvedVarResult { [salsa id]: Id(4002) })
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
        7,
        10,
        12,
        16,
        19,
        19,
        37,
        39,
        52,
        59,
        64,
        69,
        74,
        76,
        89,
        91,
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
        13,
        21,
        25,
        28,
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
