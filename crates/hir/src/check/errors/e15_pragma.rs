use crate::CallSite;
use crate::HasName;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::expressions::spec::Spec;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::pous::function::Function;
use crate::hir_def::pous::variable::VariableDecl;
use auto_lsp::lsp_types::DiagnosticSeverity;
use auto_lsp::tree_sitter;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::diag;

/// Which declaration section an `{extern}` FUNCTION cannot carry, and why.
/// One code (E1502), one message shape each — the fix differs per case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ExternForbiddenKind {
    /// `VAR_IN_OUT` hands the host a pointer into caller storage with a
    /// mutation contract; an extern's interface is copies only.
    InOut,
    /// A struct/array/STRING `VAR_OUTPUT` has no WASM result type to ride.
    AggregateOutput,
}

impl ExternForbiddenKind {
    fn section(self) -> &'static str {
        match self {
            Self::InOut => "VAR_IN_OUT",
            Self::AggregateOutput => "VAR_OUTPUT",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::InOut => "cannot cross a WASM import: an extern takes copies, not references",
            Self::AggregateOutput => "cannot be a WASM result: only scalar outputs cross an import",
        }
    }

    fn note(self) -> &'static str {
        match self {
            Self::InOut => {
                "an extern FUNCTION receives VAR_INPUT copies and returns scalar \
                 VAR_OUTPUT results (the return value last)"
            }
            Self::AggregateOutput => "return scalars, or split the aggregate into scalar outputs",
        }
    }
}

/// Why an `{export}` FUNCTION has no export to give. One code (E1509), one
/// message shape each.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ExportForbiddenKind {
    /// An `{extern}` FUNCTION is an import: there is no body to export.
    Extern,
    /// A `{test}` FUNCTION is exported for the runner, and only in a debug
    /// build. A second export under the same name is an invalid module.
    Test,
    /// One copy per implementation it is called with (`drive$Worker`).
    InterfaceParam,
    /// One copy per arity it is called with (`sum_all$3`).
    Variadic,
    /// The symbol carries the signature (`SHL$Byte`), and a host finds an
    /// export by its name.
    Overloaded,
}

