use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::{IdeDiagnostic, Related, diag};

use crate::{
    AstId, HirNodeInfo,
    check::errors::analysis_error::ToIdeDiagnostic,
    hir_def::{
        expressions::{
            expression::{AddOperatorKind, Expr, InitExpr, MultOperatorKind, PathExpr},
            statement::Stmt,
        },
        interned::identifier::Ident,
        scope::ScopeId,
    },
    hir_ty::{
        body_inference::{BodyInferenceResult, infer_body_scope}, infer::ctx::CallSite, init_inference::infer_init_expr, ty::Type
    },
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum BodyInferenceError<'db> {
    NoItemInScope {
        expr: PathExpr<'db>,
        scope: ScopeId<'db>,
    },
    NoSuchField {
        expr: PathExpr<'db>,
        ident: Ident,
        ty: Type<'db>,
    },
    DerefNonRefType {
        expr: PathExpr<'db>,
        ty: Type<'db>,
    },
    IndexNonArrayType {
        expr: PathExpr<'db>,
        ty: Type<'db>,
    },
    SuperBodyOnIncompatiblePou {
        call_site: CallSite<'db>,
    },
    SuperOnIncompatiblePou {
        call_site: CallSite<'db>,
    },
    ThisOnIncompatiblePou {
        call_site: CallSite<'db>,
    },
    ContinueOutsideLoop {
        stmt: Stmt<'db>,
    },
    ExitOutsideLoop {
        stmt: Stmt<'db>,
    },
    TypeMismatch(TypeError<'db>),
}

impl<'db> From<TypeError<'db>> for BodyInferenceError<'db> {
    fn from(value: TypeError<'db>) -> Self {
        BodyInferenceError::TypeMismatch(value)
    }
}

impl<'db> ToIdeDiagnostic<'db> for BodyInferenceError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::SuperBodyOnIncompatiblePou { call_site } => diag()
                .message("'SUPER()' is not valid in this context".to_string())
                .range(call_site.get_span(db))
                .call(),
            Self::SuperOnIncompatiblePou { call_site } => diag()
                .message("'SUPER' is not valid in this context".to_string())
                .range(call_site.get_span(db))
                .call(),
            Self::ThisOnIncompatiblePou { call_site } => diag()
                .message("'THIS' is not valid in this context".to_string())
                .range(call_site.get_span(db))
                .call(),
            Self::ContinueOutsideLoop { stmt } => diag()
                .message("'CONTINUE' can only be used inside loops".to_string())
                .range(stmt.get_span(db))
                .call(),
            Self::ExitOutsideLoop { stmt } => diag()
                .message("'EXIT' can only be used inside loops".to_string())
                .range(stmt.get_span(db))
                .call(),
            Self::NoItemInScope { expr, scope } => diag()
                .message(format!("no item found in scope",))
                .range(expr.get_span(db))
                .call(),
            Self::NoSuchField { expr, ident, ty } => diag()
                .message(format!(
                    "'{}' has no field named '{}'",
                    ty.full_type_name(db),
                    ident.text(db)
                ))
                .range(expr.get_span(db))
                .call(),
            Self::DerefNonRefType { expr, ty } => diag()
                .message(format!(
                    "cannot dereference non-reference type '{}'",
                    ty.full_type_name(db)
                ))
                .range(expr.get_span(db))
                .call(),
            Self::IndexNonArrayType { expr, ty } => diag()
                .message(format!(
                    "cannot index non-array type '{}'",
                    ty.full_type_name(db)
                ))
                .range(expr.get_span(db))
                .call(),
            Self::TypeMismatch(mismatch) => mismatch.to_diagnostic(db),
        }
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, salsa::Update, salsa::Supertype)]
pub enum InitOrExpr<'db> {
    Expr(Expr<'db>),
    InitExpr(InitExpr<'db>),
}

impl<'db> From<Expr<'db>> for InitOrExpr<'db> {
    fn from(value: Expr<'db>) -> Self {
        InitOrExpr::Expr(value)
    }
}

