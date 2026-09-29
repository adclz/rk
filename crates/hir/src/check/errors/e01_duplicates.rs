use crate::HasName;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::expressions::expression::InitExpr;
use crate::hir_def::expressions::expression::ParamAssign;
use crate::hir_def::expressions::spec::StructElement;
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::pous::class::MethodDecl;
use crate::hir_def::pous::interface::MethodPrototype;
use crate::hir_def::pous::pou::Pou;
use crate::hir_def::pous::variable::VariableDecl;
use crate::hir_def::program::ProgramDecl;
use crate::hir_def::using::Using;
use crate::hir_ty::head::inheritance::InheritedMethod;
use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::Related;
use ide_diagnostic::diag;

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum DuplicateError<'db> {
    Variable {
        var1: VariableDecl<'db>,
        var2: VariableDecl<'db>,
    },
    Pou {
        pou1: Pou<'db>,
        pou2: Pou<'db>,
    },
    Parameter {
        param_1: ParamAssign<'db>,
        param_2: ParamAssign<'db>,
        /// As the second one writes it.
        name: Ident,
    },
    StructField {
        field1: StructElement<'db>,
        field2: StructElement<'db>,
    },
    EnumVariant {
        variant1: SpanIdent<'db>,
        variant2: SpanIdent<'db>,
    },
    InitExprField {
        field1: InitExpr<'db>,
        field2: InitExpr<'db>,
        /// As the second one writes it.
        name: Ident,
    },
    /// A variable named like the enclosing callable. Inside a FUNCTION or
    /// METHOD that name IS the return value, so the local is a second
    /// declaration of it — one the body then binds to, at the local's type,
    /// while the signature still promises the return type's.
    VariableIsReturnValue {
        var: VariableDecl<'db>,
        pou_kind: &'static str,
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
    Using {
        using: Using<'db>,
        other: Using<'db>,
    },
    Program {
        prog1: ProgramDecl<'db>,
        prog2: ProgramDecl<'db>,
    },
    /// A PROGRAM and another POU of the same name: a configuration naming it
    /// could reach either, and an FB's body would share the program body's
    /// symbol. Reported at both, at the PROGRAM when `at_program`.
    ProgramPou {
        program: ProgramDecl<'db>,
        pou: Pou<'db>,
        at_program: bool,
    },
    /// Duplicate PROGRAM instance name within the same configuration or resource scope.
    ProgInstance {
        prog1: SpanIdent<'db>,
        prog2: SpanIdent<'db>,
    },
    /// A program instance and a VAR_GLOBAL of the configuration share a
    /// name, which both root a path in the debug symbols. Reported at both,
    /// at the instance when `at_instance`.
    InstanceGlobal {
        instance: SpanIdent<'db>,
        global: VariableDecl<'db>,
        at_instance: bool,
    },
    /// Duplicate TASK name within the same configuration or resource scope.
    Task {
        task1: SpanIdent<'db>,
        task2: SpanIdent<'db>,
    },
    /// Duplicate RESOURCE name within the same configuration.
    Resource {
        res1: SpanIdent<'db>,
        res2: SpanIdent<'db>,
    },
}

