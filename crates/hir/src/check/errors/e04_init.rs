use crate::CallSite;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::expressions::expression::Expr;
use crate::hir_def::expressions::expression::ExprKind;
use crate::hir_def::expressions::expression::InitExpr;
use crate::hir_def::expressions::expression::PrimaryExpr;
use crate::hir_def::pous::pou::Pou;
use crate::hir_def::scope::ScopeKind;
use crate::hir_def::semantic_index::get_scope;
use crate::hir_ty::infer::const_eval;
use crate::hir_ty::ty::Type;
use auto_lsp::lsp_types::DiagnosticSeverity;
use auto_lsp::tree_sitter::Range;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::Related;
use ide_diagnostic::diag;

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum InitError<'db> {
    /// A once-per-type initializer (a TYPE default, an FB/CLASS member
    /// default, a static PROGRAM field or config global) referenced something
    /// with no compile-time value. Before this code the leaf was silently
    /// DROPPED: the slot read zero from source the check called clean.
    InitNotConstant {
        value: Expr<'db>,
    },
    FunctionCallInInitExpression(Range),
    NoFieldOnElementaryType {
        expr: InitExpr<'db>,
        ty: Type<'db>,
    },
    AssignToConstant {
        access: CallSite<'db>,
    },
    /// An instance's initializer names a member that has no value of its own
    /// for it to give.
    UninitializableMember {
        expr: InitExpr<'db>,
        /// The member, pointed at where it is declared.
        var: crate::hir_def::pous::variable::VariableDecl<'db>,
        kind: UninitializableMember,
    },
}

/// A member an instance's initializer names but cannot set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum UninitializableMember {
    /// Each call binds it to its argument. The value was written into the
    /// pointer the binding lives in, and `__init` failed to validate.
    InOut,
    /// Each call makes it afresh. The value was silently dropped.
    Temp,
    /// It names a VAR_GLOBAL. The value was silently dropped.
    External,
    /// Its reads fold to its declared value, while `__init` wrote this one.
    Constant,
}

impl<'db> ErrorCode for InitError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::InitNotConstant { .. } => "E0401",
            Self::FunctionCallInInitExpression(_) => "E0402",
            Self::NoFieldOnElementaryType { .. } => "E0403",
            Self::AssignToConstant { .. } => "E0404",
            Self::UninitializableMember { .. } => "E0405",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::InitNotConstant { .. } => "initial value is not constant",
            Self::FunctionCallInInitExpression(_) => "syntax",
            Self::NoFieldOnElementaryType { .. } => "invalid operation",
            Self::AssignToConstant { .. } => "semantic violation",
            Self::UninitializableMember { .. } => "member cannot be initialized",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for InitError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            Self::InitNotConstant { value } => {
                let mut d = diag()
                    .message(
                        "this initial value must be a constant: it is fixed before the program runs"
                            .to_string(),
                    )
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &value.get_span(db)).unwrap_or_default())
                    .call();
                // Say WHY when the refused thing is a bare name — especially
                // when it IS a constant, just not one this scope can fold.

                if let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) = value.expr(db) {
                    use crate::Qualifier;
                    match const_eval::spec_name_binding(db, *va) {
                        Some(decl) if decl.qualifier(db).contains(Qualifier::CONSTANT) => {
                            // Three reasons a CONSTANT still refuses, told apart
                            // so the advice is not a catch-all.
                            if decl.init(db).is_none() {
                                d.with_note(format!(
                                    "'{}' is CONSTANT but declares no initial value, \
                                     so there is nothing to fold",
                                    decl.name(db).text(db)
                                ));
                            } else {
                                d.with_note(format!(
                                    "'{}' is CONSTANT, but its own value does not fold \
                                     (a reference cycle, or a non-constant initializer)",
                                    decl.name(db).text(db)
                                ));
                            }
                        }
                        Some(decl) => {
                            d.with_note(format!(
                                "'{}' is an ordinary variable; declare it CONSTANT \
                                 if its value never changes",
                                decl.name(db).text(db)
                            ));
                        }
                        None => {
                            if let Some(ident) = const_eval::bare_access_name(db, *va)
                                && let Some(global) =
                                    crate::hir_ty::index_graphs::external_var_lookup(db, ident)
                            {
                                if !global.qualifier(db).contains(Qualifier::CONSTANT) {
                                    d.with_note(format!(
                                        "'{}' is an ordinary variable; declare it CONSTANT \
                                         if its value never changes",
                                        ident.text(db)
                                    ));
                                    return d;
                                }
                                let in_type = matches!(
                                    get_scope(db, value.get_scope_id(db)).kind,
                                    ScopeKind::Pou(Pou::DataType(_))
                                );
                                if in_type {
                                    d.with_note(format!(
                                        "'{}' IS a CONSTANT, but a TYPE declaration cannot \
                                         see it: a TYPE default folds only literals, \
                                         arithmetic, and constants in its own scope",
                                        ident.text(db)
                                    ));
                                } else {
                                    d.with_note(format!(
                                        "'{}' IS a CONSTANT: declare it in this POU as \
                                         `VAR_EXTERNAL CONSTANT` and the reference folds",
                                        ident.text(db)
                                    ));
                                }
                            }
                        }
                    }
                }
                d
            }
            Self::FunctionCallInInitExpression(span) => diag()
                .message("function call in initialization expression is not allowed".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::NoFieldOnElementaryType { expr, ty } => ide_diagnostic::diag()
                .message(format!(
                    "type '{}' is an elementary type and cannot be initiliazed with '()'",
                    ty.type_name(db)
                ))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::AssignToConstant { access } => diag()
                .message("cannot assign to constant type".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &access.get_span(db)).unwrap_or_default())
                .call(),
            Self::UninitializableMember { expr, var, kind } => {
                use crate::HasName;
                let name = var.get_name_ident(db).text(db);
                let (what, note) = match kind {
                    UninitializableMember::InOut => (
                        "a VAR_IN_OUT, which each call binds to its argument",
                        format!("pass the variable in the call instead, as '{name} := x'"),
                    ),
                    UninitializableMember::Temp => (
                        "a VAR_TEMP, which each call makes afresh",
                        "give the value in its declaration, which applies at every call"
                            .to_string(),
                    ),
                    UninitializableMember::External => (
                        "a VAR_EXTERNAL, which names a VAR_GLOBAL",
                        "give the value in the VAR_GLOBAL's declaration".to_string(),
                    ),
                    UninitializableMember::Constant => (
                        "CONSTANT, whose value is its declaration's",
                        "declare it without CONSTANT to let each instance start at its own value"
                            .to_string(),
                    ),
                };
                let mut d = diag()
                    .message(format!(
                        "'{name}' is {what}, so an initializer cannot give it a value"
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                d.with_related(Related::new(
                    format!("'{name}' is declared here"),
                    var.get_scope_id(db).file(db),
                    var.get_name_span(db),
                ));
                d.with_note(note);
                d
            }
        }
    }
}
