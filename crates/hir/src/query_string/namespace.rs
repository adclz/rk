use std::ops::ControlFlow;

use db::WorkspaceDataBase;

use crate::{
    hir_def::{interned::namespace::NamespacePath, namespace::NamespaceDecl},
    query_string::{
        file::file_symbol_index,
        query::{Query, SymbolKind},
    },
};

pub struct NamespaceSearchCtx {
    namespace: NamespacePath,
}

impl<'db> NamespaceSearchCtx {
    pub fn new(namespace: NamespacePath) -> Self {
        Self { namespace }
    }

    pub fn search(&self, db: &'db dyn WorkspaceDataBase) -> Vec<NamespaceDecl<'db>> {
        let mut query = Query::new(self.namespace.to_string(db));
        query.prefix();

        // Collect all POU symbol indexes from all files
        let indexes: Vec<_> = db
            .get_files()
            .iter()
            .map(|file| file_symbol_index(db, *file))
            .collect();

        let mut results = vec![];

        query.search(db, &indexes, |symbol| {
            if let SymbolKind::Namespace(ns) = symbol.kind {
                results.push(ns);
            }
            ControlFlow::Continue::<()>(())
        });

        results
    }
}
