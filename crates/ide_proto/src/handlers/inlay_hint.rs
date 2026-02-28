use auto_lsp::lsp_types::{InlayHint, InlayHintKind, InlayHintLabel, Position};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::expression::{InitExpr, InitExprKind, ParamAssign}, hir_node::HirNode, namespace::NamespaceDecl, pous::pou::Pou
    },
    hir_ty::{body::infer_body, infer::Infer, ty::Type},
};

use crate::{
    handlers::InlayHintHandler,
    hir_node::{get_param_start_pos},
};

impl<'db> InlayHintHandler<'db> for HirNode<'db> {
    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        match self {
            HirNode::Namespace(n) => n.inlay_hint(db),
            HirNode::PouDecl(p) => p.inlay_hint(db),
            HirNode::Param(p) => p.inlay_hint(db),
            HirNode::InitExpr(i) => i.inlay_hint(db),
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
        let infer = infer_body(db, self.scope_id(db));
        infer.variable_of_param.get(self).and_then(|var| {
            Type::new_var(db, *var)
                .inlay_hint(db)
                .map(|inlay_hint| InlayHint {
                    position: get_param_start_pos(db, self).get_span(db).lsp().end,
                    ..inlay_hint
                })
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

impl<'db> InlayHintHandler<'db> for Type<'db> {
    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        match self {
            Type::Variable((_, _multibits)) => Some(InlayHint {
                position: Position::default(),
                label: InlayHintLabel::String(format!(": {}", self.type_name(db))),
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
