use auto_lsp::core::semantic_tokens_builder::SemanticTokensBuilder;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{
                BeginPathExpr, Expr, ExprKind, ParamAssign, ParamAssignKind, PathExpr, PrimaryExpr,
            },
            spec::{Spec, SpecKind, StructElement},
        },
        hir_node::HirNode,
        namespace::NamespaceDecl,
        pous::{pou::Pou, variable::VariableDecl},
        using::Using,
    },
    hir_ty::{
        head::inheritance::MethodRef,
        infer::Infer,
        ty::{CallableType, Type},
    },
};

use crate::{
    CLASS, ENUM, ENUM_MEMBER, FUNCTION, INTERFACE, METHOD, NAMESPACE, PARAMETER, PROPERTY, STRUCT,
    SUPPORTED_TYPES, TYPE, VARIABLE,
    comment_index::comment_index,
    handlers::SemanticTokensHandler,
    handlers::document_links::{byte_range_to_span, find_bracket_refs, resolve_bracket_ref_to_pou},
};

/// Collects tokens, then emits them in document order.
///
/// Two nodes can legitimately name the same span - a qualified enum value is
/// an `Expr` whose qualifier the walk yields again as a `PathExpr` - and the
/// walk hands nodes over in AST order, which is not always source order. The
/// builder encodes each token as a delta from the previous one and underflows
/// on a step backwards, so ordering is established once, here, instead of
/// being an obligation on every handler. Overlapping claims on one span
/// resolve to the first one made.
#[derive(Default)]
pub struct TokenSink {
    tokens: Vec<(auto_lsp::lsp_types::Range, u32)>,
}

impl TokenSink {
    pub fn push(&mut self, range: auto_lsp::lsp_types::Range, token_type: u32, _modifiers: u32) {
        self.tokens.push((range, token_type));
    }

    pub fn drain_into(mut self, builder: &mut SemanticTokensBuilder) {
        self.tokens
            .sort_by_key(|(range, _)| (range.start.line, range.start.character));
        self.tokens
            .dedup_by_key(|(range, _)| (range.start.line, range.start.character));
        for (range, token_type) in self.tokens {
            builder.push(range, token_type, 0);
        }
    }
}

impl<'db> SemanticTokensHandler<'db> for HirNode<'db> {
    fn semantic_tokens(&'db self, db: &'db dyn WorkspaceDataBase, builder: &mut TokenSink) {
        match self {
            HirNode::Namespace(n) => n.semantic_tokens(db, builder),
            HirNode::PouDecl(p) => p.semantic_tokens(db, builder),
            HirNode::MethodRef(m) => m.semantic_tokens(db, builder),
            HirNode::VariableDecl(v) => v.semantic_tokens(db, builder),
            HirNode::StructElement(s) => s.semantic_tokens(db, builder),
            HirNode::Program(p) => push_name(db, builder, p, FUNCTION),
            HirNode::Spec(v) => v.semantic_tokens(db, builder),
            // A path is coloured one SEGMENT at a time: the walk yields each
            // step with its own span, so `pt.x` is a variable and a property,
            // not one property token over the whole text.
            HirNode::PathExpr(p) => p.semantic_tokens(db, builder),
            HirNode::Param(p) => p.semantic_tokens(db, builder),
            HirNode::Expr(e) => e.semantic_tokens(db, builder),
            _ => {}
        }
    }
}

/// Emit semantic tokens for resolved `[TypeName]` bracket references in a node's associated comment.
///
/// Only comments ABOVE the node are processed: a trailing one names a type
/// after the declaration it annotates, which is a claim on a span the
/// declaration already made.
fn comment_bracket_ref_tokens<'db>(
    node: &'db dyn HirNodeInfo<'db>,
    db: &'db dyn WorkspaceDataBase,
    builder: &mut TokenSink,
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
    fn semantic_tokens(&'db self, db: &'db dyn WorkspaceDataBase, builder: &mut TokenSink) {
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
    fn semantic_tokens(&'db self, db: &'db dyn WorkspaceDataBase, builder: &mut TokenSink) {
        comment_bracket_ref_tokens(self, db, builder);
        push_name(db, builder, self, METHOD);
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
    fn semantic_tokens(&'db self, db: &'db dyn WorkspaceDataBase, builder: &mut TokenSink) {
        comment_bracket_ref_tokens(self, db, builder);

        // The name being declared. Only its USES were coloured, so a VAR
        // section came back blank.
        push_name(db, builder, self, variable_kind(db, *self));
    }
}

impl<'db> SemanticTokensHandler<'db> for NamespaceDecl<'db> {
    fn semantic_tokens(&'db self, db: &'db dyn WorkspaceDataBase, builder: &mut TokenSink) {
        push_span(
            db,
            builder,
            self.get_scope_id(db).file(db),
            self.name_span(db),
            NAMESPACE,
        );
    }
}

