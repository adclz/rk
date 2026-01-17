use auto_lsp::lsp_types::{GotoDefinitionResponse, Hover, InlayHint, request::GotoDeclarationResponse};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{ParamAssign, ParamAssignKind},
    hir_ty::{body_inference::infer_body_scope, ty::Type},
};

use crate::{to_proto::ToProtocol, typ::TypeProto};

impl<'db> ToProtocol<'db> for ParamAssign<'db> {
    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .variable_of_param
            .get(self)
            .and_then(|var| Type::new_var(db, *var).inlay_hint(db, &*get_param_start_pos(db, self)))
    }

    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer.variable_of_param.get(self).and_then(|var| {
            Type::new_var(db, *var).hover(db, offset, &*get_param_start_pos(db, self))
        })
    }

    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .variable_of_param
            .get(self)
            .and_then(|var| Type::new_var(db, *var).declaration(db))
    }

    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .variable_of_param
            .get(self)
            .and_then(|var| Type::new_var(db, *var).definition(db))
    }
}

fn get_param_start_pos<'db>(
    db: &'db dyn WorkspaceDataBase,
    param: &'db ParamAssign<'db>,
) -> Box<dyn HirNodeInfo<'db> + 'db> {
    match param.kind(db) {
        ParamAssignKind::FormalInput { param, .. } => Box::new(param) as _,
        ParamAssignKind::FormalOutput { param, .. } => Box::new(param) as _,
        ParamAssignKind::NonFormal { value } => Box::new(value) as _,
    }
}
