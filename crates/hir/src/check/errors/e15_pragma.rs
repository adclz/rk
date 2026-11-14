// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use crate::CallSite;
use crate::HasName;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::expressions::spec::Spec;
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::pous::function::Function;
use crate::hir_def::pous::function_block::FunctionBlock;
use crate::hir_def::pous::pragma::ModuleNameError;
use crate::hir_def::pous::variable::VariableDecl;
use auto_lsp::lsp_types::DiagnosticSeverity;
use auto_lsp::tree_sitter;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::Related;
use ide_diagnostic::diag;

/// Why a stream's or a future's built-in does not fit the POU its `{extern}`
/// stands on (E1516).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum BuiltinMisfit {
    /// A read or a write, which waits for the other end, on a FUNCTION.
    Waits,
    /// A built-in that returns at once on an `{extern async}` block.
    Instant,
    /// A future's write on an `{extern async}` block: a block imports a
    /// call, a stream's read or write, or a future's read.
    NoBlock,
}

/// What an async import's FUNCTION_BLOCK declares besides its VAR_INPUTs and
/// VAR_OUTPUTs (E1502).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ImportMember {
    Method,
    Extends,
    Implements,
}

/// Which declaration section an `{extern}` FUNCTION cannot carry, and why.
/// One code (E1502), one message shape each — the fix differs per case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ExternForbiddenKind {
    /// `VAR_IN_OUT` hands the host a pointer into caller storage with a
    /// mutation contract; an extern's interface is copies only.
    InOut,
    /// A STRUCT, ARRAY, instance, reference or STRING `VAR_OUTPUT`: the
    /// import's result is a record of scalars.
    AggregateOutput,
    /// An `ARRAY[*]` `VAR_INPUT` has the size of each call's array, and an
    /// import's copies have one size.
    Conformand,
    /// A variadic `VAR_INPUT` has one copy per argument count, and an import
    /// has one signature: a call looked for a copy no module holds.
    Variadic,
    /// A `REF_TO` `VAR_INPUT` of an import from a module other than WASI's:
    /// an address in the module means nothing to a host that only receives
    /// WIT values.
    Reference,
    /// A STRUCT, ARRAY or instance `VAR_INPUT` of an import from a module
    /// other than WASI's, which would go as an address.
    AggregateInput,
    /// A STRUCT, ARRAY or instance `VAR_INPUT` of a WASI import. It would go
    /// as an address into rk's own layout, which no WASI function reads: WASI
    /// takes a record as its fields' values, and a list as packed elements.
    WasiAggregateInput,
    /// The `VAR_INPUT` that takes an import from a module other than WASI's
    /// past 16 values, a STRING counting two: past them the component model
    /// passes the values in memory, which the call does not.
    TooManyValues,
}

impl ExternForbiddenKind {
    fn section(self) -> &'static str {
        match self {
            Self::InOut => "VAR_IN_OUT",
            Self::AggregateOutput => "VAR_OUTPUT",
            Self::Conformand
            | Self::Variadic
            | Self::Reference
            | Self::AggregateInput
            | Self::WasiAggregateInput
            | Self::TooManyValues => "VAR_INPUT",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::InOut => "cannot cross a WASM import: an extern takes copies, not references",
            Self::AggregateOutput => "cannot be a field of the import's result: only a scalar can",
            Self::Conformand => {
                "cannot cross a WASM import: an ARRAY[*] has the size of each call's array"
            }
            Self::Variadic => {
                "cannot cross a WASM import: a variadic parameter has the argument count of each call"
            }
            Self::Reference => {
                "cannot cross a component import: an address in the module means nothing to the host"
            }
            Self::AggregateInput => {
                "cannot cross a component import: only a scalar or a STRING can"
            }
            Self::WasiAggregateInput => {
                "cannot cross a WASI import: no WASI function reads rk's own layout"
            }
            Self::TooManyValues => {
                "cannot cross a component import: the import would take more than 16 values"
            }
        }
    }

    fn note(self) -> &'static str {
        match self {
            Self::InOut => "an extern FUNCTION receives copies of its VAR_INPUT",
            Self::AggregateOutput => {
                "an {extern} FUNCTION returns its VAR_OUTPUTs and its return value as one record of scalars"
            }
            Self::Conformand => "an extern FUNCTION receives VAR_INPUT copies of a fixed size",
            Self::Variadic => "a WASM import has one signature",
            Self::Reference | Self::AggregateInput => {
                "an {extern} FUNCTION from a module other than WASI's takes scalars and STRINGs"
            }
            Self::WasiAggregateInput => {
                "a WASI import takes scalars, STRINGs, and a REF_TO where a record comes back"
            }
            Self::TooManyValues => {
                "a component import takes 16 values at most, a STRING counting two"
            }
        }
    }
}

