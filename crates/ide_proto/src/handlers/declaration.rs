use auto_lsp::lsp_types::{Location, request::GotoDeclarationResponse};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, Expr, InitExpr, ParamAssign, PathExpr, VariableAccess},
            spec::{Spec, StructElement},
        },
        hir_node::HirNode,
        pous::variable::VariableDecl,
    },
    hir_ty::{infer::Infer, ty::Type},
};

use crate::handlers::DeclarationHandler;

impl<'db> DeclarationHandler<'db> for HirNode<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        match self {
            HirNode::VariableDecl(v) => v.declaration(db),
            HirNode::StructElement(s) => s.declaration(db),
            HirNode::InitExpr(i) => i.declaration(db),
            HirNode::Spec(s) => s.declaration(db),
            HirNode::PathExpr(p) => p.declaration(db),
            HirNode::VariableAccess(v) => v.declaration(db),
            HirNode::Expr(e) => e.declaration(db),
            HirNode::Param(p) => p.declaration(db),
            // Structured Text declares these where it defines them, so their
            // declaration IS their definition. They answered nothing at all
            // before, which is not the same as the two being distinct.
            HirNode::PouDecl(_)
            | HirNode::Program(_)
            | HirNode::MethodRef(_)
            | HirNode::Namespace(_)
            | HirNode::Using(_)
            | HirNode::Config(_)
            | HirNode::Resource(_)
            | HirNode::Task(_)
            | HirNode::ProgConfig(_) => crate::handlers::DefinitionHandler::definition(self, db, 0),
            _ => None,
        }
    }
}

impl<'db> DeclarationHandler<'db> for VariableDecl<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        Some(GotoDeclarationResponse::Scalar(Location::new(
            self.get_scope_id(db).file(db).url(db).to_owned(),
            hir::denormalize(db, self.get_scope_id(db).file(db), &self.get_span(db))
                .unwrap_or_default(),
        )))
    }
}

impl<'db> DeclarationHandler<'db> for StructElement<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        Some(GotoDeclarationResponse::Scalar(Location::new(
            self.scope_id(db).file(db).url(db).to_owned(),
            hir::denormalize(db, self.get_scope_id(db).file(db), &self.get_span(db))
                .unwrap_or_default(),
        )))
    }
}

impl<'db> DeclarationHandler<'db> for Spec<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        self.infer(db).declaration(db)
    }
}

impl<'db> DeclarationHandler<'db> for InitExpr<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        self.infer(db).declaration(db)
    }
}

impl<'db> DeclarationHandler<'db> for BeginPathExpr<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        self.infer(db).declaration(db)
    }
}

impl<'db> DeclarationHandler<'db> for PathExpr<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        self.infer(db).declaration(db)
    }
}

impl<'db> DeclarationHandler<'db> for Expr<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        self.infer(db).declaration(db)
    }
}

impl<'db> DeclarationHandler<'db> for VariableAccess<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        self.infer(db).declaration(db)
    }
}

impl<'db> DeclarationHandler<'db> for ParamAssign<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        self.infer(db).declaration(db)
    }
}

impl<'db> DeclarationHandler<'db> for Type<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        match self {
            Type::Variable((var, _multibits)) => var.declaration(db),
            _ => None,
        }
    }
}
