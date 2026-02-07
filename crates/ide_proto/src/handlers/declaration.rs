use auto_lsp::lsp_types::{Location, request::GotoDeclarationResponse};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, Expr, InitExpr, ParamAssign, PathExpr, VariableAccess},
            spec::{Spec, StructElement},
        },
        pous::variable::VariableDecl,
    },
    hir_ty::{body::infer_body, infer::Infer, ty::Type},
};

use crate::{handlers::DeclarationHandler, hir_node::HirNode};

impl<'db> HirNode<'db> {
    pub fn declaration(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<GotoDeclarationResponse> {
        match self {
            HirNode::VariableDecl(v) => v.declaration(db),
            HirNode::StructElement(s) => s.declaration(db),
            HirNode::InitExpr { curr, .. } => curr.declaration(db),
            HirNode::Spec(s) => s.declaration(db),
            HirNode::PathExpr { curr, .. } => curr.declaration(db),
            HirNode::VariableAccess(v) => v.declaration(db),
            HirNode::Expr(e) => e.declaration(db),
            HirNode::Param(p) => p.declaration(db),
            _ => None,
        }
    }
}

impl<'db> DeclarationHandler<'db> for VariableDecl<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        Some(GotoDeclarationResponse::Scalar(Location::new(
            self.get_scope_id(db).file(db).url(db).to_owned(),
            self.get_span(db).into(),
        )))
    }
}

impl<'db> DeclarationHandler<'db> for StructElement<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        Some(GotoDeclarationResponse::Scalar(Location::new(
            self.scope_id(db).file(db).url(db).to_owned(),
            self.get_span(db).into(),
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
        let infer = infer_body(db, self.scope_id(db));
        infer
            .variable_of_param
            .get(self)
            .and_then(|var| Type::new_var(db, *var).declaration(db))
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
