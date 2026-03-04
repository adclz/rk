use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::GotoDefinitionResponse;
use db::RootDatabase;
use hir::HirNodeInfo;
use hir::hir_def::hir_node::HirNode;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::handlers::{DefinitionHandler, HoverHandler};
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
    assert_snapshot!(format_definition_response(&def_target), @"/test0.st:2:4-3:22");
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
    assert!(def.is_some(), "definition should return Some for namespace PathExpr");
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
    assert_snapshot!(format_definition_response(&def), @"/test0.st:3:8-3:15");
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
    assert_snapshot!(format_definition_response(&def), @"/test0.st:1:0-5:18");
}

#[rstest]
pub fn definition_config_prog_type(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(PRIORITY := 5);
    PROGRAM inst1 WITH t1 : MyProg;
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
    assert_snapshot!(format_definition_response(&def), @"/test0.st:1:0-2:11");
}

#[rstest]
pub fn definition_config_with_task_ref(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM MyProg
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(PRIORITY := 5);
    PROGRAM inst1 WITH t1 : MyProg;
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
    assert_snapshot!(format_definition_response(&def), @"/test0.st:5:9-5:11");
}

#[rstest]
pub fn definition_config_task_node(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    TASK t1(PRIORITY := 5);
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
    assert_snapshot!(format_definition_response(&def), @"/test0.st:2:9-2:11");
}
