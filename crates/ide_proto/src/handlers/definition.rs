// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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
        index_graphs::{absolute_namespace_path, namespace_index},
        infer::Infer,
        ty::{CallableType, Type},
    },
};

use crate::handlers::{
    DefinitionHandler,
    completions::{resolve_namespace_prefix, try_build_namespace_path},
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
            // Both are named where they are written, and neither had an arm
            // at all: F12 on a PROGRAM's or a METHOD's own name answered
            // nothing.
            HirNode::Program(p) => Some(named_location(db, p)),
            HirNode::MethodRef(m) => Some(named_location(db, m)),
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
        namespace_definitions(db, self.path(db))
    }
}

impl<'db> DefinitionHandler<'db> for Using<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        // A USING inside a namespace may name a sibling by its relative path.
        namespace_definitions(
            db,
            absolute_namespace_path(db, self.scope_id(db), self.path(db).path(db)),
        )
    }
}

impl<'db> DefinitionHandler<'db> for Pou<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        Some(named_location(db, self))
    }
}

impl<'db> DefinitionHandler<'db> for VariableDecl<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        // On the name, the declaration IS the definition. Forwarding to the
        // type answered nothing for `p : INT`, which is what left an inlay
        // hint's link on a parameter resolving nowhere.
        let name = self.get_name_span(db);
        if (name.start_byte..=name.end_byte).contains(&offset) {
            return Some(named_location(db, self));
        }
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

            for (index, fragment) in path.path(db).fragments(db).iter().enumerate() {
                let span = path.get_fragment_ast_node(db, index).get_range().to_owned();
                accumulated.push(*fragment);

                if offset >= span.start_byte && offset <= span.end_byte {
                    let written = NamespacePath::new(db, accumulated);
                    return namespace_definitions(
                        db,
                        absolute_namespace_path(db, self.scope_id(db), written),
                    );
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
        // The resource or the instance a VAR_CONFIG path starts with.
        if ty.is_never()
            && let Some(step) = crate::hir_node::config_path_step(db, *self)
        {
            use hir::hir_ty::config::ConfigPathStep;
            let (scope, name) = match step {
                ConfigPathStep::Resource(r) => (r.get_scope_id(db), r.name(db).get_span(db)),
                ConfigPathStep::Instance(p) => (p.get_scope_id(db), p.name(db).get_span(db)),
            };
            let file = scope.file(db);
            return Some(GotoDefinitionResponse::Scalar(Location::new(
                file.url(db).to_owned(),
                hir::denormalize(db, file, &name).unwrap_or_default(),
            )));
        }
        if ty.is_never()
            && let Some(written) = try_build_namespace_path(db, self)
            && let Some(ns_path) = resolve_namespace_prefix(db, self.get_scope_id(db), written)
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

/// `ty` with its aliases followed: a variable or an alias declared as
/// another name goes to that name's declaration, as many times as it takes.
/// A cycle of aliases, `TYPE A : B; B : A;`, is E1302 where it is declared;
/// followed name by name it took the language server down. A second walker
/// two names ahead meets the first only on a cycle, and the chase then
/// stops at the alias it is on.
fn through_aliases<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Type<'db> {
    let mut slow = ty;
    let mut fast = ty;
    loop {
        for _ in 0..2 {
            fast = match aliased(db, fast) {
                Some(next) => next,
                None => return fast,
            };
        }
        slow = match aliased(db, slow) {
            Some(next) => next,
            None => return slow,
        };
        if slow == fast {
            return slow;
        }
    }
}

/// The name `ty` is declared as, when it is a variable or an alias declared
/// as another name.
fn aliased<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Option<Type<'db>> {
    let spec = match ty {
        Type::DataType(dt) => dt.spec(db),
        Type::Variable((var, _)) => var.spec(db),
        _ => return None,
    };
    matches!(spec.kind(db), SpecKind::Target(_)).then(|| spec.infer(db))
}

impl<'db> DefinitionHandler<'db> for Type<'db> {
    fn definition(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<GotoDefinitionResponse> {
        // Each arm names its holder by value: a `dyn HasName` would pin the
        // borrow of a local to `'db`.
        match through_aliases(db, *self) {
            Type::CallableType(c) | Type::ReturnValue((c, _)) => c.definition(db, _offset),
            Type::Program(p) => Some(named_location(db, &p)),
            Type::Function(f) => Some(named_location(db, &f)),
            Type::FunctionBlock(f) => Some(named_location(db, &f)),
            Type::Class(c) => Some(named_location(db, &c)),
            Type::Interface(i) => Some(named_location(db, &i)),
            Type::StructElement(st) => Some(named_location(db, &st)),
            Type::MethodDecl(m) => Some(named_location(db, &m)),
            Type::DataType(dt) => Some(named_location(db, &dt)),
            // A qualified value names the VARIANT, so it goes to where that
            // variant is written. Only the type half of `Mode#Running`
            // resolved; the half the reader clicked answered nothing.
            Type::EnumVariant(data_type, variant) => {
                let SpecKind::Enum(enm) = data_type.spec(db).kind(db) else {
                    None?
                };
                let declared = enm.enum_variants(db).get(&variant).copied()?;
                let file = data_type.get_scope_id(db).file(db);
                Some(GotoDefinitionResponse::Scalar(Location::new(
                    file.url(db).to_owned(),
                    hir::denormalize(db, file, &declared.name.get_span(db)).unwrap_or_default(),
                )))
            }
            Type::Variable((var, _multibits)) => Some(named_location(db, &var)),
            _ => None,
        }
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
            CallableType::MethodDecl(m) => Some(named_location(db, m)),
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
            hir::denormalize(db, self.get_scope_id(db).file(db), &self.get_name_span(db))
                .unwrap_or_default(),
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
            hir::denormalize(
                db,
                self.name(db).get_scope_id(db).file(db),
                &self.name(db).get_span(db),
            )
            .unwrap_or_default(),
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
        // On the task a function block runs under: `fb1 WITH FAST`.
        if let Some(task) = crate::hir_node::element_task_at(db, *self, offset) {
            return task.definition(db, task.name(db).get_span(db).start_byte);
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
                hir::denormalize(db, ns.scope_id(db).file(db), &ns.name_span(db))
                    .unwrap_or_default(),
            )
        })
        .collect();
    if decls.is_empty() {
        None
    } else {
        Some(GotoDefinitionResponse::Array(decls))
    }
}

/// A definition points at the NAME, not the whole declaration. An editor
/// asked to go to a definition it is already standing inside shows the
/// references instead, which a range covering a whole POU triggered from
/// anywhere in its body.
fn named_location<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: &'db (impl HasName<'db> + ?Sized),
) -> GotoDefinitionResponse {
    let file = node.get_scope_id(db).file(db);
    GotoDefinitionResponse::Scalar(Location::new(
        file.url(db).to_owned(),
        hir::denormalize(db, file, &node.get_name_span(db)).unwrap_or_default(),
    ))
}
