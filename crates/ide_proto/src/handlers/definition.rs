use auto_lsp::lsp_types::{GotoDefinitionResponse, Location};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, Expr, InitExpr, ParamAssign, PathExpr, VariableAccess},
            spec::{Spec, StructElement},
        },
        pous::{pou::Pou, variable::VariableDecl},
    },
    hir_ty::{body::infer_body, signature::infer_signature, ty::Type},
};

use crate::{handlers::DefinitionHandler, hir_node::HirNode};

impl<'db> HirNode<'db> {
    pub fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        match self {
            HirNode::PouDecl(pou) => pou.definition(db),
            HirNode::VariableDecl(v) => v.definition(db),
            HirNode::StructElement(s) => s.definition(db),
            HirNode::InitExpr { curr, .. } => curr.definition(db),
            HirNode::Spec(s) => s.definition(db),
            HirNode::PathExpr { curr, .. } => curr.definition(db),
            HirNode::VariableAccess(v) => v.definition(db),
            HirNode::Expr(e) => e.definition(db),
            HirNode::Param(p) => p.definition(db),
            _ => None,
        }
    }
}

impl<'db> DefinitionHandler<'db> for Pou<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        Some(GotoDefinitionResponse::Scalar(Location::new(
            self.get_scope_id(db).file(db).url(db).to_owned(),
            self.get_span(db).into(),
        )))
    }
}

impl<'db> DefinitionHandler<'db> for VariableDecl<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        self.spec(db).definition(db)
    }
}

impl<'db> DefinitionHandler<'db> for StructElement<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        Some(GotoDefinitionResponse::Scalar(Location::new(
            self.scope_id(db).file(db).url(db).to_owned(),
            self.get_span(db).into(),
        )))
    }
}

impl<'db> DefinitionHandler<'db> for Spec<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        Some(GotoDefinitionResponse::Scalar(Location::new(
            self.get_scope_id(db).file(db).url(db).to_owned(),
            self.get_span(db).into(),
        )))
    }
}

impl<'db> DefinitionHandler<'db> for InitExpr<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        let infer = infer_signature(db, self.scope_id(db));
        infer
            .init_expr_result
            .type_of_init_expr
            .get(self)
            .and_then(|typ| typ.definition(db))
    }
}

impl<'db> DefinitionHandler<'db> for BeginPathExpr<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .get_type_of_begin_path_expr(db, *self)
            .and_then(|typ| typ.definition(db))
    }
}

impl<'db> DefinitionHandler<'db> for PathExpr<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .get_type_of_path_expr(db, *self)
            .and_then(|typ| typ.definition(db))
    }
}

impl<'db> DefinitionHandler<'db> for VariableAccess<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .get_type_of_variable_access(db, *self)
            .and_then(|typ| typ.definition(db))
    }
}

impl<'db> DefinitionHandler<'db> for Expr<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        if let Some(r) = infer_signature(db, self.scope_id(db))
            .body_infer_result
            .get_type_of_expr(*self)
            .and_then(|typ| typ.definition(db))
        {
            return Some(r);
        }

        infer_body(db, self.scope_id(db))
            .get_type_of_expr(*self)
            .and_then(|typ| typ.definition(db))
    }
}

impl<'db> DefinitionHandler<'db> for ParamAssign<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .variable_of_param
            .get(self)
            .and_then(|var| Type::new_var(db, *var).definition(db))
    }
}

impl<'db> DefinitionHandler<'db> for Type<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        let loc: &'db dyn HirNodeInfo<'db> = match self {
            Type::Function(f) => f as _,
            Type::FunctionBlock(f) => f as _,
            Type::Class(c) => c as _,
            Type::Interface(i) => i as _,
            Type::DataType(dt) => return dt.spec(db).definition(db),
            Type::Variable((var, _multibits)) => return var.definition(db),
            Type::StructElement(st) => return st.definition(db),
            _ => None?,
        };

        Some(GotoDefinitionResponse::Scalar(Location::new(
            loc.get_scope_id(db).file(db).url(db).to_owned(),
            loc.get_span(db).into(),
        )))
    }
}
