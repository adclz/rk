use auto_lsp::lsp_types::{GotoDefinitionResponse, Location};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        config::{ConfigDecl, ProgConfig, TaskConfig},
        expressions::{
            expression::{BeginPathExpr, Expr, InitExpr, ParamAssign, PathExpr, VariableAccess},
            spec::{Spec, SpecKind, StructElement},
        },
        hir_node::HirNode,
        interned::namespace::NamespacePath,
        namespace::NamespaceDecl,
        pous::{pou::Pou, variable::VariableDecl},
        scope::ScopeKind,
        semantic_index::get_scope,
        using::Using,
    },
    hir_ty::{
        config::infer_config_result,
        index_graphs::namespace_index,
        infer::Infer,
        ty::{CallableType, Type},
    },
};

use crate::handlers::{
    DefinitionHandler,
    completions::{is_namespace_prefix, try_build_namespace_path},
};

impl<'db> DefinitionHandler<'db> for HirNode<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        match self {
            HirNode::Namespace(ns) => ns.definition(db, offset),
            HirNode::Using(u) => u.definition(db, offset),
            HirNode::PouDecl(pou) => pou.definition(db, offset),
            HirNode::VariableDecl(v) => v.definition(db, offset),
            HirNode::StructElement(s) => s.definition(db, offset),
            HirNode::InitExpr(i) => i.definition(db, offset),
            HirNode::Spec(s) => s.definition(db, offset),
            HirNode::PathExpr(p) => p.definition(db, offset),
            HirNode::VariableAccess(v) => v.definition(db, offset),
            HirNode::Expr(e) => e.definition(db, offset),
            HirNode::Param(p) => p.definition(db, offset),
            HirNode::Config(c) => c.definition(db, offset),
            HirNode::Task(t) => t.definition(db, offset),
            HirNode::ProgConfig(p) => p.definition(db, offset),
            _ => None,
        }
    }
}

impl<'db> DefinitionHandler<'db> for NamespaceDecl<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        namespace_definitions(db, *self.path(db))
    }
}

impl<'db> DefinitionHandler<'db> for Using<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        namespace_definitions(db, self.path(db).path)
    }
}

impl<'db> DefinitionHandler<'db> for Pou<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        Some(GotoDefinitionResponse::Scalar(Location::new(
            self.get_scope_id(db).file(db).url(db).to_owned(),
            hir::denormalize(db, self.get_scope_id(db).file(db), &self.get_span(db)).unwrap_or_default(),
        )))
    }
}

impl<'db> DefinitionHandler<'db> for VariableDecl<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        self.spec(db).definition(db, offset)
    }
}

impl<'db> DefinitionHandler<'db> for StructElement<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        self.spec(db).definition(db, offset)
    }
}

impl<'db> DefinitionHandler<'db> for Spec<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        // Check if offset is on a namespace fragment
        if let SpecKind::Target(target) = self.kind(db)
            && let Some(path) = &target.path.namespace
        {
            let mut accumulated = vec![];

            for (index, fragment) in path.fragments(db).iter().enumerate() {
                let span = path.get_fragment_ast_node(db, index).get_range().to_owned();
                accumulated.push(*fragment);

                if offset >= span.start_byte && offset <= span.end_byte {
                    let ns_path = NamespacePath::new(db, accumulated);
                    return namespace_definitions(db, ns_path);
                }
            }
        }

        self.infer(db).definition(db, offset)
    }
}

impl<'db> DefinitionHandler<'db> for InitExpr<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        self.infer(db).definition(db, offset)
    }
}

impl<'db> DefinitionHandler<'db> for BeginPathExpr<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        self.infer(db).definition(db, offset)
    }
}

impl<'db> DefinitionHandler<'db> for PathExpr<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        let ty = self.infer(db);
        if ty.is_never()
            && let Some(ns_path) = try_build_namespace_path(db, self)
            && is_namespace_prefix(db, ns_path)
        {
            return namespace_definitions(db, ns_path);
        }
        ty.definition(db, offset)
    }
}

