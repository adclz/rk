use auto_lsp::core::semantic_tokens_builder::SemanticTokensBuilder;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, Expr, ExprKind, PathExpr, PrimaryExpr, VariableAccess},
            spec::{Spec, SpecKind},
        },
        hir_node::HirNode,
        pous::{pou::Pou, variable::VariableDecl},
        using::Using,
    },
    hir_ty::{head::inheritance::MethodRef, infer::Infer, ty::Type},
};

use crate::{
    CLASS, ENUM, ENUM_MEMBER, FUNCTION, INTERFACE, METHOD, NAMESPACE, PARAMETER, PROPERTY, STRUCT,
    SUPPORTED_TYPES, VARIABLE,
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
            HirNode::Program(p) => {
                let file = p.get_scope_id(db).file(db);
                if let Some(range) = hir::denormalize(db, file, &p.get_name_span(db))
                    && range.start.line == range.end.line
                {
                    builder.push(range, token(FUNCTION), 0);
                }
            }
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
            semantic_tokens_for_type(db, Type::new_pou(db, pou), builder, span, file);
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
            self.get_scope_id(db).file(db),
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
            hir::denormalize(db, self.get_scope_id(db).file(db), &self.get_name_span(db))
                .unwrap_or_default(),
            SUPPORTED_TYPES.iter().position(|x| *x == METHOD).unwrap() as u32,
            0,
        );
        if let Some(ret) = self.return_type(db) {
            semantic_tokens_for_type(
                db,
                ret.infer(db),
                builder,
                ret.get_span(db),
                self.get_scope_id(db).file(db),
            );
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

        // The name being declared. Only its USES were coloured, so a VAR
        // section came back blank.
        let file = self.get_scope_id(db).file(db);
        if let Some(range) = hir::denormalize(db, file, &self.get_name_span(db))
            && range.start.line == range.end.line
        {
            builder.push(range, token(variable_kind(db, *self)), 0);
        }
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
                // A token marks a NAME. Taking the spec's own span coloured
                // `(Idle, Running)` and a whole `STRUCT ... END_STRUCT` as
                // one enum and one struct token.
                _ => return,
            },
            self.get_scope_id(db).file(db),
        );
    }
}

impl<'db> SemanticTokensHandler<'db> for BeginPathExpr<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        semantic_tokens_for_type(
            db,
            self.infer(db),
            builder,
            self.get_span(db),
            self.get_scope_id(db).file(db),
        );
    }
}

impl<'db> SemanticTokensHandler<'db> for PathExpr<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        semantic_tokens_for_type(
            db,
            self.infer(db),
            builder,
            self.get_span(db),
            self.get_scope_id(db).file(db),
        );
    }
}

impl<'db> SemanticTokensHandler<'db> for VariableAccess<'db> {
    fn semantic_tokens(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut SemanticTokensBuilder,
    ) {
        semantic_tokens_for_type(
            db,
            self.infer(db),
            builder,
            self.get_span(db),
            self.get_scope_id(db).file(db),
        );
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
                hir::denormalize(db, name.get_scope_id(db).file(db), &name.get_span(db))
                    .unwrap_or_default(),
                SUPPORTED_TYPES.iter().position(|x| *x == ENUM).unwrap() as u32,
                0,
            );
            semantic_tokens_for_type(
                db,
                typ,
                builder,
                variant.get_span(db),
                self.get_scope_id(db).file(db),
            );
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
            let span = self
                .path(db)
                .get_fragment_ast_node(db, index)
                .get_range()
                .to_owned();
            builder.push(
                hir::denormalize(db, self.get_scope_id(db).file(db), &span).unwrap_or_default(),
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
    span: auto_lsp::tree_sitter::Range,
    file: auto_lsp::default::db::file::File,
) {
    let range = hir::denormalize(db, file, &span).unwrap_or_default();
    // Semantic tokens cannot span multiple lines; skip if the span is multi-line
    // to avoid subtract-with-overflow in the builder.
    if range.start.line != range.end.line {
        return;
    }
    // What the name IS comes before what it is OF. A variable used to take
    // its type's colour, so `m : Mode` read as the enum itself.
    if let Type::Variable((var, _)) = typ {
        builder.push(range, token(variable_kind(db, var)), 0);
        return;
    }
    if let Type::StructElement(_) = typ {
        builder.push(range, token(PROPERTY), 0);
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
        Type::EnumVariant(..) => {
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

/// The legend index for a token type.
fn token(name: auto_lsp::lsp_types::SemanticTokenType) -> u32 {
    SUPPORTED_TYPES.iter().position(|x| *x == name).unwrap() as u32
}

/// A declaration the caller writes at the call site is a parameter; anything
/// else it owns is a variable.
fn variable_kind<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
) -> auto_lsp::lsp_types::SemanticTokenType {
    use hir::hir_def::pous::variable::VariableKind;
    match var.kind(db) {
        VariableKind::Input | VariableKind::Output | VariableKind::InOut => PARAMETER,
        _ => VARIABLE,
    }
}
