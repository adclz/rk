use compact_str::CompactString;
use db::WorkspaceDataBase;

use crate::hir_def::expressions::expression::ParamAssign;
use crate::hir_def::interned::identifier::SpanIdent;

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Pragma<'db> {
    Test(SpanIdent<'db>),
    Once(SpanIdent<'db>),
    Warn(SpanIdent<'db>, WarnPragma),
    Case(SpanIdent<'db>, Vec<ParamAssign<'db>>),
}

impl<'db> Pragma<'db> {
    /// Get the span identifier for this pragma (for diagnostics).
    pub fn span_ident(&self) -> &SpanIdent<'db> {
        match self {
            Pragma::Test(s) => s,
            Pragma::Once(s) => s,
            Pragma::Warn(s, _) => s,
            Pragma::Case(s, _) => s,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Pragma::Test(_) => "{test}",
            Pragma::Once(_) => "{once}",
            Pragma::Warn(_, _) => "{warn}",
            Pragma::Case(_, _) => "{case}",
        }
    }
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

pub fn warn_pragma<'a>(pragmas: &'a [Pragma<'_>]) -> Option<&'a WarnPragma> {
    pragmas.iter().find_map(|p| match p {
        Pragma::Warn(_, w) => Some(w),
        _ => None,
    })
}

pub fn cases<'a, 'db>(pragmas: &'a [Pragma<'db>]) -> Vec<&'a Vec<ParamAssign<'db>>> {
    pragmas
        .iter()
        .filter_map(|p| match p {
            Pragma::Case(_, c) => Some(c),
            _ => None,
        })
        .collect()
}
