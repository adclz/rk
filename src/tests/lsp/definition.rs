use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::GotoDefinitionResponse;
use db::RootDatabase;
use hir::HirNodeInfo;
use hir::hir_def::hir_node::HirNode;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::handlers::DefinitionHandler;
use ide_proto::walk::{WalkHir, descendant_at};
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::add_sources;
use crate::tests::utils::with_db;

fn format_definition_response(resp: &GotoDefinitionResponse) -> String {
    match resp {
        GotoDefinitionResponse::Scalar(loc) => {
            format!(
                "{}:{}:{}-{}:{}",
                loc.uri.path(),
                loc.range.start.line,
                loc.range.start.character,
                loc.range.end.line,
                loc.range.end.character
            )
        }
        GotoDefinitionResponse::Array(locs) => locs
            .iter()
            .map(|loc| {
                format!(
                    "{}:{}:{}-{}:{}",
                    loc.uri.path(),
                    loc.range.start.line,
                    loc.range.start.character,
                    loc.range.end.line,
                    loc.range.end.character
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        GotoDefinitionResponse::Link(_) => "Link(...)".to_string(),
    }
}

#[rstest]
pub fn definition_namespace_fragments_in_spec(mut with_db: RootDatabase) {
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

    // Definition on "System" fragment -> should go to namespace declaration
    let system_offset = source.find("System.Controller;").unwrap();
    let def_ns = ns_spec.definition(&with_db, system_offset).unwrap();
    assert_snapshot!(format_definition_response(&def_ns), @"/test0.st:1:10-1:16");

    // Definition on "Controller" target -> should go to the POU definition
    let ctrl_offset = source.find("System.Controller;").unwrap() + "System.".len();
    let def_target = ns_spec.definition(&with_db, ctrl_offset).unwrap();
    assert_snapshot!(format_definition_response(&def_target), @"/test0.st:2:19-2:29");
}

#[rstest]
pub fn definition_namespace_path_expr_in_body(mut with_db: RootDatabase) {
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

    // Find the PathExpr nodes
    let mut path_exprs = vec![];
    let _ = sema.walk_hir(&with_db, &mut |node| {
        if let HirNode::PathExpr(p) = node {
            path_exprs.push(p);
        }
        ControlFlow::Continue(())
    });

    // The PathExpr for "System" in the body should go to namespace definition
    let system_offset = source.find("System.Sin").unwrap();
    let system_path = path_exprs
        .iter()
        .find(|p| {
            let span = p.get_span(&with_db);
            system_offset >= span.start_byte && system_offset <= span.end_byte
        })
        .expect("should find PathExpr at System position");

    let def = system_path.definition(&with_db, system_offset).unwrap();
    assert_snapshot!(format_definition_response(&def), @"/test0.st:1:10-1:16");
}

#[rstest]
pub fn definition_namespace_path_expr_via_descendant_at(mut with_db: RootDatabase) {
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
    let file = *with_db.get_files().iter().last().unwrap();

    // Use descendant_at (like the server does) to find the node at "System" offset
    let system_offset = source.find("System.Sin").unwrap();
    let node = descendant_at(&with_db, file, system_offset);
    let node = node.expect("should find a node at System offset");

    // Debug: what node type did we find?
    let node_type = match &node {
        HirNode::PathExpr(_) => "PathExpr",
        HirNode::PouDecl(_) => "PouDecl",
        HirNode::VariableDecl(_) => "VariableDecl",
        HirNode::Spec(_) => "Spec",
        HirNode::Expr(_) => "Expr",
        HirNode::VariableAccess(_) => "VariableAccess",
        HirNode::Namespace(_) => "Namespace",
        _ => "Other",
    };
    assert_snapshot!(node_type, @"PathExpr");

    let def = node.definition(&with_db, system_offset);
    assert!(
        def.is_some(),
        "definition should return Some for namespace PathExpr"
    );
    assert_snapshot!(format_definition_response(&def.unwrap()), @"/test0.st:1:10-1:16");
}

#[rstest]
pub fn definition_variable_ref_elementary_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
    END_VAR
        x := 5;
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Find the node at "x" in the body (x := 5)
    let body_x_offset = source.rfind("x").unwrap();
    let node = descendant_at(&with_db, file, body_x_offset).expect("should find node at x");

    let def = node.definition(&with_db, body_x_offset).unwrap();
    // Should go to the variable declaration, not the type
    assert_snapshot!(format_definition_response(&def), @"/test0.st:3:8-3:9");
}

#[rstest]
pub fn definition_variable_ref_target_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Engine
    VAR
        power : INT;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK fb1
    VAR
        motor : Engine;
    END_VAR
        motor.power := 5;
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Find the node at "motor" in the body (motor.power := 5)
    let motor_offset = source.rfind("motor").unwrap();
    let node = descendant_at(&with_db, file, motor_offset).expect("should find node at motor");

    let def = node.definition(&with_db, motor_offset).unwrap();
    // Should go to the Engine FUNCTION_BLOCK definition, not the variable declaration
    assert_snapshot!(format_definition_response(&def), @"/test0.st:1:15-1:21");
}

#[rstest]
pub fn definition_config_prog_type(mut with_db: RootDatabase) {
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

    // Find the ProgConfig node
    let mut prog_configs = vec![];
    let _ = sema.walk_hir(&with_db, &mut |node| {
        if let HirNode::ProgConfig(p) = node {
            prog_configs.push(p);
        }
        ControlFlow::Continue(())
    });

    assert_eq!(prog_configs.len(), 1);
    let prog = prog_configs[0];

    // Definition on the prog_type (MyProg) should go to the PROGRAM declaration
    let prog_type_offset = source.rfind("MyProg").unwrap();
    let def = prog.definition(&with_db, prog_type_offset).unwrap();
    // Should point to the PROGRAM MyProg declaration (line 1)
    assert_snapshot!(format_definition_response(&def), @"/test0.st:1:8-1:14");
}

#[rstest]
pub fn definition_config_with_task_ref(mut with_db: RootDatabase) {
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

    // Find the ProgConfig node
    let mut prog_configs = vec![];
    let _ = sema.walk_hir(&with_db, &mut |node| {
        if let HirNode::ProgConfig(p) = node {
            prog_configs.push(p);
        }
        ControlFlow::Continue(())
    });

    assert_eq!(prog_configs.len(), 1);
    let prog = prog_configs[0];

    // Definition on "t1" in "WITH t1" should go to the TASK declaration
    let task_ref = prog.task(&with_db).unwrap();
    let task_offset = task_ref.get_span(&with_db).start_byte;
    let def = prog.definition(&with_db, task_offset).unwrap();
    // Should point to the task name "t1" in the TASK declaration (line 5)
    assert_snapshot!(format_definition_response(&def), @"/test0.st:6:13-6:15");
}

#[rstest]
pub fn definition_config_task_node(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(PRIORITY := 5);
    END_RESOURCE
END_CONFIGURATION
"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // Find the Task node
    let mut tasks = vec![];
    let _ = sema.walk_hir(&with_db, &mut |node| {
        if let HirNode::Task(t) = node {
            tasks.push(t);
        }
        ControlFlow::Continue(())
    });

    assert_eq!(tasks.len(), 1);
    let task = tasks[0];

    // Definition on the task name should go to itself
    let name_offset = task.name(&with_db).get_span(&with_db).start_byte;
    let def = task.definition(&with_db, name_offset).unwrap();
    assert_snapshot!(format_definition_response(&def), @"/test0.st:3:13-3:15");
}

/// A namespace path written RELATIVE to the enclosing namespace resolves the
/// way the checker resolves it: `Impl` inside `NAMESPACE Lib` is `Lib.Impl`.
/// The IDE used to look the written path up as if it were absolute, so
/// go-to-definition on `Impl` here found nothing while the call compiled.
#[rstest]
pub fn definition_relative_namespace_path_in_body(mut with_db: RootDatabase) {
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

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let mut path_exprs = vec![];
    let _ = sema.walk_hir(&with_db, &mut |node| {
        if let HirNode::PathExpr(p) = node {
            path_exprs.push(p);
        }
        ControlFlow::Continue(())
    });

    let impl_offset = source.find("Impl.hidden()").unwrap();
    let impl_path = path_exprs
        .iter()
        .find(|p| {
            let span = p.get_span(&with_db);
            impl_offset >= span.start_byte && impl_offset <= span.end_byte
        })
        .expect("a PathExpr at Impl");

    let def = impl_path
        .definition(&with_db, impl_offset)
        .expect("Impl resolves to NAMESPACE Lib.Impl from inside Lib");
    assert_snapshot!(format_definition_response(&def), @"/test0.st:2:14-2:18");
}

/// The same relative rule in a type spec and in a USING: both are written
/// inside `Lib`, so `Impl` in each means `Lib.Impl`.
#[rstest]
pub fn definition_relative_namespace_path_in_spec_and_using(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Lib
    USING Impl;
    NAMESPACE Impl
        TYPE T : INT; END_TYPE
    END_NAMESPACE
    FUNCTION api : INT
        VAR x : Impl.T; END_VAR
    END_FUNCTION
END_NAMESPACE
"#;

    add_sources(&mut with_db, &[source]);
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    let spec_offset = source.find("Impl.T").unwrap();
    let mut using = None;
    let mut spec = None;
    let _ = sema.walk_hir(&with_db, &mut |node| {
        match node {
            HirNode::Using(u) => using = Some(u),
            HirNode::Spec(s) => {
                let span = s.get_span(&with_db);
                if spec_offset >= span.start_byte && spec_offset <= span.end_byte {
                    spec = Some(s);
                }
            }
            _ => {}
        }
        ControlFlow::Continue(())
    });

    let def = using
        .expect("the USING")
        .definition(&with_db, 0)
        .expect("USING Impl inside Lib names Lib.Impl");
    assert_snapshot!(format_definition_response(&def), @"/test0.st:3:14-3:18");

    let def = spec
        .expect("the spec at Impl.T")
        .definition(&with_db, spec_offset)
        .expect("Impl in the spec names Lib.Impl");
    assert_snapshot!(format_definition_response(&def), @"/test0.st:3:14-3:18");
}

/// On its own name a declaration IS the definition. Forwarding to the type
/// answered nothing for an elementary one, which left the link an inlay hint
/// puts on a parameter resolving nowhere.
#[rstest]
pub fn definition_on_a_declaration_name(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb
    VAR_INPUT
        p : INT;
    END_VAR
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find("p : INT").expect("the declaration");
    let node = descendant_at(&with_db, file, offset).expect("a node at p");
    let def = node.definition(&with_db, offset).expect("a definition");

    assert_snapshot!(format_definition_response(&def), @"/test0.st:3:8-3:9");
}

/// Structured Text declares these where it defines them, so asking for a
/// declaration answers. It used to answer for variables and fields only, and
/// a PROGRAM's or a METHOD's own name had no definition arm either.
#[rstest]
#[case::a_function_block("fb\nMETHOD")]
#[case::a_method("m : INT")]
#[case::a_namespace("ns\nFUNCTION_BLOCK")]
#[case::a_program("prog\nVAR")]
#[case::a_variable("inst : ")]
pub fn declaration_answers_for_every_named_thing(mut with_db: RootDatabase, #[case] needle: &str) {
    let source = r#"
NAMESPACE ns
FUNCTION_BLOCK fb
METHOD m : INT
END_METHOD
END_FUNCTION_BLOCK
END_NAMESPACE

PROGRAM prog
VAR
    inst : ns.fb;
END_VAR
END_PROGRAM
"#;
    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();
    let offset = source.find(needle).expect("the name");
    let node = descendant_at(&with_db, file, offset).expect("a node");

    assert!(
        ide_proto::handlers::DeclarationHandler::declaration(&node, &with_db).is_some(),
        "no declaration for {node:?}"
    );
}
