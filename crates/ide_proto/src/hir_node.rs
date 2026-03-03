pub use hir::hir_def::hir_node::HirNode;

use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{ParamAssign, ParamAssignKind},
    hir_ty::ty::Type,
};

use crate::comment_index::comment_index;

pub trait MaybeHirNode<'db> {
    fn as_hir_node(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<&'db dyn HirNodeInfo<'db>>;
}

impl<'db> MaybeHirNode<'db> for Type<'db> {
    fn as_hir_node(
        &'db self,
        _db: &'db dyn WorkspaceDataBase,
    ) -> Option<&'db dyn HirNodeInfo<'db>> {
        match self {
            Type::Function(f) => Some(f),
            Type::FunctionBlock(fb) => Some(fb),
            Type::Class(c) => Some(c),
            Type::Interface(i) => Some(i),
            Type::DataType(dt) => Some(dt),
            Type::Variable((v, _)) => Some(v),
            Type::StructElement(st) => Some(st),
            _ => None,
        }
    }
}

pub trait HasComment<'db>: HirNodeInfo<'db> {
    fn get_comment(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<String> {
        let comment = match comment_index(db, self.get_scope_id(db).file(db)).find_nearby_comment(
            self.get_scope_id(db).file(db).document(db),
            &self.get_span(db),
        ) {
            Some(c) => c.to_string(self.get_scope_id(db).file(db).document(db)),
            None => "".to_string(),
        };
        Some(comment)
    }
}

impl<'db, T> HasComment<'db> for T where T: HirNodeInfo<'db> + ?Sized {}

pub fn get_param_start_pos<'db>(
    db: &'db dyn WorkspaceDataBase,
    param: &'db ParamAssign<'db>,
) -> Box<dyn HirNodeInfo<'db> + 'db> {
    match param.kind(db) {
        ParamAssignKind::FormalInput { param, .. } => Box::new(param) as _,
        ParamAssignKind::FormalOutput { param, .. } => Box::new(param) as _,
        ParamAssignKind::NonFormal { value } => Box::new(value) as _,
    }
}
