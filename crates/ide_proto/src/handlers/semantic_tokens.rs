use auto_lsp::core::{semantic_tokens_builder::SemanticTokensBuilder, span::Span};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, Expr, PathExpr, VariableAccess},
            spec::StructElement,
        },
        interned::namespace::{NamespaceAccess, SpanNamespaceAccess},
        pous::{pou::Pou, variable::VariableDecl},
        using::Using,
    },
    hir_ty::{
        body::infer_body,
        signature::{infer_signature, inheritance::MethodRef},
        ty::Type,
    },
};

use crate::{
    CLASS, ENUM, FUNCTION, INTERFACE, METHOD, NAMESPACE, STRUCT, SUPPORTED_TYPES,
    handlers::SemanticTokensHandler,
};

impl<'db> SemanticTokensHandler<'db> for Pou<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        semantic_tokens_for_type(
            db,
            Type::new_pou(db, *self),
            builder,
            self.get_name_span(db),
        );
    }
}

impl<'db> SemanticTokensHandler<'db> for SpanNamespaceAccess<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        let infer = infer_signature(db, self.get_scope_id(db));
        push_fragments(db, &self.path, builder);
        if let Some(resolved) = infer.namespace_access_to_pou.get(&self.path) {
            semantic_tokens_for_type(db, *resolved, builder, self.get_span(db));
        }
    }
}

impl<'db> SemanticTokensHandler<'db> for MethodRef<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        builder.push(
            self.get_name_span(db).lsp(),
            SUPPORTED_TYPES.iter().position(|x| *x == METHOD).unwrap() as u32,
            0,
        );
        if let Some(ret) = self.return_type(db) {
            let infer = infer_signature(db, self.get_scope_id(db));
            semantic_tokens_for_type(db, infer.type_of_specs[ret], builder, ret.get_span(db));
        }
    }
}

impl<'db> SemanticTokensHandler<'db> for VariableDecl<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        let infer = infer_signature(db, self.get_scope_id(db));
        semantic_tokens_for_type(
            db,
            infer.type_of_specs[&self.spec(db)],
            builder,
            self.spec(db).get_span(db),
        );
    }
}

impl<'db> SemanticTokensHandler<'db> for StructElement<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        let infer = infer_signature(db, self.get_scope_id(db));
        semantic_tokens_for_type(
            db,
            infer.type_of_specs[&self.spec(db)],
            builder,
            self.spec(db).get_span(db),
        );
    }
}

impl<'db> SemanticTokensHandler<'db> for BeginPathExpr<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        let infer = infer_body(db, self.get_scope_id(db));
        let typ = infer
            .get_type_of_begin_path_expr(db, *self)
            .unwrap_or_default();
        semantic_tokens_for_type(db, typ, builder, self.get_span(db));
    }
}

impl<'db> SemanticTokensHandler<'db> for PathExpr<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        let infer = infer_body(db, self.get_scope_id(db));
        let typ = infer.get_type_of_path_expr(db, *self).unwrap_or_default();
        semantic_tokens_for_type(db, typ, builder, self.get_span(db));
    }
}

impl<'db> SemanticTokensHandler<'db> for VariableAccess<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        let infer = infer_body(db, self.get_scope_id(db));
        let typ = infer
            .get_type_of_variable_access(db, *self)
            .unwrap_or_default();
        semantic_tokens_for_type(db, typ, builder, self.get_span(db));
    }
}

impl<'db> SemanticTokensHandler<'db> for Expr<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        if let Some(typ) = infer_signature(db, self.scope_id(db))
            .body_infer_result
            .get_type_of_expr(*self) { semantic_tokens_for_type(db, typ, builder, self.get_span(db)); }

        if let Some(typ) = infer_body(db, self.scope_id(db))
            .get_type_of_expr(*self) { semantic_tokens_for_type(db, typ, builder, self.get_span(db)); }
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

fn semantic_tokens_for_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    typ: Type<'db>,
    builder: &mut SemanticTokensBuilder,
    span: Span,
) {
    match typ.normalize(db) {
        Type::Class(_) => {
            builder.push(
                span.lsp(),
                SUPPORTED_TYPES.iter().position(|x| *x == CLASS).unwrap() as u32,
                0,
            );
        }
        Type::FunctionBlock(_) | Type::Function(_) => {
            builder.push(
                span.lsp(),
                SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32,
                0,
            );
        }
        Type::MethodDecl(_) => {
            builder.push(
                span.lsp(),
                SUPPORTED_TYPES.iter().position(|x| *x == METHOD).unwrap() as u32,
                0,
            );
        }
        Type::Interface(_) => {
            builder.push(
                span.lsp(),
                SUPPORTED_TYPES
                    .iter()
                    .position(|x| *x == INTERFACE)
                    .unwrap() as u32,
                0,
            );
        }
        Type::Struct(_) => {
            builder.push(
                span.lsp(),
                SUPPORTED_TYPES.iter().position(|x| *x == STRUCT).unwrap() as u32,
                0,
            );
        }
        Type::Enum(_) => {
            builder.push(
                span.lsp(),
                SUPPORTED_TYPES.iter().position(|x| *x == ENUM).unwrap() as u32,
                0,
            );
        }
        _ => {}
    }
}
