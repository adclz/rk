// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use compact_str::CompactString;
use db::WorkspaceDataBase;

use crate::hir_def::interned::identifier::SpanIdent;

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Pragma<'db> {
    Test(SpanIdent<'db>),
    Once(SpanIdent<'db>),
    /// `{must_call}` above a FUNCTION_BLOCK: every instance of it is to be
    /// called, at every scan.
    MustCall(SpanIdent<'db>),
    /// `{export}` above a FUNCTION: a host calls it, under its WIT name.
    /// Nothing else a workspace declares is exported, so an optimizer may
    /// drop what nothing calls.
    Export(SpanIdent<'db>, ExportPragma),
    Warn(SpanIdent<'db>, WarnPragma),
    Extern(SpanIdent<'db>, ExternPragma),
    Allow(SpanIdent<'db>, AllowPragma),
}

impl<'db> Pragma<'db> {
    /// Get the span identifier for this pragma (for diagnostics).
    pub fn span_ident(&self) -> &SpanIdent<'db> {
        match self {
            Pragma::Test(s) => s,
            Pragma::Once(s) => s,
            Pragma::MustCall(s) => s,
            Pragma::Export(s, _) => s,
            Pragma::Warn(s, _) => s,
            Pragma::Extern(s, _) => s,
            Pragma::Allow(s, _) => s,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Pragma::Test(_) => "{test}",
            Pragma::Once(_) => "{once}",
            Pragma::MustCall(_) => "{must_call}",
            Pragma::Export(_, _) => "{export}",
            Pragma::Warn(_, _) => "{warn}",
            Pragma::Extern(_, _) => "{extern}",
            Pragma::Allow(_, _) => "{allow}",
        }
    }
}

/// `{export}` or `{export 'name'}` above a FUNCTION.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ExportPragma {
    /// The WIT name the pragma gives, `{export 'read-coils'}`, with its
    /// string's span, quotes included. Without one the FUNCTION's own
    /// names give it ([`crate::hir_ty::exports::export_wit_name`]).
    pub name: Option<(CompactString, auto_lsp::tree_sitter::Range)>,
}

/// `{extern 'module' 'name'}` above a FUNCTION: the FUNCTION is a WASM
/// import. The pragma carries only what the declaration cannot know — the
/// import's module and name, written as WIT writes them. The signature IS
/// the declaration: `VAR_INPUT` become the params (copies; aggregates as a
/// pointer to the call-entry snapshot), scalar `VAR_OUTPUT` become the
/// results in declaration order, and the return type, when declared, is the
/// LAST result. Nothing is restated, so nothing can disagree. An import from
/// a module other than WASI's is typed in WIT from the declaration, which
/// takes it down to scalar and STRING inputs and the return value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ExternPragma {
    /// The WASM import module (e.g. "wasi:clocks/monotonic-clock@0.3.0").
    pub module: CompactString,
    /// The WASM import name (e.g. "now").
    pub name: CompactString,
    /// The module's string, quotes included.
    pub module_span: auto_lsp::tree_sitter::Range,
    /// The name's string, quotes included.
    pub name_span: auto_lsp::tree_sitter::Range,
}

impl ExternPragma {
    /// Whether the import is one of WASI's, its module in the `wasi`
    /// namespace. A WASI function keeps the types WASI's WIT gives it, which
    /// a declaration matches through `REF_TO` where a record comes back; any
    /// other module's are the declaration's own, in WIT's types.
    pub fn is_wasi(&self) -> bool {
        is_wasi_module(&self.module)
    }
}

/// Whether an import's module is one of WASI's: in the `wasi` namespace.
pub fn is_wasi_module(module: &str) -> bool {
    module.starts_with("wasi:")
}

/// What an `{extern}`'s module names, to the component model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportModule<'a> {
    /// A plain WIT name, `modbus`: the world imports an interface of that
    /// name, which it declares itself.
    Plain(&'a str),
    /// An interface of a package, `acme:modbus/rtu@1.0.0`.
    Interface {
        namespace: &'a str,
        package: &'a str,
        interface: &'a str,
        version: Option<&'a str>,
    },
}

/// Why an `{extern}`'s module names nothing the component model takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum ModuleNameError {
    /// Neither a WIT name nor an interface's name.
    NotAName,
    /// More than one namespace, or more than one interface: the nested
    /// names the component model's tools do not take yet.
    Nested,
    /// What follows `@` is not a semantic version.
    Version,
}

/// The module of an `{extern}` as the component model reads it: a plain WIT
/// name, or `namespace:package/interface` with an optional `@version`. Its
/// namespace and package are lowercase, as the component model wants a
/// package's name.
pub fn import_module(module: &str) -> Result<ImportModule<'_>, ModuleNameError> {
    if !module.contains([':', '/', '@']) {
        return match is_wit_name(module) {
            true => Ok(ImportModule::Plain(module)),
            false => Err(ModuleNameError::NotAName),
        };
    }
    let (path, version) = match module.split_once('@') {
        Some((path, version)) => (path, Some(version)),
        None => (module, None),
    };
    if !path.split([':', '/']).all(is_wit_name) {
        return Err(ModuleNameError::NotAName);
    }
    if path.matches(':').count() > 1 || path.matches('/').count() > 1 {
        return Err(ModuleNameError::Nested);
    }
    let Some((namespace, rest)) = path.split_once(':') else {
        return Err(ModuleNameError::NotAName);
    };
    let Some((package, interface)) = rest.split_once('/') else {
        return Err(ModuleNameError::NotAName);
    };
    let lowercase = |name: &str| !name.contains(|c: char| c.is_ascii_uppercase());
    if !lowercase(namespace) || !lowercase(package) {
        return Err(ModuleNameError::NotAName);
    }
    if version.is_some_and(|version| semver::Version::parse(version).is_err()) {
        return Err(ModuleNameError::Version);
    }
    Ok(ImportModule::Interface {
        namespace,
        package,
        interface,
        version,
    })
}

/// A WIT name, as the component model spells one: words of ASCII letters
/// and digits joined by `-`, each word all lowercase or all uppercase, the
/// first one starting with a letter (`read-coils`, `HTTP-get`, `axis-2`).
pub fn is_wit_name(name: &str) -> bool {
    !name.is_empty()
        && name.split('-').enumerate().all(|(at, word)| {
            let Some(first) = word.chars().next() else {
                return false;
            };
            let lower = word
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
            let upper = word
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
            (at > 0 || first.is_ascii_alphabetic()) && (lower || upper)
        })
}

/// A declared name as WIT writes it, a parameter's or an exported
/// FUNCTION's: its letters in lowercase, each `_` a `-`, as a WIT binding
/// writes `start-address` in Rust or C as `start_address`. `None` when that
/// is no WIT name: a word is empty (`x_`, `_x`, `a__b`), or a letter is not
/// ASCII.
pub fn wit_parameter_name(name: &str) -> Option<String> {
    let wit: String = name
        .chars()
        .map(|c| match c {
            '_' => '-',
            c => c.to_ascii_lowercase(),
        })
        .collect();
    is_wit_name(&wit).then_some(wit)
}

/// What a name WIT does not take could be written as: its words in
/// lowercase, joined by `-`, where `_` or a capital starts a word
/// (`read_coils` and `ReadCoils` are `read-coils`). `None` when that is no
/// WIT name either.
pub fn wit_spelling(name: &str) -> Option<String> {
    let mut wit = String::new();
    let mut previous: Option<char> = None;
    for c in name.chars() {
        match c {
            '_' => wit.push('-'),
            c if c.is_ascii_uppercase()
                && previous.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit()) =>
            {
                wit.push('-');
                wit.push(c.to_ascii_lowercase());
            }
            c => wit.push(c.to_ascii_lowercase()),
        }
        previous = Some(c);
    }
    (is_wit_name(&wit) && wit != name).then_some(wit)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum WarnPragmaLevel {
    Warn,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct WarnPragma {
    pub level: WarnPragmaLevel,
    pub message: CompactString,
}

/// What the module itself exports for the host: `__init`, the memory, the
/// base and size of each band, and the functions of the control interface
/// (`rk:plc/control`) with the `cabi_realloc` they take their lists by. An
/// exported FUNCTION, `{export}` or `{test}`, may not have one of these
/// names (E1509): a module with two exports of one name does not load.
/// `wasm_codegen` writes them; a codegen test checks it writes these and no
/// others.
pub const MODULE_EXPORTS: &[&str] = &[
    "__init",
    "memory",
    "cabi_realloc",
    "rk:plc/control@0.1.0#init",
    "rk:plc/control@0.1.0#run-task",
    "rk:plc/control@0.1.0#save-retain",
    "rk:plc/control@0.1.0#restore-retain",
    "rk:plc/control@0.1.0#run-test",
    "retain_base",
    "retain_size",
    "globals_base",
    "globals_size",
    "input_base",
    "input_size",
    "output_base",
    "output_size",
    "marker_base",
    "marker_size",
];

/// Helper methods for querying a pragma list.
pub fn is_test(_db: &dyn WorkspaceDataBase, pragmas: &[Pragma<'_>]) -> bool {
    pragmas.iter().any(|p| matches!(p, Pragma::Test(_)))
}

pub fn is_once(_db: &dyn WorkspaceDataBase, pragmas: &[Pragma<'_>]) -> bool {
    pragmas.iter().any(|p| matches!(p, Pragma::Once(_)))
}

pub fn is_export(_db: &dyn WorkspaceDataBase, pragmas: &[Pragma<'_>]) -> bool {
    pragmas.iter().any(|p| matches!(p, Pragma::Export(_, _)))
}

pub fn warn_pragma<'a>(pragmas: &'a [Pragma<'_>]) -> Option<&'a WarnPragma> {
    pragmas.iter().find_map(|p| match p {
        Pragma::Warn(_, w) => Some(w),
        _ => None,
    })
}

pub fn extern_pragma<'a, 'db>(
    pragmas: &'a [Pragma<'db>],
) -> Option<(&'a SpanIdent<'db>, &'a ExternPragma)> {
    pragmas.iter().find_map(|p| match p {
        Pragma::Extern(s, e) => Some((s, e)),
        _ => None,
    })
}

/// `{allow 'rule-name' ...}`: silences the named lint rules at this site. As
/// a statement it covers the NEXT statement; above a POU, the whole POU. The
/// names are `[linter.rules]` names, and each keeps its span so an unknown
/// one is underlined at the name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct AllowPragma {
    pub rules: Vec<(CompactString, auto_lsp::tree_sitter::Range)>,
}

/// `name` as a WIT name, for a project's: its words lowercase, joined by
/// `-`, a word that starts with a digit kept on the one before (WIT starts
/// each word with a letter). A name that starts with a digit, or has no
/// word, gives `plc`.
pub fn wit_name(name: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    for word in name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
    {
        let word = word.to_ascii_lowercase();
        match words.last_mut() {
            Some(last) if word.starts_with(|c: char| c.is_ascii_digit()) => last.push_str(&word),
            _ => words.push(word),
        }
    }
    match words.first() {
        Some(first) if first.starts_with(|c: char| c.is_ascii_lowercase()) => words.join("-"),
        _ => "plc".to_string(),
    }
}
