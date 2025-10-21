use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        GotoDefinitionResponse, Hover, HoverContents, Location, MarkupContent, MarkupKind,
        request::GotoDeclarationResponse,
    },
};
use hir::{
    HirNodeInfo,
    hir_def::expressions::spec::{Spec, SpecKind},
    hir_ty::name_res::resolve_namespace_access,
};

use crate::{HasComment, ToProtocol};

impl<'db> ToProtocol<'db> for Spec<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, _offset: usize) -> Option<Hover> {
        let comment = match self.kind(db) {
            SpecKind::Target(t) => {
                let pou = resolve_namespace_access(db, t.scope_id, t.path)?;
                pou.get_comment(db).unwrap_or_default()
            }
            SpecKind::Ref(r) => r.get_comment(db).unwrap_or_default(),
            _ => Default::default(),
        };
        let desc = self.full_type_name(db);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
{desc}
```
                "#
                ),
            }),
            range: Some(self.get_span(db).into()),
        })
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        match self.kind(db) {
            SpecKind::Target(target) => {
                let pou = resolve_namespace_access(db, target.scope_id, target.path)?;
                pou.definition(db)
            }
            SpecKind::Ref(_ref) => _ref.definition(db),
            _ => Some(GotoDefinitionResponse::Scalar(Location::new(
                self.scope_id(db).file(db).url(db).to_owned(),
                self.get_span(db).into(),
            ))),
        }
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        match self.kind(db) {
            SpecKind::Target(target) => {
                let pou = resolve_namespace_access(db, target.scope_id, target.path)?;
                pou.definition(db)
            }
            SpecKind::Ref(_ref) => _ref.declaration(db),
            _ => Some(GotoDeclarationResponse::Scalar(Location::new(
                self.scope_id(db).file(db).url(db).to_owned(),
                self.get_span(db).into(),
            ))),
        }
    }
}