impl<'db> From<InitExpr<'db>> for InitOrExpr<'db> {
    fn from(value: InitExpr<'db>) -> Self {
        InitOrExpr::InitExpr(value)
    }
}

impl<'db> HirNodeInfo<'db> for InitOrExpr<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        match self {
            InitOrExpr::Expr(expr) => expr.get_id(db),
            InitOrExpr::InitExpr(expr) => expr.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        match self {
            InitOrExpr::Expr(expr) => expr.get_scope_id(db),
            InitOrExpr::InitExpr(expr) => expr.get_scope_id(db),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum TypeError<'db> {
    NotAssignable {
        target: Type<'db>,
        value: Type<'db>,
        expr: InitOrExpr<'db>,
    },
    NotComparable {
        lhs: Type<'db>,
        rhs: Type<'db>,
        expr: Expr<'db>,
    },
    NotMultiplicable {
        operator: MultOperatorKind,
        lhs: Type<'db>,
        rhs: Type<'db>,
        expr: Expr<'db>,
    },
    NotAddable {
        operator: AddOperatorKind,
        lhs: Type<'db>,
        rhs: Type<'db>,
        expr: Expr<'db>,
    },
    NotABoolean {
        typ: Type<'db>,
        expr: Expr<'db>,
    },
    Other {
        message: String,
        expr: Expr<'db>,
    },
}

impl<'db> ToIdeDiagnostic<'db> for TypeError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::NotAssignable {
                target,
                value,
                expr,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "expected '{}', got '{}'",
                        target.full_type_name(db),
                        value.full_type_name(db),
                    ))
                    .range(expr.get_span(db))
                    .call();

                match expr {
                    InitOrExpr::Expr(expr) => {
                        let inference = infer_body_scope(db, expr.get_scope_id(db));

                        if let Some(var) = inference.variable_for_type(*target) {
                            diag.with_related(Related::new(
                                format!(
                                    "type is declared by variable '{}' here",
                                    var.name(db).text(db)
                                ),
                                var.get_scope_id(db).file(db),
                                var.get_span(db),
                            ));
                        }
                    },
                    InitOrExpr::InitExpr(init) => {
                        let inference = infer_init_expr(db,  *target, *init);

                        /*if let Some(var) = inference.t(*target) {
                            diag.with_related(Related::new(
                                format!(
                                    "type is declared by variable '{}' here",
                                    var.name(db).text(db)
                                ),
                                var.get_scope_id(db).file(db),
                                var.get_span(db),
                            ));
                        }*/
                    }
                }

                diag
            }
            Self::NotComparable { lhs, rhs, expr } => diag()
                .message(format!(
                    "can't compare '{}' with '{}'",
                    lhs.full_type_name(db),
                    rhs.full_type_name(db)
                ))
                .range(expr.get_span(db))
                .call(),
            Self::NotAddable {
                lhs,
                operator,
                rhs,
                expr,
            } => diag()
                .message(format!(
                    "can not {} '{}' with '{}'",
                    rhs.full_type_name(db),
                    match operator {
                        AddOperatorKind::Plus => "add",
                        AddOperatorKind::Minus => "subtract",
                    },
                    lhs.full_type_name(db)
                ))
                .range(expr.get_span(db))
                .call(),
            Self::NotMultiplicable {
                lhs,
                operator,
                rhs,
                expr,
            } => diag()
                .message(format!(
                    "can not {} '{}' with '{}'",
                    match operator {
                        MultOperatorKind::Mul => "multiply",
                        MultOperatorKind::Div => "divide",
                        MultOperatorKind::Mod => "modulus",
                    },
                    rhs.full_type_name(db),
                    lhs.full_type_name(db)
                ))
                .range(expr.get_span(db))
                .call(),
            Self::NotABoolean { typ, expr } => diag()
                .message(format!(
                    "expected a boolean, got {}",
                    typ.full_type_name(db)
                ))
                .range(expr.get_span(db))
                .call(),
            Self::Other { message, expr } => diag()
                .message(message.clone())
                .range(expr.get_span(db))
                .call(),
        }
    }
}
