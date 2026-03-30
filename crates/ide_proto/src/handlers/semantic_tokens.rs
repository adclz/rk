use auto_lsp::core::{semantic_tokens_builder::SemanticTokensBuilder, span::Span};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, Expr, ExprKind, PathExpr, PrimaryExpr, VariableAccess},
            spec::{Spec, SpecKind},
        },
        hir_node::HirNode,
        interned::namespace::NamespaceAccess,
        pous::{pou::Pou, variable::VariableDecl},
        using::Using,
    },
    hir_ty::{head::inheritance::MethodRef, infer::Infer, ty::Type},
};

use crate::{
    CLASS, ENUM, ENUM_MEMBER, FUNCTION, INTERFACE, METHOD, NAMESPACE, STRUCT, SUPPORTED_TYPES,
    comment_index::comment_index,
    handlers::SemanticTokensHandler,
    handlers::document_links::{byte_range_to_span, find_bracket_refs, resolve_bracket_ref_to_pou},
};

impl<'db> SemanticTokensHandler<'db> for HirNode<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        match self {
            HirNode::PouDecl(p) => p.semantic_tokens(db, builder),
            HirNode::MethodRef(m) => m.semantic_tokens(db, builder),
            HirNode::VariableDecl(v) => v.semantic_tokens(db, builder),
            HirNode::Spec(v) => v.semantic_tokens(db, builder),
            // todo: The first path expr will highlight the whole path
            //HirNode::PathExpr(p) => p.semantic_tokens(db, builder),
            HirNode::VariableAccess(v) => v.semantic_tokens(db, builder),
            HirNode::Expr(e) => e.semantic_tokens(db, builder),
            _ => {}
        }
    }
}

/// Emit semantic tokens for resolved `[TypeName]` bracket references in a node's associated comment.
///
/// Only processes comments that appear above the node (not same-line) to maintain
/// document ordering required by `SemanticTokensBuilder`.
fn comment_bracket_ref_tokens<'db>(
    node: &'db dyn HirNodeInfo<'db>,
    db: &'db dyn WorkspaceDataBase,
    builder: &mut SemanticTokensBuilder,
) {
    let file = node.get_scope_id(db).file(db);
    let document = file.document(db);
    let source = document.as_str();
    let node_span = node.get_span(db);

    let comment = match comment_index(db, file).find_nearby_comment(document, &node_span) {
        Some(c) => c,
        None => return,
    };

    // Only process comments above the node for correct token ordering
    if comment.range.end_point.row >= node_span.start_point.row {
        return;
    }

    let text = match source.get(comment.range.start_byte..comment.range.end_byte) {
        Some(t) => t,
        None => return,
    };

    if !text.contains('[') {
        return;
    }

    for bref in &find_bracket_refs(text, comment.range.start_byte) {
        if let Some(pou) = resolve_bracket_ref_to_pou(db, &bref.content) {
            let content_start = bref.open_byte + 1;
            let content_end = bref.close_byte - 1;
            let span = byte_range_to_span(source, content_start, content_end);
            if let Some(enc_range) = document.ts_range_to_enc_range(&span) {
                let span: Span = enc_range.into();
                semantic_tokens_for_type(db, Type::new_pou(db, pou), builder, span);
            }
        }
    }
}

impl<'db> SemanticTokensHandler<'db> for Pou<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        comment_bracket_ref_tokens(self, db, builder);
        semantic_tokens_for_type(
            db,
            Type::new_pou(db, *self),
            builder,
            self.get_name_span(db),
        );
    }
}

impl<'db> SemanticTokensHandler<'db> for MethodRef<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        comment_bracket_ref_tokens(self, db, builder);
        builder.push(
            self.get_name_span(db).lsp(),
            SUPPORTED_TYPES.iter().position(|x| *x == METHOD).unwrap() as u32,
            0,
        );
        if let Some(ret) = self.return_type(db) {
            semantic_tokens_for_type(db, ret.infer(db), builder, ret.get_span(db));
        }
    }
}

impl<'db> SemanticTokensHandler<'db> for VariableDecl<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        comment_bracket_ref_tokens(self, db, builder);
    }
}

impl<'db> SemanticTokensHandler<'db> for Spec<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        semantic_tokens_for_type(
            db,
            self.infer(db),
            builder,
            match self.kind(db) {
                SpecKind::Target(t) => t.path.target.get_span(db),
                _ => self.get_span(db),
            },
        );
    }
}

impl<'db> SemanticTokensHandler<'db> for BeginPathExpr<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        semantic_tokens_for_type(db, self.infer(db), builder, self.get_span(db));
    }
}

impl<'db> SemanticTokensHandler<'db> for PathExpr<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        semantic_tokens_for_type(db, self.infer(db), builder, self.get_span(db));
    }
}

impl<'db> SemanticTokensHandler<'db> for VariableAccess<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        semantic_tokens_for_type(db, self.infer(db), builder, self.get_span(db));
    }
}

impl<'db> SemanticTokensHandler<'db> for Expr<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        let typ = self.infer(db);

        if let ExprKind::PrimaryExpr(PrimaryExpr::EnumValue { name, variant }) = self.expr(db) {
            builder.push(
                name.get_span(db).lsp(),
                SUPPORTED_TYPES.iter().position(|x| *x == ENUM).unwrap() as u32,
                0,
            );
            semantic_tokens_for_type(db, typ, builder, variant.get_span(db));
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

pub(crate) fn semantic_tokens_for_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    typ: Type<'db>,
    builder: &mut SemanticTokensBuilder,
    span: Span,
) {
    let range = span.lsp();
    // Semantic tokens cannot span multiple lines; skip if the span is multi-line
    // to avoid subtract-with-overflow in the builder.
    if range.start.line != range.end.line {
        return;
    }
    match typ.normalize(db) {
        Type::Class(_) => {
            builder.push(
                range,
                SUPPORTED_TYPES.iter().position(|x| *x == CLASS).unwrap() as u32,
                0,
            );
        }
        Type::FunctionBlock(_) | Type::Function(_) | Type::Program(_) => {
            builder.push(
                range,
                SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32,
                0,
            );
        }
        Type::MethodDecl(_) => {
            builder.push(
                range,
                SUPPORTED_TYPES.iter().position(|x| *x == METHOD).unwrap() as u32,
                0,
            );
        }
        Type::Interface(_) => {
            builder.push(
                range,
                SUPPORTED_TYPES
                    .iter()
                    .position(|x| *x == INTERFACE)
                    .unwrap() as u32,
                0,
            );
        }
        Type::Struct(_) => {
            builder.push(
                range,
                SUPPORTED_TYPES.iter().position(|x| *x == STRUCT).unwrap() as u32,
                0,
            );
        }
        Type::Enum(_) => {
            builder.push(
                range,
                SUPPORTED_TYPES.iter().position(|x| *x == ENUM).unwrap() as u32,
                0,
            );
        }
        Type::EnumVariant(_) => {
            builder.push(
                range,
                SUPPORTED_TYPES
                    .iter()
                    .position(|x| *x == ENUM_MEMBER)
                    .unwrap() as u32,
                0,
            );
        }
        _ => {}
    }
}