/// What an `{export}` FUNCTION declared that a host cannot pass it or get
/// back: its export is typed in WIT, whose functions take values and return
/// one. One code (E1509), one message shape each.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ExportForbiddenSection {
    /// A STRUCT, ARRAY, instance or reference `VAR_OUTPUT`: the export's
    /// result is a record of scalars and STRINGs.
    AggregateOutput,
    /// A `VAR_IN_OUT`: the host passes values, not references.
    InOut,
    /// A `REF_TO` `VAR_INPUT`: the host has no address in the module.
    Reference,
    /// A STRUCT, ARRAY or instance `VAR_INPUT`, which would go as an
    /// address.
    AggregateInput,
    /// The `VAR_INPUT` that takes the export past 16 values, a STRING
    /// counting two: past them the component model passes the values in
    /// memory, which the export does not read them from.
    TooManyValues,
}

impl ExportForbiddenSection {
    fn section(self) -> &'static str {
        match self {
            Self::AggregateOutput => "VAR_OUTPUT",
            Self::InOut => "VAR_IN_OUT",
            Self::Reference | Self::AggregateInput | Self::TooManyValues => "VAR_INPUT",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::AggregateOutput => {
                "cannot be a field of the export's result: only a scalar or a STRING can"
            }
            Self::InOut => {
                "cannot cross a component export: the host passes values, not references"
            }
            Self::Reference => {
                "cannot cross a component export: the host has no address in the module to pass"
            }
            Self::AggregateInput => {
                "cannot cross a component export: only a scalar or a STRING can"
            }
            Self::TooManyValues => {
                "cannot cross a component export: the export would take more than 16 values"
            }
        }
    }

    fn note(self) -> &'static str {
        match self {
            Self::AggregateOutput => {
                "an {export} FUNCTION returns its VAR_OUTPUTs and its return value as one record of scalars and STRINGs"
            }
            Self::InOut | Self::Reference | Self::AggregateInput => {
                "an {export} FUNCTION takes scalars and STRINGs, as VAR_INPUT"
            }
            Self::TooManyValues => {
                "a component export takes 16 values at most, a STRING counting two"
            }
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
    /// One copy per implementation it is called with (`drive$@Worker`).
    InterfaceParam,
    /// One copy per arity it is called with (`sum_all$3`).
    Variadic,
    /// One copy per array type it is called with (`Sum$[0..9]`).
    Conformand,
    /// The symbol carries the signature (`SHL$BYTE`), and a host finds an
    /// export by its name.
    Overloaded,
    /// Named like an export the module makes for the host
    /// ([`MODULE_EXPORTS`]): the module would export the name twice.
    ///
    /// [`MODULE_EXPORTS`]: crate::hir_def::pous::pragma::MODULE_EXPORTS
    Reserved,
}

