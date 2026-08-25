use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

use crate::{
    HasName, HirNodeInfo,
    check::errors::ToIdeDiagnostic,
    hir_def::{
        expressions::{
            expression::{InitExpr, ParamAssign},
            spec::StructElement,
        },
        interned::identifier::{Ident, SpanIdent},
        pous::{class::MethodDecl, interface::MethodPrototype, pou::Pou, variable::VariableDecl},
        program::ProgramDecl,
        using::Using,
    },
    hir_ty::head::inheritance::InheritedMethod,
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum DuplicateError<'db> {
    Pou {
        pou1: Pou<'db>,
        pou2: Pou<'db>,
    },
    Variable {
        var1: VariableDecl<'db>,
        var2: VariableDecl<'db>,
    },
    /// A variable named like the enclosing callable. Inside a FUNCTION or
    /// METHOD that name IS the return value, so the local is a second
    /// declaration of it — one the body then binds to, at the local's type,
    /// while the signature still promises the return type's.
    VariableIsReturnValue {
        var: VariableDecl<'db>,
        pou_kind: &'static str,
    },
    StructField {
        field1: StructElement<'db>,
        field2: StructElement<'db>,
    },
    EnumVariant {
        variant1: SpanIdent<'db>,
        variant2: SpanIdent<'db>,
    },
    MethodDecl {
        method1: MethodDecl<'db>,
        method2: MethodDecl<'db>,
    },
    MethodProt {
        method1: MethodPrototype<'db>,
        method2: MethodPrototype<'db>,
    },
    InheritedMethod {
        method1: InheritedMethod<'db>,
        method2: InheritedMethod<'db>,
    },
    Parameter {
        param_1: ParamAssign<'db>,
        param_2: ParamAssign<'db>,
        name: Ident,
    },
    Using {
        using: Using<'db>,
        other: Using<'db>,
    },
    InitExprField {
        name: Ident,
        field1: InitExpr<'db>,
        field2: InitExpr<'db>,
    },
    Program {
        prog1: ProgramDecl<'db>,
        prog2: ProgramDecl<'db>,
    },
    /// Duplicate TASK name within the same configuration or resource scope.
    Task {
        task1: SpanIdent<'db>,
        task2: SpanIdent<'db>,
    },
    /// Duplicate PROGRAM instance name within the same configuration or resource scope.
    ProgInstance {
        prog1: SpanIdent<'db>,
        prog2: SpanIdent<'db>,
    },
    /// Duplicate RESOURCE name within the same configuration.
    Resource {
        res1: SpanIdent<'db>,
        res2: SpanIdent<'db>,
    },
}

impl ErrorCode for DuplicateError<'_> {
    fn code(&self) -> &'static str {
        match self {
            Self::Pou { .. } => "E0101",
            Self::Variable { .. } => "E0102",
            Self::VariableIsReturnValue { .. } => "E0117",
            Self::StructField { .. } => "E0103",
            Self::EnumVariant { .. } => "E0104",
            Self::MethodDecl { .. } => "E0105",
            Self::MethodProt { .. } => "E0106",
            Self::InheritedMethod { .. } => "E0107",
            Self::Parameter { .. } => "E0108",
            Self::Using { .. } => "E0109",
            Self::InitExprField { .. } => "E0110",
            Self::Program { .. } => "E0111",
            Self::Task { .. } => "E0114",
            Self::ProgInstance { .. } => "E0115",
            Self::Resource { .. } => "E0116",
        }
    }

    fn description(&self) -> &'static str {
        "duplicate definitions"
    }
}

