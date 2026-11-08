// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::CompletionItem;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::invocation::{Invocation, InvocationKind},
        scope::ScopeId,
    },
    hir_ty::{
        infer::Infer,
        oop::{ancestry, class_members, instance_members},
        resolver::visibility::member_visible_from,
        ty::Type,
    },
};

use crate::handlers::completions_utils::{
    CompletionCtx, completion_item_builder::CompletionBuilder,
};

impl<'db> CompletionCtx {
    /// What `invocation` reaches: `THIS^` the members of its POU, and `SUPER^`
    /// the methods of its base, the only thing it gives access to.
    pub fn invocation_completion(
        &mut self,
        invocation: Invocation<'db>,
        db: &'db dyn WorkspaceDataBase,
    ) -> &mut Self {
        let asking = invocation.get_scope_id(db);
        let ty = invocation.infer(db);
        let base = match (invocation.kind(db), ty.as_pou(db)) {
            (InvocationKind::Super, Some(pou)) => ancestry(db, pou).base(),
            _ => return self.field_completion(ty, asking, db),
        };
        if let Some(base) = base {
            let builder = CompletionBuilder::default().with_mode(self.mode);
            let mut methods: Vec<_> = class_members(db, base)
                .methods
                .iter()
                .filter(|(_, member)| member_visible_from(db, asking, &member.method))
                .map(|(name, member)| (name.text(db), member.method))
                .collect();
            methods.sort_by_key(|(name, _)| *name);
            methods
                .iter()
                .for_each(|(_, method)| self.items.push(builder.build_method(db, method)));
        }
        self
    }

    /// The members of `ty` that `asking` may name: a PRIVATE or PROTECTED
    /// method is offered only where the checker would accept the call.
    pub fn field_completion(
        &mut self,
        ty: Type<'db>,
        asking: ScopeId<'db>,
        db: &'db dyn WorkspaceDataBase,
    ) -> &mut Self {
        let normalized = ty.normalize(db);
        match normalized {
            Type::FunctionBlock(_) | Type::Class(_) | Type::Interface(_) | Type::MethodDecl(_) => {
                let scope = match normalized {
                    Type::FunctionBlock(fb) => fb.get_scope_id(db),
                    Type::Class(c) => c.get_scope_id(db),
                    Type::Interface(i) => i.get_scope_id(db),
                    Type::MethodDecl(d) => d.get_scope_id(db),
                    _ => unreachable!(),
                };

                let def_map = scope.def_map(db);
                let builder = CompletionBuilder::default().with_mode(self.mode);

                def_map
                    .global_variables
                    .iter()
                    .for_each(|(_, v)| self.items.push(builder.build_variable(db, v)));

                def_map
                    .declared_methods
                    .iter()
                    .filter(|(_, m)| member_visible_from(db, asking, *m))
                    .for_each(|(_, m)| self.items.push(builder.build_method(db, m)));

                // What its bases hold and answer to, nearest first, as a path
                // finds them: a name of its own hides theirs.
                if let Some(pou) = normalized.as_pou(db) {
                    instance_members(db, pou)
                        .iter()
                        .rev()
                        .filter(|member| member.owner != pou)
                        .filter(|member| {
                            !def_map.global_variables.contains_key(&member.var.name(db))
                        })
                        .for_each(|member| {
                            self.items.push(builder.build_variable(db, &member.var))
                        });
                    let mut methods: Vec<_> = class_members(db, pou)
                        .inherited(pou)
                        .filter(|(_, member)| member_visible_from(db, asking, &member.method))
                        .map(|(name, member)| (name.text(db), member.method))
                        .collect();
                    methods.sort_by_key(|(name, _)| *name);
                    methods
                        .iter()
                        .for_each(|(_, method)| self.items.push(builder.build_method(db, method)));
                }
            }
            Type::Struct(st) => st.elements(db).iter().for_each(|el| {
                self.items.push(CompletionItem::new_simple(
                    el.name_with_case(db).text(db).to_string(),
                    "".to_string(),
                ))
            }),
            Type::Enum(enm) => enm.variants(db).iter().for_each(|var| {
                self.items.push(CompletionItem::new_simple(
                    var.name.as_str(db).to_string(),
                    "".to_string(),
                ))
            }),
            _ => (),
        }
        self
    }
}
