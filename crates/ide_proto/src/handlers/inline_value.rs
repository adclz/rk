//! Inline values: where a debugger should show what a variable holds.
//!
//! The server never sees a runtime value. It answers with the RANGES that name
//! a variable and the name to look up; the debugger supplies the value for
//! the frame the client is stopped in. That split is the whole point of the
//! request, and it is why this needs nothing from the debugger.

use auto_lsp::lsp_types::{InlineValue, InlineValueParams, InlineValueVariableLookup, Range};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::expression::PathExprKind, hir_node::HirNode, semantic_index::semantic_index,
    },
    hir_ty::body::infer_body,
};

use crate::walk::WalkHir;

/// Whether `inner` sits within `outer`, by line.
fn within(outer: &Range, inner: &Range) -> bool {
    inner.start.line >= outer.start.line && inner.start.line <= outer.end.line
}

pub fn inline_values<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: auto_lsp::default::db::file::File,
    params: &InlineValueParams,
) -> Vec<InlineValue> {
    let stopped = params.context.stopped_location.end.line;
    let mut values: Vec<InlineValue> = Vec::new();

    let _ = semantic_index(db, file).walk_hir(db, &mut |node| {
        let named = match &node {
            // The VAR line, so the value is visible beside the declaration.
            HirNode::VariableDecl(var) => Some((var.get_name_span(db), var.get_name_ident(db))),
            // A bare name in the body. A FIELD step (`a.b`) is deliberately
            // skipped: `b` alone is not a name the debugger's scope holds,
            // and the path that would resolve it is the runtime's shape, not
            // the source's.
            HirNode::PathExpr(p) => match p.expr(db) {
                PathExprKind::VarAccess(_) => infer_body(db, p.get_scope_id(db))
                    .variable_for_path_expr(*p)
                    .map(|var| (p.get_span(db), var.get_name_ident(db))),
                _ => None,
            },
            _ => None,
        };

        let Some((span, ident)) = named else {
            return std::ops::ControlFlow::Continue(());
        };
        let Some(range) = hir::denormalize(db, file, &span) else {
            return std::ops::ControlFlow::Continue(());
        };
        // Only what the client asked about, and only what execution has
        // reached: a value shown below the stopped line is last scan's.
        if !within(&params.range, &range) || range.start.line > stopped {
            return std::ops::ControlFlow::Continue(());
        }

        values.push(InlineValue::VariableLookup(InlineValueVariableLookup {
            range,
            variable_name: Some(ident.text(db).to_string()),
            // IEC folds case, so `Motor` and `motor` are one name. The flag
            // exists for exactly this.
            case_sensitive_lookup: false,
        }));
        std::ops::ControlFlow::<()>::Continue(())
    });

    // The walk can reach one name through more than one node.
    values.dedup_by_key(|v| match v {
        InlineValue::VariableLookup(l) => l.range,
        InlineValue::Text(t) => t.range,
        InlineValue::EvaluatableExpression(e) => e.range,
    });
    values
}
