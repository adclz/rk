use auto_lsp::{
    core::semantic_tokens_builder::SemanticTokensBuilder,
    lsp_types::{CompletionItem, GotoDefinitionResponse, Hover, request::GotoDeclarationResponse},
};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{interned::namespace::SpanNamespaceAccess, pous::pou::Pou},
    hir_ty::name_res::resolve_namespace_access,
    query_string::scope::query_scope_items,
};

use crate::{
    CLASS, FUNCTION, INTERFACE, NAMESPACE, SUPPORTED_TYPES,
    completions::item_builder::CompletionBuilder,
    to_proto::{ToProtocol, hir_node::SpanNamespaceAccessContext},
};

impl<'db> ToProtocol<'db> for SpanNamespaceAccessContext<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, _offset: usize) -> Option<Hover> {
        match resolve_namespace_access(db, &self.get_access().path) {
            Some(resolved) => resolved.hover(db, resolved.get_name_span(db).start_byte),
            None => None,
        }
    }

    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        match resolve_namespace_access(db, &self.get_access().path) {
            Some(resolved) => resolved.declaration(db),
            None => None,
        }
    }

    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        match resolve_namespace_access(db, &self.get_access().path) {
            Some(resolved) => resolved.definition(db),
            None => None,
        }
    }

    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let mut results = vec![];

        match self {
            // Only FUNCTION_BLOCK  and CLASS can be extended
            Self::Extends(ext) => {
                let pous =
                    query_scope_items(db, &ext.to_string(db), self.get_scope_id(db), |pou| {
                        matches!(pou, Pou::FunctionBlock(_) | Pou::Class(_))
                    });

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
                let pous =
                    query_scope_items(db, &imp.to_string(db), self.get_scope_id(db), |pou| {
                        matches!(pou, Pou::Interface(_))
                    });

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

    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        match self {
            Self::Extends(ext) => {
                push_fragments(db, ext, builder);
                if let Some(resolved) = resolve_namespace_access(db, &ext.path) {
                    match resolved {
                        Pou::Class(_) => {
                            builder.push(
                                self.get_access().path.target.get_span(db).lsp(),
                                SUPPORTED_TYPES.iter().position(|x| *x == CLASS).unwrap() as u32,
                                0,
                            );
                        }
                        Pou::FunctionBlock(_) => {
                            builder.push(
                                self.get_access().path.target.get_span(db).lsp(),
                                SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32,
                                0,
                            );
                        }
                        _ => {}
                    }
                }
            }
            Self::Implements(imp) => {
                push_fragments(db, imp, builder);
                if let Some(_resolved) = resolve_namespace_access(db, &imp.path) {
                    builder.push(
                        self.get_access().path.target.get_span(db).lsp(),
                        SUPPORTED_TYPES
                            .iter()
                            .position(|x| *x == INTERFACE)
                            .unwrap() as u32,
                        0,
                    );
                }
            }
        }
    }
}

pub fn push_fragments(
    db: &dyn WorkspaceDataBase,
    access: &SpanNamespaceAccess,
    builder: &mut SemanticTokensBuilder,
) {
    if let Some(path) = &access.path.namespace {
        for (index, _) in path.fragments(db).iter().enumerate() {
            let span = path.get_fragment_ast_node(db, index).get_span().lsp();
            builder.push(
                span,
                SUPPORTED_TYPES
                    .iter()
                    .position(|x| *x == NAMESPACE)
                    .unwrap() as u32,
                0,
            );
        }
    }
}
