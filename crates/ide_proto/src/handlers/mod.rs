use auto_lsp::{
    core::document_symbols_builder::DocumentSymbolsBuilder,
    lsp_types::{
        CodeLens, CompletionItem, GotoDefinitionResponse, Hover, InlayHint, Location,
        WorkspaceEdit,
        request::{GotoDeclarationResponse, GotoImplementationResponse},
    },
};
use db::WorkspaceDataBase;
use hir::hir_def::semantic_index::NodeKey;

use crate::handlers::references::ReferenceLocation;

pub mod call_hierarchy;
pub mod code_lens;
pub mod completions;
pub mod completions_utils;
pub mod declaration;
pub mod definition;
pub mod document_links;
pub mod document_symbols;
pub mod hover;
pub mod implementation;
pub mod inlay_hint;
pub mod inline_value;
pub mod references;
pub mod rename;
pub mod semantic_tokens;
pub mod signature_help;
pub mod workspace_symbols;

pub trait SemanticTokensHandler<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut crate::handlers::semantic_tokens::TokenSink,
    );
}

pub trait HoverHandler<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover>;
}

pub struct CompletionRequest {
    pub offset: usize,
    pub trigger_character: Option<String>,
    pub query: String,
    pub node_index_pos: Option<NodeKey>,
    pub is_last_before: bool, // indicates if the node is the closest preceding one (e.g. for `my_var.inner.|`)
}

impl CompletionRequest {
    /// Create a new request with a different query, keeping other fields.
    pub fn with_query(&self, query: String) -> Self {
        Self {
            offset: self.offset,
            trigger_character: self.trigger_character.clone(),
            query,
            node_index_pos: self.node_index_pos,
            is_last_before: self.is_last_before,
        }
    }
}

pub trait CompletionHandler<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>>;
}

pub trait DefinitionHandler<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<GotoDefinitionResponse>;
}

pub trait DeclarationHandler<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse>;
}

pub trait ImplementationHandler<'db> {
    fn implementation(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<GotoImplementationResponse>;
}

pub trait InlayHintHandler<'db> {
    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint>;
}

pub trait CodeLensHandler<'db> {
    fn code_lens(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<CodeLens>;
}

pub trait ReferencesHandler<'db> {
    fn locations(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<Vec<ReferenceLocation>>;
    fn references(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<Vec<Location>>;
}

pub trait RenameHandler<'db> {
    fn rename(&'db self, db: &'db dyn WorkspaceDataBase, new_name: &str) -> Option<WorkspaceEdit>;
}

pub trait DocumentSymbolsHandler<'db> {
    fn document_symbols(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut DocumentSymbolsBuilder,
    );
}
