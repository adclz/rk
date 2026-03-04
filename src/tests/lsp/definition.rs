use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::GotoDefinitionResponse;
use db::RootDatabase;
use hir::HirNodeInfo;
use hir::hir_def::hir_node::HirNode;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::handlers::DefinitionHandler;
use ide_proto::walk::WalkHir;
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
