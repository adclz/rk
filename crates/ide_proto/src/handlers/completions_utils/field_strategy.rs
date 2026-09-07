use auto_lsp::lsp_types::CompletionItem;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo, hir_def::scope::ScopeId, hir_ty::resolver::visibility::member_visible_from,
    hir_ty::ty::Type,
};

use crate::handlers::completions_utils::{
    CompletionCtx, completion_item_builder::CompletionBuilder,
};

impl<'db> CompletionCtx {
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
            }
            Type::Struct(st) => st.elements(db).iter().for_each(|el| {
                self.items.push(CompletionItem::new_simple(
                    el.name(db).text(db).to_string(),
                    "".to_string(),
                ))
            }),
            Type::Enum(enm) => enm.variants(db).iter().for_each(|var| {
                self.items.push(CompletionItem::new_simple(
                    var.name.text(db).to_string(),
                    "".to_string(),
                ))
            }),
            _ => (),
        }
        self
    }
}
