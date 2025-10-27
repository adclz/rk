use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{GotoDefinitionResponse, Hover, request::GotoDeclarationResponse},
};
use hir::{
    hir_def::expressions::expression::{Expr, ExprKind, PrimaryExpr, RefValue},
    hir_ty::ty_var_access_resolver::{LookUp},
};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for Expr<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        match self.expr(db) {
            ExprKind::PrimaryExpr(primary) => match primary {
                PrimaryExpr::VariableAccess(variable) => {
                    variable.lookup(db).hover(db, offset)
                }
                PrimaryExpr::RefValue { value } => match value {
                    RefValue::Null => None,
                    RefValue::Address(constant) => {
                        constant.lookup(db).hover(db, offset)
                    }
                },
                _ => None,
            },
            _ => None,
        }
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        match self.expr(db) {
            ExprKind::PrimaryExpr(primary) => match primary {
                PrimaryExpr::VariableAccess(variable) => variable.lookup(db).definition(db),
                PrimaryExpr::RefValue { value } => match value {
                    RefValue::Null => None,
                    RefValue::Address(constant) => {
                        constant.lookup(db).definition(db)
                    }
                },
                _ => None,
            },
            _ => None,
        }
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        match self.expr(db) {
            ExprKind::PrimaryExpr(primary) => match primary {
                PrimaryExpr::VariableAccess(variable) => variable.lookup(db).declaration(db),
                PrimaryExpr::RefValue { value } => match value {
                    RefValue::Null => None,
                    RefValue::Address(constant) => {
                        constant.lookup(db).declaration(db)
                    }
                },
                _ => None,
            },
            _ => None,
        }
    }
}
