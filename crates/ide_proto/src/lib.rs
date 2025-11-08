use auto_lsp::{
    core::document_symbols_builder::DocumentSymbolsBuilder,
    default::db::BaseDatabase,
    lsp_types::{
        CodeLens, CompletionItem, GotoDefinitionResponse, Hover, InlayHint,
        request::{GotoDeclarationResponse, GotoImplementationResponse},
    },
};
use hir::{
    HirNodeInfo, hir_def::semantic_index::HirNode
};

pub mod completions;
pub mod namespace_access;
pub mod method_ref;
pub mod namespace;
pub mod pou;
pub mod resolved_expr;
pub mod resolved_init_expr;
pub mod resolved_param;
pub mod resolved_path_element;
pub mod resolved_stmt;
pub mod resolved_using;
pub mod resolved_var_access;
pub mod spec;
pub mod struct_element;
pub mod variable;
pub mod implementation;
pub mod comment_index;

pub trait HasComment<'db>: HirNodeInfo<'db> {
    fn get_comment(&'db self, db: &'db dyn BaseDatabase) -> Option<String> {
        let comment = match comment_index::comment_index(db, self.get_scope_id(db).file(db)).find_nearby_comment(
            self.get_scope_id(db).file(db).document(db),
            &self.get_span(db),
        ) {
            Some(c) => c.to_string(self.get_scope_id(db).file(db).document(db)),
            None => "".to_string(),
        };
        Some(comment)
    }
}

impl<'db, T> HasComment<'db> for T where T: HirNodeInfo<'db> {}

pub trait ToProtocol<'db>: HirNodeInfo<'db> {
    fn document_symbols(&self, _db: &'db dyn BaseDatabase, _builder: &mut DocumentSymbolsBuilder) {}

    fn completion(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        None
    }

    fn code_lens(&self, _db: &'db dyn BaseDatabase) -> Option<CodeLens> {
        None
    }

    fn implementation(&'db self, _db: &'db dyn BaseDatabase) -> Option<GotoImplementationResponse> {
        None
    }

    fn inlay_hint(&'db self, _db: &'db dyn BaseDatabase) -> Option<InlayHint> {
        None
    }

    fn hover(&'db self, _db: &'db dyn BaseDatabase, _offset: usize) -> Option<Hover> {
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
            HirNode::SpanNamespaceAccess(s) => s,
            HirNode::PouDecl(p) => p,
            HirNode::VariableDecl(v) => v,
            HirNode::StructElement(s) => s,
            HirNode::Spec(s) => s,
            HirNode::MethodRef(m) => m,
            HirNode::Stmt(s) => s,
            HirNode::Using(u) => u,
            HirNode::ResolvedAccess(v) => v,
            HirNode::ResolvedPath(p) => p,
            HirNode::ResolvedInitExpr(i) => i,
            HirNode::Expr(e) => e,
            HirNode::ResolvedParam(p) => p,
        }
    }
}
