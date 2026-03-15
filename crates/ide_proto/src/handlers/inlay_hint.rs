use auto_lsp::lsp_types::{InlayHint, InlayHintKind, InlayHintLabel};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        config::ConfigDecl,
        expressions::expression::{
            InitExpr, InitExprKind, ParamAssign, ParamAssignKind,
        },
        hir_node::HirNode,
        namespace::NamespaceDecl,
        pous::pou::Pou,
    },
    hir_ty::{body::infer_body, infer::Infer},
};

use crate::{handlers::InlayHintHandler, hir_node::get_param_start_pos};

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
            label: InlayHintLabel::String(format!("NAMESPACE {}", self.path(db).to_string(db))),
            position: self.get_span(db).lsp().end,
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
            label: InlayHintLabel::String(format!(
                "{} {}",
                match self {
                    Pou::Function(_) => "FUNCTION",
                    Pou::FunctionBlock(_) => "FUNCTION_BLOCK",
                    Pou::Class(_) => "CLASS",
                    Pou::Interface(_) => "INTERFACE",
                    Pou::DataType(_) => None?,
                },
                self.get_name_ident(db).text(db)
            )),
            position: self.get_span(db).lsp().end,
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
            let label = if var.variadic(db) {
                let pos = infer
                    .variadic_position
                    .get(self)
                    .copied()
                    .unwrap_or(0);
                format!("({pos}):")
            } else {
                let name = var.name(db).text(db);
                format!("{name}:")
            };
            InlayHint {
                position: get_param_start_pos(db, self).get_span(db).lsp().start,
                label: InlayHintLabel::String(label),
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
                position: name.get_span(db).lsp().end,
                label: InlayHintLabel::String(format!(": {}", typ.type_name(db))),
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
            label: InlayHintLabel::String(format!("CONFIGURATION {}", self.name(db).text(db))),
            position: self.get_span(db).lsp().end,
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
            tooltip: None,
        })
    }
}
