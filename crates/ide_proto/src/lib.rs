use auto_lsp::{
    core::document_symbols_builder::DocumentSymbolsBuilder,
    default::db::BaseDatabase,
    lsp_types::{
        CodeLens, CompletionItem, GotoDefinitionResponse, Hover, InlayHint,
        request::{GotoDeclarationResponse, GotoImplementationResponse},
    },
};
use hir::{HirNodeInfo, hir_def::semantic_index::HirNode};

pub mod completions;
pub mod method_decl;
pub mod method_prot;
pub mod namespace;
pub mod pou;
pub mod resolved_expr;
pub mod resolved_init_expr;
pub mod resolved_path_expr;
pub mod resolved_stmt;
pub mod resolved_var_access;
pub mod ty;
pub mod using;
pub mod variable;

pub trait ToProtocol<'db>: HirNodeInfo<'db> {
    fn document_symbols(&self, db: &'db dyn BaseDatabase, _builder: &mut DocumentSymbolsBuilder) {}

    fn completion(
        &'db self,
        _db: &'db dyn BaseDatabase,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        None
    }

    fn code_lens(&self, _db: &'db dyn BaseDatabase) -> Option<CodeLens> {
        None
    }

    fn implementation(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoImplementationResponse> {
        None
    }

    fn inlay_hint(&'db self, _db: &'db dyn BaseDatabase) -> Option<InlayHint> {
        None
    }

    fn hover(&'db self, _db: &'db dyn BaseDatabase, _offset: Option<usize>) -> Option<Hover> {
        None
    }

    fn declaration(&'db self, _db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        None
    }

    fn definition(&'db self, _db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        None
    }
}

pub trait AsProtocol<'db> {
    fn as_proto(&'db self) -> &'db dyn ToProtocol<'db>;
}

impl<'db> AsProtocol<'db> for HirNode<'db> {
    fn as_proto(&'db self) -> &'db dyn ToProtocol<'db> {
        match self {
            HirNode::Namespace(ns) => ns,
            HirNode::Ty(ty) => ty,
            HirNode::Using(u) => u,
            HirNode::ResolvedVarResult(v) => v,
            HirNode::ResolvedPathResult(p) => p,
            HirNode::ResolvedInitExpr(i) => i,
            HirNode::ResolvedExpr(e) => e,
            HirNode::ResolvedStmt(s) => s,
        }
    }
}
