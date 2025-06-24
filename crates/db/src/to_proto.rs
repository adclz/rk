use auto_lsp::{default::db::{BaseDatabase, File}, lsp_types::SymbolKind};

use crate::{diagnostics::diagnostic_builder::RangeKind, hir::namespace::PouDecl, solver::NamespacePath};

pub(crate) trait Spanned {
    fn span(&self, db: &dyn crate::BaseDatabase) -> RangeKind;
}

#[derive(bon::Builder, Clone)]
pub struct SymbolInfo<'a> {
    pub range: RangeKind<'a>,
    pub name: String,
    pub name_range: RangeKind<'a>,
    pub kind: Option<SymbolKind>,
}

pub trait ToProto<'db> {
    fn symbol_info(&'db self, db: &'db dyn crate::BaseDatabase) -> SymbolInfo<'db>;
}

pub trait IterToProto<'db> {
    fn iter(&'db self, db: &'db dyn crate::BaseDatabase) -> impl Iterator<Item = SymbolInfo<'db>>;

    fn descendant_at(&'db self, db: &'db dyn crate::BaseDatabase, offset: usize) -> Option<SymbolInfo<'db>> {
        let mut result = None;
        for node in self.iter(db) {
            let range = &node.name_range;

            if range.start_byte <= offset && offset <= range.end_byte {
                result = Some(node);
            } else {
                continue;
            }
        }
        result
    }
}