impl<'db> DefinitionHandler<'db> for VariableAccess<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        self.infer(db).definition(db, offset)
    }
}

impl<'db> DefinitionHandler<'db> for Expr<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        self.infer(db).definition(db, offset)
    }
}

impl<'db> DefinitionHandler<'db> for ParamAssign<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        self.infer(db).definition(db, offset)
    }
}

impl<'db> DefinitionHandler<'db> for Type<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        let loc: &'db dyn HirNodeInfo<'db> = match self {
            Type::CallableType(c) => return c.definition(db, _offset),
            Type::Program(p) => p as _,
            Type::Function(f) => f as _,
            Type::FunctionBlock(f) => f as _,
            Type::Class(c) => c as _,
            Type::Interface(i) => i as _,
            Type::StructElement(st) => st as _,
            Type::MethodDecl(m) => m as _,
            Type::DataType(dt) => match dt.spec(db).kind(db) {
                SpecKind::Target(_) => return dt.spec(db).infer(db).definition(db, _offset),
                _ => dt as _,
            },
            Type::Variable((var, _multibits)) => match var.spec(db).kind(db) {
                SpecKind::Target(_) => return var.spec(db).infer(db).definition(db, _offset),
                _ => var as _,
            },
            _ => None?,
        };

        Some(GotoDefinitionResponse::Scalar(Location::new(
            loc.get_scope_id(db).file(db).url(db).to_owned(),
            hir::denormalize(db, loc.get_scope_id(db).file(db), &loc.get_span(db)).unwrap_or_default(),
        )))
    }
}

impl<'db> DefinitionHandler<'db> for CallableType<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        match self {
            CallableType::Function(f) => Pou::Function(*f).definition(db, offset),
            CallableType::FunctionBlock(fb) => Pou::FunctionBlock(*fb).definition(db, offset),
            CallableType::MethodDecl(m) => Some(GotoDefinitionResponse::Scalar(Location::new(
                m.get_scope_id(db).file(db).url(db).to_owned(),
                hir::denormalize(db, m.get_scope_id(db).file(db), &m.get_span(db)).unwrap_or_default(),
            ))),
        }
    }
}

impl<'db> DefinitionHandler<'db> for ConfigDecl<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        Some(GotoDefinitionResponse::Scalar(Location::new(
            self.get_scope_id(db).file(db).url(db).to_owned(),
            hir::denormalize(db, self.get_scope_id(db).file(db), &self.get_name_span(db)).unwrap_or_default(),
        )))
    }
}

impl<'db> DefinitionHandler<'db> for TaskConfig<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        Some(GotoDefinitionResponse::Scalar(Location::new(
            self.get_scope_id(db).file(db).url(db).to_owned(),
            hir::denormalize(db, self.name(db).get_scope_id(db).file(db), &self.name(db).get_span(db)).unwrap_or_default(),
        )))
    }
}

impl<'db> DefinitionHandler<'db> for ProgConfig<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        // Check if cursor is on the WITH <task> reference
        if let Some(task_ref) = self.task(db) {
            let span = task_ref.get_span(db);
            if offset >= span.start_byte && offset <= span.end_byte {
                if let ScopeKind::Config(config) = get_scope(db, self.scope_id(db)).kind
                    && let Some(task) = infer_config_result(db, config).task_of_prog.get(self)
                {
                    return task.definition(db, task.name(db).get_span(db).start_byte);
                }
                return None;
            }
        }

        self.prog_type(db).infer(db).definition(db, offset)
    }
}

fn namespace_definitions(
    db: &dyn WorkspaceDataBase,
    path: NamespacePath,
) -> Option<GotoDefinitionResponse> {
    let decls: Vec<_> = namespace_index(db, path)
        .iter()
        .map(|ns| {
            Location::new(
                ns.scope_id(db).file(db).url(db).to_owned(),
                hir::denormalize(db, ns.scope_id(db).file(db), &ns.name_span(db)).unwrap_or_default(),
            )
        })
        .collect();
    if decls.is_empty() {
        None
    } else {
        Some(GotoDefinitionResponse::Array(decls))
    }
}
