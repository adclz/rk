use compact_str::CompactString;
use db::WorkspaceDataBase;

use crate::hir_def::interned::identifier::SpanIdent;

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Pragma<'db> {
    Test(SpanIdent<'db>),
    Once(SpanIdent<'db>),
    /// `{export}` above a FUNCTION: it is a WASM export, under its name.
    /// Nothing else a workspace declares is, so an optimizer may drop what
    /// nothing calls.
    Export(SpanIdent<'db>),
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
            Pragma::Export(s) => s,
            Pragma::Warn(s, _) => s,
            Pragma::Extern(s, _) => s,
            Pragma::Allow(s, _) => s,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Pragma::Test(_) => "{test}",
            Pragma::Once(_) => "{once}",
            Pragma::Export(_) => "{export}",
            Pragma::Warn(_, _) => "{warn}",
            Pragma::Extern(_, _) => "{extern}",
            Pragma::Allow(_, _) => "{allow}",
        }
    }
}

/// `{extern 'module' 'name'}` above a FUNCTION: the FUNCTION is a WASM
/// import. The pragma carries only what the declaration cannot know — the
/// import's module and name. The signature IS the declaration: `VAR_INPUT`
/// become the params (copies; aggregates as a pointer to the call-entry
/// snapshot), scalar `VAR_OUTPUT` become the results in declaration order,
/// and the return type, when declared, is the LAST result. Nothing is
/// restated, so nothing can disagree.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ExternPragma {
    /// The WASM import module (e.g. "wasi:clocks/monotonic-clock@0.2.6").
    pub module: CompactString,
    /// The WASM import name (e.g. "now").
    pub name: CompactString,
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

/// Helper methods for querying a pragma list.
pub fn is_test(_db: &dyn WorkspaceDataBase, pragmas: &[Pragma<'_>]) -> bool {
    pragmas.iter().any(|p| matches!(p, Pragma::Test(_)))
}

pub fn is_once(_db: &dyn WorkspaceDataBase, pragmas: &[Pragma<'_>]) -> bool {
    pragmas.iter().any(|p| matches!(p, Pragma::Once(_)))
}

pub fn is_export(_db: &dyn WorkspaceDataBase, pragmas: &[Pragma<'_>]) -> bool {
    pragmas.iter().any(|p| matches!(p, Pragma::Export(_)))
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
