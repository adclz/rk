use auto_lsp::{core::{document_symbols_builder::DocumentSymbolsBuilder, semantic_tokens_builder::SemanticTokensBuilder}, lsp_types::{CodeLens, CompletionItem, GotoDefinitionResponse, Hover, InlayHint, request::{GotoDeclarationResponse, GotoImplementationResponse}}};
use db::WorkspaceDataBase;

pub mod code_lens;
pub mod completion;
pub mod definition;
pub mod declaration;
pub mod document_symbols;
pub mod hover;
pub mod implementation;
pub mod inlay_hint;
pub mod semantic_tokens;


pub trait SemanticTokensHandler<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    );
}

pub trait HoverHandler<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover>;
}

pub trait CompletionHandler<'db> {
    fn completion(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Vec<CompletionItem>>;
}

pub trait DefinitionHandler<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse>;
}

pub trait DeclarationHandler<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse>;
}

pub trait ImplementationHandler<'db> {
    fn implementation(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoImplementationResponse>;
}

pub trait InlayHintHandler<'db> {
    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint>;
}

pub trait CodeLensHandler<'db> {
    fn code_lens(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<CodeLens>;
}

pub trait DocumentSymbolsHandler<'db> {
    fn document_symbols(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut DocumentSymbolsBuilder,
    );
}