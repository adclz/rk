use auto_lsp::core::semantic_tokens_builder::SemanticTokensBuilder;
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_def::{
    interned::namespace::NamespaceAccess, pous::{pou::Pou, variable::VariableDecl}, using::Using,
}, hir_ty::name_res::resolve_namespace_access};

use crate::{CLASS, FUNCTION, INTERFACE, NAMESPACE, SUPPORTED_TYPES, handlers::SemanticTokensHandler};

impl<'db> SemanticTokensHandler<'db> for VariableDecl<'db> {
    fn semantic_tokens(
        &'db self,
        _db: &'db dyn WorkspaceDataBase,
        _builder: &mut SemanticTokensBuilder,
    ) {
        /*match self.spec(db).tokens(db, builder) {
            Some((typ, modi)) => {
                builder.push(
                    self.spec(db).get_span(db).lsp(),
                    SUPPORTED_TYPES.iter().position(|x| *x == typ).unwrap() as u32,
                    modi,
                );
            }
            None => {}
        };*/
    }
}

impl<'db> SemanticTokensHandler<'db> for NamespaceAccess<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        push_fragments(db, self, builder);
        if let Some(resolved) = resolve_namespace_access(db, &self) {
            match resolved {
                Pou::Class(_) => {
                    builder.push(
                        self.target.get_span(db).lsp(),
                        SUPPORTED_TYPES.iter().position(|x| *x == CLASS).unwrap() as u32,
                        0,
                    );
                }
                Pou::FunctionBlock(_) => {
                    builder.push(
                        self.target.get_span(db).lsp(),
                        SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32,
                        0,
                    );
                }
                Pou::Interface(_) => {
                    builder.push(
                        self.target.get_span(db).lsp(),
                        SUPPORTED_TYPES
                            .iter()
                            .position(|x| *x == INTERFACE)
                            .unwrap() as u32,
                        0,
                    );
                }
                _ => {}
            }
        }
    }
}

impl<'db> SemanticTokensHandler<'db> for Using<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        for (index, _fragment) in self.path(db).fragments(db).iter().enumerate() {
            let span = self.path(db).get_fragment_ast_node(db, index).get_span();
            builder.push(
                span.lsp(),
                SUPPORTED_TYPES
                    .iter()
                    .position(|x| *x == NAMESPACE)
                    .unwrap() as u32,
                0,
            );
        }
    }
}

pub fn push_fragments(
    db: &dyn WorkspaceDataBase,
    access: &NamespaceAccess,
    builder: &mut SemanticTokensBuilder,
) {
    if let Some(path) = &access.namespace {
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
