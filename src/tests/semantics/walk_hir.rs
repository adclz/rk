use std::ops::ControlFlow;

use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use hir::walk::WalkHir;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::make_db_with_source;
use crate::tests::utils::with_db;

#[rstest]
pub fn walk_init_expr(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT := INT#5;
    END_VAR

END_FUNCTION_BLOCK"#;

    let file = make_db_with_source(&mut with_db, source);
    let sema = semantic_index(&with_db, file);

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
    ");
}

#[rstest]
pub fn walk_class_methods(mut with_db: RootDatabase) {
    let source = r#"
CLASS C2
   METHOD INTERNAL myInternalMethod: INT  END_METHOD
   METHOD PUBLIC myPublicMethod: INT  END_METHOD
END_CLASS"#;

    let file = make_db_with_source(&mut with_db, source);
    let sema = semantic_index(&with_db, file);

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

    let file = make_db_with_source(&mut with_db, source);
    let sema = semantic_index(&with_db, file);

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

    let file = make_db_with_source(&mut with_db, source);
    let sema = semantic_index(&with_db, file);

    let mut nodes = vec![];

    let _ = sema.walk_hir(&with_db, &mut |node| {
        nodes.push(format!("{node:?}"));
        ControlFlow::Continue(())
    });

    assert_snapshot!(nodes.join("\n"), @r"
    Ty(Ty { [salsa id]: Id(2800) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4400) })
    ResolvedVarResult(ResolvedVarResult { [salsa id]: Id(3c00) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4000) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4401) })
    ResolvedPathResult(ResolvedPathResult { [salsa id]: Id(3802) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4402) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4004) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4403) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4005) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4006) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(4007) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4405) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(400a) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4404) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4407) })
    ResolvedExpr(ResolvedExpr { [salsa id]: Id(400d) })
    ResolvedStmt(ResolvedStmt { [salsa id]: Id(4406) })
    ");
}
