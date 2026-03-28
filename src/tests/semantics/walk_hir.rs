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
use crate::tests::utils::test_snapshot;
use crate::tests::utils::walk_hir_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
pub fn walk_init_expr(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT := INT#5;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_snapshot(&mut with_db, &[source], walk_hir_diagnostics), @r"
    Advice: PouDecl(FunctionBlock)
       ,-[ file:///test0.st:2:16 ]
       |
     2 | FUNCTION_BLOCK fb1
       |                ^|^
       |                 `--- PouDecl(FunctionBlock)
       |
     4 |         test: INT := INT#5;
       |         ^^|^  ^|^ ^^^^||^^
       |           `----------------- VariableDecl
       |                |      ||
       |                `------------ Spec
       |                       ||
       |                       `----- InitExpr
       |                        |
       |                        `---- Expr
    ---'
    ");
}

#[rstest]
pub fn walk_class_methods(mut with_db: RootDatabase) {
    let source = r#"
CLASS C2
   METHOD INTERNAL myInternalMethod: INT  END_METHOD
   METHOD PUBLIC myPublicMethod: INT  END_METHOD
END_CLASS"#;

    assert_snapshot!(test_snapshot(&mut with_db, &[source], walk_hir_diagnostics), @r"
    Advice: PouDecl(Class)
       ,-[ file:///test0.st:2:7 ]
       |
     2 | CLASS C2
       |       ^|
       |        `-- PouDecl(Class)
     3 |    METHOD INTERNAL myInternalMethod: INT  END_METHOD
       |                    ^^^^^^^^|^^^^^^^  ^|^
       |                            `-------------- MethodRef(Declared)
       |                                       |
       |                                       `--- Spec
     4 |    METHOD PUBLIC myPublicMethod: INT  END_METHOD
       |                  ^^^^^^^|^^^^^^  ^|^
       |                         `------------- MethodRef(Declared)
       |                                   |
       |                                   `--- Spec
    ---'
    ");
}

#[rstest]
pub fn walk_interface_methods(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE ROOM
    METHOD DAYTIME END_METHOD // Called in day-time
    METHOD NIGHTTIME END_METHOD // in night-time
END_INTERFACE "#;

    assert_snapshot!(test_snapshot(&mut with_db, &[source], walk_hir_diagnostics), @r"
    Advice: PouDecl(Interface)
       ,-[ file:///test0.st:2:11 ]
       |
     2 | INTERFACE ROOM
       |           ^^|^
       |             `--- PouDecl(Interface)
     3 |     METHOD DAYTIME END_METHOD // Called in day-time
       |            ^^^|^^^
       |               `----- MethodRef(Prototype)
     4 |     METHOD NIGHTTIME END_METHOD // in night-time
       |            ^^^^|^^^^
       |                `------ MethodRef(Prototype)
    ---'
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

    assert_snapshot!(test_snapshot(&mut with_db, &[source], walk_hir_diagnostics), @r"
    Advice: PouDecl(FunctionBlock)
        ,-[ file:///test0.st:2:16 ]
        |
      2 | FUNCTION_BLOCK fb1
        |                ^|^
        |                 `--- PouDecl(FunctionBlock)
      3 |     test := fn_vall();
        |     ^^|^    ^^^||^^^^
        |       `---------------- VariableAccess
        |       |        ||
        |       `---------------- PathExpr
        |                ||
        |                `------- PathExpr
        |                 |
        |                 `------ Expr
        |
      5 |     fn_call(input := 5, output => test);
        |     ^^^|^^^ ^^^^^|^^^|  ^^^^^^^|^^^^|^
        |        `-------------------------------- PathExpr
        |                  |   |         |    |
        |                  `---------------------- Param
        |                      |         |    |
        |                      `------------------ Expr
        |                                |    |
        |                                `-------- Param
        |                                     |
        |                                     `--- VariableAccess
        |                                     |
        |                                     `--- PathExpr
        |
      7 |     IF test > 5 THEN
        |        ^^|^|^^|
        |          `------- VariableAccess
        |          | |  |
        |          `------- Expr
        |          | |  |
        |          `------- PathExpr
        |            |  |
        |            `----- Expr
        |               |
        |               `-- Expr
        |
     10 |     FOR i := 1 TO 10 BY 1 DO
        |         |    |    ^|    |
        |         `------------------ VariableAccess
        |         |    |     |    |
        |         `------------------ PathExpr
        |              |     |    |
        |              `------------- Expr
        |                    |    |
        |                    `------- Expr
        |                         |
        |                         `-- Expr
        |
     13 |     REPEAT UNTIL test = 100
        |                  ^^|^^|^^|^
        |                    `--------- VariableAccess
        |                    |  |  |
        |                    `--------- Expr
        |                    |  |  |
        |                    `--------- PathExpr
        |                       |  |
        |                       `------ Expr
        |                          |
        |                          `--- Expr
        |
     16 |     WHILE test < 100 DO
        |           ^^|^^|^^|^
        |             `--------- VariableAccess
        |             |  |  |
        |             `--------- Expr
        |             |  |  |
        |             `--------- PathExpr
        |                |  |
        |                `------ Expr
        |                   |
        |                   `--- Expr
    ----'
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

    assert_snapshot!(test_snapshot(&mut with_db, &[source], walk_hir_diagnostics), @r"
    Advice: PouDecl(FunctionBlock)
       ,-[ file:///test0.st:2:16 ]
       |
     2 | FUNCTION_BLOCK fb1 EXTENDS base
       |                ^|^         ^^|^
       |                 `---------------- PouDecl(FunctionBlock)
       |                              |
       |                              `--- Spec
     3 |     METHOD decl
       |            ^^|^
       |              `--- MethodRef(Declared)
     4 |         VAR_INPUT input1 : INT; END_VAR
       |                   ^^^|^^   ^|^
       |                      `---------- VariableDecl
       |                             |
       |                             `--- Spec
       |
     7 |     THIS.decl(0.5);
       |     ^^^^|^^|^ ^|^
       |         `---------- Invocation
       |            |   |
       |            `------- PathExpr
       |                |
       |                `--- Param
       |                |
       |                `--- Expr
    ---'
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
        11,
        13,
        17,
        22,
        24,
        26,
        33,
        35,
        39,
        42,
        43,
        43,
        48,
        50,
        56,
        60,
        62,
        67,
        72,
        79,
        80,
        80,
        85,
        87,
        94,
        95,
        95,
        100,
        102,
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
        29,
        31,
        32,
        39,
        40,
        47,
        48,
    ]
    ");
}
