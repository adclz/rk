use auto_lsp::default::db::BaseDatabase;

use crate::{
    diagnostics::diagnostic_builder::RangeKind,
    hir::{
        expression::Expr,
        variable::{Spec, Variable},
    },
    ident::Ident,
    to_proto::{IterToProto, SymbolInfo, ToProto},
};

#[salsa::tracked(debug)]
pub struct Interface<'db> {
    #[returns(as_ref)]
    pub extends: Option<Vec<Expr<'db>>>,

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

#[salsa::tracked(debug)]
pub struct Method<'db> {
    pub range: auto_lsp::tree_sitter::Range,
    pub name: Ident,
    pub name_span: auto_lsp::tree_sitter::Range,
    #[returns(as_ref)]
    pub return_type: Option<Spec<'db>>,
    #[returns(ref)]
    pub variables: Vec<Variable<'db>>,
}

impl<'db> ToProto<'db> for Method<'db> {
    fn spanned(&'db self, db: &'db dyn BaseDatabase) -> RangeKind<'db> {
        self.range(db).into()
    }

    fn named_span(&'db self, db: &'db dyn BaseDatabase) -> RangeKind<'db> {
        self.name_span(db).into()
    }

    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> SymbolInfo<'db> {
        SymbolInfo::builder()
            .kind(auto_lsp::lsp_types::SymbolKind::METHOD)
            .name(self.name(db).text(db))
            .range(self.range(db).into())
            .name_range(self.name_span(db).into())
            .maybe_spec(self.return_type(db).cloned())
            .build()
    }
}

impl<'db> IterToProto<'db> for Method<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self.variables(db).iter().map(|v| v as _)
    }
}
