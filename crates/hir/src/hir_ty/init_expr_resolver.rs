use crate::{
    check::errors::init_expr::InitExprError, hir_def::{
        expressions::expression::{Expr, InitExprKind}, interned::identifier::SpanIdent, scope::FileScopeId,
    }, hir_ty::{
        expr_resolver::{resolve_expr, ResolvedExpr},
        ty::{Ty, TyKind},
        ty_var_access_resolver::{CallSite, Place, ResolvedAccess},
    }, AstId, HirNodeInfo
};
use auto_lsp::default::db::BaseDatabase;

use crate::hir_def::expressions::expression::InitExpr;

#[salsa::tracked]
pub fn flatten<'db>(db: &'db dyn BaseDatabase, expr: InitExpr<'db>) -> UnResolvedInitExpr<'db> {
    let kind = match expr.kind(db) {
        InitExprKind::ArrayInit { values } => {
            let resolved = values.iter().map(|v| flatten(db, *v)).collect();
            UnResolvedInitExprKind::ArrayInit { values: resolved }
        }
        InitExprKind::ArrayIndexedElement { size, values } => {
            let resolved = values.iter().map(|v| flatten(db, *v)).collect();
            UnResolvedInitExprKind::ArrayIndexedElement {
                size,
                values: resolved,
            }
        }
        InitExprKind::StructInit { values } => {
            let resolved = values.iter().map(|v| flatten(db, *v)).collect();
            UnResolvedInitExprKind::StructInit { values: resolved }
        }
        InitExprKind::StructElement { name, value } => {
            let resolved = Box::new(flatten(db, *value));
            UnResolvedInitExprKind::StructElement {
                name,
                value: resolved,
            }
        }
        InitExprKind::ConstantExpr(expr) => {
            UnResolvedInitExprKind::ConstantExpr(expr)
        }
    };
    UnResolvedInitExpr { expr, kind }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct UnResolvedInitExpr<'db> {
    pub expr: InitExpr<'db>,
    pub kind: UnResolvedInitExprKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum UnResolvedInitExprKind<'db> {
    ArrayInit {
        values: Vec<UnResolvedInitExpr<'db>>,
    },
    ArrayIndexedElement {
        size: SpanIdent<'db>,
        values: Vec<UnResolvedInitExpr<'db>>,
    },
    StructInit {
        values: Vec<UnResolvedInitExpr<'db>>,
    },
    StructElement {
        name: SpanIdent<'db>,
        value: Box<UnResolvedInitExpr<'db>>,
    },
    ConstantExpr(Expr<'db>),
}

#[salsa::tracked(returns(ref))]
pub fn resolve_init_expr<'db>(
    db: &'db dyn BaseDatabase,
    ty: Ty<'db>,
    init: InitExpr<'db>,
) -> ResolvedInitExpr<'db> {
    let unordered = flatten(db, init);
    resolve_unresolved(db, ty, unordered)
}

fn resolve_unresolved<'db>(db: &'db dyn BaseDatabase, ty: Ty<'db>, init: UnResolvedInitExpr<'db>) -> ResolvedInitExpr<'db> {
    let kind = match init.kind {
        UnResolvedInitExprKind::ArrayInit { values } => {
            match ty.kind(db) {
                TyKind::Array(array) => {
                    // Valid: Array type with ArrayInit
                    let resolved = values
                        .into_iter()
                        .map(|v| resolve_unresolved(db, array.of_type.spec_to_ty(db), v))
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
                    TyKind::Array(array)=> resolve_unresolved(db, array.of_type.spec_to_ty(db), v),
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
                        let resolved_value = Box::new(resolve_unresolved(db, element_ty.spec(db).spec_to_ty(db), *value));
                        let field = ResolvedAccess::new(
                            db,
                            CallSite::Formal(name),
                            Place::StructElement(*element_ty),
                            vec![]
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
        UnResolvedInitExprKind::ConstantExpr(expr) => {
            ResolvedInitExprKind::ConstantExpr(resolve_expr(db, expr))
        }
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
    ConstantExpr(ResolvedExpr<'db>),
    Error(InitExprError<'db>),
}

impl<'db> HirNodeInfo<'db> for ResolvedInitExpr<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.expr(db).id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.expr(db).scope_id(db)
    }
}
