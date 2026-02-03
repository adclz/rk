use auto_lsp::lsp_types::CompletionItem;
use db::WorkspaceDataBase;
use hir::{HasName, HirNodeInfo, hir_ty::ty::Type};

pub trait FieldCompletion<'db> {
    fn field_completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>>;
}

impl<'db> FieldCompletion<'db> for Type<'db> {
    fn field_completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        Some(match self {
            Type::Function(_) | Type::FunctionBlock(_) | Type::Class(_) | Type::Interface(_) => {
                let def_map = match self {
                    Type::Function(f) => f.get_scope_id(db).def_map(db),
                    Type::FunctionBlock(fb) => fb.get_scope_id(db).def_map(db),
                    Type::Class(c) => c.get_scope_id(db).def_map(db),
                    Type::Interface(i) => i.get_scope_id(db).def_map(db),
                    _ => None?,
                };

                let mut items = vec![];

                def_map
                    .global_variables
                    .iter()
                    .for_each(|(_, v)| {
                        items.push(CompletionItem::new_simple(
                            v.get_name_ident(db).text(db).to_string(),
                            "".to_string(),
                        ))
                    });

                def_map
                    .declared_methods
                    .iter()
                    .for_each(|(_, m)| {
                        items.push(CompletionItem::new_simple(
                            m.get_name_ident(db).text(db).to_string(),
                            "".to_string(),
                        ))
                    });

                items   
            }
            Type::Struct(st) => st
                .elements(db)
                .iter()
                .map(|el| {
                    CompletionItem::new_simple(el.name(db).text(db).to_string(), "".to_string())
                })
                .collect(),
            Type::Enum(enm) => enm
                .variants(db)
                .iter()
                .map(|var| {
                    CompletionItem::new_simple(var.name.text(db).to_string(), "".to_string())
                })
                .collect(),
            _ => None?,
        })
    }
}
