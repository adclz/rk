use auto_lsp::lsp_types::{GotoDefinitionResponse, Location};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, Expr, ParamAssign, PathExpr, VariableAccess},
            spec::{Spec, StructElement},
        },
        pous::{pou::Pou, variable::VariableDecl},
    },
    hir_ty::{body::infer_body, ty::Type},
};

use crate::handlers::DefinitionHandler;

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

impl<'db> DefinitionHandler<'db> for BeginPathExpr<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .type_of_begin_expr_with_adjustments(db, *self)
            .and_then(|typ| typ.definition(db))
    }
}

impl<'db> DefinitionHandler<'db> for PathExpr<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .type_of_path_expr_with_adjustments(*self)
            .and_then(|typ| typ.definition(db))
    }
}

impl<'db> DefinitionHandler<'db> for VariableAccess<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .type_of_variable_access_with_adjustments(db, *self)
            .and_then(|typ| typ.definition(db))
    }
}

impl<'db> DefinitionHandler<'db> for Expr<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .type_of_expr_with_adjustments(db, *self)
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
            Type::DataType(dt) =>  return dt.spec(db).definition(db),
            Type::Variable((var, _multibits)) => return var.definition(db),
            _ => None?,
        };

        Some(GotoDefinitionResponse::Scalar(Location::new(
            loc.get_scope_id(db).file(db).url(db).to_owned(),
            loc.get_span(db).into(),
        )))
    }
}