impl ExportForbiddenKind {
    fn message(self) -> &'static str {
        match self {
            Self::Extern => "is an {extern} FUNCTION",
            Self::Test => "is a {test} FUNCTION",
            Self::InterfaceParam => "takes an interface",
            Self::Variadic => "is variadic",
            Self::Overloaded => "is overloaded",
        }
    }

    fn note(self) -> &'static str {
        match self {
            Self::Extern => "an {extern} FUNCTION is an import, it has no body to export",
            Self::Test => {
                "a {test} FUNCTION is already exported for `rk test`, and left out of a release build"
            }
            Self::InterfaceParam => {
                "it is compiled once per implementation it is called with, so there is no single function to export"
            }
            Self::Variadic => {
                "it is compiled once per number of arguments it is called with, so there is no single function to export"
            }
            Self::Overloaded => {
                "an export is found by its name, and this name belongs to several FUNCTIONs"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum PragmaError<'db> {
    /// `{extern}` on something other than a FUNCTION.
    ExternOutsideFunction {
        anchor: SpanIdent<'db>,
        pou_kind: &'static str,
    },
    /// What an `{extern}` FUNCTION declared that a WASM import cannot carry.
    /// The import's interface is copies in (`VAR_INPUT`) and scalar results
    /// out (`VAR_OUTPUT` in declaration order, the return type last).
    ExternForbiddenSection {
        var: VariableDecl<'db>,
        kind: ExternForbiddenKind,
    },
    /// An `{extern}` FUNCTION returning something no WASM result can carry.
    /// The return type is the import's LAST result, so the same scalar rule
    /// that governs `VAR_OUTPUT` governs it — and it is the one place a
    /// STRING or a STRUCT can still be written.
    ExternNonScalarReturn { func: Function<'db>, ret: Spec<'db> },
    /// An `{extern}` FUNCTION with statements — the import IS the body.
    ExternWithBody { site: CallSite<'db> },
    /// `{test}` on something other than a FUNCTION. A test is a `()` entry the
    /// runner calls; an FB, PROGRAM or METHOD has no such entry, and a helper
    /// that only tests use is hidden with PRIVATE, not marked as a test.
    TestOutsideFunction {
        anchor: SpanIdent<'db>,
        pou_kind: &'static str,
    },
    /// `{export}` on something other than a FUNCTION. A PROGRAM is already
    /// exported for the schedule; an FB, a CLASS or a METHOD needs an
    /// instance the host does not have.
    ExportOutsideFunction {
        anchor: SpanIdent<'db>,
        pou_kind: &'static str,
    },
    /// `{export}` on a FUNCTION that has no single export to give.
    ExportForbidden {
        anchor: SpanIdent<'db>,
        func: Function<'db>,
        kind: ExportForbiddenKind,
    },
    /// A `{wasm}` pragma outside a FUNCTION body. Only FUNCTION bodies are
    /// scanned for one; anywhere else the statement was silently dropped and
    /// the surrounding body compiled as if it were not there.
    WasmPragmaOutsideFunction { span: tree_sitter::Range },
    /// A `{wasm}` pragma names an instruction the emitter has no arm for.
    /// The name used to fall through to `unreachable` (or, on the conversion
    /// shape, to a silent identity): a valid module carrying code the
    /// program never asked for, from a compile that exited 0.
    UnknownWasmInstruction {
        name: compact_str::CompactString,
        span: tree_sitter::Range,
    },
    /// A `{wasm}` operand that is not a parameter, a local or the return of
    /// the FUNCTION. The lowering runs the instruction on exactly the names
    /// the pragma gives, so an unknown one has no slot to read or write.
    UnknownWasmOperand {
        name: compact_str::CompactString,
        span: tree_sitter::Range,
    },
    /// A `{wasm}` pragma whose operands do not fit its instruction: the
    /// wrong lanes, count, or result. The module validator used to be the
    /// first to say so, at load, from a compile that exited 0.
    WasmSignatureMismatch {
        instruction: compact_str::CompactString,
        expected: String,
        actual: String,
        span: tree_sitter::Range,
    },
}

impl<'db> ErrorCode for PragmaError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::ExternOutsideFunction { .. } => "E1501",
            Self::ExternForbiddenSection { .. } => "E1502",
            Self::ExternNonScalarReturn { .. } => "E1502",
            Self::ExternWithBody { .. } => "E1502",
            Self::TestOutsideFunction { .. } => "E1503",
            Self::WasmPragmaOutsideFunction { .. } => "E1504",
            Self::UnknownWasmInstruction { .. } => "E1505",
            Self::UnknownWasmOperand { .. } => "E1506",
            Self::WasmSignatureMismatch { .. } => "E1507",
            Self::ExportOutsideFunction { .. } => "E1508",
            Self::ExportForbidden { .. } => "E1509",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::ExternOutsideFunction { .. } => "extern pragma outside a FUNCTION",
            Self::ExternForbiddenSection { .. } => "not representable on an extern FUNCTION",
            Self::ExternNonScalarReturn { .. } => "not representable on an extern FUNCTION",
            Self::ExternWithBody { .. } => "not representable on an extern FUNCTION",
            Self::TestOutsideFunction { .. } => "test pragma outside a FUNCTION",
            Self::WasmPragmaOutsideFunction { .. } => "invalid wasm pragma",
            Self::UnknownWasmInstruction { .. } => "invalid wasm pragma",
            Self::UnknownWasmOperand { .. } => "invalid wasm pragma",
            Self::WasmSignatureMismatch { .. } => "invalid wasm pragma",
            Self::ExportOutsideFunction { .. } => "export pragma outside a FUNCTION",
            Self::ExportForbidden { .. } => "this FUNCTION cannot be exported",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for PragmaError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            Self::ExternOutsideFunction { anchor, pou_kind } => {
                let mut diag = diag()
                    .message(format!("an {{extern}} pragma cannot be placed on a {pou_kind}"))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &anchor.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "{extern} pragmas can ony be used with FUNCTION"
                        .to_string(),
                );
                diag
            }
            Self::ExternForbiddenSection { var, kind } => {
                let mut diag = diag()
                    .message(format!(
                        "{} '{}' {}",
                        kind.section(),
                        var.name(db).text(db),
                        kind.message(),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(kind.note().to_string());
                diag
            }
            Self::ExternNonScalarReturn { func, ret } => diag()
                .message(format!(
                    "the return type of '{}' can only be a scalar",
                    func.get_name_ident(db).text(db),
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &ret.get_span(db)).unwrap_or_default())
                .call(),
            Self::ExternWithBody { site } => {
                let mut diag = diag()
                    .message("an extern FUNCTION has no statements".to_string())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &site.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "FUNCTIONs marked with {extern} act as external calls, they can not have a body"
                        .to_string(),
                );
                diag
            }
            Self::TestOutsideFunction { anchor, pou_kind } => diag()
                .message(format!("a {{test}} pragma cannot be placed on a {pou_kind}"))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &anchor.get_span(db)).unwrap_or_default())
                .call(),
            Self::ExportOutsideFunction { anchor, pou_kind } => {
                let mut diag = diag()
                    .message(format!("an {{export}} pragma cannot be placed on a {pou_kind}"))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &anchor.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note("{export} pragmas can only be used with FUNCTION".to_string());
                diag
            }
            Self::ExportForbidden { anchor, func, kind } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' cannot be exported: it {}",
                        func.get_name_ident(db).text(db),
                        kind.message(),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &anchor.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(kind.note().to_string());
                diag
            }
            Self::WasmPragmaOutsideFunction { span } => diag()
                .message(
                    "a {wasm} body is only available on a FUNCTION; here the pragma would be silently dropped"
                        .to_string(),
                )
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::UnknownWasmInstruction { name, span } => diag()
                .message(format!(
                    "'{name}' is not a wasm instruction this compiler emits"
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::UnknownWasmOperand { name, span } => diag()
                .message(format!(
                    "'{name}' is not a parameter, a local or the return of this FUNCTION"
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::WasmSignatureMismatch {
                instruction,
                expected,
                actual,
                span,
            } => diag()
                .message(format!(
                    "'{instruction}' takes {expected}; this pragma gives it {actual}"
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
        }
    }
}
