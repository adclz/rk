use crate::{
    check::errors::init_expr::InitExprError, hir_def::{
        expressions::expression::InitExprKind, interned::identifier::SpanIdent, scope::FileScopeId,
    }, hir_ty::{
        expr_resolver::{resolve_expr, ResolvedExpr},
        ty::{ty_for_struct_field, Ty, TyKind},
        ty_var_access_resolver::{ResolvedVarKind, ResolvedVarOrigin, ResolvedVarResult},
    }, AstId, HirNodeInfo
};
use auto_lsp::default::db::BaseDatabase;

use crate::hir_def::expressions::expression::InitExpr;

#[salsa::tracked(no_eq)]
pub fn order<'db>(db: &'db dyn BaseDatabase, expr: InitExpr<'db>) -> UnResolvedInitExpr<'db> {
    let kind = match expr.kind(db) {
        InitExprKind::ArrayInit { values } => {
            let resolved = values.iter().map(|v| order(db, *v)).collect();
            UnResolvedInitExprKind::ArrayInit { values: resolved }
        }
        InitExprKind::ArrayIndexedElement { size, values } => {
            let resolved = values.iter().map(|v| order(db, *v)).collect();
            UnResolvedInitExprKind::ArrayIndexedElement {
                size,
                values: resolved,
            }
        }
        InitExprKind::StructInit { values } => {
            let resolved = values.iter().map(|v| order(db, *v)).collect();
            UnResolvedInitExprKind::StructInit { values: resolved }
        }
        InitExprKind::StructElement { name, value } => {
            let resolved = Box::new(order(db, *value));
            UnResolvedInitExprKind::StructElement {
                name,
                value: resolved,
            }
        }
        InitExprKind::ConstantExpr(expr) => {
            UnResolvedInitExprKind::ConstantExpr(*resolve_expr(db, expr))
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
    ConstantExpr(ResolvedExpr<'db>),
}

#[salsa::tracked(no_eq, returns(ref))]
pub fn resolve_init_expr<'db>(
    db: &'db dyn BaseDatabase,
    ty: Ty<'db>,
    init: InitExpr<'db>,
) -> ResolvedInitExpr<'db> {
    let unordered = order(db, init);
    resolve_unresolved(db, ty, unordered)
}

fn resolve_unresolved<'db>(db: &'db dyn BaseDatabase, ty: Ty<'db>, init: UnResolvedInitExpr<'db>) -> ResolvedInitExpr<'db> {
    // Handle Target wrapper
    if let TyKind::Target(target) = ty.kind(db) {
        return resolve_unresolved(db, *target, init);
    }

    let kind = match init.kind {
        UnResolvedInitExprKind::ArrayInit { values } => {
            match ty.kind(db) {
                TyKind::Array { typ, .. } => {
                    // Valid: Array type with ArrayInit
                    let resolved = values
                        .into_iter()
                        .map(|v| resolve_unresolved(db, typ.spec_to_ty(db, ty.decl(db)), v))
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
                    TyKind::Array { typ, .. } => resolve_unresolved(db, typ.spec_to_ty(db, ty.decl(db)), v),
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
                TyKind::Struct { elements, .. } => {
                    if let Some(element_ty) = elements.get(&name) {
                        // Valid field - resolve with field type
                        let resolved_value = Box::new(resolve_unresolved(db, ty_for_struct_field(db, *element_ty), *value));
                        let field = ResolvedVarResult::new(
                            db,
                            ResolvedVarOrigin::Formal(name),
                            ResolvedVarKind::Param(ty_for_struct_field(db, *element_ty)),
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
            ResolvedInitExprKind::ConstantExpr(*resolve_expr(db, expr.expr(db)))
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
        field: ResolvedVarResult<'db>,
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
