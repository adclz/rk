use auto_lsp::lsp_types::{
    GotoDefinitionResponse, InlayHint, InlayHintKind, InlayHintLabel, InlayHintLabelPart, Location,
};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        config::ConfigDecl,
        expressions::expression::{InitExpr, InitExprKind, ParamAssign, ParamAssignKind},
        hir_node::HirNode,
        namespace::NamespaceDecl,
        pous::pou::Pou,
    },
    hir_ty::{body::infer_body, infer::Infer, ty::Type},
};

use crate::{
    handlers::{DefinitionHandler, InlayHintHandler},
    hir_node::get_param_start_pos,
};

impl<'db> InlayHintHandler<'db> for HirNode<'db> {
    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        match self {
            HirNode::Namespace(n) => n.inlay_hint(db),
            HirNode::PouDecl(p) => p.inlay_hint(db),
            HirNode::Param(p) => p.inlay_hint(db),
            HirNode::InitExpr(i) => i.inlay_hint(db),
            HirNode::Config(c) => c.inlay_hint(db),
            _ => None,
        }
    }
}

impl<'db> InlayHintHandler<'db> for NamespaceDecl<'db> {
    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        Some(InlayHint {
            label: marker_label(
                "NAMESPACE",
                self.path(db).to_string(db),
                located(db, self.get_scope_id(db), &self.name_span(db)),
            ),
            position: hir::denormalize(db, self.get_scope_id(db).file(db), &self.get_span(db))
                .unwrap_or_default()
                .end,
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
            tooltip: None,
        })
    }
}

impl<'db> InlayHintHandler<'db> for Pou<'db> {
    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        Some(InlayHint {
            label: marker_label(
                match self {
                    Pou::Function(_) => "FUNCTION",
                    Pou::FunctionBlock(_) => "FUNCTION_BLOCK",
                    Pou::Class(_) => "CLASS",
                    Pou::Interface(_) => "INTERFACE",
                    Pou::DataType(_) => None?,
                },
                self.get_name_ident(db).text(db).to_string(),
                located(db, self.get_scope_id(db), &self.get_name_span(db)),
            ),
            position: hir::denormalize(db, self.get_scope_id(db).file(db), &self.get_span(db))
                .unwrap_or_default()
                .end,
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
            tooltip: None,
        })
    }
}

impl<'db> InlayHintHandler<'db> for ParamAssign<'db> {
    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        // Formal parameters already show the name in code — no hint needed.
        match self.kind(db) {
            ParamAssignKind::FormalInput { .. } | ParamAssignKind::FormalOutput { .. } => {
                return None;
            }
            ParamAssignKind::NonFormal { .. } => {}
        }

        let infer = infer_body(db, self.scope_id(db));
        infer.variable_of_param.get(self).map(|var| {
            // A variadic argument is named by position, which is not written
            // anywhere; a formal one links to the parameter it fills.
            let label = if var.variadic(db) {
                let pos = infer.variadic_position.get(self).copied().unwrap_or(0);
                InlayHintLabel::String(format!("({pos}):"))
            } else {
                InlayHintLabel::LabelParts(vec![part(
                    format!("{}:", var.name(db).text(db)),
                    located(db, var.get_scope_id(db), &var.get_name_span(db)),
                )])
            };
            InlayHint {
                position: hir::denormalize(
                    db,
                    get_param_start_pos(db, self).get_scope_id(db).file(db),
                    &get_param_start_pos(db, self).get_span(db),
                )
                .unwrap_or_default()
                .start,
                label,
                kind: Some(InlayHintKind::PARAMETER),
                padding_left: Some(false),
                padding_right: Some(true),
                text_edits: None,
                tooltip: None,
                data: None,
            }
        })
    }
}

impl<'db> InlayHintHandler<'db> for InitExpr<'db> {
    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        let typ = self.infer(db);

        match self.kind(db) {
            InitExprKind::StructElement { name, value: _ } => Some(InlayHint {
                position: hir::denormalize(db, name.get_scope_id(db).file(db), &name.get_span(db))
                    .unwrap_or_default()
                    .end,
                // The hint names the field's TYPE, so its link goes to where
                // that type is declared. Resolving the element itself would
                // send the reader back to the field the hint already sits on.
                label: type_label(
                    typ.type_name(db),
                    match typ {
                        Type::StructElement(element) => element.spec(db).definition(db, 0),
                        _ => typ.definition(db, 0),
                    },
                ),
                kind: Some(InlayHintKind::TYPE),
                padding_left: Some(false),
                padding_right: Some(false),
                text_edits: None,
                tooltip: None,
                data: None,
            }),
            _ => None,
        }
    }
}

impl<'db> InlayHintHandler<'db> for ConfigDecl<'db> {
    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        Some(InlayHint {
            label: marker_label(
                "CONFIGURATION",
                self.name(db).text(db).to_string(),
                located(db, self.get_scope_id(db), &self.get_name_span(db)),
            ),
            position: hir::denormalize(db, self.get_scope_id(db).file(db), &self.get_span(db))
                .unwrap_or_default()
                .end,
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
            tooltip: None,
        })
    }
}

/// A type's name in a hint, as a link to its declaration when it has one.
/// An elementary type is declared nowhere, so it stays plain text rather
/// than reading as a link that goes nowhere.
fn type_label(name: String, definition: Option<GotoDefinitionResponse>) -> InlayHintLabel {
    let Some(GotoDefinitionResponse::Scalar(location)) = definition else {
        return InlayHintLabel::String(format!(": {name}"));
    };
    InlayHintLabel::LabelParts(vec![
        part(": ".to_string(), None),
        part(name, Some(location)),
    ])
}

fn part(value: String, location: Option<Location>) -> InlayHintLabelPart {
    InlayHintLabelPart {
        value,
        location,
        tooltip: None,
        command: None,
    }
}

/// The marker closing a declaration, naming what it closes. The name links
/// to the header, which is the one thing a reader at `END_FUNCTION_BLOCK`
/// has scrolled away from.
fn marker_label(keyword: &str, name: String, header: Option<Location>) -> InlayHintLabel {
    InlayHintLabel::LabelParts(vec![part(format!("{keyword} "), None), part(name, header)])
}

/// Where a node is written, as a location an editor can jump to.
fn located<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: hir::hir_def::scope::ScopeId<'db>,
    range: &auto_lsp::tree_sitter::Range,
) -> Option<Location> {
    let file = scope.file(db);
    Some(Location::new(
        file.url(db).to_owned(),
        hir::denormalize(db, file, range)?,
    ))
}
