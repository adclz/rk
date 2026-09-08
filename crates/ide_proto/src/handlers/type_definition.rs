//! Go to the TYPE of what the cursor is on, rather than to its declaration.
//!
//! `definition` on `m : Mode` lands on `m`; this lands on `Mode`. The two
//! differ by one peel, so the target is resolved here and handed to the same
//! [`DefinitionHandler`] that already knows where every kind of type is
//! written.

use auto_lsp::lsp_types::GotoDefinitionResponse;
use db::WorkspaceDataBase;
use hir::{
    hir_def::hir_node::HirNode,
    hir_ty::{infer::Infer, ty::Type},
};

use crate::handlers::DefinitionHandler;

pub fn type_definition<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: &HirNode<'db>,
    offset: usize,
) -> Option<GotoDefinitionResponse> {
    let ty = match node {
        HirNode::VariableDecl(var) => var.spec(db).infer(db),
        HirNode::StructElement(st) => st.spec(db).infer(db),
        HirNode::Spec(spec) => spec.infer(db),
        HirNode::PathExpr(p) => p.infer(db),
        HirNode::VariableAccess(v) => v.infer(db),
        HirNode::Expr(e) => e.infer(db),
        HirNode::Param(p) => p.infer(db),
        HirNode::InitExpr(i) => i.infer(db),
        // A POU name IS a type; asking for its type is asking for itself.
        HirNode::PouDecl(pou) => Type::new_pou(db, *pou),
        _ => return None,
    };

    let declared = match ty {
        // The declared type of the slot, not the slot.
        Type::Variable((var, _)) => var.spec(db).infer(db),
        Type::StructElement(st) => st.spec(db).infer(db),
        // A call is worth its return type, which is the type the reader is
        // asking about at a call site.
        Type::CallableType(_) | Type::Function(_) | Type::MethodDecl(_) => {
            ty.with_return_type(db).unwrap_or(ty)
        }
        other => other,
    };

    // Left UNNORMALIZED on purpose: `normalize` walks an alias down to the
    // elementary spec it wraps, which loses the very name the reader is
    // asking to land on. An elementary type has no declaration, and
    // `definition` says so by answering nothing.
    declared.definition(db, offset)
}
