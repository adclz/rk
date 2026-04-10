use compact_str::CompactString;
use db::WorkspaceDataBase;

use crate::hir_def::expressions::expression::ParamAssign;

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Pragma<'db> {
    Test,
    Once,
    Warn(WarnPragma),
    Case(Vec<ParamAssign<'db>>),
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
    pragmas.iter().any(|p| matches!(p, Pragma::Test))
}

pub fn is_once(_db: &dyn WorkspaceDataBase, pragmas: &[Pragma<'_>]) -> bool {
    pragmas.iter().any(|p| matches!(p, Pragma::Once))
}

pub fn warn_pragma<'a>(pragmas: &'a [Pragma<'_>]) -> Option<&'a WarnPragma> {
    pragmas.iter().find_map(|p| match p {
        Pragma::Warn(w) => Some(w),
        _ => None,
    })
}

pub fn cases<'a, 'db>(pragmas: &'a [Pragma<'db>]) -> Vec<&'a Vec<ParamAssign<'db>>> {
    pragmas
        .iter()
        .filter_map(|p| match p {
            Pragma::Case(c) => Some(c),
            _ => None,
        })
        .collect()
}
