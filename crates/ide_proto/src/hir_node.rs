pub use hir::hir_def::hir_node::HirNode;

use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{ParamAssign, ParamAssignKind},
    hir_ty::ty::Type,
};

use crate::comment_index::comment_index;
use crate::handlers::document_links::replace_bracket_refs_with_links;

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
            None => return Some(String::new()),
        };
        let comment = replace_bracket_refs_with_links(db, &comment);
        if comment.is_empty() {
            Some(String::new())
        } else {
            Some(format!("\n---\n{comment}"))
        }
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

/// The task a `fb WITH task` element of `prog` names, when `offset` is on
/// the task's name.
pub fn element_task_at<'db>(
    db: &'db dyn WorkspaceDataBase,
    prog: hir::hir_def::config::ProgConfig<'db>,
    offset: usize,
) -> Option<hir::hir_def::config::TaskConfig<'db>> {
    use hir::hir_def::{config::ProgConfElement, scope::ScopeKind, semantic_index::get_scope};
    let ScopeKind::Config(config) = get_scope(db, prog.scope_id(db)).kind else {
        return None;
    };
    let path = prog
        .conf_elements(db)
        .iter()
        .find_map(|element| match element {
            ProgConfElement::FbTask(fb) => {
                let span = fb.task.get_span(db);
                (span.start_byte <= offset && offset <= span.end_byte).then_some(fb.path)
            }
            ProgConfElement::Connection(_) => None,
        })?;
    hir::hir_ty::config::prog_elements(db, config)
        .tasks
        .get(&path)
        .copied()
}

/// The resource or the program instance a step of a VAR_CONFIG path names.
pub fn config_path_step<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: hir::hir_def::expressions::expression::PathExpr<'db>,
) -> Option<hir::hir_ty::config::ConfigPathStep<'db>> {
    use hir::hir_def::{scope::ScopeKind, semantic_index::get_scope};
    let ScopeKind::Config(config) = get_scope(db, path.get_scope_id(db)).kind else {
        return None;
    };
    hir::hir_ty::config::resolve_config_entries(db, config)
        .steps
        .get(&path)
        .copied()
}
