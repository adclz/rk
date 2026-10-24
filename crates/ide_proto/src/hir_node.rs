// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

pub use hir::hir_def::hir_node::HirNode;

use auto_lsp::default::db::file::File;
use auto_lsp::tree_sitter;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::expression::{ParamAssign, ParamAssignKind},
        pous::function::Function,
    },
    hir_ty::{head::signature::overload_set, ty::Type},
};

use crate::comments::{doc_text, find_nearby_comment};
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

/// The documentation of the node at `span` in `file`, as hover shows it:
/// after a rule, with its `[TypeName]` references as links. Empty when the
/// node has none.
fn comment_at(db: &dyn WorkspaceDataBase, file: File, span: &tree_sitter::Range) -> String {
    let Some(comment) = find_nearby_comment(db, file, span) else {
        return String::new();
    };
    let comment = replace_bracket_refs_with_links(db, &doc_text(comment, file.document(db)));
    if comment.is_empty() {
        String::new()
    } else {
        format!("\n---\n{comment}")
    }
}

pub trait HasComment<'db>: HirNodeInfo<'db> {
    fn get_comment(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<String> {
        Some(comment_at(
            db,
            self.get_scope_id(db).file(db),
            &self.get_span(db),
        ))
    }
}

/// The documentation of a FUNCTION: its own comment, or else the first one
/// from the top among its overloads. An overloaded name is documented once,
/// above its first declaration, and above each of the others is the
/// END_FUNCTION of the one before.
pub fn function_comment<'db>(db: &'db dyn WorkspaceDataBase, function: Function<'db>) -> String {
    let at = |function: Function<'db>| {
        comment_at(
            db,
            function.get_scope_id(db).file(db),
            &function.get_span(db),
        )
    };
    let own = at(function);
    if !own.is_empty() {
        return own;
    }
    let file = function.get_scope_id(db).file(db);
    let mut overloads = overload_set(db, function);
    // In the order of the text, the function's own file first.
    overloads.sort_by_cached_key(|overload| {
        let of = overload.get_scope_id(db).file(db);
        (
            of != file,
            of.url(db).to_string(),
            overload.get_span(db).start_byte,
        )
    });
    overloads
        .into_iter()
        .map(at)
        .find(|comment| !comment.is_empty())
        .unwrap_or_default()
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