impl<'db> ToIdeDiagnostic<'db> for DuplicateError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            Self::Pou { pou1, pou2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate POU '{}'",
                        pou1.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &pou1.get_name_span(db)).unwrap_or_default(),
                    )
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "POU '{}' is already defined here",
                        pou2.get_name_ident(db).text(db)
                    ),
                    pou2.get_scope_id(db).file(db),
                    pou2.get_name_span(db),
                ));

                diag
            }
            Self::VariableIsReturnValue { var, pou_kind } => diag()
                .message(format!(
                    "variable '{}' is the {pou_kind}'s return value",
                    var.get_name_ident(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &var.get_name_span(db)).unwrap_or_default())
                .call(),
            Self::Variable { var1, var2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate variable '{}'",
                        var1.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &var1.get_name_span(db)).unwrap_or_default(),
                    )
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "variable '{}' is already defined here",
                        var2.get_name_ident(db).text(db)
                    ),
                    var2.get_scope_id(db).file(db),
                    var2.get_name_span(db),
                ));

                diag
            }
            Self::EnumVariant { variant1, variant2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate enum variant '{}'",
                        variant1.ident.text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &variant1.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "enum variant '{}' is already defined here",
                        variant2.ident.text(db)
                    ),
                    variant2.get_scope_id(db).file(db),
                    variant2.get_span(db),
                ));

                diag
            }
            Self::StructField { field1, field2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate field '{}'",
                        field1.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &field1.get_name_span(db)).unwrap_or_default(),
                    )
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "field '{}' is already defined here",
                        field2.get_name_ident(db).text(db)
                    ),
                    field2.get_scope_id(db).file(db),
                    field2.get_name_span(db),
                ));

                diag
            }
            Self::MethodDecl { method1, method2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate method '{}'",
                        method1.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &method1.get_name_span(db))
                            .unwrap_or_default(),
                    )
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "method '{}' is already defined here",
                        method2.get_name_ident(db).text(db)
                    ),
                    method2.get_scope_id(db).file(db),
                    method2.get_name_span(db),
                ));

                diag
            }
            Self::MethodProt { method1, method2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate method '{}'",
                        method1.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &method1.get_name_span(db))
                            .unwrap_or_default(),
                    )
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "method '{}' is already defined here",
                        method2.get_name_ident(db).text(db)
                    ),
                    method2.get_scope_id(db).file(db),
                    method2.get_name_span(db),
                ));

                diag
            }
            Self::InheritedMethod { method1, method2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate method '{}'",
                        method1.method.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &method1.method.get_name_span(db))
                            .unwrap_or_default(),
                    )
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "method '{}' is already defined here",
                        method2.method.get_name_ident(db).text(db)
                    ),
                    method2.method.get_scope_id(db).file(db),
                    method2.method.get_name_span(db),
                ));

                diag.with_note(format!(
                    "this error happens because both interfaces '{}' and '{}' define a method '{}'",
                    method1.source.get_name_ident(db).text(db),
                    method2.source.get_name_ident(db).text(db),
                    method1.method.get_name_ident(db).text(db)
                ));

                diag
            }
            Self::Parameter {
                param_1,
                param_2,
                name,
            } => {
                let mut diag = diag()
                    .message(format!("duplicate parameter '{}' found", name.text(db)))
                    .range(crate::denormalize(db, file, &param_2.get_span(db)).unwrap_or_default())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .call();

                diag.with_related(Related::new(
                    "previously defined here".to_string(),
                    param_1.get_scope_id(db).file(db),
                    param_1.get_span(db),
                ));

                diag
            }
            Self::Using { using, other } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate `USING` for namespace '{}'",
                        using.path(db).to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &using.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "namespace '{}' is already imported here",
                        other.path(db).to_string(db)
                    ),
                    other.scope_id(db).file(db),
                    other.get_span(db),
                ));

                diag
            }
            Self::InitExprField {
                name,
                field1,
                field2,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate field '{}' in initializer expression",
                        name.text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &field1.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!("field '{}' is already initialized here", name.text(db)),
                    field2.get_scope_id(db).file(db),
                    field2.get_span(db),
                ));

                diag
            }
            Self::Program { prog1, prog2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate program '{}'",
                        prog1.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &prog1.get_name_span(db)).unwrap_or_default(),
                    )
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "program '{}' is already defined here",
                        prog2.get_name_ident(db).text(db)
                    ),
                    prog2.get_scope_id(db).file(db),
                    prog2.get_name_span(db),
                ));

                diag
            }
            Self::Task { task1, task2 } => {
                let mut diag = diag()
                    .message(format!("duplicate task '{}'", task1.ident.text(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &task1.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!("task '{}' is already defined here", task2.ident.text(db)),
                    task2.get_scope_id(db).file(db),
                    task2.get_span(db),
                ));

                diag
            }
            Self::ProgInstance { prog1, prog2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate program instance '{}'",
                        prog1.ident.text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &prog1.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "program instance '{}' is already defined here",
                        prog2.ident.text(db)
                    ),
                    prog2.get_scope_id(db).file(db),
                    prog2.get_span(db),
                ));

                diag
            }
            Self::Resource { res1, res2 } => {
                let mut diag = diag()
                    .message(format!("duplicate resource '{}'", res1.ident.text(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &res1.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!("resource '{}' is already defined here", res2.ident.text(db)),
                    res2.get_scope_id(db).file(db),
                    res2.get_span(db),
                ));

                diag
            }
        }
    }
}
