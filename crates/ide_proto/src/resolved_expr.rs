use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{GotoDefinitionResponse, Hover, request::GotoDeclarationResponse},
};
use hir::{hir_def::expressions::expression::{Expr, ExprKind, PrimaryExpr, RefValue}, hir_ty::ty_var_access_resolver::{resolve_local_path_expr, resolve_var_access}};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for Expr<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        match self.expr(db) {
            ExprKind::PrimaryExpr(primary) => {
                match primary {
                    PrimaryExpr::VariableAccess { variable, .. } => resolve_var_access(db, *variable).hover(db, offset),
                    PrimaryExpr::RefValue { value } => match value {
                        RefValue::Null => None,
                        RefValue::Address(constant) => resolve_local_path_expr(db, constant.kind).hover(db, offset),
                    },
                    _ => None,
                }
                
            },
            _ => None,
        }
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
         match self.expr(db) {
            ExprKind::PrimaryExpr(primary) => {
                match primary {
                    PrimaryExpr::VariableAccess { variable, .. } => resolve_var_access(db, *variable).definition(db),
                    PrimaryExpr::RefValue { value } => match value {
                        RefValue::Null => None,
                        RefValue::Address(constant) => resolve_local_path_expr(db, constant.kind).definition(db),
                    },
                    _ => None,
                }
                
            },
            _ => None,
        }
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
         match self.expr(db) {
            ExprKind::PrimaryExpr(primary) => {
                match primary {
                    PrimaryExpr::VariableAccess { variable, .. } => resolve_var_access(db, *variable).declaration(db),
                    PrimaryExpr::RefValue { value } => match value {
                        RefValue::Null => None,
                        RefValue::Address(constant) => resolve_local_path_expr(db, constant.kind).declaration(db),
                    },
                    _ => None,
                }
                
            },
            _ => None,
        }
    }
}
