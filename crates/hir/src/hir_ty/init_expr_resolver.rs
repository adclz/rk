use crate::{
    check::errors::init_expr::InitExprError, hir_def::{
        expressions::expression::{Expr},
        interned::identifier::SpanIdent,
        scope::ScopeId,
    }, hir_ty::{
        flatten::{Flatten, UnResolvedInitExpr, UnResolvedInitExprKind}, ty::{Ty, TyKind}, ty_var_access_resolver::{CallSite, ResolvedAccess}, walk::{Adjustement, ResolvedPath, ResolvedPathKind, ResolvedPathResult}
    }, AstId, HirNodeInfo
};
use auto_lsp::default::db::BaseDatabase;

use crate::hir_def::expressions::expression::InitExpr;

#[salsa::tracked(returns(ref))]
pub fn resolve_init_expr<'db>(
    db: &'db dyn BaseDatabase,
    ty: Ty<'db>,
    init: InitExpr<'db>,
) -> ResolvedInitExpr<'db> {
    let unordered = init.flatten(db);
    resolve_unresolved(db, ty, unordered)
}

fn resolve_unresolved<'db>(
    db: &'db dyn BaseDatabase,
    ty: Ty<'db>,
    init: UnResolvedInitExpr<'db>,
) -> ResolvedInitExpr<'db> {
    let kind = match init.kind {
        UnResolvedInitExprKind::ArrayInit { values } => {
            match ty.kind(db) {
                TyKind::Array(array) => {
                    // Valid: Array type with ArrayInit
                    let resolved = values
                        .into_iter()
                        .map(|v| resolve_unresolved(db, array.of_type(db).to_ty(db), v))
                        .collect();
                    ResolvedInitExprKind::ArrayInit { values: resolved }
                }
                _ => {
                    // Invalid: Non-array type with ArrayInit - generate error instead of recursing
                    return ResolvedInitExpr::new(
                        db,
                        init.expr,
                        ResolvedInitExprKind::Error(InitExprError::TypeInitExprMismatch {
                            expected: ty,
                            found: init.expr,
                        }),
                    );
                }
            }
        }
        UnResolvedInitExprKind::ArrayIndexedElement { size, values } => {
            let resolved = values
                .into_iter()
                .map(|v| match ty.kind(db) {
                    // For array types, resolve values with element type
                    TyKind::Array(array) => resolve_unresolved(db, array.of_type(db).to_ty(db), v),
                    // For simple types (multidimensional case), resolve with same type
                    _ => resolve_unresolved(db, ty, v),
                })
                .collect();
            ResolvedInitExprKind::ArrayIndexedElement {
                size,
                values: resolved,
            }
        }
        UnResolvedInitExprKind::StructInit { values } => {
            match ty.kind(db) {
                TyKind::Struct { .. } => {
                    // Valid: Struct type with StructInit
                    let resolved = values
                        .into_iter()
                        .map(|v| resolve_unresolved(db, ty, v))
                        .collect();
                    ResolvedInitExprKind::StructInit { values: resolved }
                }
                _ => {
                    // Invalid: Non-struct type with StructInit - generate error instead of recursing
                    return ResolvedInitExpr::new(
                        db,
                        init.expr,
                        ResolvedInitExprKind::Error(InitExprError::TypeInitExprMismatch {
                            expected: ty,
                            found: init.expr,
                        }),
                    );
                }
            }
        }
        UnResolvedInitExprKind::StructElement { name, value } => {
            match ty.kind(db) {
                TyKind::Struct(ztruct) => {
                    if let Some(element_ty) = ztruct.resolve_elements(db).get(&name) {
                        // Valid field - resolve with field type
                        let resolved_value = Box::new(resolve_unresolved(
                            db,
                            element_ty.spec(db).to_ty(db),
                            *value,
                        ));
                        let field = ResolvedAccess::new(
                            db,
                            ResolvedPathResult::Ok(ResolvedPath {
                                kind: ResolvedPathKind::StructElement(*element_ty),
                                expr: CallSite::new( name.scope_id, name.id),
                                adjustement: Adjustement::None,
                            }),
                            vec![],
                        );

                        ResolvedInitExprKind::StructElement {
                            field,
                            value: resolved_value,
                        }
                    } else {
                        // Unknown field - generate error
                        ResolvedInitExprKind::Error(InitExprError::UnknownStructField {
                            ztruct: ty,
                            field_name: name,
                            unknown_field: resolve_unresolved(db, ty, *value),
                        })
                    }
                }
                _ => {
                    // Invalid: StructElement against non-struct type
                    return ResolvedInitExpr::new(
                        db,
                        init.expr,
                        ResolvedInitExprKind::Error(InitExprError::TypeInitExprMismatch {
                            expected: ty,
                            found: init.expr,
                        }),
                    );
                }
            }
        }
        UnResolvedInitExprKind::ConstantExpr(expr) => ResolvedInitExprKind::ConstantExpr(expr),
    };

    ResolvedInitExpr::new(db, init.expr, kind)
}

#[salsa::tracked(debug)]
pub struct ResolvedInitExpr<'db> {
    pub expr: InitExpr<'db>,
    pub kind: ResolvedInitExprKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolvedInitExprKind<'db> {
    ArrayInit {
        values: Vec<ResolvedInitExpr<'db>>,
    },
    ArrayIndexedElement {
        size: SpanIdent<'db>,
        values: Vec<ResolvedInitExpr<'db>>,
    },
    StructInit {
        values: Vec<ResolvedInitExpr<'db>>,
    },
    StructElement {
        field: ResolvedAccess<'db>,
        value: Box<ResolvedInitExpr<'db>>,
    },
    ConstantExpr(Expr<'db>),
    Error(InitExprError<'db>),
}

impl<'db> HirNodeInfo<'db> for ResolvedInitExpr<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.expr(db).id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.expr(db).scope_id(db)
    }
}
