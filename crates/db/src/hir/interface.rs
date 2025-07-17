use auto_lsp::{core::span::Span, default::db::BaseDatabase};

use crate::{
    hir::{
        namespace::Using,
        variable::{Spec, Variable},
    },
    ident::Ident,
    solver::fq_name::SpannedNamespaceAccess,
    to_proto::{IterToProto, SymbolInfo, ToProto},
};

#[salsa::tracked]
pub struct Interface<'db> {
    #[returns(as_ref)]
    pub extends: Option<Vec<SpannedNamespaceAccess>>,

    #[tracked]
    #[returns(ref)]
    pub using: Vec<Using<'db>>,

    #[returns(ref)]
    pub methods: Vec<Method<'db>>,
}

impl<'db> IterToProto<'db> for Interface<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self.methods(db)
            .iter()
            .map(|m| m.iter(db).map(|n| n as _))
            .flatten()
    }
}

#[salsa::tracked]
pub struct Method<'db> {
    #[returns(ref)]
    pub range: Span,
    pub name: Ident,
    #[returns(ref)]
    pub name_span: Span,
    #[returns(as_ref)]
    pub return_type: Option<Spec<'db>>,
    #[returns(ref)]
    pub variables: Vec<Variable<'db>>,
}

impl<'db> ToProto<'db> for Method<'db> {
    fn spanned(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.range(db)
    }

    fn named_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.name_span(db)
    }

    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> Option<SymbolInfo<'db>> {
        Some(
            SymbolInfo::builder()
                .kind(auto_lsp::lsp_types::SymbolKind::METHOD)
                .name(self.name(db).text(db))
                .range(self.range(db).clone())
                .name_range(self.name_span(db).clone())
                .maybe_spec(self.return_type(db).cloned())
                .build(),
        )
    }
}

impl<'db> IterToProto<'db> for Method<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self.variables(db).iter().map(|v| v as _)
    }
}
