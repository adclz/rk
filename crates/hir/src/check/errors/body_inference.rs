use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use db::WorkspaceDataBase;
use ide_diagnostic::{IdeDiagnostic, Related, diag};

use crate::{
    AstId, CallSite, HasName, HirNodeInfo,
    check::errors::{analysis_error::ToIdeDiagnostic, literals::InferLiteralError},
    hir_def::{
        expressions::{
            expression::{
                AddOperatorKind, BeginPathExpr, Expr, FuncCall, InitExpr, MultOperatorKind,
                ParamAssign, PathExpr,
            },
            spec::{Enum, Spec},
            statement::Stmt,
        },
        interned::identifier::{Ident, SpanIdent},
        pous::variable::VariableDecl,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body_inference::{Adjust, Adjustment},
        ty::{CallableType, Type},
    },
    query_string::{
        method::fuzzy_callable_type_parameters,
        variables::fuzzy_variables,
    },
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum BodyInferenceError<'db> {
    IsVarInput {
        var: VariableDecl<'db>,
        access: CallSite<'db>,
    },
    AssignCallableType {
        typ: CallableType<'db>,
        access: CallSite<'db>,
    },
    DirectType {
        typ: Type<'db>,
        expr: CallSite<'db>,
    },
    CallNonCallableType {
        typ: Type<'db>,
        func_call: FuncCall<'db>,
    },
    IncorrectNumberOfParameters {
        expected: usize,
        actual: usize,
        func_call: FuncCall<'db>,
        callable: CallableType<'db>,
    },
    UnknownNonFormalParameter {
        func: CallableType<'db>,
        expr: Expr<'db>,
        param: usize,
    },
    OutputParameterUsedAsInput {
        func: CallableType<'db>,
        var: VariableDecl<'db>,
        expr: Expr<'db>,
        param: usize,
    },
    UnknownInputParameter {
        func: CallableType<'db>,
        param: SpanIdent<'db>,
    },
    UnknownOutputParameter {
        func: CallableType<'db>,
        param: SpanIdent<'db>,
    },
    DuplicateParameter {
        param_1: ParamAssign<'db>,
        param_2: ParamAssign<'db>,
        name: Ident,
    },
    NoItemInScope {
        expr: PathExpr<'db>,
        scope: ScopeId<'db>,
    },
    NoSpecItemInScope {
        spec: Spec<'db>,
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
    NotAnEnum {
        expr: BeginPathExpr<'db>,
        item: Type<'db>,
    },
    EnumVariantNotFound {
        enum_: Enum<'db>,
        variant_name: SpanIdent<'db>,
    },
    InferLiteralError {
        expr: Expr<'db>,
        source: Option<CallSite<'db>>,
        target: Type<'db>,
        err: InferLiteralError,
    },
    TypeMismatch(TypeError<'db>),
}

impl<'db> From<TypeError<'db>> for BodyInferenceError<'db> {
    fn from(value: TypeError<'db>) -> Self {
        BodyInferenceError::TypeMismatch(value)
    }
}

impl<'db> ToIdeDiagnostic<'db> for BodyInferenceError<'db> {
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic {
        match self {
            Self::IsVarInput { var, access } => {
                
                diag()
                    .message(format!(
                        "{} is an input variable and can not be assigned",
                        var.get_name_ident(db).text(db)
                    ))
                    .range(access.get_span(db))
                    .call()
            }
            Self::AssignCallableType { typ, access } => {
                

                diag()
                    .message(format!(
                        "'{}' is a callable type and can not be assigned",
                        typ.get_name_ident(db).text(db)
                    ))
                    .range(access.get_span(db))
                    .call()
            }
            Self::DirectType { expr, typ } => diag()
                .message(format!(
                    "cannot use direct type '{}' here",
                    typ.type_name(db)
                ))
                .range(expr.get_span(db))
                .call(),
            Self::CallNonCallableType { typ, func_call } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is not a callable type",
                        typ.full_type_name(db)
                    ))
                    .range(func_call.path(db).get_span(db))
                    .call();

                if let Type::FunctionBlock(db) = typ {
                    diag.with_note(
                        "to call a FUNCTION_BLOCK, you need to instantiate it first.".into(),
                    );
                }

                diag
            }
            Self::IncorrectNumberOfParameters {
                expected,
                actual,
                func_call,
                callable,
            } => {
                

                diag()
                    .message(format!(
                        "'{}' expects {} parameter{}, but got {}",
                        callable.get_name_ident(db).text(db),
                        expected,
                        match expected {
                            1 => "",
                            _ => "s",
                        },
                        actual
                    ))
                    .range(func_call.path(db).get_span(db))
                    .call()
            }
            Self::UnknownNonFormalParameter { func, expr, param } => {
                

                diag()
                    .message(format!("no parameter at index '{}'", param))
                    .range(expr.get_span(db))
                    .call()
            }
            Self::UnknownInputParameter { func, param } => {
                let mut diag = diag()
                    .message(format!("unknown input parameter '{}'", param.text(db)))
                    .range(param.get_span(db))
                    .call();

                fuzzy_callable_type_parameters(db, *func, &mut diag, param.text(db).as_str());

                diag
            }
            Self::UnknownOutputParameter { func, param } => {
                let mut diag = diag()
                    .message(format!("unknown output parameter '{}'", param.text(db)))
                    .range(param.get_span(db))
                    .call();

                fuzzy_callable_type_parameters(db, *func, &mut diag, param.text(db).as_str());

                diag
            }
            Self::DuplicateParameter {
                param_1,
                param_2,
                name,
            } => {
                let mut diag = diag()
                    .message(format!("duplicate parameter '{}' found", name.text(db)))
                    .range(param_2.get_span(db))
                    .call();

                diag.with_related(Related::new(
                    "previously defined here".to_string(),
                    param_1.get_scope_id(db).file(db),
                    param_1.get_span(db),
                ));

                diag
            }
            Self::OutputParameterUsedAsInput {
                func,
                expr,
                var,
                param,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "output parameter at index '{}' cannot be used as input",
                        param
                    ))
                    .range(expr.get_span(db))
                    .call();
                diag.with_note(format!(
                    "use formal syntax instead: {} => <variable>",
                    var.get_name_ident(db).text(db)
                ));

                diag
            }
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
            Self::NoItemInScope { expr, scope } => {
                let mut diag = diag()
                    .message(format!(
                        "no item {:?} found in scope",
                        expr.ident(db).text(db)
                    ))
                    .range(expr.get_span(db))
                    .call();

                if let ScopeKind::Pou(pou) = get_scope(db, *scope).kind {
                    fuzzy_variables(db, pou, &mut diag, expr.ident(db).as_str(db))
                }
                diag
            }
            Self::NoSpecItemInScope { spec, scope } => {
                

                diag()
                    .message("no item found in scope".to_string())
                    .range(spec.get_span(db))
                    .call()
            }
            Self::NoSuchField { expr, ident, ty } => diag()
                .message(format!(
                    "'{}' has no field named '{}'",
                    ty.with_name(db).unwrap_or_else(|| ty.full_type_name(db)),
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
            Self::NotAnEnum { expr, item } => diag()
                .message(format!("'{}' is not an ENUM type", item.full_type_name(db)))
                .range(expr.get_span(db))
                .call(),
            Self::EnumVariantNotFound {
                enum_,
                variant_name,
            } => diag()
                .message(format!(
                    "ENUM has no variant named '{}'",
                    variant_name.text(db)
                ))
                .range(variant_name.get_span(db))
                .call(),
            Self::InferLiteralError {
                err,
                expr,
                source,
                target,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "cannot infer '{}' to '{}': {}",
                        expr.to_string(db),
                        target.full_type_name(db),
                        err.to_string()
                    ))
                    .range(expr.get_span(db))
                    .call();

                if let Some(source) = source {
                    diag.with_related(Related::new(
                        format!("'{}' is expected due to this", target.full_type_name(db)),
                        source.get_scope_id(db).file(db),
                        source.get_span(db),
                    ));
                }

                target.with_location(db, &mut diag);

                diag
            }
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
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        match self {
            InitOrExpr::Expr(expr) => expr.get_id(db),
            InitOrExpr::InitExpr(expr) => expr.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        match self {
            InitOrExpr::Expr(expr) => expr.get_scope_id(db),
            InitOrExpr::InitExpr(expr) => expr.get_scope_id(db),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum TypeError<'db> {
    NotAssignable {
        base_target: Type<'db>,
        lhs: Type<'db>,
        rhs: Type<'db>,
        adjustment: Option<Adjustment<'db>>,
        expr: CallSite<'db>,
    },
    NotComparable {
        base_target: Type<'db>,
        lhs: Type<'db>,
        rhs: Type<'db>,
        adjustment: Option<Adjustment<'db>>,
        expr: CallSite<'db>,
    },
    NotMultiplicable {
        base_target: Type<'db>,
        operator: MultOperatorKind,
        lhs: Type<'db>,
        rhs: Type<'db>,
        adjustment: Option<Adjustment<'db>>,
        expr: CallSite<'db>,
    },
    NotAddable {
        base_target: Type<'db>,
        operator: AddOperatorKind,
        lhs: Type<'db>,
        rhs: Type<'db>,
        adjustment: Option<Adjustment<'db>>,
        expr: CallSite<'db>,
    },
    NotPowerable {
        base_target: Type<'db>,
        lhs: Type<'db>,
        rhs: Type<'db>,
        adjustment: Option<Adjustment<'db>>,
        expr: CallSite<'db>,
    },
    NotABoolean {
        typ: Type<'db>,
        expr: Expr<'db>,
    },
    UnusedReturnType {
        typ: Type<'db>,
        expr: Stmt<'db>,
    },
    Other {
        message: String,
        expr: Expr<'db>,
    },
}

impl<'db> ToIdeDiagnostic<'db> for TypeError<'db> {
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic {
        match self {
            Self::NotAssignable {
                base_target,
                lhs: target,
                rhs: value,
                adjustment,
                expr,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "expected '{}', got '{}'",
                        target.full_type_name(db),
                        adjustment_to_string(db, *value, adjustment),
                    ))
                    .range(expr.get_span(db))
                    .call();

                base_target.with_location(db, &mut diag);
                diag
            }
            Self::NotComparable {
                base_target,
                lhs,
                rhs,
                expr,
                adjustment,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "can't compare '{}' with '{}'",
                        lhs.full_type_name(db),
                        adjustment_to_string(db, *rhs, adjustment),
                    ))
                    .range(expr.get_span(db))
                    .call();

                base_target.with_location(db, &mut diag);
                diag
            }
            Self::NotAddable {
                base_target,
                lhs,
                operator,
                rhs,
                expr,
                adjustment,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "can not {} '{}' with '{}'",
                        match operator {
                            AddOperatorKind::Plus => "add",
                            AddOperatorKind::Minus => "subtract",
                        },
                        lhs.full_type_name(db),
                        adjustment_to_string(db, *rhs, adjustment)
                    ))
                    .range(expr.get_span(db))
                    .call();

                base_target.with_location(db, &mut diag);
                diag
            }
            Self::NotMultiplicable {
                base_target,
                lhs,
                operator,
                rhs,
                expr,
                adjustment,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "can not {} '{}' with '{}'",
                        match operator {
                            MultOperatorKind::Mul => "multiply",
                            MultOperatorKind::Div => "divide",
                            MultOperatorKind::Mod => "modulus",
                        },
                        rhs.full_type_name(db),
                        adjustment_to_string(db, *rhs, adjustment)
                    ))
                    .range(expr.get_span(db))
                    .call();

                base_target.with_location(db, &mut diag);
                diag
            }
            Self::NotPowerable {
                base_target,
                lhs,
                rhs,
                adjustment,
                expr,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "can not power '{}' with '{}'",
                        lhs.full_type_name(db),
                        adjustment_to_string(db, *rhs, adjustment)
                    ))
                    .range(expr.get_span(db))
                    .call();

                base_target.with_location(db, &mut diag);
                diag
            }
            Self::NotABoolean { typ, expr } => diag()
                .message(format!(
                    "expected a boolean, got {}",
                    typ.full_type_name(db)
                ))
                .range(expr.get_span(db))
                .call(),
            Self::UnusedReturnType { typ, expr } => diag()
                .message(format!(
                    "unused return value of '{}'",
                    typ.full_type_name(db)
                ))
                .range(expr.get_span(db))
                .severity(DiagnosticSeverity::INFORMATION)
                .call(),
            Self::Other { message, expr } => diag()
                .message(message.clone())
                .range(expr.get_span(db))
                .call(),
        }
    }
}

fn adjustment_to_string(db: &dyn WorkspaceDataBase, value: Type, adj: &Option<Adjustment>) -> String {
    match adj {
        Some(adj) => match adj.kind {
            Adjust::Ref => {
                format!("REF TO {}", adj.target.full_type_name(db))
            }
            Adjust::Deref => {
                format!("DEREF {}", adj.target.full_type_name(db))
            }
            Adjust::Index => {
                format!("INDEX {}", adj.target.full_type_name(db))
            }
        },
        None => value.full_type_name(db),
    }
}
