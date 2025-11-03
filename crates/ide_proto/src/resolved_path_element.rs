use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{CompletionItem, GotoDefinitionResponse, Hover, request::GotoDeclarationResponse},
};
use hir::{hir_ty::{
    inheritance_solver::{inherited_methods}, walk::{ResolvedPath, ResolvedPathKind}
}, HirNodeInfo};

use crate::{ToProtocol, completions::static_snippets::namespace};

impl<'db> ToProtocol<'db> for ResolvedPath<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        match &self.kind {
            ResolvedPathKind::Pou(p)
            | ResolvedPathKind::This(p)
            | ResolvedPathKind::Super(p)
            | ResolvedPathKind::SuperBody(p) => {
                p.hover(db, p.name_span(db).start_byte)
            },
            ResolvedPathKind::Spec(t) => t.hover(db, offset),
            ResolvedPathKind::StructElement(st) => st.hover(db, offset),
            ResolvedPathKind::Variable(v) => v.hover(db, offset),
            ResolvedPathKind::Method(m) => m.hover(db, offset),
        }
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        match &self.kind {
            ResolvedPathKind::Pou(p)
            | ResolvedPathKind::This(p)
            | ResolvedPathKind::Super(p)
            | ResolvedPathKind::SuperBody(p) => p.declaration(db),
            ResolvedPathKind::Spec(t) => t.declaration(db),
            ResolvedPathKind::StructElement(st) => st.declaration(db),
            ResolvedPathKind::Variable(v) => v.declaration(db),
            ResolvedPathKind::Method(m) => m.declaration(db),
        }
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        match &self.kind {
            ResolvedPathKind::Pou(p)
            | ResolvedPathKind::This(p)
            | ResolvedPathKind::Super(p)
            | ResolvedPathKind::SuperBody(p) => p.definition(db),
            ResolvedPathKind::Spec(t) => t.definition(db),
            ResolvedPathKind::StructElement(st) => st.definition(db),
            ResolvedPathKind::Variable(v) => v.definition(db),
            ResolvedPathKind::Method(m) => m.definition(db),
        }
    }

    fn completion(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        Some(match self.kind {
            ResolvedPathKind::Variable(v) => v.completion(db, offset)?,
            ResolvedPathKind::Pou(pou) => pou.completion(db, offset)?,
            ResolvedPathKind::Spec(spec) => spec.completion(db, offset)?,
            ResolvedPathKind::This(pou) => {
                pou.scope_id(db).def_map(db).declared_methods
                    .iter()
                    .filter_map(|(_, m)| m.completion(db, offset))
                    .flatten()
                    .collect::<Vec<_>>()
            }
            ResolvedPathKind::Super(pou) => {
                inherited_methods(db, pou)
                    .methods
                    .iter()
                    .filter_map(|(_, m)| m.method.completion(db, offset))
                    .flatten()
                    .collect::<Vec<_>>()
            }
            _ => None?
        })
    }
}