impl<'db> SemanticTokensHandler<'db> for StructElement<'db> {
    fn semantic_tokens(&'db self, db: &'db dyn WorkspaceDataBase, builder: &mut TokenSink) {
        push_name(db, builder, self, PROPERTY);
    }
}

impl<'db> SemanticTokensHandler<'db> for ParamAssign<'db> {
    fn semantic_tokens(&'db self, db: &'db dyn WorkspaceDataBase, builder: &mut TokenSink) {
        // The NAME half of `p := v` / `o => v`. The value half is walked as
        // its own node. A positional argument has no name to colour.
        let param = match self.kind(db) {
            ParamAssignKind::FormalInput { param, .. }
            | ParamAssignKind::FormalOutput { param, .. } => param,
            ParamAssignKind::NonFormal { .. } => return,
        };
        push_span(
            db,
            builder,
            self.get_scope_id(db).file(db),
            param.get_span(db),
            PARAMETER,
        );
    }
}

impl<'db> SemanticTokensHandler<'db> for Spec<'db> {
    fn semantic_tokens(&'db self, db: &'db dyn WorkspaceDataBase, builder: &mut TokenSink) {
        let file = self.get_scope_id(db).file(db);
        // The variants an enum spec DECLARES. Only their uses were coloured,
        // so `(Idle, Running)` came back blank.
        if let SpecKind::Enum(e) = self.kind(db) {
            for variant in e.variants(db) {
                push_span(db, builder, file, variant.name.get_span(db), ENUM_MEMBER);
            }
            return;
        }
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
            file,
        );
    }
}

impl<'db> SemanticTokensHandler<'db> for BeginPathExpr<'db> {
    fn semantic_tokens(&'db self, db: &'db dyn WorkspaceDataBase, builder: &mut TokenSink) {
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
    fn semantic_tokens(&'db self, db: &'db dyn WorkspaceDataBase, builder: &mut TokenSink) {
        let file = self.get_scope_id(db).file(db);
        let span = self.get_span(db);
        let results = hir::hir_ty::body::infer_body(db, self.get_scope_id(db));

        // A namespace has no type, so ask what the step NAMED before asking
        // what it is worth.
        if results.path_expr_is_namespace(*self) {
            push_span(db, builder, file, span, NAMESPACE);
            return;
        }
        // A CALLEE path is re-typed as the callable it resolved to, so
        // `motor()` would colour the INSTANCE as the block it invokes. The
        // declaration each step named is recorded; read that.
        if let Some(var) = results.variable_for_path_expr(*self) {
            push_span(db, builder, file, span, variable_kind(db, var));
            return;
        }
        semantic_tokens_for_type(db, self.infer(db), builder, span, file);
    }
}

impl<'db> SemanticTokensHandler<'db> for Expr<'db> {
    fn semantic_tokens(&'db self, db: &'db dyn WorkspaceDataBase, builder: &mut TokenSink) {
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
    fn semantic_tokens(&'db self, db: &'db dyn WorkspaceDataBase, builder: &mut TokenSink) {
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
    builder: &mut TokenSink,
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
    // A CALL is typed by what it RETURNS, and `normalize` peels it to that:
    // every `f()` and `o.m()` came out the colour of an INT, which is no
    // colour at all. The name at a call site is the callee.
    if let Type::CallableType(callable) = typ {
        builder.push(
            range,
            token(match callable {
                CallableType::MethodDecl(_) => METHOD,
                _ => FUNCTION,
            }),
            0,
        );
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
        // An alias, a subrange or a sized string: named like a type, shaped
        // like whatever it wraps, so the arms above have nothing to say about
        // it. Only a DATA TYPE reaches here as a name; an expression whose
        // type is elementary carries no token at all.
        _ if matches!(typ, Type::DataType(_)) => builder.push(range, token(TYPE), 0),
        _ => {}
    }
}

/// Push a token over `span`, unless it spans more than one line: the protocol
/// has no multi-line token and the builder underflows on one.
fn push_span(
    db: &dyn WorkspaceDataBase,
    builder: &mut TokenSink,
    file: auto_lsp::default::db::file::File,
    span: auto_lsp::tree_sitter::Range,
    ty: auto_lsp::lsp_types::SemanticTokenType,
) {
    if let Some(range) = hir::denormalize(db, file, &span)
        && range.start.line == range.end.line
    {
        builder.push(range, token(ty), 0);
    }
}

/// [`push_span`] over a node's NAME.
fn push_name<'db, N: HasName<'db> + ?Sized>(
    db: &'db dyn WorkspaceDataBase,
    builder: &mut TokenSink,
    node: &'db N,
    ty: auto_lsp::lsp_types::SemanticTokenType,
) {
    push_span(
        db,
        builder,
        node.get_scope_id(db).file(db),
        node.get_name_span(db),
        ty,
    );
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