impl<'db> ErrorCode for DuplicateError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::Variable { .. } => "E0101",
            Self::Pou { .. } => "E0102",
            Self::ProgramPou { .. } => "E0102",
            Self::Parameter { .. } => "E0103",
            Self::StructField { .. } => "E0104",
            Self::EnumVariant { .. } => "E0105",
            Self::InitExprField { .. } => "E0106",
            Self::VariableIsReturnValue { .. } => "E0107",
            Self::MethodDecl { .. } => "E0108",
            Self::MethodProt { .. } => "E0109",
            Self::InheritedMethod { .. } => "E0110",
            Self::Using { .. } => "E0111",
            Self::Program { .. } => "E0112",
            Self::ProgInstance { .. } => "E0113",
            Self::InstanceGlobal { .. } => "E0113",
            Self::Task { .. } => "E0114",
            Self::Resource { .. } => "E0115",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::Variable { .. } => "duplicate definitions",
            Self::Pou { .. } => "duplicate definitions",
            Self::ProgramPou { .. } => "duplicate definitions",
            Self::Parameter { .. } => "duplicate definitions",
            Self::StructField { .. } => "duplicate definitions",
            Self::EnumVariant { .. } => "duplicate definitions",
            Self::InitExprField { .. } => "duplicate definitions",
            Self::VariableIsReturnValue { .. } => "duplicate definitions",
            Self::MethodDecl { .. } => "duplicate definitions",
            Self::MethodProt { .. } => "duplicate definitions",
            Self::InheritedMethod { .. } => "duplicate definitions",
            Self::Using { .. } => "duplicate definitions",
            Self::Program { .. } => "duplicate definitions",
            Self::ProgInstance { .. } => "duplicate definitions",
            Self::InstanceGlobal { .. } => "duplicate definitions",
            Self::Task { .. } => "duplicate definitions",
            Self::Resource { .. } => "duplicate definitions",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for DuplicateError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            Self::Variable { var1, var2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate variable '{}'",
                        var1.get_name_with_case(db).text(db)
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
                        var2.get_name_with_case(db).text(db)
                    ),
                    var2.get_scope_id(db).file(db),
                    var2.get_name_span(db),
                ));

                diag
            }
            Self::Pou { pou1, pou2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate POU '{}'",
                        pou1.get_name_with_case(db).text(db)
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
                        pou2.get_name_with_case(db).text(db)
                    ),
                    pou2.get_scope_id(db).file(db),
                    pou2.get_name_span(db),
                ));

                diag
            }
            Self::ProgramPou {
                program,
                pou,
                at_program,
            } => {
                let name = program.get_name_with_case(db).text(db);
                let (span, other_file, other_span) = match at_program {
                    true => (
                        program.get_name_span(db),
                        pou.get_scope_id(db).file(db),
                        pou.get_name_span(db),
                    ),
                    false => (
                        pou.get_name_span(db),
                        program.get_scope_id(db).file(db),
                        program.get_name_span(db),
                    ),
                };
                let mut diag = diag()
                    .message(format!("duplicate POU '{name}'"))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &span).unwrap_or_default())
                    .call();
                diag.with_related(Related::new(
                    format!("POU '{name}' is also defined here"),
                    other_file,
                    other_span,
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
            Self::StructField { field1, field2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate field '{}'",
                        field1.get_name_with_case(db).text(db)
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
                        field2.get_name_with_case(db).text(db)
                    ),
                    field2.get_scope_id(db).file(db),
                    field2.get_name_span(db),
                ));

                diag
            }
            Self::EnumVariant { variant1, variant2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate enum variant '{}'",
                        variant1.with_case.text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &variant1.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "enum variant '{}' is already defined here",
                        variant2.with_case.text(db)
                    ),
                    variant2.get_scope_id(db).file(db),
                    variant2.get_span(db),
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
            Self::VariableIsReturnValue { var, pou_kind } => diag()
                .message(format!(
                    "variable '{}' is the {pou_kind}'s return value",
                    var.get_name_with_case(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &var.get_name_span(db)).unwrap_or_default())
                .call(),
            Self::MethodDecl { method1, method2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate method '{}'",
                        method1.get_name_with_case(db).text(db)
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
                        method2.get_name_with_case(db).text(db)
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
                        method1.get_name_with_case(db).text(db)
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
                        method2.get_name_with_case(db).text(db)
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
                        method1.method.get_name_with_case(db).text(db)
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
                        method2.method.get_name_with_case(db).text(db)
                    ),
                    method2.method.get_scope_id(db).file(db),
                    method2.method.get_name_span(db),
                ));

                diag.with_note(format!(
                    "this error happens because both interfaces '{}' and '{}' define a method '{}'",
                    method1.source.get_name_with_case(db).text(db),
                    method2.source.get_name_with_case(db).text(db),
                    method1.method.get_name_with_case(db).text(db)
                ));

                diag
            }
            Self::Using { using, other } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate `USING` for namespace '{}'",
                        using.path(db).path_with_case.to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &using.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "namespace '{}' is already imported here",
                        other.path(db).path_with_case.to_string(db)
                    ),
                    other.scope_id(db).file(db),
                    other.get_span(db),
                ));

                diag
            }
            Self::Program { prog1, prog2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate program '{}'",
                        prog1.get_name_with_case(db).text(db)
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
                        prog2.get_name_with_case(db).text(db)
                    ),
                    prog2.get_scope_id(db).file(db),
                    prog2.get_name_span(db),
                ));

                diag
            }
            Self::ProgInstance { prog1, prog2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate program instance '{}'",
                        prog1.with_case.text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &prog1.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "program instance '{}' is already defined here",
                        prog2.with_case.text(db)
                    ),
                    prog2.get_scope_id(db).file(db),
                    prog2.get_span(db),
                ));

                diag
            }
            Self::InstanceGlobal {
                instance,
                global,
                at_instance,
            } => {
                let name = instance.with_case.text(db);
                let (message, span, related) = match at_instance {
                    true => (
                        format!("program instance '{name}' has the name of a VAR_GLOBAL"),
                        instance.get_span(db),
                        Related::new(
                            format!(
                                "VAR_GLOBAL '{}' is declared here",
                                global.get_name_with_case(db).text(db)
                            ),
                            global.get_scope_id(db).file(db),
                            global.get_name_span(db),
                        ),
                    ),
                    false => (
                        format!(
                            "VAR_GLOBAL '{}' has the name of a program instance",
                            global.get_name_with_case(db).text(db)
                        ),
                        global.get_name_span(db),
                        Related::new(
                            format!("program instance '{name}' is declared here"),
                            instance.get_scope_id(db).file(db),
                            instance.get_span(db),
                        ),
                    ),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &span).unwrap_or_default())
                    .call();
                diag.with_related(related);
                diag
            }
            Self::Task { task1, task2 } => {
                let mut diag = diag()
                    .message(format!("duplicate task '{}'", task1.with_case.text(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &task1.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "task '{}' is already defined here",
                        task2.with_case.text(db)
                    ),
                    task2.get_scope_id(db).file(db),
                    task2.get_span(db),
                ));

                diag
            }
            Self::Resource { res1, res2 } => {
                let mut diag = diag()
                    .message(format!("duplicate resource '{}'", res1.with_case.text(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &res1.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "resource '{}' is already defined here",
                        res2.with_case.text(db)
                    ),
                    res2.get_scope_id(db).file(db),
                    res2.get_span(db),
                ));

                diag
            }
        }
    }
}
