use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{CompletionItem, GotoDefinitionResponse, Hover, request::GotoDeclarationResponse},
};
use hir::{
    HirNodeInfo,
    hir_def::{
        interned::namespace::SpanNamespaceAccessContext,
        pous::pou::Pou,
    },
    hir_ty::name_res::resolve_namespace_access,
    query_string::scope::query_scope_items,
};

use crate::{ToProtocol, completions::item_builder::CompletionBuilder};

impl<'db> ToProtocol<'db> for SpanNamespaceAccessContext<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, _offset: usize) -> Option<Hover> {
        match resolve_namespace_access(db, &self.get_access().path) {
            Some(resolved) => resolved.hover(db, resolved.name_span(db).start_byte),
            None => None,
        }
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        match resolve_namespace_access(db, &self.get_access().path) {
            Some(resolved) => resolved.declaration(db),
            None => None,
        }
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        match resolve_namespace_access(db, &self.get_access().path) {
            Some(resolved) => resolved.definition(db),
            None => None,
        }
    }

    fn completion(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let mut results = vec![];

        match self {
            // Only FUNCTION_BLOCK  and CLASS can be extended
            Self::Extends(ext) => {
                let pous = query_scope_items(
                    db,
                    &ext.to_string(db),
                    self.get_scope_id(db),
                    |pou| matches!(pou.pou(db), Pou::FunctionBlock(_) | Pou::Class(_)),
                );

                let builder = CompletionBuilder::default().with_import(db, self.get_scope_id(db));

                for pou in pous.local_pous {
                    results.push(builder.build_pou(db, &pou, None));
                }

                for (ns, pou) in pous.need_imports {
                    results.push(builder.build_pou(db, &pou, Some(&ns)));
                }
            }
            // Only INTERFACE can be implemented
            Self::Implements(imp) => {
                let pous = query_scope_items(
                    db,
                    &imp.to_string(db),
                    self.get_scope_id(db),
                    |pou| matches!(pou.pou(db), Pou::Interface(_)),
                );

                let builder = CompletionBuilder::default().with_import(db, self.get_scope_id(db));

                for pou in pous.local_pous {
                    results.push(builder.build_pou(db, &pou, None));
                }

                for (ns, pou) in pous.need_imports {
                    results.push(builder.build_pou(db, &pou, Some(&ns)));
                }
            }
        }
        Some(results)
    }
}