impl ExportForbiddenKind {
    fn message(self) -> &'static str {
        match self {
            Self::Extern => "is an {extern} FUNCTION",
            Self::Test => "is a {test} FUNCTION",
            Self::InterfaceParam => "takes an interface",
            Self::Variadic => "is variadic",
            Self::Conformand => "takes an ARRAY[*]",
            Self::Overloaded => "is overloaded",
            Self::Reserved => "has the name of an export the module makes",
        }
    }

    fn note(self) -> &'static str {
        match self {
            Self::Extern => "an {extern} FUNCTION is an import, it has no body to export",
            Self::Test => {
                "a {test} FUNCTION is already exported for `rk test` and left out of a release build"
            }
            Self::InterfaceParam => {
                "it is compiled once per implementation it is called with, so there is no single function to export"
            }
            Self::Variadic => {
                "it is compiled once per number of arguments it is called with, so there is no single function to export"
            }
            Self::Conformand => {
                "it is compiled once per array type it is called with, so there is no single function to export"
            }
            Self::Overloaded => {
                "the module finds an export by its name, which several FUNCTIONs share"
            }
            Self::Reserved => {
                "the module already exports `__init`, `memory`, `cabi_realloc` and the base and size of each memory band under these names"
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
    /// An `{extern}` FUNCTION or FUNCTION_BLOCK with statements: the import
    /// IS the body.
    ExternWithBody {
        site: CallSite<'db>,
        pou_kind: &'static str,
    },
    /// An async import, an `{extern async}` FUNCTION_BLOCK, with a METHOD, a
    /// base or an INTERFACE, anchored at the METHOD's name or at the type
    /// named: the block is its call, and a METHOD, a base's body or an
    /// INTERFACE's METHODs would be code of the import the host has not.
    ExternBlockMember {
        span: tree_sitter::Range,
        member: ImportMember,
    },
    /// An `{extern}` pragma whose `async` does not fit its POU: a FUNCTION's
    /// call returns before the scan goes on, and an async call is held by a
    /// FUNCTION_BLOCK's instance from one scan to the next. `block`: the POU
    /// is a FUNCTION_BLOCK, the pragma without `async`.
    ExternAsync { anchor: SpanIdent<'db>, block: bool },
    /// An async import, an `{extern async}` FUNCTION_BLOCK, without its `DONE`,
    /// anchored at the block's name, or with a `DONE` declared in another
    /// section or with another type than BOOL, `found`.
    DoneOutput {
        block: FunctionBlock<'db>,
        found: Option<VariableDecl<'db>>,
    },
    /// A `{test}` FUNCTION_BLOCK without its `DONE`, anchored at the block's
    /// name, or with a `DONE` declared in another section or with another
    /// type than BOOL, `found`: the runner calls the block once per cycle
    /// until a cycle leaves `DONE` TRUE.
    TestDone {
        block: FunctionBlock<'db>,
        found: Option<VariableDecl<'db>>,
    },
    /// A variable holding an async import's instance in a call's frame, a
    /// FUNCTION's, a METHOD's or a VAR_TEMP, `frame`: the host writes the
    /// call's result into the instance at a later scan.
    AsyncImportInFrame {
        var: VariableDecl<'db>,
        frame: &'static str,
    },
    /// An `{extern}`'s module the component model takes no import from: a
    /// plain WIT name, `modbus`, or an interface, `acme:modbus/rtu@1.0.0`,
    /// is. Written as WIT writes it, never converted: the host implements
    /// the name the declaration gives.
    ExternModuleName {
        module: compact_str::CompactString,
        span: tree_sitter::Range,
        why: ModuleNameError,
    },
    /// A stream's or a future's built-in, `name`, imported where it does
    /// not fit, anchored at the name.
    StreamBuiltin {
        name: compact_str::CompactString,
        span: tree_sitter::Range,
        why: BuiltinMisfit,
    },
    /// A stream write whose variables are not the built-in's, anchored at
    /// the block's name ([`crate::hir_ty::async_import::stream_write_shaped`]).
    StreamWriteVariables { block: FunctionBlock<'db> },
    /// A stream read whose variables are not the built-in's, anchored at the
    /// block's name ([`crate::hir_ty::async_import::stream_read_shaped`]).
    StreamReadVariables { block: FunctionBlock<'db> },
    /// A future read that does not take the future's reading end alone,
    /// anchored at the block's name
    /// ([`crate::hir_ty::async_import::future_read_shaped`]).
    FutureReadVariables { block: FunctionBlock<'db> },
    /// A stream's or a future's `new` not declared as the two ends it makes,
    /// a UDINT VAR_OUTPUT each, the reading end first, anchored at the
    /// FUNCTION's name: the built-in returns both in one 64-bit value, which
    /// the call writes into the two outputs.
    NewEnds { func: Function<'db> },
    /// A resource's drop, `[resource-drop]R`, not declared as the handle it
    /// drops, one UDINT VAR_INPUT and nothing else, anchored at the
    /// FUNCTION's name.
    ResourceDropVariables { func: Function<'db> },
    /// An `{extern}`'s name that is not a WIT name.
    ExternFunctionName {
        name: compact_str::CompactString,
        span: tree_sitter::Range,
    },
    /// A resource's function, or the drop of a resource's handle, imported
    /// from a module other than WASI's, anchored at the name: the WIT of a
    /// host module's interface is written from its declarations, which make
    /// no resource.
    ExternResourceName {
        name: compact_str::CompactString,
        span: tree_sitter::Range,
        drop: bool,
    },
    /// A `VAR_INPUT` or a `VAR_OUTPUT` of an import from a module other
    /// than WASI's whose name gives no WIT name ([`wit_parameter_name`]):
    /// the import's WIT names each parameter and each field of its result.
    ///
    /// [`wit_parameter_name`]: crate::hir_def::pous::pragma::wit_parameter_name
    ExternParameterName { var: VariableDecl<'db> },
    /// Two `VAR_INPUT`s, or two `VAR_OUTPUT`s, of such an import whose WIT
    /// names the component model takes for one: it compares names without
    /// their `-`.
    ExternParameterNamesCollide {
        var: VariableDecl<'db>,
        first: VariableDecl<'db>,
    },
    /// `{test}` on something other than a FUNCTION or a FUNCTION_BLOCK the
    /// runner can make an instance of. A test FUNCTION is a `()` entry the
    /// runner calls once, a test FUNCTION_BLOCK one it calls cycle by cycle on
    /// an instance of its own; a PROGRAM or a METHOD has no such entry, an
    /// ABSTRACT or `{extern}` FUNCTION_BLOCK no instance the runner can make,
    /// and a helper that only tests use is hidden with PRIVATE, not marked as
    /// a test.
    TestOutsideFunction {
        anchor: SpanIdent<'db>,
        pou_kind: &'static str,
    },
    /// A `{test}` FUNCTION with a `VAR_INPUT`, a `VAR_OUTPUT` or a `VAR_IN_OUT`,
    /// or a `{test}` FUNCTION_BLOCK with a `VAR_INPUT` or a `VAR_IN_OUT`: the
    /// runner calls a test with no arguments, so nothing can bind it. A test
    /// FUNCTION's wrapper has no parameters to read: a scalar read a scratch
    /// local, what goes by address wrote near address 0, a REAL made the module
    /// invalid, and a test taking a variadic or an interface had no copy, so it
    /// never ran. A test FUNCTION_BLOCK's VAR_OUTPUTs are its instance's, its
    /// `DONE` among them.
    TestWithParameter {
        var: VariableDecl<'db>,
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
    /// What an `{export}` FUNCTION declared that a host cannot pass it or
    /// get back.
    ExportForbiddenSection {
        var: VariableDecl<'db>,
        kind: ExportForbiddenSection,
    },
    /// An `{export}` FUNCTION returning what WIT has no value for here: a
    /// STRUCT, an ARRAY, an instance or a reference.
    ExportReturn { func: Function<'db>, ret: Spec<'db> },
    /// The name an `{export 'name'}` gives is not a WIT name.
    ExportNameNotWit {
        name: compact_str::CompactString,
        span: tree_sitter::Range,
    },
    /// An `{export}` FUNCTION one of whose names gives no WIT name: `word`,
    /// a namespace's or its own.
    ExportWithoutWitName {
        func: Function<'db>,
        word: compact_str::CompactString,
    },
    /// Two `{export}` FUNCTIONs whose WIT names the component model takes
    /// for one, `name`: reported at each, with the other.
    ExportNamesCollide {
        func: Function<'db>,
        other: Function<'db>,
        name: String,
    },
    /// A `VAR_INPUT` or a `VAR_OUTPUT` of an `{export}` FUNCTION whose name
    /// gives no WIT name, as [`Self::ExternParameterName`].
    ExportParameterName { var: VariableDecl<'db> },
    /// Two `VAR_INPUT`s, or two `VAR_OUTPUT`s, of an `{export}` FUNCTION
    /// with one WIT name, as [`Self::ExternParameterNamesCollide`].
    ExportParameterNamesCollide {
        var: VariableDecl<'db>,
        first: VariableDecl<'db>,
    },
    /// A `VAR_OUTPUT` of an import or an export whose WIT name is the one
    /// its result gives the return value, `name`: the import's name, or the
    /// export's. E1512 for an import, E1513 for an export.
    OutputNamedLikeResult {
        var: VariableDecl<'db>,
        func: Function<'db>,
        name: compact_str::CompactString,
        export: bool,
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
        name: Ident,
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
    /// A pragma the grammar has no rule for, `{attribute 'hide'}`, wherever
    /// it stands. Inside a declaration section it used to be read and
    /// dropped without a word, and some attributes change what the code
    /// means.
    UnknownPragma {
        /// Its first word, empty when it has none.
        name: compact_str::CompactString,
        span: tree_sitter::Range,
    },
}

/// The first words of rk's pragmas, as the grammar spells them.
const KNOWN_PRAGMAS: &[&str] = &[
    "test", "once", "export", "warn", "info", "allow", "extern", "wasm",
];

impl<'db> ErrorCode for PragmaError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::ExternOutsideFunction { .. } | Self::ExternAsync { .. } => "E1501",
            Self::ExternForbiddenSection { .. } => "E1502",
            Self::ExternNonScalarReturn { .. } => "E1502",
            Self::ExternWithBody { .. } | Self::ExternBlockMember { .. } => "E1502",
            Self::DoneOutput { .. } => "E1514",
            Self::TestDone { .. } => "E1517",
            Self::AsyncImportInFrame { .. } => "E1515",
            Self::StreamBuiltin { .. }
            | Self::StreamWriteVariables { .. }
            | Self::StreamReadVariables { .. }
            | Self::FutureReadVariables { .. }
            | Self::NewEnds { .. }
            | Self::ResourceDropVariables { .. } => "E1516",
            Self::ExternModuleName { .. }
            | Self::ExternFunctionName { .. }
            | Self::ExternResourceName { .. }
            | Self::ExternParameterName { .. }
            | Self::ExternParameterNamesCollide { .. }
            | Self::OutputNamedLikeResult { export: false, .. } => "E1512",
            Self::TestOutsideFunction { .. } => "E1503",
            Self::WasmPragmaOutsideFunction { .. } => "E1504",
            Self::UnknownWasmInstruction { .. } => "E1505",
            Self::UnknownWasmOperand { .. } => "E1506",
            Self::WasmSignatureMismatch { .. } => "E1507",
            Self::ExportOutsideFunction { .. } => "E1508",
            Self::ExportForbidden { .. }
            | Self::ExportForbiddenSection { .. }
            | Self::ExportReturn { .. } => "E1509",
            Self::ExportNameNotWit { .. }
            | Self::ExportWithoutWitName { .. }
            | Self::ExportNamesCollide { .. }
            | Self::ExportParameterName { .. }
            | Self::ExportParameterNamesCollide { .. }
            | Self::OutputNamedLikeResult { export: true, .. } => "E1513",
            Self::UnknownPragma { .. } => "E1510",
            Self::TestWithParameter { .. } => "E1511",
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
                    .message(format!(
                        "an {{extern}} pragma cannot be placed on a {pou_kind}"
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &anchor.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note("only a FUNCTION or a FUNCTION_BLOCK can be {extern}".to_string());
                diag
            }
            Self::ExternForbiddenSection { var, kind } => {
                let mut diag = diag()
                    .message(format!(
                        "{} '{}' {}",
                        kind.section(),
                        var.name_with_case(db).text(db),
                        kind.message(),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_name_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(kind.note().to_string());
                diag
            }
            Self::ExternNonScalarReturn { func, ret } => diag()
                .message(format!(
                    "the return type of '{}' can only be a scalar",
                    func.get_name_with_case(db).text(db),
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &ret.get_span(db)).unwrap_or_default())
                .call(),
            Self::ExternWithBody { site, pou_kind } => {
                let mut diag = diag()
                    .message(format!("an extern {pou_kind} has no statements"))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &site.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(format!(
                    "an {{extern}} {pou_kind} is an import: the host has its body"
                ));
                diag
            }
            Self::ExternBlockMember { span, member } => {
                let (message, help) = match member {
                    ImportMember::Method => (
                        "an extern FUNCTION_BLOCK has no METHOD",
                        "declare the METHOD in a FUNCTION_BLOCK that holds the import",
                    ),
                    ImportMember::Extends => (
                        "an extern FUNCTION_BLOCK extends nothing",
                        "extend the base in a FUNCTION_BLOCK that holds the import",
                    ),
                    ImportMember::Implements => (
                        "an extern FUNCTION_BLOCK implements no INTERFACE",
                        "implement the INTERFACE in a FUNCTION_BLOCK that holds the import",
                    ),
                };
                let mut diag = diag()
                    .message(message.to_string())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();
                diag.with_note(
                    "an {extern} FUNCTION_BLOCK is an import: the host has its body".to_string(),
                );
                diag.with_help(help.to_string());
                diag
            }
            Self::ExternModuleName { module, span, why } => {
                let message = match why {
                    ModuleNameError::NotAName if module.contains(':') => {
                        format!("'{module}' is not an interface's name")
                    }
                    ModuleNameError::NotAName => format!("'{module}' is not a WIT name"),
                    ModuleNameError::Nested => format!("'{module}' is a nested name"),
                    ModuleNameError::Version => format!(
                        "'{}' is not a semantic version",
                        module.split_once('@').map_or("", |(_, version)| version)
                    ),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();
                diag.with_note(
                    match why {
                        ModuleNameError::NotAName => {
                            "an import's module is a WIT name, like `modbus`, or an \
                             interface, like `acme:modbus/rtu@1.0.0`"
                        }
                        ModuleNameError::Nested => {
                            "an interface is `namespace:package/interface`, and the \
                             component model's tools take no nested name yet"
                        }
                        ModuleNameError::Version => {
                            "an interface's version is `MAJOR.MINOR.PATCH`, like `1.0.0`"
                        }
                    }
                    .to_string(),
                );
                if *why == ModuleNameError::NotAName
                    && let Some(spelling) = crate::hir_def::pous::pragma::wit_spelling(module)
                {
                    diag.with_help(format!("write '{spelling}'"));
                }
                diag
            }
            Self::StreamBuiltin { name, span, why } => {
                let (message, note, help) = match why {
                    BuiltinMisfit::Waits => (
                        format!("'{name}' waits for the other end"),
                        "a read or a write of a stream or a future waits for the other end, \
                         which only the calls of an {extern async} FUNCTION_BLOCK can",
                        Some("import it with {extern async} on a FUNCTION_BLOCK"),
                    ),
                    BuiltinMisfit::Instant => (
                        format!("'{name}' returns at once"),
                        "an {extern async} FUNCTION_BLOCK imports a call that may not return \
                         at once",
                        Some("import it with {extern} on a FUNCTION"),
                    ),
                    BuiltinMisfit::NoBlock => (
                        format!("'{name}' writes a future"),
                        "an {extern async} FUNCTION_BLOCK imports the call of an async func, \
                         a stream's read or write, or a future's read",
                        None,
                    ),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();
                diag.with_note(note.to_string());
                if let Some(help) = help {
                    diag.with_help(help.to_string());
                }
                diag
            }
            Self::StreamWriteVariables { block } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' does not declare a stream write's variables",
                        block.name_with_case(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &block.get_name_span(db)).unwrap_or_default(),
                    )
                    .call();
                diag.with_note(
                    "a stream write takes the stream's writing end, a UDINT, then the bytes, \
                     a STRING, and gives after DONE how many went, a UDINT, then whether the \
                     reader left, a BOOL"
                        .to_string(),
                );
                diag.with_help(
                    "declare the VAR_INPUTs stream : UDINT and data : STRING, and the \
                     VAR_OUTPUTs written : UDINT and closed : BOOL after DONE"
                        .to_string(),
                );
                diag
            }
            Self::StreamReadVariables { block } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' does not declare a stream read's variables",
                        block.name_with_case(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &block.get_name_span(db)).unwrap_or_default(),
                    )
                    .call();
                diag.with_note(
                    "a stream read takes the stream's reading end, a UDINT, and gives after DONE \
                     the bytes, a STRING or an ARRAY OF BYTE, how many came, a UDINT, then \
                     whether the writer left, a BOOL"
                        .to_string(),
                );
                diag.with_help(
                    "declare the VAR_INPUT stream : UDINT, and the VAR_OUTPUTs data : STRING, \
                     read : UDINT and closed : BOOL after DONE"
                        .to_string(),
                );
                diag
            }
            Self::FutureReadVariables { block } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' does not take the future's reading end alone",
                        block.name_with_case(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &block.get_name_span(db)).unwrap_or_default(),
                    )
                    .call();
                diag.with_note(
                    "a future read takes the future's reading end, a UDINT, and gives after \
                     DONE the fields of the value the future holds"
                        .to_string(),
                );
                diag.with_help("declare the one VAR_INPUT future : UDINT".to_string());
                diag
            }
            Self::NewEnds { func } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' does not declare the two ends it makes",
                        func.get_name_with_case(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &func.get_name_span(db)).unwrap_or_default(),
                    )
                    .call();
                diag.with_note(
                    "a stream's or a future's new gives its reading end, then its writing end, \
                     each a UDINT"
                        .to_string(),
                );
                diag.with_help(
                    "declare the VAR_OUTPUTs readable : UDINT and writable : UDINT, and no \
                     return type"
                        .to_string(),
                );
                diag
            }
            Self::ResourceDropVariables { func } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' does not declare the handle it drops",
                        func.get_name_with_case(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &func.get_name_span(db)).unwrap_or_default(),
                    )
                    .call();
                diag.with_note(
                    "a resource's drop takes the handle, a UDINT, and gives nothing".to_string(),
                );
                diag.with_help(
                    "declare one VAR_INPUT handle : UDINT, and no VAR_OUTPUT or return type"
                        .to_string(),
                );
                diag
            }
            Self::ExternResourceName { name, span, drop } => {
                let mut diag = diag()
                    .message(match drop {
                        true => format!("'{name}' drops a resource's handle"),
                        false => format!("'{name}' names a resource's function"),
                    })
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();
                diag.with_note(
                    "the WIT of a host module's interface is written from its declarations, \
                     which make no resource: only WASI's interfaces have resources"
                        .to_string(),
                );
                diag
            }
            Self::ExternFunctionName { name, span } => {
                let mut diag = diag()
                    .message(format!("'{name}' is not a WIT name"))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();
                diag.with_note(
                    "an import's name is written as WIT writes one, words of lowercase \
                     letters and digits joined by '-'"
                        .to_string(),
                );
                if let Some(spelling) = crate::hir_def::pous::pragma::wit_spelling(name) {
                    diag.with_help(format!("write '{spelling}'"));
                }
                diag
            }
            Self::ExternParameterName { var } | Self::ExportParameterName { var } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' has no WIT name",
                        var.name_with_case(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_name_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "a parameter's WIT name is its name in lowercase with each '_' written \
                     '-', and a WIT name has no empty word and no letter outside ASCII"
                        .to_string(),
                );
                diag
            }
            Self::ExternParameterNamesCollide { var, first }
            | Self::ExportParameterNamesCollide { var, first } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' cannot be told from '{}' in WIT",
                        var.name_with_case(db).text(db),
                        first.name_with_case(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_name_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "the component model compares two names without their '-'".to_string(),
                );
                diag.with_related(Related::new(
                    format!("'{}' is declared here", first.name_with_case(db).text(db)),
                    first.get_scope_id(db).file(db),
                    first.get_name_span(db),
                ));
                diag
            }
            Self::OutputNamedLikeResult {
                var,
                func,
                name,
                export,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' cannot be told from the return value '{name}' in WIT",
                        var.name_with_case(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_name_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(match export {
                    true => "an {export} FUNCTION returns its VAR_OUTPUTs and its return value \
                             as one record, the return value named as the export is"
                        .to_string(),
                    false => "an {extern} FUNCTION returns its VAR_OUTPUTs and its return value \
                              as one record, the return value named as the import is"
                        .to_string(),
                });
                diag.with_help("rename the VAR_OUTPUT".to_string());
                diag.with_related(Related::new(
                    format!(
                        "'{}' is declared here",
                        func.get_name_with_case(db).text(db)
                    ),
                    func.get_scope_id(db).file(db),
                    func.get_name_span(db),
                ));
                diag
            }
            Self::ExternAsync { anchor, block } => {
                let (message, note, help) = match block {
                    true => (
                        "an {extern} pragma on a FUNCTION_BLOCK needs async",
                        "a FUNCTION_BLOCK imports an async function, whose call its instance \
                         holds from one scan to the next",
                        "write {extern async 'module' 'name'}",
                    ),
                    false => (
                        "an {extern async} pragma cannot be placed on a FUNCTION",
                        "an async call is held from one scan to the next, which only a \
                         FUNCTION_BLOCK's instance can",
                        "declare a FUNCTION_BLOCK with a VAR_OUTPUT DONE : BOOL",
                    ),
                };
                let mut diag = diag()
                    .message(message.to_string())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &anchor.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(note.to_string());
                diag.with_help(help.to_string());
                diag
            }
            Self::DoneOutput { block, found } | Self::TestDone { block, found } => {
                let (message, range) = match found {
                    None => (
                        format!(
                            "'{}' has no VAR_OUTPUT DONE",
                            block.name_with_case(db).text(db)
                        ),
                        block.get_name_span(db),
                    ),
                    Some(var) => (
                        format!(
                            "'{}' is not a VAR_OUTPUT of type BOOL",
                            var.name_with_case(db).text(db)
                        ),
                        var.get_name_span(db),
                    ),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &range).unwrap_or_default())
                    .call();
                let note = match self {
                    Self::DoneOutput { .. } => {
                        "an {extern async} FUNCTION_BLOCK's DONE is its own and no part of the \
                         function's signature: it is TRUE in the call of the block that saw the \
                         call finish, and the other VAR_OUTPUTs are the result's fields"
                    }
                    _ => {
                        "the runner calls a {test} FUNCTION_BLOCK once per cycle, until a cycle \
                         leaves its DONE TRUE"
                    }
                };
                diag.with_note(note.to_string());
                diag.with_help("declare DONE : BOOL in VAR_OUTPUT".to_string());
                diag
            }
            Self::AsyncImportInFrame { var, frame } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' holds an async import, which cannot live in {frame}",
                        var.name_with_case(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_name_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "the host writes an async call's result into its instance when it \
                     finishes, at a later scan, when the frame is gone"
                        .to_string(),
                );
                diag.with_help(
                    "declare it in a PROGRAM, a FUNCTION_BLOCK or a VAR_GLOBAL".to_string(),
                );
                diag
            }
            Self::TestOutsideFunction { anchor, pou_kind } => {
                let mut diag = diag()
                    .message(format!(
                        "a {{test}} pragma cannot be placed on a {pou_kind}"
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &anchor.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "a test is a FUNCTION, which the runner calls once, or a FUNCTION_BLOCK, \
                     which it calls cycle by cycle on an instance of its own"
                        .to_string(),
                );
                diag
            }
            Self::TestWithParameter { var, pou_kind } => {
                let mut diag = diag()
                    .message(format!(
                        "the {{test}} {pou_kind} cannot take the parameter '{}'",
                        var.name_with_case(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_name_span(db)).unwrap_or_default())
                    .call();
                diag.with_note("the runner calls a test with no arguments".to_string());
                diag.with_help(
                    "declare it in VAR, or call a FUNCTION taking it from the test".to_string(),
                );
                diag
            }
            Self::ExportOutsideFunction { anchor, pou_kind } => {
                let mut diag = diag()
                    .message(format!(
                        "an {{export}} pragma cannot be placed on a {pou_kind}"
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &anchor.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note("only a FUNCTION can be {export}".to_string());
                diag
            }
            Self::ExportForbidden { anchor, func, kind } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' cannot be exported: it {}",
                        func.get_name_with_case(db).text(db),
                        kind.message(),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &anchor.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(kind.note().to_string());
                diag
            }
            Self::ExportForbiddenSection { var, kind } => {
                let mut diag = diag()
                    .message(format!(
                        "{} '{}' {}",
                        kind.section(),
                        var.name_with_case(db).text(db),
                        kind.message(),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_name_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(kind.note().to_string());
                diag
            }
            Self::ExportReturn { func, ret } => {
                let mut diag = diag()
                    .message(format!(
                        "the return type of '{}' can only be a scalar or a STRING",
                        func.get_name_with_case(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &ret.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note("an {export} FUNCTION returns one WIT value".to_string());
                diag
            }
            Self::ExportNameNotWit { name, span } => {
                let mut diag = diag()
                    .message(format!("'{name}' is not a WIT name"))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();
                diag.with_note(
                    "an export's name is written as WIT writes one, words of lowercase \
                     letters and digits joined by '-'"
                        .to_string(),
                );
                if let Some(spelling) = crate::hir_def::pous::pragma::wit_spelling(name) {
                    diag.with_help(format!("write '{spelling}'"));
                }
                diag
            }
            Self::ExportWithoutWitName { func, word } => {
                let mut diag = diag()
                    .message(format!("'{word}' has no WIT name"))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &func.get_name_span(db)).unwrap_or_default(),
                    )
                    .call();
                diag.with_note(
                    "an export's WIT name is its namespaces' names and its own in lowercase \
                     with each '_' written '-', and a WIT name has no empty word and no \
                     letter outside ASCII"
                        .to_string(),
                );
                diag.with_help("name the export, {export 'name'}".to_string());
                diag
            }
            Self::ExportNamesCollide { func, other, name } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is exported as '{name}', which cannot be told from the export of '{}'",
                        func.get_name_with_case(db).text(db),
                        other.get_name_with_case(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &func.get_name_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "an export's WIT name is its namespaces' names and its own, joined by \
                     '-', and the component model compares two names without their '-'"
                        .to_string(),
                );
                diag.with_help("name one of them, {export 'name'}".to_string());
                diag.with_related(Related::new(
                    format!(
                        "'{}' is declared here",
                        other.get_name_with_case(db).text(db)
                    ),
                    other.get_scope_id(db).file(db),
                    other.get_name_span(db),
                ));
                diag
            }
            Self::WasmPragmaOutsideFunction { span } => diag()
                .message("a {wasm} body is only available on a FUNCTION".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::UnknownPragma { name, span } => {
                let message = match name.is_empty() {
                    true => "the pragma names nothing".to_string(),
                    false => format!("no pragma named '{name}'"),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();
                let lowered = name.to_lowercase();
                let advice = match KNOWN_PRAGMAS.contains(&lowered.as_str()) {
                    true => format!("write it in lowercase, {{{lowered}}}"),
                    false => "remove it".to_string(),
                };
                diag.with_advice(
                    Some(
                        "the pragmas are {test}, {once}, {export}, {warn}, {info}, {allow}, {extern} and {wasm}"
                            .to_string(),
                    ),
                    Some(advice),
                );
                diag
            }
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
                    "'{}' is not a parameter, a local or the return of this FUNCTION",
                    name.text(db)
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
                .message(format!("'{instruction}' takes {expected}, not {actual}"))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
        }
    }
}
